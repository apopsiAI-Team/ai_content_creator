"""
LLM Service - Provider-agnostic base class for Claude and OpenAI.

The base class owns all the educational-content business logic
(multi-batch continuation, auto-continue when truncated, MCQ counts,
bibliography fallbacks, summary, ESCO skill review, citation polishing,
quality scoring). Provider-specific subclasses only implement the raw
streaming and completion calls plus error retry-classification.
"""
from __future__ import annotations

import asyncio
import json
import random
from abc import ABC, abstractmethod
from typing import AsyncGenerator, AsyncIterator, Callable, Optional

from ..config import settings
from ..rate_limiter import get_rate_limiter, Priority
from ..prompts.system_prompt import (
    SYSTEM_PROMPT,
    OUTLINE_PROMPT,
    EXPAND_PROMPT,
    CITATIONS_PROMPT,
    REVIEW_PROMPT,
)
from ..prompts.structure import (
    StructureConfig,
    build_system_prompt,
    build_structure_block,
    build_expand_prompt,
    build_final_reminder,
    end_sections_label,
)


# =============================================================================
# Helpers shared by all providers
# =============================================================================

def extract_json_object(text: str) -> dict:
    """Extract the outermost JSON object from text, handling surrounding prose.

    Uses brace-counting to find the correct closing brace rather than
    fragile find/rfind which breaks when JSON contains nested objects.
    """
    start = text.find("{")
    if start == -1:
        raise ValueError("No JSON object found in text")

    depth = 0
    in_string = False
    escape_next = False
    for i in range(start, len(text)):
        ch = text[i]
        if escape_next:
            escape_next = False
            continue
        if ch == "\\":
            if in_string:
                escape_next = True
            continue
        if ch == '"':
            in_string = not in_string
            continue
        if in_string:
            continue
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return json.loads(text[start:i + 1])

    return json.loads(text[start:text.rfind("}") + 1])


# =============================================================================
# Base service
# =============================================================================

class LLMService(ABC):
    """Provider-agnostic base. Subclasses implement raw streaming/completion;
    the base class owns all educational-content business logic."""

    # Subclasses override
    provider_name: str = "abstract"

    def __init__(self) -> None:
        # Conservative retry policy for transient provider overload/rate-limit issues.
        self.stream_max_retries = 5
        self.stream_backoff_base_seconds = 1.0
        self.stream_backoff_max_seconds = 16.0

    # ------------------------------------------------------------------
    # Abstract raw I/O — implemented by ClaudeService and OpenAIService.
    # ------------------------------------------------------------------
    @abstractmethod
    def stream_text(
        self,
        system: str,
        messages: list[dict],
        max_tokens: int,
        usage_out: dict,
    ) -> AsyncIterator[str]:
        """Stream text deltas as strings.

        On completion, populate ``usage_out`` in-place with:
        ``input_tokens`` (int), ``output_tokens`` (int), ``truncated`` (bool).
        Raises on error; the caller may retry transient failures.
        """

    @abstractmethod
    async def complete_text(
        self,
        system: Optional[str],
        messages: list[dict],
        max_tokens: int,
    ) -> tuple[str, dict]:
        """One-shot completion. Returns (text, {input_tokens, output_tokens})."""

    # ------------------------------------------------------------------
    # Shared retry helpers
    # ------------------------------------------------------------------
    def _is_retryable_stream_error(self, exc: Exception) -> bool:
        """Return True for transient errors where retry/backoff is appropriate."""
        status_code = getattr(exc, "status_code", None)
        if status_code in {429, 500, 502, 503, 504, 529}:
            return True

        name = exc.__class__.__name__.lower()
        message = str(exc).lower()
        retry_markers = (
            "overload",
            "overloaded",
            "rate limit",
            "timeout",
            "temporar",
            "connection",
            "network",
            "unavailable",
            "529",
            "429",
        )
        if any(marker in message for marker in retry_markers):
            return True
        if any(marker in name for marker in ("ratelimit", "timeout", "connection")):
            return True
        return False

    def _retry_delay_seconds(self, attempt: int) -> float:
        """Exponential backoff with jitter."""
        base = min(
            self.stream_backoff_base_seconds * (2 ** (attempt - 1)),
            self.stream_backoff_max_seconds,
        )
        jitter = random.uniform(0.0, 0.5)
        return base + jitter

    # ------------------------------------------------------------------
    # Stream-with-retry primitive used by all higher-level methods.
    # ------------------------------------------------------------------
    async def _stream_with_retry(
        self,
        system: str,
        messages: list[dict],
        max_tokens: int,
        usage_out: dict,
        full_text_ref: list[str],
        label: str = "stream",
    ) -> AsyncGenerator[str, None]:
        """Stream text with retry on transient errors.

        ``full_text_ref`` is a single-element list mutated in place so the
        caller can read accumulated text after the iterator finishes.
        Retries only allowed before any text has been emitted (consistent
        with the existing Claude policy).
        """
        attempt = 0
        while True:
            attempt += 1
            text_len_before_attempt = len(full_text_ref[0])
            try:
                async for chunk in self.stream_text(system, messages, max_tokens, usage_out):
                    full_text_ref[0] += chunk
                    yield chunk
                return
            except Exception as exc:
                emitted_any_text = len(full_text_ref[0]) > text_len_before_attempt
                can_retry = (
                    not emitted_any_text
                    and attempt < self.stream_max_retries
                    and self._is_retryable_stream_error(exc)
                )
                if can_retry:
                    delay = self._retry_delay_seconds(attempt)
                    print(
                        f"{self.provider_name} {label} transient failure "
                        f"(attempt {attempt}/{self.stream_max_retries - 1} retries): "
                        f"{exc}. Retrying in {delay:.1f}s..."
                    )
                    await asyncio.sleep(delay)
                    continue
                raise

    # ------------------------------------------------------------------
    # Multi-pass content generation (non-streaming)
    # ------------------------------------------------------------------
    async def generate_content_multipass(
        self,
        module: dict,
        references: list[dict],
        on_progress: Optional[Callable[[str, str], None]] = None,
    ) -> dict:
        """Pass 1: Outline → Pass 2: Expand → Pass 3: Citations & Polish."""
        formatted_refs = self._format_references(references)
        module_context = self._format_module(module)

        if on_progress:
            on_progress("pass1", "Δημιουργία δομής...")
        outline = await self._generate_outline(module_context, formatted_refs)

        if on_progress:
            on_progress("pass2", "Ανάπτυξη περιεχομένου...")
        expanded = await self._expand_content(module_context, outline, formatted_refs)

        if on_progress:
            on_progress("pass3", "Προσθήκη αναφορών και τελική επεξεργασία...")
        final = await self._add_citations(expanded, formatted_refs)

        quality_score = await self._quality_check(final)

        return {
            "content": final,
            "outline": outline,
            "references": references,
            "quality_score": quality_score,
        }

    async def _generate_outline(self, module_context: str, references: str) -> str:
        prompt = f"""
{OUTLINE_PROMPT}

## ΕΝΟΤΗΤΑ
{module_context}

## ΔΙΑΘΕΣΙΜΕΣ ΑΝΑΦΟΡΕΣ
{references}

Δημιούργησε τη δομή σε JSON format.
"""
        text, _ = await self.complete_text(
            system=SYSTEM_PROMPT,
            messages=[{"role": "user", "content": prompt}],
            max_tokens=4000,
        )
        return text

    async def _expand_content(
        self, module_context: str, outline: str, references: str
    ) -> str:
        prompt = f"""
{EXPAND_PROMPT}

## ΕΝΟΤΗΤΑ
{module_context}

## ΔΟΜΗ ΠΟΥ ΠΡΕΠΕΙ ΝΑ ΑΚΟΛΟΥΘΗΣΕΙΣ
{outline}

## ΔΙΑΘΕΣΙΜΕΣ ΑΝΑΦΟΡΕΣ
{references}

Ανέπτυξε το πλήρες περιεχόμενο ακολουθώντας τη δομή.
"""
        text, _ = await self.complete_text(
            system=SYSTEM_PROMPT,
            messages=[{"role": "user", "content": prompt}],
            max_tokens=settings.max_tokens,
        )
        return text

    async def _add_citations(self, content: str, references: str) -> str:
        prompt = CITATIONS_PROMPT.format(references=references) + f"""

## ΠΕΡΙΕΧΟΜΕΝΟ ΓΙΑ ΕΠΕΞΕΡΓΑΣΙΑ
{content}

Πρόσθεσε τις αναφορές και κάνε την τελική επεξεργασία.
"""
        text, _ = await self.complete_text(
            system=SYSTEM_PROMPT,
            messages=[{"role": "user", "content": prompt}],
            max_tokens=settings.max_tokens,
        )
        return text

    async def _quality_check(self, content: str) -> dict:
        prompt = f"""
Αξιολόγησε την ποιότητα του παρακάτω εκπαιδευτικού υλικού.

Βαθμολόγησε 1-10:
1. Ακαδημαϊκό ύφος
2. Πλήρεις παράγραφοι (όχι μόνο bullets)
3. In-text citations
4. Δομή και ροή
5. Κάλυψη θέματος

## ΠΕΡΙΕΧΟΜΕΝΟ
{content[:10000]}

Απάντησε σε JSON:
{{
  "academic_style": 1-10,
  "paragraph_quality": 1-10,
  "citations": 1-10,
  "structure": 1-10,
  "coverage": 1-10,
  "overall": 1-10,
  "notes": "..."
}}
"""
        text, _ = await self.complete_text(
            system=None,
            messages=[{"role": "user", "content": prompt}],
            max_tokens=1000,
        )
        try:
            return extract_json_object(text)
        except Exception:
            return {"overall": 0, "notes": "Quality check failed"}

    # ------------------------------------------------------------------
    # Revision streaming — targeted edits to an existing draft
    # ------------------------------------------------------------------
    async def _revision_stream(
        self,
        module: dict,
        experimental_mode: bool,
        user_instructions: str,
        target_pages: Optional[int],
        learning_outcomes: str,
        keywords: str,
        current_draft: str,
        user_id: str,
        structure_config: Optional[StructureConfig] = None,
    ) -> AsyncGenerator[str, None]:
        """Apply targeted user changes to ``current_draft`` and stream the
        full revised batch. Unchanged sections are copied verbatim.

        Caching: system prompt is wrapped with cache_control by the provider
        (`_cacheable_system`). The draft itself sits in the assistant turn
        and changes between revisions, so message-level cache hits don't
        accrue across rounds — only the system prefix is cached.
        """
        module_context = self._format_module(
            module,
            target_pages=target_pages,
            learning_outcomes=learning_outcomes,
            keywords=keywords,
        )

        cfg = structure_config or StructureConfig()
        system_prompt = build_system_prompt(cfg, experimental_mode)

        setup_prompt = f"""Παρακάτω είναι το πλαίσιο της ενότητας. Έχεις ήδη παράγει ένα draft εκπαιδευτικού υλικού και θα ακολουθήσει αίτημα στοχευμένης αναθεώρησης.

## ΕΝΟΤΗΤΑ
{module_context}

Περίμενε το draft και τις οδηγίες αναθεώρησης."""

        instruction = (user_instructions or "").strip() or "(δεν δόθηκαν συγκεκριμένες οδηγίες)"

        revision_prompt = f"""Εφάρμοσε ΣΤΟΧΕΥΜΕΝΕΣ αλλαγές στο παραπάνω draft.

## ΖΗΤΟΥΜΕΝΕΣ ΑΛΛΑΓΕΣ
{instruction}

## ΚΑΝΟΝΕΣ ΑΝΑΘΕΩΡΗΣΗΣ

### Τι αλλάζει
- Εφάρμοσε τις αλλαγές που ζητήθηκαν παραπάνω.
- Αν η ζητούμενη αλλαγή χρειάζεται νέες ακαδημαϊκές πηγές για να εμπλουτιστεί ουσιαστικά το περιεχόμενο, **ΕΠΙΤΡΕΠΕΤΑΙ** να προστεθούν, **ΥΠΟ ΤΟΝ ΟΡΟ**:
  - Κάθε νέο in-text citation (Επώνυμο, Έτος) ΠΡΕΠΕΙ να έχει αντίστοιχη πλήρη βιβλιογραφική εγγραφή (APA 7th) στη Βιβλιογραφία.
  - Διατήρησε την αλφαβητική ταξινόμηση στη λίστα Βιβλιογραφίας.
  - Επιτρέπονται: peer-reviewed papers, βιβλία, επίσημες αναφορές οργανισμών, **διδακτορικές διατριβές**. ΟΧΙ πτυχιακές/μεταπτυχιακές.

### Τι μένει ως έχει
- ΟΛΟ το υπόλοιπο κείμενο που δεν αφορούν οι ζητούμενες αλλαγές ΠΡΕΠΕΙ να μείνει ΑΥΤΟΛΕΞΕΙ ίδιο — αντίγραψέ το χαρακτήρα-προς-χαρακτήρα.
- ΜΗΝ ξαναγράψεις, ΜΗΝ βελτιώσεις, ΜΗΝ αναδομήσεις σημεία που δεν ζητήθηκε να αλλάξουν.
- ΜΗΝ αφαιρέσεις ή τροποποιήσεις **υπάρχουσες** in-text citations ή βιβλιογραφικές εγγραφές, εκτός αν ζητείται ρητά.
- Διατήρησε ΑΚΡΙΒΩΣ ίδια: Ερωτήσεις Αυτοαξιολόγησης, Απαντήσεις, Γλωσσάρι — εκτός αν ζητείται ρητά αλλαγή.
- Διατήρησε την ίδια αρίθμηση υποενοτήτων, τους ίδιους τίτλους, τις ίδιες κεφαλίδες.
- ΜΗΝ προσθέσεις νέες υποενότητες παρά μόνο αν ζητήθηκε ρητά.

### Μορφή output
- ΜΗΝ προσθέσεις εισαγωγικά σχόλια, επεξηγήσεις ή σύνοψη αλλαγών στην απάντηση.
- Επέστρεψε το ΠΛΗΡΕΣ αναθεωρημένο τμήμα, από την πρώτη μέχρι την τελευταία γραμμή — όχι μόνο τα τμήματα που άλλαξαν."""

        messages = [
            {"role": "user", "content": setup_prompt},
            {"role": "assistant", "content": current_draft},
            {"role": "user", "content": revision_prompt},
        ]

        rate_limiter = get_rate_limiter()
        # Estimated output ≈ size of the draft (revised version returned in full).
        est_output = max(8000, min(len(current_draft) // 3, 60000))

        async with rate_limiter.throttle(user_id, Priority.HEAVY, estimated_output=est_output):
            full_text_ref: list[str] = [""]
            usage: dict = {}
            async for chunk in self._stream_with_retry(
                system_prompt, messages, settings.max_tokens, usage, full_text_ref, label="revision"
            ):
                yield chunk

            await rate_limiter.tracker.record_usage(
                int(usage.get("input_tokens", 0)),
                int(usage.get("output_tokens", 0)),
            )

    # ------------------------------------------------------------------
    # Streaming content generation (single-pass) — the main hot path
    # ------------------------------------------------------------------
    async def generate_content_stream(
        self,
        module: dict,
        references: list[dict],
        experimental_mode: bool = False,
        user_instructions: str = "",
        target_pages: Optional[int] = None,
        learning_outcomes: str = "",
        keywords: str = "",
        previous_content: str = "",
        batch_number: int = 1,
        total_batches: int = 1,
        user_id: str = "anonymous",
        mode: str = "generate",
        current_draft: str = "",
        occupation: Optional[dict] = None,
        structure_config: Optional[StructureConfig] = None,
    ) -> AsyncGenerator[str, None]:
        """Stream content generation with auto-continuation if truncated.

        When ``mode == "revision"`` the existing ``current_draft`` is sent
        back to the model as an assistant turn followed by a targeted
        user instruction; the model returns the full revised batch, keeping
        everything that wasn't asked to change verbatim. No continuation
        blocks, MCQ counts, or auto-continue logic apply in revision mode.
        """
        if mode == "revision":
            async for chunk in self._revision_stream(
                module=module,
                experimental_mode=experimental_mode,
                user_instructions=user_instructions,
                target_pages=target_pages,
                learning_outcomes=learning_outcomes,
                keywords=keywords,
                current_draft=current_draft,
                user_id=user_id,
                structure_config=structure_config,
            ):
                yield chunk
            return

        cfg = structure_config or StructureConfig()
        formatted_refs = self._format_references(references)
        module_context = self._format_module(
            module,
            target_pages=target_pages,
            learning_outcomes=learning_outcomes,
            keywords=keywords,
            occupation=occupation,
        )

        page_target = target_pages or 20
        is_standard_mode = not module.get("skills")

        # Dynamic MCQ count: 20 total — proportional per batch
        if total_batches <= 1:
            per_batch = 20
        else:
            per_batch = max(5, 20 // total_batches + (1 if batch_number <= 20 % total_batches else 0))
        mcq_instruction = f"ΑΚΡΙΒΩΣ {per_batch} ΑΡΙΘΜΗΜΕΝΕΣ ερωτήσεις πολλαπλής επιλογής (1. [Ερώτηση] α) β) γ) δ)"

        # Optional user instructions block
        instructions_block = ""
        if user_instructions.strip():
            instructions_block = f"""

## ΟΔΗΓΙΕΣ ΧΡΗΣΤΗ (ΑΚΟΛΟΥΘΗΣΕ ΤΕΣ)
{user_instructions.strip()}
"""

        # Continuation block if previous content exists (batch > 1)
        continuation_block = ""
        if previous_content.strip() and batch_number > 1:
            headings = [
                line.strip()
                for line in previous_content.split("\n")
                if line.strip().startswith("#")
            ]
            heading_outline = "\n".join(headings) if headings else "Δεν βρέθηκαν επικεφαλίδες."
            last_context = previous_content[-3000:]

            continuation_block = f"""

## ΚΡΙΣΙΜΟ: ΑΥΤΟ ΕΙΝΑΙ ΤΜΗΜΑ {batch_number} ΑΠΟ ~{total_batches} (ΣΥΝΕΧΕΙΑ)

ΠΡΕΠΕΙ να ΣΥΝΕΧΙΣΕΙΣ ακριβώς από εκεί που σταμάτησε το προηγούμενο τμήμα.
ΜΗΝ ξεκινήσεις από την αρχή. ΜΗΝ επαναλάβεις ορισμούς ή θέματα που ήδη καλύφθηκαν.
ΜΗΝ γράψεις Εισαγωγή — η Εισαγωγή γράφτηκε στο Τμήμα 1.
ΜΗΝ γράψεις Σκοπό, Προσδοκώμενα Αποτελέσματα ή Λέξεις Κλειδιά — γράφτηκαν στο Τμήμα 1.

### ΑΠΑΓΟΡΕΥΕΤΑΙ Η ΕΠΑΝΑΛΗΨΗ — Οι παρακάτω ενότητες ΗΔΗ γράφτηκαν:
{heading_outline}

ΜΗΝ ξαναγράψεις ΚΑΜΙΑ από τις παραπάνω ενότητες. Η αρίθμηση υποενοτήτων
ΠΡΕΠΕΙ να ΣΥΝΕΧΙΣΕΙ από εκεί που σταμάτησε (π.χ. αν τελείωσε στην 1.17, ξεκίνα από 1.18).

### ΤΕΛΟΣ ΠΡΟΗΓΟΥΜΕΝΟΥ ΤΜΗΜΑΤΟΣ (ΣΥΝΕΧΙΣΕ ΑΠΟ ΕΔΩ):
{last_context}
"""

        # Standard mode structure block (config-aware: only active elements)
        structure_block = ""
        if is_standard_mode:
            structure_block = f"\n{build_structure_block(cfg)}\n"

        batch_title = ""
        if batch_number > 1:
            batch_title = f"\nΤίτλος: # Ενότητα {module.get('number', '')}: {module.get('title', '')} (Συνέχεια)\n"

        final_reminder = build_final_reminder(cfg, mcq_instruction)

        # In-text citation density directive (experimental body) — gated by config
        citation_directive = "Κάθε 2-3 παράγραφοι ΠΡΕΠΕΙ να έχουν παραπομπή." if cfg.in_text_citations else ""

        # Choose system prompt + user prompt based on mode
        if experimental_mode:
            system_prompt = build_system_prompt(cfg, experimental=True)
            prompt = f"""
Δημιούργησε πλήρες εκπαιδευτικό υλικό για την παρακάτω ενότητα.
{structure_block}{batch_title}
## ΕΝΟΤΗΤΑ
{module_context}
{instructions_block}{continuation_block}
Χρησιμοποίησε ΠΡΑΓΜΑΤΙΚΕΣ ακαδημαϊκές πηγές από τη γνώση σου.
{citation_directive}

Δημιούργησε ΠΕΡΙΠΟΥ {page_target} σελίδες (~{page_target * 3000} χαρακτήρες) ακαδημαϊκού περιεχομένου.
ΜΕΓΙΣΤΟ ΟΡΙΟ: {page_target + 5} σελίδες (~{(page_target + 5) * 3000} χαρακτήρες) — ΜΗΝ υπερβείς αυτό το όριο.
Γράψε αναλυτικά, με πλήρεις παραγράφους 200+ λέξεων.
{final_reminder}"""
        else:
            system_prompt = build_system_prompt(cfg)
            prompt = f"""
{build_expand_prompt(cfg)}
{structure_block}{batch_title}
## ΕΝΟΤΗΤΑ
{module_context}

## ΔΙΑΘΕΣΙΜΕΣ ΑΝΑΦΟΡΕΣ (ΧΡΗΣΙΜΟΠΟΙΗΣΕ ΜΟΝΟ ΑΥΤΕΣ)
{formatted_refs}
{instructions_block}{continuation_block}
Δημιούργησε πλήρες εκπαιδευτικό υλικό ΠΕΡΙΠΟΥ {page_target} σελίδες (~{page_target * 3000} χαρακτήρες).
ΜΕΓΙΣΤΟ ΟΡΙΟ: {page_target + 5} σελίδες (~{(page_target + 5) * 3000} χαρακτήρες) — ΜΗΝ υπερβείς αυτό το όριο.
Γράψε αναλυτικά, με πλήρεις παραγράφους 200+ λέξεων.
{final_reminder}"""

        rate_limiter = get_rate_limiter()
        total_input_tokens = 0
        total_output_tokens = 0

        async with rate_limiter.throttle(user_id, Priority.HEAVY, estimated_output=30000):
            full_text_ref: list[str] = [""]
            was_truncated = False

            # First streaming attempt (with retry on transient failures)
            usage: dict = {}
            messages = [{"role": "user", "content": prompt}]
            async for chunk in self._stream_with_retry(
                system_prompt, messages, settings.max_tokens, usage, full_text_ref, label="stream"
            ):
                yield chunk

            total_input_tokens += int(usage.get("input_tokens", 0))
            total_output_tokens += int(usage.get("output_tokens", 0))
            if usage.get("truncated"):
                was_truncated = True

            # Auto-continue if truncated
            if was_truncated:
                full_text = full_text_ref[0]
                has_mcqs = "## Ερωτήσεις Αυτοαξιολόγησης" in full_text or "## Ερωτήσεις αυτοαξιολόγησης" in full_text
                has_biblio = "## Βιβλιογραφία" in full_text or "## ΒΙΒΛΙΟΓΡΑΦΙΑ" in full_text
                has_glossary = "## Γλωσσάρι" in full_text or "## ΓΛΩΣΣΑΡΙ" in full_text

                # Only re-request sections that are actually enabled — otherwise
                # the auto-continue would re-inject an element the user disabled.
                missing_parts = []
                if cfg.self_assessment and not has_mcqs:
                    missing_parts.append(f"- {mcq_instruction} + Απαντήσεις (1. α, 2. β, κ.ο.κ.)")
                if not has_biblio:
                    missing_parts.append("- ## Βιβλιογραφία (APA 7th, όλες οι αναφορές)")
                if cfg.glossary and not has_glossary:
                    missing_parts.append("- ## Γλωσσάρι (αλφαβητικά, βασικοί όροι)")

                last_context = full_text[-2000:]
                continuation_prompt = f"""ΣΥΝΕΧΙΣΕ ΑΚΡΙΒΩΣ από εκεί που σταμάτησες. Το κείμενο κόπηκε.

### ΤΕΛΕΥΤΑΙΟ ΚΟΜΜΑΤΙ ΠΟΥ ΕΓΡΑΨΕΣ:
...{last_context}

### ΣΥΝΕΧΙΣΕ ΑΠΟ ΕΔΩ.
Ολοκλήρωσε την τρέχουσα πρόταση/παράγραφο και μετά πρόσθεσε τα ενότητες που λείπουν:
{chr(10).join(missing_parts) if missing_parts else "Ολοκλήρωσε το κείμενο."}

ΜΗΝ επαναλάβεις περιεχόμενο που ήδη γράφτηκε. Ξεκίνα ακριβώς από εκεί που κόπηκε."""

                cont_messages = [
                    {"role": "user", "content": prompt},
                    {"role": "assistant", "content": full_text[-4000:]},
                    {"role": "user", "content": continuation_prompt},
                ]
                cont_usage: dict = {}
                async for chunk in self._stream_with_retry(
                    system_prompt, cont_messages, settings.max_tokens, cont_usage,
                    full_text_ref, label="continuation",
                ):
                    yield chunk
                total_input_tokens += int(cont_usage.get("input_tokens", 0))
                total_output_tokens += int(cont_usage.get("output_tokens", 0))

            # Auto-continue if content is significantly shorter than target
            estimated_pages = len(full_text_ref[0]) // 3000
            if estimated_pages < page_target * 0.85 and not was_truncated:
                remaining_pages = page_target - estimated_pages
                full_text = full_text_ref[0]
                headings = [l.strip() for l in full_text.split("\n") if l.strip().startswith("#")]
                heading_outline = "\n".join(headings[-30:]) if headings else ""
                last_context = full_text[-3000:]

                content_continuation = f"""Το κείμενο που έγραψες είναι ~{estimated_pages} σελίδες, αλλά ο στόχος είναι ~{page_target} σελίδες.
ΣΥΝΕΧΙΣΕ να γράφεις ~{remaining_pages} ακόμα σελίδες περιεχομένου.

### ΕΝΟΤΗΤΕΣ ΠΟΥ ΗΔΗ ΚΑΛΥΦΘΗΚΑΝ:
{heading_outline}

### ΤΕΛΟΣ ΚΕΙΜΕΝΟΥ (ΣΥΝΕΧΙΣΕ ΑΠΟ ΕΔΩ):
...{last_context}

ΜΗΝ επαναλάβεις υποενότητες που ήδη γράφτηκαν. Πρόσθεσε ΝΕΕΣ υποενότητες.
Στο ΤΕΛΟΣ πρόσθεσε τις υποχρεωτικές ενότητες ({end_sections_label(cfg)}) αν λείπουν."""

                yield "\n\n"

                cont_messages = [
                    {"role": "user", "content": prompt},
                    {"role": "assistant", "content": full_text[-4000:]},
                    {"role": "user", "content": content_continuation},
                ]
                cont_usage: dict = {}
                async for chunk in self._stream_with_retry(
                    system_prompt, cont_messages, settings.max_tokens, cont_usage,
                    full_text_ref, label="length-continuation",
                ):
                    yield chunk
                total_input_tokens += int(cont_usage.get("input_tokens", 0))
                total_output_tokens += int(cont_usage.get("output_tokens", 0))

        # Record actual token usage after the throttle context exits
        await rate_limiter.tracker.record_usage(total_input_tokens, total_output_tokens)

    # ------------------------------------------------------------------
    # Auxiliary single-call generations
    # ------------------------------------------------------------------
    async def generate_bibliography(
        self, citations: list[str], topic: str = "", user_id: str = "anonymous"
    ) -> str:
        citations_text = "\n".join(f"- {c}" for c in citations)
        prompt = f"""Γράψε ΠΛΗΡΕΙΣ βιβλιογραφικές εγγραφές σε μορφή APA 7th Edition
για τις παρακάτω αναφορές που βρέθηκαν ως in-text citations σε εκπαιδευτικό υλικό{f' σχετικό με "{topic}"' if topic else ''}.

## In-text citations:
{citations_text}

## ΚΑΝΟΝΕΣ APA 7th:
- Γράψε ΜΟΝΟ τις βιβλιογραφικές εγγραφές, μία ανά γραμμή, ΑΛΦΑΒΗΤΙΚΑ κατά επώνυμο
- Μορφές ανά τύπο πηγής:
  ΒΙΒΛΙΟ: Επώνυμο, Α. Β. (Έτος). *Τίτλος βιβλίου* (Χη εκδ.). Εκδότης.
  - Τίτλος σε *italics*, sentence case. Εκδότης ΥΠΟΧΡΕΩΤΙΚΟΣ, χωρίς τοποθεσία.
  - Έκδοση αν δεν είναι η πρώτη: (2nd ed.), (7th ed.)
  ΑΡΘΡΟ: Επώνυμο, Α. Β., & Επώνυμο, Γ. Δ. (Έτος). Τίτλος. *Περιοδικό*, *τόμος*(τεύχος), σελ.–σελ. https://doi.org/xxxxx
  - Τίτλος ΟΧΙ italics. Περιοδικό σε *italics* + Title Case. Τόμος *italic*, τεύχος σε παρένθεση ΟΧΙ italic.
  - ΟΧΙ τελεία μετά DOI. Σελίδες με en-dash (–).
  ΙΣΤΟΣΕΛΙΔΑ: Οργανισμός. (Έτος, Μήνας Ημέρα). *Τίτλος*. Ιστότοπος. URL
  - Χωρίς ημερομηνία: (n.d.). ΟΧΙ "Retrieved from".
  ΣΥΓΓΡΑΦΕΙΣ: Όλοι (1-20), "&" πριν τελευταίο. 21+: πρώτοι 19 … τελευταίος.
  ΟΧΙ "et al." στη λίστα βιβλιογραφίας. Μορφή αρχικών: Α. Β.
  ΟΧΙ διπλές τελείες. DOI ως URL: https://doi.org/xxxxx
- Χρησιμοποίησε ΠΡΑΓΜΑΤΙΚΕΣ πληροφορίες από τη γνώση σου
- Αν δεν γνωρίζεις τα πλήρη στοιχεία, χρησιμοποίησε placeholder:
  π.χ. Επώνυμο, Α. Β. (Έτος). [Τίτλος δεν διατίθεται]. [Χρειάζεται επαλήθευση]
- ΜΗΝ επινοείς ψευδή στοιχεία (DOI, τόμο, σελίδες) αν δεν τα γνωρίζεις
- ΜΗΝ προσθέσεις αναφορές που δεν υπάρχουν στη λίστα
- ΑΠΑΓΟΡΕΥΟΝΤΑΙ: πτυχιακές εργασίες, μεταπτυχιακές διατριβές. ΕΠΙΤΡΕΠΟΝΤΑΙ: δημοσιευμένα άρθρα, βιβλία, εκθέσεις οργανισμών, **διδακτορικές διατριβές**
- ΜΗΝ γράψεις τίποτα άλλο εκτός από τις εγγραφές (χωρίς εισαγωγή, χωρίς σχόλια)
"""
        rate_limiter = get_rate_limiter()
        async with rate_limiter.throttle(user_id, Priority.LIGHT, estimated_output=4000):
            text, usage = await self.complete_text(
                system=None,
                messages=[{"role": "user", "content": prompt}],
                max_tokens=4000,
            )
        await rate_limiter.tracker.record_usage(
            int(usage.get("input_tokens", 0)), int(usage.get("output_tokens", 0))
        )
        return text

    async def generate_summary(
        self, module_title: str, full_content: str, user_id: str = "anonymous"
    ) -> str:
        if len(full_content) > 50000:
            content_for_summary = full_content[:40000] + "\n\n[...]\n\n" + full_content[-10000:]
        else:
            content_for_summary = full_content

        prompt = f"""Γράψε μια ολοκληρωμένη Περίληψη (500-800 λέξεις) για την εκπαιδευτική ενότητα "{module_title}".

Η Περίληψη ΠΡΕΠΕΙ να:
- Συνοψίζει τα βασικά θέματα που αναπτύχθηκαν
- Αναφέρει τις κύριες θεωρητικές προσεγγίσεις
- Υπογραμμίζει τα σημαντικότερα συμπεράσματα
- Συνδέει τα θέματα μεταξύ τους
- Γράφεται σε ακαδημαϊκό ύφος, τρίτο πρόσωπο, παθητική φωνή
- Γράφεται σε πλήρεις παραγράφους (ΟΧΙ bullets)
- Είναι στα ελληνικά

ΜΗΝ προσθέσεις νέες αναφορές ή βιβλιογραφία. Ανακεφαλαίωσε ΜΟΝΟ αυτά που ήδη αναπτύχθηκαν.

## ΕΚΠΑΙΔΕΥΤΙΚΟ ΥΛΙΚΟ:
{content_for_summary}

Γράψε ΜΟΝΟ την Περίληψη, χωρίς τίτλο ή εισαγωγικό κείμενο."""

        rate_limiter = get_rate_limiter()
        async with rate_limiter.throttle(user_id, Priority.LIGHT, estimated_output=4000):
            text, usage = await self.complete_text(
                system=None,
                messages=[{"role": "user", "content": prompt}],
                max_tokens=4000,
            )
        await rate_limiter.tracker.record_usage(
            int(usage.get("input_tokens", 0)), int(usage.get("output_tokens", 0))
        )
        return text

    async def generate_skill_review(
        self,
        module: dict,
        content: str,
        skill_descriptions: list[dict],
        user_id: str = "anonymous",
        occupation: Optional[dict] = None,
    ) -> dict:
        # Merge module skills (code/name/type) with ESCO descriptions (name/description) by name.
        desc_map = {
            (s.get("name") or "").lower(): s.get("description")
            for s in skill_descriptions
        }
        module_skills = module.get("skills", []) or []
        if module_skills:
            skill_lines = []
            for i, s in enumerate(module_skills, start=1):
                code = s.get("code", "")
                name = s.get("name", "")
                stype = s.get("type", "essential")
                desc = desc_map.get(name.lower())
                line = f"{i}. [code={code}] [{stype}] {name}"
                if desc:
                    line += f"\n   Περιγραφή ESCO: {desc}"
                skill_lines.append(line)
            skills_text = "\n".join(skill_lines)
        else:
            # Fallback if module skills not present — only descriptions
            skills_text = "\n".join(
                f"- {s.get('name', '')}: {s.get('description') or 'No description'}"
                for s in skill_descriptions
            )

        # Optional program-level ESCO occupation context block.
        occupation_block = ""
        if occupation and (occupation.get("name") or "").strip():
            occ_name = occupation["name"].strip()
            occ_code = (occupation.get("code") or "").strip()
            occ_desc = (occupation.get("description") or "").strip()
            code_suffix = f" (ESCO code: {occ_code})" if occ_code else ""
            occupation_block = f"\n## ΕΠΑΓΓΕΛΜΑ-ΣΤΟΧΟΣ (ESCO)\n{occ_name}{code_suffix}"
            if occ_desc:
                occupation_block += f"\nΠεριγραφή: {occ_desc}"
            occupation_block += (
                "\nΣημείωση: το υλικό αξιολογείται ως κατάρτιση για το παραπάνω επάγγελμα — "
                "λάβε υπόψη αυτό το πλαίσιο κατά την εκτίμηση κάλυψης δεξιοτήτων.\n"
            )

        prompt = f"""
{REVIEW_PROMPT}
{occupation_block}
## ΕΝΟΤΗΤΑ
{module.get('title', '')}

## ΔΕΞΙΟΤΗΤΕΣ ESCO ΠΡΟΣ ΑΞΙΟΛΟΓΗΣΗ
{skills_text}

## ΕΚΠΑΙΔΕΥΤΙΚΟ ΥΛΙΚΟ ΠΡΟΣ ΑΞΙΟΛΟΓΗΣΗ
{content[:50000]}

Αξιολόγησε την κάλυψη κάθε μίας από τις παραπάνω δεξιότητες σύμφωνα με τις οδηγίες. Απάντησε ΜΟΝΟ με το JSON object.
"""
        rate_limiter = get_rate_limiter()
        async with rate_limiter.throttle(user_id, Priority.LIGHT, estimated_output=5000):
            text, usage = await self.complete_text(
                system=None,
                messages=[{"role": "user", "content": prompt}],
                max_tokens=8000,
            )
        await rate_limiter.tracker.record_usage(
            int(usage.get("input_tokens", 0)), int(usage.get("output_tokens", 0))
        )

        try:
            return extract_json_object(text)
        except Exception as e:
            return {
                "error": str(e),
                "skillAnalysis": [],
                "overallAssessment": "Analysis failed",
            }

    # ------------------------------------------------------------------
    # Formatting helpers
    # ------------------------------------------------------------------
    def _format_module(
        self,
        module: dict,
        target_pages: Optional[int] = None,
        learning_outcomes: str = "",
        keywords: str = "",
        occupation: Optional[dict] = None,
    ) -> str:
        parts = [f"## Ενότητα {module.get('number', '')}: {module.get('title', '')}"]

        # Optional program-level ESCO occupation context — helps the model anchor
        # the educational material to the target profession.
        if occupation:
            occ_name = (occupation.get("name") or "").strip()
            occ_code = (occupation.get("code") or "").strip()
            occ_desc = (occupation.get("description") or "").strip()
            if occ_name:
                code_suffix = f" (ESCO code: {occ_code})" if occ_code else ""
                parts.append(f"\n**Επάγγελμα-στόχος (ESCO):** {occ_name}{code_suffix}")
                if occ_desc:
                    parts.append(f"**Περιγραφή επαγγέλματος:** {occ_desc}")
                parts.append(
                    "Το υλικό αυτής της ενότητας πρέπει να εξυπηρετεί την κατάρτιση "
                    "για το παραπάνω επάγγελμα — προσάρμοσε ορολογία, παραδείγματα "
                    "και πρακτικές εφαρμογές αναλόγως."
                )

        hours = module.get("hours", 0) or 0
        if hours > 0:
            parts.append(f"\n**Διάρκεια:** {hours} ώρες")

        if target_pages:
            parts.append(
                f"\n**Στόχος σελίδων:** ~{target_pages} σελίδες "
                f"(~{target_pages * 3000} χαρακτήρες). Μέγιστο: {target_pages + 5} σελίδες."
            )

        content = module.get("content", "")
        if content:
            parts.append(f"\n**Περιεχόμενο:**\n{content}")

        activities = module.get("activities", "")
        if activities:
            parts.append(f"\n**Δραστηριότητες:**\n{activities}")

        skills = module.get("skills", [])
        if skills:
            skills_text = "\n".join(
                f"- {s.get('name', '')} [{s.get('type', 'E')[0].upper()}]"
                for s in skills
            )
            parts.append(f"\n**Δεξιότητες ESCO:**\n{skills_text}")
            parts.append(
                "\n**ΣΗΜΑΝΤΙΚΟ:** Το περιεχόμενο ΠΡΕΠΕΙ να καλύπτει ΟΛΕΣ τις παραπάνω δεξιότητες ESCO. "
                "Κάθε δεξιότητα πρέπει να αναπτύσσεται με θεωρία, παραδείγματα και πρακτική εφαρμογή "
                "ώστε να αποκτηθεί η συγκεκριμένη ικανότητα."
            )

        if learning_outcomes.strip():
            parts.append(f"\n**Μαθησιακά Αποτελέσματα:**\n{learning_outcomes.strip()}")

        if keywords.strip():
            parts.append(f"\n**Λέξεις Κλειδιά:** {keywords.strip()}")

        return "\n".join(parts)

    def _format_references(self, references: list[dict]) -> str:
        if not references:
            return "Δεν υπάρχουν διαθέσιμες αναφορές."
        lines = []
        for i, ref in enumerate(references, 1):
            authors = ", ".join(ref.get("authors", [])) or "Unknown"
            year = ref.get("year", "n.d.")
            title = ref.get("title", "")
            journal = ref.get("journal", "")
            doi = ref.get("doi", "")

            line = f"{i}. {authors} ({year}). {title}."
            if journal:
                line += f" {journal}."
            if doi:
                line += f" https://doi.org/{doi}"
            lines.append(line)
        return "\n".join(lines)


# =============================================================================
# Factory
# =============================================================================
_service_instances: dict[str, LLMService] = {}


def get_llm_service(provider: str = "claude") -> LLMService:
    """Return a cached singleton service for the given provider."""
    provider = (provider or "claude").lower()
    if provider not in _service_instances:
        if provider == "openai":
            from .openai_service import OpenAIService
            _service_instances[provider] = OpenAIService()
        else:
            # default: claude
            from .claude_service import ClaudeService
            _service_instances["claude"] = ClaudeService()
            provider = "claude"
    return _service_instances[provider]

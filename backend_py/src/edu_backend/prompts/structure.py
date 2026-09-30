"""
Structure configuration for educational-content generation.

Selects which OPTIONAL structural elements appear in generated material.
The "quality core" — academic register, paragraph depth, APA citation
format, the mandatory Βιβλιογραφία, anti-hallucination rules,
theoretical-framework depth — is NOT configurable and is always present.

Only these optional elements can be toggled from the UI:

  - activities          Δραστηριότητες (2 ανά υποενότητα)
  - self_assessment     Ερωτήσεις / Απαντήσεις Αυτοαξιολόγησης
  - glossary            Γλωσσάρι
  - subsection_keywords **Βασικές λέξεις:** ανά υποενότητα
  - in_text_citations   Υποχρεωτική πυκνότητα παρενθετικών αναφορών (Επώνυμο, Έτος)

Defaults: everything OFF except in-text citations — elements are on demand.
Bibliography stays mandatory in every mode.

How it works: the prompt constants in ``system_prompt`` are left UNTOUCHED.
For each element that is turned OFF:

  1. the text span that mandates it is removed from the prompt (_REGIONS),
  2. stray mentions elsewhere in the prompt are neutralised (_REPLACEMENTS),
  3. an explicit manifest (``build_structure_manifest``) is appended to the
     user prompt, naming each disabled element and forbidding it. Removal by
     absence alone was not enough — the model's prior for "educational
     material" kept re-adding activities.

When every element is ON the builders return the original constants verbatim.
``detect_structure_violations`` is a post-generation safety net.
"""
from __future__ import annotations

import itertools
import re
from dataclasses import dataclass

from .system_prompt import (
    SYSTEM_PROMPT,
    EXPERIMENTAL_SYSTEM_PROMPT,
    EXPAND_PROMPT,
    STANDARD_CONTENT_STRUCTURE,
)

OPTIONAL_KEYS = (
    "activities",
    "self_assessment",
    "glossary",
    "subsection_keywords",
    "in_text_citations",
)


@dataclass(frozen=True)
class StructureConfig:
    """Which optional structural elements to include (on demand — only
    in-text citations are ON by default)."""

    activities: bool = False
    self_assessment: bool = False
    glossary: bool = False
    subsection_keywords: bool = False
    in_text_citations: bool = True

    @classmethod
    def from_dict(cls, data: dict | None) -> "StructureConfig":
        if not data:
            return cls()
        defaults = cls()
        return cls(**{k: bool(data.get(k, getattr(defaults, k))) for k in OPTIONAL_KEYS})

    def as_flags(self) -> dict:
        return {k: getattr(self, k) for k in OPTIONAL_KEYS}

    @property
    def all_on(self) -> bool:
        return all(self.as_flags().values())


ALL_ON = StructureConfig(**{k: True for k in OPTIONAL_KEYS})


# (flag, start_anchor, end_anchor) — the inclusive span is removed when the
# flag is OFF. Anchors are validated at import time (see _validate below).
_REGIONS = {
    "system": [
        ("activities", "## 2. ΠΡΑΚΤΙΚΕΣ ΕΦΑΡΜΟΓΕΣ", "Γράψε τις συγκρίσεις σε παραγράφους."),
        ("self_assessment", "## 3. ΕΡΩΤΗΣΕΙΣ ΑΥΤΟΑΞΙΟΛΟΓΗΣΗΣ", "Μορφή: 1. α, 2. β, 3. γ, κ.ο.κ."),
        ("glossary", "## 5. ΓΛΩΣΣΑΡΙ", "Αλφαβητική λίστα βασικών όρων που εισάγονται σε ΑΥΤΟ το τμήμα, με σύντομους ορισμούς."),
        ("in_text_citations", "## IN-TEXT CITATIONS", 'ΛΑΘΟΣ: "(Ducatel κ.ά., 2001)" — Πάντα "et al." αντί "κ.ά."'),
    ],
    "experimental": [
        ("subsection_keywords", "### ΒΑΣΙΚΕΣ ΛΕΞΕΙΣ ΑΝΑ ΥΠΟΕΝΟΤΗΤΑ", "**Βασικές λέξεις:** όρος1, όρος2, όρος3, ..."),
        ("activities", "## 2. ΠΡΑΚΤΙΚΕΣ ΕΦΑΡΜΟΓΕΣ", "Συγκρίσεις σε παραγράφους (ΟΧΙ πίνακες)."),
        ("self_assessment", "## 3. ΕΡΩΤΗΣΕΙΣ ΑΥΤΟΑΞΙΟΛΟΓΗΣΗΣ", "Ακολουθεί ενότητα Απαντήσεων: 1. α, 2. β, κ.ο.κ."),
        ("glossary", "## 5. ΓΛΩΣΣΑΡΙ", "Αλφαβητική λίστα βασικών όρων που εισάγονται σε ΑΥΤΟ το τμήμα."),
        ("in_text_citations", "## ΥΠΟΧΡΕΩΤΙΚΕΣ IN-TEXT ΑΝΑΦΟΡΕΣ", "ΔΕΝ επιτρέπεται κείμενο χωρίς καμία βιβλιογραφική τεκμηρίωση."),
    ],
    "structure": [
        ("subsection_keywords", "### ΒΑΣΙΚΕΣ ΛΕΞΕΙΣ ΑΝΑ ΥΠΟΕΝΟΤΗΤΑ", "Αυτοί είναι οι κύριοι όροι/έννοιες που αναπτύσσονται στη συγκεκριμένη υποενότητα."),
        ("activities", "### ΣΗΜΑΝΤΙΚΟ: ΔΡΑΣΤΗΡΙΟΤΗΤΕΣ ΑΝΑ ΥΠΟΕΝΟΤΗΤΑ", "Να εξασκηθείτε στην εφαρμογή θεωρητικών μοντέλων σε πραγματικά σενάρια."),
        ("self_assessment", "## Ερωτήσεις Αυτοαξιολόγησης", "Μορφή: 1. α, 2. β, 3. γ, 4. δ, κ.ο.κ."),
        ("glossary", "## Γλωσσάρι", "Αλφαβητική λίστα βασικών όρων που εισάγονται σε ΑΥΤΟ το τμήμα, με σύντομους ορισμούς."),
    ],
}

# (flag, old, new) — stray mentions outside the regions above, replaced
# (all occurrences) when the flag is OFF. Applied in order after the regions.
_ACTIVITY_BULLETS = '3. "Οδηγίες" δραστηριοτήτων (bullets •)\n'
_PLURAL_ACTIVITIES = ("σε δραστηριότητες, προσδοκώμενα", "σε προσδοκώμενα")
_PLURAL_QUESTIONS = ("προσδοκώμενα αποτελέσματα και ερωτήσεις", "προσδοκώμενα αποτελέσματα")

_REPLACEMENTS = {
    "system": [
        ("activities", _ACTIVITY_BULLETS, ""),
    ],
    "experimental": [
        ("activities", _ACTIVITY_BULLETS, ""),
        ("activities", *_PLURAL_ACTIVITIES),
        ("self_assessment", *_PLURAL_QUESTIONS),
    ],
    "structure": [
        ("activities", *_PLURAL_ACTIVITIES),
        ("self_assessment", *_PLURAL_QUESTIONS),
    ],
}

# Human-readable names + a one-line description, so the model knows exactly
# what a disabled element looks like and can avoid it.
_ELEMENT_INFO = {
    "activities": (
        "Δραστηριότητες",
        "ασκήσεις/δραστηριότητες με Τίτλο, Περιγραφή, Οδηγίες και Στόχο, "
        "συνήθως στο τέλος των υποενοτήτων",
    ),
    "self_assessment": (
        "Ερωτήσεις Αυτοαξιολόγησης",
        "ερωτήσεις πολλαπλής επιλογής και ενότητα Απαντήσεων",
    ),
    "glossary": (
        "Γλωσσάρι",
        "ενότητα με ορισμούς βασικών όρων στο τέλος του τμήματος",
    ),
    "subsection_keywords": (
        "Βασικές λέξεις ανά υποενότητα",
        'γραμμή "**Βασικές λέξεις:** ..." κάτω από τον τίτλο κάθε υποενότητας',
    ),
    "in_text_citations": (
        "Ενδοκειμενικές αναφορές",
        "παρενθετικές αναφορές (Επώνυμο, Έτος) μέσα στο κείμενο",
    ),
}


def _strip_region(text: str, start_anchor: str, end_anchor: str) -> str:
    i = text.find(start_anchor)
    if i == -1:
        raise KeyError(f"structure: start anchor not found: {start_anchor!r}")
    j = text.find(end_anchor, i)
    if j == -1:
        raise KeyError(f"structure: end anchor not found after start: {end_anchor!r}")
    return text[:i] + text[j + len(end_anchor):]


def _replace_region(text: str, start_anchor: str, end_anchor: str, replacement: str) -> str:
    i = text.find(start_anchor)
    if i == -1:
        raise KeyError(f"structure: start anchor not found: {start_anchor!r}")
    j = text.find(end_anchor, i)
    if j == -1:
        raise KeyError(f"structure: end anchor not found after start: {end_anchor!r}")
    return text[:i] + replacement + text[j + len(end_anchor):]


def _replace_exact(text: str, old: str, new: str) -> str:
    if old not in text:
        raise KeyError(f"structure: replacement text not found: {old!r}")
    return text.replace(old, new)


def _apply(text: str, region_key: str, cfg: StructureConfig) -> str:
    if cfg.all_on:
        return text  # byte-identical default path
    flags = cfg.as_flags()
    for flag, start, end in _REGIONS.get(region_key, []):
        if not flags[flag]:
            text = _strip_region(text, start, end)
    for flag, old, new in _REPLACEMENTS.get(region_key, []):
        if not flags[flag]:
            text = _replace_exact(text, old, new)
    return re.sub(r"\n{3,}", "\n\n", text)


def build_system_prompt(cfg: StructureConfig, experimental: bool = False) -> str:
    if experimental:
        return _apply(EXPERIMENTAL_SYSTEM_PROMPT, "experimental", cfg)
    return _apply(SYSTEM_PROMPT, "system", cfg)


def build_structure_block(cfg: StructureConfig) -> str:
    return _apply(STANDARD_CONTENT_STRUCTURE, "structure", cfg)


def _expand_rule8(cfg: StructureConfig) -> str:
    """Rebuild EXPAND_PROMPT rule 8 (mandatory end sections) from active elements.

    With every flag ON this reproduces the original rule 8 verbatim.
    """
    parts = []
    if cfg.self_assessment:
        parts.append("ΑΡΙΘΜΗΜΕΝΕΣ Ερωτήσεις Αυτοαξιολόγησης (1. [Ερώτηση] α) β) γ) δ)) + Απαντήσεις")
    parts.append("Βιβλιογραφία (APA 7th)")
    if cfg.glossary:
        parts.append("Γλωσσάρι")
    return "8. ΥΠΟΧΡΕΩΤΙΚΑ στο ΤΕΛΟΣ: " + " + ".join(parts) + ". ΔΕΝ ΕΠΙΤΡΕΠΕΤΑΙ ΠΑΡΑΛΕΙΨΗ."


def build_expand_prompt(cfg: StructureConfig) -> str:
    if cfg.all_on:
        return EXPAND_PROMPT  # byte-identical default path
    text = _replace_region(
        EXPAND_PROMPT,
        "8. ΥΠΟΧΡΕΩΤΙΚΑ στο ΤΕΛΟΣ:",
        "ΔΕΝ ΕΠΙΤΡΕΠΕΤΑΙ ΠΑΡΑΛΕΙΨΗ.",
        _expand_rule8(cfg),
    )
    if not cfg.subsection_keywords:
        text = _strip_region(text, "6. ΑΜΕΣΩΣ μετά τον τίτλο κάθε υποενότητας:", "**Βασικές λέξεις:** όρος1, όρος2, ...")
    if not cfg.activities:
        text = _strip_region(text, "7. 2 δραστηριότητες στο ΤΕΛΟΣ", "Στόχο δραστηριότητας")
        text = _replace_exact(text, ", bullets (•) στις Οδηγίες δραστηριοτήτων.", ".")
    if not (cfg.activities and cfg.self_assessment):
        addressed = [
            name for on, name in ((cfg.activities, "δραστηριότητες"), (cfg.self_assessment, "ερωτήσεις")) if on
        ]
        plural = (
            f"Β' πληθυντικό σε {addressed[0]}."
            if addressed
            else "Β' πληθυντικό όπου απευθύνεσαι στον αναγνώστη."
        )
        text = _replace_exact(text, "Β' πληθυντικό σε δραστηριότητες & ερωτήσεις.", plural)
    return text


def build_final_reminder(cfg: StructureConfig, mcq_instruction: str) -> str:
    """The "ΥΠΟΧΡΕΩΤΙΚΟ στο ΤΕΛΟΣ" block, listing only active end sections.

    Βιβλιογραφία is always present, so the block is never empty.
    """
    items = []
    if cfg.self_assessment:
        items.append(f"## Ερωτήσεις Αυτοαξιολόγησης — {mcq_instruction}")
        items.append("## Απαντήσεις Αυτοαξιολόγησης — μορφή: 1. α, 2. β, κ.ο.κ.")
    items.append("## Βιβλιογραφία — APA 7th, ΟΛΕΣ οι αναφορές που χρησιμοποιήθηκαν")
    if cfg.glossary:
        items.append("## Γλωσσάρι — αλφαβητικά, βασικοί όροι με ορισμούς")
    numbered = "\n".join(f"{n}. {x}" for n, x in enumerate(items, 1))
    return (
        "\n\n## ΥΠΟΧΡΕΩΤΙΚΟ: Στο ΤΕΛΟΣ αυτού του τμήματος ΠΡΕΠΕΙ να υπάρχουν "
        f"(ΜΗΝ ΠΑΡΑΛΕΙΨΕΙΣ):\n{numbered}\n"
    )


def build_structure_manifest(cfg: StructureConfig) -> str:
    """Explicit list of included / forbidden optional elements.

    Appended at the END of user prompts (recency) so the model knows what
    each disabled element is and that it must not add it. Empty when every
    element is ON. In-text citations turned OFF are relaxed, not forbidden.
    """
    if cfg.all_on:
        return ""
    flags = cfg.as_flags()
    included = [_ELEMENT_INFO[k][0] for k in OPTIONAL_KEYS if flags[k]]
    included.append("Βιβλιογραφία (πάντα υποχρεωτική)")
    forbidden = [
        f"- {_ELEMENT_INFO[k][0]} ({_ELEMENT_INFO[k][1]})"
        for k in OPTIONAL_KEYS
        if not flags[k] and k != "in_text_citations"
    ]

    lines = [
        "",
        "",
        "## ΔΟΜΗ ΠΟΥ ΕΠΕΛΕΞΕ Ο ΧΡΗΣΤΗΣ (ΚΡΙΣΙΜΟ — ΥΠΕΡΙΣΧΥΕΙ κάθε γενικής σύμβασης για εκπαιδευτικό υλικό)",
        "Προαιρετικά στοιχεία που ΠΕΡΙΛΑΜΒΑΝΟΝΤΑΙ: " + ", ".join(included) + ".",
    ]
    if forbidden:
        lines.append(
            "ΔΕΝ ΠΕΡΙΛΑΜΒΑΝΟΝΤΑΙ — ΑΠΑΓΟΡΕΥΕΤΑΙ να τα προσθέσεις, ακόμη και σύντομα ή με άλλον τίτλο:"
        )
        lines.extend(forbidden)
    if not cfg.in_text_citations:
        lines.append(
            "Οι ενδοκειμενικές αναφορές δεν είναι υποχρεωτικές σε κάθε παράγραφο — "
            "χρησιμοποίησέ τες όπου τεκμηριώνουν κάτι ουσιαστικό."
        )
    lines.append(
        "Εξαίρεση: αν οι ΟΔΗΓΙΕΣ ΧΡΗΣΤΗ ζητούν ρητά κάποιο από τα παραπάνω, ακολούθησε τις οδηγίες χρήστη."
    )
    return "\n".join(lines) + "\n"


def end_sections_label(cfg: StructureConfig) -> str:
    """Comma-joined active end-section names, e.g. 'Ερωτήσεις, Βιβλιογραφία, Γλωσσάρι'."""
    names = []
    if cfg.self_assessment:
        names.append("Ερωτήσεις")
    names.append("Βιβλιογραφία")
    if cfg.glossary:
        names.append("Γλωσσάρι")
    return ", ".join(names)


# Patterns that reveal a disabled element in generated text.
_VIOLATION_PATTERNS = {
    "activities": re.compile(
        r"^\s*(?:#+\s*|\*\*)?\s*Δραστηριότητ(?:α\s+\d|ες\b)", re.IGNORECASE | re.MULTILINE
    ),
    "self_assessment": re.compile(
        r"^\s*#+\s*(?:Ερωτήσεις|Απαντήσεις)\s+Αυτοαξιολόγησης", re.IGNORECASE | re.MULTILINE
    ),
    "glossary": re.compile(r"^\s*#+\s*Γλωσσάρι", re.IGNORECASE | re.MULTILINE),
    "subsection_keywords": re.compile(r"\*\*Βασικές λέξεις:\*\*", re.IGNORECASE),
}


def detect_structure_violations(text: str, cfg: StructureConfig) -> list[str]:
    """Return the keys of disabled elements that nevertheless appear in ``text``."""
    flags = cfg.as_flags()
    return [
        key for key, pattern in _VIOLATION_PATTERNS.items()
        if not flags[key] and pattern.search(text)
    ]


def _validate() -> None:
    """Fail fast at import time if any anchor drifts out of the prompts."""
    assert build_system_prompt(ALL_ON) == SYSTEM_PROMPT
    assert build_system_prompt(ALL_ON, experimental=True) == EXPERIMENTAL_SYSTEM_PROMPT
    assert build_structure_block(ALL_ON) == STANDARD_CONTENT_STRUCTURE
    assert build_expand_prompt(ALL_ON) == EXPAND_PROMPT
    # every anchor / replacement must resolve under every combination of flags
    for combo in itertools.product((True, False), repeat=len(OPTIONAL_KEYS)):
        cfg = StructureConfig(**dict(zip(OPTIONAL_KEYS, combo)))
        build_system_prompt(cfg)
        build_system_prompt(cfg, experimental=True)
        build_structure_block(cfg)
        build_expand_prompt(cfg)


_validate()

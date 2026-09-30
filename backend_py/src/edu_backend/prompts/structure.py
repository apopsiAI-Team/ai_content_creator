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
import unicodedata
from dataclasses import dataclass, replace

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
        ("in_text_citations", "## ΣΤΥΛ ΠΑΡΕΝΘΕΤΙΚΗΣ ΑΝΑΦΟΡΑΣ", "κανόνας APA 7th ανεξαρτήτως γλώσσας κειμένου."),
        ("in_text_citations", "## Στυλ Αναφορών (παρενθετική στο τέλος)", 'σημαντικά τα τελευταία χρόνια (Smith & Jones, 2022)."'),
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
        # In-text citations OFF = no (Επώνυμο, Έτος) in the body; the
        # bibliography stays as the list of sources the content is based on.
        ("in_text_citations",
         "- Όλες τις αναφορές που χρησιμοποιήθηκαν στο κείμενο, ΑΛΦΑΒΗΤΙΚΑ",
         "- Τις πηγές στις οποίες βασίζεται το περιεχόμενο, ΑΛΦΑΒΗΤΙΚΑ"),
        ("in_text_citations",
         "ΚΡΙΤΙΚΟ: ΚΑΘΕ in-text citation (Επώνυμο, Έτος) ΠΡΕΠΕΙ να έχει αντίστοιχη εγγραφή στη Βιβλιογραφία.\n", ""),
        ("in_text_citations", "ΜΗΝ γράφεις αριθμούς σελίδων σε παραπομπές, εκτός αν είσαι βέβαιος.\n", ""),
        ("in_text_citations",
         'Ορισμοί με παραπομπές στο τέλος: "...ορίζεται ως η διαδικασία X (Smith, 2022)."',
         "Ορισμοί σε πλήρεις παραγράφους."),
        ("in_text_citations", "της αγοράς\n(Smith, 2022). Παράλληλα", "της αγοράς. Παράλληλα"),
        ("in_text_citations", 'ανταγωνιστικότητας (Johnson & Brown, 2021)."', 'ανταγωνιστικότητας."'),
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
        "παραπομπές (Επώνυμο, Έτος) ή «Σύμφωνα με τον Χ» μέσα στο κείμενο — "
        "η Βιβλιογραφία στο τέλος παραμένει ως οι πηγές στις οποίες βασίζεται το περιεχόμενο",
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
    element is ON.
    """
    if cfg.all_on:
        return ""
    flags = cfg.as_flags()
    included = [_ELEMENT_INFO[k][0] for k in OPTIONAL_KEYS if flags[k]]
    included.append("Βιβλιογραφία (πάντα υποχρεωτική)")
    forbidden = [
        f"- {_ELEMENT_INFO[k][0]} ({_ELEMENT_INFO[k][1]})"
        for k in OPTIONAL_KEYS
        if not flags[k]
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
    lines.append(
        "Εξαίρεση: αν οι ΟΔΗΓΙΕΣ ΧΡΗΣΤΗ ή οι ΖΗΤΟΥΜΕΝΕΣ ΑΛΛΑΓΕΣ ζητούν ρητά την προσθήκη ή αφαίρεση "
        "κάποιου από τα παραπάνω, ακολούθησέ τες."
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
    # "(Smith, 2020)", "(Porter & Kramer, 2011; OECD, 2019)" — not the bare
    # "(2020)." of a bibliography entry.
    "in_text_citations": re.compile(r"\([^()]*?[^\W\d_]{2,}[^()]*?,\s*\d{4}[a-z]?\b[^()]*\)"),
}

# Words in the user's free-text instructions that ask for an element — an
# element the user explicitly requested is never reported as a violation.
_REQUEST_PATTERNS = {
    "activities": re.compile(r"δραστηριοτητ|ασκησ|activit|exercis"),
    "self_assessment": re.compile(r"ερωτησ|αυτοαξιολογ|mcq|quiz"),
    "glossary": re.compile(r"γλωσσαρ|glossary"),
    "subsection_keywords": re.compile(r"βασικες λεξεις|keywords"),
    "in_text_citations": re.compile(r"ενδοκειμενικ|παραπομπ|citation"),
}


def _fold(text: str) -> str:
    """Lowercase and strip Greek accents so "Δραστηριότητες" matches "δραστηριοτητ"."""
    decomposed = unicodedata.normalize("NFKD", text.lower())
    return "".join(c for c in decomposed if not unicodedata.combining(c))


# A mention preceded by one of these in the same clause is a prohibition
# ("μην βάλεις δραστηριότητες", "χωρίς γλωσσάρι"), not a request.
_NEGATION = re.compile(
    r"\b(?:μην|μη|χωρις|οχι|δεν|without|no|not|dont|don't)\b|αφαιρεσ|βγαλ|παραλει|remove|omit|exclude"
)
# Clause boundaries: a negation only applies within its own clause, so in
# "Χωρίς γλωσσάρι, πρόσθεσε δραστηριότητες" the activities are requested.
_CLAUSE_BOUNDARY = re.compile(r"[.;!?\n,]")


def elements_requested(instructions: str) -> set[str]:
    """Keys of the optional elements that the user's instructions ask FOR.

    An element counts as requested when at least one mention of it is not
    negated within its clause.
    """
    folded = _fold(instructions or "")
    requested = set()
    for key, pattern in _REQUEST_PATTERNS.items():
        for match in pattern.finditer(folded):
            boundaries = [b.end() for b in _CLAUSE_BOUNDARY.finditer(folded, 0, match.start())]
            clause_prefix = folded[(boundaries[-1] if boundaries else 0):match.start()]
            if not _NEGATION.search(clause_prefix):
                requested.add(key)
                break
    return requested


def present_elements(text: str) -> set[str]:
    """Keys of the optional elements that already appear in ``text``."""
    return {key for key, pattern in _VIOLATION_PATTERNS.items() if pattern.search(text)}


def revision_config(cfg: StructureConfig, draft: str) -> StructureConfig:
    """Structure for a revision ("Αλλαγές") of an existing draft.

    Whatever the draft already contains is part of its structure — e.g. the
    glossary of an uploaded document — so it is kept and never flagged, even
    when that element is OFF in the UI. Only ADDING a disabled element stays
    forbidden.
    """
    present = present_elements(draft)
    return replace(cfg, **{key: True for key in present})


def detect_structure_violations(text: str, cfg: StructureConfig, instructions: str = "") -> list[str]:
    """Return the keys of disabled elements that nevertheless appear in ``text``.

    Elements mentioned in the user's ``instructions`` are skipped: the user
    asked for them, so they are not "unrequested".
    """
    flags = cfg.as_flags()
    requested = elements_requested(instructions)
    return [
        key for key, pattern in _VIOLATION_PATTERNS.items()
        if not flags[key] and key not in requested and pattern.search(text)
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

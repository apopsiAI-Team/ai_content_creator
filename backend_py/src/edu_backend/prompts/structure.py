"""
Structure configuration for educational-content generation.

Selects which OPTIONAL structural elements appear in generated material.
The "quality core" — academic register, paragraph depth (150-250 words),
APA citation format, the mandatory Βιβλιογραφία, anti-hallucination rules,
theoretical-framework depth — is NOT configurable and is always present.

Only these optional elements can be toggled off from the UI:

  - activities          Δραστηριότητες (2 ανά υποενότητα)
  - self_assessment     Ερωτήσεις / Απαντήσεις Αυτοαξιολόγησης
  - glossary            Γλωσσάρι
  - subsection_keywords **Βασικές λέξεις:** ανά υποενότητα
  - in_text_citations   Υποχρεωτική πυκνότητα παρενθετικών αναφορών (Επώνυμο, Έτος)

Bibliography stays mandatory in every mode.

How it works (and why it is safe): the prompt constants in ``system_prompt``
are left UNTOUCHED. For each element that is turned OFF, the exact text span
that mandates it is removed from the prompt. When every element is ON, the
builders return the original constant verbatim (fast path) — i.e. the default
generation path is byte-identical to before this feature existed, so there is
zero regression and no possibility of silently weakening quality.

The contradiction the free-text "οδηγίες" used to fight (system prompt screams
"ΥΠΟΧΡΕΩΤΙΚΟ γλωσσάρι" while the user asks for none) is eliminated by ABSENCE,
never by an override instruction.
"""
from __future__ import annotations

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
    """Which optional structural elements to include (all default ON)."""

    activities: bool = True
    self_assessment: bool = True
    glossary: bool = True
    subsection_keywords: bool = True
    in_text_citations: bool = True

    @classmethod
    def from_dict(cls, data: dict | None) -> "StructureConfig":
        if not data:
            return cls()
        return cls(**{k: bool(data.get(k, True)) for k in OPTIONAL_KEYS})

    def as_flags(self) -> dict:
        return {k: getattr(self, k) for k in OPTIONAL_KEYS}

    @property
    def all_on(self) -> bool:
        return all(self.as_flags().values())


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


def _apply(text: str, region_key: str, cfg: StructureConfig) -> str:
    if cfg.all_on:
        return text  # byte-identical default path
    flags = cfg.as_flags()
    for flag, start, end in _REGIONS.get(region_key, []):
        if not flags[flag]:
            text = _strip_region(text, start, end)
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
    return text


def build_final_reminder(cfg: StructureConfig, mcq_instruction: str) -> str:
    """The "ΥΠΟΧΡΕΩΤΙΚΟ στο ΤΕΛΟΣ" block, listing only active end sections.

    Βιβλιογραφία is always present. Returns "" only in the impossible case of
    no end sections (bibliography keeps it non-empty).
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


def end_sections_label(cfg: StructureConfig) -> str:
    """Comma-joined active end-section names, e.g. 'Ερωτήσεις, Βιβλιογραφία, Γλωσσάρι'."""
    names = []
    if cfg.self_assessment:
        names.append("Ερωτήσεις")
    names.append("Βιβλιογραφία")
    if cfg.glossary:
        names.append("Γλωσσάρι")
    return ", ".join(names)


def _validate() -> None:
    """Fail fast at import time if any anchor drifts out of the prompts."""
    all_on = StructureConfig()
    assert build_system_prompt(all_on) == SYSTEM_PROMPT
    assert build_system_prompt(all_on, experimental=True) == EXPERIMENTAL_SYSTEM_PROMPT
    assert build_structure_block(all_on) == STANDARD_CONTENT_STRUCTURE
    assert build_expand_prompt(all_on) == EXPAND_PROMPT
    # every region anchor must resolve when its flag is OFF
    for key in OPTIONAL_KEYS:
        cfg = StructureConfig(**{key: False})
        build_system_prompt(cfg)
        build_system_prompt(cfg, experimental=True)
        build_structure_block(cfg)
        build_expand_prompt(cfg)


_validate()

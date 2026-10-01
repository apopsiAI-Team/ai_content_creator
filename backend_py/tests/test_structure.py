from edu_backend.prompts.structure import (
    ALL_ON,
    StructureConfig,
    build_expand_prompt,
    build_structure_block,
    build_structure_manifest,
    build_system_prompt,
    detect_structure_violations,
    elements_requested,
    revision_config,
)
from edu_backend.services.llm_service import LLMService, page_budget


def test_defaults_are_on_demand():
    cfg = StructureConfig()
    assert cfg.in_text_citations is True
    assert not (cfg.activities or cfg.self_assessment or cfg.glossary or cfg.subsection_keywords)
    # a partial dict keeps the on-demand defaults for missing keys
    assert StructureConfig.from_dict({"glossary": True}) == StructureConfig(glossary=True)


def test_no_activity_mentions_when_disabled():
    cfg = StructureConfig()
    for prompt in (
        build_system_prompt(cfg),
        build_system_prompt(cfg, experimental=True),
        build_structure_block(cfg),
        build_expand_prompt(cfg),
    ):
        assert "δραστηριότητ" not in prompt.lower()


def test_format_module_drops_docx_activities_when_disabled():
    module = {"number": 1, "title": "T", "activities": "Άσκηση Χ", "skills": [{"name": "s", "type": "essential"}]}
    off = LLMService._format_module(None, module, cfg=StructureConfig())
    on = LLMService._format_module(None, module, cfg=StructureConfig(activities=True))
    assert "Άσκηση Χ" not in off and "πρακτική εφαρμογή" not in off
    assert "Άσκηση Χ" in on


def test_manifest_lists_forbidden_elements():
    manifest = build_structure_manifest(StructureConfig())
    assert "ΑΠΑΓΟΡΕΥΕΤΑΙ" in manifest
    for name in ("Δραστηριότητες", "Ερωτήσεις Αυτοαξιολόγησης", "Γλωσσάρι", "Βασικές λέξεις"):
        assert name in manifest
    assert build_structure_manifest(ALL_ON) == ""


def test_detect_structure_violations():
    text = (
        "## 1.1 Θέμα\n**Βασικές λέξεις:** α, β\nΚείμενο.\n\n"
        "**Δραστηριότητα 1: Ανάλυση**\n\n## Γλωσσάρι\nΌρος: ορισμός\n"
    )
    assert detect_structure_violations(text, StructureConfig()) == [
        "activities", "glossary", "subsection_keywords",
    ]
    assert detect_structure_violations(text, ALL_ON) == []


def test_page_budget_counts_end_sections():
    body_off, _ = page_budget(10, StructureConfig(), mcq_count=0)
    body_on, end_on = page_budget(10, StructureConfig(self_assessment=True, glossary=True), mcq_count=20)
    assert body_on < body_off <= 10
    assert body_on + end_on <= 10.5
    assert page_budget(1, StructureConfig(), 0)[0] == 1


UPLOADED_DRAFT = """## 1.1 Θέμα
Κείμενο του χρήστη.

## Ερωτήσεις Αυτοαξιολόγησης
1. Ερώτηση
α) Α

## Γλωσσάρι
Όρος: ορισμός
"""


def test_revision_keeps_elements_the_draft_already_has():
    cfg = revision_config(StructureConfig(), UPLOADED_DRAFT)
    assert cfg.self_assessment and cfg.glossary
    assert not cfg.activities and not cfg.subsection_keywords
    manifest = build_structure_manifest(cfg)
    forbidden = manifest.split("ΑΠΑΓΟΡΕΥΕΤΑΙ", 1)[1]
    assert "Γλωσσάρι" not in forbidden and "Ερωτήσεις Αυτοαξιολόγησης" not in forbidden
    assert "Δραστηριότητες" in forbidden  # adding a disabled element stays forbidden


def test_revision_flags_only_newly_added_elements():
    revised = UPLOADED_DRAFT + "\n**Δραστηριότητα 1: Νέα**\n"
    cfg = revision_config(StructureConfig(), UPLOADED_DRAFT)
    assert detect_structure_violations(revised, cfg) == ["activities"]
    assert detect_structure_violations(UPLOADED_DRAFT, cfg) == []


def test_no_citations_mode_removes_citation_rules_and_forbids_them():
    cfg = StructureConfig(in_text_citations=False)
    prompt = build_system_prompt(cfg, experimental=True)
    assert "(Smith, 2022)" not in prompt and "ΣΤΥΛ ΠΑΡΕΝΘΕΤΙΚΗΣ" not in prompt
    assert "Βιβλιογραφία" in prompt  # the bibliography stays
    forbidden = build_structure_manifest(cfg).split("ΑΠΑΓΟΡΕΥΕΤΑΙ", 1)[1]
    assert "Ενδοκειμενικές αναφορές" in forbidden


def test_citations_are_detected_only_in_the_body():
    cfg = StructureConfig(in_text_citations=False)
    body = "Κείμενο με παραπομπή (Porter & Kramer, 2011).\n"
    biblio = "## Βιβλιογραφία\nPorter, M. E., & Kramer, M. R. (2011). Creating shared value.\n"
    assert detect_structure_violations(body + biblio, cfg) == ["in_text_citations"]
    assert detect_structure_violations(biblio, cfg) == []


def test_elements_requested_by_the_user_are_not_violations():
    assert elements_requested("Πρόσθεσε 2 Δραστηριότητες και ένα ΓΛΩΣΣΑΡΙ") == {"activities", "glossary"}
    text = "**Δραστηριότητα 1: Άσκηση**\n"
    assert detect_structure_violations(text, StructureConfig()) == ["activities"]
    assert detect_structure_violations(text, StructureConfig(), "πρόσθεσε δραστηριότητες") == []


def test_negated_mentions_are_prohibitions_not_requests():
    cases = {
        "Μην βάλεις δραστηριότητες": set(),
        "Μην βάλεις δραστηριότητες και γλωσσάρι": set(),
        "Δεν θέλω ερωτήσεις αυτοαξιολόγησης": set(),
        "Αφαίρεσε το γλωσσάρι": set(),
        "Χωρίς γλωσσάρι, πρόσθεσε δραστηριότητες": {"activities"},
        "Πρόσθεσε δραστηριότητες και μην βάλεις γλωσσάρι": {"activities"},
        "Μην βάλεις δραστηριότητες στην αρχή. Πρόσθεσε δραστηριότητες στο τέλος.": {"activities"},
        "Please add a glossary, no activities": {"glossary"},
    }
    for text, expected in cases.items():
        assert elements_requested(text) == expected, text


def test_negated_instruction_still_flags_the_element():
    text = "**Δραστηριότητα 1: Άσκηση**\n"
    assert detect_structure_violations(text, StructureConfig(), "Μην βάλεις δραστηριότητες") == ["activities"]


def test_summary_scales_with_module_and_is_reserved_in_the_budget():
    from edu_backend.services.llm_service import summary_word_target
    assert summary_word_target(7) == 150          # small module → short summary
    assert summary_word_target(55) == 784         # ~5% of 55 pages
    assert summary_word_target(200) == 800        # capped
    body, _ = page_budget(7, StructureConfig(), 0)
    body_last, _ = page_budget(7, StructureConfig(), 0, reserved_pages=150 / 285)
    assert body_last < body

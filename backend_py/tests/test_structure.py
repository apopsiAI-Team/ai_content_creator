from edu_backend.prompts.structure import (
    ALL_ON,
    StructureConfig,
    build_expand_prompt,
    build_structure_block,
    build_structure_manifest,
    build_system_prompt,
    detect_structure_violations,
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

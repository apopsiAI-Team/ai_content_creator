import asyncio
from types import SimpleNamespace

from edu_backend.services import citation_relevance as rel
from edu_backend.services import citation_verifier as cv

CONTENT = """## 1.1 Θέμα
Η αποδοχή της τεχνολογίας εξαρτάται από τη χρησιμότητα (Davis, 1989). Οι αισθητήρες μειώνουν
την κατανάλωση των κτιρίων κατά 20% (Davis, 1989).

## Βιβλιογραφία
Davis, F. D. (1989). Perceived usefulness, perceived ease of use, and user acceptance of information technology. *MIS Quarterly*, *13*(3), 319–340.
Porter, M. E. (1985). *Competitive advantage*. Free Press.
"""


def test_citation_contexts_return_the_citing_sentence():
    body, _ = cv._split_sections(CONTENT)
    sentences = [s for _, _, s in cv.extract_citation_contexts(body)]
    assert sentences[0].startswith("Η αποδοχή της τεχνολογίας")
    assert sentences[1].startswith("Οι αισθητήρες") and "20%" in sentences[1]


def test_uncited_entries_and_relevance_annotation(monkeypatch):
    async def fake_verify(client, entry, semaphore=None):
        return cv._result(entry, "verified", True)

    seen = {}

    async def fake_relevance(results, user_id="anonymous"):
        seen["contexts"] = {r["text"][:5]: r.get("contexts") for r in results}
        for r in results:
            if r.get("contexts"):
                r["relevance"], r["relevance_reason"] = "unrelated", "άλλο θέμα"

    monkeypatch.setattr(cv, "verify_entry", fake_verify)
    monkeypatch.setattr(cv, "assess_relevance", fake_relevance)
    result = asyncio.run(cv.verify_bibliography(CONTENT))

    assert result["uncited_entries"] == [result["entries"][1]["text"]]  # Porter is never cited
    assert len(seen["contexts"]["Davis"]) == 2
    assert result["entries"][0]["relevance"] == "unrelated"
    assert "abstract" not in result["entries"][0]  # internal field stripped


def test_no_uncited_flags_when_text_has_no_citations(monkeypatch):
    async def fake_verify(client, entry, semaphore=None):
        return cv._result(entry, "verified", True)

    monkeypatch.setattr(cv, "verify_entry", fake_verify)
    no_cites = CONTENT.replace(" (Davis, 1989)", "")
    assert asyncio.run(cv.verify_bibliography(no_cites))["uncited_entries"] == []


def test_assess_relevance_parses_model_verdicts(monkeypatch):
    reply = '{"results": [{"id": 1, "verdict": "unrelated", "reason": "αφορά αποδοχή τεχνολογίας"}, {"id": 2, "verdict": "bogus"}]}'

    async def create(**kwargs):
        assert "Οι αισθητήρες" in kwargs["messages"][0]["content"]
        return SimpleNamespace(
            content=[SimpleNamespace(type="text", text=reply)],
            usage=SimpleNamespace(input_tokens=10, output_tokens=10),
        )

    monkeypatch.setattr(rel, "_get_client", lambda: SimpleNamespace(messages=SimpleNamespace(create=create)))
    monkeypatch.setattr(rel.settings, "anthropic_api_key", "test")
    results = [
        {"text": "Davis (1989)", "status": "verified", "contexts": ["Οι αισθητήρες μειώνουν ... (Davis, 1989)."]},
        {"text": "Porter (1985)", "status": "verified", "contexts": ["Στρατηγική (Porter, 1985)."]},
        {"text": "Χωρίς παραπομπή", "status": "verified", "contexts": []},
    ]
    asyncio.run(rel.assess_relevance(results))
    assert results[0]["relevance"] == "unrelated"
    assert "relevance" not in results[1]  # unknown verdict ignored
    assert "relevance" not in results[2]  # not cited → not judged


def test_openalex_abstract_is_rebuilt():
    assert cv._openalex_abstract({"energy": [1], "Building": [0], "use": [2]}) == "Building energy use"

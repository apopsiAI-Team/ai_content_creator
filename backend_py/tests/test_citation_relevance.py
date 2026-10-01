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


def test_no_citations_skips_uncited_and_runs_the_topic_check(monkeypatch):
    async def fake_verify(client, entry, semaphore=None):
        return cv._result(entry, "verified", True)

    calls = {}

    async def fake_sentence_check(results, user_id="anonymous"):
        calls["sentence"] = True

    async def fake_topic_check(results, topic, headings, user_id="anonymous"):
        calls["topic"] = (topic, headings)

    monkeypatch.setattr(cv, "verify_entry", fake_verify)
    monkeypatch.setattr(cv, "assess_relevance", fake_sentence_check)
    monkeypatch.setattr(cv, "assess_topic_relevance", fake_topic_check)
    no_cites = CONTENT.replace(" (Davis, 1989)", "")
    result = asyncio.run(cv.verify_bibliography(no_cites, topic="Ενεργειακή απόδοση κτιρίων"))

    assert result["uncited_entries"] == []          # every entry is uncited by design
    assert "sentence" not in calls                  # no (source, sentence) pairs
    assert calls["topic"] == ("Ενεργειακή απόδοση κτιρίων", ["1.1 Θέμα"])


def test_topic_check_marks_off_topic_sources(monkeypatch):
    reply = '{"results": [{"id": 1, "verdict": "unrelated", "reason": "ιατρική, όχι ενέργεια"}, {"id": 2, "verdict": "related", "reason": "ενέργεια κτιρίων"}]}'

    async def create(**kwargs):
        prompt = kwargs["messages"][0]["content"]
        assert "ΘΕΜΑ ΕΝΟΤΗΤΑΣ: Ενέργεια" in prompt and "- 1.1 Βασικές έννοιες" in prompt
        return SimpleNamespace(
            content=[SimpleNamespace(type="text", text=reply)],
            usage=SimpleNamespace(input_tokens=10, output_tokens=10),
        )

    monkeypatch.setattr(rel, "_get_client", lambda: SimpleNamespace(messages=SimpleNamespace(create=create)))
    monkeypatch.setattr(rel.settings, "anthropic_api_key", "test")
    results = [
        {"text": "Gray's anatomy (2020)", "status": "verified"},
        {"text": "Pérez-Lombard (2008)", "status": "verified"},
        {"text": "Κάποιος (2020) [Χρειάζεται επαλήθευση]", "status": "placeholder"},
    ]
    asyncio.run(rel.assess_topic_relevance(results, "Ενέργεια", ["1.1 Βασικές έννοιες"]))
    assert (results[0]["relevance"], results[0]["relevance_scope"]) == ("unrelated", "topic")
    assert results[1]["relevance"] == "related"
    assert "relevance" not in results[2]  # placeholders are not judged


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

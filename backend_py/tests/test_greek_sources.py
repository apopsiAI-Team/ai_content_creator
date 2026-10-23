import asyncio

import httpx

from edu_backend.services import citation_verifier as cv
from edu_backend.services import greek_sources as gs

CONTENT = """## 1.1 Θερμομόνωση
Η θερμομόνωση μειώνει τις απώλειες θερμότητας του κελύφους (Smith, 2020). Τα υλικά διαφέρουν
ως προς τη θερμική αγωγιμότητα.

## Βιβλιογραφία
- Smith, J. (2020). Building physics. *Energy Journal*, *1*(1), 1–10.

## Γλωσσάρι
Κέλυφος: το περίβλημα του κτιρίου
"""

BOOK = gs.Candidate(
    kind="kallipos",
    title="Τα θερμομονωτικά υλικά και η θερμομονωτική προστασία των κτιρίων",
    authors=[("Aravantinos", "Dimitrios")],
    year=2023,
    doi="10.57713/kallipos-281",
    venue="Κάλλιπος",
)
ARTICLE = gs.Candidate(
    kind="journal",
    title="Υγεία και ασφάλεια των εργαζομένων στην Ελλάδα",
    authors=[("Μέκος", "Κωνσταντίνος"), ("Παπάς", "Ιωάννης"), ("Νίκου", "Άννα")],
    year=2016,
    doi="10.12681/scad.8958",
    venue="Social Cohesion and Development",
    volume="5",
    issue="2",
)


def test_apa_and_citation_label():
    assert gs.format_apa(BOOK) == (
        "Aravantinos, D. (2023). *Τα θερμομονωτικά υλικά και η θερμομονωτική προστασία των κτιρίων*. "
        "Κάλλιπος, Ανοικτές Ακαδημαϊκές Εκδόσεις. https://doi.org/10.57713/kallipos-281"
    )
    assert gs.citation_label(BOOK) == "Aravantinos, 2023"
    assert gs.format_apa(ARTICLE).startswith(
        "Μέκος, Κ., Παπάς, Ι., & Νίκου, Ά. (2016). Υγεία και ασφάλεια των εργαζομένων στην Ελλάδα. "
        "*Social Cohesion and Development*, *5*(2)."
    )
    assert gs.citation_label(ARTICLE) == "Μέκος et al., 2016"


def test_generated_entries_parse_like_model_entries():
    for cand in (BOOK, ARTICLE):
        entry = cv._parse_entry(gs.format_apa(cand))
        assert entry.surname == cand.authors[0][0]
        assert entry.year == cand.year
        assert entry.doi == cand.doi
        assert cv._title_similarity(entry.title, cand.title) >= 0.85


def test_apply_adds_citation_and_entry_without_replacing():
    sug = {"key": BOOK.doi, "apa": gs.format_apa(BOOK), "citation": gs.citation_label(BOOK),
           "doi": BOOK.doi, "sentence": "Η θερμομόνωση μειώνει τις απώλειες θερμότητας του κελύφους"}
    out = gs.apply_greek_sources(CONTENT, [sug])
    content = out["content"]
    assert out["applied"] == [BOOK.doi]
    # Appended to the existing parenthetical — Smith stays.
    assert "κελύφους (Smith, 2020; Aravantinos, 2023)." in content
    # Entry goes into the Βιβλιογραφία (same list marker), before the Γλωσσάρι.
    biblio = content.split("## Βιβλιογραφία")[1].split("## Γλωσσάρι")[0]
    assert "- Smith, J. (2020)" in biblio and f"- {gs.format_apa(BOOK)}" in biblio
    # Applying again does not duplicate the bibliography entry.
    again = gs.apply_greek_sources(content, [sug])["content"]
    assert again.count("kallipos-281") == 1


def test_apply_new_parenthetical_and_unplaced():
    sug = {"key": "a", "apa": gs.format_apa(ARTICLE), "citation": gs.citation_label(ARTICLE),
           "sentence": "Τα υλικά διαφέρουν ως προς τη θερμική αγωγιμότητα."}
    content = gs.apply_greek_sources(CONTENT, [sug])["content"]
    assert "θερμική αγωγιμότητα (Μέκος et al., 2016)." in content

    missing = dict(sug, sentence="Μια πρόταση που δεν υπάρχει πουθενά στο κείμενο αυτό.")
    out = gs.apply_greek_sources(CONTENT, [missing])
    assert out["unplaced"] == ["a"] and out["content"] == CONTENT


def test_apply_without_citations_only_adds_entry():
    sug = {"key": "b", "apa": gs.format_apa(BOOK), "citation": gs.citation_label(BOOK), "sentence": None}
    content = gs.apply_greek_sources(CONTENT, [sug])["content"]
    assert "Aravantinos, 2023" not in content.split("## Βιβλιογραφία")[0]
    assert gs.format_apa(BOOK) in content


def test_sentence_spans_ignore_punctuation_inside_citations():
    text = "Πρώτη πρόταση (Smith et al., 2020; Jones, 2019). Δεύτερη πρόταση."
    spans = [text[s:e].strip() for s, e in gs._sentence_spans(text)]
    assert spans == ["Πρώτη πρόταση (Smith et al., 2020; Jones, 2019).", "Δεύτερη πρόταση."]


def test_crossref_filters_forbidden_and_non_works():
    master = {"DOI": "10.26240/heal.ntua.1", "type": "dissertation", "title": ["Ενεργειακή απόδοση"],
              "issued": {"date-parts": [[2011]]}}
    phd = {"DOI": "10.12681/eadd/48220", "type": "dissertation", "title": ["ΣΔΙΤ και ενεργειακή απόδοση"],
           "author": [{"family": "Μανιάτης", "given": "Αντώνιος"}], "issued": {"date-parts": [[2020]]},
           "institution": [{"name": "ΕΚΠΑ, Σχολή Νομικής"}]}
    preface = {"DOI": "10.12681/x.1", "type": "journal-article", "title": ["Πρόλογος των επιμελητών"],
               "issued": {"date-parts": [[2020]]}}
    assert gs._crossref_to_candidate(master) is None          # PhDs only from the ΕΚΤ archive
    assert gs._crossref_to_candidate(preface) is None
    cand = gs._crossref_to_candidate(phd)
    assert cand.kind == "doctoral" and cand.institution == "ΕΚΠΑ"
    assert "[Διδακτορική διατριβή, ΕΚΠΑ]" in gs.format_apa(cand)


def test_rank_kallipos_prefers_topic_matches():
    other = gs.Candidate(kind="kallipos", title="Βελτίωση φυτών", authors=[("X", "")], year=2020,
                         doi="10.57713/kallipos-1", subjects=["ΓΕΝΕΤΙΚΗ"])
    book = gs.Candidate(**{**BOOK.__dict__, "subjects": ["ΘΕΡΜΟΜΟΝΩΣΗ", "ΚΤΙΡΙΑ"]})
    ranked = gs.rank_kallipos([other, book], ["Θερμομόνωση κτιρίων"])
    assert ranked == [book]


def test_verifier_resolves_datacite_doi():
    """Κάλλιπος DOIs live at DataCite, not CrossRef — they must not be flagged invalid."""
    attrs = {
        "doi": "10.57713/kallipos-281",
        "titles": [{"title": "Thermal insulation materials", "lang": "en"},
                   {"title": BOOK.title, "lang": "el"}],
        "creators": [{"familyName": "Aravantinos", "givenName": "Dimitrios"}],
        "publicationYear": 2023,
        "publisher": "Kallipos, Open Academic Editions",
        "descriptions": [],
    }

    def handler(request: httpx.Request) -> httpx.Response:
        url = str(request.url)
        if "api.datacite.org/dois/10.57713/kallipos-281" in url:
            return httpx.Response(200, json={"data": {"attributes": attrs}})
        return httpx.Response(404)

    async def run():
        async with httpx.AsyncClient(transport=httpx.MockTransport(handler)) as client:
            return await cv.verify_entry(client, cv._parse_entry(gs.format_apa(BOOK)))
    cv._cache.clear()
    result = asyncio.run(run())
    assert result["status"] == "verified" and result["source"] == "datacite"


def test_country_words_never_count_as_a_match():
    politics = gs.Candidate(kind="kallipos", title="Ελληνική πολιτική", authors=[("X", "")], year=2023,
                            doi="10.57713/kallipos-2", subjects=["ΕΛΛΑΔΑ"])
    fisheries = gs.Candidate(kind="kallipos", title="Αλιευτική βιολογία και αλιεία", authors=[("Y", "")],
                             year=2016, doi="10.57713/kallipos-3", subjects=["ΑΛΙΕΙΑ", "ΑΛΙΕΥΤΙΚΑ ΑΠΟΘΕΜΑΤΑ"])
    assert gs.rank_kallipos([politics, fisheries], ["Ελληνική αλιεία", "αλιευτικά αποθέματα"]) == [fisheries]


def test_integration_block_rules_follow_citation_setting():
    with_cites = gs.build_integration_block([BOOK], in_text_citations=True)
    assert "Παραπομπή: (Aravantinos, 2023)" in with_cites
    assert gs.format_apa(BOOK) in with_cites
    assert "ΜΗΝ τους αποδώσεις" in with_cites and "ΔΕΝ είναι υποχρεωτικό" in with_cites
    no_cites = gs.build_integration_block([BOOK], in_text_citations=False)
    assert "Παραπομπή:" not in no_cites and gs.format_apa(BOOK) in no_cites
    assert gs.build_integration_block([]) == ""


def test_query_generation_falls_back_to_topic_and_headings(monkeypatch):
    async def fail(prompt, user_id):
        return ""
    monkeypatch.setattr(gs, "_ask", fail)
    greek, english = asyncio.run(gs.generate_queries("Θερμομόνωση", "## 1.1 Υλικά\n## Βιβλιογραφία", "u"))
    assert greek == ["Θερμομόνωση", "Υλικά"] and english == []

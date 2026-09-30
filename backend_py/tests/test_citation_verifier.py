import asyncio

import httpx

from edu_backend.services import citation_verifier as cv

CONTENT = """## 1.1 Εισαγωγή
Κείμενο (Davis, 1989). Άλλο (Porter & Kramer, 2011; Άγνωστος, 2020).

## Βιβλιογραφία
- Davis, F. D. (1989). Perceived usefulness, perceived ease of use, and user acceptance of information technology. *MIS Quarterly*, *13*(3), 319–340. https://doi.org/10.2307/249008
- Porter, M. E., & Kramer, M. R. (2011). Creating shared value. *Harvard Business Review*, *89*(1/2), 62–77. https://doi.org/10.1111/wrong.2011
- Fake, A. (2021). Totally invented study of nothing at all. *Journal of Nowhere*, *1*(1), 1–2.
- Κάποιος, Α. (2020). [Τίτλος δεν διατίθεται]. [Χρειάζεται επαλήθευση]

## Γλωσσάρι
Όρος: ορισμός (2020)
"""

DAVIS = {
    "DOI": "10.2307/249008",
    "title": ["Perceived Usefulness, Perceived Ease of Use, and User Acceptance of Information Technology"],
    "author": [{"family": "Davis"}],
    "issued": {"date-parts": [[1989]]},
}
PORTER = {
    "DOI": "10.5555/hbr.2011.real",
    "title": ["Creating Shared Value"],
    "author": [{"family": "Porter"}, {"family": "Kramer"}],
    "issued": {"date-parts": [[2011]]},
}


def _handler(request: httpx.Request) -> httpx.Response:
    url = str(request.url)
    if "api.crossref.org/works/10.2307/249008" in url:
        return httpx.Response(200, json={"message": DAVIS})
    if "api.crossref.org/works/" in url:
        return httpx.Response(404)
    if "api.crossref.org/works" in url:
        query = request.url.params.get("query.bibliographic", "")
        items = [PORTER] if "Creating shared value" in query else []
        return httpx.Response(200, json={"message": {"items": items}})
    if "api.openalex.org" in url:
        return httpx.Response(200, json={"results": []})
    return httpx.Response(500)


def _verify_all(content: str) -> list[dict]:
    async def run():
        async with httpx.AsyncClient(transport=httpx.MockTransport(_handler)) as client:
            return [await cv.verify_entry(client, e) for e in cv.parse_bibliography(content)]
    cv._cache.clear()
    return asyncio.run(run())


def test_parse_bibliography_only_reads_bibliography_section():
    entries = cv.parse_bibliography(CONTENT)
    assert [e.surname for e in entries] == ["Davis", "Porter", "Fake", "Κάποιος"]
    assert entries[0].doi == "10.2307/249008"
    assert entries[0].year == 1989
    assert entries[1].title == "Creating shared value"


def test_verify_statuses():
    statuses = [r["status"] for r in _verify_all(CONTENT)]
    assert statuses == ["verified", "doi_corrected", "unverified", "placeholder"]


def test_doi_fix_and_orphans():
    results = _verify_all(CONTENT)
    fixed = cv._apply_doi_fixes(CONTENT, [dict(r) for r in results])
    assert "10.5555/hbr.2011.real" in fixed and "10.1111/wrong.2011" not in fixed

    body, _ = cv._split_sections(CONTENT)
    cites = [raw for _, _, raw in cv.extract_in_text_citations(body)]
    assert cites == ["Davis, 1989", "Porter & Kramer, 2011", "Άγνωστος, 2020"]


def test_network_error_is_unverified():
    def boom(request):
        raise httpx.ConnectError("down")

    async def run():
        async with httpx.AsyncClient(transport=httpx.MockTransport(boom)) as client:
            entry = cv.parse_bibliography(CONTENT)[2]
            return await cv.verify_entry(client, entry)
    cv._cache.clear()
    assert asyncio.run(run())["status"] == "unverified"


ORG_CONTENT = """## 1.1 Θέμα
Η ψηφιοποίηση αλλάζει την ενέργεια (IEA, 2017). Οι αρχές ΤΝ (OECD, 2019).
Στατιστικά στοιχεία (Ελληνική Στατιστική Αρχή, 2021). Υγεία (WHO, 2020). Άλλο (ILO, 2018).

## Βιβλιογραφία
International Energy Agency. (2017). *Digitalization and energy*. IEA.
Organisation for Economic Co-operation and Development [OECD]. (2019). *Recommendation of the Council on Artificial Intelligence*. OECD.
ΕΛΣΤΑΤ. (2021). *Έρευνα εργατικού δυναμικού*. ΕΛΣΤΑΤ.
World Health Organization. (2020). *World health statistics 2020*. WHO.
"""


def test_organisation_acronyms_are_matched():
    entries = cv.parse_bibliography(ORG_CONTENT)
    assert "iea" in entries[0].aliases
    assert "oecd" in entries[1].aliases  # acronym in [brackets]

    body, _ = cv._split_sections(ORG_CONTENT)
    citations = cv.extract_in_text_citations(body)
    orphans = [
        raw for aliases, year, raw in citations
        if not any(e.year == year and cv._names_match(aliases, e.aliases) for e in entries)
    ]
    # IEA/OECD/WHO resolve; full name cited against an acronym-only entry and
    # an unknown acronym stay orphans (APA requires the acronym to be defined).
    assert orphans == ["Ελληνική Στατιστική Αρχή, 2021", "ILO, 2018"]


def test_short_acronym_never_matches_by_substring():
    assert not cv._names_match(frozenset({"iea"}), frozenset({"liea"}))
    assert cv._names_match(frozenset({"aalst"}), frozenset({"van der aalst"}))


def test_full_name_in_text_matches_acronym_entry():
    cited = cv._name_aliases("International Energy Agency")
    entry = cv._parse_entry("IEA. (2017). *Digitalization and energy*. IEA Publications.")
    assert cv._names_match(cited, entry.aliases)

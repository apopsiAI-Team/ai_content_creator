"""
Citation Verifier — checks LLM-written bibliography entries against
CrossRef, OpenAlex and Open Library (books), plus a URL check for reports.

The model writes references from its own knowledge ("Πειραματική" mode), so
every entry of the generated "## Βιβλιογραφία" is checked after generation:

  verified       the work was found (or its cited web URL resolves); its DOI (if any) is correct
  doi_corrected  the work was found but the DOI was wrong → ``suggested_doi``
  doi_invalid    the DOI is wrong / does not resolve and no correct DOI is known
  unverified     no DOI and nothing matching was found
  placeholder    the model marked it "[Χρειάζεται επαλήθευση]" / "[Τίτλος δεν διατίθεται]"
  thesis         undergraduate/master thesis (forbidden source type)

Nothing is deleted: wrong DOIs are fixed or removed in the returned content,
everything else is only flagged for the user. Network errors never raise —
the entry is simply reported ``unverified``.
"""
from __future__ import annotations

import asyncio
import re
import unicodedata
from dataclasses import dataclass
from difflib import SequenceMatcher
from typing import Optional

import httpx

from ..config import settings
from .research_service import THESIS_KEYWORDS

TITLE_MATCH_THRESHOLD = 0.85
HTTP_TIMEOUT_SECONDS = 8.0
MAX_CONCURRENT_LOOKUPS = 5
_CACHE_MAX_ENTRIES = 2000

_BIBLIO_HEADING = re.compile(
    r"^\s*(?:#{1,4}\s*|\*\*)\s*(?:\d+\.\s*)?(?:Βιβλιογραφία|ΒΙΒΛΙΟΓΡΑΦΙΑ|Βιβλιογραφικές αναφορές|References)\b",
    re.IGNORECASE,
)
_ANY_HEADING = re.compile(r"^\s*#{1,6}\s")
_DOI = re.compile(r"10\.\d{4,9}/[^\s<>\"]+")
_URL = re.compile(r"https?://[^\s<>\"]+")
_YEAR = re.compile(r"\((\d{4})[a-z]?(?:,[^)]*)?\)|\((n\.d\.)\)")
_PLACEHOLDER = re.compile(r"\[(?:Χρειάζεται επαλήθευση|Τίτλος δεν διατίθεται)\]", re.IGNORECASE)
_LIST_MARKER = re.compile(r"^\s*(?:[-*•]\s+|\d+[.)]\s+)")
_PARENTHETICAL = re.compile(r"\(([^()]*?\d{4}[a-z]?)\)")
_CITATION_PART = re.compile(r"^\s*([^\d;()]+?)\s*,?\s*(\d{4})[a-z]?\b")

_cache: dict[str, dict] = {}


@dataclass
class BibEntry:
    text: str
    surname: str
    year: Optional[int]
    title: str
    doi: Optional[str]
    # Normalised names the first author may be cited by in the text — the
    # surname, or for an organisation also its acronym ("iea", "oecd").
    aliases: frozenset[str] = frozenset()


# =============================================================================
# Parsing
# =============================================================================

def _norm(text: str) -> str:
    """Lowercase, strip accents/markdown/punctuation, collapse whitespace."""
    text = unicodedata.normalize("NFKD", text or "")
    text = "".join(c for c in text if not unicodedata.combining(c))
    text = re.sub(r"[^\w\s]", " ", text.lower())
    return re.sub(r"\s+", " ", text).strip()


def _split_sections(content: str) -> tuple[str, list[str]]:
    """Return (body_text, bibliography_lines). Handles several bibliography
    sections (e.g. after an auto-continuation)."""
    body, biblio = [], []
    in_biblio = False
    for line in content.split("\n"):
        if _BIBLIO_HEADING.match(line):
            in_biblio = True
            continue
        if in_biblio and _ANY_HEADING.match(line):
            in_biblio = False
        (biblio if in_biblio else body).append(line)
    return "\n".join(body), biblio


def _clean_doi(raw: str) -> str:
    return raw.rstrip(".,;:)]*_")


def _parse_entry(line: str) -> Optional[BibEntry]:
    text = _LIST_MARKER.sub("", line).strip()
    if len(text) < 20 or not _YEAR.search(text):
        return None

    doi_match = _DOI.search(text)
    doi = _clean_doi(doi_match.group(0)) if doi_match else None

    year_match = _YEAR.search(text)
    year = int(year_match.group(1)) if year_match and year_match.group(1) else None

    # First author surname (or organisation): up to the first "," / "(" / ". ("
    head = text[: year_match.start()] if year_match else text
    surname = re.split(r",|\(|\.\s*$", head.replace("*", ""), maxsplit=1)[0].strip(" .")

    # Title: right after "(Year). " — italic (*...*) for books, else up to the next ". "
    title = ""
    if year_match:
        rest = text[year_match.end():].lstrip(". ").strip()
        if rest.startswith("*"):
            end = rest.find("*", 1)
            title = rest[1:end] if end > 0 else rest[1:]
        else:
            title = re.split(r"\.\s|\.$|\*", rest, maxsplit=1)[0]
    # "Porter, M. E., & ..." is a person; "International Energy Agency." an organisation.
    is_person = "," in head
    aliases = frozenset({_norm(surname)}) if is_person else _name_aliases(head)
    return BibEntry(text=text, surname=surname, year=year, title=title.strip(), doi=doi, aliases=aliases)


_ACRONYM_IN_BRACKETS = re.compile(r"[\[(]\s*([^\W\d_][\w.&-]{1,14})\s*[\])]")
_NAME_STOPWORDS = {
    "of", "the", "and", "for", "on", "in", "de", "des", "du", "la", "le", "und", "fur",
    "και", "της", "του", "των", "για", "στην", "στο",
}


def _name_aliases(name: str) -> frozenset[str]:
    """Normalised forms an author/organisation name may be cited by:
    the full name, the name without a bracketed acronym, the acronym in
    brackets ("... Development [OECD]"), and the initials of a multi-word
    name ("International Energy Agency" → "iea")."""
    name = name.replace("*", "").strip(" .")
    aliases = {_norm(name)}
    bracket = _ACRONYM_IN_BRACKETS.search(name)
    if bracket:
        aliases.add(_norm(bracket.group(1)))
        aliases.add(_norm(_ACRONYM_IN_BRACKETS.sub("", name)))
    words = [
        w for w in re.findall(r"[^\W\d_]+", _ACRONYM_IN_BRACKETS.sub("", name))
        if _norm(w) not in _NAME_STOPWORDS
    ]
    if len(words) >= 2:
        aliases.add(_norm("".join(w[0] for w in words)))
    aliases.discard("")
    return frozenset(aliases)


def _names_match(cited: frozenset[str], entry: frozenset[str]) -> bool:
    """Exact match on any alias; substring match only between longer forms
    (e.g. "aalst" vs "van der aalst") so short acronyms never match by accident."""
    if cited & entry:
        return True
    return any(
        (a in b or b in a) for a in cited for b in entry if min(len(a), len(b)) >= 4
    )


def parse_bibliography(content: str) -> list[BibEntry]:
    _, lines = _split_sections(content)
    entries = []
    for line in lines:
        entry = _parse_entry(line)
        if entry:
            entries.append(entry)
    return entries


def extract_in_text_citations(body: str) -> list[tuple[frozenset[str], int, str]]:
    """Return (first-author aliases, year, raw text) for each parenthetical
    APA citation, e.g. "(Smith & Jones, 2020; OECD, 2019)"."""
    found = []
    for match in _PARENTHETICAL.finditer(body):
        for part in match.group(1).split(";"):
            cite = _CITATION_PART.match(part)
            if not cite:
                continue
            authors = cite.group(1)
            first = re.split(r"\s+&\s+|\s+και\s+|\s+et al\.?|,", authors, maxsplit=1)[0]
            aliases = _name_aliases(first)
            if aliases:
                found.append((aliases, int(cite.group(2)), part.strip()))
    return found


# =============================================================================
# Matching
# =============================================================================

def _title_similarity(a: str, b: str) -> float:
    na, nb = _norm(a), _norm(b)
    if not na or not nb:
        return 0.0
    short, long_ = sorted((na, nb), key=len)
    # Tolerate a missing/extra subtitle ("Title" vs "Title: subtitle").
    if len(short) >= 20 and long_.startswith(short):
        return 0.95
    return SequenceMatcher(None, na, nb).ratio()


def _author_matches(surname: str, candidate_names: list[str]) -> bool:
    ns = _norm(surname)
    if not ns or not candidate_names:
        return False
    names = [n for n in (_norm(name) for name in candidate_names) if n]
    return any(ns in n or (len(n) >= 4 and n in ns) for n in names)


def _is_thesis(*texts: str) -> bool:
    joined = " ".join(t for t in texts if t).lower()
    return any(kw in joined for kw in THESIS_KEYWORDS)


@dataclass
class _Candidate:
    title: str
    authors: list[str]
    year: Optional[int]
    doi: Optional[str]
    container: str
    source: str


def _crossref_candidate(item: dict) -> _Candidate:
    year = None
    for field in ("issued", "published-print", "published-online", "created"):
        parts = (item.get(field) or {}).get("date-parts") or [[None]]
        if parts and parts[0] and parts[0][0]:
            year = parts[0][0]
            break
    authors = [a.get("family") or a.get("name") or "" for a in item.get("author", [])]
    authors += [e.get("family") or e.get("name") or "" for e in item.get("editor", [])]
    container = item.get("container-title") or [""]
    return _Candidate(
        title=(item.get("title") or [""])[0],
        authors=authors,
        year=year,
        doi=item.get("DOI"),
        container=container[0] if isinstance(container, list) and container else str(container),
        source="crossref",
    )


def _openalex_candidate(item: dict) -> _Candidate:
    doi = item.get("doi") or ""
    doi = doi.replace("https://doi.org/", "") or None
    authors = [
        ((a.get("author") or {}).get("display_name") or "") for a in item.get("authorships", [])
    ]
    venue = ((item.get("primary_location") or {}).get("source") or {}).get("display_name") or ""
    return _Candidate(
        title=item.get("display_name") or item.get("title") or "",
        authors=authors,
        year=item.get("publication_year"),
        doi=doi,
        container=venue,
        source="openalex",
    )


def _matches(entry: BibEntry, cand: _Candidate) -> bool:
    if _title_similarity(entry.title, cand.title) < TITLE_MATCH_THRESHOLD:
        return False
    # Books are often indexed under a later edition/reprint — the year is not
    # required to match, only title + first author (or organisation).
    return _author_matches(entry.surname, cand.authors) or _author_matches(entry.surname, [cand.container])


def _same_edition(entry: BibEntry, cand: _Candidate) -> bool:
    """Stricter check used before swapping in a DOI: the year must also agree
    (±1), so a later reprint/chapter is never substituted for the original."""
    if entry.year is None or cand.year is None:
        return False
    return abs(entry.year - cand.year) <= 1


def _headers() -> dict:
    return {"User-Agent": f"EducationalMaterialCreator/1.0 (mailto:{settings.crossref_mailto})"}


async def _crossref_by_doi(client: httpx.AsyncClient, doi: str) -> Optional[_Candidate]:
    resp = await client.get(f"https://api.crossref.org/works/{doi}", headers=_headers())
    if resp.status_code != 200:
        return None
    return _crossref_candidate(resp.json().get("message", {}))


async def _crossref_search(client: httpx.AsyncClient, entry: BibEntry) -> list[_Candidate]:
    query = entry.text
    if entry.doi:
        query = query.replace(entry.doi, "")
    resp = await client.get(
        "https://api.crossref.org/works",
        params={
            "query.bibliographic": query[:400],
            "rows": 3,
            "select": "DOI,title,author,editor,issued,container-title,type",
        },
        headers=_headers(),
    )
    if resp.status_code != 200:
        return []
    return [_crossref_candidate(i) for i in resp.json().get("message", {}).get("items", [])]


async def _openalex_search(client: httpx.AsyncClient, entry: BibEntry) -> list[_Candidate]:
    if not entry.title:
        return []
    resp = await client.get(
        "https://api.openalex.org/works",
        params={"search": entry.title[:300], "per-page": 3, "mailto": settings.crossref_mailto},
        headers=_headers(),
    )
    if resp.status_code != 200:
        return []
    return [_openalex_candidate(i) for i in resp.json().get("results", [])]


async def _openlibrary_search(client: httpx.AsyncClient, entry: BibEntry) -> list[_Candidate]:
    """Books are poorly covered by CrossRef/OpenAlex (they mostly return
    reviews of the book), so fall back to Open Library."""
    if not entry.title:
        return []
    # Free-text `q` — the fielded `title=` search misses titles with punctuation.
    resp = await client.get(
        "https://openlibrary.org/search.json",
        params={
            "q": f"{_norm(entry.title)[:200]} {entry.surname}",
            "limit": 3,
            "fields": "title,subtitle,author_name,first_publish_year",
        },
        headers=_headers(),
    )
    if resp.status_code != 200:
        return []
    return [
        _Candidate(
            title=f"{d.get('title') or ''}: {d['subtitle']}" if d.get("subtitle") else d.get("title") or "",
            authors=d.get("author_name") or [],
            year=d.get("first_publish_year"),
            doi=None,
            container="",
            source="openlibrary",
        )
        for d in resp.json().get("docs", [])
    ]


async def _url_resolves(client: httpx.AsyncClient, text: str) -> Optional[str]:
    """For reports/web pages: the cited (non-DOI) URL must actually resolve."""
    match = _URL.search(text)
    if not match:
        return None
    url = match.group(0).rstrip(".,;)")
    if "doi.org/" in url:
        return None
    resp = await client.get(url, headers=_headers())
    return url if resp.status_code < 400 else None


def _result(entry: BibEntry, status: str, verified: bool, cand: Optional[_Candidate] = None,
            suggested_doi: Optional[str] = None) -> dict:
    return {
        "text": entry.text,
        "status": status,
        "verified": verified,
        "doi": entry.doi,
        "suggested_doi": suggested_doi,
        "matched_title": cand.title if cand else None,
        "source": cand.source if cand else None,
    }


async def verify_entry(
    client: httpx.AsyncClient, entry: BibEntry, semaphore: Optional[asyncio.Semaphore] = None
) -> dict:
    if _PLACEHOLDER.search(entry.text):
        return _result(entry, "placeholder", False)
    if _is_thesis(entry.text):
        return _result(entry, "thesis", False)

    key = _norm(entry.text)
    if key in _cache:
        return _cache[key]

    try:
        async with semaphore or asyncio.Semaphore(1):
            doi_ok = False
            if entry.doi:
                by_doi = await _crossref_by_doi(client, entry.doi)
                if by_doi and _matches(entry, by_doi):
                    doi_ok = True
                    result = _result(entry, "verified", True, by_doi)
                    if _is_thesis(by_doi.title, by_doi.container):
                        result["status"] = "thesis"
                        result["verified"] = False

            if not doi_ok:
                candidates: list[_Candidate] = []
                for search in (_crossref_search, _openalex_search, _openlibrary_search):
                    candidates = [c for c in await search(client, entry) if _matches(entry, c)]
                    if candidates:
                        break
                # Prefer the candidate from the same edition/year.
                candidates.sort(key=lambda c: not _same_edition(entry, c))
                match = candidates[0] if candidates else None

                url = None if (match or entry.doi) else await _url_resolves(client, entry.text)
                if url:
                    result = _result(entry, "verified", True)
                    result["source"] = "url"
                elif match is None:
                    result = _result(entry, "doi_invalid" if entry.doi else "unverified", False)
                elif _is_thesis(match.title, match.container):
                    result = _result(entry, "thesis", False, match)
                elif (entry.doi and match.doi and match.doi.lower() != entry.doi.lower()
                      and _same_edition(entry, match)):
                    result = _result(entry, "doi_corrected", True, match, suggested_doi=match.doi)
                elif entry.doi:
                    # Work exists but the written DOI is wrong and no DOI is known.
                    result = _result(entry, "doi_invalid", True, match)
                else:
                    result = _result(entry, "verified", True, match)
    except (httpx.HTTPError, ValueError) as e:
        print(f"Citation verifier lookup error: {e}")
        return _result(entry, "unverified", False)

    if len(_cache) >= _CACHE_MAX_ENTRIES:
        _cache.clear()
    _cache[key] = result
    return result


# =============================================================================
# Public API
# =============================================================================

def _apply_doi_fixes(content: str, results: list[dict]) -> str:
    """Replace wrong DOIs with the correct one; drop DOIs that are wrong and
    have no known replacement. Only the bibliography entry line is touched."""
    for r in results:
        doi = r.get("doi")
        if not doi or r["status"] not in ("doi_corrected", "doi_invalid"):
            continue
        old_line = r["text"]
        if r["status"] == "doi_corrected" and r.get("suggested_doi"):
            new_line = old_line.replace(doi, r["suggested_doi"])
        else:
            new_line = re.sub(
                r"\s*(?:https?://(?:dx\.)?doi\.org/|doi:\s*)?" + re.escape(doi) + r"[.]?",
                "",
                old_line,
            ).rstrip()
        if new_line != old_line:
            content = content.replace(old_line, new_line)
            r["text"] = new_line
    return content


async def verify_bibliography(content: str) -> dict:
    body, _ = _split_sections(content)
    entries = parse_bibliography(content)

    async with httpx.AsyncClient(timeout=HTTP_TIMEOUT_SECONDS, follow_redirects=True) as client:
        # Created per call: an asyncio.Semaphore is bound to the loop it first waits on.
        semaphore = asyncio.Semaphore(MAX_CONCURRENT_LOOKUPS)
        results = list(await asyncio.gather(*(verify_entry(client, e, semaphore) for e in entries)))
    # Cached dicts are shared — copy before annotating.
    results = [dict(r) for r in results]

    citations = extract_in_text_citations(body)

    orphans, seen = [], set()
    for aliases, year, raw in citations:
        if (aliases, year) in seen:
            continue
        seen.add((aliases, year))
        if not any(e.year == year and _names_match(aliases, e.aliases) for e in entries):
            orphans.append(raw)

    for entry, res in zip(entries, results):
        res["cited"] = any(
            year == entry.year and _names_match(aliases, entry.aliases)
            for aliases, year, _ in citations
        )

    corrected = _apply_doi_fixes(content, results)
    return {
        "entries": results,
        "orphan_citations": orphans,
        "summary": {
            "total": len(results),
            "verified": sum(1 for r in results if r["verified"]),
        },
        "content": corrected,
        "changed": corrected != content,
    }

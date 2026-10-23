"""
Greek sources — suggest verified Greek references for a generated batch.

The model rarely cites Greek works (it knows few of them with certainty), so
after a batch is generated we search Greek open repositories and propose
sources the USER may add. Nothing is replaced: an accepted suggestion only
ADDS an in-text citation next to the existing ones and an entry to the
Βιβλιογραφία, so no better source is ever displaced.

Sources (all real records — nothing is model-written):
  Κάλλιπος   open Greek university textbooks; DOIs 10.57713 at DataCite. The
             whole catalogue (~1,150 books) is cached and searched locally.
  ΕΚΤ        ePublishing journals / conference proceedings (CrossRef prefix
             10.12681) and the National Archive of PhD Theses (10.12681/eadd).
  OpenAlex   Greek-language articles in journals only — institutional
             repositories are excluded because they mix diploma/master theses
             (forbidden) with PhDs and label them all "dissertation".

Pipeline: search → drop entries already in the bibliography → one small-model
call (Haiku) keeps only candidates that DIRECTLY support a specific sentence
(or, in text without in-text citations, the module's topic) → APA entries.
"""
from __future__ import annotations

import asyncio
import json
import re
import time
from dataclasses import dataclass, field
from difflib import SequenceMatcher
from typing import Optional

import anthropic
import httpx

from ..config import settings
from ..rate_limiter import Priority, get_rate_limiter
from .citation_relevance import _get_client
from .citation_verifier import (
    _PARENTHETICAL,
    _headers,
    _headings,
    _is_thesis,
    _norm,
    _openalex_abstract,
    _split_sections,
    _strip_markup,
    _title_similarity,
    parse_bibliography,
)

KALLIPOS_PREFIX = "10.57713"
EKT_PREFIX = "10.12681"
EADD_PREFIX = "10.12681/eadd"
HTTP_TIMEOUT_SECONDS = 10.0
KALLIPOS_TTL_SECONDS = 24 * 3600
MAX_QUERIES = 5            # topic + up to 4 section headings
MAX_KALLIPOS = 6
MAX_CANDIDATES = 30
MAX_SUGGESTIONS = 6
BODY_MAX_CHARS = 40000
ABSTRACT_MAX_CHARS = 500
SENTENCE_MATCH_THRESHOLD = 0.75

# Display/ranking order: textbooks and peer-reviewed work before PhD theses.
KIND_ORDER = {"kallipos": 0, "journal": 1, "conference": 2, "doctoral": 3}

# Fixed sections of every batch — not useful as search queries.
_GENERIC_HEADINGS = {
    "εισαγωγη", "σκοπος", "προσδοκωμενα αποτελεσματα", "λεξεις κλειδια", "βιβλιογραφια",
    "γλωσσαρι", "ερωτησεις αυτοαξιολογησης", "απαντησεις", "συμπερασματα", "περιληψη",
    "δραστηριοτητες", "συνοψη",
}
_STOPWORDS = {
    "και", "της", "του", "των", "τους", "στην", "στον", "στο", "στη", "στις", "στα", "για",
    "απο", "προς", "κατα", "μετα", "οπως", "μεσω", "μεταξυ", "σχετικα", "ειναι", "αυτο",
    "αυτη", "οποια", "οποιο", "ενος", "μιας", "with", "from", "that", "this", "and", "the",
}
_HEADING_NUMBER = re.compile(r"^[\d.\s]+")
# Front matter and reviews — not sources for a claim.
_NOT_A_WORK = re.compile(
    r"^(?:πρόλογος|προλογος|εισαγωγικό σημείωμα|σημείωμα|editorial|εκδοτικό σημείωμα|"
    r"βιβλιοκρισία|βιβλιοπαρουσίαση|book review|review of|περιεχόμενα|errata)\b",
    re.IGNORECASE,
)
_GREEK = re.compile(r"[Ͱ-Ͽἀ-῿]")

_kallipos: Optional[list["Candidate"]] = None
_kallipos_loaded_at = 0.0
_kallipos_lock: Optional[asyncio.Lock] = None


@dataclass
class Candidate:
    kind: str                                  # kallipos | journal | conference | doctoral
    title: str
    authors: list[tuple[str, str]]             # (family, given)
    year: Optional[int]
    doi: Optional[str]
    venue: str = ""
    volume: str = ""
    issue: str = ""
    pages: str = ""
    institution: str = ""
    abstract: str = ""
    subjects: list[str] = field(default_factory=list)


# =============================================================================
# APA formatting
# =============================================================================

def _initials(given: str) -> str:
    parts = [p for p in re.split(r"\s+", given.strip()) if p.strip(".")]
    out = []
    for part in parts:
        if part.endswith(".") and len(part) <= 4:
            out.append(part)
        else:
            out.append("-".join(f"{sub[0]}." for sub in part.split("-") if sub))
    return " ".join(out)


def _author_name(family: str, given: str) -> str:
    return f"{family}, {_initials(given)}" if given else family


def _apa_authors(authors: list[tuple[str, str]]) -> str:
    names = [_author_name(f, g) for f, g in authors[:20] if f]
    if not names:
        return ""
    if len(names) == 1:
        return names[0]
    return ", ".join(names[:-1]) + ", & " + names[-1]


def citation_label(cand: Candidate) -> str:
    """In-text form: "Μέκος", "Α & Β", "Α et al." + year (APA 7)."""
    families = [f for f, _ in cand.authors if f]
    if len(families) >= 3:
        who = f"{families[0]} et al."
    elif len(families) == 2:
        who = f"{families[0]} & {families[1]}"
    else:
        who = families[0] if families else cand.venue
    return f"{who}, {cand.year}"


def _end(title: str) -> str:
    return title if title.endswith(("?", "!", ".")) else f"{title}."


def format_apa(cand: Candidate) -> str:
    authors = _apa_authors(cand.authors)
    head = f"{_end(authors)} ({cand.year})." if authors else f"({cand.year})."
    doi = f" https://doi.org/{cand.doi}" if cand.doi else ""
    if cand.kind == "kallipos":
        return f"{head} *{cand.title}*. Κάλλιπος, Ανοικτές Ακαδημαϊκές Εκδόσεις.{doi}"
    if cand.kind == "doctoral":
        where = f", {cand.institution}" if cand.institution else ""
        return (f"{head} *{cand.title}* [Διδακτορική διατριβή{where}]. "
                f"Εθνικό Αρχείο Διδακτορικών Διατριβών.{doi}")
    if cand.kind == "conference":
        pages = f" (σσ. {cand.pages})" if cand.pages else ""
        return f"{head} {_end(cand.title)} Στο *{cand.venue}*{pages}.{doi}"
    source = f"*{cand.venue}*"
    if cand.volume:
        source += f", *{cand.volume}*"
    if cand.issue:
        source += f"({cand.issue})"
    if cand.pages:
        source += f", {cand.pages}"
    return f"{head} {_end(cand.title)} {source}.{doi}"


# =============================================================================
# Search
# =============================================================================

# "Ελλάδα/ελληνικός/Greek" is in half the titles of a Greek repository — it
# says nothing about the subject, so it never counts as a match.
_COUNTRY_STEMS = {"ελλην", "ελλαδ", "greec", "greek", "helle"}


def _stems(text: str) -> set[str]:
    """Crude Greek/English stemming: accent-folded 5-letter prefixes."""
    return {
        w[:5] for w in _norm(text).split()
        if len(w) >= 4 and w not in _STOPWORDS and w[:5] not in _COUNTRY_STEMS
    }


def search_queries(topic: str, body: str) -> list[str]:
    queries = [topic] if topic.strip() else []
    for heading in _headings(body):
        clean = _HEADING_NUMBER.sub("", heading).strip(" :")
        if clean and _norm(clean) not in _GENERIC_HEADINGS and clean not in queries:
            queries.append(clean)
    return queries[:MAX_QUERIES]


def _first_segment(text: str) -> str:
    return text.split(",")[0].strip()


def _crossref_to_candidate(item: dict) -> Optional[Candidate]:
    doi = item.get("DOI", "")
    item_type = item.get("type")
    if item_type == "dissertation":
        if not doi.lower().startswith(EADD_PREFIX):
            return None
        kind = "doctoral"
    elif item_type == "proceedings-article":
        kind = "conference"
    elif item_type == "journal-article":
        kind = "journal"
    else:
        return None
    year = None
    for key in ("issued", "published-print", "published-online", "approved", "created"):
        parts = (item.get(key) or {}).get("date-parts") or [[None]]
        if parts and parts[0] and parts[0][0]:
            year = parts[0][0]
            break
    title = (item.get("title") or [""])[0]
    venue = (item.get("container-title") or [""])[0]
    if not title or not year or _is_thesis(title, venue) or _NOT_A_WORK.match(title.strip()):
        return None
    institution = _first_segment(((item.get("institution") or [{}])[0]).get("name", ""))
    return Candidate(
        kind=kind,
        title=_strip_markup(title),
        authors=[(a.get("family") or a.get("name") or "", a.get("given") or "")
                 for a in item.get("author", [])],
        year=year,
        doi=doi,
        venue=_strip_markup(venue),
        volume=item.get("volume") or "",
        issue=item.get("issue") or "",
        pages=(item.get("page") or "").replace("-", "–"),
        institution=institution,
        abstract=_strip_markup(item.get("abstract", ""))[:ABSTRACT_MAX_CHARS],
    )


async def _search_ekt(client: httpx.AsyncClient, query: str, item_type: str, rows: int) -> list[Candidate]:
    resp = await client.get(
        f"https://api.crossref.org/prefixes/{EKT_PREFIX}/works",
        params={
            "query.bibliographic": query[:300],
            "filter": f"type:{item_type}",
            "rows": rows,
            "select": "DOI,type,title,author,issued,approved,container-title,volume,issue,page,institution,abstract",
        },
        headers=_headers(),
    )
    if resp.status_code != 200:
        return []
    items = resp.json().get("message", {}).get("items", [])
    return [c for c in (_crossref_to_candidate(i) for i in items) if c]


def _split_display_name(name: str) -> tuple[str, str]:
    parts = name.split()
    return (parts[-1], " ".join(parts[:-1])) if len(parts) > 1 else (name, "")


async def _search_openalex(client: httpx.AsyncClient, query: str, rows: int = 5) -> list[Candidate]:
    resp = await client.get(
        "https://api.openalex.org/works",
        params={
            "search": query[:300],
            "filter": "language:el,type:article,primary_location.source.type:journal",
            "per-page": rows,
            "select": "title,publication_year,doi,authorships,primary_location,biblio,abstract_inverted_index",
            "mailto": settings.crossref_mailto,
        },
        headers=_headers(),
    )
    if resp.status_code != 200:
        return []
    return _openalex_candidates(resp.json().get("results", []))


def _openalex_candidates(results: list[dict]) -> list[Candidate]:
    out = []
    for w in results:
        venue = ((w.get("primary_location") or {}).get("source") or {}).get("display_name") or ""
        title = w.get("title") or ""
        if (not title or not venue or not w.get("publication_year") or _is_thesis(title, venue)
                or _NOT_A_WORK.match(title.strip())):
            continue
        biblio = w.get("biblio") or {}
        first, last = biblio.get("first_page"), biblio.get("last_page")
        pages = first if first and first == last else "–".join(p for p in (first, last) if p)
        out.append(Candidate(
            kind="journal",
            title=title,
            authors=[_split_display_name((a.get("author") or {}).get("display_name") or "")
                     for a in w.get("authorships", [])],
            year=w["publication_year"],
            doi=(w.get("doi") or "").replace("https://doi.org/", "") or None,
            venue=venue,
            volume=biblio.get("volume") or "",
            issue=biblio.get("issue") or "",
            pages=pages,
            abstract=_openalex_abstract(w.get("abstract_inverted_index"))[:ABSTRACT_MAX_CHARS],
        ))
    return out


async def _search_openalex_greek_institutions(client: httpx.AsyncClient, query: str, rows: int = 5) -> list[Candidate]:
    """English-language journal articles whose FIRST author is at a Greek
    institution (e.g. ΕΛΚΕΘΕ in Mediterranean Marine Science) — a Greek source
    even though it is not written in Greek."""
    resp = await client.get(
        "https://api.openalex.org/works",
        params={
            "search": query[:300],
            "filter": "authorships.institutions.country_code:GR,type:article,primary_location.source.type:journal",
            "per-page": rows * 2,
            "select": "title,publication_year,doi,authorships,primary_location,biblio,abstract_inverted_index",
            "mailto": settings.crossref_mailto,
        },
        headers=_headers(),
    )
    if resp.status_code != 200:
        return []
    results = resp.json().get("results", [])
    first_author_greek = [
        w for w in results
        if any(i.get("country_code") == "GR"
               for i in ((w.get("authorships") or [{}])[0].get("institutions") or []))
    ]
    return _openalex_candidates(first_author_greek)[:rows]


def _kallipos_candidate(attrs: dict) -> Optional[Candidate]:
    titles = [t.get("title", "") for t in attrs.get("titles", []) if t.get("title")]
    title = next((t for t in titles if _GREEK.search(t)), titles[0] if titles else "")
    if not title or not attrs.get("publicationYear"):
        return None
    abstracts = [d for d in attrs.get("descriptions", []) if d.get("descriptionType") == "Abstract"]
    abstract = next((d["description"] for d in abstracts if d.get("lang") == "el"),
                    abstracts[0]["description"] if abstracts else "")
    return Candidate(
        kind="kallipos",
        title=_strip_markup(title),
        authors=[(c.get("familyName") or c.get("name") or "", c.get("givenName") or "")
                 for c in attrs.get("creators", [])],
        year=int(attrs["publicationYear"]),
        doi=attrs.get("doi"),
        venue="Κάλλιπος",
        abstract=_strip_markup(abstract)[:ABSTRACT_MAX_CHARS],
        subjects=[s.get("subject", "") for s in attrs.get("subjects", [])] + titles,
    )


async def _kallipos_catalog(client: httpx.AsyncClient) -> list[Candidate]:
    """The whole Κάλλιπος catalogue from DataCite, cached for a day."""
    global _kallipos, _kallipos_loaded_at, _kallipos_lock
    if _kallipos is not None and time.time() - _kallipos_loaded_at < KALLIPOS_TTL_SECONDS:
        return _kallipos
    if _kallipos_lock is None:
        _kallipos_lock = asyncio.Lock()
    async with _kallipos_lock:
        if _kallipos is not None and time.time() - _kallipos_loaded_at < KALLIPOS_TTL_SECONDS:
            return _kallipos
        books: list[Candidate] = []
        url: Optional[str] = (
            "https://api.datacite.org/dois?prefix=" + KALLIPOS_PREFIX + "&page[size]=1000"
            "&fields[dois]=doi,titles,creators,publicationYear,descriptions,subjects"
        )
        while url:
            resp = await client.get(url, headers=_headers(), timeout=30.0)
            if resp.status_code != 200:
                break
            data = resp.json()
            books += [c for c in (_kallipos_candidate(d.get("attributes", {}))
                                  for d in data.get("data", [])) if c]
            url = (data.get("links") or {}).get("next")
        if books:
            _kallipos, _kallipos_loaded_at = books, time.time()
        return books


def rank_kallipos(books: list[Candidate], queries: list[str], limit: int = MAX_KALLIPOS) -> list[Candidate]:
    terms = _stems(" ".join(queries))
    scored = []
    for book in books:
        in_title = len(terms & _stems(book.title))
        in_subjects = len(terms & _stems(" ".join(book.subjects)))
        in_abstract = len(terms & _stems(book.abstract))
        score = 3 * in_title + 2 * in_subjects + in_abstract
        if (in_title or in_subjects) and score >= 4:
            scored.append((score, book))
    scored.sort(key=lambda s: -s[0])
    return [book for _, book in scored[:limit]]


async def _safe(coro) -> list[Candidate]:
    try:
        return await coro
    except (httpx.HTTPError, ValueError) as e:
        print(f"Greek sources lookup error: {e}")
        return []


async def find_candidates(
    client: httpx.AsyncClient, queries: list[str], english_queries: Optional[list[str]] = None
) -> list[Candidate]:
    english_queries = english_queries or []
    if not queries and not english_queries:
        return []
    tasks = [_safe(_kallipos_catalog(client))]
    for q in queries:
        tasks.append(_safe(_search_ekt(client, q, "journal-article", 4)))
        tasks.append(_safe(_search_openalex(client, q, 4)))
    for q in english_queries:
        # ΕΚΤ hosts Greek journals that publish in English (e.g. Mediterranean Marine Science).
        tasks.append(_safe(_search_ekt(client, q, "journal-article", 4)))
        tasks.append(_safe(_search_openalex_greek_institutions(client, q, 4)))
    first = (queries or english_queries)[0]
    tasks.append(_safe(_search_ekt(client, first, "proceedings-article", 3)))
    tasks.append(_safe(_search_ekt(client, first, "dissertation", 3)))
    catalog, *found = await asyncio.gather(*tasks)

    candidates = rank_kallipos(catalog, queries + english_queries)
    seen = {c.doi.lower() for c in candidates if c.doi}
    # Round-robin across searches so no single source crowds out the rest.
    for rank in range(max((len(f) for f in found), default=0)):
        for batch in found:
            if rank < len(batch):
                cand = batch[rank]
                key = (cand.doi or _norm(cand.title)).lower()
                if key not in seen:
                    seen.add(key)
                    candidates.append(cand)
    return candidates[:MAX_CANDIDATES]


def _already_cited(cand: Candidate, content: str) -> bool:
    if cand.doi and cand.doi.lower() in content.lower():
        return True
    return any(_title_similarity(e.title, cand.title) >= 0.85 for e in parse_bibliography(content))


# =============================================================================
# Sentence location (shared by suggest + apply)
# =============================================================================

def _sentence_spans(text: str, start: int = 0, end: Optional[int] = None) -> list[tuple[int, int]]:
    """Sentence spans of ``text[start:end]``, ignoring punctuation inside
    parentheses (so "(Smith et al., 2020; ...)" never splits a sentence)."""
    end = len(text) if end is None else end
    spans, depth, s = [], 0, start
    i = start
    while i < end:
        ch = text[i]
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth = max(0, depth - 1)
        elif ch == "\n" or (depth == 0 and ch in ".!?;" and (i + 1 >= end or text[i + 1].isspace())):
            stop = i if ch == "\n" else i + 1
            if text[s:stop].strip():
                spans.append((s, stop))
            s = i + 1
        i += 1
    if text[s:end].strip():
        spans.append((s, end))
    return spans


def _body_ranges(content: str) -> list[tuple[int, int]]:
    """Character ranges of body lines: not headings, not inside the bibliography."""
    body_text, _ = _split_sections(content)
    body_lines = set(body_text.split("\n"))
    ranges, pos = [], 0
    for line in content.split("\n"):
        if line in body_lines and line.strip() and not line.lstrip().startswith("#"):
            ranges.append((pos, pos + len(line)))
        pos += len(line) + 1
    return ranges


def locate_sentence(content: str, sentence: str) -> Optional[tuple[int, int]]:
    """Best-matching body sentence span for ``sentence`` (fuzzy)."""
    target = _norm(sentence)
    if not target:
        return None
    best, best_score = None, 0.0
    for a, b in _body_ranges(content):
        for s, e in _sentence_spans(content, a, b):
            score = SequenceMatcher(None, target, _norm(content[s:e])).ratio()
            if score > best_score:
                best, best_score = (s, e), score
    return best if best_score >= SENTENCE_MATCH_THRESHOLD else None


def _heading_before(content: str, pos: int) -> str:
    headings = [m for m in re.finditer(r"^#{1,6}\s*(.+)$", content[:pos], re.MULTILINE)]
    return headings[-1].group(1).strip() if headings else ""


# =============================================================================
# Small-model selection
# =============================================================================

SENTENCE_PROMPT = """Είσαι βοηθός βιβλιογραφίας για ελληνικό εκπαιδευτικό υλικό.
Παρακάτω δίνονται το ΚΕΙΜΕΝΟ ενός τμήματος και ΥΠΟΨΗΦΙΕΣ ελληνικές πηγές από ανοιχτές
ακαδημαϊκές βάσεις (Κάλλιπος, ΕΚΤ). Οι πηγές προτείνονται ως ΕΠΙΠΛΕΟΝ παραπομπές.

Επίλεξε ΜΟΝΟ πηγές που τεκμηριώνουν ΑΜΕΣΑ συγκεκριμένη πρόταση του κειμένου: το αντικείμενο
της πηγής πρέπει να καλύπτει αυτό ακριβώς που λέει η πρόταση. Ίδιο ευρύτερο πεδίο ΔΕΝ αρκεί.
Για κάθε επιλογή αντέγραψε ΑΥΤΟΥΣΙΑ (λέξη προς λέξη) την πρόταση του κειμένου που τεκμηριώνει.
Είναι απολύτως αποδεκτό να μην επιλέξεις καμία. Το πολύ {max} πηγές, μία πρόταση ανά πηγή.
Στην αιτιολόγηση μην αναφέρεις τον αριθμό της πηγής.

Απάντησε ΜΟΝΟ με JSON, χωρίς κείμενο πριν ή μετά:
{{"results": [{{"id": 1, "sentence": "αυτούσια πρόταση του κειμένου", "reason": "σύντομη αιτιολόγηση (έως 20 λέξεις)"}}]}}

ΚΕΙΜΕΝΟ:
{body}

ΥΠΟΨΗΦΙΕΣ ΠΗΓΕΣ:
{items}"""

TOPIC_PROMPT = """Είσαι βοηθός βιβλιογραφίας για ελληνικό εκπαιδευτικό υλικό.
Το υλικό ΔΕΝ έχει ενδοκειμενικές παραπομπές: η Βιβλιογραφία είναι η λίστα πηγών στις
οποίες βασίζεται. Παρακάτω δίνονται το θέμα, οι επικεφαλίδες και ΥΠΟΨΗΦΙΕΣ ελληνικές
πηγές από ανοιχτές ακαδημαϊκές βάσεις (Κάλλιπος, ΕΚΤ), ως ΕΠΙΠΛΕΟΝ πηγές.

Επίλεξε ΜΟΝΟ πηγές που πραγματεύονται ΑΜΕΣΑ το θέμα ή μία συγκεκριμένη υποενότητα.
Ίδιο ευρύτερο πεδίο ΔΕΝ αρκεί. Είναι απολύτως αποδεκτό να μην επιλέξεις καμία.
Το πολύ {max} πηγές. Στην αιτιολόγηση μην αναφέρεις τον αριθμό της πηγής.

Απάντησε ΜΟΝΟ με JSON, χωρίς κείμενο πριν ή μετά:
{{"results": [{{"id": 1, "section": "επικεφαλίδα που αφορά", "reason": "σύντομη αιτιολόγηση (έως 20 λέξεις)"}}]}}

ΘΕΜΑ: {topic}
ΕΠΙΚΕΦΑΛΙΔΕΣ:
{headings}

ΥΠΟΨΗΦΙΕΣ ΠΗΓΕΣ:
{items}"""

KIND_LABELS = {
    "kallipos": "Σύγγραμμα Κάλλιπος",
    "journal": "Άρθρο περιοδικού",
    "conference": "Πρακτικά συνεδρίου",
    "doctoral": "Διδακτορική διατριβή",
}


def _format_items(candidates: list[Candidate]) -> str:
    blocks = []
    for i, c in enumerate(candidates, 1):
        authors = ", ".join(f for f, _ in c.authors[:3] if f)
        blocks.append(
            f"[{i}] {KIND_LABELS[c.kind]} | {c.title} | {authors} ({c.year}) | {c.venue}\n"
            f"Περίληψη: {c.abstract or '(δεν διατίθεται)'}"
        )
    return "\n\n".join(blocks)


def _parse_selection(text: str, count: int) -> list[dict]:
    start, end = text.find("{"), text.rfind("}")
    if start == -1 or end == -1:
        return []
    rows = json.loads(text[start:end + 1]).get("results", [])
    return [r for r in rows if isinstance(r.get("id"), int) and 1 <= r["id"] <= count]


async def _ask(prompt: str, user_id: str) -> str:
    """One small-model call; returns its text ("" on failure)."""
    rate_limiter = get_rate_limiter()
    try:
        async with rate_limiter.throttle(user_id, Priority.LIGHT, estimated_output=1500):
            response = await _get_client().messages.create(
                model=settings.relevance_model_id,
                max_tokens=4000,
                messages=[{"role": "user", "content": prompt}],
            )
        await rate_limiter.tracker.record_usage(response.usage.input_tokens, response.usage.output_tokens)
        return "".join(block.text for block in response.content if block.type == "text")
    except anthropic.APIError as e:
        print(f"Greek sources model call failed: {e}")
        return ""


async def _select(prompt: str, count: int, user_id: str) -> list[dict]:
    try:
        return _parse_selection(await _ask(prompt, user_id), count)
    except ValueError as e:
        print(f"Greek sources selection unparseable: {e}")
        return []


QUERY_PROMPT = """Φτιάξε ερωτήματα αναζήτησης σε ακαδημαϊκές βάσεις, για να βρεθεί βιβλιογραφία
σχετική με το παρακάτω εκπαιδευτικό υλικό.
Δώσε {n} ερωτήματα στα ελληνικά και {n} στα αγγλικά. Κάθε ερώτημα: 2-4 λέξεις, ουσιαστικοί
επιστημονικοί όροι του αντικειμένου (όχι γενικές λέξεις όπως «εισαγωγή», «έννοια», «ρόλος»).
Μη βάζεις «Ελλάδα», «ελληνικός», «Greece», «Greek»: οι βάσεις είναι ήδη ελληνικές.

Απάντησε ΜΟΝΟ με JSON, χωρίς κείμενο πριν ή μετά:
{{"el": ["..."], "en": ["..."]}}

ΘΕΜΑ: {topic}
ΠΕΡΙΕΧΟΜΕΝΟ:
{context}"""

QUERIES_PER_LANGUAGE = 3


async def generate_queries(topic: str, context: str, user_id: str) -> tuple[list[str], list[str]]:
    """Targeted Greek + English search queries (small model); falls back to
    the topic and headings when the call fails."""
    text = await _ask(
        QUERY_PROMPT.format(n=QUERIES_PER_LANGUAGE, topic=topic or "(βλ. περιεχόμενο)", context=context[:3000]),
        user_id,
    ) if settings.anthropic_api_key else ""
    try:
        start, end = text.find("{"), text.rfind("}")
        data = json.loads(text[start:end + 1]) if start != -1 and end != -1 else {}
    except ValueError:
        data = {}
    clean = lambda qs: [q.strip() for q in qs if isinstance(q, str) and q.strip()][:QUERIES_PER_LANGUAGE]
    greek, english = clean(data.get("el", [])), clean(data.get("en", []))
    if not greek and not english:
        return search_queries(topic, context), []
    return greek, english


# =============================================================================
# Public API
# =============================================================================

def _suggestion(cand: Candidate, reason: str, sentence: Optional[str], section: str) -> dict:
    return {
        "key": (cand.doi or _norm(cand.title))[:200],
        "kind": cand.kind,
        "title": cand.title,
        "year": cand.year,
        "venue": cand.venue,
        "doi": cand.doi,
        "apa": format_apa(cand),
        "citation": citation_label(cand),
        "sentence": sentence,
        "section": section,
        "reason": reason,
    }


async def suggest_greek_sources(content: str, topic: str = "", user_id: str = "anonymous") -> dict:
    body, _ = _split_sections(content)
    headings = "\n".join(f"- {h}" for h in _headings(body)[:40])
    queries, english = await generate_queries(topic, headings, user_id)
    async with httpx.AsyncClient(timeout=HTTP_TIMEOUT_SECONDS, follow_redirects=True) as client:
        candidates = [c for c in await find_candidates(client, queries, english)
                      if not _already_cited(c, content)]

    result = {"suggestions": [], "searched": len(candidates), "queries": queries + english}
    if not candidates or not settings.anthropic_api_key:
        return result

    with_citations = bool(_PARENTHETICAL.search(body))
    if with_citations:
        prompt = SENTENCE_PROMPT.format(max=MAX_SUGGESTIONS, body=body[:BODY_MAX_CHARS],
                                        items=_format_items(candidates))
    else:
        headings = _headings(body)
        prompt = TOPIC_PROMPT.format(max=MAX_SUGGESTIONS, topic=topic or "(βλ. επικεφαλίδες)",
                                     headings="\n".join(f"- {h}" for h in headings[:40]) or "(δεν υπάρχουν)",
                                     items=_format_items(candidates))

    suggestions, used = [], set()
    for row in await _select(prompt, len(candidates), user_id):
        cand = candidates[row["id"] - 1]
        if row["id"] in used:
            continue
        reason = re.sub(r"^(?:Η\s+πηγή\s+)?\[\d+\]\s*", "", str(row.get("reason", "")).strip())
        if with_citations:
            # The quoted sentence must really be in the text, or the citation can't be placed.
            span = locate_sentence(content, str(row.get("sentence", "")))
            if not span:
                continue
            sentence = content[span[0]:span[1]].strip()
            section = _heading_before(content, span[0])
        else:
            sentence, section = None, str(row.get("section", "")).strip()
        used.add(row["id"])
        suggestions.append(_suggestion(cand, reason, sentence, section))

    suggestions.sort(key=lambda s: KIND_ORDER[s["kind"]])
    result["suggestions"] = suggestions[:MAX_SUGGESTIONS]
    return result


def _add_citation(content: str, span: tuple[int, int], label: str) -> str:
    """Append ``label`` to the sentence's closing citation, or add one before
    its final punctuation: "... (Smith, 2020)." → "... (Smith, 2020; Μέκος, 2016)."."""
    s, e = span
    sentence = content[s:e].rstrip()
    e = s + len(sentence)
    punct = sentence[-1] if sentence and sentence[-1] in ".!?;" else ""
    stem_end = e - len(punct)
    stem = content[s:stem_end].rstrip()
    stem_end = s + len(stem)
    if stem.endswith(")"):
        last = list(_PARENTHETICAL.finditer(stem))
        if last and last[-1].end() == len(stem):
            return content[:stem_end - 1] + f"; {label})" + content[stem_end:]
    return content[:stem_end] + f" ({label})" + content[stem_end:]


def _add_bibliography_entry(content: str, apa: str) -> str:
    lines = content.split("\n")
    _, biblio_lines = _split_sections(content)
    entry_lines = [l for l in biblio_lines if l.strip()]
    if not entry_lines:
        return content.rstrip() + f"\n\n## Βιβλιογραφία\n\n{apa}\n"
    last = entry_lines[-1]
    marker = re.match(r"^\s*(?:[-*•]\s+)?", last).group(0)
    idx = len(lines) - 1 - lines[::-1].index(last)
    lines.insert(idx + 1, f"{marker}{apa}")
    return "\n".join(lines)


def apply_greek_sources(content: str, suggestions: list[dict]) -> dict:
    """Add accepted suggestions: in-text citation (when the text uses them)
    + bibliography entry. Nothing existing is removed or replaced."""
    applied, unplaced = [], []
    for sug in suggestions:
        apa, label, sentence = sug.get("apa", ""), sug.get("citation", ""), sug.get("sentence")
        if not apa:
            continue
        if sentence and label:
            span = locate_sentence(content, sentence)
            if span:
                content = _add_citation(content, span, label)
            else:
                unplaced.append(sug.get("key"))
                continue
        doi = sug.get("doi")
        if not (doi and doi.lower() in "\n".join(_split_sections(content)[1]).lower()):
            content = _add_bibliography_entry(content, apa)
        applied.append(sug.get("key"))
    return {"content": content, "applied": applied, "unplaced": unplaced}


# =============================================================================
# Integration mode ("Β"): sources given to the writer BEFORE generation
# =============================================================================

MAX_INTEGRATION_SOURCES = 8

INTEGRATION_SELECT_PROMPT = """Είσαι βοηθός βιβλιογραφίας για ελληνικό εκπαιδευτικό υλικό που ΠΡΟΚΕΙΤΑΙ να γραφτεί.
Παρακάτω δίνονται το θέμα και το περιεχόμενο που θα καλυφθεί, και ΥΠΟΨΗΦΙΕΣ πηγές από
ελληνικές ανοιχτές βάσεις (Κάλλιπος, ΕΚΤ, ελληνικοί ερευνητικοί φορείς).

Επίλεξε ΜΟΝΟ πηγές που πραγματεύονται ΑΜΕΣΑ το θέμα ή κάποιο συγκεκριμένο αντικείμενο του
περιεχομένου, ώστε ο συγγραφέας να μπορεί να τις παραθέσει για συγκεκριμένους ισχυρισμούς.
Ίδιο ευρύτερο πεδίο ΔΕΝ αρκεί. Είναι απολύτως αποδεκτό να μην επιλέξεις καμία. Το πολύ {max}.
Στην αιτιολόγηση μην αναφέρεις τον αριθμό της πηγής.

Απάντησε ΜΟΝΟ με JSON, χωρίς κείμενο πριν ή μετά:
{{"results": [{{"id": 1, "section": "αντικείμενο που αφορά", "reason": "σύντομη αιτιολόγηση (έως 20 λέξεις)"}}]}}

ΘΕΜΑ: {topic}
ΠΕΡΙΕΧΟΜΕΝΟ:
{context}

ΥΠΟΨΗΦΙΕΣ ΠΗΓΕΣ:
{items}"""


async def prepare_integration_sources(
    topic: str, context: str, previous_content: str = "", user_id: str = "anonymous"
) -> list[Candidate]:
    """Find verified Greek sources for a batch that is about to be written.
    ``context`` = learning outcomes / keywords / module description; sources
    already in ``previous_content`` (earlier batches) are skipped."""
    queries, english = await generate_queries(topic, context, user_id)
    async with httpx.AsyncClient(timeout=HTTP_TIMEOUT_SECONDS, follow_redirects=True) as client:
        candidates = [c for c in await find_candidates(client, queries, english)
                      if not _already_cited(c, previous_content)]
    if not candidates or not settings.anthropic_api_key:
        return []
    prompt = INTEGRATION_SELECT_PROMPT.format(
        max=MAX_INTEGRATION_SOURCES, topic=topic or "(βλ. περιεχόμενο)",
        context=context[:3000] or "(δεν δόθηκε)", items=_format_items(candidates),
    )
    chosen, used = [], set()
    for row in await _select(prompt, len(candidates), user_id):
        if row["id"] not in used:
            used.add(row["id"])
            chosen.append(candidates[row["id"] - 1])
    chosen.sort(key=lambda c: KIND_ORDER[c.kind])
    return chosen[:MAX_INTEGRATION_SOURCES]


def build_integration_block(sources: list[Candidate], in_text_citations: bool = True) -> str:
    """Prompt section handing the verified sources to the writer — optional,
    additive, and only for what each source's title/abstract supports."""
    if not sources:
        return ""
    items = []
    for i, c in enumerate(sources, 1):
        cite = f"\n    Παραπομπή: ({citation_label(c)})" if in_text_citations else ""
        items.append(
            f"[{i}] {KIND_LABELS[c.kind]}{cite}\n    APA: {format_apa(c)}\n"
            f"    Περίληψη: {c.abstract or '(δεν διατίθεται, κρίνε από τον τίτλο)'}"
        )
    if in_text_citations:
        rules = """- Χρησιμοποίησέ τες ΕΠΙΠΛΕΟΝ των άλλων πηγών σου, ΜΟΝΟ όπου τεκμηριώνουν ΑΜΕΣΑ αυτό που γράφεις.
- Παράθεσέ τες ΜΟΝΟ για ό,τι προκύπτει από τον τίτλο ή την περίληψή τους. ΜΗΝ τους αποδώσεις
  αριθμούς, ποσοστά, ευρήματα ή ισχυρισμούς που δεν αναφέρονται εκεί.
- ΜΗΝ τις χρησιμοποιήσεις στη θέση ισχυρότερης πηγής· μπορούν να συνυπάρχουν: (Smith, 2020; Μέκος, 2016).
- Χρησιμοποίησε την παραπομπή ΑΚΡΙΒΩΣ όπως δίνεται."""
    else:
        rules = """- Αξιοποίησέ τες ΕΠΙΠΛΕΟΝ των άλλων πηγών σου, ΜΟΝΟ όπου αφορούν ΑΜΕΣΑ αυτό που γράφεις,
  και μόνο για ό,τι προκύπτει από τον τίτλο ή την περίληψή τους."""
    return f"""

## ΕΛΛΗΝΙΚΕΣ ΠΗΓΕΣ (ΠΡΑΓΜΑΤΙΚΕΣ, ΕΠΑΛΗΘΕΥΜΕΝΕΣ — ΠΡΟΑΙΡΕΤΙΚΕΣ)
Οι παρακάτω πηγές προέρχονται από ελληνικές ανοιχτές βάσεις (Κάλλιπος, ΕΚΤ, ελληνικοί φορείς).
{rules}
- ΔΕΝ είναι υποχρεωτικό να τις χρησιμοποιήσεις· αγνόησε όσες δεν ταιριάζουν.
- Όσες χρησιμοποιήσεις, γράψε τις στη Βιβλιογραφία ΑΚΡΙΒΩΣ όπως δίνονται (APA).

{chr(10).join(items)}
"""

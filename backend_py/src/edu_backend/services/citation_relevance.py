"""
Citation relevance — does each cited source actually support the sentences
that cite it?

The citation verifier proves a source EXISTS. This module catches the other
failure: a real source cited for a claim it has nothing to do with (e.g. a
famous paper on technology acceptance cited for a building-energy statistic).

One call to a small, cheap model per batch judges every (source, sentences)
pair from the source's title and, when the index has one, its abstract:

  supports    the source's subject directly covers the claim
  general     same broad field; the specific claim is not evident from the
              source (typical for seminal works cited as background)
  unrelated   the source is clearly about something else  → flagged to the user
  uncertain   not enough information to judge (no abstract, unknown work)

Only "unrelated" is surfaced as a problem. The judge sees titles/abstracts,
not full texts, so it catches gross mismatches, not wrong page-level details.
Any failure leaves ``relevance`` unset — it never blocks a batch.

When the text has NO in-text citations (the user turned them off) there are
no (source, sentence) pairs, so ``assess_topic_relevance`` runs instead: it
judges each entry against the module's topic and section headings
(related / unrelated / uncertain) and catches sources that are off-topic for
the whole module. Results carry ``relevance_scope`` = "sentence" or "topic".
"""
from __future__ import annotations

import json
from typing import Optional

import anthropic

from ..config import settings
from ..rate_limiter import Priority, get_rate_limiter

VERDICTS = {"supports", "general", "unrelated", "uncertain"}
TOPIC_VERDICTS = {"related", "unrelated", "uncertain"}
MAX_HEADINGS = 40
MAX_ITEMS = 30
_client: Optional[anthropic.AsyncAnthropic] = None

PROMPT = """Είσαι αξιολογητής βιβλιογραφικών παραπομπών σε ελληνικό εκπαιδευτικό υλικό.
Για κάθε πηγή παρακάτω δίνονται: ο τίτλος της, η περίληψή της (αν υπάρχει) και οι
προτάσεις του κειμένου που την επικαλούνται. Κρίνε αν η πηγή ΤΕΚΜΗΡΙΩΝΕΙ αυτές τις προτάσεις.

Κατηγορίες (ακριβώς μία ανά πηγή):
- "supports": το αντικείμενο της πηγής καλύπτει άμεσα τον ισχυρισμό των προτάσεων.
- "general": ίδιο ευρύτερο πεδίο, αλλά ο συγκεκριμένος ισχυρισμός δεν προκύπτει εμφανώς
  από την πηγή (π.χ. θεμελιώδες έργο που παρατίθεται ως γενικό πλαίσιο).
- "unrelated": το θέμα της πηγής είναι ΣΑΦΩΣ διαφορετικό από τον ισχυρισμό.
- "uncertain": δεν υπάρχει αρκετή πληροφορία για κρίση (π.χ. χωρίς περίληψη και δεν γνωρίζεις το έργο).

Να είσαι συντηρητικός: "unrelated" ΜΟΝΟ όταν η αναντιστοιχία είναι προφανής.
Μπορείς να χρησιμοποιήσεις και τη δική σου γνώση για γνωστά έργα.

Απάντησε ΜΟΝΟ με JSON, χωρίς κείμενο πριν ή μετά:
{{"results": [{{"id": 1, "verdict": "supports", "reason": "σύντομη αιτιολόγηση στα ελληνικά (έως 20 λέξεις)"}}]}}

ΠΗΓΕΣ:
{items}"""


TOPIC_PROMPT = """Είσαι αξιολογητής βιβλιογραφίας σε ελληνικό εκπαιδευτικό υλικό.
Το υλικό ΔΕΝ έχει ενδοκειμενικές παραπομπές: η Βιβλιογραφία είναι η λίστα πηγών
στις οποίες βασίζεται το περιεχόμενο. Για κάθε πηγή παρακάτω δίνονται ο τίτλος της
και η περίληψή της (αν υπάρχει). Κρίνε αν η πηγή ΣΧΕΤΙΖΕΤΑΙ με το θέμα της ενότητας.

ΘΕΜΑ ΕΝΟΤΗΤΑΣ: {topic}
ΕΠΙΚΕΦΑΛΙΔΕΣ ΤΟΥ ΥΛΙΚΟΥ:
{headings}

Κατηγορίες (ακριβώς μία ανά πηγή):
- "related": η πηγή πραγματεύεται το θέμα ή κάποια από τις υποενότητες.
- "unrelated": το θέμα της πηγής είναι ΣΑΦΩΣ εκτός της ενότητας.
- "uncertain": δεν υπάρχει αρκετή πληροφορία για κρίση.

Να είσαι συντηρητικός: "unrelated" ΜΟΝΟ όταν η πηγή είναι προφανώς εκτός θέματος.
Μπορείς να χρησιμοποιήσεις και τη δική σου γνώση για γνωστά έργα.

Απάντησε ΜΟΝΟ με JSON, χωρίς κείμενο πριν ή μετά:
{{"results": [{{"id": 1, "verdict": "related", "reason": "σύντομη αιτιολόγηση στα ελληνικά (έως 20 λέξεις)"}}]}}

ΠΗΓΕΣ:
{items}"""


def _get_client() -> anthropic.AsyncAnthropic:
    global _client
    if _client is None:
        _client = anthropic.AsyncAnthropic(api_key=settings.anthropic_api_key)
    return _client


def _format_items(items: list[tuple[int, dict]], with_sentences: bool = True) -> str:
    blocks = []
    for item_id, res in items:
        title = res.get("matched_title") or res["text"]
        abstract = res.get("abstract") or "(δεν διατίθεται)"
        block = f"[{item_id}] Εγγραφή: {res['text']}\nΤίτλος: {title}\nΠερίληψη: {abstract}"
        if with_sentences:
            sentences = "\n".join(f'  - "{s}"' for s in res["contexts"])
            block += f"\nΠροτάσεις που την επικαλούνται:\n{sentences}"
        blocks.append(block)
    return "\n\n".join(blocks)


def _parse(text: str, allowed: set[str] = VERDICTS) -> dict[int, tuple[str, str]]:
    start, end = text.find("{"), text.rfind("}")
    if start == -1 or end == -1:
        return {}
    data = json.loads(text[start:end + 1])
    parsed = {}
    for row in data.get("results", []):
        verdict = str(row.get("verdict", "")).strip().lower()
        if verdict in allowed and isinstance(row.get("id"), int):
            parsed[row["id"]] = (verdict, str(row.get("reason", "")).strip())
    return parsed


async def _judge(prompt: str, allowed: set[str], user_id: str) -> Optional[dict[int, tuple[str, str]]]:
    """One small-model call; returns {item id: (verdict, reason)} or None on failure."""
    rate_limiter = get_rate_limiter()
    try:
        async with rate_limiter.throttle(user_id, Priority.LIGHT, estimated_output=1500):
            response = await _get_client().messages.create(
                model=settings.relevance_model_id,
                max_tokens=4000,
                messages=[{"role": "user", "content": prompt}],
            )
        await rate_limiter.tracker.record_usage(
            response.usage.input_tokens, response.usage.output_tokens
        )
        text = "".join(block.text for block in response.content if block.type == "text")
        return _parse(text, allowed)
    except (anthropic.APIError, ValueError) as e:
        print(f"Citation relevance check failed: {e}")
        return None


def _annotate(items: list[tuple[int, dict]], verdicts: Optional[dict], scope: str) -> None:
    for item_id, res in items:
        if verdicts and item_id in verdicts:
            res["relevance"], res["relevance_reason"] = verdicts[item_id]
            res["relevance_scope"] = scope


async def assess_relevance(results: list[dict], user_id: str = "anonymous") -> None:
    """Annotate each cited entry in ``results`` with ``relevance`` and
    ``relevance_reason`` (in place). Entries without citing sentences, and
    placeholders, are skipped."""
    items = [
        (i, res) for i, res in enumerate(results, 1)
        if res.get("contexts") and res.get("status") != "placeholder"
    ][:MAX_ITEMS]
    if not items or not settings.anthropic_api_key:
        return
    verdicts = await _judge(PROMPT.format(items=_format_items(items)), VERDICTS, user_id)
    _annotate(items, verdicts, "sentence")


async def assess_topic_relevance(
    results: list[dict], topic: str, headings: list[str], user_id: str = "anonymous"
) -> None:
    """Text without in-text citations: judge each entry against the module's
    topic and headings instead of against citing sentences."""
    items = [
        (i, res) for i, res in enumerate(results, 1) if res.get("status") != "placeholder"
    ][:MAX_ITEMS]
    if not items or not settings.anthropic_api_key or not (topic or headings):
        return
    prompt = TOPIC_PROMPT.format(
        topic=topic or "(βλ. επικεφαλίδες)",
        headings="\n".join(f"- {h}" for h in headings[:MAX_HEADINGS]) or "(δεν υπάρχουν)",
        items=_format_items(items, with_sentences=False),
    )
    verdicts = await _judge(prompt, TOPIC_VERDICTS, user_id)
    _annotate(items, verdicts, "topic")

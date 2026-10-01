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
"""
from __future__ import annotations

import json
from typing import Optional

import anthropic

from ..config import settings
from ..rate_limiter import Priority, get_rate_limiter

VERDICTS = {"supports", "general", "unrelated", "uncertain"}
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


def _get_client() -> anthropic.AsyncAnthropic:
    global _client
    if _client is None:
        _client = anthropic.AsyncAnthropic(api_key=settings.anthropic_api_key)
    return _client


def _format_items(items: list[tuple[int, dict]]) -> str:
    blocks = []
    for item_id, res in items:
        title = res.get("matched_title") or res["text"]
        abstract = res.get("abstract") or "(δεν διατίθεται)"
        sentences = "\n".join(f'  - "{s}"' for s in res["contexts"])
        blocks.append(
            f"[{item_id}] Εγγραφή: {res['text']}\n"
            f"Τίτλος: {title}\n"
            f"Περίληψη: {abstract}\n"
            f"Προτάσεις που την επικαλούνται:\n{sentences}"
        )
    return "\n\n".join(blocks)


def _parse(text: str) -> dict[int, tuple[str, str]]:
    start, end = text.find("{"), text.rfind("}")
    if start == -1 or end == -1:
        return {}
    data = json.loads(text[start:end + 1])
    parsed = {}
    for row in data.get("results", []):
        verdict = str(row.get("verdict", "")).strip().lower()
        if verdict in VERDICTS and isinstance(row.get("id"), int):
            parsed[row["id"]] = (verdict, str(row.get("reason", "")).strip())
    return parsed


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

    rate_limiter = get_rate_limiter()
    try:
        async with rate_limiter.throttle(user_id, Priority.LIGHT, estimated_output=1500):
            response = await _get_client().messages.create(
                model=settings.relevance_model_id,
                max_tokens=4000,
                messages=[{"role": "user", "content": PROMPT.format(items=_format_items(items))}],
            )
        await rate_limiter.tracker.record_usage(
            response.usage.input_tokens, response.usage.output_tokens
        )
        text = "".join(block.text for block in response.content if block.type == "text")
        verdicts = _parse(text)
    except (anthropic.APIError, ValueError) as e:
        print(f"Citation relevance check failed: {e}")
        return

    for item_id, res in items:
        if item_id in verdicts:
            res["relevance"], res["relevance_reason"] = verdicts[item_id]

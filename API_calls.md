# API Calls — Anthropic & OpenAI Integration Mechanics

This document is a technical reference for **how the backend calls the Anthropic and
OpenAI APIs**, with a focus on the mechanisms the user asked about: **parallelization /
concurrency (workers)**, **rate limiting**, and **retry / exponential backoff**.

All line references point at `backend_py/src/edu_backend/…` unless noted otherwise. It
reflects the code as of this writing; no source was modified to produce it.

> **Note on credentials:** API keys are loaded from the project-root `.env` via
> `pydantic-settings` (`config.py:6`, `config.py:9-10`). This document never reproduces the
> key values. (The `.env` file currently contains live, unredacted keys committed to the
> working tree — consider rotating them and removing them from version control; that is a
> separate concern from this documentation.)

---

## 1. Overview / TL;DR

The backend supports **two LLM providers behind one shared abstraction**:

- **Anthropic / Claude** — `ClaudeService`, using the `anthropic` SDK's
  `AsyncAnthropic` client. Default model `claude-opus-5`.
- **OpenAI / GPT** — `OpenAIService`, using the `openai` SDK's `AsyncOpenAI` client.
  Default model `gpt-5.6-sol`.

Both subclass the provider-agnostic base class **`LLMService`** (`llm_service.py`), so all
the cross-cutting mechanics — **retry, exponential backoff, auto-continuation, and token
accounting — are identical for both providers**. The provider is chosen per request via a
`model_provider: "claude" | "openai"` flag (default `"claude"`).

Responsibilities are cleanly separated:

| Concern | File | Role |
|---|---|---|
| Raw Anthropic SDK calls + prompt caching | `services/claude_service.py` | Thin adapter: `messages.stream` / `messages.create` |
| Raw OpenAI SDK calls | `services/openai_service.py` | Thin adapter: `chat.completions.create` |
| Retry, backoff, auto-continuation, token accounting, orchestration | `services/llm_service.py` | Provider-agnostic base class |
| Concurrency + token-budget admission control | `rate_limiter.py` | Priority semaphore + sliding-window OTPM budget |
| Configuration (model IDs, tier, max_tokens) | `config.py` | Pydantic settings |

**Call stack for a content-generation request** (the main hot path):

```
router /api/generate-stream (generate.py)
  → llm.generate_content_stream(...)            (llm_service.py:427)
      → async with rate_limiter.throttle(user_id, HEAVY, estimated_output=30000)
          → _stream_with_retry(...)             (llm_service.py:163)  ← app-level retry/backoff
              → ClaudeService.stream_text(...)  (claude_service.py:33)
                  → AsyncAnthropic.messages.stream(system=_cacheable_system(...),
                                                   max_tokens=settings.max_tokens)
      → (optional) up to 2 auto-continuation passes, each wrapped in _stream_with_retry
      → rate_limiter.tracker.record_usage(total_input_tokens, total_output_tokens)
```

With `model_provider="openai"` the only difference is the leaf call
(`OpenAIService.stream_text` → `AsyncOpenAI.chat.completions.create`); everything else in
the stack is shared.

---

## 2. Providers & Clients

### 2.1 Claude (`claude_service.py`)

- **Client:** `anthropic.AsyncAnthropic(api_key=settings.anthropic_api_key)` — the **async**
  variant (`claude_service.py:30`). Constructed with `api_key` only. **No custom `timeout`,
  `max_retries`, `base_url`, or `default_headers`** are passed, so the SDK's own defaults
  apply beneath the application's retry layer (see §2.3).
- **Model:** `self.model = settings.model_id` → `"claude-opus-5"` (`config.py:20`).
- **Streaming** (`claude_service.py:33-51`):
  ```python
  async with self.client.messages.stream(
      model=self.model,
      max_tokens=max_tokens,
      system=_cacheable_system(system),
      messages=messages,
  ) as stream:
      async for text in stream.text_stream:
          yield text
      final_message = await stream.get_final_message()
      usage_out["input_tokens"]  = final_message.usage.input_tokens
      usage_out["output_tokens"] = final_message.usage.output_tokens
      usage_out["truncated"]     = final_message.stop_reason == "max_tokens"
  ```
  Truncation is detected via **`stop_reason == "max_tokens"`** and written into the
  caller-supplied `usage_out` dict (in-place).
- **Non-streaming** (`claude_service.py:53-71`): `client.messages.create(**kwargs)`; the
  `system` block is included only when truthy; text extracted from `response.content[0].text`.
- **Prompt caching** (`claude_service.py:15-22`): every system prompt is wrapped by
  `_cacheable_system()` into the list-of-blocks form with
  `cache_control: {"type": "ephemeral"}` (~5-minute TTL). This is applied to the **system
  prefix only**; message-level content (e.g. the changing draft on revision passes) does not
  accrue cache hits.

### 2.2 OpenAI (`openai_service.py`)

- **Client:** `AsyncOpenAI(api_key=settings.openai_api_key)` (`openai_service.py:24`).
- **Model:** `self.model = settings.openai_model_id` → `"gpt-5.6-sol"` (`config.py:21`).
- **Streaming** (`openai_service.py:35-77`):
  ```python
  stream = await self.client.chat.completions.create(
      model=self.model,
      messages=full_messages,
      max_completion_tokens=max_tokens,
      stream=True,
      stream_options={"include_usage": True},
  )
  ```
  Differences from the Anthropic adapter (documented in the module docstring,
  `openai_service.py:6-10`):
  - The system prompt is prepended as a `role: "system"` message (no separate `system`
    parameter) — `_build_messages()` at `openai_service.py:27-33`.
  - Uses **`max_completion_tokens`**, not `max_tokens`.
  - Truncation detected via **`finish_reason == "length"`** (`openai_service.py:66`).
  - Usage fields `prompt_tokens` / `completion_tokens` are mapped to
    `input_tokens` / `output_tokens` for the shared rate limiter.
  - No explicit `cache_control` — relies on OpenAI's automatic prefix caching.
- **Non-streaming** (`openai_service.py:79-98`): `chat.completions.create(...)` without
  `stream`.

### 2.3 Provider selection & SDK defaults

- **Factory:** `get_llm_service(provider)` (`llm_service.py`, factory at the bottom of the
  file) caches singletons; `provider == "openai"` lazily imports and instantiates
  `OpenAIService`, otherwise `ClaudeService`. Both are `LLMService` subclasses.
- **Request flag:** `model_provider: Literal["claude", "openai"] = "claude"` appears in the
  request models of `routers/generate.py` (multiple endpoints) and `routers/claude.py`. The
  **frontend never calls the LLM providers directly** — `web/src/services/api.ts` only sends
  the `modelProvider` string in its POST bodies; the UI exposes a "GPT-5.6-sol" vs "Claude"
  toggle (`web/src/components/LandingPage.tsx`).
- **SDK-level retry/timeout defaults:** Because neither client is given a custom `timeout`
  or `max_retries`, the SDK defaults are in effect underneath the app's own retry layer. For
  the `anthropic` SDK these defaults are `max_retries = 2` (retrying 408/409/429/5xx +
  connection errors with its own exponential backoff) and a 10-minute request timeout. The
  `openai` SDK uses comparable defaults. This means transient failures can be retried **at
  two layers**: the app-level `_stream_with_retry` (§3) sits on top of whatever the SDK
  already retried internally.

---

## 3. Retry & Exponential Backoff (shared — `llm_service.py`)

All retry/backoff logic lives in the base class and therefore applies identically to both
providers.

### 3.1 Configuration (`llm_service.py:89-93`)

```python
self.stream_max_retries         = 5
self.stream_backoff_base_seconds = 1.0
self.stream_backoff_max_seconds  = 16.0
```

### 3.2 Which errors are retried — `_is_retryable_stream_error` (`llm_service.py:125-149`)

An error is considered transient/retryable if **any** of the following hold:

1. `exc.status_code` is in `{429, 500, 502, 503, 504, 529}` — i.e. rate-limit (429),
   server errors (5xx), and Anthropic's **overloaded (529)**.
2. The lowercased exception **message** contains any of:
   `overload`, `overloaded`, `rate limit`, `timeout`, `temporar`, `connection`, `network`,
   `unavailable`, `529`, `429`.
3. The exception **class name** contains `ratelimit`, `timeout`, or `connection`.

Everything else is treated as non-retryable and re-raised.

### 3.3 Backoff with jitter — `_retry_delay_seconds` (`llm_service.py:151-158`)

```python
base   = min(self.stream_backoff_base_seconds * (2 ** (attempt - 1)),
             self.stream_backoff_max_seconds)
jitter = random.uniform(0.0, 0.5)
return base + jitter
```

So the delay sequence is **1s → 2s → 4s → 8s → 16s** (capped at 16s), each plus 0–0.5s of
uniform jitter. Sleeping is `await asyncio.sleep(delay)`.

### 3.4 The retry driver — `_stream_with_retry` (`llm_service.py:163-204`)

This is the single primitive that wraps **every streaming call**. Key properties:

- Loops up to `stream_max_retries` (5) attempts.
- **Critical guard: retry is only allowed if no bytes have been emitted yet.** It records
  `text_len_before_attempt` before each attempt and computes
  `emitted_any_text = len(full_text_ref[0]) > text_len_before_attempt`. The combined retry
  condition is:
  ```python
  can_retry = (not emitted_any_text
               and attempt < self.stream_max_retries
               and self._is_retryable_stream_error(exc))
  ```
  Once any partial text has streamed to the client, a mid-stream failure is **re-raised, not
  retried** — this avoids duplicating partial output.
- Accumulates the full response into `full_text_ref[0]` (a one-element list mutated in
  place) so higher layers can read the assembled text after the generator finishes.
- Transient failures are logged to stdout via `print` (`llm_service.py:197-201`).
- **Only streaming paths get this app-level retry.** The one-shot `complete_text` calls
  (bibliography, summary, skill review, quality check) are **not** wrapped and rely solely on
  the SDK's built-in retry.

Distinct `label` values are used for observability: `"stream"`, `"continuation"`,
`"length-continuation"`, `"revision"`.

---

## 4. Auto-Continuation (why one generation can be up to 3 API calls)

A single logical content-generation call can issue **multiple sequential API calls within
one held rate-limit slot**, all booked into one `record_usage` at the end. This lives in
`generate_content_stream` (`llm_service.py:427-675`).

1. **Truncation continuation** (`llm_service.py:595-635`): triggered when
   `usage["truncated"]` is `True` (the model stopped at `max_tokens`). It scans the
   accumulated text for still-missing required sections (self-assessment MCQs,
   `## Βιβλιογραφία`, `## Γλωσσάρι`), builds a `continuation_prompt` listing only the
   missing enabled sections, and re-streams. The continuation request replays the original
   prompt + the **last 4000 chars** of prior output as an assistant turn + the continuation
   instruction:
   ```python
   cont_messages = [
       {"role": "user", "content": prompt},
       {"role": "assistant", "content": full_text[-4000:]},
       {"role": "user", "content": continuation_prompt},
   ]
   ```
2. **Length continuation** (`llm_service.py:638-672`): triggered when the output came in
   materially short of the page target — `estimated_pages < page_target * 0.85` **and not**
   already truncated. Uses a **3000-chars-per-page** heuristic
   (`estimated_pages = len(full_text) // 3000`). Re-streams asking for the remaining pages,
   again replaying prompt + last-4000-char assistant turn (label `"length-continuation"`).
3. **Multi-batch continuation** (`llm_service.py:497-525`): cross-request continuation
   driven by the frontend via `batch_number` / `total_batches`. A `continuation_block`
   (prior headings + last 3000 chars) is injected into the prompt with explicit
   "continue from here / do not repeat" guards.

There is also a separate **revision path**, `_revision_stream` (`llm_service.py:336-422`),
which sends the current draft as an assistant turn between two user turns and streams via
`_stream_with_retry` with label `"revision"`. Its output budget is
`est_output = max(8000, min(len(current_draft)//3, 60000))`.

Each of these passes uses `settings.max_tokens` (= 64000) as the per-call output cap and is
individually wrapped in `_stream_with_retry`. Token totals are summed across all passes and
recorded once after the throttle context exits.

---

## 5. Rate Limiting & Concurrency (`rate_limiter.py`)

This is the core of the "workers / parallelization" answer. It is a **single, hand-rolled
admission-control layer** shared by both providers. Every application LLM call runs inside
`rate_limiter.throttle(...)`.

### 5.1 Priority (`rate_limiter.py:16-19`)

```python
class Priority(IntEnum):
    LIGHT = 0   # summary, bibliography, review
    HEAVY = 1   # content generation
```

Lower value = higher priority. The wait queue is sorted by priority
(`rate_limiter.py:232`), so **LIGHT auxiliary calls are scheduled ahead of HEAVY content
generation**.

### 5.2 Tier configuration (`rate_limiter.py:29-65`)

The limiter is shaped around **Anthropic API tiers**, selected by `settings.anthropic_tier`
(default **2** — `config.py:25`):

| Tier | rpm | input_tpm | output_tpm | max_concurrent_heavy | max_concurrent_light |
|------|-----|-----------|------------|----------------------|----------------------|
| **2** | 2000 | 200,000 | 90,000 | 1 | 3 |
| **3** | 4000 | 400,000 | 160,000 | 2 | 5 |
| **4** | 4000 | 400,000 | 400,000 | 4 | 8 |

Per-user caps are dataclass defaults for all tiers (`rate_limiter.py:39-40`):
**`per_user_heavy = 1`, `per_user_light = 2`**, keyed by user identity (the `X-Session-ID`
header; see §5.6).

Global constants (`rate_limiter.py:67-69`):
```python
_SAFETY_FACTOR  = 0.85   # only use 85% of the tier's output_tpm
_WINDOW_SECONDS = 60.0
```

### 5.3 Sliding-window token budget — `TokenBudgetTracker` (`rate_limiter.py:72-144`)

- Keeps a list of `_UsageRecord(timestamp, input_tokens, output_tokens)`, pruned to the
  trailing **60 seconds** (`_prune`, using `time.monotonic()`).
- **Admission gates on OUTPUT tokens only.** `can_proceed(estimated_output)`
  (`rate_limiter.py:104-109`):
  ```python
  effective_otpm = int(output_tpm * 0.85)      # Tier 2 → 76,500
  return (current_output + estimated_output) <= effective_otpm
  ```
  Input TPM is tracked for status reporting but **never enforced**.
- `estimated_wait_seconds(estimated_output)` (`rate_limiter.py:111-134`): computes how many
  output tokens must age out of the window, walks records oldest-first, and returns when
  enough will expire (`record.timestamp + 60 - now`). Worst case returns the full 60s.
- `record_usage(input, output)` appends a record stamped with `time.monotonic()`.

### 5.4 Concurrency control — `PrioritySemaphore` (`rate_limiter.py:158-285`)

This is **not** `asyncio.Semaphore` — it's a custom priority queue built from a single
`asyncio.Lock` plus per-waiter `asyncio.Event`s:

- Integer counters `_active_heavy` / `_active_light`; per-user counts in
  `_user_slots: dict[user_id, {HEAVY: int, LIGHT: int}]`.
- `_can_acquire` (`rate_limiter.py:178-190`) checks **both** the global concurrency cap
  **and** the per-user cap; both must pass.
- `acquire(user_id, priority)` (`rate_limiter.py:220-250`): if a slot is free, take it and
  return `0` (immediate). Otherwise enqueue a `_QueueEntry`, re-sort by priority, and
  `await entry.event.wait()` **outside the lock**, returning the queue position. On
  `asyncio.CancelledError` (client disconnect), it removes itself from the queue or — if a
  slot was already pre-acquired for it — releases that slot and wakes the next waiter, to
  avoid leaked slots.
- `release` → `_wake_next` (`rate_limiter.py:257-266`): scans the (priority-sorted) queue
  and wakes the **first eligible** waiter (one that passes `_can_acquire`), pre-acquiring its
  slot. A HEAVY waiter blocked by its per-user cap can therefore be skipped in favor of a
  later-but-eligible waiter.

### 5.5 Admission flow — `_ThrottleContext` (`rate_limiter.py:327-378`)

Entering `async with rate_limiter.throttle(user_id, priority, estimated_output)` performs a
**two-stage acquire** (`__aenter__`, `rate_limiter.py:345-362`):

1. **Slot** — `await semaphore.acquire(user_id, priority)` (may queue).
2. **Token budget** — if `estimated_output > 0`, loop while `not can_proceed(...)`:
   compute the wait, store it on `self.estimated_wait`, then
   `await asyncio.sleep(min(wait, 5.0))` — i.e. **recheck at most every 5 seconds**.

On exit (`__aexit__`) the slot is released. **Token usage is NOT recorded here** — callers
must call `tracker.record_usage(...)` themselves after the context exits, with the *actual*
token counts returned by the provider. So `estimated_output` is used only for admission and
queue-wait estimation; real usage is booked afterward.

### 5.6 Throttle call sites (`llm_service.py`)

| Function (def line) | Priority | `estimated_output` |
|---|---|---|
| `generate_content_stream` (`:427`) | **HEAVY** | `30000` (hardcoded) |
| `_revision_stream` (`:336`) | **HEAVY** | `max(8000, min(len(draft)//3, 60000))` |
| `generate_bibliography` (`:680`) | **LIGHT** | `4000` |
| `generate_summary` (`:724`) | **LIGHT** | `4000` |
| `generate_skill_review` (`:762`) | **LIGHT** | `5000` |

The generic proxy `routers/claude.py` also throttles directly — `/api/claude/generate` and
`/api/claude/generate-stream` wrap the call in `throttle(user_id, Priority.LIGHT,
estimated_output=request.maxTokens)` (default `maxTokens = 16000`) and call `record_usage`
after the context exits.

**Not throttled:** the non-stream multipass path `generate_content_multipass`
(`llm_service.py:209`, used by `/api/generate`) and the entire Research Hub layer
(`research_service.py`) do not go through the rate limiter.

### 5.7 User identity & queue feedback

- **User id** (`routers/generate.py:24-30`, duplicated in `routers/claude.py:24-30`): the
  `X-Session-ID` header; falls back to `sha256(client_ip)[:16]`. This keys the per-user caps.
  CORS explicitly allows the `X-Session-ID` header (`main.py:38`).
- **Queue position to the frontend** (`routers/generate.py:228-236`): before streaming,
  `/api/generate-stream` *peeks* at limiter state (`tracker.can_proceed(30000)` and
  `semaphore.get_status()`); if HEAVY is full or the budget is exhausted, it emits an SSE
  event `{"type": "queue", "position": …, "estimated_wait": …}`. The actual throttling still
  happens inside `generate_content_stream`.
- **Status endpoint:** `GET /api/rate-limit/status` (`main.py:52-57`, JWT-protected) returns
  the merged tier + token + semaphore status. `init_rate_limiter()` runs at startup
  (`main.py:66`).

---

## 6. Parallelization — there is (deliberately) none in the Python backend

A repository-wide search for fan-out / worker-pool primitives —
`asyncio.gather`, `asyncio.wait`, `as_completed`, `TaskGroup`, `create_task`,
`ThreadPoolExecutor`, `ProcessPoolExecutor`, `run_in_executor`, `concurrent.futures`,
`asyncio.Semaphore` — returns **no matches** in the backend `src/` tree. (The only
"semaphore" is the custom `PrioritySemaphore` class name.)

Concretely:

- Concurrency is **request-level only**: each incoming HTTP request runs its own coroutine
  under Uvicorn, and the shared `RateLimiter` singleton admits/serializes them via the
  priority queue + OTPM budget.
- **Within a single request, all provider API calls are sequential** — the initial stream
  followed by any auto-continuation passes, one after another, inside a single held slot.
- There are **no background workers** and **no parallel fan-out of LLM calls**. The
  "workers" concept in this system is effectively the concurrency *caps*
  (`max_concurrent_heavy` / `max_concurrent_light` / per-user caps), not a worker pool.

---

## 7. Research Hub / external HTTP (context)

Not LLM API calls, but relevant because this is where real parallelism and adaptive rate
limiting actually exist in the system.

### 7.1 Python side — `research_service.py` (sequential, no retries)

Uses **`httpx.AsyncClient`**. Calls are `await`-ed **one after another** in `search_papers`
— there is **no `asyncio.gather`**, no retry/backoff, no rate limiting; each call is
wrapped in try/except and degrades gracefully:

- `POST {rust_hub}/api/search` → the local Rust hub at `http://localhost:8091`
  (`config.py:32`), 60s timeout.
- `GET https://api.crossref.org/works` (30s timeout, custom User-Agent) — used as a direct
  fallback when the Rust hub is unavailable.
- `GET https://repository.kallipos.gr/...` (30s timeout).
- `GET {rust_hub}/health` (5s timeout) — a `ConnectError` flips `_rust_available = False`.

### 7.2 Rust side — `research_hub_mcp/` (where the parallelism lives)

The Rust MCP server is the component that genuinely fans out and rate-limits:

- **HTTP client:** `reqwest 0.11` (the only external HTTP client in the Rust crate).
- **Fan-out:** `meta_search.rs` queries up to 13 providers (arXiv, CrossRef, OpenAlex,
  PubMed Central, Semantic Scholar, Unpaywall, …) **in parallel via `tokio::spawn`**, gated
  by a **`tokio::sync::Semaphore`** whose size is **adaptive** based on measured provider
  response times (`max_parallel_providers = 3` baseline). Each task applies a per-provider
  rate limit and a per-provider timeout; partial success is tolerated.
- **Rate limiting / resilience:** per-provider `AdaptiveRateLimiter` (`rate_limiter.rs`), a
  circuit breaker (`circuit_breaker_service.rs`), and mirror failover (`mirror.rs`). Retry
  crates in use: `tokio-retry`, `backoff`, `failsafe`, with `rand` for jitter.

The Rust server makes **no LLM API calls** — the only OpenAI reference in that crate is an
unused config field in `extension/manifest.json` for a feature not implemented in the Rust
source.

---

## 8. Dependency versions relevant to API calls

**Python** (`backend_py/pyproject.toml`):

- `anthropic >= 0.40.0`
- `openai >= 1.50.0`
- `httpx >= 0.26.0`
- `fastapi >= 0.109.0`, `uvicorn[standard] >= 0.27.0`

**Rust** (`research_hub_mcp/Cargo.toml`):

- `reqwest = "0.11"` (json/stream/gzip/rustls-tls)
- `tokio = "1.0"` (full), `futures = "0.3"`
- `tokio-retry = "0.3"`, `backoff = "0.4"`, `failsafe = "1.3"`

**Frontend** (`web/package.json`): no HTTP-client dependency — uses the native `fetch` API.
No client-side retry loops or `Promise.all` fan-out; the frontend only calls the backend.

---

## 9. Observations & gaps

Non-prescriptive notes surfaced while mapping the code:

- **OpenAI shares the Anthropic-shaped budget.** The rate limiter is configured entirely
  from Anthropic tier numbers (`TIER_CONFIGS`), and OpenAI traffic is booked into the *same*
  `TokenBudgetTracker` with no per-provider tracking. There is no OpenAI-specific rate
  configuration.
- **Two retry layers stack.** The SDK's built-in retry (default `max_retries = 2`) sits
  underneath the app's `_stream_with_retry` (up to 5 attempts). Worst-case cumulative retry
  behavior for a transient error is the product of both layers.
- **Retry only wraps streaming.** One-shot `complete_text` calls (bibliography, summary,
  skill review, quality check, outline/expand/citations passes) get only the SDK's retry,
  not the app-level backoff.
- **Some paths bypass the limiter.** The non-stream multipass generation
  (`generate_content_multipass`) and all Research Hub HTTP calls are not throttled.
- **Admission uses only output TPM.** Input TPM and RPM from the tier config are tracked or
  declared but not enforced for admission — the design comment calls output TPM "the real
  bottleneck."
- **Secrets in `.env`.** Live keys are present in the committed `.env` (values not
  reproduced here). Rotating them and removing them from version control is advisable.

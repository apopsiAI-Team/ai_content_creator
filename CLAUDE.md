# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Educational Material Creator v2 — A web application for generating Greek-language training materials with academic-quality bibliography and (optionally) ESCO skill coverage analysis.

The project has four main components:

1. **Web App** (`web/`): React 19 + TypeScript + Vite + Zustand frontend
2. **Python Backend** (`backend_py/`): FastAPI service that fronts the Anthropic API, handles rate limiting, runs the multi-stage prompts, and brokers Research Hub / ESCO data
3. **Research Hub MCP** (`research_hub_mcp/`): Rust HTTP/MCP server for multi-source academic paper search (CrossRef, Semantic Scholar, Unpaywall, arXiv, PubMed, OpenAlex, …)
4. **ESCO Dataset**: Greek skill classification (`ESCO dataset - v1.2.1 - classification - el - csv/`), preprocessed into `backend_py/src/edu_backend/data/skills_compact.json`

## Common Commands

### Web Application (React/Vite)
```bash
cd web
npm install              # Install dependencies
npm run dev              # Dev server on :5173
npm run build            # tsc -b && vite build
npm run lint             # ESLint
npm run preview          # Preview production build on :4173
```

### Python Backend (FastAPI)
```bash
# From project root
python -m venv venv && source venv/bin/activate
pip install -e backend_py        # Installs edu_backend package
uvicorn edu_backend.main:app --reload --host 0.0.0.0 --port 8000
# or:  python -m edu_backend.main
```
Requires `ANTHROPIC_API_KEY` in `.env` at project root (loaded by `main.py`).

Tests: `pip install -e "backend_py[dev]"` then `cd backend_py && pytest tests` (structure prompts, page budget, citation verifier with mocked HTTP).

### Research Hub MCP (Rust)
```bash
cd research_hub_mcp
cargo nextest run                      # Tests (parallel)
cargo clippy -- -D warnings            # Lint (must pass)
cargo fmt
cargo build --release
cargo run --release -- http --port 8091    # HTTP server consumed by the Python backend
cargo run -- serve                          # MCP stdio mode (alt)
```
The Python backend probes `http://localhost:8091` on startup; if the Rust server is down it falls back to direct CrossRef calls.

## Architecture

### Two workflow modes (`workflowMode` in `useStore`)

The Landing Page exposes a tab bar with two distinct entry points:

1. **Standard** (`workflowMode: 'standard'`) — Free-form topic generation
   - User types a Greek topic, total hours, total module pages, pages per batch
   - A synthetic single-module is created (no ESCO skills attached)
   - Skips the ESCO upload + skill-coverage review steps entirely
   - Within Standard, `contentMode` selects the **content sub-mode** (see below)

2. **ESCO Integrated** (`workflowMode: 'esco'`) — DOCX-driven, skills-aware
   - User uploads a `.docx` educational design; `mammoth` + `parseEducationalDesign` extract title, total hours, modules, and per-module ESCO skills
   - User picks a module from `ModuleList`; generation runs against that module's ESCO skill list
   - After generation, an ESCO **skill coverage review** runs (full / partial / missing per skill, with evidence quotes)

### Content mode (`contentMode`)

Only `experimental` is exposed in the UI (the "Κανονική"/`standard` Research Hub mode was retired — its bibliography was too thin in practice). The type still allows `'standard'` for compatibility, but the store always sets `'experimental'` and `claudeService` never requests the Research Hub.

- `experimental` — Anti-hallucination prompt (`EXPERIMENTAL_SYSTEM_PROMPT` in `prompts/system_prompt.py`); the model cites real sources from its own knowledge (seminal works/textbooks → official reports → highly cited articles), DOI only when certain, theses forbidden, full APA bibliography mandatory.
- **Bibliography verification** — after every batch the frontend calls `POST /api/verify-bibliography` (`services/citation_verifier.py`): each entry is checked against CrossRef → OpenAlex → Open Library (books), or its cited URL must resolve (reports). Wrong DOIs are corrected (same year ±1 only) or removed in the content; unverified entries, placeholders, theses and in-text citations without an entry are flagged on the batch card, with a one-click revision to replace them.
- **Citation support** — existence is not enough, so the same endpoint also flags (a) bibliography entries never cited in the text (`uncited_entries`; only when the text uses in-text citations) and (b) real sources that do not support the sentences citing them: `services/citation_relevance.py` sends each cited entry's title + abstract (from CrossRef/OpenAlex) and its citing sentences to a small model (`settings.relevance_model_id`, Haiku 4.5) in one call per batch → `relevance` ∈ `supports | general | unrelated | uncertain`. Only `unrelated` is surfaced. It catches gross topic mismatches, not wrong page-level details (no full texts).

`userInstructions` is an independent free-text steer, entered on the generation screen ("Προσθήκη οδηγιών" in `ContentGenerator`, stored in `useStore`); it applies to every following batch until the user edits or clears it. The Landing Page and ESCO module list no longer have their own instructions box.

### Structure (`structureConfig`)

Optional elements are **on demand**: `activities`, `self_assessment`, `glossary`, `subsection_keywords` default OFF, `in_text_citations` defaults ON (`DEFAULT_STRUCTURE_CONFIG` in `useStore.ts`, mirrored in `prompts/structure.py` and `StructureConfigModel`). For a disabled element `prompts/structure.py` strips its prompt section, neutralises stray mentions, and appends an explicit manifest (`build_structure_manifest`) forbidding it. The DOCX `activities` field is only sent when activities are ON. `in_text_citations` OFF means **no** (Επώνυμο, Έτος) in the body at all — the citation rules/examples are stripped from the prompt and the Βιβλιογραφία stays as the list of sources the content is based on. `detect_structure_violations` runs on the finished stream; hits are sent as a `structure_warnings` SSE event and shown on the batch card with a one-click removal. Elements the user's instructions ask for (`elements_requested`) are never flagged. In revisions (`mode: "revision"`) `revision_config` treats elements already present in the draft (e.g. an uploaded document's glossary) as part of its structure: they are kept and not flagged; only adding a disabled element is forbidden.

### End-to-end content flow

1. Frontend posts to `POST /api/generate-stream` with the module dict, `experimental_mode`, `target_pages`, `learning_outcomes`, `keywords`, `previous_content`, `batch_number`, `total_batches`, optional `user_instructions`.
2. Backend (`generate.py` → `claude_service.generate_content_stream`):
   - Emits `{type: 'references', data: [...]}` first (empty — the Research Hub path is no longer used by the UI)
   - May emit a `{type: 'queue', position, estimated_wait}` event if the rate limiter is saturated
   - Streams `{type: 'content', text}` deltas as Claude streams
   - **Auto-continuation:** if `stop_reason == "max_tokens"` OR final length is < 70% of the page target, sends a follow-up turn that includes the last 4k chars as the assistant message and asks Claude to continue — at most up to the target (and to add any missing enabled end sections). Token usage is summed across all turns.
   - `{type: 'structure_warnings', data: [...]}` if disabled elements appeared anyway, then the final `{type: 'done'}` event
3. Frontend (`claudeService.generateWithStreaming`) accumulates the text and, **if no Bibliography section landed**, scrapes in-text citations from the body and calls `POST /api/generate-bibliography` to synthesize an APA list as a fallback. It then calls `POST /api/verify-bibliography` and stores the result on the batch (`bibliographyCheck`).
4. User approves/rejects each batch; rejected batches are regenerated; pages add up to `totalModulePages`.
5. ESCO mode: after all batches are approved, frontend calls `POST /api/review` for skill-coverage analysis (`SkillCoverageReview` rendered by `SkillCoverageReview.tsx`).
6. **Approve & Finish** triggers `POST /api/generate-summary` (Περίληψη) over the concatenated approved content. Its length scales with the module (`summary_word_target`: ~5% of `total_pages`, 150–800 words).
7. `wordExport.exportToWord` consolidates all batches into a single `.docx`: renumbered MCQs across batches, deduped alphabetical bibliography, merged glossary, the generated Περίληψη before the bibliography, optional TOC and page numbers, APOPSI e-learning branding.

### Backend layout (`backend_py/src/edu_backend/`)
- `main.py` — FastAPI app, CORS allowlist (5173 / 4173 only), startup banner, Rust hub health probe, rate-limit status endpoint
- `config.py` — `Settings` (pydantic): **`model_id = "claude-opus-5"`**, `max_tokens = 64000`, `anthropic_tier = 2`, `rust_research_hub_url = http://localhost:8091`
- `rate_limiter.py` — Semaphore-based concurrency control + sliding-window token tracker; tier-aware (2/3/4); `Priority.HEAVY` (content gen) vs `Priority.LIGHT` (summary/bibliography/review); per-user caps; emits queue position to frontend
- `services/claude_service.py` — All Anthropic calls. Prompts cached via `cache_control: ephemeral` (90% cost reduction, ~5 min TTL). Stream retries with exponential backoff + jitter for 429/5xx/overload.
- `services/research_service.py` — CrossRef + Rust hub queries; thesis/dissertation filtering; English keyword translation for Greek-language queries
- `services/citation_verifier.py` — Bibliography verification (CrossRef / OpenAlex / Open Library / URL check), DOI correction, orphan/uncited-citation detection
- `services/citation_relevance.py` — Small-model check that each cited source supports its citing sentences
- `prompts/structure.py` — `StructureConfig`, prompt stripping + manifest for disabled elements, `detect_structure_violations`
- `services/esco_service.py` — Loads `data/skills_compact.json` (preprocessed Greek ESCO skills); name lookup + partial search
- `prompts/system_prompt.py` — `SYSTEM_PROMPT`, `OUTLINE_PROMPT`, `EXPAND_PROMPT`, `CITATIONS_PROMPT`, `REVIEW_PROMPT`, `STANDARD_CONTENT_STRUCTURE`
- `routers/`:
  - `health.py` — `GET /api/health`
  - `esco.py` — `GET /api/esco/skills`, `GET /api/esco/search`
  - `generate.py` — `POST /api/generate` (multipass), `POST /api/generate-stream`, `POST /api/generate-summary`, `POST /api/generate-bibliography`, `POST /api/verify-bibliography`, `POST /api/review`, `GET /api/research/search`
  - `claude.py` — Generic `POST /api/claude/generate` and `/api/claude/generate-stream` proxies (used by the frontend's ESCO skill-review JSON call)

### Web app key files
- `web/src/store/useStore.ts` — Zustand store with `persist` middleware on **sessionStorage** (clears on tab close). Auto-removes any stale `localStorage` key from a prior config.
- `web/src/services/api.ts` — Fetch wrappers; injects `X-Session-ID` header (stable per tab via `crypto.randomUUID()`) used by the rate limiter for per-user budgeting; SSE parsing; `VITE_API_URL` overrides default same-origin
- `web/src/services/claudeService.ts` — Orchestrates streaming generation, citation extraction, and the bibliography-fallback follow-up
- `web/src/components/LandingPage.tsx` — Tab bar (Standard / ESCO Integrated) + per-tab forms
- `web/src/components/ModuleList.tsx` — ESCO mode module picker
- `web/src/components/ContentGenerator.tsx` — Per-batch generate / approve / regenerate UI, queue indicator, "Approve & Finish" trigger
- `web/src/components/SkillCoverageReview.tsx` — ESCO coverage report UI
- `web/src/utils/docxParser.ts` — DOCX → modules + skills (mammoth)
- `web/src/utils/wordExport.ts` — Final consolidated `.docx` generation (`docx` library)

### Zustand state slices
- **Workflow**: `workflowMode: 'standard' | 'esco'`
- **Document**: `documentFile`, `documentTitle`, `modules`, `totalHours`
- **Standard extras**: `totalModulePages`, `targetPages`, `learningOutcomes`, `keywords`
- **Generation**: `selectedModule`, `currentBatch`, `generatedBatches`, `isGenerating`, `generationProgress`
- **Content sub-mode**: `contentMode` (always `'experimental'`), `userInstructions`, `structureConfig`
- **References**: `pendingReferences`, `approvedReferences`
- **ESCO review**: `skillReviews`, `isReviewingSkills`
- **Production**: `productionComplete`, `moduleSummaries`, `isGeneratingSummary`
- **Queue**: `queuePosition`, `estimatedWait`
- **UI**: `currentStep: 'upload' | 'modules' | 'generate' | 'review' | 'export'`, `error`

## Environment Variables

Project root `.env`:
```
ANTHROPIC_API_KEY=<key>
```
Optional: `ANTHROPIC_TIER` (2/3/4, default 2).

OpenAI (`gpt-5.6-sol`) is **disabled by default**: `get_llm_service()` falls back to Claude for every request and the Landing Page hides the model picker (it reads `openai_enabled` from `GET /api/health`). To enable it, set `OPENAI_ENABLED=true` and `OPENAI_API_KEY=<key>` in `.env` and restart the backend.

Frontend (`web/.env`, optional):
```
VITE_API_URL=http://localhost:8000   # default: same-origin
```

## Content Generation Notes

- **Model:** `claude-opus-5` (set in `backend_py/.../config.py`). Streaming via `client.messages.stream`; non-stream paths use `client.messages.create`.
- **Prompt caching:** every system prompt is wrapped via `_cacheable_system()` with `cache_control: {"type": "ephemeral"}`.
- **Per-batch sections:** Σκοπός → Προσδοκώμενα → Λέξεις Κλειδιά → (Εισαγωγή only batch 1) → Υποενότητες → [MCQs] → Βιβλιογραφία → [Γλωσσάρι] (bracketed = only when enabled in `structureConfig`).
- **Dynamic MCQ count** (only when `self_assessment` is ON): 20 total per module, distributed proportionally across batches (`per_batch = max(5, 20 // total_batches + ...)`), capped so MCQs take ≤ ~25% of the batch's pages (`floor(target × 0.25 / 0.18)`).
- **Continuation between batches:** for batch > 1, the previous content's headings + last 3000 chars are injected with explicit "ΣΥΝΕΧΙΣΕ ΑΠΟ ΕΔΩ" + "ΜΗΝ επαναλάβεις" guards.
- **Page sizing:** one page = `chars_per_page` = **2000 chars** (what fits on a page of the exported .docx: Calibri 12pt, 1.5 spacing). Keep `settings.chars_per_page` and `CHARS_PER_PAGE` (`web/src/utils/pages.ts`) in sync. The page target covers the **whole batch** (body + MCQs + bibliography + glossary): `page_budget()` subtracts the enabled end sections and the prompt states body pages, word count, subsection count, and a hard ceiling of `target_pages + 5`. MCQs are capped at ~25% of a batch's pages. The last batch requests only the remaining pages (min 1) and also reserves room for the module summary. The cover page and table of contents of the Word export are not counted.
- **Greek language throughout**, APA 7th, **parenthetical citations only** (the system prompt forbids "Σύμφωνα με τον X..." and forces "et al.", never "κ.ά.").
- **Forbidden source types:** undergraduate / master's / doctoral theses are filtered out both by the prompt and by `research_service._filter_theses`.

## ESCO Integration

- Backend loads `backend_py/.../data/skills_compact.json` (compact `{name → description}` map preprocessed from the Greek ESCO 1.2.1 CSVs).
- ESCO mode parses the uploaded `.docx` into modules with `skills: ESCOSkill[]` (`{ code, name, type: 'essential'|'optional' }`).
- Generation appends the ESCO skill list and a "must cover all of these" instruction to the module context.
- Post-generation review (`POST /api/review`): Claude returns per-skill `coverageLevel` ∈ `{full, partial, missing}` plus evidence quotes; backend computes a weighted percentage (`full = 100`, `partial = 50`).

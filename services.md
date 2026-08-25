# API Services Documentation

**Backend:** Python FastAPI
**Base URL:** `https://devai.apopsi.gr/e-learning` (production) | `http://localhost:8000` (local)
**Όλα τα endpoints** ξεκινούν με `/api/`

> **Multi-provider:** Από το 2026-04-29 όλα τα `POST` endpoints παραγωγής δέχονται προαιρετικό πεδίο `model_provider: "claude" | "openai"` (default `"claude"`). Όταν είναι `"openai"`, η ίδια λογική εκτελείται μέσω OpenAI **gpt-5.6-sol** αντί Claude Opus 5, με ίδια system prompts και preconditions.

> **Session header:** Το frontend στέλνει σε όλες τις κλήσεις τον header `X-Session-ID: <uuid>` (stable per user-session, όχι per request) που χρησιμοποιείται από τον rate limiter για per-user budgeting. Αν λείπει, ο backend κάνει fallback σε hash του client IP — που σημαίνει shared bucket για όλους τους users πίσω από το ίδιο NAT/proxy. Δες παρακάτω «Session & document correlation» για οδηγίες.

> **Per-document correlation:** Όλα τα POST endpoints παραγωγής/αξιολόγησης (`/api/generate-stream`, `/api/review`, `/api/generate-summary`, `/api/generate-bibliography`) δέχονται προαιρετικό πεδίο `document_id: string` στο body. Σταθερό ID ανά uploaded doc / draft session — εμφανίζεται στα backend logs ώστε να διακρίνονται δύο users που δουλεύουν στο ίδιο αρχικό αρχείο. Default κενό string.

---

## Session & document correlation

**X-Session-ID (header):**
- Σταθερό UUID **ανά user-session** (όχι ανά κλήση). Το frontend της εφαρμογής το γεννά μία φορά ανά browser tab μέσω `crypto.randomUUID()`.
- Συνιστώμενη .NET υλοποίηση: `Guid.NewGuid().ToString()` αποθηκευμένο σε ASP.NET Session/Identity claim ή cookie, ώστε να ταιριάζει με one-user-many-tabs scenarios.
- Χρησιμοποιείται **μόνο για rate limiting fairness** (per-user concurrency caps στο semaphore + per-user queue position).

**document_id (body field, optional):**
- Σταθερό ID **ανά uploaded doc / draft session**, μορφή `doc-<uuid4>`.
- **Δύο πηγές**:
  - **Server-generated** όταν ξεκινάς από `.docx`: το `POST /api/parse-docx` επιστρέφει `"document_id": "doc-..."` στο response. Διάβασέ το και επαναχρησιμοποίησέ το.
  - **Client-generated** όταν δεν περνάς από upload (π.χ. Standard mode): γέννησέ το μόνος σου (`Guid.NewGuid().ToString()`).
- Χρησιμοποίησε το **ίδιο id** για όλες τις κλήσεις του ίδιου εγγράφου (όλα τα batches, revisions, review, summary, bibliography).
- Εμφανίζεται στα backend stdout logs (`[api] endpoint=/api/generate-stream user=abc123 doc=doc-xyz789 ...`) — χρήσιμο για διάκριση δύο users που δουλεύουν με ίδιο αρχικό docx, traceability, audit logs.
- Δεν επηρεάζει routing/rate-limiting/caching — μόνο logging visibility.

**occupation (body field, optional — `/api/generate-stream`, `/api/review`):**
- ESCO **occupation** (επάγγελμα) που στοχεύει το πρόγραμμα κατάρτισης. **Program-level** — ίδιο για όλες τις ενότητες του ίδιου `.docx`.
- Σχήμα: `{ "code": "...", "name": "...", "description": "..." }`. Μόνο το `name` είναι required αν στείλεις το αντικείμενο.
- Όταν παρέχεται, μπαίνει ως context line στο prompt ώστε το μοντέλο να προσαρμόσει ορολογία/παραδείγματα στο συγκεκριμένο επάγγελμα.

---

## Πίνακας Endpoints

| Method | Endpoint | Σε χρήση από frontend; | Περιγραφή |
|--------|----------|------------------------|-----------|
| GET | `/api/health` | ✓ | Health check |
| GET | `/api/rate-limit/status` | ✗ (διαθέσιμο για monitoring) | Κατάσταση rate limiter (token budget + concurrency) |
| GET | `/api/research/search` | ✗ (καλείται έμμεσα από `/api/generate-stream`) | Αναζήτηση ακαδημαϊκών papers |
| POST | `/api/parse-docx` | ✗ (frontend έχει δικό του parser) | Parsing `.docx` εκπαιδευτικού σχεδίου → structured JSON |
| POST | `/api/docx-to-markdown` | ✗ (helper για consumers) | Μετατροπή `.docx` → markdown string (για revision mode) |
| POST | `/api/generate` | ✗ (multipass — μόνο για batch/API χρήση) | Παραγωγή εκπαιδευτικού υλικού (non-streaming) |
| POST | `/api/generate-stream` | ✓ ⭐ | Παραγωγή εκπαιδευτικού υλικού (streaming/SSE) |
| POST | `/api/generate-summary` | ✓ | Παραγωγή περίληψης ενότητας |
| POST | `/api/generate-bibliography` | ✓ (fallback) | Παραγωγή βιβλιογραφίας από in-text citations |
| POST | `/api/review` | ✓ (ESCO mode) ⭐ | **Επίσημο endpoint για ESCO skill coverage** — χρησιμοποιείται και από το web app |
| POST | `/api/claude/generate` | ✗ (legacy) | LLM proxy — **deprecated για νέα integrations**· κρατείται για backward compatibility |
| POST | `/api/claude/generate-stream` | ✗ (legacy, διατηρείται) | LLM streaming proxy |

---

## 1. GET `/api/health`

Health check — ελέγχει αν το backend είναι ενεργό και σωστά ρυθμισμένο.

**Χρήση:** Καλείται αυτόματα από το frontend κατά την εκκίνηση.

**Response:**
```json
{
  "status": "ok",
  "has_api_key": true,
  "model": "claude-opus-5",
  "research_hub_available": true,
  "esco_data_available": true
}
```

| Πεδίο | Τύπος | Περιγραφή |
|-------|-------|-----------|
| `status` | string | Πάντα `"ok"` αν ο server τρέχει |
| `has_api_key` | bool | Αν υπάρχει Anthropic API key |
| `model` | string | Μοντέλο Claude που χρησιμοποιείται (default `claude-opus-5`) |
| `research_hub_available` | bool | Αν ο Rust Research Hub είναι διαθέσιμος |
| `esco_data_available` | bool | Αν τα δεδομένα ESCO (ελληνικά) είναι φορτωμένα |

> Σημείωση: το endpoint αναφέρει μόνο το Claude model. Η διαθεσιμότητα του OpenAI provider κρίνεται από την ύπαρξη του `OPENAI_API_KEY` στο `.env` και ελέγχεται κατά την πρώτη κλήση.

---

## 1.5 GET `/api/rate-limit/status`

Κατάσταση του rate limiter — token budget σε sliding window και concurrency.

**Χρήση:** Διαθέσιμο για monitoring / debugging· δεν καλείται από το frontend.

**Response:**
```json
{
  "tier": 2,
  "tokens": {
    "output_tokens_used": 12500,
    "output_tokens_limit": 81000,
    "input_tokens_used": 45000,
    "requests_in_window": 4
  },
  "semaphore": {
    "active_heavy": 1,
    "active_light": 0,
    "max_heavy": 1,
    "max_light": 3,
    "queue_heavy": 0,
    "queue_light": 0
  }
}
```

| Πεδίο | Περιγραφή |
|-------|-----------|
| `tier` | Anthropic tier (2/3/4) — οδηγεί τα concurrency caps |
| `tokens.output_tokens_used` | Output tokens που καταναλώθηκαν στο τρέχον λεπτό |
| `tokens.output_tokens_limit` | Σκληρό όριο (με safety factor) |
| `semaphore.active_heavy` | Πόσες παραγωγές υλικού τρέχουν αυτή τη στιγμή |
| `semaphore.queue_heavy` | Πόσες περιμένουν στην ουρά |

> **Provider-agnostic:** Ο rate limiter παρακολουθεί συνολικά tokens ανεξαρτήτως provider. Τα `TIER_CONFIGS` είναι σχηματισμένα για Anthropic Tier 2 (90 K OTPM). Όταν χρησιμοποιείται OpenAI gpt-5.6-sol ταυτόχρονα, καταγράφεται στο ίδιο budget — δεν υπάρχει per-provider tracking.

---

## 2. GET `/api/research/search`

Αναζήτηση ακαδημαϊκών papers μέσω Research Hub (Rust MCP server). Συνδυάζει πολλές πηγές: **CrossRef, Semantic Scholar, Unpaywall, arXiv, PubMed, OpenAlex**. Όταν ο Rust hub είναι offline, υπάρχει fallback κατευθείαν σε CrossRef. Φιλτράρει αυτόματα διπλωματικές / διδακτορικά (forbidden source types).

**Χρήση:** Καλείται **έμμεσα** από το `/api/generate-stream` (standard mode) πριν την παραγωγή. Το direct GET endpoint είναι διαθέσιμο για debugging και δεν καλείται από το frontend.

**Query Parameters:**

| Parameter | Τύπος | Required | Default | Περιγραφή |
|-----------|-------|----------|---------|-----------|
| `query` | string | Ναι | — | Λέξεις-κλειδιά αναζήτησης |
| `limit` | int | Όχι | 15 | Μέγιστος αριθμός αποτελεσμάτων |
| `include_greek` | bool | Όχι | true | Συμπεριλάβει ελληνικές πηγές |

**Παράδειγμα:**
```
GET /api/research/search?query=διοίκηση ολικής ποιότητας&limit=15&include_greek=true
```

**Response:**
```json
{
  "papers": [
    {
      "title": "Total Quality Management in Education",
      "authors": ["Sallis, E."],
      "year": 2014,
      "journal": "Routledge",
      "doi": "10.4324/9780203417010"
    }
  ],
  "count": 15
}
```

---

## 3. POST `/api/generate`

Παραγωγή εκπαιδευτικού υλικού — NON-STREAMING (multi-pass, υψηλή ποιότητα).

**Χρήση:** Δεν χρησιμοποιείται ενεργά από το frontend (προτιμάται το streaming). Υπάρχει για batch/API χρήση.

**Request Body:**
```json
{
  "module": {
    "number": 1,
    "title": "Διοίκηση Ολικής Ποιότητας",
    "hours": 10,
    "content": "Περιγραφή ενότητας...",
    "activities": "Δραστηριότητες...",
    "skills": [
      { "code": "S1.1", "name": "διαχείριση ποιότητας", "type": "essential" }
    ]
  },
  "use_research_hub": true,
  "multipass": true,
  "include_greek_sources": true,
  "experimental_mode": false,
  "user_instructions": "",
  "target_pages": 20,
  "learning_outcomes": "",
  "keywords": "",
  "previous_content": "",
  "batch_number": 1,
  "total_batches": 1,
  "model_provider": "claude"
}
```

| Πεδίο | Τύπος | Required | Default | Περιγραφή |
|-------|-------|----------|---------|-----------|
| `module` | object | Ναι | — | Στοιχεία ενότητας (βλ. παρακάτω) |
| `use_research_hub` | bool | Όχι | true | Αναζήτηση πραγματικών papers πριν την παραγωγή |
| `multipass` | bool | Όχι | true | Multi-pass generation (outline → expand → citations) |
| `include_greek_sources` | bool | Όχι | true | Αναζήτηση ελληνικών πηγών |
| `experimental_mode` | bool | Όχι | false | Χρήση μοντέλου χωρίς Research Hub, αυστηρό anti-hallucination |
| `user_instructions` | string | Όχι | "" | Οδηγίες χρήστη για το περιεχόμενο |
| `target_pages` | int/null | Όχι | 20 | Στόχος σελίδων (~3000 χαρ/σελίδα) |
| `learning_outcomes` | string | Όχι | "" | Μαθησιακά αποτελέσματα |
| `keywords` | string | Όχι | "" | Λέξεις-κλειδιά (comma-separated) |
| `previous_content` | string | Όχι | "" | Περιεχόμενο προηγούμενων batches (αποφυγή επανάληψης) |
| `batch_number` | int | Όχι | 1 | Τρέχον batch |
| `total_batches` | int | Όχι | 1 | Εκτιμώμενος συνολικός αριθμός batches |
| `model_provider` | `"claude"` \| `"openai"` | Όχι | `"claude"` | Επιλογή LLM provider — Claude Opus 5 ή OpenAI gpt-5.6-sol |
| `mode` | `"generate"` \| `"revision"` | Όχι | `"generate"` | Λειτουργία: `generate` = κανονική παραγωγή, `revision` = στοχευμένη αναθεώρηση υπάρχοντος draft |
| `current_draft` | string | Όχι (απαιτείται σε `revision`) | "" | Το υπάρχον batch content που θα αναθεωρηθεί. Στέλνεται ως assistant turn· το `user_instructions` περιγράφει τις στοχευμένες αλλαγές |
| `document_id` | string | Όχι | "" | Optional correlation id (stable per uploaded doc/draft session) — εμφανίζεται στα backend logs για διάκριση πολλαπλών users |
| `occupation` | object \| null | Όχι | null | ESCO occupation που στοχεύει το πρόγραμμα κατάρτισης (program-level metadata). Όταν παρέχεται, μπαίνει ως context line στο prompt ώστε το μοντέλο να προσαρμόσει ορολογία/παραδείγματα. Σχήμα: `{ code, name, description? }`. |

**Module object:**

| Πεδίο | Τύπος | Required | Περιγραφή |
|-------|-------|----------|-----------|
| `number` | int | Ναι | Αριθμός ενότητας |
| `title` | string | Ναι | Τίτλος ενότητας |
| `hours` | int | Όχι | Ώρες διδασκαλίας |
| `content` | string | Όχι | Περιγραφή περιεχομένου από εκπαιδευτικό σχέδιο |
| `activities` | string | Όχι | Δραστηριότητες |
| `skills` | array | Όχι | ESCO δεξιότητες `[{code, name, type}]` |

**Response:**
```json
{
  "content": "# Ενότητα 1: Διοίκηση Ολικής Ποιότητας\n\n## Σκοπός...",
  "references": [...],
  "quality_score": {
    "academic_style": 8,
    "paragraph_quality": 9,
    "citations": 7,
    "structure": 8,
    "coverage": 9,
    "overall": 8,
    "notes": "Καλή ποιότητα..."
  },
  "outline": "JSON outline...",
  "page_count": 22
}
```

---

## 4. POST `/api/generate-stream` ⭐ (Κύριο endpoint παραγωγής)

Παραγωγή εκπαιδευτικού υλικού σε **streaming mode** (SSE). Αυτό χρησιμοποιεί το frontend.

**Χρήση:** Καλείται κάθε φορά που ο χρήστης πατάει "Δημιουργία" στο UI. Τα chunks εμφανίζονται real-time.

**Request Body schema:** Ίδιος με `/api/generate` (πεδία στον πίνακα της §5) **εκτός** του `multipass` που εδώ δεν χρησιμοποιείται. Παρακάτω συγκεκριμένα παραδείγματα για τις δύο λειτουργίες:

### Παράδειγμα — Standard mode (κενά `skills`, μόνο topic + parameters)

Single synthetic module από topic που δίνει ο χρήστης. Δεν υπάρχουν ESCO δεξιότητες — το `module.skills` παραμένει κενό array.

```json
{
  "module": {
    "number": 1,
    "title": "Διοίκηση Ολικής Ποιότητας",
    "hours": 10,
    "content": "",
    "activities": "",
    "skills": []
  },
  "use_research_hub": true,
  "experimental_mode": false,
  "user_instructions": "Έμφαση σε πρακτικά παραδείγματα από ελληνικές επιχειρήσεις.",
  "target_pages": 20,
  "learning_outcomes": "Ο εκπαιδευόμενος θα μπορεί να εφαρμόζει τις αρχές TQM σε οργανισμούς.",
  "keywords": "TQM, ISO 9001, συνεχής βελτίωση, Deming",
  "previous_content": "",
  "batch_number": 1,
  "total_batches": 3,
  "model_provider": "claude",
  "document_id": "doc-abc123"
}
```

### Παράδειγμα — ESCO mode (γεμάτο `module.skills` από εκπαιδευτικό σχέδιο)

Στο ESCO mode το `module.skills` περιέχει τη λίστα δεξιοτήτων από το `.docx` εκπαιδευτικού σχεδίου (παρμένη με `/api/parse-docx` ή με δικό σου parser). Το backend προσθέτει αυτόματα στο prompt οδηγία ότι **πρέπει να καλυφθούν όλες οι παραπάνω δεξιότητες**. **Το `.docx` δεν ανεβαίνει** στο endpoint — μόνο το JSON.

Προαιρετικά, αν γνωρίζεις το ESCO **occupation** που στοχεύει το πρόγραμμα κατάρτισης (program-level — ίδιο για όλες τις ενότητες), στείλε το ως `occupation` ώστε το μοντέλο να προσαρμόσει ορολογία και παραδείγματα στο συγκεκριμένο επάγγελμα.

```json
{
  "module": {
    "number": 1,
    "title": "Διοίκηση Ολικής Ποιότητας",
    "hours": 10,
    "content": "Αρχές TQM και εφαρμογή τους στις σύγχρονες επιχειρήσεις.",
    "activities": "Μελέτη περίπτωσης σε ελληνική επιχείρηση.",
    "skills": [
      { "code": "S1.1", "name": "διαχείριση ποιότητας", "type": "essential" },
      { "code": "S1.2", "name": "ηγεσία", "type": "essential" },
      { "code": "S1.3", "name": "ανάλυση δεδομένων", "type": "optional" }
    ]
  },
  "use_research_hub": true,
  "experimental_mode": false,
  "user_instructions": "",
  "target_pages": 20,
  "learning_outcomes": "",
  "keywords": "",
  "previous_content": "",
  "batch_number": 1,
  "total_batches": 1,
  "model_provider": "claude",
  "document_id": "doc-abc123",
  "occupation": {
    "code": "1213.1.1",
    "name": "διευθυντής/-ντρια ποιοτικού ελέγχου",
    "description": "Σχεδιάζει, οργανώνει και επιβλέπει το σύστημα διαχείρισης ποιότητας μιας επιχείρησης."
  }
}
```

**Παράδειγμα κλήσης με curl (ESCO mode):**
```bash
curl -N -X POST http://localhost:8000/api/generate-stream \
  -H "Content-Type: application/json" \
  -H "X-Session-ID: $(uuidgen)" \
  -d '{
    "module": {
      "number": 1,
      "title": "Διοίκηση Ολικής Ποιότητας",
      "hours": 10,
      "content": "Αρχές TQM και εφαρμογή τους...",
      "activities": "Μελέτη περίπτωσης...",
      "skills": [
        { "code": "S1.1", "name": "διαχείριση ποιότητας", "type": "essential" },
        { "code": "S1.2", "name": "ηγεσία", "type": "essential" }
      ]
    },
    "use_research_hub": true,
    "target_pages": 20,
    "batch_number": 1,
    "total_batches": 1,
    "model_provider": "claude",
    "document_id": "doc-abc123"
  }'
```

> **Διαφορά μεταξύ Standard / ESCO mode**: Η μόνη διαφορά στο request είναι το `module.skills` array. Το SSE flow και τα events είναι **ίδια** και στις δύο περιπτώσεις. Στο ESCO mode, μετά την έγκριση του υλικού καλείς το `POST /api/review` (§9) για ανάλυση κάλυψης δεξιοτήτων ESCO.

**Response:** Server-Sent Events (SSE) stream — `Content-Type: text/event-stream`

Κάθε event είναι `data: {JSON}\n\n`. Τύποι events:

### Event: `references` (πρώτο event)
```
data: {"type": "references", "data": [{"title": "...", "authors": [...], "year": 2020, "journal": "...", "doi": "..."}]}
```
Στέλνει τα academic papers που βρήκε ο Research Hub. Αυτά εμφανίζονται στον χρήστη.

### Event: `queue` (προαιρετικό, εμφανίζεται μόνο όταν ο rate limiter είναι κορεσμένος)
```
data: {"type": "queue", "position": 2, "estimated_wait": 18}
```
Ενημερώνει τον χρήστη ότι το αίτημα μπήκε σε ουρά πριν αρχίσει streaming.

### Event: `content` (πολλαπλά events)
```
data: {"type": "content", "text": "κομμάτι κειμένου..."}
```
Κάθε chunk κειμένου που παράγεται. Ο frontend τα ενώνει σε ολοκληρωμένο markdown.

### Event: `done` (τελευταίο event)
```
data: {"type": "done"}
```
Σηματοδοτεί το τέλος του stream.

**Σημαντικά features:**
- **Auto-continuation:** Αν η απάντηση κοπεί λόγω max tokens, το backend στέλνει αυτόματα continuation request
- **Length check:** Αν το περιεχόμενο είναι < 85% του στόχου σελίδων, στέλνεται continuation
- **Exponential backoff:** Αν το Anthropic API είναι overloaded (429/529), γίνεται αυτόματο retry με backoff (1s→2s→4s→8s→16s, max 5 attempts)
- **AbortController:** Ο frontend στέλνει `signal` για cancellation (αν ο χρήστης πατήσει ακύρωση)

### Revision mode (στοχευμένες αλλαγές σε υπάρχον draft)

Όταν ο χρήστης πατήσει «Αλλαγές» σε ένα ήδη παραγμένο τμήμα, το frontend ξανακαλεί το `/api/generate-stream` με `mode: "revision"` και το υπάρχον draft στο `current_draft`. Το backend στέλνει το draft στο μοντέλο ως assistant turn ακολουθούμενο από στοχευμένη οδηγία αναθεώρησης· το μοντέλο επιστρέφει **πλήρες αναθεωρημένο τμήμα** κρατώντας αυτολεξεί ό,τι δεν ζητήθηκε να αλλάξει.

**Διαφορές revision από generate:**
- Παρακάμπτεται ο Research Hub (το draft έχει ήδη references).
- Παρακάμπτεται το auto-continuation, οι MCQ counts και το final-section reminder.
- Δεν εφαρμόζονται continuation blocks (`previous_content` αγνοείται).
- Cache: η μόνη σταθερή πηγή cache hits είναι το system prompt· το draft αλλάζει μεταξύ διαδοχικών αναθεωρήσεων.

**Παράδειγμα request body:**
```json
{
  "module": { "number": 1, "title": "Διοίκηση Ολικής Ποιότητας", "hours": 10, "skills": [] },
  "mode": "revision",
  "current_draft": "# Ενότητα 1: Διοίκηση Ολικής Ποιότητας\n\n## Σκοπός...\n\n[ολόκληρο το προηγούμενο draft]",
  "user_instructions": "Στην υποενότητα 1.2, αντικατάστησε το παράδειγμα του Toyota με ένα από ελληνική επιχείρηση.",
  "target_pages": 20,
  "batch_number": 1,
  "total_batches": 1,
  "model_provider": "claude"
}
```

---

## 5. POST `/api/generate-summary`

Παραγωγή περίληψης (Περίληψη) μετά την έγκριση όλων των batches.

**Χρήση:** Καλείται αυτόματα όταν ο χρήστης πατάει "Έγκριση & Τέλος" στο τελευταίο batch.

**Request Body:**
```json
{
  "module_title": "Διοίκηση Ολικής Ποιότητας",
  "full_content": "Ολόκληρο το εγκεκριμένο περιεχόμενο (όλα τα batches ενωμένα)...",
  "model_provider": "claude"
}
```

| Πεδίο | Τύπος | Required | Default | Περιγραφή |
|-------|-------|----------|---------|-----------|
| `module_title` | string | Ναι | — | Τίτλος ενότητας |
| `full_content` | string | Ναι | — | Ολόκληρο το εγκεκριμένο περιεχόμενο |
| `model_provider` | `"claude"` \| `"openai"` | Όχι | `"claude"` | Επιλογή LLM provider |
| `document_id` | string | Όχι | "" | Optional correlation id (stable per uploaded doc/draft session) |

**Response:**
```json
{
  "summary": "Η παρούσα ενότητα εξετάζει τις βασικές αρχές της Διοίκησης Ολικής Ποιότητας..."
}
```

Η περίληψη είναι 500-800 λέξεις, σε ακαδημαϊκό ύφος, χωρίς νέες αναφορές.

---

## 6. POST `/api/generate-bibliography`

Παραγωγή πλήρων βιβλιογραφικών εγγραφών APA 7th από in-text citations.

**Χρήση:** Fallback — καλείται αυτόματα αν η παραγωγή κοπεί και λείπει η βιβλιογραφία.

**Request Body:**
```json
{
  "citations": ["Deming, 1986", "Porter & Kramer, 2011", "Κωνσταντινίδης, 2024"],
  "topic": "Διοίκηση Ολικής Ποιότητας",
  "model_provider": "claude"
}
```

| Πεδίο | Τύπος | Required | Default | Περιγραφή |
|-------|-------|----------|---------|-----------|
| `citations` | string[] | Ναι | — | Λίστα in-text citations |
| `topic` | string | Όχι | "" | Θέμα ενότητας (για context) |
| `model_provider` | `"claude"` \| `"openai"` | Όχι | `"claude"` | Επιλογή LLM provider |
| `document_id` | string | Όχι | "" | Optional correlation id (stable per uploaded doc/draft session) |

**Response:**
```json
{
  "bibliography": "Deming, W. E. (1986). *Out of the crisis*. MIT Press.\n\nPorter, M. E., & Kramer, M. R. (2011). Creating shared value. *Harvard Business Review*, *89*(1/2), 62–77."
}
```

---

## 7. POST `/api/review` ⭐ (Επίσημο endpoint για ESCO skill coverage)

Ανάλυση κάλυψης δεξιοτήτων ESCO στο παραγόμενο εκπαιδευτικό υλικό. Το backend φορτώνει αυτόματα τις ESCO περιγραφές, χτίζει το prompt και επιστρέφει structured JSON. **Αυτό είναι το επίσημο endpoint για ESCO skill coverage analysis — το χρησιμοποιεί και το web app.**

**Χρήση:** Καλείται αφού ο χρήστης εγκρίνει το περιεχόμενο, για να δει πόσες δεξιότητες ESCO καλύπτονται.

**Request Body:**
```json
{
  "module": {
    "number": 1,
    "title": "Διοίκηση Ολικής Ποιότητας",
    "skills": [
      { "code": "S1.1", "name": "διαχείριση ποιότητας", "type": "essential" },
      { "code": "S1.2", "name": "ηγεσία", "type": "optional" }
    ]
  },
  "content": "Ολόκληρο το εκπαιδευτικό υλικό...",
  "model_provider": "claude",
  "document_id": "doc-abc123",
  "occupation": {
    "code": "1213.1.1",
    "name": "διευθυντής/-ντρια ποιοτικού ελέγχου"
  }
}
```

| Πεδίο | Τύπος | Required | Default | Περιγραφή |
|-------|-------|----------|---------|-----------|
| `module` | object | Ναι | — | Στοιχεία ενότητας με τη λίστα ESCO skills |
| `content` | string | Ναι | — | Το εγκεκριμένο εκπαιδευτικό υλικό |
| `model_provider` | `"claude"` \| `"openai"` | Όχι | `"claude"` | Επιλογή LLM provider |
| `document_id` | string | Όχι | "" | Optional correlation id (stable per uploaded doc/draft session) |
| `occupation` | object \| null | Όχι | null | ESCO occupation που στοχεύει το πρόγραμμα κατάρτισης. Όταν παρέχεται, μπαίνει ως context block στο review prompt. Σχήμα: `{ code, name, description? }`. |

**Response:**
```json
{
  "moduleNumber": 1,
  "totalSkills": 10,
  "coveredFully": 7,
  "coveredPartially": 2,
  "missing": 1,
  "coveragePercentage": 80,
  "skillAnalysis": [
    {
      "skillCode": "S1.1",
      "skillName": "διαχείριση ποιότητας",
      "skillType": "essential",
      "coverageLevel": "full",
      "evidence": ["Η διαχείριση ποιότητας αναπτύχθηκε στην ενότητα 1.2..."],
      "contentSections": ["1.2 Αρχές Ποιότητας"],
      "notes": "Πλήρης κάλυψη με θεωρία και πρακτικά παραδείγματα."
    }
  ],
  "overallAssessment": "Το υλικό καλύπτει ικανοποιητικά τις περισσότερες δεξιότητες...",
  "recommendations": ["Να ενισχυθεί η κάλυψη της δεξιότητας X..."]
}
```

| Πεδίο | Τύπος | Περιγραφή |
|-------|-------|-----------|
| `totalSkills` | int | Σύνολο δεξιοτήτων |
| `coveredFully` | int | Πλήρως καλυμμένες |
| `coveredPartially` | int | Μερικώς καλυμμένες |
| `missing` | int | Ακάλυπτες |
| `coveragePercentage` | int | Ποσοστό κάλυψης (full=100%, partial=50%) |
| `skillAnalysis` | array | Αναλυτική αξιολόγηση ανά δεξιότητα |
| `overallAssessment` | string | Γενική αξιολόγηση |
| `recommendations` | string[] | Προτάσεις βελτίωσης |

---

## 8. POST `/api/claude/generate` (legacy)

Γενικός LLM proxy — provider-agnostic παρά το όνομα `/api/claude/*` (dispatch μέσω `model_provider`).

> ⚠️ **Deprecated για νέα integrations.** Το frontend δεν το καλεί πλέον για ESCO review — έχει μεταφερθεί στο `/api/review`. Διατηρείται μόνο για backward compatibility· νέα integrations να αποφεύγουν αυτό το endpoint.

**Request Body:**
```json
{
  "system": "System prompt κείμενο...",
  "messages": [
    { "role": "user", "content": "User prompt..." }
  ],
  "maxTokens": 16000,
  "model_provider": "claude"
}
```

| Πεδίο | Τύπος | Required | Default | Περιγραφή |
|-------|-------|----------|---------|-----------|
| `system` | string | Ναι | — | System prompt |
| `messages` | array | Ναι | — | Λίστα messages `[{role, content}]` |
| `maxTokens` | int | Όχι | 16000 | Μέγιστα output tokens |
| `model_provider` | `"claude"` \| `"openai"` | Όχι | `"claude"` | Επιλογή LLM provider |

**Response:**
```json
{
  "content": [
    { "type": "text", "text": "Απάντηση Claude..." }
  ],
  "usage": {
    "input_tokens": 5234,
    "output_tokens": 2100
  }
}
```

---

## 9. POST `/api/claude/generate-stream`

Γενικός Claude API streaming proxy.

**Χρήση:** Διαθέσιμο για streaming κλήσεις με custom system prompts. Δεν χρησιμοποιείται ενεργά (το frontend χρησιμοποιεί `/api/generate-stream`).

**Request Body:** Ίδιο με `/api/claude/generate`

**Response:** SSE stream — `Content-Type: text/event-stream`
```
data: {"text": "κομμάτι κειμένου..."}
data: {"text": "..."}
data: {"done": true}
```

---

## Ροή Χρήσης (Workflow)

### Βήμα 1: Upload & Parse
Ο χρήστης ανεβάζει DOCX αρχείο → το frontend το αναλύει (mammoth.js) → εξάγει ενότητες, δεξιότητες, ώρες.

### Βήμα 2: Παραγωγή Περιεχομένου
```
Frontend                              Backend
   |                                     |
   |-- POST /api/generate-stream ------->|
   |                                     |-- Research Hub: αναζήτηση papers
   |                                     |-- Claude API: streaming generation
   |<-- SSE: {type: "references"} -------|
   |<-- SSE: {type: "content"} ----------|  (πολλαπλά)
   |<-- SSE: {type: "content"} ----------|
   |<-- SSE: {type: "done"} -------------|
   |                                     |
```

### Βήμα 3: Έγκριση
Ο χρήστης βλέπει το υλικό, πατάει "Έγκριση" → αν υπάρχουν κι άλλα batches, πάει στο Βήμα 2.

### Βήμα 4: Τελική Έγκριση
```
Frontend                              Backend
   |                                     |
   |-- POST /api/generate-summary ------>|  (Περίληψη)
   |<-- {summary: "..."} ---------------|
   |                                     |
   |-- POST /api/review --------------->|  (ESCO κάλυψη)
   |<-- {coveragePercentage: 80} -------|
   |                                     |
```

### Βήμα 5: Export
Ο frontend δημιουργεί Word αρχείο (docx library) — δεν χρησιμοποιεί backend.

---

## Content Modes

### Standard Mode (`experimental_mode: false`)
- Χρησιμοποιεί Research Hub για πραγματικές ακαδημαϊκές αναφορές
- Το LLM λαμβάνει τις αναφορές στο prompt και τις ενσωματώνει
- Υψηλότερη αξιοπιστία βιβλιογραφίας

### Experimental Mode (`experimental_mode: true`)
- ΔΕΝ χρησιμοποιεί Research Hub
- Το LLM χρησιμοποιεί τη δική του γνώση
- Αυστηρό anti-hallucination system prompt
- Χρήσιμο αν ο Research Hub δεν βρίσκει αρκετές πηγές

---

## LLM Providers (Multi-provider Support)

Το backend υποστηρίζει δύο providers μέσω του `services/llm_service.py` (`LLMService` abstract base):

| `model_provider` | Μοντέλο | Context | Ρύθμιση |
|------------------|---------|---------|---------|
| `"claude"` (default) | `claude-opus-5` (`config.model_id`) | 200 K | `ANTHROPIC_API_KEY` |
| `"openai"` | `gpt-5.6-sol` (`config.openai_model_id`) | 1.1 M | `OPENAI_API_KEY` |

**Παρατηρήσεις:**
- **Ίδια system prompts** και ίδια business logic (continuation, retry, auto-continue, MCQ counts, ESCO review, summary, bibliography fallback) και στους δύο providers — η μόνη διαφορά είναι το raw streaming/completion call.
- **Prompt caching:** Στο Claude path γίνεται explicit μέσω `cache_control: ephemeral`. Στο OpenAI path εφαρμόζεται **auto-caching** όταν το prefix είναι ≥1024 tokens (δεν χρειάζεται flag).
- **Truncation detection:** Claude `stop_reason == "max_tokens"` ↔ OpenAI `finish_reason == "length"`. Και στις δύο περιπτώσεις ενεργοποιείται continuation.
- **Token usage normalization:** OpenAI `prompt_tokens` / `completion_tokens` mapping σε `input_tokens` / `output_tokens` για τον rate limiter.
- **Επιλογή provider:** Το frontend περνάει το `modelProvider` από το store (Landing Page toggle Claude Opus 5 / GPT-5.6-sol). Επιβιώνει στο localStorage και αποθηκεύεται στα `pendingTasks` (Εκκρεμότητες).

---

## Error Handling

Όλα τα endpoints επιστρέφουν structured errors:

```json
{
  "detail": "Περιγραφή σφάλματος"
}
```

| HTTP Status | Σημασία |
|-------------|---------|
| 200 | Επιτυχία |
| 422 | Validation error (λάθος request body) |
| 500 | Internal error (βλ. `detail` για λεπτομέρειες) |

Το backend κάνει αυτόματα retry σε Anthropic API errors:
- **429** (Rate limit): Retry με exponential backoff
- **529** (Overloaded): Retry με exponential backoff
- **500/502/503/504**: Retry με exponential backoff
- Max 5 attempts, backoff: 1s → 2s → 4s → 8s → 16s (+ jitter)

---

## Prompt Caching

Όλες οι Claude API κλήσεις χρησιμοποιούν **prompt caching** στο system prompt:
- Εξοικονόμηση 90% στα cached input tokens
- TTL: ~5 λεπτά (ανανεώνεται με κάθε κλήση)
- Δεν επηρεάζει την ποιότητα — ίδιο output, ίδια tokens
- Ιδιαίτερα χρήσιμο για πολλαπλά batches της ίδιας ενότητας

---

## Environment Variables (Backend)

| Variable | Required | Default | Περιγραφή |
|----------|----------|---------|-----------|
| `ANTHROPIC_API_KEY` | Ναι (για `model_provider="claude"`) | — | Anthropic API key |
| `OPENAI_API_KEY` | Ναι (για `model_provider="openai"`) | — | OpenAI API key (gpt-5.6-sol) |
| `ANTHROPIC_TIER` | Όχι | `2` | Anthropic tier (2/3/4) — επηρεάζει concurrency caps |
| `HOST` | Όχι | `0.0.0.0` | Host binding |
| `PORT` | Όχι | `8000` | Port |
| `RUST_RESEARCH_HUB_URL` | Όχι | `http://localhost:8091` | URL του Rust Research Hub |

> Σημείωση: τουλάχιστον το ένα από τα δύο API keys πρέπει να υπάρχει· αν λείπει το αντίστοιχο για το provider που ζητείται, η παραγωγή θα αποτύχει με 500.

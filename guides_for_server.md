# Server Deployment Guide — JWT Auth, Analytics, Admin Dashboard & Roles Update

## 🚀 Latest Update — Prompt refinements (hours=0, revision citations, doctoral theses) (commit `3f7d7ff`, 2026-05-21)

Τρεις tweaks στα prompts του `llm_service.py`. **Καμία αλλαγή στο API contract** — μόνο στη συμπεριφορά του μοντέλου.

1. **`hours=0` cleanup**: το `_format_module` δεν εμφανίζει πλέον τη γραμμή «Διάρκεια: 0 ώρες» όταν δεν υπάρχει πραγματική τιμή. Καθαρότερο prompt σε standalone revision flows.

2. **Citation-aware revisions**: το revision prompt επιτρέπει στο LLM να προσθέσει νέες ακαδημαϊκές αναφορές αν αυτές εμπλουτίζουν τη ζητούμενη αλλαγή — **υπό τον όρο** ότι κάθε νέο in-text citation συνοδεύεται από αντίστοιχη πλήρη εγγραφή στη Βιβλιογραφία (APA 7th, αλφαβητική σειρά). Πριν, το prompt ήταν αυστηρό «κράτα ίδια τη βιβλιογραφία» που οδηγούσε σε ορφανές citations ή άρνηση εμπλουτισμού.

3. **Allow doctoral dissertations**: αφαιρέθηκε ο γενικός αποκλεισμός των διδακτορικών διατριβών από όλους τους prompts (main system + bibliography section + revision + bibliography fallback). Πτυχιακές και μεταπτυχιακές παραμένουν απαγορευμένες. Συμφωνία με τον υπάρχοντα `research_service._filter_theses` που ήδη επέτρεπε διδακτορικές.

**Files to rsync:**

```bash
SERVER=ykaragiorgos@devai.apopsi.gr
REMOTE=/home/ykaragiorgos/ai_content_creator

rsync -avzR \
  backend_py/src/edu_backend/services/llm_service.py \
  $SERVER:$REMOTE/
```

> Μόνο **ένα αρχείο**. Καμία αλλαγή σε deps, καμία αλλαγή σε frontend, καμία αλλαγή σε API schema. Το `postman_collection.json` που άλλαξε στο ίδιο commit είναι testing aid (τοπικό· δεν deploy-αρεται στο server).

### Post-rsync steps on the server

1. **Restart API container** (να φορτώσει τα νέα prompts):
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && docker compose restart deploy-api-1'
   ```
2. Frontend rebuild **ΔΕΝ** χρειάζεται.

### Smoke checks

- `GET /api/health` → 200.
- **Behavioral spot-check #1 (hours=0)**: τρέξε `/api/generate-stream` με `mode=revision` και `module.hours=0`. Δεν θα μπορείς να το επιβεβαιώσεις άμεσα από το output (το prompt δεν είναι ορατό), αλλά αν προσέχεις το prompt cache hit rate στο `/api/rate-limit/status` πριν και μετά, θα δεις σταθερό cache prefix (δηλαδή το prompt δεν αλλάζει για τιμές `hours=0` μεταξύ requests).
- **Behavioral spot-check #2 (revision citation)**: τρέξε ένα revision με instruction τύπου «εμπλούτισε την υποενότητα 1.2 με μία επιπλέον ακαδημαϊκή πηγή για συγκεκριμένο θέμα». Έλεγξε ότι:
  - Το αναθεωρημένο κείμενο περιέχει νέο `(Επώνυμο, Έτος)` σε σχέση με το αρχικό draft.
  - Η ενότητα `## Βιβλιογραφία` έχει αντίστοιχη πλήρη APA εγγραφή για την ίδια πηγή.
- **Behavioral spot-check #3 (doctoral theses)**: εμπλούτισε ή ξεκίνα generation στη θεματική όπου το ESCO/CrossRef συνήθως επιστρέφει διδακτορικά (π.χ. εξειδικευμένα τεχνικά πεδία). Αν το παραγόμενο υλικό περιλαμβάνει αναφορά σε `[Doctoral dissertation]` ή ισοδύναμο, ΕΠΙΤΡΕΠΕΤΑΙ τώρα.

### Rollback

```bash
ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && git checkout b3e3ebd -- \
  backend_py/src/edu_backend/services/llm_service.py \
  && docker compose restart deploy-api-1'
```

> `b3e3ebd` είναι το προηγούμενο commit (αφαίρεση `/api/esco/skills`).

---

## 🚀 Latest Update — Remove unused `/api/esco/skills` + `/api/esco/search` endpoints (commit `b3e3ebd`, 2026-05-21)

Δύο utility GET endpoints αφαιρέθηκαν επειδή είχαν μείνει dead code μετά το ESCO review consolidation:

- `GET /api/esco/skills` — παλιά καλούνταν από το frontend για να φέρει ESCO περιγραφές, πριν μεταφερθεί η λογική εσωτερικά στο `/api/review`
- `GET /api/esco/search` — παρόμοιο utility που δεν είχε ποτέ frontend consumer

**Καμία λειτουργική απώλεια**: η ESCO lookup λογική παραμένει διαθέσιμη εσωτερικά μέσω `esco_service.lookup()` και χρησιμοποιείται από `/api/parse-docx` (URI enrichment) και `/api/review` (πρόσθεση descriptions στο prompt).

**Files to rsync:**

```bash
SERVER=ykaragiorgos@devai.apopsi.gr
REMOTE=/home/ykaragiorgos/ai_content_creator

# Backend: αφαίρεση + main.py update + (καμία αλλαγή σε deps)
rsync -avzR \
  backend_py/src/edu_backend/main.py \
  web/src/services/api.ts \
  $SERVER:$REMOTE/

# Διαγραφή του router file στο server
ssh $SERVER 'rm -f /home/ykaragiorgos/ai_content_creator/backend_py/src/edu_backend/routers/esco.py'
```

> Δεν χρειάζεται `pip install`. Frontend rebuild χρειάζεται για το νέο TS bundle.

### Post-rsync steps on the server

1. **Restart API container**:
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && docker compose restart deploy-api-1'
   ```
2. **Rebuild frontend container** (πιθανό dead code elimination στο bundle):
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && docker compose build deploy-frontend-1 && docker compose up -d deploy-frontend-1'
   ```

### Smoke checks

- **Confirm removal:** πρέπει να επιστρέφει 404
  ```bash
  curl -sI https://devai.apopsi.gr/e-learning/api/esco/skills?names=test | head -1
  ```
  Αναμενόμενο: `HTTP/2 404`.

- **Confirm other endpoints still alive:**
  ```bash
  curl -fsS https://devai.apopsi.gr/e-learning/api/health | jq '.status'
  ```
  Αναμενόμενο: `"ok"`.

### Rollback

```bash
ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && git checkout 40fcbed -- \
  backend_py/src/edu_backend/main.py \
  backend_py/src/edu_backend/routers/esco.py \
  web/src/services/api.ts \
  && docker compose restart deploy-api-1 \
  && docker compose build deploy-frontend-1 \
  && docker compose up -d deploy-frontend-1'
```

> `40fcbed` είναι το προηγούμενο commit (ESCO URI resolution).

---

## 🚀 Latest Update — ESCO URI resolution (commit `40fcbed`, 2026-05-21)

Το `skills[*].code` πεδίο στα responses του `/api/parse-docx` τώρα γεμίζει με τα **επίσημα ESCO URIs** (`http://data.europa.eu/esco/skill/<uuid>`) όταν το όνομα του skill ταιριάζει με entry του ελληνικού ESCO 1.2.1 dataset. Αν δεν βρεθεί, χρησιμοποιείται **synthetic fallback** όπως πριν.

Επίσης το `/api/esco/skills` lookup endpoint επιστρέφει τώρα και `uri` field στο response.

**Παράδειγμα:**
```json
"skills": [
  { "code": "http://data.europa.eu/esco/skill/0005c151-5b5a-4a66-8aac-60e734beb1ab",
    "name": "διοίκηση του μουσικού προσωπικού", "type": "essential" },
  { "code": "αγν-σκι", "name": "άγνωστο skill", "type": "optional" }
]
```

**Files to rsync:**

```bash
SERVER=ykaragiorgos@devai.apopsi.gr
REMOTE=/home/ykaragiorgos/ai_content_creator

rsync -avzR \
  backend_py/scripts/preprocess_esco.py \
  backend_py/src/edu_backend/data/skills_compact.json \
  backend_py/src/edu_backend/services/esco_service.py \
  backend_py/src/edu_backend/routers/docx.py \
  web/src/services/api.ts \
  $SERVER:$REMOTE/
```

> **Προσοχή:** το `skills_compact.json` είναι **~6.8 MB** (από ~1.6 MB πριν — λόγω των URIs). Αν τρέχεις rsync σε αργή σύνδεση, μπορεί να πάρει λίγα δευτερόλεπτα παραπάνω.
>
> Το `preprocess_esco.py` είναι **νέο** αρχείο. Δεν τρέχει σαν deps — χρησιμοποιείται μόνο για re-generation του JSON σε νέα ESCO έκδοση.
>
> Δεν χρειάζεται `pip install` (δεν προστέθηκαν deps).

### Post-rsync steps on the server

1. **Restart API container** (να φορτώσει το νέο JSON + νέο service code):
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && docker compose restart deploy-api-1'
   ```
2. **Rebuild frontend container** (νέο TS interface):
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && docker compose build deploy-frontend-1 && docker compose up -d deploy-frontend-1'
   ```

### Smoke checks

- **Confirm new ESCO JSON loaded** (κοιτάει για 13.926 skills στο startup log):
  ```bash
  ssh $SERVER 'docker compose logs --tail=50 deploy-api-1 | grep "Loaded.*ESCO"'
  ```
  Αναμενόμενο: `Loaded 13926 ESCO skills`.

- **URI lookup μέσω `/api/esco/skills`**:
  ```bash
  curl -fsS "https://devai.apopsi.gr/e-learning/api/esco/skills?names=διοίκηση%20του%20μουσικού%20προσωπικού" | jq '.[0]'
  ```
  Αναμενόμενο: αντικείμενο με `uri` field τύπου `http://data.europa.eu/esco/skill/<uuid>`.

- **End-to-end με .docx** μέσω `/api/parse-docx`: τα skill codes πρέπει να είναι URIs για όσα ονόματα ταιριάζουν με ESCO. Στα logs:
  ```bash
  ssh $SERVER 'docker compose logs --tail=10 deploy-api-1 | grep parse-docx'
  ```
  Αναμενόμενο: `esco_uris_resolved=N` (N > 0 για docx με ESCO-aligned skill names).

### Rollback

```bash
ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && git checkout f078950 -- \
  backend_py/src/edu_backend/data/skills_compact.json \
  backend_py/src/edu_backend/services/esco_service.py \
  backend_py/src/edu_backend/routers/docx.py \
  web/src/services/api.ts \
  && rm -f backend_py/scripts/preprocess_esco.py \
  && docker compose restart deploy-api-1 \
  && docker compose build deploy-frontend-1 \
  && docker compose up -d deploy-frontend-1'
```

> `f078950` είναι το προηγούμενο commit (POST /api/docx-to-markdown helper).

---

## 🚀 Latest Update — New `/api/docx-to-markdown` helper endpoint (commit `f078950`, 2026-05-21)

Νέο helper endpoint για consumers που θέλουν να ξεκινήσουν τη ροή revision από `.docx`.

**Σκοπός:** `.docx → markdown` conversion server-side, ώστε ο .NET dev να μη συντηρεί δικό του parser. Το markdown επιστρέφεται έτοιμο για χρήση ως `current_draft` στο `/api/generate-stream` με `mode=revision`.

**Διαφορά με το υπάρχον `/api/parse-docx`:**

| Endpoint | Επιστρέφει | Use case |
|----------|-----------|----------|
| `/api/parse-docx` (υπάρχει) | Structured JSON (`modules`, `skills`, `totalHours`) | ESCO mode — νέο εκπαιδευτικό σχέδιο |
| **`/api/docx-to-markdown` (νέο)** | Markdown string του πλήρους περιεχομένου | Revision mode — υπάρχον υλικό σε `.docx` |

**Files to rsync:**

```bash
SERVER=ykaragiorgos@devai.apopsi.gr
REMOTE=/home/ykaragiorgos/ai_content_creator

rsync -avzR \
  backend_py/pyproject.toml \
  backend_py/src/edu_backend/routers/docx.py \
  $SERVER:$REMOTE/
```

> Δύο αρχεία: το `pyproject.toml` (νέες deps: `mammoth`, `markdownify`) και ο router. Καμία αλλαγή σε frontend.

### Post-rsync steps on the server

1. **Install new deps** (`mammoth`, `markdownify` — pure Python, no system deps):
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator/backend_py && pip install -e .'
   ```
   ή rebuild του container:
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && docker compose build deploy-api-1'
   ```
2. **Restart API container**:
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && docker compose up -d deploy-api-1'
   ```
3. **Frontend rebuild ΔΕΝ χρειάζεται** — backend-only.

### Smoke checks

- `GET /api/health` → 200.
- **Verify route exists**:
  ```bash
  ssh $SERVER 'docker exec deploy-api-1 python -c "from edu_backend.main import app; [print(r.methods, r.path) for r in app.routes if hasattr(r, \"methods\") and \"docx\" in r.path]"'
  ```
  Αναμενόμενο: `{'POST'} /api/parse-docx` και `{'POST'} /api/docx-to-markdown`.
- **End-to-end test** με πραγματικό `.docx`:
  ```bash
  curl -fsS -X POST https://devai.apopsi.gr/e-learning/api/docx-to-markdown \
    -H "X-Session-ID: $(uuidgen)" \
    -F "file=@my_material.docx" | jq '{document_id, chars: (.markdown | length)}'
  ```
  Αναμενόμενο: `{ "document_id": "doc-...", "chars": <int> }`.
- **Log entry** στο stdout του API:
  ```bash
  ssh $SERVER 'docker compose logs --tail=10 deploy-api-1 | grep docx-to-markdown'
  ```
  Αναμενόμενο: `[api] endpoint=/api/docx-to-markdown doc=doc-... chars=N warnings=0`.

### Rollback

```bash
ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && git checkout cf652f0 -- \
  backend_py/pyproject.toml \
  backend_py/src/edu_backend/routers/docx.py \
  && cd backend_py && pip install -e . \
  && cd .. && docker compose restart deploy-api-1'
```

> `cf652f0` είναι το προηγούμενο commit (server-generated document_id from parse-docx).

---

## 🚀 Latest Update — Server-generated document_id from /api/parse-docx (commit `cf652f0`, 2026-05-20)

Μια αλλαγή στο `pre-jwt` branch:

**`POST /api/parse-docx` τώρα επιστρέφει `document_id`** στο response (μορφή `doc-<uuid4>`). Οι API consumers που ξεκινούν τη ροή τους από `.docx` upload παίρνουν το correlation id **έτοιμο** από τον server και το επαναχρησιμοποιούν στα επόμενα `/api/generate-stream`, `/api/review`, `/api/generate-summary`, `/api/generate-bibliography` calls. Όσοι παρακάμπτουν το upload (Standard mode) συνεχίζουν να γεννούν δικό τους client-side.

Backward compatible (purely additive response field). Καμία αλλαγή σε άλλα endpoints.

**Files to rsync (commit `cf652f0`):**

```bash
SERVER=ykaragiorgos@devai.apopsi.gr
REMOTE=/home/ykaragiorgos/ai_content_creator

rsync -avzR \
  backend_py/src/edu_backend/routers/docx.py \
  $SERVER:$REMOTE/
```

> Μόνο ένα code file αλλάζει — ο router. Καμία αλλαγή σε deps, καμία αλλαγή σε frontend.

### One-liner alternative

```bash
SERVER=ykaragiorgos@devai.apopsi.gr
REMOTE=/home/ykaragiorgos/ai_content_creator

git show --name-only --pretty=format: cf652f0 | grep -v '^$' | \
  grep -E '^(backend_py|web)/' | \
  rsync -avzR --files-from=- ./ $SERVER:$REMOTE/
```

### Post-rsync steps on the server

1. **Καμία αλλαγή σε deps** — δεν χρειάζεται `pip install` ούτε `npm install`.
2. **Restart API container** (φορτώνει τον νέο router):
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && docker compose restart deploy-api-1'
   ```
3. **Frontend rebuild ΔΕΝ χρειάζεται** — backend-only αλλαγή.

### Smoke checks

- `GET /api/health` → 200.
- **Verify `document_id` στο response** με πραγματικό `.docx`:
  ```bash
  curl -fsS -X POST https://devai.apopsi.gr/e-learning/api/parse-docx \
    -F "file=@educational_design.docx" | jq '{ document_id, documentTitle, modules: (.modules | length) }'
  ```
  Αναμενόμενο output:
  ```json
  {
    "document_id": "doc-7f3a9c12-8b4e-4d2f-9a1c-5e6f8d3b1a2c",
    "documentTitle": "...",
    "modules": 10
  }
  ```
- **Log entry** στο stdout του API:
  ```bash
  ssh $SERVER 'docker compose logs --tail=10 deploy-api-1 | grep parse-docx'
  ```
  Αναμενόμενο: `[api] endpoint=/api/parse-docx doc=doc-7f3a9c12-8b4e-4d2f modules=10 title=...`

### Rollback

```bash
ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && git checkout 4d9cc54 -- \
  backend_py/src/edu_backend/routers/docx.py \
  && docker compose restart deploy-api-1'
```

> `4d9cc54` είναι το προηγούμενο commit (ESCO review consolidation + document_id + occupation).

---

## 🚀 Latest Update — ESCO review consolidation + document_id + occupation (commit `4d9cc54`, 2026-05-20)

Τρεις αλλαγές στο `pre-jwt` branch:

1. **ESCO review consolidation** — το frontend πλέον καλεί το δεδικευμένο `POST /api/review` (αντί για `/api/claude/generate` με inline prompt). Το backend `REVIEW_PROMPT` αναβαθμίστηκε με πλήρες JSON schema (skillCode/skillName/skillType/evidence/contentSections/notes) και ρητή «JSON only» οδηγία ώστε να ταιριάζει ποιοτικά με το παλιό frontend prompt.
2. **`document_id` correlation field** — optional body field σε όλα τα POST endpoints (`/api/generate-stream`, `/api/review`, `/api/generate-summary`, `/api/generate-bibliography`). Εμφανίζεται στα backend logs για traceability/audit.
3. **`occupation` field** — optional body field σε `/api/generate-stream` και `/api/review`. Program-level ESCO occupation context (`{ code, name, description? }`). Όταν παρέχεται, το backend το προσθέτει ως context line στο prompt ώστε το μοντέλο να προσαρμόσει ορολογία στο συγκεκριμένο επάγγελμα.

**Όλες οι αλλαγές είναι backward compatible** — υπάρχοντες callers δουλεύουν χωρίς αλλαγή (defaults: `document_id=""`, `occupation=null`).

**Files to rsync (commit `4d9cc54`):**

```bash
SERVER=ykaragiorgos@devai.apopsi.gr
REMOTE=/home/ykaragiorgos/ai_content_creator

rsync -avzR \
  backend_py/src/edu_backend/prompts/system_prompt.py \
  backend_py/src/edu_backend/routers/generate.py \
  backend_py/src/edu_backend/services/llm_service.py \
  web/src/components/ContentGenerator.tsx \
  web/src/services/api.ts \
  web/src/services/claudeService.ts \
  $SERVER:$REMOTE/
```

> Όλα modifications — κανένα νέο αρχείο. Καμία αλλαγή σε `pyproject.toml` / `package.json` (μηδέν νέα dependencies).

### Πιο εύκολο: rsync το commit `4d9cc54` σε ένα command

```bash
SERVER=ykaragiorgos@devai.apopsi.gr
REMOTE=/home/ykaragiorgos/ai_content_creator

# Παίρνει τα code files του commit (φιλτράρει τα docs που μένουν local)
git show --name-only --pretty=format: 4d9cc54 | grep -v '^$' | \
  grep -E '^(backend_py|web)/' | \
  rsync -avzR --files-from=- ./ $SERVER:$REMOTE/
```

### Post-rsync steps on the server

1. **Καμία αλλαγή σε deps** — δεν χρειάζεται `pip install` ούτε `npm install`.
2. **Restart API container** (για να φορτώσει τα νέα `system_prompt.py` + `generate.py` + `llm_service.py`):
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && docker compose restart deploy-api-1'
   ```
3. **Rebuild frontend container** (Vite build χρειάζεται για τα 3 αλλαγμένα `.tsx`/`.ts`):
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && docker compose build deploy-frontend-1 && docker compose up -d deploy-frontend-1'
   ```

### Smoke checks

- `GET /api/health` → 200, ίδιο `model: claude-opus-4-6`.
- **Logging με correlation ids** — μετά από οποιοδήποτε API call:
  ```bash
  ssh $SERVER 'docker compose logs --tail=20 deploy-api-1 | grep "^\[api\]"'
  ```
  Αναμενόμενο: γραμμές τύπου `[api] endpoint=/api/generate-stream user=abc12345 doc=doc-xyz789 mode=generate batch=1/1 module=1 provider=claude occupation=-`
- **ESCO review parity** — στο UI: ξεκίνα ESCO mode, παράγαγε μία ενότητα, πάτησε Approve & Finish → trigger ESCO review. Στο DevTools→Network επιβεβαίωσε ότι η κλήση πάει σε `POST /api/review` (όχι `/api/claude/generate`). Το response πρέπει να έχει `skillAnalysis[*].evidence` (array με quotes), `contentSections`, `notes`.
- **`occupation` injection** — από Postman ή curl με ESCO body που περιέχει `"occupation": { "code": "7223.4", "name": "..." }`, το παραγόμενο υλικό πρέπει να αναφέρεται στο επάγγελμα-στόχο. Στα logs θα δεις `occupation=7223.4`.
- **Backward compatibility** — οποιαδήποτε κλήση χωρίς `document_id` ή `occupation` πρέπει να δουλεύει κανονικά (defaults).

### Rollback

```bash
ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && git checkout 70a499c -- \
  backend_py/src/edu_backend/prompts/system_prompt.py \
  backend_py/src/edu_backend/routers/generate.py \
  backend_py/src/edu_backend/services/llm_service.py \
  web/src/components/ContentGenerator.tsx \
  web/src/services/api.ts \
  web/src/services/claudeService.ts \
  && docker compose restart deploy-api-1 \
  && docker compose build deploy-frontend-1 \
  && docker compose up -d deploy-frontend-1'
```

> `70a499c` είναι το προηγούμενο commit (revision-mode + upload-docx workflow).

---

## 🚀 Latest Update — Revision-mode + Upload-docx workflow (commit `70a499c`, 2026-05-15)

Δύο νέα features στο `pre-jwt` branch:

1. **Revision-mode στις «Αλλαγές»** — όταν ο χρήστης ζητάει αλλαγή σε ένα παραγμένο τμήμα, το μοντέλο πλέον βλέπει το draft ως assistant turn και αλλάζει ΜΟΝΟ τα ζητούμενα σημεία (κρατάει τα υπόλοιπα αυτολεξεί). Πριν, ξαναέγραφε όλο το τμήμα.
2. **«Επεξεργασία υλικού» tab** στο Landing — upload οποιουδήποτε `.docx`, parse σε markdown με mammoth, και iterative αλλαγές με την παραπάνω revision-mode capability.

**Files to rsync (commit `70a499c`):**

```bash
SERVER=ykaragiorgos@devai.apopsi.gr
REMOTE=/home/ykaragiorgos/ai_content_creator

# Όλα τα αρχεία του commit, με preserved paths (-R)
rsync -avzR \
  backend_py/src/edu_backend/routers/generate.py \
  backend_py/src/edu_backend/services/llm_service.py \
  web/src/components/ContentGenerator.tsx \
  web/src/components/LandingPage.tsx \
  web/src/services/api.ts \
  web/src/services/claudeService.ts \
  web/src/store/useStore.ts \
  web/src/utils/docxToMarkdown.ts \
  $SERVER:$REMOTE/
```

> Το `-R` διατηρεί το full path. Το `web/src/utils/docxToMarkdown.ts` είναι **νέο** αρχείο, τα υπόλοιπα είναι modifications. Καμία αλλαγή σε `package.json` / `pyproject.toml` — το `mammoth` (frontend) είναι ήδη installed.

### Πιο εύκολο: rsync το commit `70a499c` σε ένα command

Αν θέλεις να αφήσεις το git να σου πει ακριβώς τι άλλαξε σε αυτό το commit:

```bash
SERVER=ykaragiorgos@devai.apopsi.gr
REMOTE=/home/ykaragiorgos/ai_content_creator

# Παίρνει τη λίστα αρχείων του commit (modified + added) και τα στέλνει.
git show --name-only --pretty=format: 70a499c | grep -v '^$' | \
  rsync -avzR --files-from=- ./ $SERVER:$REMOTE/
```

### Post-rsync steps on the server

1. **Καμία αλλαγή σε deps** — δεν χρειάζεται `pip install` ούτε `npm install`.
2. **Restart API container** (μόνο για να φορτώσει τα νέα `llm_service.py` + `generate.py`):
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && docker compose restart deploy-api-1'
   ```
3. **Rebuild frontend container** (Vite build χρειάζεται για τα `.tsx` αρχεία):
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && docker compose build deploy-frontend-1 && docker compose up -d deploy-frontend-1'
   ```

### Smoke checks

- `GET /api/health` → 200, ίδιο `model: claude-opus-4-6`.
- Στο UI: tab bar στο Landing πρέπει να δείχνει **3 tabs** (Standard / ESCO Integrated / **Επεξεργασία υλικού**).
- Παραγωγή τμήματος → πάτησε «Αλλαγές» με συγκεκριμένη οδηγία → το αναθεωρημένο κείμενο πρέπει να έχει αλλάξει **μόνο** εκεί που ζητήθηκε. Συγκριτική επιβεβαίωση: σώσε το αρχικό content, κάνε diff μετά την αναθεώρηση.
- Upload-docx flow: tab «Επεξεργασία υλικού» → ανέβασε `.docx` (κατά προτίμηση παραγμένο από το app) → πρέπει να σε πάει στο ContentGenerator με pending batch το ίδιο το ανεβασμένο κείμενο.

### Rollback

```bash
ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator && git checkout c65e11d -- \
  backend_py/src/edu_backend/routers/generate.py \
  backend_py/src/edu_backend/services/llm_service.py \
  web/src/components/ContentGenerator.tsx \
  web/src/components/LandingPage.tsx \
  web/src/services/api.ts \
  web/src/services/claudeService.ts \
  web/src/store/useStore.ts \
  && rm -f web/src/utils/docxToMarkdown.ts \
  && docker compose restart deploy-api-1 \
  && docker compose build deploy-frontend-1 \
  && docker compose up -d deploy-frontend-1'
```

> `c65e11d` είναι το προηγούμενο commit (parse-docx endpoint).

---

## 🚀 Latest Update — `POST /api/parse-docx` endpoint (2026-05-08)

Νέο backend endpoint που δέχεται `.docx` upload και επιστρέφει το ίδιο JSON shape που παράγει το frontend (mammoth.js parser). Επιτρέπει σε API consumers να χρησιμοποιούν τη ροή ESCO **χωρίς το React UI**.

**Files to rsync:**

```bash
SERVER=ykaragiorgos@devai.apopsi.gr
REMOTE=/home/ykaragiorgos/ai_content_creator

rsync -avz \
  backend_py/pyproject.toml \
  backend_py/src/edu_backend/main.py \
  backend_py/src/edu_backend/routers/docx.py \
  backend_py/src/edu_backend/services/docx_parser.py \
  $SERVER:$REMOTE/
```

> Αν τρέχεις `rsync -R --relative`, ίδιο file list. Το `routers/docx.py` και το `services/docx_parser.py` είναι **νέα** αρχεία — βεβαιώσου ότι τα directories `backend_py/src/edu_backend/routers/` και `backend_py/src/edu_backend/services/` υπάρχουν στον server (υπάρχουν ήδη).

### Post-rsync steps on the server

1. **Backend deps** — εγκατάσταση `python-docx` και `python-multipart`:
   ```bash
   ssh $SERVER 'cd /home/ykaragiorgos/ai_content_creator/backend_py && pip install -e .'
   ```
   (ή rebuild του API container: `docker compose -f deploy/docker-compose.yml build api && docker compose up -d api`)

2. **Restart** του API container ώστε να φορτώσει ο νέος router.

### Smoke checks

- `GET /api/health` → 200.
- Λίστα routes:
  ```bash
  ssh $SERVER 'docker exec deploy-api-1 python -c "from edu_backend.main import app; [print(r.methods, r.path) for r in app.routes if hasattr(r, \"methods\")]"' | grep parse-docx
  ```
  Αναμενόμενο: `{'POST'} /api/parse-docx`
- E2E test με ένα πραγματικό `.docx` (από αυτά που χρησιμοποιούσες στο UI):
  ```bash
  curl -X POST https://devai.apopsi.gr/e-learning/api/parse-docx \
    -F "file=@educational_design.docx" | jq '.modules | length'
  ```
  Πρέπει να επιστρέψει τον ίδιο αριθμό modules με το UI.

> **Frontend:** δεν χρειάζεται rebuild — αυτό είναι backend-only feature.

---

**Server:** devai.apopsi.gr (172.104.146.160)
**User:** ykaragiorgos
**App path:** /home/ykaragiorgos/ai_content_creator
**Deploy method:** Docker Compose
**Date:** 2026-03-20 (includes changes from 2026-03-17, 2026-03-18, 2026-03-20)

> **Σημείωση:** Ο server τρέχει τον κώδικα μέχρι commit `c2ecd51` (2026-03-04).
> Αυτό το deploy θα ενημερώσει σε commit `26121bb` που περιλαμβάνει:
> - Όλα τα rate limiting / page accuracy / word export fixes (ήδη deployed)
> - JWT authentication + SQLite analytics + admin dashboard (commit `d1b83f0`)
> - Νέοι ρόλοι + standalone admin password (commit `26121bb`)

## Νέες Αλλαγές (μετά το τελευταίο deploy)

### Commit `d1b83f0` — JWT Auth, Analytics, Admin Dashboard
| Container | File | Change |
|-----------|------|--------|
| deploy-api-1 | `auth.py` | **NEW** — JWT decode/verify, `get_current_user`, `get_optional_user`, `require_role`, `require_admin` |
| deploy-api-1 | `database.py` | **NEW** — SQLite analytics (users, sessions, events tables) |
| deploy-api-1 | `routers/analytics.py` | **NEW** — Admin-only endpoints: summary, users, user detail, CSV export |
| deploy-api-1 | `routers/export.py` | **NEW** — Backend Word export (python-docx) |
| deploy-api-1 | `config.py` | JWT settings (`jwt_secret`, `jwt_algorithm`), `platform_api_url`, `analytics_db_path` |
| deploy-api-1 | `main.py` | Analytics init, Bearer token in CORS headers |
| deploy-frontend-1 | `App.tsx` | Admin dashboard route (`?view=admin`), user name/role display |
| deploy-frontend-1 | `AdminDashboard.tsx` | **NEW** — Dashboard UI (cards, chart, user table, detail view) |
| deploy-frontend-1 | `hooks/useAuth.ts` | **NEW** — Token extraction from URL, sessionStorage, role parsing |
| deploy-frontend-1 | `api.ts` | Bearer token headers, analytics API calls, export API |
| deploy-frontend-1 | `store/useStore.ts` | Auth state (token, userId, userName, userRole), `setAuth`/`clearAuth` |

### Commit `26121bb` — Roles Update + Standalone Admin Password
| Container | File | Change |
|-----------|------|--------|
| deploy-api-1 | `auth.py` | Roles: `internal_employee`/`admin`/`external_partner` (was student/trainer/admin). New `require_admin()` — accepts JWT admin OR `admin_password` query param |
| deploy-api-1 | `config.py` | Added `admin_password: str` setting |
| deploy-api-1 | `routers/analytics.py` | Uses `require_admin` instead of `require_role("admin")` |
| deploy-frontend-1 | `App.tsx` | Dashboard accessible with `admin_password` in URL (no JWT needed) |
| deploy-frontend-1 | `api.ts` | Analytics calls pass `admin_password` query param |

### Νέα Dependencies
| Container | Package | Purpose |
|-----------|---------|---------|
| deploy-api-1 | `PyJWT` | JWT token verification |
| deploy-api-1 | `aiosqlite` | Async SQLite for analytics |
| deploy-api-1 | `python-docx` | Backend Word export |
| deploy-frontend-1 | `recharts` | Analytics dashboard charts |
| deploy-frontend-1 | `lucide-react` | Dashboard icons |

## Environment Variables

Προσθήκη στο `deploy/env/backend.env`:

```
# Ήδη υπάρχει:
ANTHROPIC_API_KEY=sk-ant-...

# Νέα (προσθήκη):
JWT_SECRET=                # Shared secret με .NET πλατφόρμα (κενό = JWT disabled)
ADMIN_PASSWORD=            # Standalone πρόσβαση στα analytics (χωρίς JWT)
```

| Variable | Υποχρεωτικό | Πότε χρειάζεται |
|----------|-------------|-----------------|
| `JWT_SECRET` | Όχι τώρα | Όταν ενεργοποιηθεί JWT με τη .NET πλατφόρμα |
| `ADMIN_PASSWORD` | Συνιστάται | Για πρόσβαση στα analytics χωρίς JWT |
| `ANTHROPIC_TIER` | Όχι (default: 2) | Αν αναβαθμίσεις tier στο Anthropic Console |

## Deploy Steps

### 1. Sync αλλαγμένα αρχεία στον server

```bash
SERVER=ykaragiorgos@devai.apopsi.gr
REMOTE=/home/ykaragiorgos/ai_content_creator

# Backend αλλαγές (auth, analytics, config, export, database)
rsync -avz backend_py/src/edu_backend/auth.py $SERVER:$REMOTE/backend_py/src/edu_backend/
rsync -avz backend_py/src/edu_backend/config.py $SERVER:$REMOTE/backend_py/src/edu_backend/
rsync -avz backend_py/src/edu_backend/database.py $SERVER:$REMOTE/backend_py/src/edu_backend/
rsync -avz backend_py/src/edu_backend/main.py $SERVER:$REMOTE/backend_py/src/edu_backend/
rsync -avz backend_py/src/edu_backend/routers/analytics.py $SERVER:$REMOTE/backend_py/src/edu_backend/routers/
rsync -avz backend_py/src/edu_backend/routers/export.py $SERVER:$REMOTE/backend_py/src/edu_backend/routers/

# Backend dependencies (νέα: PyJWT, aiosqlite, python-docx)
rsync -avz backend_py/pyproject.toml $SERVER:$REMOTE/backend_py/

# Frontend αλλαγές (auth, dashboard, api, store)
rsync -avz web/src/App.tsx $SERVER:$REMOTE/web/src/
rsync -avz web/src/services/api.ts $SERVER:$REMOTE/web/src/services/
rsync -avz web/src/store/useStore.ts $SERVER:$REMOTE/web/src/store/
rsync -avz web/src/hooks/ $SERVER:$REMOTE/web/src/hooks/
rsync -avz web/src/components/AdminDashboard.tsx $SERVER:$REMOTE/web/src/components/
rsync -avz web/src/components/AdminDashboard.module.css $SERVER:$REMOTE/web/src/components/

# Frontend dependencies (νέα: recharts, lucide-react)
rsync -avz web/package.json $SERVER:$REMOTE/web/
rsync -avz web/package-lock.json $SERVER:$REMOTE/web/
```

### 2. Env variables στον server

Ήδη προστέθηκαν στο `deploy/env/backend.env`:
- `JWT_SECRET` — shared secret με .NET πλατφόρμα
- `ADMIN_PASSWORD` — standalone πρόσβαση analytics

### 3. Rebuild και restart

```bash
cd /home/ykaragiorgos/ai_content_creator

# Rebuild frontend + backend (νέα dependencies → χρειάζεται full rebuild)
docker compose build deploy-api-1 deploy-frontend-1

# Restart
docker compose up -d deploy-api-1 deploy-frontend-1
```

### 4. Verify

```bash
# Έλεγχος ότι τρέχουν σωστά
docker compose ps

# Έλεγχος logs για errors
docker compose logs --tail=30 deploy-api-1
docker compose logs --tail=30 deploy-frontend-1

# Health check
curl -fsS http://127.0.0.1:8001/api/health

# Test analytics (αντικατέστησε XXX με το ADMIN_PASSWORD)
curl -fsS "http://127.0.0.1:8001/api/analytics/summary?admin_password=XXX"
```

## Τι αλλάζει σε αυτό το deploy

### 1. JWT Authentication (νέο)
- Optional JWT auth — δεν σπάει τίποτα αν δεν υπάρχει `JWT_SECRET`
- Ρόλοι: `internal_employee`, `admin`, `external_partner`
- Μελλοντικά: .NET πλατφόρμα στέλνει token → αφαίρεση nginx basic auth

### 2. Analytics Dashboard (νέο)
- SQLite database: `data/analytics.db` (αυτόματη δημιουργία)
- Admin dashboard: `?view=admin&admin_password=XXX`
- Δείχνει: χρήστες, generations, tokens, exports, γράφημα/ημέρα

### 3. Backend Word Export (νέο)
- Python-based export (python-docx) ως fallback
- Endpoint: `POST /api/export/word`

### 4. Standalone Admin Password (νέο)
- Πρόσβαση στα analytics χωρίς JWT
- URL: `https://devai.apopsi.gr/ai-content/?view=admin&admin_password=XXX`

### 5. Rate Limiting (ήδη deployed 2026-03-04)
- Concurrency control για ~50 ταυτόχρονους χρήστες

### 6. Page Accuracy + Word Export Fix (ήδη deployed)
- "ΠΕΡΙΠΟΥ X σελίδες" + max ceiling
- Corrupted .docx fix

## Σημειώσεις

- **Νέα dependencies**: PyJWT, aiosqlite, python-docx (backend), recharts, lucide-react (frontend)
- **Frontend + Backend**: Rebuild απαραίτητο (docker compose build)
- **Research Hub**: Rebuild αν δεν έγινε στο προηγούμενο deploy
- **Analytics DB**: Δημιουργείται αυτόματα στο `data/analytics.db` — δεν χρειάζεται setup
- **JWT**: Αν `JWT_SECRET` είναι κενό, τα JWT endpoints απλά δεν λειτουργούν (δεν σπάει τίποτα)
- **Nginx**: Δεν αλλάζει τίποτα τώρα. Basic auth μένει. Θα αφαιρεθεί μόνο αφού ενεργοποιηθεί JWT
- Αν κάτι πάει στραβά: `docker compose logs -f deploy-api-1` για live logs

# Update Guide — June 2026: Επέκταση + Configurable Content Structure

> Οδηγός για ενημέρωση του κώδικα **σε αυτόν τον server** (`~/apps/ai_material_creator_v2/`) μετά το rsync της 2026-06-10.
> Ο server **δεν τρέχει με docker** εδώ — τρέχει με τοπικό Python venv (`backend_py/.venv`) + static frontend bundle.
> Στόχος αυτού του αρχείου: ένα Claude που τρέχει πάνω στον server να εφαρμόσει τα βήματα και να επιβεβαιώσει.

---

## 1. Τι ΕΓΙΝΕ ήδη (rsync από local, 2026-06-10)

Μεταφέρθηκαν **9 αρχεία** (local HEAD → `~/apps/ai_material_creator_v2/`). Καλύπτουν **δύο features**:

- **Επέκταση** (edit-doc flow: προσθήκη N σελίδων σε ανεβασμένο κείμενο)
- **Configurable content structure** (toggle προαιρετικών δομικών στοιχείων)

Αρχεία που ήρθαν:

```
backend_py/src/edu_backend/prompts/system_prompt.py     (EXPERIMENTAL_SYSTEM_PROMPT μετακινήθηκε εδώ)
backend_py/src/edu_backend/prompts/structure.py         (ΝΕΟ αρχείο)
backend_py/src/edu_backend/services/llm_service.py      (EXPERIMENTAL αφαιρέθηκε + config wiring)
backend_py/src/edu_backend/routers/generate.py          (νέο field structure_config)
web/src/components/ContentGenerator.tsx
web/src/components/ContentGenerator.module.css
web/src/services/api.ts
web/src/services/claudeService.ts
web/src/store/useStore.ts
```

**Ήδη επαληθευμένο μετά το rsync:** το `backend_py/.venv/bin/python -c "import edu_backend.prompts.structure"` τρέχει καθαρά — δηλαδή το import-time `_validate()` πέρασε (όλα τα prompt anchors συνεπή, default = byte-identical με πριν).

### Τι αλλάζει λειτουργικά

- **Καμία αλλαγή σε dependencies** — το `structure.py` χρησιμοποιεί μόνο stdlib (`dataclasses`, `re`). **ΔΕΝ χρειάζεται `pip install`.**
- **API contract (backward-compatible):** νέο **προαιρετικό** πεδίο `structure_config` στο `POST /api/generate-stream`:
  ```json
  { "structure_config": {
      "activities": true, "self_assessment": true, "glossary": true,
      "subsection_keywords": true, "in_text_citations": true } }
  ```
  Αν παραλειφθεί → όλα `true` → **ίδια συμπεριφορά με πριν**. Άρα παλιοί clients δεν σπάνε.
- **Frontend:** νέο UI «Δομή υλικού» (5 checkboxes) + Επέκταση control. Θέλει **rebuild** για να φανεί.

---

## 2. Τι ΠΡΕΠΕΙ να γίνει για να τρέξει ο νέος κώδικας

### Βήμα A — Restart backend (Python/uvicorn)

Ο νέος κώδικας prompts φορτώνεται στο import, άρα χρειάζεται restart της API διεργασίας.

1. **Βρες πώς τρέχει το backend εδώ.** Έλεγξε με αυτή τη σειρά:
   - `cat ~/apps/ai_material_creator_v2/systemctl_guide.md` — ο πιο πιθανός μηχανισμός (systemd service).
   - `systemctl list-units --type=service | grep -iE 'ai|edu|material|uvicorn|backend'`
   - `systemctl --user list-units --type=service | grep -iE 'ai|edu|material|uvicorn|backend'`
   - αν δεν υπάρχει service: ψάξε running process `ps aux | grep -E 'uvicorn|edu_backend' | grep -v grep` και δες πώς ξεκίνησε (port, working dir). Δες και `start-local-server.sh`, `run_all.sh`.
2. **Restart** τη διεργασία/service που βρήκες (π.χ. `sudo systemctl restart <service>` ή kill+restart του uvicorn).
3. Αν τρέχει με `--reload`, το restart γίνεται αυτόματα μόλις άλλαξαν τα `.py` — αλλά κάνε explicit restart για σιγουριά (το import-time `_validate` θέλει καθαρό φόρτωμα).

> **Σημαντικό:** αν το backend **ξεκινήσει καθαρά**, σημαίνει ότι το `structure.py::_validate()` πέρασε → τα prompts είναι συνεπή. Αν για κάποιο λόγο κάποιο anchor είχε ξεφύγει, το backend **δεν θα ξεκινούσε** (fail-fast). Δες τα logs στο startup.

### Βήμα B — Rebuild frontend (static bundle)

Οι αλλαγές στο `web/src/` χρειάζονται build.

```bash
cd ~/apps/ai_material_creator_v2/web
# npm install ΜΟΝΟ αν άλλαξαν deps — ΔΕΝ άλλαξαν, οπότε προσπέρασέ το (τρέξε το μόνο αν το build παραπονεθεί)
npm run build
```

Μετά βεβαιώσου ότι ο web server σερβίρει το νέο bundle:
- Βρες πώς σερβίρεται το frontend εδώ: `cat ~/apps/ai_material_creator_v2/CURRENT_SERVER_NGINX_HTTPS.md` και `server_config.md` / `server_config_executive.md`.
- Συνήθως nginx δείχνει σε ένα `web/dist` (ή αντιγράφεται κάπου). Αν χρειάζεται, αντίγραψε το `web/dist` στη σωστή θέση και κάνε `sudo nginx -s reload` (ή ό,τι λέει ο οδηγός).

---

## 3. Smoke checks (μετά τα A + B)

1. `curl -s http://localhost:<API_PORT>/api/health` → 200. (Βρες το port από το service/process — τοπικά συνήθως 8002.)
2. **Default path:** κάνε ένα κανονικό generation **χωρίς** να αλλάξεις τα checkboxes «Δομή υλικού» → output ίδιο όπως πάντα (Σκοπός → … → Δραστηριότητες → Αυτοαξιολόγηση → Βιβλιογραφία → Γλωσσάρι).
3. **Toggle off:** ξετσέκαρε «Γλωσσάρι» + «Δραστηριότητες» → Δημιουργία. Το output **δεν** πρέπει να έχει αυτά τα δύο, αλλά **πρέπει** να έχει Βιβλιογραφία + πλήρες ακαδημαϊκό ύφος. Στο DevTools → Network → `/api/generate-stream` το body δείχνει `structure_config` με `false` στα αντίστοιχα.
4. **Επέκταση:** ανέβασε docx για editing → approve → όρισε «Σελίδες προς προσθήκη» → «Προσθήκη N σελίδων» → νέο τμήμα συνεχίζει χωρίς επανάληψη.

---

## 4. Rollback (αν χρειαστεί)

Ο server **δεν είναι git repo** εδώ. Για rollback των 9 αρχείων:
- Ο καθαρός τρόπος: ξανα-rsync από local checkout στο commit **`21372d1`** (το προηγούμενο των features) — ή στο σημείο που ήταν πριν.
- Backend-only quick revert: διέγραψε το νέο `backend_py/src/edu_backend/prompts/structure.py` **και** επανάφερε τα `system_prompt.py`, `llm_service.py`, `generate.py` στις προηγούμενες εκδόσεις (το `structure.py` μόνο του δεν φτάνει — το `llm_service.py`/`generate.py` το εισάγουν). Μετά restart backend + rebuild frontend.

> Reference commits (local repo): `038e709` (structure config), `21372d1` (Επέκταση), `d723df3` (deploy guide sync). Πλήρες incremental entry στο `guides_for_server.md` (κορυφή).

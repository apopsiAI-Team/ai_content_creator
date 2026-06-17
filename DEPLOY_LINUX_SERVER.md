# Deploy σε `devai.apopsi.gr` με Docker + Nginx

Target path στο server: `/home/ykaragiorgos/ai_content_creator`

## 1) Αντιγραφή project στο server

Από το local machine:

```bash
rsync -avz --delete \
  --exclude '.venv' \
  --exclude 'venv' \
  --exclude 'web/node_modules' \
  /home/yiannis/coding/ai_material_creator_v2/ \
  ykaragiorgos@172.104.146.160:/home/ykaragiorgos/ai_content_creator/
```

## 2) Secrets (backend env + basic auth)

Στον server:

```bash
ssh ykaragiorgos@172.104.146.160
cd /home/ykaragiorgos/ai_content_creator
mkdir -p deploy/env
cp deploy/env/backend.env.example deploy/env/backend.env
```

Συμπλήρωσε το `deploy/env/backend.env` με το πραγματικό `ANTHROPIC_API_KEY`.

### htpasswd

Το αρχείο πρέπει να δημιουργηθεί στο:
`/home/ykaragiorgos/frameworks-app/Frameworks/secrets/htpasswd`

```bash
mkdir -p /home/ykaragiorgos/frameworks-app/Frameworks/secrets
htpasswd -cB /home/ykaragiorgos/frameworks-app/Frameworks/secrets/htpasswd elearning_admin
```

## 3) Compose vars

```bash
cp deploy/.env.compose.example .env.compose
```

Έλεγξε/άλλαξε στο `.env.compose`:
- `SHARED_NGINX_NETWORK`: το docker network όπου βρίσκεται το nginx container.
- `APP_ENV_FILE`: συνήθως `./deploy/env/backend.env`.

Αν δεν ξέρεις το network:

```bash
docker network ls
```

## 4) Build + up

```bash
docker compose --env-file .env.compose up -d --build
docker compose --env-file .env.compose ps
```

## 5) Nginx (χωρίς επίδραση στα άλλα apps)

Χρησιμοποίησε τα location blocks από `deploy/nginx/e-learning.locations.conf`.
Αν υπάρχουν ήδη ίδια blocks στο `site.conf`, δεν αλλάζεις τίποτα.
Αν λείπουν, πρόσθεσε **μόνο** αυτά στο ίδιο `server { ... }`.

Path config:
`/home/ykaragiorgos/frameworks-app/Frameworks/nginx/site.conf`

Σημαντικό για containerized nginx:
- Το nginx container πρέπει να είναι στο ίδιο docker network με το app stack (`frameworks_default`).
- Το `htpasswd` από `~/frameworks-app/Frameworks/secrets/htpasswd` πρέπει να γίνεται mount στο `/etc/nginx/htpasswd`.

Έλεγχος και reload:

```bash
cd /home/ykaragiorgos/frameworks-app/Frameworks/nginx
nginx -t
sudo systemctl reload nginx
```

## 6) Validation

```bash
curl -I https://devai.apopsi.gr/e-learning/
curl -u elearning_admin:YOUR_PASSWORD https://devai.apopsi.gr/e-learning/api/health
```

Αναμενόμενο:
- Frontend στο `https://devai.apopsi.gr/e-learning/`
- Backend health `{"status":"ok", ...}` στο `/e-learning/api/health`

## 7) Update process

Κάθε νέο deploy:

```bash
cd /home/ykaragiorgos/ai_content_creator
docker compose --env-file .env.compose up -d --build
```

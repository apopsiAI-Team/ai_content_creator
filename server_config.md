# Server Configuration (Docker + Nginx) — CTO Brief

## 1. Scope

Αυτό το έγγραφο περιγράφει το τρέχον setup του app σε Linux server με Docker + shared Nginx και δίνει ειλικρινή αξιολόγηση ασφαλείας/ανθεκτικότητας.

Βασικό περιβάλλον:
- Host: `devai.apopsi.gr` (`172.104.146.160`)
- App path (ai-content): `/home/ykaragiorgos/ai_content_creator`
- Shared Nginx config path: `/home/ykaragiorgos/frameworks-app/Frameworks/nginx/site.conf`
- Shared secrets (basic auth): `/home/ykaragiorgos/frameworks-app/Frameworks/secrets/htpasswd`

## 2. Current Architecture

### 2.1 Containers (ai-content stack)
Από το `docker-compose.yml`:
- `deploy-api-1` (FastAPI backend, port `8001` internal)
- `deploy-frontend-1` (Nginx static frontend, port `80` internal)
- `research-hub` (Rust service, port `8091` internal)

Networks:
- `app-internal` (backend <-> research-hub)
- `shared-nginx` (external Docker network για πρόσβαση από shared Nginx container)

### 2.2 Ingress / Reverse Proxy
Ο shared Nginx container (`frameworks-nginx-1`) κάνει path-based routing:
- `/e-learning/` -> `deploy-frontend-1:80`
- `/e-learning/api/` -> `deploy-api-1:8001` (με rewrite)
- `/ai-content/` -> `ai-content-frontend:80`
- `/ai-content/api/` -> `ai-content-api:8001/api/` (με `proxy_buffering off` για SSE)
- `/` -> default app (`app:7860`)

### 2.3 TLS & Auth
- TLS termination στον shared Nginx με Let’s Encrypt certs.
- Basic Auth (`htpasswd`) στο Nginx level ανά location.

## 3. Why It Is Reasonably Safe Today (for ~10 users)

Θετικά:
- HTTPS-only ingress (TLS termination).
- Basic Auth πριν το app layer.
- Containers απομονωμένα σε Docker networks.
- No direct public exposure των backend ports.
- Healthchecks για αυτο-ανίχνευση degraded container states.
- Restart policies (`unless-stopped`) για basic resilience.

Πρακτικά, για μικρή ομάδα (~10 users), το setup είναι λειτουργικό και αρκετά ασφαλές για controlled/internal χρήση.

## 4. Current Limitations (Honest Assessment)

Δεν είναι πλήρες production-grade architecture για scale:

1. Single host / single point of failure  
- Ένα VM, ένας shared Nginx, ένα Docker engine.

2. Shared Nginx για πολλά apps  
- Operational coupling: αλλαγές/reload στο Nginx επηρεάζουν πολλαπλά services.

3. Basic Auth only  
- Δεν υπάρχει SSO/OIDC, fine-grained RBAC, MFA policy integration.

4. Secrets handling  
- API keys σε env files. Αποδεκτό για small setup, αλλά όχι ιδανικό για enterprise compliance.

5. Limited observability  
- Δεν υπάρχει κεντρικό log aggregation + structured tracing + alerting pipeline.

6. No autoscaling / HA  
- Static capacity.

7. External model dependency risk  
- Σε Anthropic overload (`529`) επηρεάζεται η παραγωγή περιεχομένου.
- Έχει μπει retry/backoff στο backend, αλλά δεν μηδενίζει provider-side overload.

## 5. Notes from Real Incidents

Τι έχει παρατηρηθεί στην πράξη:
- `502 Bad Gateway` από stale upstream state μετά container recreate/project-name mismatch.
- Streaming issues όταν `proxy_buffering` ήταν ενεργό ή frontend parser δεν χειριζόταν σωστά chunk boundaries.
- Provider overload (`529`) σε ταυτόχρονες βαριές γεννήσεις.

Τρέχον mitigations:
- `proxy_buffering off` στα streaming API paths.
- SSE parser fix στο frontend.
- Exponential backoff retries στο backend streaming path.
- Safer dedup logic σε research results (null-title guard).

## 6. Production Upgrade Path (Recommended)

### Phase A (low effort, high impact)
1. Standardize deploy commands  
- Πάντα explicit `-p <project>` και σταθερό compose path.
- Prevent container-name drift που οδηγεί σε 502.

2. Add runbook + rollback SOP  
- Versioned deploy checklist.
- Fast rollback commands per service.

3. Better error UX  
- Μην θεωρείται success το empty stream.
- Καθαρά messages σε overload/retry exhaustion.

4. Concurrency guard  
- 1 active generation per user/session.

### Phase B (security + ops hardening)
1. Replace Basic Auth with SSO/OIDC (or at least app-level auth + RBAC).
2. Move secrets to secret manager (Vault/1Password Connect/Docker secrets).
3. Centralized logs + alerts (Loki/ELK + Grafana + alert rules).
4. Add request correlation IDs (Nginx -> API -> model calls).

### Phase C (true production scaling)
1. Separate ingress tier (dedicated reverse proxy / LB).
2. Multi-host orchestration (Kubernetes or Swarm with HA strategy).
3. Horizontal scaling for API workers.
4. Queue-based generation workers (Celery/RQ/Redis) for long-running jobs.
5. Capacity policy for LLM usage (rate limiting, budget guards, model fallback).

## 7. Nginx Configuration Principles (Operational)

Για σταθερότητα:
- Keep path mappings explicit (`location` order, rewrite behavior, trailing slashes).
- Για streaming routes: `proxy_buffering off`.
- Preserve forwarding headers:
  - `Host`
  - `X-Real-IP`
  - `X-Forwarded-For`
  - `X-Forwarded-Proto`
- Always run:
  - `nginx -t` πριν reload
  - `nginx -s reload` μετά validate

Σημαντικό:
- Αν το `site.conf` είναι bind-mounted σε `/etc/nginx/conf.d/default.conf`, οι edits να γίνονται in-place (όχι inode-replacing patterns που μπερδεύουν mounts).

## 8. Docker Configuration Principles (Operational)

- Rebuild only what changed (`api`, `frontend`, or specific service).
- Use explicit compose project names to preserve expected container DNS names.
- Verify after deploy:
  - `docker compose ps`
  - health endpoints
  - ingress checks through shared nginx

## 9. Is This Setup Acceptable Today?

Ναι, για μικρή ελεγχόμενη χρήση (~10 άτομα), με υπάρχοντα controls και operational discipline.

Όχι, αν ο στόχος είναι:
- strict enterprise security/compliance,
- high availability SLO,
- predictable performance under higher concurrency.

## 10. Minimal Next Actions (Recommended Immediately)

1. Keep current stack, but formalize deploy/runbook (mandatory).  
2. Add app-level auth/RBAC plan (replace pure basic-auth dependence).  
3. Add centralized monitoring + alerts.  
4. Implement generation queue/concurrency caps before user growth.  
5. Plan dedicated ingress + multi-host strategy before larger rollout.


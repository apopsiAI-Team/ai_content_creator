# JWT Integration Guide - AI Content Creator Server Version

This guide documents the current JWT integration for the app hosted at:

```text
https://apopsi-ai.apopsi.gr/ai-content/
```

The app is part of a broader e-mentoring platform. User management remains in the e-mentoring platform. The AI Content app does not implement its own login UI.

## Current State

JWT support has been added, but enforcement is disabled by default so existing production usage is not interrupted.

```env
JWT_AUTH_ENABLED=false
```

With this setting, all existing app functionality continues to work without a token. The JWT mint endpoint is available for integration testing.

When the platform integration is ready, enforcement can be enabled by changing:

```env
JWT_AUTH_ENABLED=true
```

and restarting the backend service:

```bash
sudo systemctl restart edu-backend
```

## Authentication Flow

The agreed flow is server-to-server token minting:

```text
1. User logs in to the e-mentoring platform.
2. User clicks the AI Content link inside the platform.
3. The e-mentoring backend calls the AI Content backend server-to-server.
4. AI Content verifies X-Platform-Secret.
5. AI Content mints an HS256 JWT valid for 8 hours.
6. AI Content returns a URL containing ?token=...
7. The e-mentoring platform redirects the browser to that URL.
8. The frontend stores the token in sessionStorage and removes it from the browser URL.
9. Subsequent API calls send Authorization: Bearer <token>.
```

## Environment Variables

These values live in:

```text
/home/yiannis-apopsi/apps/ai_material_creator_v2/.env
```

Required JWT settings:

```env
JWT_AUTH_ENABLED=false
JWT_SIGNING_SECRET=<random-secret-used-to-sign-ai-content-jwts>
JWT_PLATFORM_SECRET=<shared-secret-used-by-e-mentoring-server-to-call-mint>
JWT_ISSUER=apopsi-ai
JWT_EXPIRE_HOURS=8
APP_PUBLIC_URL=https://apopsi-ai.apopsi.gr/ai-content/
```

Do not commit `.env`. It is already ignored by `.gitignore`.

Generate fresh secrets with:

```bash
openssl rand -hex 32
openssl rand -hex 32
```

Use different values for `JWT_SIGNING_SECRET` and `JWT_PLATFORM_SECRET`.

## Mint Endpoint

The e-mentoring platform calls:

```http
POST https://apopsi-ai.apopsi.gr/ai-content/api/auth/mint
Content-Type: application/json
X-Platform-Secret: <JWT_PLATFORM_SECRET>
```

Request body:

```json
{
  "sub": "4821",
  "email": "user@example.com",
  "role": "internal_employee"
}
```

Response body:

```json
{
  "url": "https://apopsi-ai.apopsi.gr/ai-content/?token=JWT_HERE",
  "expires_at": "2026-06-17T15:02:11Z"
}
```

The platform should redirect the user to the `url` value.

## JWT Payload

The minted token contains this payload shape:

```json
{
  "sub": "4821",
  "email": "user@example.com",
  "role": "internal_employee",
  "iss": "apopsi-ai",
  "iat": 1781679731,
  "exp": 1781708531
}
```

Claim meanings:

- `sub`: stable unique user id from the e-mentoring platform.
- `email`: user email from the e-mentoring platform.
- `role`: authorization role for this app.
- `iss`: token issuer. Currently `apopsi-ai`.
- `iat`: issued-at Unix timestamp.
- `exp`: expiration Unix timestamp.

Allowed roles:

```text
internal_employee
external_partner
```

## Protected Endpoints

The JWT dependency is connected to the main production API routers. While `JWT_AUTH_ENABLED=false`, the dependency does not block requests.

When `JWT_AUTH_ENABLED=true`, these endpoints require:

```http
Authorization: Bearer <jwt>
```

Protected endpoint groups:

```text
GET  /api/research/search
POST /api/generate
POST /api/generate-stream
POST /api/generate-summary
POST /api/generate-bibliography
POST /api/review
POST /api/claude/generate
POST /api/claude/generate-stream
POST /api/parse-docx
POST /api/docx-to-markdown
GET  /api/rate-limit/status
```

Public endpoints:

```text
GET  /api/health
POST /api/auth/mint
```

`POST /api/auth/mint` is public only in the JWT sense. It is protected by:

```http
X-Platform-Secret: <JWT_PLATFORM_SECRET>
```

## Frontend Behavior

The frontend handles links like:

```text
https://apopsi-ai.apopsi.gr/ai-content/?token=JWT_HERE
```

On load it:

```text
1. Reads token from the URL.
2. Stores it in sessionStorage under edu-material-auth-token.
3. Removes ?token=... from the browser address bar.
4. Adds Authorization: Bearer <token> to API calls.
```

If enforcement is enabled and a request receives `401`, the user sees:

```text
Δεν υπάρχει ενεργή πρόσβαση. Παρακαλώ ανοίξτε το εργαλείο μέσα από την πλατφόρμα e-mentoring.
```

No redirect is currently implemented. Redirect can be added later when the platform URL is finalized.

## Implementation Files

Backend:

```text
backend_py/src/edu_backend/auth.py
backend_py/src/edu_backend/routers/auth.py
backend_py/src/edu_backend/config.py
backend_py/src/edu_backend/main.py
backend_py/src/edu_backend/routers/generate.py
backend_py/src/edu_backend/routers/claude.py
backend_py/src/edu_backend/routers/docx.py
```

Frontend:

```text
web/src/services/api.ts
web/src/App.tsx
```

## Verification Commands

Run from the server:

```bash
cd /home/yiannis-apopsi/apps/ai_material_creator_v2
```

Health check through local nginx HTTPS:

```bash
curl -sS --resolve apopsi-ai.apopsi.gr:443:127.0.0.1 \
  https://apopsi-ai.apopsi.gr/ai-content/api/health
```

Mint token test:

```bash
curl -sS -X POST \
  --resolve apopsi-ai.apopsi.gr:443:127.0.0.1 \
  https://apopsi-ai.apopsi.gr/ai-content/api/auth/mint \
  -H 'Content-Type: application/json' \
  -H 'X-Platform-Secret: <JWT_PLATFORM_SECRET>' \
  -d '{"sub":"123","email":"user@example.com","role":"internal_employee"}'
```

Expected response contains:

```json
{
  "url": "https://apopsi-ai.apopsi.gr/ai-content/?token=...",
  "expires_at": "..."
}
```

Frontend build:

```bash
cd /home/yiannis-apopsi/apps/ai_material_creator_v2/web
VITE_BASE_PATH=/ai-content/ VITE_API_URL=/ai-content npm run build
```

Deploy frontend build:

```bash
cd /home/yiannis-apopsi/apps/ai_material_creator_v2
sudo rsync -a --delete web/dist/ /var/www/apopsi-ai/ai-content/
sudo systemctl restart edu-backend
```

## Enabling JWT Enforcement

Only do this after the e-mentoring platform can mint links successfully.

1. Edit `.env`:

```env
JWT_AUTH_ENABLED=true
```

2. Restart backend:

```bash
sudo systemctl restart edu-backend
```

3. Test direct access without token. Frontend should load, but protected API calls should fail with a clear access message.

4. Test access through a minted URL. API calls should succeed with `Authorization: Bearer <token>`.

## Immediate Rollback

Fast rollback without changing code:

```env
JWT_AUTH_ENABLED=false
```

Then restart:

```bash
sudo systemctl restart edu-backend
```

This disables JWT enforcement and returns API access to the pre-enforcement behavior.

## Code Rollback

If the JWT code itself must be removed, revert the JWT commit from git after this repository is pushed.

Check history:

```bash
git log --oneline
```

Revert a specific commit:

```bash
git revert <commit-sha>
```

Push rollback:

```bash
git push
```

Because the initial repository commit includes the current codebase, the safest operational rollback remains `JWT_AUTH_ENABLED=false` unless a code-level issue is found.

## Future Alternative: Platform-Issued JWT

If the e-mentoring platform later wants to issue the JWT itself, the frontend flow can remain the same:

```text
https://apopsi-ai.apopsi.gr/ai-content/?token=...
```

and API calls can keep using:

```http
Authorization: Bearer <token>
```

For HS256, both systems must share the same signing secret. The AI Content backend would verify platform-issued tokens instead of using `/api/auth/mint`.

The payload should remain compatible:

```json
{
  "sub": "4821",
  "email": "user@example.com",
  "role": "internal_employee",
  "iss": "e-mentoring",
  "iat": 1781679731,
  "exp": 1781708531
}
```

Then update `.env` accordingly:

```env
JWT_ISSUER=e-mentoring
```

`/api/auth/mint` can remain available or be retired later.

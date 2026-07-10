# JWT Integration Guide - AI Content Creator Server Version

This guide documents the agreed JWT integration for:

```text
https://apopsi-ai.apopsi.gr/ai-content/
```

The AI Content app is part of the broader e-mentoring platform. User management remains in the e-mentoring platform. The AI Content app does not implement its own login UI.

## Current Decision

The e-mentoring platform issues the JWT.

AI Content does not need to mint the production login token. It verifies the token that arrives in the URL and then protects API calls with:

```http
Authorization: Bearer <jwt>
```

The old AI Content mint endpoint can remain available as a legacy/test endpoint, but it is not the primary production flow.

## Production Flow

```text
1. User logs in to the e-mentoring platform.
2. User clicks the AI Content link inside the platform.
3. The e-mentoring platform creates an HS256 JWT for that user.
4. The platform redirects the browser to:
   https://apopsi-ai.apopsi.gr/ai-content/?token=<JWT>
5. The frontend stores the token in sessionStorage.
6. The frontend removes ?token=... from the browser URL.
7. Subsequent API calls include Authorization: Bearer <JWT>.
8. The AI Content backend verifies the JWT signature, expiry, user id, and role.
```

## Shared Secret

Because the algorithm is HS256, both systems must use the same signing secret.

The platform uses the secret to sign the token. AI Content uses the same secret to verify the token.

The secret must never be exposed in frontend JavaScript, URLs, Postman collections, GitHub, screenshots, or public documentation. It belongs only in backend/server configuration.

## Environment Variables

These values live on the server in:

```text
/home/yiannis-apopsi/apps/ai_material_creator_v2/.env
```

Recommended JWT settings for the platform-issued-token flow:

```env
JWT_AUTH_ENABLED=false
JWT_SIGNING_SECRET=<shared-hs256-secret-from-platform>
JWT_VALIDATE_ISSUER=false
JWT_ISSUER=apopsi-ai
APP_PUBLIC_URL=https://apopsi-ai.apopsi.gr/ai-content/
```

Notes:

- Keep `JWT_AUTH_ENABLED=false` until the platform redirect has been tested end to end.
- `JWT_SIGNING_SECRET` must match the secret used by the e-mentoring platform to sign JWTs.
- `JWT_VALIDATE_ISSUER=false` means the backend does not require an `iss` claim.
- If the platform later adds a stable issuer claim, set `JWT_VALIDATE_ISSUER=true` and set `JWT_ISSUER` to that exact value.
- `JWT_PLATFORM_SECRET` and `JWT_EXPIRE_HOURS` are only relevant to the legacy `/api/auth/mint` endpoint.

After `.env` changes, restart the backend:

```bash
sudo systemctl restart edu-backend
```

## JWT Header

The platform token header should be:

```json
{
  "alg": "HS256",
  "typ": "JWT"
}
```

`HS256` means the token is signed with a shared secret.

## JWT Payload

The agreed platform payload shape is:

```json
{
  "sub": "38B3CCA1-D6A4-4037-95EF-BCA03FFA3AA8",
  "name": "System Administrator",
  "role": "internal_employee",
  "exp": 1783538196
}
```

Claim meanings:

- `sub`: stable unique user id from the e-mentoring platform.
- `name`: display name from the e-mentoring platform. Useful for future exports/auditing, but not currently required for access.
- `role`: authorization role for this app.
- `exp`: expiration as Unix timestamp in seconds.

Allowed roles:

```text
internal_employee
external_partner
```

Currently required by AI Content:

```text
sub
role
exp
```

Currently optional:

```text
name
iss
iat
email
```

## Login URL

The platform should redirect the browser to:

```text
https://apopsi-ai.apopsi.gr/ai-content/?token=<JWT>
```

Do not call this URL server-to-server. It is a browser redirect target.

## Frontend Behavior

On load, the frontend:

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

If a request receives `403`, the user sees:

```text
Ο λογαριασμός σας δεν έχει δικαίωμα πρόσβασης σε αυτό το εργαλείο.
```

## Protected Endpoints

While `JWT_AUTH_ENABLED=false`, the dependency does not block requests.

When `JWT_AUTH_ENABLED=true`, these endpoint groups require:

```http
Authorization: Bearer <jwt>
```

Protected endpoint groups:

```text
POST /api/generate...
POST /api/claude...
POST /api/docx...
GET  /api/rate-limit/status
```

Public endpoints:

```text
GET  /api/health
POST /api/auth/mint       legacy/test only
```

## Legacy Mint Endpoint

The old endpoint still exists:

```http
POST https://apopsi-ai.apopsi.gr/ai-content/api/auth/mint
Content-Type: application/json
X-Platform-Secret: <JWT_PLATFORM_SECRET>
```

This endpoint lets AI Content mint a token itself. It is no longer the preferred production flow.

Use it only for legacy compatibility or controlled local testing.

## Testing With a Platform Token

After the platform generates a JWT, test from the server:

```bash
TOKEN="JWT_FROM_PLATFORM"

curl -sS --resolve apopsi-ai.apopsi.gr:443:127.0.0.1 \
  -H "Authorization: Bearer $TOKEN" \
  https://apopsi-ai.apopsi.gr/ai-content/api/rate-limit/status
```

Expected result when the token is valid and `JWT_AUTH_ENABLED=true`:

```text
HTTP 200 / JSON response
```

Common failures:

```text
401 Invalid token             wrong secret, malformed token, or bad signature
401 Token expired             exp is in the past
401 Missing bearer token      frontend did not send Authorization header
401 Invalid token expiration  exp is missing or not numeric
403 Invalid role              role is not internal_employee or external_partner
```

## Enabling Enforcement

Enable only after the platform redirect URL has been tested with a real token.

1. Keep the current app running with `JWT_AUTH_ENABLED=false`.
2. Ask the platform to generate a test token and redirect URL.
3. Open the URL in a browser:

```text
https://apopsi-ai.apopsi.gr/ai-content/?token=<JWT>
```

4. Confirm that the frontend loads and API calls include `Authorization: Bearer`.
5. Change `.env`:

```env
JWT_AUTH_ENABLED=true
```

6. Restart backend:

```bash
sudo systemctl restart edu-backend
```

7. Test direct access without token. API calls should fail with a clear access message.
8. Test access through the platform URL. API calls should succeed.

## Operational Rollback

Fast rollback:

```env
JWT_AUTH_ENABLED=false
```

Then:

```bash
sudo systemctl restart edu-backend
```

This returns protected API access to the pre-enforcement behavior.

## Export Back To Platform

This JWT flow is only for access to AI Content.

Sending generated material back to the e-mentoring platform is a separate integration. The platform has provided an import endpoint using Basic Auth. AI Content should call that endpoint server-to-server when export is implemented.


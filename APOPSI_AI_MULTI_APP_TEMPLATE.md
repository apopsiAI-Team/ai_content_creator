# Adding More Apps Under apopsi-ai.apopsi.gr

The TLS certificate is for the hostname:

```text
apopsi-ai.apopsi.gr
```

It covers all paths under that hostname, for example:

```text
https://apopsi-ai.apopsi.gr/ai-content/
https://apopsi-ai.apopsi.gr/other-app/
https://apopsi-ai.apopsi.gr/internal-tool/
```

Do not request a new certificate for every path. Request a new certificate only when using a different hostname/subdomain.

## Pattern For A Static Frontend + API App

Example app path:

```text
/other-app/
```

Example internal backend:

```text
127.0.0.1:8010
```

Build the frontend with the correct base path. For Vite apps:

```bash
cd /path/to/other-app/web
VITE_BASE_PATH=/other-app/ VITE_API_URL=/other-app npm run build
sudo mkdir -p /var/www/apopsi-ai/other-app
sudo rsync -a --delete dist/ /var/www/apopsi-ai/other-app/
```

Add these locations inside the existing HTTPS `server` block for `apopsi-ai.apopsi.gr`:

```nginx
location = /other-app {
    return 301 /other-app/;
}

location ^~ /other-app/api/ {
    proxy_pass http://127.0.0.1:8010/api/;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto https;
    proxy_set_header Connection "";
    proxy_buffering off;
    proxy_cache off;
    proxy_read_timeout 3600s;
    proxy_send_timeout 3600s;
}

location ^~ /other-app/ {
    alias /var/www/apopsi-ai/other-app/;
    try_files $uri $uri/ /other-app/index.html;
}
```

Then validate and reload nginx:

```bash
sudo nginx -t
sudo systemctl reload nginx
```

Checks:

```bash
curl -I --resolve apopsi-ai.apopsi.gr:443:127.0.0.1 https://apopsi-ai.apopsi.gr/other-app/
curl -sS --resolve apopsi-ai.apopsi.gr:443:127.0.0.1 https://apopsi-ai.apopsi.gr/other-app/api/health
```

Also test externally from mobile data:

```text
https://apopsi-ai.apopsi.gr/other-app/
```

## Pattern For Backend-Only Apps

If an app has no static frontend and is only an HTTP service:

```nginx
location ^~ /service-name/ {
    proxy_pass http://127.0.0.1:8020/;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto https;
    proxy_read_timeout 3600s;
    proxy_send_timeout 3600s;
}
```

## Rules To Keep

- Use one unique path prefix per app.
- Keep each app backend on a different local port.
- Bind app backends to `127.0.0.1`, not `0.0.0.0`, unless there is a specific reason.
- Put more specific API/proxy locations before the static frontend location.
- Always run `sudo nginx -t` before reload.
- The existing certificate renews for the whole hostname, not per app path.


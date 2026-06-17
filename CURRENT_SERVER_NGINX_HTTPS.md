# Current Server Nginx/HTTPS Setup

This file is only for the current server:

- public IP: `62.74.214.123`
- local IP: `192.168.0.233`
- user: `yiannis-apopsi`
- app path: `/home/yiannis-apopsi/apps/ai_material_creator_v2`

It does not describe the older Docker deployment on `devai.apopsi.gr`.

## Target Runtime

- Nginx listens publicly on `80` and `443`.
- React/Vite is built once into `web/dist` and served as static files by nginx.
- FastAPI runs only on `127.0.0.1:8002`.
- Rust Research Hub runs only on `127.0.0.1:8091`.
- The old `edu-frontend` Vite dev service is disabled for production.

## 1. Stop Dev Services

```bash
sudo systemctl stop edu-frontend edu-backend edu-research-hub
systemctl is-active edu-research-hub edu-backend edu-frontend
```

Expected:

```text
inactive
inactive
inactive
```

## 2. Build Frontend

```bash
cd /home/yiannis-apopsi/apps/ai_material_creator_v2/web
npm run build
```

Expected output directory:

```text
/home/yiannis-apopsi/apps/ai_material_creator_v2/web/dist
```

## 3. Install Nginx And Certbot

```bash
sudo apt update
sudo apt install -y nginx snapd
sudo snap install core
sudo snap refresh core
sudo snap install --classic certbot
sudo ln -sf /snap/bin/certbot /usr/bin/certbot
nginx -v
certbot --version
```

For IP-address HTTPS, Certbot must be `5.4` or newer.

## 4. Disable The Old Frontend Service

The Vite dev server should not be public in production.

```bash
sudo systemctl disable --now edu-frontend
```

## 5. Update Backend Systemd Unit

```bash
sudo tee /etc/systemd/system/edu-backend.service >/dev/null <<'EOF'
[Unit]
Description=Educational Material Creator - FastAPI Backend
After=network.target edu-research-hub.service
Wants=edu-research-hub.service

[Service]
Type=simple
User=yiannis-apopsi
WorkingDirectory=/home/yiannis-apopsi/apps/ai_material_creator_v2
Environment=PYTHONPATH=/home/yiannis-apopsi/apps/ai_material_creator_v2/backend_py/src
ExecStart=/home/yiannis-apopsi/apps/ai_material_creator_v2/backend_py/.venv/bin/python -m uvicorn edu_backend.main:app --host 127.0.0.1 --port 8002
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
```

## 6. Update Research Hub Systemd Unit

```bash
sudo tee /etc/systemd/system/edu-research-hub.service >/dev/null <<'EOF'
[Unit]
Description=Educational Material Creator - Rust Research Hub
After=network.target

[Service]
Type=simple
User=yiannis-apopsi
WorkingDirectory=/home/yiannis-apopsi/apps/ai_material_creator_v2/research_hub_mcp
ExecStart=/home/yiannis-apopsi/apps/ai_material_creator_v2/research_hub_mcp/target/release/rust-research-mcp http --host 127.0.0.1 --port 8091
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
```

Reload and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now edu-research-hub edu-backend
systemctl is-active edu-research-hub edu-backend edu-frontend
```

Expected:

```text
active
active
inactive
```

Health checks:

```bash
curl -sS http://127.0.0.1:8091/health
curl -sS http://127.0.0.1:8002/api/health
```

## 7. Configure Nginx For HTTP First

```bash
sudo mkdir -p /var/www/letsencrypt
sudo tee /etc/nginx/sites-available/ai-material-creator >/dev/null <<'EOF'
server {
    listen 80 default_server;
    listen [::]:80 default_server;
    server_name 62.74.214.123 _;

    root /home/yiannis-apopsi/apps/ai_material_creator_v2/web/dist;
    index index.html;

    client_max_body_size 25m;

    location ^~ /.well-known/acme-challenge/ {
        root /var/www/letsencrypt;
        try_files $uri =404;
    }

    location /api/ {
        proxy_pass http://127.0.0.1:8002;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Connection "";
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 3600s;
        proxy_send_timeout 3600s;
    }

    location / {
        try_files $uri $uri/ /index.html;
    }
}
EOF

sudo rm -f /etc/nginx/sites-enabled/default
sudo ln -sf /etc/nginx/sites-available/ai-material-creator /etc/nginx/sites-enabled/ai-material-creator
sudo nginx -t
sudo systemctl enable --now nginx
sudo systemctl reload nginx
```

HTTP checks:

```bash
curl -I http://127.0.0.1/
curl -sS http://127.0.0.1/api/health
curl -I http://62.74.214.123/
curl -sS http://62.74.214.123/api/health
```

## 8. Firewall

If `ufw` is active:

```bash
sudo ufw allow OpenSSH
sudo ufw allow 'Nginx Full'
sudo ufw status
```

## 9. Issue HTTPS Certificate For Public IP

Let's Encrypt IP certificates are short-lived certificates. They are valid for about six days and require working automatic renewal.

Replace the email address before running:

```bash
sudo certbot certonly \
  --webroot \
  --webroot-path /var/www/letsencrypt \
  --preferred-profile shortlived \
  --ip-address 62.74.214.123 \
  --email you@example.com \
  --agree-tos \
  --deploy-hook "systemctl reload nginx"
```

Then replace the nginx config with HTTPS:

```bash
sudo tee /etc/nginx/sites-available/ai-material-creator >/dev/null <<'EOF'
server {
    listen 80 default_server;
    listen [::]:80 default_server;
    server_name 62.74.214.123 _;

    location ^~ /.well-known/acme-challenge/ {
        root /var/www/letsencrypt;
        try_files $uri =404;
    }

    location / {
        return 301 https://$host$request_uri;
    }
}

server {
    listen 443 ssl http2 default_server;
    listen [::]:443 ssl http2 default_server;
    server_name 62.74.214.123;

    ssl_certificate /etc/letsencrypt/live/62.74.214.123/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/62.74.214.123/privkey.pem;

    root /home/yiannis-apopsi/apps/ai_material_creator_v2/web/dist;
    index index.html;

    client_max_body_size 25m;

    location /api/ {
        proxy_pass http://127.0.0.1:8002;
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

    location / {
        try_files $uri $uri/ /index.html;
    }
}
EOF

sudo nginx -t
sudo systemctl reload nginx
```

HTTPS checks:

```bash
curl -I https://62.74.214.123/
curl -sS https://62.74.214.123/api/health
sudo certbot renew --dry-run
```

## 10. After Code Changes

Frontend:

```bash
cd /home/yiannis-apopsi/apps/ai_material_creator_v2/web
npm run build
sudo systemctl reload nginx
```

Backend:

```bash
sudo systemctl restart edu-backend
```

Research Hub:

```bash
cd /home/yiannis-apopsi/apps/ai_material_creator_v2/research_hub_mcp
source "$HOME/.cargo/env"
cargo build --release
sudo systemctl restart edu-research-hub
```


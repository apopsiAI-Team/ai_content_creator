# Systemd Guide for Local Server

This app runs on the local server with three `systemd` services:

- `edu-research-hub`: Rust Research Hub service on port `8091`
- `edu-backend`: FastAPI backend on port `8002`
- `edu-frontend`: Vite frontend on port `5173`

Local users open:

```text
http://192.168.0.233:5173
```

## Normal Usage

The services are enabled on boot, so after a server restart they should start automatically.

Check if they are running:

```bash
systemctl is-active edu-research-hub edu-backend edu-frontend
```

Expected output:

```text
active
active
active
```

Check if they are enabled on boot:

```bash
systemctl is-enabled edu-research-hub edu-backend edu-frontend
```

Expected output:

```text
enabled
enabled
enabled
```

## Start, Stop, Restart

Start all services:

```bash
sudo systemctl start edu-research-hub edu-backend edu-frontend
```

Stop all services:

```bash
sudo systemctl stop edu-frontend edu-backend edu-research-hub
```

Restart all services:

```bash
sudo systemctl restart edu-research-hub edu-backend edu-frontend
```

Restart only the frontend:

```bash
sudo systemctl restart edu-frontend
```

Restart only the backend:

```bash
sudo systemctl restart edu-backend
```

Restart only Research Hub:

```bash
sudo systemctl restart edu-research-hub
```

## Logs

Follow backend logs:

```bash
journalctl -u edu-backend -f
```

Follow frontend logs:

```bash
journalctl -u edu-frontend -f
```

Follow Research Hub logs:

```bash
journalctl -u edu-research-hub -f
```

Show recent logs for all services:

```bash
journalctl -u edu-research-hub -u edu-backend -u edu-frontend -n 150 --no-pager
```

## Status Details

Show detailed status:

```bash
systemctl status edu-research-hub edu-backend edu-frontend --no-pager
```

Show backend status only:

```bash
systemctl status edu-backend --no-pager
```

## Important Rule

Do not run the old manual dev script while `systemd` is managing the app:

```bash
./start-local-server.sh
```

That script uses the same ports as the services and can cause:

```text
Address already in use
```

If you need manual testing, stop the services first:

```bash
sudo systemctl stop edu-frontend edu-backend edu-research-hub
```

Then run manual commands. When finished, stop the manual processes and start services again:

```bash
sudo systemctl start edu-research-hub edu-backend edu-frontend
```

## Common Problems

### Backend Stuck Activating

Check status:

```bash
systemctl status edu-backend --no-pager
```

Check logs:

```bash
journalctl -u edu-backend -n 120 --no-pager
```

If logs show:

```text
Address already in use
```

then port `8002` is already used by another process.

Find the process:

```bash
sudo ss -ltnp | grep ':8002'
```

If it is an old manual `uvicorn` or `python` process, stop it:

```bash
sudo fuser -k 8002/tcp
```

Then restart backend:

```bash
sudo systemctl restart edu-backend
```

### Frontend Not Loading

Check frontend:

```bash
systemctl status edu-frontend --no-pager
journalctl -u edu-frontend -n 120 --no-pager
```

Check if port `5173` is open:

```bash
sudo ss -ltnp | grep ':5173'
```

If another process uses port `5173`:

```bash
sudo fuser -k 5173/tcp
sudo systemctl restart edu-frontend
```

### Research Hub Not Healthy

Check Research Hub:

```bash
systemctl status edu-research-hub --no-pager
journalctl -u edu-research-hub -n 120 --no-pager
```

Check if port `8091` is open:

```bash
sudo ss -ltnp | grep ':8091'
```

Restart it:

```bash
sudo systemctl restart edu-research-hub
```

## After Code Changes

If you edit code through VS Code Remote SSH:

- Frontend code changes are usually picked up by Vite automatically.
- Backend code changes are usually picked up because `uvicorn` runs with `--reload`.
- If something behaves strangely, restart the relevant service.

Restart backend and frontend:

```bash
sudo systemctl restart edu-backend edu-frontend
```

If you change Rust code:

```bash
cd ~/apps/ai_material_creator_v2/research_hub_mcp
source "$HOME/.cargo/env"
cargo build --release
sudo systemctl restart edu-research-hub
```

## After Syncing From Local Machine

From the local development machine:

```bash
rsync -avz --delete \
  --exclude '.git/' \
  --exclude '.venv/' \
  --exclude 'venv/' \
  --exclude 'node_modules/' \
  --exclude 'dist/' \
  --exclude 'target/' \
  --exclude '__pycache__/' \
  --exclude '*.pyc' \
  --exclude '.env' \
  --exclude '.env.*' \
  ./ yiannis-apopsi@192.168.0.233:/home/yiannis-apopsi/apps/ai_material_creator_v2/
```

Then on the server:

```bash
cd ~/apps/ai_material_creator_v2/web
npm ci
```

If Python dependencies changed:

```bash
cd ~/apps/ai_material_creator_v2
source backend_py/.venv/bin/activate
pip install -e ./backend_py
sudo systemctl restart edu-backend
```

If Rust dependencies or code changed:

```bash
cd ~/apps/ai_material_creator_v2/research_hub_mcp
source "$HOME/.cargo/env"
cargo build --release
sudo systemctl restart edu-research-hub
```

Restart frontend after frontend dependency changes:

```bash
sudo systemctl restart edu-frontend
```

## Disable Auto Start

If the app should not start automatically after reboot:

```bash
sudo systemctl disable edu-research-hub edu-backend edu-frontend
```

This does not stop currently running services.

To disable and stop:

```bash
sudo systemctl disable --now edu-research-hub edu-backend edu-frontend
```

Re-enable auto start:

```bash
sudo systemctl enable --now edu-research-hub edu-backend edu-frontend
```

## Service Files

The service definitions are stored here:

```text
/etc/systemd/system/edu-research-hub.service
/etc/systemd/system/edu-backend.service
/etc/systemd/system/edu-frontend.service
```

After editing any service file:

```bash
sudo systemd-analyze verify \
  /etc/systemd/system/edu-research-hub.service \
  /etc/systemd/system/edu-backend.service \
  /etc/systemd/system/edu-frontend.service
```

If there is no output, the files are valid.

Then reload `systemd`:

```bash
sudo systemctl daemon-reload
```

Restart services:

```bash
sudo systemctl restart edu-research-hub edu-backend edu-frontend
```

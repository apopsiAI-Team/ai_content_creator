#!/usr/bin/env bash
set -euo pipefail

# Load nvm if available so Node version is correct.
export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
if [ -s "$NVM_DIR/nvm.sh" ]; then
  # shellcheck source=/dev/null
  . "$NVM_DIR/nvm.sh"
fi

if command -v nvm >/dev/null 2>&1; then
  nvm use 22.12.0 >/dev/null
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cleanup() {
  if [ -n "${BACKEND_PID:-}" ]; then
    kill "$BACKEND_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

(
  cd "$ROOT_DIR/backend_py"
  uvicorn edu_backend.main:app --host 0.0.0.0 --port 8000 --reload
) &
BACKEND_PID=$!

cd "$ROOT_DIR/web"
npm run dev

#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKEND_DIR="$ROOT_DIR/backend_py"
WEB_DIR="$ROOT_DIR/web"

cleanup() {
  if [ -n "${BACKEND_PID:-}" ]; then
    kill "$BACKEND_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

if [ ! -d "$BACKEND_DIR/.venv" ]; then
  echo "Missing backend virtualenv: $BACKEND_DIR/.venv"
  echo "Create it with: python3 -m venv backend_py/.venv && source backend_py/.venv/bin/activate && pip install -e ./backend_py"
  exit 1
fi

source "$BACKEND_DIR/.venv/bin/activate"

(
  cd "$ROOT_DIR"
  PYTHONPATH="$BACKEND_DIR/src" uvicorn edu_backend.main:app --host 0.0.0.0 --port 8002 --reload
) &
BACKEND_PID=$!

cd "$WEB_DIR"
npm run dev -- --host 0.0.0.0

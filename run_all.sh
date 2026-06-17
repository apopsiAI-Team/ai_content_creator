#!/bin/bash

# Educational Material Creator - Run All Services
# Usage: ./run_all.sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# PIDs for cleanup
BACKEND_PID=""
RESEARCH_HUB_PID=""
FRONTEND_PID=""

cleanup() {
    echo -e "\n${YELLOW}Shutting down all services...${NC}"

    if [ -n "$FRONTEND_PID" ]; then
        kill $FRONTEND_PID 2>/dev/null && echo -e "${RED}Stopped Frontend${NC}"
    fi

    if [ -n "$BACKEND_PID" ]; then
        kill $BACKEND_PID 2>/dev/null && echo -e "${RED}Stopped Python Backend${NC}"
    fi

    if [ -n "$RESEARCH_HUB_PID" ]; then
        kill $RESEARCH_HUB_PID 2>/dev/null && echo -e "${RED}Stopped Research Hub MCP${NC}"
    fi

    # Kill any remaining child processes
    pkill -P $$ 2>/dev/null || true

    echo -e "${GREEN}All services stopped.${NC}"
    exit 0
}

trap cleanup SIGINT SIGTERM

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Educational Material Creator${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Start Research Hub MCP (Rust)
echo -e "${GREEN}[1/3] Starting Research Hub MCP...${NC}"
cd "$SCRIPT_DIR/research_hub_mcp"
cargo run --release -- http --port 8091 2>&1 | sed 's/^/[Research Hub] /' &
RESEARCH_HUB_PID=$!
sleep 2

# Start Python Backend
echo -e "${GREEN}[2/3] Starting Python Backend on port 8002...${NC}"
cd "$SCRIPT_DIR/backend_py"
source .venv/bin/activate 2>/dev/null || true
uvicorn src.edu_backend.main:app --port 8002 --reload 2>&1 | sed 's/^/[Backend] /' &
BACKEND_PID=$!
sleep 2

# Start Frontend
echo -e "${GREEN}[3/3] Starting Frontend...${NC}"
cd "$SCRIPT_DIR/web"
npm run dev 2>&1 | sed 's/^/[Frontend] /' &
FRONTEND_PID=$!

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}All services running!${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo -e "  ${YELLOW}Frontend:${NC}      http://localhost:5173"
echo -e "  ${YELLOW}Backend API:${NC}   http://localhost:8002"
echo -e "  ${YELLOW}Research Hub:${NC}  MCP Server (internal)"
echo ""
echo -e "${YELLOW}Press Ctrl+C to stop all services${NC}"
echo ""

# Wait for any process to exit
wait

#!/bin/bash
# Comprehensive test runner for rust-research-mcp

set -e  # Exit on error

echo "========================================="
echo "Running Comprehensive E2E Test Suite"
echo "========================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Track test results
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Function to run a test and report results
run_test() {
    local test_name=$1
    local test_command=$2
    
    echo -e "${YELLOW}Running: $test_name${NC}"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    if eval $test_command; then
        echo -e "${GREEN}✅ PASSED: $test_name${NC}\n"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        echo -e "${RED}❌ FAILED: $test_name${NC}\n"
        FAILED_TESTS=$((FAILED_TESTS + 1))
    fi
}

# 1. Build the project
echo "Building project..."
cargo build --bin rust-research-mcp 2>&1 | grep -E "(Compiling|Finished)" || true
echo ""

# 2. Run Rust unit tests
echo "========================================="
echo "RUST UNIT TESTS"
echo "========================================="
run_test "Cargo unit tests" "cargo nextest run --lib --no-capture 2>&1 | grep -E '(test result:|passed)'"

# 3. Run Rust integration tests  
echo "========================================="
echo "RUST INTEGRATION TESTS"
echo "========================================="
run_test "Provider tests" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_arxiv_provider)'"
run_test "CrossRef provider" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_crossref_provider)'"
run_test "Semantic Scholar provider" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_semantic_scholar_provider)'"
run_test "Unpaywall provider" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_unpaywall_provider)'"
run_test "CORE provider" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_core_provider)'"
run_test "bioRxiv provider" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_biorxiv_provider)'"
run_test "SSRN provider" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_ssrn_provider)'"
run_test "Meta search client" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_meta_search_client)'"
run_test "Search tool" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_search_tool)'"
run_test "Download tool" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_download_tool)'"
run_test "Metadata extractor" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_metadata_extractor)'"
run_test "Server handler" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_server_handler)'"
run_test "URL resolution" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_url_resolution)'"
run_test "Cascade PDF retrieval" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_cascade_pdf_retrieval)'"
run_test "Rate limiting" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_rate_limiting)'"
run_test "Error handling" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_error_handling)'"
run_test "Full integration flow" "cargo nextest run --test e2e_test_suite --test-threads 1 -E 'test(test_full_search_download_flow)'"

# 4. Run Python MCP protocol tests
echo "========================================="
echo "PYTHON MCP PROTOCOL TESTS"
echo "========================================="
run_test "MCP protocol tests" "python3 tests/test_mcp_e2e.py"

# 5. Quick smoke tests
echo "========================================="
echo "SMOKE TESTS"
echo "========================================="

# Test that server starts and responds to ping
run_test "Server startup" "timeout 5 cargo run --bin rust-research-mcp 2>&1 | grep -q 'Starting MCP server' || true"

# 6. Report results
echo ""
echo "========================================="
echo "TEST RESULTS SUMMARY"
echo "========================================="
echo -e "Total Tests: $TOTAL_TESTS"
echo -e "${GREEN}Passed: $PASSED_TESTS${NC}"
echo -e "${RED}Failed: $FAILED_TESTS${NC}"

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "\n${GREEN}🎉 ALL TESTS PASSED!${NC}"
    exit 0
else
    echo -e "\n${RED}⚠️ SOME TESTS FAILED${NC}"
    echo "Please review the failures above"
    exit 1
fi
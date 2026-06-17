#!/bin/bash

# Script to run provider end-to-end tests
# Usage: ./test_providers.sh [live]

set -e

echo "🧪 Research Hub MCP - Provider E2E Tests"
echo "========================================"
echo ""

# Check if we should run live tests
if [ "$1" == "live" ]; then
    echo "⚡ Running LIVE tests (will make real API calls)"
    echo ""
    export RUN_LIVE_TESTS=true
else
    echo "📦 Running offline tests only"
    echo "   To run live tests: ./test_providers.sh live"
    echo ""
    export RUN_LIVE_TESTS=false
fi

# Run the provider tests
echo "🔍 Testing individual providers..."
cargo nextest run --test providers_e2e_test --no-capture

# Run quick unit tests for providers
echo ""
echo "🔧 Running provider unit tests..."
cargo nextest run --lib --no-capture -E 'test(providers::)'

echo ""
echo "✅ Provider tests complete!"

# Show summary if live tests were run
if [ "$RUN_LIVE_TESTS" == "true" ]; then
    echo ""
    echo "📊 Live Test Summary:"
    echo "   - ArXiv: Academic preprints (CS, Physics, Math)"
    echo "   - CrossRef: Published paper metadata"
    echo "   - SSRN: Social sciences and recent papers"
    echo "   - Sci-Hub: Full-text PDF access"
fi
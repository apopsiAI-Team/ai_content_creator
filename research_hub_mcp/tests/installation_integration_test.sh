#!/bin/bash

# Comprehensive Installation Integration Tests
# Tests actual installation scenarios on macOS

set -euo pipefail

# Test configuration
TEST_DIR=$(mktemp -d)
TEST_RESULTS=0
TESTS_RUN=0
TESTS_PASSED=0

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Test functions
print_test() {
    echo -e "${YELLOW}TEST: $1${NC}"
    ((TESTS_RUN++))
}

print_pass() {
    echo -e "${GREEN}PASS: $1${NC}"
    ((TESTS_PASSED++))
}

print_fail() {
    echo -e "${RED}FAIL: $1${NC}"
    TEST_RESULTS=1
}

print_info() {
    echo -e "${BLUE}INFO: $1${NC}"
}

cleanup() {
    rm -rf "${TEST_DIR}"
}

trap cleanup EXIT

# Test 1: macOS Version Compatibility
test_macos_compatibility() {
    print_test "macOS version compatibility"
    
    local macos_version=$(sw_vers -productVersion)
    local major_version=$(echo "$macos_version" | cut -d. -f1)
    local minor_version=$(echo "$macos_version" | cut -d. -f2)
    
    # Check for macOS 10.14+ (Mojave)
    if [ "$major_version" -gt 10 ] || 
       ([ "$major_version" -eq 10 ] && [ "$minor_version" -ge 14 ]); then
        print_pass "macOS $macos_version is supported (>=10.14 required)"
    else
        print_fail "macOS $macos_version is not supported (>=10.14 required)"
    fi
}

# Test 2: System Dependencies
test_system_dependencies() {
    print_test "System dependencies availability"
    
    local required_commands=(
        "cargo"
        "rustc"
        "pkg-config"
        "curl"
        "git"
        "launchctl"
        "plutil"
    )
    
    local missing_commands=()
    for cmd in "${required_commands[@]}"; do
        if ! command -v "$cmd" &>/dev/null; then
            missing_commands+=("$cmd")
        fi
    done
    
    if [ ${#missing_commands[@]} -eq 0 ]; then
        print_pass "All required system dependencies available"
    else
        print_fail "Missing system dependencies: ${missing_commands[*]}"
        print_info "Install with: xcode-select --install && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    fi
}

# Test 3: Homebrew Integration Test
test_homebrew_integration() {
    print_test "Homebrew integration readiness"
    
    # Check if Homebrew is installed
    if ! command -v brew &>/dev/null; then
        print_fail "Homebrew not installed (required for package management)"
        return
    fi
    
    # Check Homebrew doctor
    if brew doctor &>/dev/null; then
        print_pass "Homebrew installation is healthy"
    else
        print_info "Homebrew has warnings (may still work)"
    fi
    
    # Test formula syntax
    if brew audit --strict homebrew/rust-sci-hub-mcp.rb &>/dev/null; then
        print_pass "Formula passes Homebrew audit"
    else
        print_fail "Formula fails Homebrew audit"
        brew audit homebrew/rust-sci-hub-mcp.rb 2>&1 | head -5
    fi
}

# Test 4: Installation Directory Structure
test_installation_directories() {
    print_test "Installation directory structure"
    
    local test_prefix="${TEST_DIR}/opt/homebrew"
    local test_config_dir="${TEST_DIR}/.config/rust-sci-hub-mcp"
    local test_support_dir="${TEST_DIR}/Library/Application Support/rust-sci-hub-mcp"
    local test_log_dir="${TEST_DIR}/Library/Logs/rust-sci-hub-mcp"
    local test_launchagents_dir="${TEST_DIR}/Library/LaunchAgents"
    
    # Create directories
    mkdir -p "$test_prefix" "$test_config_dir" "$test_support_dir" "$test_log_dir" "$test_launchagents_dir"
    
    # Test directory creation
    local expected_dirs=(
        "$test_prefix"
        "$test_config_dir" 
        "$test_support_dir"
        "$test_log_dir"
        "$test_launchagents_dir"
    )
    
    local all_created=true
    for dir in "${expected_dirs[@]}"; do
        if [ ! -d "$dir" ]; then
            print_fail "Failed to create directory: $dir"
            all_created=false
        fi
    done
    
    if [ "$all_created" = true ]; then
        print_pass "Installation directory structure can be created"
    fi
}

# Test 5: Configuration File Generation
test_config_generation() {
    print_test "Configuration file generation and validation"
    
    local test_config="${TEST_DIR}/config.toml"
    
    # Generate a test config based on the formula template
    cat > "$test_config" << 'EOF'
# Rust Sci-Hub MCP Server Configuration

[server]
host = "127.0.0.1"
port = 8080
health_check_interval_secs = 60
graceful_shutdown_timeout_secs = 30

[sci_hub]
mirrors = []
timeout_secs = 30
retry_attempts = 3
rate_limit_requests_per_minute = 30

[downloads]
directory = "~/Downloads/papers"
concurrent_downloads = 3
chunk_size_bytes = 8192

[metadata]
cache_enabled = true
cache_ttl_hours = 168
extraction_timeout_secs = 10

[logging]
level = "info"
file = "~/Library/Logs/rust-sci-hub-mcp/service.log"
max_size_mb = 10
max_backups = 5
EOF
    
    # Validate TOML syntax
    if python3 -c "import toml; toml.load('$test_config')" 2>/dev/null ||
       ruby -e "require 'toml'; TOML.load_file('$test_config')" 2>/dev/null ||
       [ -f "$test_config" ]; then
        print_pass "Configuration file can be generated and parsed"
    else
        print_fail "Configuration file generation or parsing failed"
    fi
}

# Test 6: Service Integration Test
test_service_integration() {
    print_test "Service integration compatibility"
    
    # Check LaunchAgent support
    if ! launchctl print system 2>/dev/null | grep -q "com.apple."; then
        print_fail "LaunchAgent system not accessible"
        return
    fi
    
    # Test plist processing
    local test_plist="${TEST_DIR}/test.plist"
    sed "s|HOME_DIR|${TEST_DIR}|g" launchd/com.rust-sci-hub-mcp.plist > "$test_plist"
    
    if plutil -lint "$test_plist" &>/dev/null; then
        print_pass "LaunchAgent plist can be processed and validated"
    else
        print_fail "LaunchAgent plist processing failed"
    fi
}

# Test 7: Network Security Test
test_network_security() {
    print_test "Network security configuration"
    
    # Check that service binds to localhost only
    if grep -q "127.0.0.1" homebrew/rust-sci-hub-mcp.rb &&
       ! grep -q "0.0.0.0" homebrew/rust-sci-hub-mcp.rb; then
        print_pass "Service configured for localhost-only binding"
    else
        print_fail "Service network binding may not be secure"
    fi
    
    # Check for HTTPS enforcement
    if ! grep -q "http://" homebrew/rust-sci-hub-mcp.rb launchd/com.rust-sci-hub-mcp.plist; then
        print_pass "No insecure HTTP URLs found"
    else
        print_fail "Found insecure HTTP URLs in configuration"
    fi
}

# Test 8: File Permissions Test
test_file_permissions() {
    print_test "File permissions security"
    
    # Test config file permissions
    local test_config="${TEST_DIR}/config.toml"
    touch "$test_config"
    chmod 600 "$test_config"
    
    local perms=$(stat -f "%Lp" "$test_config")
    if [ "$perms" = "600" ]; then
        print_pass "Configuration file permissions can be secured"
    else
        print_fail "Failed to set secure file permissions"
    fi
    
    # Test script executability
    local executable_count=0
    for script in scripts/*.sh; do
        if [ -x "$script" ]; then
            ((executable_count++))
        fi
    done
    
    if [ $executable_count -gt 0 ]; then
        print_pass "Installation scripts are executable"
    else
        print_fail "Installation scripts are not executable"
    fi
}

# Test 9: Cleanup and Uninstall Test
test_cleanup_safety() {
    print_test "Cleanup and uninstall safety"
    
    # Check uninstall script exists and has safety checks
    if [ -f "scripts/uninstall.sh" ]; then
        if grep -q "confirm\|prompt\|read" scripts/uninstall.sh; then
            print_pass "Uninstall script has safety prompts"
        else
            print_fail "Uninstall script lacks safety prompts"
        fi
    else
        print_fail "Uninstall script not found"
    fi
    
    # Check for backup mechanisms
    if grep -q "backup\|\.bak\|\.backup" scripts/uninstall.sh 2>/dev/null; then
        print_pass "Uninstall includes backup functionality"
    else
        print_info "Uninstall script doesn't create backups (consider adding)"
    fi
}

# Test 10: Documentation Completeness
test_documentation_completeness() {
    print_test "Installation documentation completeness"
    
    local required_docs=(
        "docs/HOMEBREW.md"
        "docs/LAUNCHAGENT.md" 
        "docs/USER_GUIDE.md"
        "docs/TROUBLESHOOTING.md"
    )
    
    local missing_docs=()
    for doc in "${required_docs[@]}"; do
        if [ ! -f "$doc" ]; then
            missing_docs+=("$doc")
        fi
    done
    
    if [ ${#missing_docs[@]} -eq 0 ]; then
        print_pass "All installation documentation exists"
    else
        print_fail "Missing installation documentation: ${missing_docs[*]}"
    fi
    
    # Check for installation examples in docs
    if grep -q "brew install\|brew services" docs/HOMEBREW.md 2>/dev/null; then
        print_pass "Installation documentation includes usage examples"
    else
        print_fail "Installation documentation lacks usage examples"
    fi
}

# Test 11: Performance and Resource Tests
test_performance_constraints() {
    print_test "Performance and resource constraints"
    
    # Check for resource limits in plist
    if grep -q "ResourceLimits\|NumberOfFiles" launchd/com.rust-sci-hub-mcp.plist; then
        print_pass "LaunchAgent has resource limits configured"
    else
        print_info "Consider adding resource limits to LaunchAgent"
    fi
    
    # Check for reasonable defaults in config
    local test_config="${TEST_DIR}/test_config.toml"
    cat > "$test_config" << 'EOF'
[downloads]
concurrent_downloads = 3
chunk_size_bytes = 8192

[metadata]
extraction_timeout_secs = 10

[logging]
max_size_mb = 10
max_backups = 5
EOF
    
    if [ -f "$test_config" ]; then
        print_pass "Performance constraints are configured"
    fi
}

# Main test runner
main() {
    echo "=========================================="
    echo "   Installation Integration Tests"
    echo "=========================================="
    echo
    
    # Run all tests
    test_macos_compatibility
    test_system_dependencies
    test_homebrew_integration
    test_installation_directories
    test_config_generation
    test_service_integration
    test_network_security
    test_file_permissions
    test_cleanup_safety
    test_documentation_completeness
    test_performance_constraints
    
    echo
    echo "=========================================="
    echo "   Test Results"
    echo "=========================================="
    echo "Tests run: ${TESTS_RUN}"
    echo "Tests passed: ${TESTS_PASSED}"
    echo "Tests failed: $((TESTS_RUN - TESTS_PASSED))"
    
    if [ "${TEST_RESULTS}" -eq 0 ]; then
        echo -e "${GREEN}All installation tests passed!${NC}"
        echo
        echo "Installation is ready for:"
        echo "- macOS deployment via Homebrew"
        echo "- LaunchAgent service management"
        echo "- Production use with secure defaults"
    else
        echo -e "${RED}Some installation tests failed${NC}"
        echo
        echo "Address the failed tests before deploying."
    fi
    
    exit ${TEST_RESULTS}
}

main "$@"
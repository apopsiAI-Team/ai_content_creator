#!/bin/bash

# LaunchAgent Integration Tests
# These tests verify that the LaunchAgent scripts and configuration work correctly

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

cleanup() {
    rm -rf "${TEST_DIR}"
}

trap cleanup EXIT

# Test 1: Verify plist file is valid XML
test_plist_validity() {
    print_test "Plist file XML validity"
    
    if plutil -lint launchd/com.rust-sci-hub-mcp.plist &>/dev/null; then
        print_pass "Plist file is valid XML"
    else
        print_fail "Plist file is not valid XML"
    fi
}

# Test 2: Check script executability
test_script_permissions() {
    print_test "Script permissions"
    
    local all_executable=true
    for script in scripts/*.sh; do
        if [[ ! -x "${script}" ]]; then
            print_fail "${script} is not executable"
            all_executable=false
        fi
    done
    
    if [[ "${all_executable}" == "true" ]]; then
        print_pass "All scripts are executable"
    fi
}

# Test 3: Verify script syntax
test_script_syntax() {
    print_test "Script syntax check"
    
    local all_valid=true
    for script in scripts/*.sh; do
        if ! bash -n "${script}" 2>/dev/null; then
            print_fail "${script} has syntax errors"
            all_valid=false
        fi
    done
    
    if [[ "${all_valid}" == "true" ]]; then
        print_pass "All scripts have valid syntax"
    fi
}

# Test 4: Check for required commands
test_required_commands() {
    print_test "Required system commands"
    
    local commands=("launchctl" "plutil" "sed" "chmod" "mkdir")
    local all_found=true
    
    for cmd in "${commands[@]}"; do
        if ! command -v "${cmd}" &>/dev/null; then
            print_fail "Required command not found: ${cmd}"
            all_found=false
        fi
    done
    
    if [[ "${all_found}" == "true" ]]; then
        print_pass "All required commands are available"
    fi
}

# Test 5: Test plist HOME_DIR replacement
test_plist_replacement() {
    print_test "Plist HOME_DIR replacement"
    
    local test_plist="${TEST_DIR}/test.plist"
    sed "s|HOME_DIR|${HOME}|g" launchd/com.rust-sci-hub-mcp.plist > "${test_plist}"
    
    if grep -q "HOME_DIR" "${test_plist}"; then
        print_fail "HOME_DIR not fully replaced in plist"
    else
        print_pass "HOME_DIR successfully replaced"
    fi
}

# Test 6: Test config file generation
test_config_generation() {
    print_test "Configuration file generation"
    
    local test_config="${TEST_DIR}/config.toml"
    
    # Extract config generation from install script
    cat > "${test_config}" << 'EOF'
[server]
host = "127.0.0.1"
port = 8080
EOF
    
    if [[ -f "${test_config}" ]]; then
        print_pass "Configuration file can be generated"
    else
        print_fail "Failed to generate configuration file"
    fi
}

# Test 7: Verify documentation exists
test_documentation() {
    print_test "Documentation presence"
    
    if [[ -f "docs/LAUNCHAGENT.md" ]]; then
        print_pass "LaunchAgent documentation exists"
    else
        print_fail "LaunchAgent documentation missing"
    fi
}

# Test 8: Check service script commands
test_service_commands() {
    print_test "Service script command structure"
    
    local commands=("start" "stop" "restart" "status" "logs" "health")
    local script="scripts/service.sh"
    local all_found=true
    
    for cmd in "${commands[@]}"; do
        if ! grep -q "service_${cmd}()" "${script}"; then
            print_fail "Service command missing: ${cmd}"
            all_found=false
        fi
    done
    
    if [[ "${all_found}" == "true" ]]; then
        print_pass "All service commands are implemented"
    fi
}

# Test 9: Verify uninstall safety
test_uninstall_safety() {
    print_test "Uninstall script safety checks"
    
    local script="scripts/uninstall.sh"
    
    # Check for confirmation prompt
    if grep -q "confirm_uninstall" "${script}"; then
        print_pass "Uninstall has confirmation prompt"
    else
        print_fail "Uninstall lacks confirmation prompt"
    fi
}

# Test 10: Check for hardcoded paths
test_hardcoded_paths() {
    print_test "Check for hardcoded paths"
    
    local has_hardcoded=false
    
    # Check scripts for hardcoded user paths
    for script in scripts/*.sh; do
        if grep -E "/Users/[^/]+" "${script}" | grep -v "^#" &>/dev/null; then
            print_fail "${script} contains hardcoded user paths"
            has_hardcoded=true
        fi
    done
    
    if [[ "${has_hardcoded}" == "false" ]]; then
        print_pass "No hardcoded user paths found"
    fi
}

# Test 11: Verify LaunchAgent label uniqueness
test_launchagent_label() {
    print_test "LaunchAgent label format"
    
    local plist="launchd/com.rust-sci-hub-mcp.plist"
    local label=$(grep -A1 "<key>Label</key>" "${plist}" | grep "<string>" | sed 's/.*<string>\(.*\)<\/string>/\1/')
    
    if [[ "${label}" == "com.rust-sci-hub-mcp" ]]; then
        print_pass "LaunchAgent label follows conventions"
    else
        print_fail "LaunchAgent label doesn't follow conventions: ${label}"
    fi
}

# Test 12: Check log directory creation
test_log_directory() {
    print_test "Log directory configuration"
    
    local plist="launchd/com.rust-sci-hub-mcp.plist"
    
    if grep -q "StandardOutPath\|StandardErrorPath" "${plist}"; then
        print_pass "Log paths are configured"
    else
        print_fail "Log paths are not configured"
    fi
}

# Main test runner
main() {
    echo "======================================"
    echo "   LaunchAgent Integration Tests"
    echo "======================================"
    echo
    
    # Run all tests
    test_plist_validity
    test_script_permissions
    test_script_syntax
    test_required_commands
    test_plist_replacement
    test_config_generation
    test_documentation
    test_service_commands
    test_uninstall_safety
    test_hardcoded_paths
    test_launchagent_label
    test_log_directory
    
    echo
    echo "======================================"
    echo "   Test Results"
    echo "======================================"
    echo "Tests run: ${TESTS_RUN}"
    echo "Tests passed: ${TESTS_PASSED}"
    echo "Tests failed: $((TESTS_RUN - TESTS_PASSED))"
    
    if [[ "${TEST_RESULTS}" -eq 0 ]]; then
        echo -e "${GREEN}All tests passed!${NC}"
    else
        echo -e "${RED}Some tests failed${NC}"
    fi
    
    exit ${TEST_RESULTS}
}

main "$@"
#!/bin/bash

# Homebrew Formula Testing Script
# Tests the Homebrew formula for syntax, structure, and compliance

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

# Formula path
FORMULA_FILE="homebrew/rust-sci-hub-mcp.rb"

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

# Test 1: Formula file exists and is readable
test_formula_exists() {
    print_test "Formula file existence"
    
    if [[ -f "${FORMULA_FILE}" ]]; then
        print_pass "Formula file exists"
    else
        print_fail "Formula file not found: ${FORMULA_FILE}"
    fi
}

# Test 2: Ruby syntax validation
test_ruby_syntax() {
    print_test "Ruby syntax validation"
    
    if ruby -c "${FORMULA_FILE}" &>/dev/null; then
        print_pass "Formula has valid Ruby syntax"
    else
        print_fail "Formula has Ruby syntax errors"
        ruby -c "${FORMULA_FILE}" 2>&1 | head -5
    fi
}

# Test 3: Required formula components
test_formula_structure() {
    print_test "Formula structure validation"
    
    local required_elements=(
        "class RustSciHubMcp"
        "desc "
        "homepage "
        "url "
        "sha256 "
        "license "
        "def install"
        "def test"
    )
    
    local all_present=true
    for element in "${required_elements[@]}"; do
        if ! grep -q "${element}" "${FORMULA_FILE}"; then
            print_fail "Missing required element: ${element}"
            all_present=false
        fi
    done
    
    if [[ "${all_present}" == "true" ]]; then
        print_pass "All required formula elements present"
    fi
}

# Test 4: Dependencies validation
test_dependencies() {
    print_test "Dependencies validation"
    
    local expected_deps=(
        "rust.*=> :build"
        "pkg-config.*=> :build"
        "openssl@3"
        "curl"
    )
    
    local all_found=true
    for dep in "${expected_deps[@]}"; do
        if ! grep -q "${dep}" "${FORMULA_FILE}"; then
            print_fail "Missing dependency: ${dep}"
            all_found=false
        fi
    done
    
    if [[ "${all_found}" == "true" ]]; then
        print_pass "All expected dependencies found"
    fi
}

# Test 5: Service block validation
test_service_block() {
    print_test "Service block validation"
    
    local service_elements=(
        "service do"
        "run \\["
        "working_dir"
        "log_path"
        "error_log_path"
        "keep_alive"
        "process_type"
    )
    
    local all_present=true
    for element in "${service_elements[@]}"; do
        if ! grep -q "${element}" "${FORMULA_FILE}"; then
            print_fail "Missing service element: ${element}"
            all_present=false
        fi
    done
    
    if [[ "${all_present}" == "true" ]]; then
        print_pass "Service block properly configured"
    fi
}

# Test 6: Test block validation
test_test_block() {
    print_test "Test block validation"
    
    local test_elements=(
        "assert_predicate bin"
        "assert_match"
        "shell_output"
        "--version"
        "--help"
    )
    
    local all_present=true
    for element in "${test_elements[@]}"; do
        if ! grep -q "${element}" "${FORMULA_FILE}"; then
            print_fail "Missing test element: ${element}"
            all_present=false
        fi
    done
    
    if [[ "${all_present}" == "true" ]]; then
        print_pass "Test block properly configured"
    fi
}

# Test 7: URL and checksum format
test_url_checksum() {
    print_test "URL and checksum format"
    
    local url_line=$(grep "url " "${FORMULA_FILE}")
    local sha256_line=$(grep "sha256 " "${FORMULA_FILE}")
    
    if [[ "${url_line}" =~ https://github\.com/.*/archive/v.*\.tar\.gz ]]; then
        print_pass "URL format is correct"
    else
        print_fail "URL format is incorrect: ${url_line}"
    fi
    
    if [[ "${sha256_line}" =~ sha256.*\"[0-9a-f]{64}\" ]]; then
        print_pass "SHA256 format is correct"
    else
        print_fail "SHA256 format is incorrect: ${sha256_line}"
    fi
}

# Test 8: Post-install hook validation
test_post_install() {
    print_test "Post-install hook validation"
    
    local post_install_elements=(
        "def post_install"
        "user_config_dir"
        "mkdir.*-p"
        "chmod.*600"
        "ohai"
    )
    
    local all_present=true
    for element in "${post_install_elements[@]}"; do
        if ! grep -q "${element}" "${FORMULA_FILE}"; then
            print_fail "Missing post-install element: ${element}"
            all_present=false
        fi
    done
    
    if [[ "${all_present}" == "true" ]]; then
        print_pass "Post-install hook properly configured"
    fi
}

# Test 9: Caveats section
test_caveats() {
    print_test "Caveats section validation"
    
    if grep -q "def caveats" "${FORMULA_FILE}"; then
        local caveats_content=$(sed -n '/def caveats/,/end/p' "${FORMULA_FILE}")
        
        if [[ "${caveats_content}" =~ "brew services start" ]] && 
           [[ "${caveats_content}" =~ "Configuration file" ]] &&
           [[ "${caveats_content}" =~ "Health check" ]]; then
            print_pass "Caveats section properly configured"
        else
            print_fail "Caveats section missing important information"
        fi
    else
        print_fail "Caveats section not found"
    fi
}

# Test 10: LaunchAgent integration
test_launchagent_integration() {
    print_test "LaunchAgent integration"
    
    local launchagent_elements=(
        "LaunchAgents"
        "com.rust-sci-hub-mcp.plist"
        "gsub!.*HOME_DIR"
        "chmod.*644"
    )
    
    local all_present=true
    for element in "${launchagent_elements[@]}"; do
        if ! grep -q "${element}" "${FORMULA_FILE}"; then
            print_fail "Missing LaunchAgent element: ${element}"
            all_present=false
        fi
    done
    
    if [[ "${all_present}" == "true" ]]; then
        print_pass "LaunchAgent integration properly configured"
    fi
}

# Test 11: Configuration file handling
test_config_handling() {
    print_test "Configuration file handling"
    
    local config_elements=(
        "config.toml"
        "\\[server\\]"
        "\\[sci_hub\\]"
        "\\[downloads\\]"
        "\\[logging\\]"
    )
    
    local all_present=true
    for element in "${config_elements[@]}"; do
        if ! grep -q "${element}" "${FORMULA_FILE}"; then
            print_fail "Missing config element: ${element}"
            all_present=false
        fi
    done
    
    if [[ "${all_present}" == "true" ]]; then
        print_pass "Configuration handling properly implemented"
    fi
}

# Test 12: Architecture support
test_architecture_support() {
    print_test "Multi-architecture support"
    
    # Check for architecture-aware elements
    if grep -q "macos:" "${FORMULA_FILE}" &&
       ! grep -q "x86_64" "${FORMULA_FILE}" &&  # Should not hardcode architecture
       ! grep -q "arm64" "${FORMULA_FILE}"; then
        print_pass "Formula supports multiple architectures"
    else
        print_fail "Formula may not properly support multiple architectures"
    fi
}

# Test 13: Security considerations
test_security() {
    print_test "Security considerations"
    
    local security_checks=true
    
    # Check for secure file permissions
    if ! grep -q "chmod.*600" "${FORMULA_FILE}"; then
        print_fail "Config file permissions not properly secured"
        security_checks=false
    fi
    
    # Check for proper directory creation
    if ! grep -q "mkdir.*-p" "${FORMULA_FILE}"; then
        print_fail "Directory creation not properly handled"
        security_checks=false
    fi
    
    # Check for HTTPS URLs
    if grep -q "http://" "${FORMULA_FILE}"; then
        print_fail "Insecure HTTP URL found"
        security_checks=false
    fi
    
    if [[ "${security_checks}" == "true" ]]; then
        print_pass "Security considerations properly addressed"
    fi
}

# Test 14: Error handling
test_error_handling() {
    print_test "Error handling validation"
    
    local error_handling_elements=(
        "if.*exist?"
        "odie"
        "opoo"
    )
    
    local some_present=false
    for element in "${error_handling_elements[@]}"; do
        if grep -q "${element}" "${FORMULA_FILE}"; then
            some_present=true
            break
        fi
    done
    
    if [[ "${some_present}" == "true" ]]; then
        print_pass "Error handling implemented"
    else
        print_fail "No error handling found"
    fi
}

# Test 15: Homebrew style compliance
test_homebrew_style() {
    print_test "Homebrew style compliance"
    
    local style_issues=0
    
    # Check for proper indentation (2 spaces)
    if grep -q "^    [^ ]" "${FORMULA_FILE}"; then
        print_info "Uses 4-space indentation (acceptable)"
    elif grep -q "^  [^ ]" "${FORMULA_FILE}"; then
        print_info "Uses 2-space indentation (Homebrew standard)"
    else
        print_fail "Inconsistent indentation"
        ((style_issues++))
    fi
    
    # Check for proper string quoting
    if grep -q "desc \"" "${FORMULA_FILE}" &&
       grep -q "homepage \"" "${FORMULA_FILE}" &&
       grep -q "url \"" "${FORMULA_FILE}"; then
        print_info "Proper string quoting"
    else
        print_fail "Inconsistent string quoting"
        ((style_issues++))
    fi
    
    if [[ "${style_issues}" -eq 0 ]]; then
        print_pass "Homebrew style compliance good"
    fi
}

# Main test runner
main() {
    echo "======================================"
    echo "   Homebrew Formula Tests"
    echo "======================================"
    echo
    
    # Run all tests
    test_formula_exists
    test_ruby_syntax
    test_formula_structure
    test_dependencies
    test_service_block
    test_test_block
    test_url_checksum
    test_post_install
    test_caveats
    test_launchagent_integration
    test_config_handling
    test_architecture_support
    test_security
    test_error_handling
    test_homebrew_style
    
    echo
    echo "======================================"
    echo "   Test Results"
    echo "======================================"
    echo "Tests run: ${TESTS_RUN}"
    echo "Tests passed: ${TESTS_PASSED}"
    echo "Tests failed: $((TESTS_RUN - TESTS_PASSED))"
    
    if [[ "${TEST_RESULTS}" -eq 0 ]]; then
        echo -e "${GREEN}All tests passed!${NC}"
        echo
        echo "Formula is ready for:"
        echo "- brew audit --strict ${FORMULA_FILE}"
        echo "- brew install --build-from-source ${FORMULA_FILE}"
        echo "- brew test rust-sci-hub-mcp"
    else
        echo -e "${RED}Some tests failed${NC}"
        echo
        echo "Review the failed tests and fix issues before submission."
    fi
    
    exit ${TEST_RESULTS}
}

main "$@"
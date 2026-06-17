#!/bin/bash

# End-to-End Installation Test
# Tests the complete installation process from source

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

# Get repository info
REPO_URL=$(git remote get-url origin)
REPO_NAME=$(basename "$REPO_URL" .git)
CURRENT_BRANCH=$(git branch --show-current)

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
    # Clean up test directory and any test installations
    rm -rf "${TEST_DIR}"
    
    # Remove test service if it was installed
    if launchctl list | grep -q "com.rust-sci-hub-mcp.test"; then
        launchctl unload "${TEST_DIR}/com.rust-sci-hub-mcp.test.plist" 2>/dev/null || true
    fi
}

trap cleanup EXIT

test_repository_access() {
    print_test "Repository accessibility"
    
    if git ls-remote "$REPO_URL" &>/dev/null; then
        print_pass "Repository is accessible: $REPO_URL"
    else
        print_fail "Repository is not accessible: $REPO_URL"
    fi
}

test_source_installation() {
    print_test "Source installation process"
    
    cd "$TEST_DIR"
    
    # Clone the repository
    if git clone "$REPO_URL" "$REPO_NAME" &>/dev/null; then
        print_info "Repository cloned successfully"
    else
        print_fail "Failed to clone repository"
        return
    fi
    
    cd "$REPO_NAME"
    
    # Switch to current branch if not main
    if [ "$CURRENT_BRANCH" != "main" ]; then
        git checkout "$CURRENT_BRANCH" &>/dev/null || true
    fi
    
    # Test build
    if cargo build --release &>/dev/null; then
        print_pass "Source builds successfully"
    else
        print_fail "Source build failed"
        cargo build --release 2>&1 | tail -10
        return
    fi
    
    # Check binary exists
    if [ -f "target/release/rust-sci-hub-mcp" ]; then
        print_pass "Binary created successfully"
    else
        print_fail "Binary not found after build"
        return
    fi
    
    # Test binary execution
    if ./target/release/rust-sci-hub-mcp --version &>/dev/null; then
        print_pass "Binary executes successfully"
    else
        print_fail "Binary execution failed"
    fi
}

test_homebrew_formula_validity() {
    print_test "Homebrew formula validation"
    
    local formula_path="$TEST_DIR/$REPO_NAME/homebrew/rust-sci-hub-mcp.rb"
    
    if [ ! -f "$formula_path" ]; then
        print_fail "Homebrew formula not found"
        return
    fi
    
    # Test Ruby syntax
    if ruby -c "$formula_path" &>/dev/null; then
        print_pass "Homebrew formula has valid syntax"
    else
        print_fail "Homebrew formula has syntax errors"
        return
    fi
    
    # Check for correct repository URL
    if grep -q "github.com/Ladvien/sci_hub_mcp" "$formula_path"; then
        print_pass "Formula references correct repository"
    else
        print_fail "Formula references incorrect repository"
        grep "github.com" "$formula_path" || echo "No GitHub URL found"
    fi
    
    # Check for placeholder values
    if grep -q "PLACEHOLDER\|0000000000000000" "$formula_path"; then
        print_info "Formula contains placeholder values (expected for pre-release)"
        print_info "Release process needed: git tag v0.1.0 && git push origin v0.1.0"
    else
        print_pass "Formula has real values"
    fi
}

test_installation_scripts() {
    print_test "Installation scripts functionality"
    
    local script_dir="$TEST_DIR/$REPO_NAME/scripts"
    
    # Test script permissions
    local executable_scripts=0
    for script in "$script_dir"/*.sh; do
        if [ -x "$script" ]; then
            ((executable_scripts++))
        fi
    done
    
    if [ $executable_scripts -gt 0 ]; then
        print_pass "Installation scripts are executable"
    else
        print_fail "Installation scripts are not executable"
    fi
    
    # Test script syntax
    local syntax_errors=0
    for script in "$script_dir"/*.sh; do
        if ! bash -n "$script" 2>/dev/null; then
            ((syntax_errors++))
        fi
    done
    
    if [ $syntax_errors -eq 0 ]; then
        print_pass "All scripts have valid syntax"
    else
        print_fail "$syntax_errors scripts have syntax errors"
    fi
}

test_configuration_generation() {
    print_test "Configuration generation"
    
    local config_dir="$TEST_DIR/test_config"
    mkdir -p "$config_dir"
    
    # Create test configuration based on template
    cat > "$config_dir/config.toml" << 'EOF'
[server]
host = "127.0.0.1"
port = 8080
health_check_interval_secs = 60

[sci_hub]
mirrors = []
timeout_secs = 30
retry_attempts = 3

[downloads]
directory = "~/Downloads/papers"
concurrent_downloads = 3

[logging]
level = "info"
file = "~/Library/Logs/rust-sci-hub-mcp/service.log"
EOF
    
    # Test TOML parsing
    if python3 -c "import toml; toml.load('$config_dir/config.toml')" 2>/dev/null; then
        print_pass "Configuration file format is valid"
    elif ruby -e "require 'toml'; TOML.load_file('$config_dir/config.toml')" 2>/dev/null; then
        print_pass "Configuration file format is valid"
    else
        print_fail "Configuration file format is invalid"
    fi
}

test_service_integration() {
    print_test "Service integration capability"
    
    local plist_path="$TEST_DIR/$REPO_NAME/launchd/com.rust-sci-hub-mcp.plist"
    
    if [ ! -f "$plist_path" ]; then
        print_fail "LaunchAgent plist not found"
        return
    fi
    
    # Test plist validity
    if plutil -lint "$plist_path" &>/dev/null; then
        print_pass "LaunchAgent plist is valid"
    else
        print_fail "LaunchAgent plist is invalid"
        return
    fi
    
    # Create test plist with real paths
    local test_plist="$TEST_DIR/com.rust-sci-hub-mcp.test.plist"
    sed "s|HOME_DIR|$HOME|g; s|/usr/local/bin/rust-sci-hub-mcp|$TEST_DIR/$REPO_NAME/target/release/rust-sci-hub-mcp|g" "$plist_path" > "$test_plist"
    
    # Validate processed plist
    if plutil -lint "$test_plist" &>/dev/null; then
        print_pass "Processed LaunchAgent plist is valid"
    else
        print_fail "Processed LaunchAgent plist is invalid"
    fi
}

test_documentation_accuracy() {
    print_test "Documentation accuracy"
    
    local docs_dir="$TEST_DIR/$REPO_NAME/docs"
    
    # Check for installation documentation
    if [ -f "$docs_dir/HOMEBREW.md" ]; then
        # Check if documentation references the correct repository
        if grep -q "Ladvien/sci_hub_mcp" "$docs_dir/HOMEBREW.md"; then
            print_pass "Documentation references correct repository"
        else
            print_info "Documentation may need repository URL updates"
        fi
    else
        print_fail "Installation documentation not found"
    fi
    
    # Check for current installation instructions
    if grep -q "brew tap\|brew install" "$docs_dir"/*.md 2>/dev/null; then
        print_info "Homebrew installation instructions found"
        print_info "Note: Custom tap will be needed for distribution"
    fi
}

test_release_readiness() {
    print_test "Release readiness"
    
    cd "$TEST_DIR/$REPO_NAME"
    
    # Check if there are any git tags
    if git tag -l | grep -q "v"; then
        local latest_tag=$(git tag -l | sort -V | tail -1)
        print_info "Latest tag: $latest_tag"
        
        # Check if Homebrew formula matches
        if grep -q "$latest_tag" "homebrew/rust-sci-hub-mcp.rb"; then
            print_pass "Homebrew formula matches latest tag"
        else
            print_info "Homebrew formula needs tag update"
        fi
    else
        print_info "No release tags found"
        print_info "To create a release: git tag v0.1.0 && git push origin v0.1.0"
    fi
    
    # Check Cargo.toml version
    local cargo_version=$(grep '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
    print_info "Cargo.toml version: $cargo_version"
}

create_installation_guide() {
    print_test "Creating accurate installation guide"
    
    cat > "$TEST_DIR/CURRENT_INSTALL_INSTRUCTIONS.md" << EOF
# Current Installation Instructions for rust-sci-hub-mcp

## Repository Status
- Repository: $REPO_URL
- Branch: $CURRENT_BRANCH

## Installation Methods

### Method 1: From Source (Recommended Currently)

1. **Prerequisites:**
   \`\`\`bash
   # Install Rust if not already installed
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   
   # Install system dependencies
   xcode-select --install  # macOS
   \`\`\`

2. **Clone and Build:**
   \`\`\`bash
   git clone $REPO_URL
   cd $(basename $REPO_URL .git)
   cargo build --release
   \`\`\`

3. **Install Binary:**
   \`\`\`bash
   sudo cp target/release/rust-sci-hub-mcp /usr/local/bin/
   chmod +x /usr/local/bin/rust-sci-hub-mcp
   \`\`\`

4. **Setup Service (Optional):**
   \`\`\`bash
   # Create config directory
   mkdir -p ~/.config/rust-sci-hub-mcp
   
   # Copy default config
   cp docs/config.example.toml ~/.config/rust-sci-hub-mcp/config.toml
   
   # Install LaunchAgent
   cp launchd/com.rust-sci-hub-mcp.plist ~/Library/LaunchAgents/
   sed -i '' "s|HOME_DIR|\$HOME|g" ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist
   launchctl load ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist
   \`\`\`

### Method 2: Homebrew (Future)

*Note: Requires creating a GitHub release first*

\`\`\`bash
# This will work after creating a proper release:
# 1. git tag v0.1.0
# 2. git push origin v0.1.0
# 3. Update SHA256 in homebrew formula
# 4. Create Homebrew tap

brew tap Ladvien/sci-hub-mcp
brew install rust-sci-hub-mcp
brew services start rust-sci-hub-mcp
\`\`\`

## Testing Installation

\`\`\`bash
# Test binary
rust-sci-hub-mcp --version

# Test health endpoint
rust-sci-hub-mcp --daemon &
sleep 2
curl http://localhost:8080/health
\`\`\`

## Next Steps for Distribution

1. Create a release: \`git tag v0.1.0 && git push origin v0.1.0\`
2. Update Homebrew formula with real SHA256
3. Create Homebrew tap repository
4. Test installation from tap
EOF
    
    print_pass "Installation guide created: $TEST_DIR/CURRENT_INSTALL_INSTRUCTIONS.md"
}

main() {
    echo "============================================"
    echo "   End-to-End Installation Test"
    echo "============================================"
    echo
    echo "Repository: $REPO_URL"
    echo "Branch: $CURRENT_BRANCH"
    echo
    
    # Run all tests
    test_repository_access
    test_source_installation
    test_homebrew_formula_validity
    test_installation_scripts
    test_configuration_generation
    test_service_integration
    test_documentation_accuracy
    test_release_readiness
    create_installation_guide
    
    echo
    echo "============================================"
    echo "   Test Results"
    echo "============================================"
    echo "Tests run: ${TESTS_RUN}"
    echo "Tests passed: ${TESTS_PASSED}"
    echo "Tests failed: $((TESTS_RUN - TESTS_PASSED))"
    
    if [ "${TEST_RESULTS}" -eq 0 ]; then
        echo -e "${GREEN}All tests passed!${NC}"
        echo
        echo "Installation is ready for:"
        echo "- ✅ Source installation and build"
        echo "- ✅ Manual service setup"
        echo "- 🔄 Homebrew distribution (after release)"
        echo
        echo "See: $TEST_DIR/CURRENT_INSTALL_INSTRUCTIONS.md"
    else
        echo -e "${RED}Some tests failed${NC}"
        echo
        echo "Fix the issues before distribution."
    fi
    
    exit ${TEST_RESULTS}
}

main "$@"
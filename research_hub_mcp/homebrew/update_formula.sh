#!/bin/bash

# Homebrew Formula Update Script
# Updates the Homebrew formula with new version and checksums

set -euo pipefail

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
FORMULA_FILE="homebrew/rust-sci-hub-mcp.rb"
RELEASE_CONFIG="homebrew/release.toml"
CHECKSUMS_FILE="homebrew/checksums.txt"

print_error() {
    echo -e "${RED}ERROR: $1${NC}" >&2
}

print_success() {
    echo -e "${GREEN}SUCCESS: $1${NC}"
}

print_info() {
    echo -e "${BLUE}INFO: $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}WARNING: $1${NC}"
}

usage() {
    cat << EOF
Usage: $0 <version> [options]

Updates the Homebrew formula with a new version and checksums.

Arguments:
    version     New version number (e.g., 1.0.0)

Options:
    -u, --url       Override the download URL
    -s, --sha256    Provide SHA256 checksum directly
    -d, --dry-run   Show what would be changed without modifying files
    -h, --help      Show this help message

Examples:
    $0 1.0.0
    $0 1.0.0 --sha256 abc123...
    $0 1.0.0 --url https://github.com/user/repo/archive/v1.0.0.tar.gz
    $0 1.0.0 --dry-run

EOF
}

validate_version() {
    local version="$1"
    
    if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.-]+)?$ ]]; then
        print_error "Invalid version format. Expected: x.y.z or x.y.z-suffix"
        exit 1
    fi
}

get_current_version() {
    if [[ ! -f "$FORMULA_FILE" ]]; then
        print_error "Formula file not found: $FORMULA_FILE"
        exit 1
    fi
    
    local current_version
    current_version=$(grep -E '^\s*url\s+' "$FORMULA_FILE" | sed -n 's/.*\/v\([0-9.]*\)\.tar\.gz.*/\1/p')
    
    if [[ -z "$current_version" ]]; then
        print_warning "Could not determine current version from formula"
        echo "unknown"
    else
        echo "$current_version"
    fi
}

generate_download_url() {
    local version="$1"
    local base_url
    
    # Extract base repository URL from current formula
    base_url=$(grep -E '^\s*homepage\s+' "$FORMULA_FILE" | sed 's/.*"\(.*\)"/\1/')
    
    if [[ -z "$base_url" ]]; then
        print_error "Could not determine repository URL from formula"
        exit 1
    fi
    
    echo "${base_url}/archive/v${version}.tar.gz"
}

calculate_sha256() {
    local url="$1"
    local temp_file
    
    print_info "Downloading archive to calculate SHA256..."
    temp_file=$(mktemp)
    
    if curl -sL "$url" -o "$temp_file"; then
        local sha256
        sha256=$(shasum -a 256 "$temp_file" | cut -d' ' -f1)
        rm -f "$temp_file"
        echo "$sha256"
    else
        rm -f "$temp_file"
        print_error "Failed to download archive from: $url"
        exit 1
    fi
}

update_formula() {
    local version="$1"
    local url="$2"
    local sha256="$3"
    local dry_run="$4"
    
    if [[ "$dry_run" == "true" ]]; then
        print_info "DRY RUN: Would update formula with:"
        echo "  Version: $version"
        echo "  URL: $url"
        echo "  SHA256: $sha256"
        return 0
    fi
    
    print_info "Updating formula..."
    
    # Create backup
    local backup_file="${FORMULA_FILE}.backup.$(date +%Y%m%d_%H%M%S)"
    cp "$FORMULA_FILE" "$backup_file"
    print_info "Backup created: $backup_file"
    
    # Update URL
    sed -i.tmp "s|url \".*\"|url \"$url\"|" "$FORMULA_FILE"
    
    # Update SHA256
    sed -i.tmp "s|sha256 \".*\"|sha256 \"$sha256\"|" "$FORMULA_FILE"
    
    # Clean up temp file
    rm -f "${FORMULA_FILE}.tmp"
    
    print_success "Formula updated successfully"
}

validate_formula() {
    print_info "Validating updated formula..."
    
    # Check if brew is available
    if ! command -v brew &> /dev/null; then
        print_warning "Homebrew not found, skipping formula validation"
        return 0
    fi
    
    # Run basic syntax check
    if brew audit --formula "$FORMULA_FILE" 2>/dev/null; then
        print_success "Formula passes audit"
    else
        print_warning "Formula audit failed - please review manually"
    fi
}

update_checksums_file() {
    local version="$1"
    local sha256="$2"
    local dry_run="$3"
    
    if [[ "$dry_run" == "true" ]]; then
        print_info "DRY RUN: Would update checksums file"
        return 0
    fi
    
    print_info "Updating checksums file..."
    
    # Create checksums directory if it doesn't exist
    mkdir -p "$(dirname "$CHECKSUMS_FILE")"
    
    # Add entry to checksums file
    echo "$version $sha256" >> "$CHECKSUMS_FILE"
    
    print_success "Checksums file updated"
}

create_release_notes() {
    local version="$1"
    local dry_run="$2"
    
    if [[ "$dry_run" == "true" ]]; then
        print_info "DRY RUN: Would create release notes"
        return 0
    fi
    
    local notes_file="homebrew/RELEASE_NOTES_${version}.md"
    
    cat > "$notes_file" << EOF
# Release Notes - Version $version

## Changes in this Release

- Updated Homebrew formula to version $version
- [Add specific changes here]

## Installation

\`\`\`bash
brew install yourusername/rust-sci-hub-mcp/rust-sci-hub-mcp
\`\`\`

## Upgrade from Previous Version

\`\`\`bash
brew upgrade rust-sci-hub-mcp
\`\`\`

## Service Management

\`\`\`bash
# Start the service
brew services start rust-sci-hub-mcp

# Check status
brew services list | grep rust-sci-hub-mcp

# Stop the service
brew services stop rust-sci-hub-mcp
\`\`\`

## Health Check

\`\`\`bash
curl http://localhost:8090/health
\`\`\`

EOF

    print_success "Release notes created: $notes_file"
}

main() {
    local version=""
    local custom_url=""
    local custom_sha256=""
    local dry_run="false"
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -u|--url)
                custom_url="$2"
                shift 2
                ;;
            -s|--sha256)
                custom_sha256="$2"
                shift 2
                ;;
            -d|--dry-run)
                dry_run="true"
                shift
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            -*)
                print_error "Unknown option: $1"
                usage
                exit 1
                ;;
            *)
                if [[ -z "$version" ]]; then
                    version="$1"
                else
                    print_error "Unexpected argument: $1"
                    usage
                    exit 1
                fi
                shift
                ;;
        esac
    done
    
    # Validate required arguments
    if [[ -z "$version" ]]; then
        print_error "Version is required"
        usage
        exit 1
    fi
    
    validate_version "$version"
    
    # Show current state
    local current_version
    current_version=$(get_current_version)
    print_info "Current version: $current_version"
    print_info "New version: $version"
    
    # Generate or use provided URL
    local url
    if [[ -n "$custom_url" ]]; then
        url="$custom_url"
        print_info "Using custom URL: $url"
    else
        url=$(generate_download_url "$version")
        print_info "Generated URL: $url"
    fi
    
    # Calculate or use provided SHA256
    local sha256
    if [[ -n "$custom_sha256" ]]; then
        sha256="$custom_sha256"
        print_info "Using provided SHA256: $sha256"
    else
        sha256=$(calculate_sha256 "$url")
        print_info "Calculated SHA256: $sha256"
    fi
    
    # Update formula
    update_formula "$version" "$url" "$sha256" "$dry_run"
    
    # Update checksums file
    update_checksums_file "$version" "$sha256" "$dry_run"
    
    # Create release notes
    create_release_notes "$version" "$dry_run"
    
    # Validate formula
    if [[ "$dry_run" == "false" ]]; then
        validate_formula
    fi
    
    print_success "Formula update complete!"
    
    if [[ "$dry_run" == "false" ]]; then
        echo
        echo "Next steps:"
        echo "1. Review the updated formula: $FORMULA_FILE"
        echo "2. Test the formula: brew install --build-from-source $FORMULA_FILE"
        echo "3. Commit and push changes"
        echo "4. Create GitHub release with tag v$version"
    fi
}

main "$@"
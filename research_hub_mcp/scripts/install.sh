#!/bin/bash

# Rust Sci-Hub MCP Server - macOS Installation Script
# This script installs the rust-sci-hub-mcp server as a LaunchAgent on macOS

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SERVICE_NAME="rust-sci-hub-mcp"
PLIST_NAME="com.${SERVICE_NAME}.plist"
BINARY_NAME="rust-sci-hub-mcp"
CONFIG_DIR="${HOME}/.config/${SERVICE_NAME}"
SUPPORT_DIR="${HOME}/Library/Application Support/${SERVICE_NAME}"
LOG_DIR="${HOME}/Library/Logs/${SERVICE_NAME}"
LAUNCHAGENT_DIR="${HOME}/Library/LaunchAgents"

# Functions
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

check_requirements() {
    print_info "Checking system requirements..."
    
    # Check if running on macOS
    if [[ "$(uname)" != "Darwin" ]]; then
        print_error "This script is only for macOS systems"
        exit 1
    fi
    
    # Check if binary exists
    if [[ ! -f "target/release/${BINARY_NAME}" ]] && ! command -v "${BINARY_NAME}" &> /dev/null; then
        print_warning "Binary not found. Building from source..."
        cargo build --release
        if [[ ! -f "target/release/${BINARY_NAME}" ]]; then
            print_error "Failed to build ${BINARY_NAME}"
            exit 1
        fi
    fi
    
    # Check for existing installation
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        print_warning "Service ${SERVICE_NAME} is already installed and running"
        read -p "Do you want to reinstall? (y/n): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            print_info "Installation cancelled"
            exit 0
        fi
        # Unload existing service
        launchctl unload "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null || true
    fi
    
    print_success "System requirements check passed"
}

create_directories() {
    print_info "Creating necessary directories..."
    
    # Create config directory
    mkdir -p "${CONFIG_DIR}"
    print_info "Created config directory: ${CONFIG_DIR}"
    
    # Create application support directory
    mkdir -p "${SUPPORT_DIR}"
    print_info "Created support directory: ${SUPPORT_DIR}"
    
    # Create log directory
    mkdir -p "${LOG_DIR}"
    print_info "Created log directory: ${LOG_DIR}"
    
    # Create LaunchAgents directory if it doesn't exist
    mkdir -p "${LAUNCHAGENT_DIR}"
    print_info "Created LaunchAgents directory: ${LAUNCHAGENT_DIR}"
    
    print_success "Directories created successfully"
}

install_binary() {
    print_info "Installing binary..."
    
    local source_binary=""
    
    # Find the binary
    if [[ -f "target/release/${BINARY_NAME}" ]]; then
        source_binary="target/release/${BINARY_NAME}"
    elif command -v "${BINARY_NAME}" &> /dev/null; then
        source_binary=$(which "${BINARY_NAME}")
        print_info "Using existing binary: ${source_binary}"
    else
        print_error "Binary not found"
        exit 1
    fi
    
    # Copy binary to /usr/local/bin
    if [[ ! -d "/usr/local/bin" ]]; then
        print_info "Creating /usr/local/bin directory..."
        if ! sudo mkdir -p /usr/local/bin; then
            print_error "Failed to create /usr/local/bin directory"
            print_info "You may need to enter your administrator password"
            exit 1
        fi
    fi
    
    print_info "Installing binary to /usr/local/bin/${BINARY_NAME}..."
    print_info "This requires administrator privileges..."
    
    if ! sudo cp "${source_binary}" "/usr/local/bin/${BINARY_NAME}"; then
        print_error "Failed to copy binary to /usr/local/bin"
        exit 1
    fi
    
    if ! sudo chmod 755 "/usr/local/bin/${BINARY_NAME}"; then
        print_error "Failed to set binary permissions"
        exit 1
    fi
    
    # Verify installation
    if [[ -f "/usr/local/bin/${BINARY_NAME}" ]]; then
        print_success "Binary installed successfully"
        /usr/local/bin/${BINARY_NAME} --version || true
    else
        print_error "Failed to install binary"
        exit 1
    fi
}

install_config() {
    print_info "Setting up configuration..."
    
    # Check if config already exists
    if [[ -f "${CONFIG_DIR}/config.toml" ]]; then
        print_warning "Configuration file already exists at ${CONFIG_DIR}/config.toml"
        read -p "Do you want to overwrite it? (y/n): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            print_info "Keeping existing configuration"
            return
        fi
    fi
    
    # Generate default configuration
    print_info "Generating default configuration..."
    cat > "${CONFIG_DIR}/config.toml" << 'EOF'
# Rust Sci-Hub MCP Server Configuration

[server]
host = "127.0.0.1"
port = 8080
health_check_interval_secs = 60
graceful_shutdown_timeout_secs = 30

[sci_hub]
# Sci-Hub mirror URLs (will be auto-discovered if empty)
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
cache_ttl_hours = 168  # 1 week
extraction_timeout_secs = 10

[logging]
level = "info"
file = "~/Library/Logs/rust-sci-hub-mcp/service.log"
max_size_mb = 10
max_backups = 5
EOF
    
    print_success "Configuration file created at ${CONFIG_DIR}/config.toml"
}

install_launchagent() {
    print_info "Installing LaunchAgent..."
    
    local plist_source="launchd/${PLIST_NAME}"
    local plist_dest="${LAUNCHAGENT_DIR}/${PLIST_NAME}"
    
    if [[ ! -f "${plist_source}" ]]; then
        print_error "LaunchAgent plist file not found at ${plist_source}"
        exit 1
    fi
    
    # Copy and process the plist file
    print_info "Processing plist file..."
    
    # Read the plist, replace HOME_DIR with actual HOME path, and write to destination
    sed "s|HOME_DIR|${HOME}|g" "${plist_source}" > "${plist_dest}"
    
    # Set proper permissions
    chmod 644 "${plist_dest}"
    
    print_success "LaunchAgent plist installed at ${plist_dest}"
}

load_service() {
    print_info "Loading service..."
    
    # Load the LaunchAgent
    if launchctl load -w "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null; then
        print_success "Service loaded successfully"
    else
        print_warning "Service may already be loaded, attempting to restart..."
        launchctl unload "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null || true
        sleep 1
        launchctl load -w "${LAUNCHAGENT_DIR}/${PLIST_NAME}"
        print_success "Service reloaded successfully"
    fi
    
    # Verify service is running
    sleep 2
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        print_success "Service ${SERVICE_NAME} is running"
        
        # Show service status
        print_info "Service status:"
        launchctl list | grep "${SERVICE_NAME}" || true
    else
        print_error "Service failed to start"
        print_info "Check logs at: ${LOG_DIR}"
        exit 1
    fi
}

verify_installation() {
    print_info "Verifying installation..."
    
    local all_good=true
    
    # Check binary
    if [[ -f "/usr/local/bin/${BINARY_NAME}" ]]; then
        print_success "Binary installed: /usr/local/bin/${BINARY_NAME}"
    else
        print_error "Binary not found"
        all_good=false
    fi
    
    # Check config
    if [[ -f "${CONFIG_DIR}/config.toml" ]]; then
        print_success "Configuration found: ${CONFIG_DIR}/config.toml"
    else
        print_error "Configuration not found"
        all_good=false
    fi
    
    # Check LaunchAgent
    if [[ -f "${LAUNCHAGENT_DIR}/${PLIST_NAME}" ]]; then
        print_success "LaunchAgent installed: ${LAUNCHAGENT_DIR}/${PLIST_NAME}"
    else
        print_error "LaunchAgent not found"
        all_good=false
    fi
    
    # Check if service is loaded
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        print_success "Service is loaded and running"
    else
        print_error "Service is not running"
        all_good=false
    fi
    
    # Check health endpoint
    if command -v curl &> /dev/null; then
        print_info "Checking health endpoint..."
        if curl -s -f http://localhost:8090/health > /dev/null 2>&1; then
            print_success "Health endpoint is responding"
        else
            print_warning "Health endpoint not responding (service may still be starting)"
        fi
    fi
    
    if [[ "${all_good}" == "true" ]]; then
        print_success "Installation verification completed successfully"
    else
        print_warning "Some installation checks failed"
    fi
}

print_post_install() {
    echo
    echo "========================================="
    echo "   Installation Complete!"
    echo "========================================="
    echo
    echo "The ${SERVICE_NAME} service has been installed and started."
    echo
    echo "Useful commands:"
    echo "  - Check status:  launchctl list | grep ${SERVICE_NAME}"
    echo "  - View logs:     tail -f '${LOG_DIR}/stderr.log'"
    echo "  - Stop service:  launchctl unload '${LAUNCHAGENT_DIR}/${PLIST_NAME}'"
    echo "  - Start service: launchctl load '${LAUNCHAGENT_DIR}/${PLIST_NAME}'"
    echo "  - Uninstall:     ./scripts/uninstall.sh"
    echo
    echo "Configuration file: ${CONFIG_DIR}/config.toml"
    echo "Log directory:      ${LOG_DIR}"
    echo "PID file:          ${SUPPORT_DIR}/${SERVICE_NAME}.pid"
    echo
    echo "Health check URL:   http://localhost:8090/health"
    echo
}

# Main installation flow
main() {
    echo "========================================="
    echo "   Rust Sci-Hub MCP Server Installer"
    echo "========================================="
    echo
    
    check_requirements
    create_directories
    install_binary
    install_config
    install_launchagent
    load_service
    verify_installation
    print_post_install
}

# Run main function
main "$@"
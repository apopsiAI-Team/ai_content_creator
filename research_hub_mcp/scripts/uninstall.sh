#!/bin/bash

# Rust Sci-Hub MCP Server - macOS Uninstallation Script
# This script removes the rust-sci-hub-mcp server LaunchAgent from macOS

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

confirm_uninstall() {
    echo "========================================="
    echo "   Rust Sci-Hub MCP Server Uninstaller"
    echo "========================================="
    echo
    print_warning "This will remove the following:"
    echo "  - LaunchAgent service"
    echo "  - Binary from /usr/local/bin"
    echo "  - Configuration files (optional)"
    echo "  - Log files (optional)"
    echo "  - Application support files"
    echo
    read -p "Are you sure you want to uninstall? (y/n): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_info "Uninstallation cancelled"
        exit 0
    fi
}

stop_service() {
    print_info "Stopping service..."
    
    # Check if service is loaded
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        # Unload the service
        if launchctl unload "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null; then
            print_success "Service stopped successfully"
        else
            print_warning "Service may have already been stopped"
        fi
    else
        print_info "Service is not running"
    fi
    
    # Wait for service to stop
    sleep 2
    
    # Kill any remaining processes
    if pgrep -x "${BINARY_NAME}" > /dev/null; then
        print_warning "Found running ${BINARY_NAME} processes, terminating..."
        pkill -x "${BINARY_NAME}" || true
        sleep 1
        
        # Force kill if still running
        if pgrep -x "${BINARY_NAME}" > /dev/null; then
            print_warning "Force killing remaining processes..."
            pkill -9 -x "${BINARY_NAME}" || true
        fi
    fi
    
    print_success "Service stopped"
}

remove_launchagent() {
    print_info "Removing LaunchAgent..."
    
    local plist_path="${LAUNCHAGENT_DIR}/${PLIST_NAME}"
    
    if [[ -f "${plist_path}" ]]; then
        rm -f "${plist_path}"
        print_success "LaunchAgent removed: ${plist_path}"
    else
        print_info "LaunchAgent not found: ${plist_path}"
    fi
    
    # Remove from launchctl database
    launchctl remove "${SERVICE_NAME}" 2>/dev/null || true
}

remove_binary() {
    print_info "Removing binary..."
    
    local binary_path="/usr/local/bin/${BINARY_NAME}"
    
    if [[ -f "${binary_path}" ]]; then
        if sudo rm -f "${binary_path}"; then
            print_success "Binary removed: ${binary_path}"
        else
            print_error "Failed to remove binary: ${binary_path}"
        fi
    else
        print_info "Binary not found: ${binary_path}"
    fi
}

remove_config() {
    if [[ -d "${CONFIG_DIR}" ]]; then
        print_warning "Configuration directory found: ${CONFIG_DIR}"
        read -p "Do you want to remove configuration files? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            # Backup config before removing
            local backup_name="${CONFIG_DIR}.backup.$(date +%Y%m%d_%H%M%S)"
            mv "${CONFIG_DIR}" "${backup_name}"
            print_success "Configuration backed up to: ${backup_name}"
            print_info "Configuration removed"
        else
            print_info "Configuration files preserved"
        fi
    else
        print_info "No configuration directory found"
    fi
}

remove_logs() {
    if [[ -d "${LOG_DIR}" ]]; then
        print_warning "Log directory found: ${LOG_DIR}"
        read -p "Do you want to remove log files? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            # Backup logs before removing
            local backup_name="${LOG_DIR}.backup.$(date +%Y%m%d_%H%M%S)"
            mv "${LOG_DIR}" "${backup_name}"
            print_success "Logs backed up to: ${backup_name}"
            print_info "Log files removed"
        else
            print_info "Log files preserved"
        fi
    else
        print_info "No log directory found"
    fi
}

remove_support_files() {
    print_info "Removing application support files..."
    
    if [[ -d "${SUPPORT_DIR}" ]]; then
        # Check for PID file
        local pid_file="${SUPPORT_DIR}/${SERVICE_NAME}.pid"
        if [[ -f "${pid_file}" ]]; then
            print_info "Removing PID file: ${pid_file}"
            rm -f "${pid_file}"
        fi
        
        # Remove support directory if empty
        if [[ -z "$(ls -A "${SUPPORT_DIR}")" ]]; then
            rmdir "${SUPPORT_DIR}"
            print_success "Support directory removed: ${SUPPORT_DIR}"
        else
            print_warning "Support directory not empty, preserving: ${SUPPORT_DIR}"
        fi
    else
        print_info "No support directory found"
    fi
}

verify_uninstall() {
    print_info "Verifying uninstallation..."
    
    local all_removed=true
    
    # Check service
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        print_error "Service still appears to be loaded"
        all_removed=false
    else
        print_success "Service removed from launchctl"
    fi
    
    # Check LaunchAgent
    if [[ -f "${LAUNCHAGENT_DIR}/${PLIST_NAME}" ]]; then
        print_error "LaunchAgent file still exists"
        all_removed=false
    else
        print_success "LaunchAgent file removed"
    fi
    
    # Check binary
    if [[ -f "/usr/local/bin/${BINARY_NAME}" ]]; then
        print_error "Binary still exists"
        all_removed=false
    else
        print_success "Binary removed"
    fi
    
    # Check processes
    if pgrep -x "${BINARY_NAME}" > /dev/null; then
        print_error "Process still running"
        all_removed=false
    else
        print_success "No running processes found"
    fi
    
    if [[ "${all_removed}" == "true" ]]; then
        print_success "Uninstallation verification completed successfully"
    else
        print_warning "Some components may not have been fully removed"
    fi
}

print_post_uninstall() {
    echo
    echo "========================================="
    echo "   Uninstallation Complete"
    echo "========================================="
    echo
    echo "The ${SERVICE_NAME} service has been uninstalled."
    echo
    
    # Check if config was preserved
    if [[ -d "${CONFIG_DIR}" ]]; then
        echo "Configuration preserved at: ${CONFIG_DIR}"
    fi
    
    # Check if logs were preserved
    if [[ -d "${LOG_DIR}" ]]; then
        echo "Logs preserved at: ${LOG_DIR}"
    fi
    
    # List any backup directories
    local config_backups=$(ls -d "${CONFIG_DIR}.backup."* 2>/dev/null || true)
    local log_backups=$(ls -d "${LOG_DIR}.backup."* 2>/dev/null || true)
    
    if [[ -n "${config_backups}" ]] || [[ -n "${log_backups}" ]]; then
        echo
        echo "Backup directories created:"
        [[ -n "${config_backups}" ]] && echo "${config_backups}"
        [[ -n "${log_backups}" ]] && echo "${log_backups}"
    fi
    
    echo
    echo "To reinstall, run: ./scripts/install.sh"
    echo
}

# Main uninstallation flow
main() {
    confirm_uninstall
    stop_service
    remove_launchagent
    remove_binary
    remove_config
    remove_logs
    remove_support_files
    verify_uninstall
    print_post_uninstall
}

# Run main function
main "$@"
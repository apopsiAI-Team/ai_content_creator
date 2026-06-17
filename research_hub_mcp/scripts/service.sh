#!/bin/bash

# Rust Sci-Hub MCP Server - Service Management Script
# Provides convenient commands for managing the LaunchAgent service

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
NC='\033[0m' # No Color

# Configuration
SERVICE_NAME="rust-sci-hub-mcp"
PLIST_NAME="com.${SERVICE_NAME}.plist"
BINARY_NAME="rust-sci-hub-mcp"
CONFIG_DIR="${HOME}/.config/${SERVICE_NAME}"
SUPPORT_DIR="${HOME}/Library/Application Support/${SERVICE_NAME}"
LOG_DIR="${HOME}/Library/Logs/${SERVICE_NAME}"
LAUNCHAGENT_DIR="${HOME}/Library/LaunchAgents"
HEALTH_URL="http://localhost:8090/health"

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

print_status() {
    echo -e "${MAGENTA}STATUS: $1${NC}"
}

usage() {
    cat << EOF
Usage: $0 <command> [options]

Service management commands for ${SERVICE_NAME}

Commands:
    start       Start the service
    stop        Stop the service
    restart     Restart the service
    status      Show service status
    logs        Show service logs (tail -f)
    health      Check service health
    reload      Reload service configuration
    enable      Enable service to start at login
    disable     Disable service from starting at login
    pid         Show service PID
    config      Edit configuration file
    info        Show service information
    help        Show this help message

Options:
    -h, --help  Show help for specific command
    -f, --force Force operation without confirmation

Examples:
    $0 status           # Check if service is running
    $0 restart          # Restart the service
    $0 logs             # Watch service logs
    $0 health           # Check health endpoint

EOF
}

check_service_installed() {
    if [[ ! -f "${LAUNCHAGENT_DIR}/${PLIST_NAME}" ]]; then
        print_error "Service not installed. Run ./scripts/install.sh first"
        exit 1
    fi
}

service_start() {
    print_info "Starting ${SERVICE_NAME}..."
    check_service_installed
    
    # Check if already running
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        print_warning "Service is already running"
        return 0
    fi
    
    # Load the service
    if launchctl load -w "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null; then
        print_success "Service started successfully"
        sleep 2
        service_status
    else
        print_error "Failed to start service"
        print_info "Check logs: tail -f '${LOG_DIR}/stderr.log'"
        exit 1
    fi
}

service_stop() {
    print_info "Stopping ${SERVICE_NAME}..."
    check_service_installed
    
    # Check if running
    if ! launchctl list | grep -q "${SERVICE_NAME}"; then
        print_warning "Service is not running"
        return 0
    fi
    
    # Unload the service
    if launchctl unload "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null; then
        print_success "Service stopped successfully"
    else
        print_error "Failed to stop service"
        exit 1
    fi
}

service_restart() {
    print_info "Restarting ${SERVICE_NAME}..."
    service_stop
    sleep 2
    service_start
}

service_status() {
    print_info "Checking ${SERVICE_NAME} status..."
    
    # Check LaunchAgent status
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        print_status "Service is RUNNING"
        
        # Get detailed status
        local status_line=$(launchctl list | grep "${SERVICE_NAME}")
        echo "  LaunchAgent: ${status_line}"
        
        # Get PID
        local pid=$(echo "${status_line}" | awk '{print $1}')
        if [[ "${pid}" != "-" ]]; then
            echo "  PID: ${pid}"
            
            # Get process info
            if ps -p "${pid}" > /dev/null 2>&1; then
                local proc_info=$(ps -p "${pid}" -o %cpu,%mem,etime,command | tail -1)
                echo "  Process: ${proc_info}"
            fi
        fi
        
        # Check PID file
        local pid_file="${SUPPORT_DIR}/${SERVICE_NAME}.pid"
        if [[ -f "${pid_file}" ]]; then
            local file_pid=$(cat "${pid_file}")
            echo "  PID file: ${file_pid}"
        fi
        
        # Check health endpoint
        if command -v curl &> /dev/null; then
            if curl -s -f "${HEALTH_URL}" > /dev/null 2>&1; then
                print_status "Health check: HEALTHY"
                
                # Get detailed health info
                local health_json=$(curl -s "${HEALTH_URL}" 2>/dev/null || echo "{}")
                if command -v jq &> /dev/null && [[ -n "${health_json}" ]]; then
                    echo "  Health details:"
                    echo "${health_json}" | jq '.' | sed 's/^/    /'
                fi
            else
                print_warning "Health check: NOT RESPONDING"
            fi
        fi
    else
        print_status "Service is STOPPED"
        
        # Check if process is running anyway
        if pgrep -x "${BINARY_NAME}" > /dev/null; then
            print_warning "Process found running outside of LaunchAgent"
            local pids=$(pgrep -x "${BINARY_NAME}")
            echo "  PIDs: ${pids}"
        fi
    fi
}

service_logs() {
    print_info "Showing logs for ${SERVICE_NAME}..."
    
    local log_files=(
        "${LOG_DIR}/stderr.log"
        "${LOG_DIR}/stdout.log"
        "${LOG_DIR}/service.log"
    )
    
    # Find existing log files
    local existing_logs=()
    for log in "${log_files[@]}"; do
        if [[ -f "${log}" ]]; then
            existing_logs+=("${log}")
        fi
    done
    
    if [[ ${#existing_logs[@]} -eq 0 ]]; then
        print_warning "No log files found"
        print_info "Log directory: ${LOG_DIR}"
        return 1
    fi
    
    print_info "Following log files:"
    printf '%s\n' "${existing_logs[@]}"
    echo
    echo "Press Ctrl+C to stop..."
    echo
    
    # Tail all existing logs
    tail -f "${existing_logs[@]}"
}

service_health() {
    print_info "Checking ${SERVICE_NAME} health..."
    
    if ! command -v curl &> /dev/null; then
        print_error "curl is required for health checks"
        exit 1
    fi
    
    # Check main health endpoint
    print_info "Health endpoint: ${HEALTH_URL}"
    
    if response=$(curl -s -w "\n%{http_code}" "${HEALTH_URL}" 2>/dev/null); then
        local body=$(echo "${response}" | head -n -1)
        local http_code=$(echo "${response}" | tail -n 1)
        
        if [[ "${http_code}" == "200" ]]; then
            print_success "Service is healthy (HTTP ${http_code})"
            
            # Pretty print JSON if jq is available
            if command -v jq &> /dev/null; then
                echo "${body}" | jq '.'
            else
                echo "${body}"
            fi
        else
            print_warning "Service returned HTTP ${http_code}"
            echo "${body}"
        fi
    else
        print_error "Failed to connect to health endpoint"
        print_info "Service may not be running or may still be starting"
    fi
    
    # Check additional health endpoints
    for endpoint in "live" "ready" "startup"; do
        local url="${HEALTH_URL}/${endpoint}"
        if curl -s -f "${url}" > /dev/null 2>&1; then
            print_success "  /${endpoint}: OK"
        else
            print_warning "  /${endpoint}: FAILED"
        fi
    done
}

service_reload() {
    print_info "Reloading ${SERVICE_NAME} configuration..."
    
    # Send SIGHUP to reload config
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        local status_line=$(launchctl list | grep "${SERVICE_NAME}")
        local pid=$(echo "${status_line}" | awk '{print $1}')
        
        if [[ "${pid}" != "-" ]] && [[ -n "${pid}" ]]; then
            print_info "Sending SIGHUP to PID ${pid}..."
            if kill -HUP "${pid}"; then
                print_success "Configuration reload signal sent"
                print_info "Check logs for reload status"
            else
                print_error "Failed to send reload signal"
                exit 1
            fi
        else
            print_warning "Could not find service PID"
            print_info "Performing restart instead..."
            service_restart
        fi
    else
        print_error "Service is not running"
        exit 1
    fi
}

service_enable() {
    print_info "Enabling ${SERVICE_NAME} to start at login..."
    check_service_installed
    
    # Enable the service
    if launchctl load -w "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null; then
        print_success "Service enabled"
    else
        print_warning "Service may already be enabled"
    fi
}

service_disable() {
    print_info "Disabling ${SERVICE_NAME} from starting at login..."
    check_service_installed
    
    # Stop if running
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        service_stop
    fi
    
    # Disable the service
    if launchctl unload -w "${LAUNCHAGENT_DIR}/${PLIST_NAME}" 2>/dev/null; then
        print_success "Service disabled"
    else
        print_warning "Service may already be disabled"
    fi
}

service_pid() {
    # Check LaunchAgent
    if launchctl list | grep -q "${SERVICE_NAME}"; then
        local status_line=$(launchctl list | grep "${SERVICE_NAME}")
        local pid=$(echo "${status_line}" | awk '{print $1}')
        
        if [[ "${pid}" != "-" ]] && [[ -n "${pid}" ]]; then
            echo "${pid}"
            return 0
        fi
    fi
    
    # Check PID file
    local pid_file="${SUPPORT_DIR}/${SERVICE_NAME}.pid"
    if [[ -f "${pid_file}" ]]; then
        local file_pid=$(cat "${pid_file}")
        if ps -p "${file_pid}" > /dev/null 2>&1; then
            echo "${file_pid}"
            return 0
        fi
    fi
    
    # Check by process name
    local pids=$(pgrep -x "${BINARY_NAME}" 2>/dev/null || true)
    if [[ -n "${pids}" ]]; then
        echo "${pids}" | head -1
        return 0
    fi
    
    print_error "Service PID not found"
    return 1
}

service_config() {
    local config_file="${CONFIG_DIR}/config.toml"
    
    if [[ ! -f "${config_file}" ]]; then
        print_error "Configuration file not found: ${config_file}"
        exit 1
    fi
    
    print_info "Opening configuration file: ${config_file}"
    
    # Use default editor or vi
    local editor="${EDITOR:-vi}"
    
    # Create backup
    local backup="${config_file}.backup.$(date +%Y%m%d_%H%M%S)"
    cp "${config_file}" "${backup}"
    print_info "Backup created: ${backup}"
    
    # Open editor
    "${editor}" "${config_file}"
    
    # Ask to reload
    read -p "Reload service configuration? (y/n): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        service_reload
    fi
}

service_info() {
    echo "========================================="
    echo "   ${SERVICE_NAME} Service Information"
    echo "========================================="
    echo
    echo "Service Name:      ${SERVICE_NAME}"
    echo "Binary:            /usr/local/bin/${BINARY_NAME}"
    echo "LaunchAgent:       ${LAUNCHAGENT_DIR}/${PLIST_NAME}"
    echo "Configuration:     ${CONFIG_DIR}/config.toml"
    echo "Logs:              ${LOG_DIR}/"
    echo "PID File:          ${SUPPORT_DIR}/${SERVICE_NAME}.pid"
    echo "Health URL:        ${HEALTH_URL}"
    echo
    
    # Show version if binary exists
    if [[ -f "/usr/local/bin/${BINARY_NAME}" ]]; then
        echo -n "Binary Version:    "
        /usr/local/bin/${BINARY_NAME} --version 2>/dev/null || echo "unknown"
    fi
    
    echo
    service_status
}

# Main command dispatcher
main() {
    if [[ $# -eq 0 ]]; then
        usage
        exit 0
    fi
    
    local command="$1"
    shift
    
    case "${command}" in
        start)
            service_start "$@"
            ;;
        stop)
            service_stop "$@"
            ;;
        restart)
            service_restart "$@"
            ;;
        status)
            service_status "$@"
            ;;
        logs|log)
            service_logs "$@"
            ;;
        health)
            service_health "$@"
            ;;
        reload)
            service_reload "$@"
            ;;
        enable)
            service_enable "$@"
            ;;
        disable)
            service_disable "$@"
            ;;
        pid)
            service_pid "$@"
            ;;
        config)
            service_config "$@"
            ;;
        info)
            service_info "$@"
            ;;
        help|-h|--help)
            usage
            ;;
        *)
            print_error "Unknown command: ${command}"
            echo
            usage
            exit 1
            ;;
    esac
}

# Run main function
main "$@"
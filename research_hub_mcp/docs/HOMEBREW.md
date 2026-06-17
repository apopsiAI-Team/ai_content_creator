# Homebrew Installation Guide

This guide covers installing and managing the rust-sci-hub-mcp server using Homebrew on macOS.

## Table of Contents

- [Quick Installation](#quick-installation)
- [Installation Options](#installation-options)
- [Service Management](#service-management)
- [Configuration](#configuration)
- [Upgrading](#upgrading)
- [Troubleshooting](#troubleshooting)
- [Uninstallation](#uninstallation)
- [Development Installation](#development-installation)

## Quick Installation

## Installation Status

✅ **Homebrew Installation**: Now available and working!

### Homebrew Installation (Recommended)
```bash
# Clone repository to get the formula
git clone https://github.com/Ladvien/sci_hub_mcp.git
cd sci_hub_mcp

# Install via Homebrew
brew install --build-from-source homebrew/rust-sci-hub-mcp.rb

# Configure Sci-Hub mirrors (required)
mkdir -p ~/.config/rust-sci-hub-mcp

# Edit config file to add mirrors
cat > ~/.config/rust-sci-hub-mcp/config.toml << 'EOF'
[server]
host = "127.0.0.1"
port = 8080
health_check_interval_secs = 60
graceful_shutdown_timeout_secs = 30

[sci_hub]
mirrors = [
    "https://sci-hub.se",
    "https://sci-hub.st", 
    "https://sci-hub.ru"
]
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

# Start the service
brew services start rust-sci-hub-mcp

# Test the binary
rust-sci-hub-mcp --version
```

### Source Installation (Alternative)
```bash
# Clone and build from source
git clone https://github.com/Ladvien/sci_hub_mcp.git
cd sci_hub_mcp
cargo build --release

# Test the binary
./target/release/rust-sci-hub-mcp --version
```

## Installation Options

### Standard Installation

```bash
# Install from the official tap
brew install yourusername/rust-sci-hub-mcp/rust-sci-hub-mcp
```

### Build from Source

```bash
# Install with full compilation (takes longer but ensures compatibility)
brew install --build-from-source rust-sci-hub-mcp
```

### Development Version

```bash
# Install the latest development version
brew install --HEAD rust-sci-hub-mcp
```

## Service Management

Homebrew provides built-in service management through `brew services`:

### Starting the Service

```bash
# Start the service now
brew services start rust-sci-hub-mcp

# Start the service and enable it to start at login
brew services restart rust-sci-hub-mcp
```

### Checking Service Status

```bash
# List all Homebrew services
brew services list

# Check only rust-sci-hub-mcp status
brew services list | grep rust-sci-hub-mcp

# Get detailed service information
brew services info rust-sci-hub-mcp
```

### Stopping the Service

```bash
# Stop the service
brew services stop rust-sci-hub-mcp

# Stop and disable from starting at login
brew services stop rust-sci-hub-mcp --all
```

### Restarting the Service

```bash
# Restart the service (stop then start)
brew services restart rust-sci-hub-mcp
```

### Service Logs

```bash
# View service logs
tail -f ~/Library/Logs/rust-sci-hub-mcp/stderr.log

# View stdout logs
tail -f ~/Library/Logs/rust-sci-hub-mcp/stdout.log

# View all logs
tail -f ~/Library/Logs/rust-sci-hub-mcp/*.log
```

## Configuration

### Default Configuration

After installation, a default configuration is created at:
```
~/.config/rust-sci-hub-mcp/config.toml
```

### Editing Configuration

```bash
# Edit the configuration file
brew services stop rust-sci-hub-mcp
nano ~/.config/rust-sci-hub-mcp/config.toml
brew services start rust-sci-hub-mcp
```

### Configuration Options

Key configuration sections:

```toml
[server]
host = "127.0.0.1"          # Bind address
port = 8080                 # HTTP port
health_check_interval_secs = 60

[sci_hub]
mirrors = []                # Auto-discovered if empty
timeout_secs = 30
retry_attempts = 3
rate_limit_requests_per_minute = 30

[downloads]
directory = "~/Downloads/papers"
concurrent_downloads = 3
chunk_size_bytes = 8192

[logging]
level = "info"
file = "~/Library/Logs/rust-sci-hub-mcp/service.log"
```

### Reloading Configuration

```bash
# The service supports configuration reload via SIGHUP
brew services restart rust-sci-hub-mcp

# Or send the signal directly
pkill -HUP rust-sci-hub-mcp
```

## Health Checks

The service provides health check endpoints:

```bash
# Basic health check
curl http://localhost:8080/health

# Detailed health information
curl http://localhost:8080/health | jq .

# Check specific probes
curl http://localhost:8080/health/live     # Liveness probe
curl http://localhost:8080/health/ready    # Readiness probe
curl http://localhost:8080/health/startup  # Startup probe
```

## Upgrading

### Check for Updates

```bash
# Update Homebrew and check for updates
brew update
brew outdated | grep rust-sci-hub-mcp
```

### Upgrade the Package

```bash
# Upgrade to the latest version
brew upgrade rust-sci-hub-mcp

# The service will be automatically restarted after upgrade
```

### Version Information

```bash
# Check installed version
brew list --versions rust-sci-hub-mcp

# Check running version
rust-sci-hub-mcp --version

# Compare with latest available
brew info rust-sci-hub-mcp
```

## Troubleshooting

### Installation Issues

**Problem**: Formula not found
```bash
Error: No available formula with name "rust-sci-hub-mcp"
```
**Solution**:
```bash
# Add the tap first
brew tap yourusername/rust-sci-hub-mcp
# Then install
brew install rust-sci-hub-mcp
```

**Problem**: Build errors
```bash
Error: rust-sci-hub-mcp: failed to build
```
**Solution**:
```bash
# Check dependencies
brew doctor
# Update Homebrew
brew update
# Try building from source
brew install --build-from-source rust-sci-hub-mcp
```

### Service Issues

**Problem**: Service won't start
```bash
# Check logs
tail -f ~/Library/Logs/rust-sci-hub-mcp/stderr.log

# Check if port is in use
lsof -i :8080

# Check service status
brew services list | grep rust-sci-hub-mcp
```

**Problem**: Service crashes repeatedly
```bash
# Check configuration
rust-sci-hub-mcp --config ~/.config/rust-sci-hub-mcp/config.toml --dry-run

# Check system resources
top -pid $(pgrep rust-sci-hub-mcp)

# Reset to default configuration
brew services stop rust-sci-hub-mcp
rm ~/.config/rust-sci-hub-mcp/config.toml
brew services start rust-sci-hub-mcp
```

### Permission Issues

**Problem**: Permission denied errors
```bash
# Fix configuration file permissions
chmod 600 ~/.config/rust-sci-hub-mcp/config.toml

# Fix log directory permissions
chown -R $(whoami) ~/Library/Logs/rust-sci-hub-mcp/
```

### Network Issues

**Problem**: Health check fails
```bash
# Check if service is running
pgrep rust-sci-hub-mcp

# Check listening ports
netstat -an | grep 8080

# Test with verbose curl
curl -v http://localhost:8080/health
```

## Uninstallation

### Stop and Remove Service

```bash
# Stop the service
brew services stop rust-sci-hub-mcp

# Uninstall the package
brew uninstall rust-sci-hub-mcp

# Remove the tap (optional)
brew untap yourusername/rust-sci-hub-mcp
```

### Clean Up User Data

```bash
# Remove configuration (optional)
rm -rf ~/.config/rust-sci-hub-mcp/

# Remove logs (optional)
rm -rf ~/Library/Logs/rust-sci-hub-mcp/

# Remove application support files (optional)
rm -rf ~/Library/Application\ Support/rust-sci-hub-mcp/

# Remove LaunchAgent (if manually installed)
rm -f ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist
```

## Development Installation

### Installing from Local Source

```bash
# Clone the repository
git clone https://github.com/yourusername/rust-sci-hub-mcp.git
cd rust-sci-hub-mcp

# Install from local directory
brew install --build-from-source homebrew/rust-sci-hub-mcp.rb
```

### Testing the Formula

```bash
# Audit the formula
brew audit --strict homebrew/rust-sci-hub-mcp.rb

# Test installation in isolated environment
brew install --build-from-source homebrew/rust-sci-hub-mcp.rb

# Run formula tests
brew test rust-sci-hub-mcp
```

### Formula Development

```bash
# Edit the formula
brew edit rust-sci-hub-mcp

# Reinstall after changes
brew reinstall rust-sci-hub-mcp

# Validate changes
brew audit rust-sci-hub-mcp
```

## Advanced Usage

### Multiple Instances

To run multiple instances with different configurations:

```bash
# Create separate config files
cp ~/.config/rust-sci-hub-mcp/config.toml ~/.config/rust-sci-hub-mcp/config-dev.toml

# Edit port numbers and other settings
nano ~/.config/rust-sci-hub-mcp/config-dev.toml

# Run additional instance manually
rust-sci-hub-mcp --config ~/.config/rust-sci-hub-mcp/config-dev.toml --daemon
```

### Integration with Other Tools

```bash
# Use with jq for JSON processing
curl -s http://localhost:8080/health | jq '.status'

# Monitor with watch
watch "curl -s http://localhost:8080/health | jq '.uptime'"

# Log analysis with grep
tail -f ~/Library/Logs/rust-sci-hub-mcp/stderr.log | grep ERROR
```

## Support and Resources

### Getting Help

- Check logs: `~/Library/Logs/rust-sci-hub-mcp/`
- Service status: `brew services list | grep rust-sci-hub-mcp`
- Health check: `curl http://localhost:8080/health`
- Configuration: `~/.config/rust-sci-hub-mcp/config.toml`

### Useful Commands Reference

```bash
# Installation
brew install rust-sci-hub-mcp

# Service management
brew services start|stop|restart rust-sci-hub-mcp

# Status and health
brew services list | grep rust-sci-hub-mcp
curl http://localhost:8080/health

# Logs and debugging
tail -f ~/Library/Logs/rust-sci-hub-mcp/stderr.log
rust-sci-hub-mcp --version
brew info rust-sci-hub-mcp

# Updates and maintenance
brew update && brew upgrade rust-sci-hub-mcp
brew uninstall rust-sci-hub-mcp
```

For more detailed information, see the main documentation at `docs/README.md`.
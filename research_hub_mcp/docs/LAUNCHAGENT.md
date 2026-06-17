# macOS LaunchAgent Integration Guide

This guide covers the installation, configuration, and management of the rust-sci-hub-mcp server as a macOS LaunchAgent service.

## Table of Contents

- [Overview](#overview)
- [Requirements](#requirements)
- [Installation](#installation)
- [Configuration](#configuration)
- [Service Management](#service-management)
- [Troubleshooting](#troubleshooting)
- [Security Considerations](#security-considerations)
- [Manual Installation](#manual-installation)
- [Uninstallation](#uninstallation)

## Overview

The rust-sci-hub-mcp server can run as a background service on macOS using LaunchAgent. This allows the service to:

- Start automatically when you log in
- Run in the background without a terminal window
- Restart automatically if it crashes
- Integrate with macOS logging (Console.app)
- Be managed using standard macOS tools

## Requirements

- macOS 10.14 (Mojave) or later
- Rust toolchain (for building from source)
- Administrator access (for installing to /usr/local/bin)
- Approximately 100MB of free disk space

## Installation

### Quick Installation

The easiest way to install the service is using the provided installation script:

```bash
# Clone the repository (if not already done)
git clone https://github.com/yourusername/rust-sci-hub-mcp.git
cd rust-sci-hub-mcp

# Build the project
cargo build --release

# Make scripts executable
chmod +x scripts/*.sh

# Run the installation script
./scripts/install.sh
```

The installation script will:
1. Check system requirements
2. Build the binary (if needed)
3. Install the binary to `/usr/local/bin`
4. Create necessary directories
5. Generate a default configuration
6. Install and start the LaunchAgent

### What Gets Installed

| Component | Location | Purpose |
|-----------|----------|---------|
| Binary | `/usr/local/bin/rust-sci-hub-mcp` | Main executable |
| LaunchAgent | `~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist` | Service definition |
| Configuration | `~/.config/rust-sci-hub-mcp/config.toml` | Service configuration |
| Logs | `~/Library/Logs/rust-sci-hub-mcp/` | Service logs |
| PID file | `~/Library/Application Support/rust-sci-hub-mcp/rust-sci-hub-mcp.pid` | Process ID tracking |

## Configuration

### Service Configuration

Edit the configuration file at `~/.config/rust-sci-hub-mcp/config.toml`:

```toml
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
```

To edit the configuration:

```bash
./scripts/service.sh config
```

### LaunchAgent Configuration

The LaunchAgent plist file controls how macOS manages the service. Key settings:

- **RunAtLoad**: Service starts when you log in
- **KeepAlive**: Service restarts if it crashes
- **ThrottleInterval**: Wait 10 seconds between restart attempts
- **ProcessType**: Runs as a background process
- **StandardOutPath/StandardErrorPath**: Log file locations

To modify LaunchAgent behavior, edit:
`~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist`

After editing, reload the service:

```bash
launchctl unload ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist
launchctl load ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist
```

## Service Management

### Using the Service Script

The `service.sh` script provides convenient commands for managing the service:

```bash
# Check service status
./scripts/service.sh status

# Start the service
./scripts/service.sh start

# Stop the service
./scripts/service.sh stop

# Restart the service
./scripts/service.sh restart

# View logs (tail -f)
./scripts/service.sh logs

# Check health endpoint
./scripts/service.sh health

# Reload configuration (without restart)
./scripts/service.sh reload

# Show service information
./scripts/service.sh info
```

### Using launchctl Directly

You can also use macOS's `launchctl` command directly:

```bash
# List all services (check if rust-sci-hub-mcp is running)
launchctl list | grep rust-sci-hub-mcp

# Start the service
launchctl load ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist

# Stop the service
launchctl unload ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist

# Get detailed service info
launchctl print user/$(id -u)/com.rust-sci-hub-mcp
```

### Checking Service Health

The service provides a health endpoint for monitoring:

```bash
# Check basic health
curl http://localhost:8080/health

# Check liveness
curl http://localhost:8080/health/live

# Check readiness
curl http://localhost:8080/health/ready

# Check startup status
curl http://localhost:8080/health/startup
```

## Troubleshooting

### Service Won't Start

1. **Check logs**:
   ```bash
   tail -f ~/Library/Logs/rust-sci-hub-mcp/stderr.log
   ```

2. **Verify binary exists**:
   ```bash
   ls -la /usr/local/bin/rust-sci-hub-mcp
   ```

3. **Check LaunchAgent status**:
   ```bash
   launchctl list | grep rust-sci-hub-mcp
   ```

4. **Verify permissions**:
   ```bash
   ls -la ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist
   # Should be -rw-r--r-- and owned by your user
   ```

### Port Already in Use

If port 8080 is already in use:

1. Find the process using the port:
   ```bash
   lsof -i :8080
   ```

2. Either stop the conflicting service or change the port in the configuration

### Service Crashes Repeatedly

1. Check the crash logs:
   ```bash
   tail -f ~/Library/Logs/rust-sci-hub-mcp/stderr.log
   ```

2. Check system logs:
   ```bash
   log show --predicate 'process == "rust-sci-hub-mcp"' --last 1h
   ```

3. Temporarily disable auto-restart to debug:
   - Edit `~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist`
   - Set `<key>KeepAlive</key>` to `<false/>`
   - Reload the service

### Permission Issues

If you encounter permission errors:

1. **For /usr/local/bin**: Use `sudo` for installation
2. **For config/logs**: Ensure your user owns the directories:
   ```bash
   chown -R $(whoami) ~/.config/rust-sci-hub-mcp
   chown -R $(whoami) ~/Library/Logs/rust-sci-hub-mcp
   ```

## Security Considerations

### Network Security

- The service binds to localhost (127.0.0.1) by default
- To allow external connections, change the host in config.toml
- Consider using a firewall to restrict access

### File Permissions

- Configuration files should be readable only by your user:
  ```bash
  chmod 600 ~/.config/rust-sci-hub-mcp/config.toml
  ```

- The binary should not be writable by non-admin users:
  ```bash
  ls -la /usr/local/bin/rust-sci-hub-mcp
  # Should be -rwxr-xr-x and owned by root or admin
  ```

### Privacy

- The service runs under your user account
- It has access to your files and network
- Review the configuration to ensure appropriate access levels

## Manual Installation

If you prefer to install manually without the script:

1. **Build the binary**:
   ```bash
   cargo build --release
   ```

2. **Install the binary**:
   ```bash
   sudo cp target/release/rust-sci-hub-mcp /usr/local/bin/
   sudo chmod 755 /usr/local/bin/rust-sci-hub-mcp
   ```

3. **Create directories**:
   ```bash
   mkdir -p ~/.config/rust-sci-hub-mcp
   mkdir -p ~/Library/Application\ Support/rust-sci-hub-mcp
   mkdir -p ~/Library/Logs/rust-sci-hub-mcp
   ```

4. **Create configuration**:
   ```bash
   cp docs/config.example.toml ~/.config/rust-sci-hub-mcp/config.toml
   ```

5. **Install LaunchAgent**:
   ```bash
   cp launchd/com.rust-sci-hub-mcp.plist ~/Library/LaunchAgents/
   # Replace ~ with $HOME in the plist file
   sed -i '' "s|~|$HOME|g" ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist
   ```

6. **Load the service**:
   ```bash
   launchctl load ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist
   ```

## Uninstallation

To completely remove the service:

### Using the Uninstall Script

```bash
./scripts/uninstall.sh
```

This will:
- Stop the running service
- Remove the LaunchAgent
- Remove the binary from /usr/local/bin
- Optionally remove configuration and logs
- Create backups of configuration and logs

### Manual Uninstallation

1. **Stop and unload the service**:
   ```bash
   launchctl unload ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist
   ```

2. **Remove the LaunchAgent**:
   ```bash
   rm ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist
   ```

3. **Remove the binary**:
   ```bash
   sudo rm /usr/local/bin/rust-sci-hub-mcp
   ```

4. **Remove configuration and data** (optional):
   ```bash
   rm -rf ~/.config/rust-sci-hub-mcp
   rm -rf ~/Library/Application\ Support/rust-sci-hub-mcp
   rm -rf ~/Library/Logs/rust-sci-hub-mcp
   ```

## Advanced Topics

### Running Multiple Instances

To run multiple instances with different configurations:

1. Create separate plist files with unique labels
2. Use different ports and PID files for each instance
3. Specify different config files in the ProgramArguments

### Integration with Homebrew Services

If installed via Homebrew, you can use:

```bash
brew services start rust-sci-hub-mcp
brew services stop rust-sci-hub-mcp
brew services restart rust-sci-hub-mcp
```

### Monitoring with Third-Party Tools

The service can be monitored using tools like:
- **LaunchControl**: GUI for managing LaunchAgents
- **Lingon**: Advanced LaunchAgent editor
- **Console.app**: View system and application logs

### Debugging

For detailed debugging:

1. Enable debug logging:
   ```toml
   [logging]
   level = "debug"
   ```

2. Run in foreground mode:
   ```bash
   /usr/local/bin/rust-sci-hub-mcp --config ~/.config/rust-sci-hub-mcp/config.toml
   ```

3. Use system diagnostics:
   ```bash
   launchctl print user/$(id -u)/com.rust-sci-hub-mcp
   ```

## Support

For issues or questions:
1. Check the logs in `~/Library/Logs/rust-sci-hub-mcp/`
2. Run `./scripts/service.sh status` for current status
3. Consult the main README.md for general usage
4. Open an issue on GitHub with relevant log excerpts
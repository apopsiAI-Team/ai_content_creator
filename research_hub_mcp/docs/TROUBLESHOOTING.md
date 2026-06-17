# Troubleshooting Guide

This guide helps resolve common issues with the rust-sci-hub-mcp server.

## Table of Contents

- [Quick Diagnostics](#quick-diagnostics)
- [Installation Issues](#installation-issues)
- [Service Startup Issues](#service-startup-issues)
- [Network and Connectivity Issues](#network-and-connectivity-issues)
- [Configuration Issues](#configuration-issues)
- [Performance Issues](#performance-issues)
- [Security Issues](#security-issues)
- [Error Reference](#error-reference)
- [Advanced Debugging](#advanced-debugging)

## Quick Diagnostics

Run this diagnostic script to quickly identify common issues:

```bash
#!/bin/bash
# Quick diagnostics for rust-sci-hub-mcp

echo "=== Rust Sci-Hub MCP Diagnostics ==="
echo

# Check if service is running
echo "1. Service Status:"
if pgrep -f rust-sci-hub-mcp > /dev/null; then
    echo "   ✓ Service is running (PID: $(pgrep -f rust-sci-hub-mcp))"
else
    echo "   ✗ Service is not running"
fi
echo

# Check if port is listening
echo "2. Port Status:"
if lsof -i :8080 > /dev/null 2>&1; then
    echo "   ✓ Port 8080 is listening"
else
    echo "   ✗ Port 8080 is not listening"
fi
echo

# Check health endpoint
echo "3. Health Check:"
if curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "   ✓ Health endpoint responding"
    curl -s http://localhost:8080/health | jq .
else
    echo "   ✗ Health endpoint not responding"
fi
echo

# Check configuration file
echo "4. Configuration:"
CONFIG_FILE="$HOME/.config/rust-sci-hub-mcp/config.toml"
if [ -f "$CONFIG_FILE" ]; then
    echo "   ✓ Configuration file exists"
    echo "   File: $CONFIG_FILE"
    echo "   Size: $(wc -c < "$CONFIG_FILE") bytes"
    echo "   Modified: $(stat -f "%Sm" "$CONFIG_FILE")"
else
    echo "   ✗ Configuration file not found"
    echo "   Expected: $CONFIG_FILE"
fi
echo

# Check logs
echo "5. Recent Logs:"
LOG_FILE="$HOME/Library/Logs/rust-sci-hub-mcp/service.log"
if [ -f "$LOG_FILE" ]; then
    echo "   ✓ Log file exists"
    echo "   Last 5 lines:"
    tail -5 "$LOG_FILE" | sed 's/^/   │ /'
else
    echo "   ✗ Log file not found"
    echo "   Expected: $LOG_FILE"
fi
```

## Installation Issues

### Issue: Command not found

**Symptom**:
```bash
$ rust-sci-hub-mcp
command not found: rust-sci-hub-mcp
```

**Solutions**:

1. **Check if installed via Homebrew**:
   ```bash
   brew list | grep rust-sci-hub-mcp
   ```

2. **Reinstall via Homebrew**:
   ```bash
   brew uninstall rust-sci-hub-mcp
   brew install rust-sci-hub-mcp
   ```

3. **Check PATH**:
   ```bash
   echo $PATH
   which rust-sci-hub-mcp
   ```

4. **Build from source**:
   ```bash
   git clone <repository-url>
   cd rust-sci-hub-mcp
   cargo build --release
   ./target/release/rust-sci-hub-mcp --version
   ```

### Issue: Build failures

**Symptom**:
```bash
error: failed to compile `rust-sci-hub-mcp`
```

**Solutions**:

1. **Update Rust toolchain**:
   ```bash
   rustup update stable
   rustc --version  # Should be 1.70+
   ```

2. **Clear cache and rebuild**:
   ```bash
   cargo clean
   cargo build --release
   ```

3. **Check system dependencies**:
   ```bash
   # macOS
   xcode-select --install
   
   # Linux
   sudo apt-get install build-essential pkg-config libssl-dev
   ```

4. **Check available disk space**:
   ```bash
   df -h .
   # Ensure at least 2GB free for compilation
   ```

## Service Startup Issues

### Issue: Service fails to start

**Symptom**:
```bash
$ brew services start rust-sci-hub-mcp
Error: Failure while executing; `/bin/launchctl load -w ...`
```

**Diagnostic Steps**:

1. **Check service logs**:
   ```bash
   tail -f ~/Library/Logs/rust-sci-hub-mcp/stderr.log
   ```

2. **Try manual start**:
   ```bash
   rust-sci-hub-mcp --verbose
   # Look for specific error messages
   ```

3. **Check configuration syntax**:
   ```bash
   rust-sci-hub-mcp --config ~/.config/rust-sci-hub-mcp/config.toml --dry-run
   ```

### Issue: Port already in use

**Symptom**:
```
Error: Address already in use (os error 48)
```

**Solutions**:

1. **Find process using the port**:
   ```bash
   lsof -i :8080
   # Kill the process if safe to do so
   kill -9 <PID>
   ```

2. **Use a different port**:
   ```bash
   # Edit config file
   nano ~/.config/rust-sci-hub-mcp/config.toml
   
   [server]
   port = 8081  # Change to available port
   ```

3. **Check for multiple instances**:
   ```bash
   pgrep -f rust-sci-hub-mcp
   # Should only show one PID
   ```

### Issue: Permission denied

**Symptom**:
```
Permission denied (os error 13)
```

**Solutions**:

1. **Fix file permissions**:
   ```bash
   chmod 600 ~/.config/rust-sci-hub-mcp/config.toml
   chmod 755 ~/.config/rust-sci-hub-mcp/
   ```

2. **Fix log directory permissions**:
   ```bash
   mkdir -p ~/Library/Logs/rust-sci-hub-mcp/
   chown -R $(whoami) ~/Library/Logs/rust-sci-hub-mcp/
   ```

3. **Check downloads directory**:
   ```bash
   mkdir -p ~/Downloads/papers
   chmod 755 ~/Downloads/papers
   ```

## Network and Connectivity Issues

### Issue: Health check fails

**Symptom**:
```bash
$ curl http://localhost:8080/health
curl: (7) Failed to connect to localhost port 8080: Connection refused
```

**Diagnostic Steps**:

1. **Check if service is running**:
   ```bash
   pgrep -f rust-sci-hub-mcp
   ps aux | grep rust-sci-hub-mcp
   ```

2. **Check listening ports**:
   ```bash
   netstat -an | grep 8080
   lsof -i :8080
   ```

3. **Check firewall settings**:
   ```bash
   # macOS
   sudo pfctl -sr | grep 8080
   
   # Check if Little Snitch or similar is blocking
   ```

4. **Test with different address**:
   ```bash
   curl http://127.0.0.1:8080/health
   curl http://[::1]:8080/health  # IPv6
   ```

### Issue: Sci-Hub mirrors unreachable

**Symptom**:
```
All Sci-Hub mirrors failed: Network timeout
```

**Solutions**:

1. **Check internet connectivity**:
   ```bash
   ping google.com
   curl -I https://httpbin.org/status/200
   ```

2. **Test mirror accessibility**:
   ```bash
   curl -I https://sci-hub.se/
   curl -I https://sci-hub.st/
   curl -I https://sci-hub.ru/
   ```

3. **Check DNS resolution**:
   ```bash
   nslookup sci-hub.se
   dig sci-hub.se
   ```

4. **Configure alternative mirrors**:
   ```toml
   [sci_hub]
   mirrors = [
       "https://sci-hub.se",
       "https://sci-hub.st", 
       "https://sci-hub.ru",
       "https://sci-hub.ren"
   ]
   ```

5. **Check proxy settings**:
   ```bash
   echo $HTTP_PROXY
   echo $HTTPS_PROXY
   echo $NO_PROXY
   ```

### Issue: SSL/TLS errors

**Symptom**:
```
SSL certificate verify failed
```

**Solutions**:

1. **Update CA certificates**:
   ```bash
   # macOS
   brew install ca-certificates
   
   # Linux
   sudo apt-get update && sudo apt-get install ca-certificates
   ```

2. **Check system time**:
   ```bash
   date
   # Ensure system time is correct
   ```

3. **Test SSL connectivity**:
   ```bash
   openssl s_client -connect sci-hub.se:443 -servername sci-hub.se
   ```

## Configuration Issues

### Issue: Invalid configuration file

**Symptom**:
```
Configuration error: invalid TOML syntax
```

**Solutions**:

1. **Validate TOML syntax**:
   ```bash
   # Install TOML validator
   cargo install toml-cli
   
   # Validate config
   toml get ~/.config/rust-sci-hub-mcp/config.toml
   ```

2. **Reset to default configuration**:
   ```bash
   mv ~/.config/rust-sci-hub-mcp/config.toml ~/.config/rust-sci-hub-mcp/config.toml.backup
   rust-sci-hub-mcp --create-default-config
   ```

3. **Check specific syntax errors**:
   ```bash
   # Common issues:
   # - Missing quotes around strings
   # - Invalid port numbers (must be 1-65535)
   # - Invalid paths or special characters
   ```

### Issue: Environment variables not working

**Symptom**:
Configuration doesn't change when setting environment variables.

**Solutions**:

1. **Check variable naming**:
   ```bash
   # Correct format:
   export RUST_SCI_HUB_MCP_SERVER_PORT=8081
   export RUST_SCI_HUB_MCP_SCI_HUB_RATE_LIMIT_PER_SEC=2
   ```

2. **Verify environment**:
   ```bash
   env | grep RUST_SCI_HUB_MCP
   ```

3. **Check precedence order**:
   ```
   1. Command-line arguments (highest)
   2. Environment variables
   3. Configuration file
   4. Defaults (lowest)
   ```

## Performance Issues

### Issue: Slow download speeds

**Symptom**:
Downloads take much longer than expected.

**Solutions**:

1. **Check network bandwidth**:
   ```bash
   curl -o /dev/null -s -w "Speed: %{speed_download} bytes/sec\n" \
        http://speedtest.tele2.net/10MB.zip
   ```

2. **Adjust concurrent downloads**:
   ```toml
   [downloads]
   max_concurrent = 5  # Increase from default 3
   ```

3. **Check timeout settings**:
   ```toml
   [sci_hub]
   timeout_secs = 60  # Increase for slow connections
   ```

4. **Monitor resource usage**:
   ```bash
   top -pid $(pgrep rust-sci-hub-mcp)
   ```

### Issue: High memory usage

**Symptom**:
Service consumes excessive memory.

**Solutions**:

1. **Check memory usage patterns**:
   ```bash
   ps -o pid,ppid,pmem,pcpu,etime,command -p $(pgrep rust-sci-hub-mcp)
   ```

2. **Reduce concurrent operations**:
   ```toml
   [downloads]
   max_concurrent = 1  # Reduce from default 3
   
   [sci_hub]
   max_retries = 2     # Reduce from default 3
   ```

3. **Set file size limits**:
   ```toml
   [downloads]
   max_file_size_mb = 50  # Reduce from default 100
   ```

4. **Enable cleanup**:
   ```bash
   # Add to crontab for periodic cleanup
   0 2 * * * find ~/Downloads/papers -name "*.pdf" -mtime +30 -delete
   ```

## Security Issues

### Issue: Exposed to network

**Symptom**:
Service accessible from other machines.

**Solutions**:

1. **Bind to localhost only**:
   ```toml
   [server]
   host = "127.0.0.1"  # Never use "0.0.0.0" in production
   ```

2. **Check firewall settings**:
   ```bash
   # macOS
   sudo pfctl -sr
   
   # Ensure port 8080 is not open to external traffic
   ```

3. **Use VPN or SSH tunnel**:
   ```bash
   # For remote access, use SSH tunnel
   ssh -L 8080:localhost:8080 user@remote-host
   ```

### Issue: File permission vulnerabilities

**Symptom**:
Downloaded files have overly permissive access.

**Solutions**:

1. **Check file permissions**:
   ```bash
   ls -la ~/Downloads/papers/
   # Files should be 644, directories 755
   ```

2. **Set secure umask**:
   ```bash
   echo "umask 022" >> ~/.bashrc
   umask 022
   ```

3. **Use secure download directory**:
   ```bash
   mkdir -p ~/Documents/Research/Papers
   chmod 700 ~/Documents/Research/Papers
   ```

## Error Reference

### Common Error Codes

| Error Code | Description | Common Causes | Solutions |
|------------|-------------|---------------|-----------|
| `CONFIG_001` | Invalid configuration | Syntax error in TOML | Validate TOML syntax |
| `NET_001` | Network timeout | Slow/unreliable connection | Increase timeout values |
| `NET_002` | DNS resolution failed | DNS issues | Check DNS settings |
| `NET_003` | SSL certificate error | Expired/invalid certificates | Update CA certificates |
| `SCI_001` | All mirrors failed | Sci-Hub unreachable | Check mirror status |
| `SCI_002` | Rate limit exceeded | Too many requests | Reduce request rate |
| `FILE_001` | Permission denied | File/directory permissions | Fix permissions |
| `FILE_002` | Disk space full | Insufficient storage | Free up disk space |
| `SERV_001` | Port already in use | Another service on port | Use different port |
| `SERV_002` | Binding failed | Network configuration | Check network settings |

### Error Log Analysis

**Pattern**: Connection timeouts
```bash
grep -i "timeout" ~/Library/Logs/rust-sci-hub-mcp/service.log
# Look for patterns in timing
```

**Pattern**: Authentication failures
```bash
grep -i "auth\|permission\|denied" ~/Library/Logs/rust-sci-hub-mcp/service.log
```

**Pattern**: Memory issues
```bash
grep -i "memory\|allocation\|oom" ~/Library/Logs/rust-sci-hub-mcp/service.log
```

## Advanced Debugging

### Enable Debug Logging

```toml
[logging]
level = "debug"
format = "pretty"  # More readable for debugging
```

### Memory Profiling

```bash
# Install memory profiler
cargo install --features="jemallocator" memory-profiler

# Run with profiling
MALLOC_CONF=prof:true rust-sci-hub-mcp
```

### Network Debugging

```bash
# Capture network traffic
sudo tcpdump -i lo0 port 8080

# Monitor DNS queries
sudo tcpdump -i any port 53

# Test with verbose curl
curl -v -X POST http://localhost:8080/search \
  -H "Content-Type: application/json" \
  -d '{"query": "test", "type": "title"}'
```

### Performance Profiling

```bash
# CPU profiling
cargo install flamegraph
sudo flamegraph -- rust-sci-hub-mcp

# Memory profiling
cargo install heaptrack
heaptrack rust-sci-hub-mcp
```

### Database Debugging

If using embedded database for caching:

```bash
# Check database file
file ~/.config/rust-sci-hub-mcp/cache.db

# Database size
du -h ~/.config/rust-sci-hub-mcp/cache.db

# Clear cache if corrupted
rm ~/.config/rust-sci-hub-mcp/cache.db
```

## Getting Help

If you can't resolve the issue:

1. **Collect diagnostic information**:
   ```bash
   # System info
   uname -a
   rust-sci-hub-mcp --version
   
   # Configuration
   cat ~/.config/rust-sci-hub-mcp/config.toml
   
   # Recent logs
   tail -50 ~/Library/Logs/rust-sci-hub-mcp/service.log
   
   # Process information
   ps aux | grep rust-sci-hub-mcp
   lsof -p $(pgrep rust-sci-hub-mcp)
   ```

2. **Check GitHub issues**: Search for similar problems

3. **Create minimal reproduction**: Simplify configuration to isolate the issue

4. **Run in debug mode**:
   ```bash
   RUST_LOG=debug rust-sci-hub-mcp --verbose
   ```

Remember to **remove sensitive information** (API keys, personal paths) before sharing logs or configuration files.
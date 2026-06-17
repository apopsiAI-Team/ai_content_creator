# Security Considerations and Best Practices

This document outlines the security model, threat analysis, and best practices for the rust-sci-hub-mcp server.

## Table of Contents

- [Security Model](#security-model)
- [Threat Analysis](#threat-analysis)
- [Security Controls](#security-controls)
- [Network Security](#network-security)
- [File System Security](#file-system-security)
- [Application Security](#application-security)
- [Configuration Security](#configuration-security)
- [Operational Security](#operational-security)
- [Incident Response](#incident-response)
- [Security Checklist](#security-checklist)

## Security Model

### Trust Boundaries

```
┌─────────────────────────────────────────────────────────────────┐
│                         Trusted Zone                           │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                    Local System                             │ │
│  │  • rust-sci-hub-mcp process                                 │ │
│  │  • Configuration files                                      │ │
│  │  • Downloaded papers                                        │ │
│  │  • Log files                                                │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                │
                        Trust Boundary
                                │
┌─────────────────────────────────────────────────────────────────┐
│                        Untrusted Zone                          │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                 External Services                           │ │
│  │  • Sci-Hub mirrors                                          │ │
│  │  • DNS servers                                              │ │
│  │  • Network infrastructure                                   │ │
│  └─────────────────────────────────────────────────────────────┘ │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                 Client Applications                         │ │
│  │  • Claude Desktop (MCP client)                              │ │
│  │  • HTTP clients                                             │ │
│  │  • Command-line tools                                       │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### Security Principles

1. **Principle of Least Privilege**: Minimal permissions and access rights
2. **Defense in Depth**: Multiple layers of security controls
3. **Fail Secure**: Secure defaults and fail-safe mechanisms
4. **Input Validation**: All inputs are validated and sanitized
5. **Privacy by Design**: No unnecessary data collection or storage

## Threat Analysis

### STRIDE Threat Model

| Threat Category | Specific Threats | Mitigation |
|-----------------|------------------|------------|
| **Spoofing** | Malicious Sci-Hub mirrors | Certificate validation, trusted mirror list |
| **Tampering** | Modified download content | File integrity verification, checksums |
| **Repudiation** | Unauthorized access claims | Audit logging, request tracking |
| **Information Disclosure** | Configuration exposure | File permissions, localhost binding |
| **Denial of Service** | Resource exhaustion | Rate limiting, resource bounds |
| **Elevation of Privilege** | Process compromise | Sandboxing, minimal privileges |

### Attack Vectors

#### 1. Network-Based Attacks

**Threat**: Man-in-the-Middle (MITM) attacks
- **Description**: Attacker intercepts HTTPS traffic to Sci-Hub
- **Impact**: Malicious content injection, data theft
- **Mitigation**: Certificate pinning, HSTS enforcement

**Threat**: DNS Poisoning
- **Description**: Malicious DNS responses redirect to attacker servers
- **Impact**: Data exfiltration, malware distribution
- **Mitigation**: DNS over HTTPS (DoH), trusted DNS servers

#### 2. Input-Based Attacks

**Threat**: Command Injection
- **Description**: Malicious input executes system commands
- **Impact**: System compromise, data theft
- **Mitigation**: Input validation, parameterized queries

**Threat**: Path Traversal
- **Description**: Crafted file paths access unauthorized files
- **Impact**: File system access, privilege escalation
- **Mitigation**: Path sanitization, chroot jail

#### 3. Resource-Based Attacks

**Threat**: Denial of Service (DoS)
- **Description**: Resource exhaustion through excessive requests
- **Impact**: Service unavailability
- **Mitigation**: Rate limiting, resource quotas

**Threat**: Storage Exhaustion
- **Description**: Large downloads fill disk space
- **Impact**: System instability
- **Mitigation**: File size limits, disk quotas

## Security Controls

### 1. Input Validation

#### DOI Validation
```rust
// Example implementation - actual implementation may vary
use regex::Regex;
use once_cell::sync::Lazy;

static DOI_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^10\.\d{4,}/[^\s]+$").unwrap()
});

pub fn validate_doi(doi: &str) -> Result<(), SecurityError> {
    // Length check
    if doi.len() > 2048 {
        return Err(SecurityError::InputTooLong);
    }
    
    // Character validation
    if !doi.chars().all(|c| c.is_ascii() && !c.is_control()) {
        return Err(SecurityError::InvalidCharacters);
    }
    
    // Format validation
    if !DOI_REGEX.is_match(doi) {
        return Err(SecurityError::InvalidFormat);
    }
    
    Ok(())
}
```

#### Path Validation
```rust
use std::path::{Path, PathBuf};

pub fn validate_file_path(path: &str) -> Result<PathBuf, SecurityError> {
    let path = Path::new(path);
    
    // Reject absolute paths outside allowed directories
    if path.is_absolute() {
        return Err(SecurityError::AbsolutePathNotAllowed);
    }
    
    // Check for path traversal attempts
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err(SecurityError::PathTraversalAttempt);
            }
            std::path::Component::Normal(name) => {
                // Check for null bytes and control characters
                if name.to_string_lossy().contains('\0') {
                    return Err(SecurityError::NullByteInPath);
                }
            }
            _ => {}
        }
    }
    
    Ok(path.to_path_buf())
}
```

#### Query Sanitization
```rust
pub fn sanitize_search_query(query: &str) -> Result<String, SecurityError> {
    // Length limits
    if query.len() > 1024 {
        return Err(SecurityError::QueryTooLong);
    }
    
    // Remove potentially dangerous characters
    let sanitized = query
        .chars()
        .filter(|&c| c.is_alphanumeric() || " .-_".contains(c))
        .collect::<String>();
    
    // Check for SQL injection patterns
    let sql_patterns = ["'", "\"", ";", "--", "/*", "*/", "xp_", "sp_"];
    let lower_query = sanitized.to_lowercase();
    
    for pattern in &sql_patterns {
        if lower_query.contains(pattern) {
            return Err(SecurityError::PotentialSQLInjection);
        }
    }
    
    Ok(sanitized)
}
```

### 2. Access Controls

#### File System Permissions
```rust
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;

pub fn create_secure_file(path: &Path) -> Result<File, io::Error> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .mode(0o600)  // Owner read/write only
        .open(path)?;
    
    Ok(file)
}

pub fn create_secure_directory(path: &Path) -> Result<(), io::Error> {
    std::fs::create_dir_all(path)?;
    
    // Set directory permissions to 700 (owner only)
    let permissions = Permissions::from_mode(0o700);
    std::fs::set_permissions(path, permissions)?;
    
    Ok(())
}
```

#### Network Access Control
```rust
// Localhost-only binding
pub fn create_secure_server() -> Result<TcpListener, io::Error> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    
    // Additional security: Set SO_REUSEADDR to false
    // This prevents other processes from hijacking the port
    listener.set_nonblocking(true)?;
    
    Ok(listener)
}
```

### 3. Rate Limiting

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub struct RateLimiter {
    requests: Mutex<HashMap<String, Vec<Instant>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub async fn check_rate_limit(&self, client_id: &str) -> bool {
        let mut requests = self.requests.lock().await;
        let now = Instant::now();
        
        // Clean old requests
        let client_requests = requests.entry(client_id.to_string()).or_default();
        client_requests.retain(|&time| now.duration_since(time) < self.window);
        
        // Check if under limit
        if client_requests.len() < self.max_requests {
            client_requests.push(now);
            true
        } else {
            false
        }
    }
}
```

## Network Security

### 1. HTTPS Configuration

```rust
use reqwest::ClientBuilder;
use std::time::Duration;

pub fn create_secure_http_client() -> Result<reqwest::Client, reqwest::Error> {
    ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent("rust-sci-hub-mcp/1.0.0")
        .min_tls_version(reqwest::tls::Version::TLS_1_2)
        .https_only(true)
        .build()
}
```

### 2. Certificate Validation

```rust
// Custom certificate validation for known Sci-Hub mirrors
pub fn validate_sci_hub_certificate(cert: &Certificate) -> bool {
    // Check certificate chain
    if !cert.is_valid() {
        return false;
    }
    
    // Check against known good certificates (implement certificate pinning)
    let known_fingerprints = [
        "sha256:AABBCC...", // sci-hub.se
        "sha256:DDEEFF...", // sci-hub.st
    ];
    
    let cert_fingerprint = cert.fingerprint();
    known_fingerprints.contains(&cert_fingerprint.as_str())
}
```

### 3. DNS Security

```toml
# Configuration for secure DNS
[network]
dns_servers = [
    "1.1.1.1",          # Cloudflare DNS
    "8.8.8.8",          # Google DNS
    "9.9.9.9"           # Quad9 DNS
]
use_dns_over_https = true
dns_timeout_secs = 5
```

## File System Security

### 1. Secure File Creation

```rust
use std::fs::File;
use std::io::Write;
use tempfile::NamedTempFile;

pub async fn download_file_securely(
    url: &str, 
    destination: &Path
) -> Result<(), DownloadError> {
    // Download to temporary file first
    let mut temp_file = NamedTempFile::new()?;
    
    // Download content
    let response = http_client.get(url).send().await?;
    let content = response.bytes().await?;
    
    // Validate content (size, type, etc.)
    validate_downloaded_content(&content)?;
    
    // Write to temporary file
    temp_file.write_all(&content)?;
    temp_file.flush()?;
    
    // Atomically move to final destination
    temp_file.persist(destination)?;
    
    // Set secure permissions
    set_secure_file_permissions(destination)?;
    
    Ok(())
}

fn validate_downloaded_content(content: &[u8]) -> Result<(), ValidationError> {
    // Check file size
    if content.len() > 100 * 1024 * 1024 {  // 100MB limit
        return Err(ValidationError::FileTooLarge);
    }
    
    // Check file signature (PDF magic bytes)
    if !content.starts_with(b"%PDF-") {
        return Err(ValidationError::InvalidFileType);
    }
    
    // Check for embedded executables or suspicious content
    if content.windows(4).any(|w| w == b"\x4D\x5A\x90\x00") {  // PE header
        return Err(ValidationError::SuspiciousContent);
    }
    
    Ok(())
}
```

### 2. Directory Traversal Prevention

```rust
use std::path::{Component, Path, PathBuf};

pub fn resolve_safe_path(base: &Path, user_path: &str) -> Result<PathBuf, SecurityError> {
    let requested_path = Path::new(user_path);
    
    // Start with the safe base directory
    let mut safe_path = base.to_path_buf();
    
    // Process each component safely
    for component in requested_path.components() {
        match component {
            Component::Normal(name) => {
                // Validate filename
                let name_str = name.to_string_lossy();
                if name_str.contains("..") || name_str.contains('\0') {
                    return Err(SecurityError::InvalidPath);
                }
                safe_path.push(name);
            }
            Component::ParentDir => {
                return Err(SecurityError::PathTraversalAttempt);
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(SecurityError::AbsolutePathNotAllowed);
            }
            Component::CurDir => {
                // Ignore current directory references
                continue;
            }
        }
    }
    
    // Ensure the resolved path is still within the base directory
    if !safe_path.starts_with(base) {
        return Err(SecurityError::PathOutsideBase);
    }
    
    Ok(safe_path)
}
```

## Application Security

### 1. Memory Safety

Rust provides memory safety by default, but additional precautions:

```rust
// Use bounded collections
use std::collections::VecDeque;

pub struct BoundedCache<T> {
    items: VecDeque<T>,
    max_size: usize,
}

impl<T> BoundedCache<T> {
    pub fn push(&mut self, item: T) {
        if self.items.len() >= self.max_size {
            self.items.pop_front();
        }
        self.items.push_back(item);
    }
}

// Explicit memory limits
pub const MAX_DOWNLOAD_SIZE: usize = 100 * 1024 * 1024;  // 100MB
pub const MAX_CONCURRENT_DOWNLOADS: usize = 3;
pub const MAX_CACHE_ENTRIES: usize = 1000;
```

### 2. Error Handling Security

```rust
// Avoid information leakage in error messages
#[derive(Debug)]
pub enum PublicError {
    InvalidInput,
    ServiceUnavailable,
    InternalError,
}

impl From<InternalError> for PublicError {
    fn from(err: InternalError) -> Self {
        // Log detailed error internally
        tracing::error!("Internal error: {:?}", err);
        
        // Return generic error to client
        match err {
            InternalError::Database(_) => PublicError::InternalError,
            InternalError::Network(_) => PublicError::ServiceUnavailable,
            InternalError::Validation(_) => PublicError::InvalidInput,
            _ => PublicError::InternalError,
        }
    }
}
```

### 3. Secure Logging

```rust
use tracing::{info, warn, error};
use serde_json::json;

// Structured logging without sensitive data
pub fn log_request(request_id: &str, endpoint: &str, client_ip: &str) {
    info!(
        request_id = request_id,
        endpoint = endpoint,
        client_ip = anonymize_ip(client_ip),
        "Request received"
    );
}

fn anonymize_ip(ip: &str) -> String {
    // Anonymize last octet for IPv4
    if let Ok(addr) = ip.parse::<std::net::Ipv4Addr>() {
        let octets = addr.octets();
        format!("{}.{}.{}.xxx", octets[0], octets[1], octets[2])
    } else {
        "anonymized".to_string()
    }
}

// Never log sensitive information
pub fn log_search_query(query: &str) {
    // Hash the query for correlation while preserving privacy
    let query_hash = sha256::digest(query);
    info!(query_hash = &query_hash[0..16], "Search performed");
}
```

## Configuration Security

### 1. Secure Configuration Storage

```rust
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;

pub fn save_config_securely(config: &Config, path: &Path) -> Result<(), ConfigError> {
    // Create parent directories with secure permissions
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        let permissions = Permissions::from_mode(0o700);
        std::fs::set_permissions(parent, permissions)?;
    }
    
    // Write config with secure permissions
    let config_data = toml::to_string(config)?;
    std::fs::write(path, config_data)?;
    
    // Set file permissions to owner-only
    let permissions = Permissions::from_mode(0o600);
    std::fs::set_permissions(path, permissions)?;
    
    Ok(())
}
```

### 2. Environment Variable Security

```rust
// Secure environment variable handling
pub fn load_sensitive_config() -> Result<Config, ConfigError> {
    let mut config = Config::default();
    
    // Load non-sensitive config from file
    config.load_from_file("~/.config/rust-sci-hub-mcp/config.toml")?;
    
    // Override with environment variables (higher priority)
    if let Ok(port) = std::env::var("RUST_SCI_HUB_MCP_PORT") {
        config.server.port = port.parse().map_err(|_| ConfigError::InvalidPort)?;
    }
    
    // Validate configuration
    config.validate()?;
    
    Ok(config)
}

// Clear sensitive environment variables after use
pub fn clear_sensitive_env_vars() {
    let sensitive_vars = [
        "RUST_SCI_HUB_MCP_API_KEY",
        "RUST_SCI_HUB_MCP_PASSWORD",
    ];
    
    for var in &sensitive_vars {
        std::env::remove_var(var);
    }
}
```

## Operational Security

### 1. Security Monitoring

```rust
use tracing::{warn, error};

pub struct SecurityMonitor {
    failed_requests: HashMap<String, u32>,
    suspicious_patterns: Vec<String>,
}

impl SecurityMonitor {
    pub fn check_request(&mut self, client_ip: &str, request: &Request) {
        // Monitor for suspicious patterns
        let user_agent = request.headers().get("user-agent")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        
        // Check for automated tools or scanners
        let suspicious_agents = [
            "sqlmap", "nmap", "nikto", "burp", "owasp",
            "python-requests", "curl", "wget"
        ];
        
        if suspicious_agents.iter().any(|&agent| user_agent.contains(agent)) {
            warn!(
                client_ip = client_ip,
                user_agent = user_agent,
                "Suspicious user agent detected"
            );
        }
        
        // Monitor for rapid requests from same IP
        let count = self.failed_requests.entry(client_ip.to_string()).or_insert(0);
        *count += 1;
        
        if *count > 10 {
            error!(
                client_ip = client_ip,
                failed_count = count,
                "Potential brute force attack detected"
            );
        }
    }
}
```

### 2. Audit Logging

```rust
use chrono::Utc;
use serde_json::json;

pub fn audit_log(event: AuditEvent) {
    let audit_entry = json!({
        "timestamp": Utc::now().to_rfc3339(),
        "event_type": event.event_type,
        "severity": event.severity,
        "details": event.details,
        "correlation_id": event.correlation_id,
    });
    
    // Write to secure audit log
    let audit_file = "~/.config/rust-sci-hub-mcp/audit.log";
    append_to_secure_log(audit_file, &audit_entry.to_string());
}

#[derive(Debug)]
pub struct AuditEvent {
    pub event_type: String,
    pub severity: String,
    pub details: serde_json::Value,
    pub correlation_id: String,
}
```

### 3. Update Management

```bash
#!/bin/bash
# Security update script

set -euo pipefail

echo "Checking for security updates..."

# Update Rust toolchain
rustup update stable

# Audit dependencies for vulnerabilities
cargo audit

# Update dependencies with security patches
cargo update

# Run security tests
cargo nextest run security_tests

# Check for outdated dependencies
cargo outdated

echo "Security update check complete"
```

## Incident Response

### 1. Security Incident Detection

```rust
pub enum SecurityIncident {
    UnauthorizedAccess {
        client_ip: String,
        attempted_action: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    SuspiciousActivity {
        pattern: String,
        frequency: u32,
        source: String,
    },
    ConfigurationTampering {
        file_path: String,
        checksum_mismatch: bool,
    },
    ResourceExhaustion {
        resource_type: String,
        current_usage: f64,
        threshold: f64,
    },
}

impl SecurityIncident {
    pub fn handle(&self) {
        match self {
            SecurityIncident::UnauthorizedAccess { client_ip, .. } => {
                // Log incident
                error!("Unauthorized access attempt from {}", client_ip);
                
                // Rate limit or block IP
                self.apply_rate_limit(client_ip);
                
                // Alert administrators
                self.send_security_alert();
            }
            _ => {
                // Handle other incident types
            }
        }
    }
}
```

### 2. Emergency Response Procedures

```rust
pub struct EmergencyResponse;

impl EmergencyResponse {
    pub async fn shutdown_emergency() {
        error!("EMERGENCY SHUTDOWN INITIATED");
        
        // Stop accepting new requests
        // Finish processing current requests
        // Save critical data
        // Shutdown gracefully
        
        std::process::exit(1);
    }
    
    pub async fn isolate_service() {
        warn!("Service isolation mode activated");
        
        // Disconnect from external networks
        // Preserve evidence
        // Alert monitoring systems
    }
}
```

## Security Checklist

### Pre-Deployment Security Review

- [ ] All inputs are validated and sanitized
- [ ] File permissions are set correctly (600 for files, 700 for directories)
- [ ] Network binding is localhost-only
- [ ] HTTPS is enforced for external requests
- [ ] Rate limiting is implemented and tested
- [ ] Error messages don't leak sensitive information
- [ ] Dependencies are audited for vulnerabilities
- [ ] Configuration files are secured
- [ ] Logging doesn't include sensitive data
- [ ] Resource limits are enforced

### Runtime Security Monitoring

- [ ] Monitor for failed authentication attempts
- [ ] Track resource usage and limits
- [ ] Log security-relevant events
- [ ] Monitor file system integrity
- [ ] Check for suspicious network patterns
- [ ] Validate SSL certificates regularly
- [ ] Monitor for configuration changes
- [ ] Track download patterns for anomalies

### Regular Security Maintenance

- [ ] Update dependencies monthly
- [ ] Run security audit tools
- [ ] Review and rotate logs
- [ ] Test backup and recovery procedures
- [ ] Review access permissions
- [ ] Update threat model
- [ ] Conduct penetration testing
- [ ] Review incident response procedures

### User Security Guidelines

#### For End Users

1. **Keep software updated**: Always use the latest version
2. **Secure configuration**: Use default secure settings
3. **Monitor downloads**: Review downloaded files before opening
4. **Network security**: Don't expose the service externally
5. **Report issues**: Report security concerns immediately

#### For Administrators

1. **Access control**: Limit who can modify configurations
2. **Monitoring**: Implement comprehensive logging and monitoring
3. **Backup security**: Encrypt and secure backup files
4. **Incident response**: Have a security incident response plan
5. **Regular audits**: Conduct regular security reviews

### Security Resources

- **OWASP Top 10**: Web application security risks
- **Rust Security Guidelines**: Rust-specific security best practices
- **CVE Database**: Common vulnerabilities and exposures
- **CIS Controls**: Center for Internet Security controls
- **NIST Cybersecurity Framework**: Comprehensive security framework

For security questions or to report vulnerabilities, please contact the security team following responsible disclosure practices.
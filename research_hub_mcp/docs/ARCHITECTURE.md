# Architecture Documentation

This document describes the system architecture, design decisions, and technical implementation of the rust-sci-hub-mcp server.

## Table of Contents

- [High-Level Architecture](#high-level-architecture)
- [Component Overview](#component-overview)
- [Data Flow](#data-flow)
- [Module Architecture](#module-architecture)
- [Design Patterns](#design-patterns)
- [Technology Choices](#technology-choices)
- [Security Architecture](#security-architecture)
- [Performance Considerations](#performance-considerations)
- [Deployment Architecture](#deployment-architecture)

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Rust Sci-Hub MCP Server                     │
├─────────────────────────────────────────────────────────────────┤
│                          Client Layer                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │  Claude Desktop │  │   HTTP Clients  │  │   Direct API    │ │
│  │      (MCP)      │  │  (curl, etc.)   │  │     Calls       │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Transport Layer                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │   MCP Protocol  │  │   HTTP Server   │  │     Stdio       │ │
│  │   (JSON-RPC)    │  │   (REST API)    │  │   Transport     │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Service Layer                            │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │   MCP Server    │  │   Tool Handler  │  │   Configuration │ │
│  │   (Handler)     │  │   (Dispatcher)  │  │    Manager      │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Business Logic Layer                     │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │   Search Tool   │  │  Download Tool  │  │ Metadata Tool   │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Infrastructure Layer                     │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │  Sci-Hub Client │  │ Resilience      │  │  File System    │ │
│  │  (HTTP + Parse) │  │ (Retry/Circuit) │  │   Operations    │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                       External Services                        │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │  Sci-Hub.se     │  │   Sci-Hub.st    │  │   Sci-Hub.ru    │ │
│  │   (Primary)     │  │   (Fallback)    │  │   (Fallback)    │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Component Overview

### Core Components

1. **MCP Server (`src/server/`)**
   - Implements the Model Context Protocol specification
   - Handles JSON-RPC communication with clients
   - Manages tool registration and lifecycle
   - Provides stdio and HTTP transport options

2. **Tools (`src/tools/`)**
   - **SearchTool**: Searches for academic papers by DOI, title, or author
   - **DownloadTool**: Downloads papers from Sci-Hub mirrors
   - **MetadataTool**: Extracts bibliographic metadata from PDFs

3. **Sci-Hub Client (`src/client/`)**
   - Manages multiple Sci-Hub mirror URLs
   - Handles HTTP requests with retry logic
   - Parses HTML responses to extract download links
   - Implements rate limiting and circuit breakers

4. **Configuration System (`src/config/`)**
   - Loads configuration from multiple sources
   - Validates configuration parameters
   - Supports hot reloading and environment variables
   - Provides type-safe configuration structs

5. **Service Management (`src/service/`)**
   - Background daemon functionality
   - Signal handling and graceful shutdown
   - Health checks and monitoring
   - Integration with macOS LaunchAgent

6. **Resilience Framework (`src/resilience/`)**
   - Circuit breaker pattern implementation
   - Exponential backoff retry logic
   - Timeout handling and health checks
   - Error categorization and recovery strategies

## Data Flow

### Search Request Flow

```
Claude Desktop → MCP Protocol → Server Handler → Search Tool → Sci-Hub Client → Mirror Selection → HTTP Request → HTML Parsing → Results → Response Chain
```

### Download Request Flow

```
Tool Request → Download Tool → URL Validation → Sci-Hub Client → Mirror Health Check → HTTP Download → File Validation → Storage → Response
```

### Error Handling Flow

```
Error Occurrence → Error Categorization → Retry Decision → Circuit Breaker Check → Backoff Calculation → Retry Attempt or Failure Response
```

## Module Architecture

### `src/main.rs` - Entry Point
- CLI argument parsing with `clap`
- Environment setup and logging initialization
- Service mode selection (daemon vs. interactive)
- Graceful shutdown signal handling

### `src/lib.rs` - Library Interface
- Public API exports
- Module organization
- Common type definitions
- Integration points for external users

### `src/server/` - MCP Server Implementation

```rust
// Server structure
pub struct Server {
    config: Arc<Config>,
    handler: Arc<ServerHandler>,
    transport: Transport,
    shutdown: Arc<AtomicBool>,
}

// Key responsibilities:
// - MCP protocol implementation
// - Request routing and validation  
// - Transport abstraction (stdio/HTTP)
// - Tool lifecycle management
```

### `src/tools/` - Business Logic

```rust
// Tool trait for MCP integration
#[async_trait]
pub trait McpTool {
    async fn call(&self, params: Value) -> Result<Value>;
    fn schema(&self) -> ToolSchema;
    fn name(&self) -> &str;
}

// Each tool implements:
// - Input validation and sanitization
// - Business logic execution
// - Error handling and recovery
// - Response formatting
```

### `src/client/` - External Integration

```rust
// Sci-Hub client with resilience
pub struct SciHubClient {
    mirrors: Arc<Vec<Mirror>>,
    circuit_breakers: HashMap<String, CircuitBreaker>,
    rate_limiter: RateLimiter,
    http_client: reqwest::Client,
}

// Key features:
// - Automatic mirror failover
// - Request deduplication
// - Response caching
// - Health monitoring
```

### `src/config/` - Configuration Management

```rust
// Layered configuration system
#[derive(Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub sci_hub: SciHubConfig,
    pub downloads: DownloadsConfig,
    pub logging: LoggingConfig,
}

// Configuration sources (priority order):
// 1. Command-line arguments
// 2. Environment variables
// 3. Configuration files
// 4. Built-in defaults
```

### `src/resilience/` - Reliability Infrastructure

```rust
// Circuit breaker implementation
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    config: CircuitBreakerConfig,
    metrics: Arc<RwLock<CircuitMetrics>>,
}

// Retry policy with backoff
pub struct RetryPolicy {
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
    jitter: bool,
}
```

## Design Patterns

### 1. Hexagonal Architecture (Ports & Adapters)

```
┌─────────────────┐
│   Application   │  ← Core business logic (tools)
│      Core       │
└─────────────────┘
         │
┌─────────────────┐
│     Ports       │  ← Interfaces (traits)
│  (Abstractions) │
└─────────────────┘
         │
┌─────────────────┐
│    Adapters     │  ← Implementations (HTTP, file system)
│ (Concrete Impl) │
└─────────────────┘
```

**Benefits**:
- Testability through dependency injection
- Clean separation of concerns
- Easy to swap implementations
- Framework-independent core logic

### 2. Circuit Breaker Pattern

```rust
// Prevents cascade failures
match circuit_breaker.call(|| sci_hub_request()).await {
    Ok(response) => handle_success(response),
    Err(Error::CircuitBreakerOpen) => use_fallback(),
    Err(other) => handle_error(other),
}
```

**Benefits**:
- Fail-fast for known bad services
- Automatic recovery testing
- System stability under load
- Graceful degradation

### 3. Repository Pattern

```rust
// Abstract data access
#[async_trait]
pub trait PaperRepository {
    async fn find_by_doi(&self, doi: &str) -> Result<Option<Paper>>;
    async fn save(&self, paper: &Paper) -> Result<()>;
}

// Concrete implementations
pub struct SciHubRepository { /* ... */ }
pub struct CacheRepository { /* ... */ }
```

**Benefits**:
- Consistent data access interface
- Easy testing with mock repositories
- Pluggable storage backends
- Clean business logic

### 4. Command Pattern (MCP Tools)

```rust
// Each tool is a command
pub struct SearchCommand {
    query: String,
    search_type: SearchType,
    limit: Option<u32>,
}

impl Command for SearchCommand {
    async fn execute(&self) -> Result<Value> {
        // Implementation
    }
}
```

**Benefits**:
- Uniform tool interface
- Easy to add new tools
- Request/response validation
- Audit trail support

### 5. Strategy Pattern (Mirror Selection)

```rust
// Different mirror selection strategies
pub trait MirrorStrategy {
    fn select_mirror(&self, mirrors: &[Mirror]) -> Option<&Mirror>;
}

pub struct HealthBasedStrategy;
pub struct RoundRobinStrategy;
pub struct RandomStrategy;
```

**Benefits**:
- Flexible mirror selection logic
- Easy to test different strategies
- Runtime strategy switching
- Performance optimization

## Technology Choices

### Core Technologies

| Technology | Purpose | Justification |
|------------|---------|---------------|
| **Rust** | Primary language | Memory safety, performance, ecosystem |
| **Tokio** | Async runtime | Industry standard, excellent ecosystem |
| **reqwest** | HTTP client | Robust, feature-rich, well-maintained |
| **serde** | Serialization | De facto standard, excellent performance |
| **thiserror** | Error handling | Ergonomic error types, good practices |

### Framework Choices

| Framework | Purpose | Alternatives Considered |
|-----------|---------|------------------------|
| **rmcp** | MCP protocol | Custom implementation |
| **clap** | CLI parsing | structopt, argh |
| **tracing** | Logging | log + env_logger |
| **config** | Configuration | figment, confy |
| **tokio-retry** | Retry logic | backoff, custom |

### Database/Storage

| Technology | Purpose | Justification |
|------------|---------|---------------|
| **sled** | Embedded DB | Zero-config, ACID, performant |
| **bincode** | Serialization | Compact, fast binary format |
| **File system** | Paper storage | Simple, reliable, user-accessible |

### Development Tools

| Tool | Purpose | Benefits |
|------|---------|----------|
| **clippy** | Linting | Catches common mistakes, enforces best practices |
| **rustfmt** | Formatting | Consistent code style |
| **cargo-audit** | Security | Vulnerability scanning |
| **tarpaulin** | Coverage | Test coverage reporting |
| **criterion** | Benchmarking | Statistical performance testing |

## Security Architecture

### Defense in Depth

```
┌─────────────────────────────────────────────────────────────────┐
│                    Network Security                            │
│  • Localhost binding only                                      │
│  • No external network exposure                                │
│  • HTTPS for external requests                                 │
└─────────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────────┐
│                 Application Security                           │
│  • Input validation and sanitization                           │
│  • SQL injection prevention                                    │
│  • Path traversal protection                                   │
│  • Rate limiting and DoS protection                            │
└─────────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────────┐
│                  System Security                               │
│  • File permission restrictions                                │
│  • Process isolation                                           │
│  • Secure configuration storage                                │
│  • Audit logging                                               │
└─────────────────────────────────────────────────────────────────┘
```

### Security Controls

1. **Input Validation**
   ```rust
   // Example: DOI validation
   pub fn validate_doi(doi: &str) -> Result<()> {
       if !DOI_REGEX.is_match(doi) {
           return Err(Error::InvalidInput {
               field: "doi".to_string(),
               reason: "Invalid DOI format".to_string(),
           });
       }
       Ok(())
   }
   ```

2. **File System Security**
   ```rust
   // Secure file creation
   use std::os::unix::fs::OpenOptionsExt;
   
   File::create(&path)
       .mode(0o600)  // Owner read/write only
       .create(true)
       .write(true)
   ```

3. **HTTP Security**
   ```rust
   // Secure HTTP client configuration
   let client = reqwest::Client::builder()
       .timeout(Duration::from_secs(30))
       .redirect(reqwest::redirect::Policy::limited(3))
       .user_agent("rust-sci-hub-mcp/1.0")
       .build()?;
   ```

## Performance Considerations

### Concurrency Model

```rust
// Async/await for I/O bound operations
async fn download_paper(url: &str) -> Result<Vec<u8>> {
    let response = client.get(url).send().await?;
    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
}

// Semaphore for limiting concurrent downloads
let semaphore = Arc::new(Semaphore::new(3));
let permit = semaphore.acquire().await?;
let result = download_paper(url).await;
drop(permit);
```

### Memory Management

1. **Streaming Downloads**
   ```rust
   // Stream large files to avoid loading into memory
   let mut stream = response.bytes_stream();
   while let Some(chunk) = stream.try_next().await? {
       file.write_all(&chunk).await?;
   }
   ```

2. **Connection Pooling**
   ```rust
   // Reuse HTTP connections
   let client = reqwest::Client::builder()
       .pool_max_idle_per_host(10)
       .pool_idle_timeout(Duration::from_secs(30))
       .build()?;
   ```

3. **Caching Strategy**
   ```rust
   // LRU cache for metadata
   use lru::LruCache;
   
   let mut cache: LruCache<String, PaperMetadata> = 
       LruCache::new(NonZeroUsize::new(1000).unwrap());
   ```

### Performance Monitoring

```rust
// Metrics collection
pub struct Metrics {
    pub requests_total: Counter,
    pub request_duration: Histogram,
    pub active_downloads: Gauge,
    pub cache_hits: Counter,
}

// Performance-critical paths
let start = Instant::now();
let result = search_papers(query).await;
metrics.request_duration.observe(start.elapsed().as_secs_f64());
```

## Deployment Architecture

### macOS LaunchAgent

```xml
<!-- ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist -->
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.rust-sci-hub-mcp</string>
    
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/rust-sci-hub-mcp</string>
        <string>--daemon</string>
    </array>
    
    <key>RunAtLoad</key>
    <true/>
    
    <key>KeepAlive</key>
    <dict>
        <key>Crashed</key>
        <true/>
    </dict>
</dict>
</plist>
```

### Directory Structure

```
~/.config/rust-sci-hub-mcp/
├── config.toml              # User configuration
├── cache.db                 # Metadata cache
└── logs/
    ├── service.log          # Application logs
    ├── access.log           # Request logs
    └── error.log            # Error logs

~/Downloads/papers/          # Default download location
├── 2024/                    # Organized by date (optional)
│   ├── 01/
│   └── 02/
└── metadata.json           # Bulk metadata export
```

### Process Management

```bash
# Service lifecycle
launchctl load ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist
launchctl start com.rust-sci-hub-mcp
launchctl stop com.rust-sci-hub-mcp
launchctl unload ~/Library/LaunchAgents/com.rust-sci-hub-mcp.plist

# Health monitoring
curl http://localhost:8080/health
tail -f ~/.config/rust-sci-hub-mcp/logs/service.log
```

### Resource Limits

```toml
# Configuration for resource management
[server]
max_connections = 100
request_timeout_secs = 30
graceful_shutdown_timeout_secs = 5

[downloads]
max_concurrent = 3
max_file_size_mb = 100
cleanup_after_days = 30

[sci_hub]
rate_limit_per_sec = 1
max_retries = 3
timeout_secs = 30
```

## Future Architecture Considerations

### Scalability Improvements

1. **Distributed Caching**: Redis for shared metadata cache
2. **Load Balancing**: Multiple server instances
3. **Database Upgrade**: PostgreSQL for advanced querying
4. **Message Queue**: RabbitMQ for async processing

### Extensibility Points

1. **Plugin System**: Dynamic tool loading
2. **Custom Protocols**: Support for other academic databases
3. **ML Integration**: Paper recommendation system
4. **API Gateway**: Rate limiting and authentication

### Monitoring and Observability

1. **Metrics**: Prometheus integration
2. **Tracing**: Distributed tracing with Jaeger
3. **Alerting**: Alert manager for error conditions
4. **Dashboards**: Grafana for visualization
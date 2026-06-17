# knowledge_accumulator_mcp

Academic knowledge accumulation MCP server. Personal research use only.

## Commands
```bash
cargo nextest run                # Run tests (parallel)
cargo nextest run TEST_NAME      # Run specific test
cargo clippy -- -D warnings      # Must pass before commit
cargo fmt                        # Format code
cargo build --release           # Production build
cargo run -- serve              # Start MCP server
cargo tarpaulin --out Html      # Coverage report
cargo audit                     # Security check
```

## TDD Workflow
1. Write failing test first
2. Run `cargo nextest run TEST_NAME` to verify failure
3. Write minimal code to pass
4. Verify test passes
5. Refactor if needed
6. Run full suite before commit

## Code Style
- Use `thiserror` for errors - no raw strings
- Use `tracing` for logging - never println
- Use `?` operator - avoid unwrap/expect
- Prefer `async/await` over manual futures
- Keep functions under 50 lines
- Test naming: `test_<function>_<case>_<expected>`

## Project Structure
```
src/
├── main.rs         # CLI entry point
├── server.rs       # MCP server implementation
├── tools/          # MCP tool implementations
├── client.rs       # External API client
├── config.rs       # Configuration handling
└── error.rs        # Error types
tests/              # Integration tests
```

## MCP Tool Pattern
```rust
use rmcp::prelude::*;
use schemars::JsonSchema;

#[derive(Debug, Deserialize, JsonSchema)]
struct InputSchema {
    #[schemars(description = "Clear description")]
    field: String,
}

#[tool]
async fn tool_name(input: InputSchema) -> Result<Value> {
    // Validate input
    // Call service with timeout
    // Return JSON response
}
```

## Error Handling
```rust
#[derive(Error, Debug)]
enum AppError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("Invalid input: {0}")]
    Validation(String),
}

// Always use timeout
let result = tokio::time::timeout(
    Duration::from_secs(30),
    client.get(url).send()
).await??;
```

## Testing
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    
    #[tokio::test]
    async fn test_function_success() {
        // Arrange
        let mock_server = MockServer::start().await;
        Mock::given(matchers::method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;
        
        // Act
        let result = function_under_test().await;
        
        // Assert
        assert!(result.is_ok());
    }
}
```

## Async Patterns
- Use `tokio::spawn` for parallel work
- Use `tokio::select!` for cancellation
- Bounded channels only: `mpsc::channel(100)`
- Share state: `Arc<RwLock<T>>`
- No blocking in async context

## HTTP Client
```rust
use once_cell::sync::Lazy;

static CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(2)
        .build()
        .expect("Failed to create client")
});
```

## Configuration
Order of precedence:
1. CLI arguments
2. Environment variables (`RUST_RESEARCH_MCP_*`)
3. config.toml
4. Defaults

## Rate Limiting
```rust
use governor::{Quota, RateLimiter};

// 1 request per second
let limiter = RateLimiter::direct(Quota::per_second(1));
limiter.until_ready().await;
```

## Circuit Breaker
```rust
use circuit_breaker::CircuitBreaker;

let breaker = CircuitBreaker::new()
    .error_threshold(3)
    .timeout(Duration::from_secs(60));

match breaker.call(async_operation).await {
    Ok(result) => process(result),
    Err(_) => fallback_strategy(),
}
```

## Git Workflow
```bash
git checkout -b feature/task-name
# TDD: Write tests first
cargo nextest run --no-capture
# Implement feature
cargo fmt && cargo clippy -- -D warnings
cargo nextest run
git add -A && git commit -m "feat: description"
git push origin feature/task-name
```

## macOS Service
```bash
# Install
cargo install --path .
# Start
launchctl load ~/Library/LaunchAgents/rust-research-mcp.plist
# Logs
tail -f ~/Library/Logs/rust-research-mcp/service.log
# Stop
launchctl unload ~/Library/LaunchAgents/rust-research-mcp.plist
```

## Performance Targets
- Response time: <500ms
- Memory usage: <100MB idle
- Startup time: <2s
- CPU usage: <5% idle

## Security
- Validate all inputs
- Use HTTPS only
- No secrets in logs
- Config files: mode 0600
- Sanitize file paths

## Common Issues

### Async runtime panic
```rust
// Wrong: blocking in async
std::thread::sleep(duration);

// Right: async sleep
tokio::time::sleep(duration).await;
```

### Test timeout
```rust
#[tokio::test(flavor = "multi_thread")]
async fn long_test() {
    // Allows parallel execution
}
```

### Serialization
```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    field_name: String,
}
```

## DO NOT
- Use `println!` - use `tracing`
- Use `.unwrap()` - use `?` or `.expect()`
- Create unbounded channels
- Block in async functions
- Log sensitive data
- Skip error handling

## ALWAYS
- Write test first (TDD)
- Validate inputs
- Use timeouts for I/O
- Handle all Results
- Run clippy before commit
- Document public APIs

## Deployment

### 1. Build Distribution
```bash
# Clean previous builds
rm -rf build/
mkdir -p build/dist

# Build release binary
cargo build --release

# Copy binary and config
cp target/release/rust-research-mcp build/dist/
cp config.example.toml build/dist/config.toml

# Create run script
cat > build/dist/run.sh << 'EOF'
#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/rust-research-mcp" serve --config "$SCRIPT_DIR/config.toml"
EOF
chmod +x build/dist/run.sh

# Package
cd build && tar -czf rust-research-mcp.tar.gz dist/
echo "Distribution created: build/rust-research-mcp.tar.gz"
```

### 2. Install in Claude Desktop
```bash
# Extract distribution
cd ~/Documents  # or preferred location
tar -xzf path/to/rust-research-mcp.tar.gz
mv dist rust-research-mcp

# Configure Claude Desktop
cat >> ~/Library/Application\ Support/Claude/claude_desktop_config.json << 'EOF'
{
  "mcpServers": {
    "rust-research-mcp": {
      "command": "/Users/$USER/Documents/rust-research-mcp/run.sh",
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
EOF

# Restart Claude Desktop
osascript -e 'quit app "Claude"'
sleep 2
open -a "Claude"
```

### 3. Install in Claude Code
```bash
# Global installation
cargo install --path . --root ~/.local

# Configure MCP for Claude Code
cat > ~/.claude/mcp.json << 'EOF'
{
  "mcpServers": {
    "rust-research-mcp": {
      "command": "rust-research-mcp",
      "args": ["serve"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
EOF

# Alternative: Project-specific installation
cat > .mcp.json << 'EOF'
{
  "mcpServers": {
    "rust-research-mcp": {
      "command": "./build/dist/rust-research-mcp",
      "args": ["serve"],
      "env": {
        "RUST_LOG": "debug"
      }
    }
  }
}
EOF
```

### 4. Test Installation

#### Claude Desktop Test
```bash
# 1. Open Claude Desktop
# 2. Type: "List available MCP tools"
# 3. Should see: search_papers, download_paper, extract_metadata

# Check if server is running
ps aux | grep rust-research-mcp

# Test a tool
# In Claude: "Search for papers about 'machine learning'"
```

#### Claude Code Test
```bash
# Start Claude Code with debug
claude --mcp-debug

# In Claude Code session:
# Type: /mcp
# Should list rust-research-mcp as available

# Test tool discovery
# Type: "What MCP tools are available?"

# Test tool execution
# Type: "Search for recent papers on rust programming"
```

### 5. Check Logs

#### Claude Desktop Logs
```bash
# View server logs
tail -f ~/Library/Logs/Claude/mcp-rust-research-mcp.log

# Check for startup
grep "Server started" ~/Library/Logs/Claude/mcp-rust-research-mcp.log

# Check for errors
grep ERROR ~/Library/Logs/Claude/mcp-rust-research-mcp.log

# Claude Desktop console
open ~/Library/Logs/Claude/claude.log
```

#### Claude Code Logs
```bash
# Enable verbose logging
export RUST_LOG=debug

# Run with MCP debug
claude --mcp-debug --verbose

# Check MCP connection
grep "MCP server connected" ~/.claude/logs/session.log

# Monitor real-time
tail -f ~/.claude/logs/session.log | grep rust-research-mcp
```

### Troubleshooting

#### Server won't start
```bash
# Check permissions
chmod +x build/dist/rust-research-mcp
chmod +x build/dist/run.sh

# Test manually
./build/dist/run.sh
# Should output: "MCP server listening on stdio"
```

#### Tools not appearing
```bash
# Verify config syntax
python3 -m json.tool < ~/.claude/mcp.json

# Test server directly
echo '{"jsonrpc":"2.0","method":"tools/list","id":1}' | ./build/dist/rust-research-mcp
```

#### Permission denied
```bash
# macOS security
xattr -d com.apple.quarantine build/dist/rust-research-mcp
spctl --add --label "MCP" build/dist/rust-research-mcp
```
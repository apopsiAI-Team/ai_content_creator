# User Guide

This guide covers common usage scenarios and workflows for the rust-sci-hub-mcp server.

## Table of Contents

- [Getting Started](#getting-started)
- [Common Workflows](#common-workflows)
- [Configuration](#configuration)
- [Integration with Claude Desktop](#integration-with-claude-desktop)
- [Usage Examples](#usage-examples)
- [Best Practices](#best-practices)
- [Tips and Tricks](#tips-and-tricks)

## Getting Started

### First Time Setup

1. **Install the server**:
   ```bash
   # Recommended: Homebrew installation
   git clone https://github.com/Ladvien/sci_hub_mcp.git
   cd sci_hub_mcp
   brew install --build-from-source homebrew/rust-sci-hub-mcp.rb
   
   # Alternative: Source installation
   cargo build --release
   ```

2. **Configure Sci-Hub mirrors** (required):
   ```bash
   # Create config directory
   mkdir -p ~/.config/rust-sci-hub-mcp
   
   # Add required mirror configuration
   echo '[sci_hub]
   mirrors = [
       "https://sci-hub.se",
       "https://sci-hub.st", 
       "https://sci-hub.ru"
   ]' >> ~/.config/rust-sci-hub-mcp/config.toml
   ```

3. **Start the service**:
   ```bash
   # Via Homebrew
   brew services start rust-sci-hub-mcp
   
   # Or run manually
   rust-sci-hub-mcp --daemon
   ```

4. **Verify it's working**:
   ```bash
   # Check version
   rust-sci-hub-mcp --version
   
   # Check service status
   brew services list | grep rust-sci-hub-mcp
   
   # Note: This is an MCP server (not HTTP), so no health endpoint
   ```

### Basic Configuration

The server automatically creates a default configuration at:
- macOS: `~/.config/rust-sci-hub-mcp/config.toml`
- Linux: `~/.config/rust-sci-hub-mcp/config.toml`

## Common Workflows

### 1. Searching for Academic Papers

The rust-sci-hub-mcp server uses the Model Context Protocol (MCP) for communication, primarily through Claude Desktop integration. Direct API calls are made via MCP JSON-RPC protocol.

#### Using MCP Tools via Claude Desktop

When integrated with Claude Desktop, you can use natural language to interact with the search tools:

**User**: "Search for papers about deep learning by Geoffrey Hinton"
**Claude**: *Uses the search_papers MCP tool internally*

**User**: "Find the paper with DOI 10.1038/nature12373"
**Claude**: *Uses the search_papers MCP tool with DOI search type*

#### MCP Tool Parameters

The search tool accepts these parameters:
- `query`: Search string (DOI, title, author name)
- `search_type`: One of "doi", "title", "author" (default: "title")
- `limit`: Maximum results to return (default: 10)
- `offset`: Pagination offset (default: 0)

### 2. Downloading Papers

#### Using MCP Tools via Claude Desktop

**User**: "Download the paper with DOI 10.1038/nature12373"
**Claude**: *Uses the download_paper MCP tool*

**User**: "Download that first search result to my papers folder"
**Claude**: *Uses the download_paper MCP tool with appropriate parameters*

#### MCP Tool Parameters

The download tool accepts these parameters:
- `doi`: DOI of the paper to download
- `url`: Direct URL to paper (alternative to DOI)
- `filename`: Optional custom filename
- `destination`: Optional download directory override

### 3. Extracting Metadata

#### Using MCP Tools via Claude Desktop

**User**: "Extract metadata from the paper I just downloaded"
**Claude**: *Uses the extract_metadata MCP tool*

**User**: "Get bibliographic information from all papers in my downloads folder"
**Claude**: *Uses the extract_metadata MCP tool with batch processing*

#### MCP Tool Parameters

The metadata extraction tool accepts these parameters:
- `file_path`: Path to PDF file for metadata extraction
- `directory`: Directory path for bulk metadata extraction
- `format`: Output format ("bibtex", "json", "ris") - default: "json"
- `include_content`: Whether to include abstract/content excerpts

## Configuration

### Server Configuration

```toml
[server]
host = "127.0.0.1"      # Bind address for health checks (localhost only for security)
port = 8080             # Health check port (MCP communication via stdio)
timeout_secs = 30       # Request timeout
health_check_interval_secs = 30
graceful_shutdown_timeout_secs = 5
```

### Sci-Hub Configuration

```toml
[sci_hub]
# Mirror URLs (auto-discovered if empty)
mirrors = [
    "https://sci-hub.se",
    "https://sci-hub.st",
    "https://sci-hub.ru"
]
timeout_secs = 30       # Request timeout
rate_limit_per_sec = 1  # Requests per second (be respectful!)
max_retries = 3         # Number of retry attempts
```

### Download Configuration

```toml
[downloads]
directory = "~/Downloads/papers"  # Where to save papers
max_concurrent = 3               # Concurrent downloads
max_file_size_mb = 100          # Maximum file size
organize_by_date = false        # Organize into date folders
verify_integrity = true         # Verify downloaded files
```

### Logging Configuration

```toml
[logging]
level = "info"                  # debug, info, warn, error
format = "json"                 # json or pretty
file = "~/Library/Logs/rust-sci-hub-mcp/service.log"
```

## Integration with Claude Desktop

### Quick Setup

1. **Install the server**: Follow installation instructions above
2. **Configure MCP**: Edit `~/Library/Application Support/Claude/claude_desktop_config.json`
3. **Add server entry**:

```json
{
  "mcpServers": {
    "rust-sci-hub-mcp": {
      "command": "/opt/homebrew/bin/rust-sci-hub-mcp",
      "args": [
        "--config",
        "/Users/ladvien/.config/rust-sci-hub-mcp/config.toml"
      ],
      "env": {
        "RUST_LOG": "info,rust_sci_hub_mcp=debug"
      }
    }
  }
}
```

4. **Restart Claude Desktop**

For detailed setup instructions, see [Claude Desktop Setup Guide](CLAUDE_DESKTOP_SETUP.md).

### Available Tools

The server provides these MCP tools to Claude:

1. **search_papers**: Search for academic papers
2. **download_paper**: Download papers from Sci-Hub
3. **extract_metadata**: Extract bibliographic information

### Example Claude Conversations

**User**: "Find papers about transformer neural networks"
**Claude**: *Uses search_papers tool with query "transformer neural networks"*

**User**: "Download the first paper from those results"
**Claude**: *Uses download_paper tool with the paper's DOI or URL*

**User**: "Extract the citation information from the downloaded paper"
**Claude**: *Uses extract_metadata tool on the downloaded file*

## Usage Examples

### Research Workflow Example

#### Using Claude Desktop with MCP Integration

1. **Search Phase**:
   - **User**: "Find 5 papers about quantum computing"
   - **Claude**: *Uses search_papers tool to find relevant papers*
   - **Result**: Claude presents a list of papers with titles, authors, DOIs

2. **Download Phase**:
   - **User**: "Download the first paper from that list"
   - **Claude**: *Uses download_paper tool with the DOI from search results*
   - **Result**: Paper is downloaded to configured directory

3. **Analysis Phase**:
   - **User**: "Extract the citation information from that downloaded paper"
   - **Claude**: *Uses extract_metadata tool on the downloaded file*
   - **Result**: Claude provides bibliographic information in requested format

### Batch Processing Example

#### Downloading Multiple Papers via Claude Desktop

**User**: "I have a list of DOIs I need to download: 10.1038/nature12373, 10.1126/science.1242072, 10.1073/pnas.1234567890. Can you download all of these papers for me?"

**Claude**: *Uses download_paper tool iteratively for each DOI, with appropriate delays between requests to be respectful to servers*

**Result**: All papers are downloaded to the configured directory with appropriate filenames.

## Best Practices

### 1. Respectful Usage

- **Rate Limiting**: Keep the default rate limit (1 req/sec) or lower
- **Mirror Rotation**: Let the server handle mirror selection automatically
- **Reasonable Requests**: Don't overwhelm the service with bulk requests

### 2. File Organization

```toml
[downloads]
directory = "~/Research/Papers"
organize_by_date = true  # Creates YYYY/MM subdirectories
verify_integrity = true  # Ensures complete downloads
```

### 3. Security Considerations

- **Localhost Only**: Keep `host = "127.0.0.1"` for security
- **Firewall**: Don't expose the server port to the internet
- **File Permissions**: Downloaded files are created with secure permissions

### 4. Performance Optimization

```toml
[downloads]
max_concurrent = 3        # Increase for faster downloads (but be respectful)
max_file_size_mb = 50    # Limit to avoid huge downloads

[sci_hub]
timeout_secs = 15        # Reduce for faster failures
max_retries = 2          # Fewer retries for quicker response
```

### 5. Monitoring and Maintenance

```bash
# Check service health
curl http://localhost:8080/health

# Monitor logs
tail -f ~/Library/Logs/rust-sci-hub-mcp/service.log

# Check download directory size
du -sh ~/Downloads/papers

# Clean up old downloads (optional)
find ~/Downloads/papers -name "*.pdf" -mtime +30 -delete
```

## Tips and Tricks

### 1. Working with Search Results

When Claude returns search results via MCP tools, you can ask for specific formatting:

**User**: "Search for machine learning papers and show me just the titles and DOIs"
**Claude**: *Uses search_papers tool and formats the output as requested*

### 2. Integration with Reference Managers

**User**: "Export the metadata from that downloaded paper in BibTeX format for my reference manager"
**Claude**: *Uses extract_metadata tool with format parameter set to "bibtex"*

### 3. Automated Research Workflows

You can chain multiple requests in natural language:

**User**: "Search for 'transformer neural networks', download the top 3 papers, extract their metadata in BibTeX format, and organize them by publication year"

**Claude**: *Executes the workflow using multiple MCP tool calls in sequence*

### 4. Health Monitoring

The service provides health monitoring through a simple HTTP endpoint (even though primary communication is via MCP):

```bash
#!/bin/bash
# health_monitor.sh - Monitor service health

check_health() {
    local response=$(curl -s http://localhost:8080/health)
    local status=$(echo $response | jq -r '.status')
    
    if [ "$status" = "healthy" ]; then
        echo "✓ Service is healthy"
        return 0
    else
        echo "✗ Service is unhealthy: $response"
        return 1
    fi
}

# Check health periodically
check_health
```

## Troubleshooting

For troubleshooting common issues, see [TROUBLESHOOTING.md](TROUBLESHOOTING.md).

## Support

- **Documentation**: Check [docs/](.) directory
- **Health Check**: `curl http://localhost:8080/health`
- **Logs**: `~/Library/Logs/rust-sci-hub-mcp/service.log`
- **Configuration**: `~/.config/rust-sci-hub-mcp/config.toml`
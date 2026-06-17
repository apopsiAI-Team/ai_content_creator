# Claude Desktop Integration Setup

This guide covers setting up the rust-sci-hub-mcp server with Claude Desktop to enable AI-powered academic paper research.

## Prerequisites

1. **Claude Desktop** installed on macOS
2. **rust-sci-hub-mcp** installed via Homebrew or from source
3. **Proper configuration** with Sci-Hub mirrors

## Quick Setup

### 1. Install rust-sci-hub-mcp

```bash
# Via Homebrew (recommended)
git clone https://github.com/Ladvien/sci_hub_mcp.git
cd sci_hub_mcp
brew install --build-from-source homebrew/rust-sci-hub-mcp.rb

# Verify installation
rust-sci-hub-mcp --version
```

### 2. Configure MCP Server

Ensure your configuration file at `~/.config/rust-sci-hub-mcp/config.toml` includes Sci-Hub mirrors:

```toml
[sci_hub]
mirrors = [
    "https://sci-hub.se",
    "https://sci-hub.st",
    "https://sci-hub.ru"
]

[downloads]
directory = "~/Downloads/papers"

[logging]
level = "info"
file = "~/Library/Logs/rust-sci-hub-mcp/service.log"
```

### 3. Add to Claude Desktop Configuration

Edit your Claude Desktop MCP configuration file:

**File location**: `~/Library/Application Support/Claude/claude_desktop_config.json`

**Configuration**:
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

**If you have existing MCP servers**, add rust-sci-hub-mcp as an additional entry:

```json
{
  "mcpServers": {
    "existing-server": {
      "command": "/path/to/existing/server",
      "args": []
    },
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

### 4. Restart Claude Desktop

Close and reopen Claude Desktop to load the new MCP server configuration.

## Available Tools

Once configured, Claude Desktop will have access to these tools:

### 1. search_papers
Search for academic papers by DOI, title, or author name.

**Example usage**:
- "Search for papers about quantum computing by David Deutsch"
- "Find the paper with DOI 10.1038/nature12373"
- "Look for recent machine learning papers from 2023"

### 2. download_paper
Download papers from Sci-Hub with progress tracking.

**Example usage**:
- "Download the paper with DOI 10.1126/science.1234567"
- "Download that first search result"
- "Get the PDF for the quantum computing paper we found"

### 3. extract_metadata
Extract bibliographic information from downloaded papers.

**Example usage**:
- "Extract citation information from the downloaded paper"
- "Get BibTeX format for the paper I just downloaded"
- "Extract metadata from all papers in my downloads folder"

## Usage Examples

### Basic Research Workflow

1. **Search for papers**:
   ```
   User: "Find 5 recent papers about CRISPR gene editing"
   Claude: [Uses search_papers tool to find relevant papers]
   ```

2. **Download interesting papers**:
   ```
   User: "Download the first two papers from those results"
   Claude: [Uses download_paper tool for each paper]
   ```

3. **Get citation information**:
   ```
   User: "Extract BibTeX citations for those downloaded papers"
   Claude: [Uses extract_metadata tool to generate citations]
   ```

### Advanced Research Tasks

**Literature Review**:
```
User: "Help me research transformer neural networks. Find 10 key papers, download them, and create a bibliography."
Claude: [Searches papers, downloads top results, extracts metadata, formats bibliography]
```

**DOI-based Download**:
```
User: "I have these DOIs: 10.1038/nature12373, 10.1126/science.1234567. Download both papers and extract their abstracts."
Claude: [Downloads each paper by DOI, extracts metadata including abstracts]
```

## Configuration Details

### Environment Variables

- `RUST_LOG`: Controls logging verbosity
  - `info`: Standard logging
  - `debug`: Detailed debugging (recommended for troubleshooting)
  - `trace`: Maximum verbosity

### Command Line Arguments

- `--config`: Path to configuration file
- `--verbose`: Enable verbose logging
- `--daemon`: Run as background daemon (not needed for MCP integration)

### File Paths

Make sure these paths are correct for your system:

- **Binary**: `/opt/homebrew/bin/rust-sci-hub-mcp` (Homebrew install)
- **Config**: `/Users/[username]/.config/rust-sci-hub-mcp/config.toml`
- **Logs**: `/Users/[username]/Library/Logs/rust-sci-hub-mcp/`
- **Downloads**: Configurable in `config.toml` (default: `~/Downloads/papers`)

## Troubleshooting

### Claude Desktop Not Showing Tools

1. **Check configuration syntax**:
   ```bash
   python3 -m json.tool ~/Library/Application\ Support/Claude/claude_desktop_config.json
   ```

2. **Verify binary path**:
   ```bash
   which rust-sci-hub-mcp
   ls -la /opt/homebrew/bin/rust-sci-hub-mcp
   ```

3. **Test configuration**:
   ```bash
   rust-sci-hub-mcp --config ~/.config/rust-sci-hub-mcp/config.toml --help
   ```

4. **Check logs**:
   ```bash
   tail -f ~/Library/Logs/rust-sci-hub-mcp/service.log
   ```

### Common Issues

**"Sci-Hub mirrors required" error**:
- Ensure `[sci_hub].mirrors` array is not empty in config.toml

**"Permission denied" errors**:
- Check file permissions: `chmod 600 ~/.config/rust-sci-hub-mcp/config.toml`

**"Binary not found" errors**:
- Reinstall: `brew reinstall rust-sci-hub-mcp`
- Check path: `which rust-sci-hub-mcp`

### Debug Mode

For troubleshooting, temporarily increase logging:

```json
{
  "mcpServers": {
    "rust-sci-hub-mcp": {
      "command": "/opt/homebrew/bin/rust-sci-hub-mcp",
      "args": [
        "--config",
        "/Users/ladvien/.config/rust-sci-hub-mcp/config.toml",
        "--verbose"
      ],
      "env": {
        "RUST_LOG": "debug"
      }
    }
  }
}
```

## Security Considerations

- **Configuration files**: Stored with secure permissions (600)
- **Download directory**: Papers saved to user-specified location
- **Network access**: Only connects to configured Sci-Hub mirrors
- **Rate limiting**: Respects configured request limits
- **Personal use**: Intended for individual research purposes

## Integration with Claude Code

This MCP server also works with Claude Code (CLI). The same configuration principles apply - just ensure the binary is accessible in your PATH.

## Support

- **Documentation**: See `docs/` directory in repository
- **Logs**: Check `~/Library/Logs/rust-sci-hub-mcp/service.log`
- **Issues**: Report at https://github.com/Ladvien/sci_hub_mcp/issues
- **Configuration**: Validate with `rust-sci-hub-mcp --help`

## Disclaimer

This tool is designed for personal research use. Users are responsible for ensuring their usage complies with applicable laws and institutional policies regarding academic paper access.
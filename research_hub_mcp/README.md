# rust-research-mcp

A Model Context Protocol (MCP) server for academic research and knowledge accumulation through intelligent paper search, retrieval, and metadata extraction.

[![License: GPL-3.0](https://img.shields.io/badge/License-GPL%20v3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![MCP Compatible](https://img.shields.io/badge/MCP-Compatible-green)](https://modelcontextprotocol.io)
[![MSRV](https://img.shields.io/badge/MSRV-1.70.0-blue)](https://github.com/rust-lang/rust/releases/tag/1.70.0)

## ⚠️ Legal Disclaimer

**IMPORTANT: This tool is intended for personal academic use only.**

This software is provided for educational and research purposes. Users are responsible for ensuring their use complies with:
- All applicable laws and regulations
- Publisher terms of service
- Institutional policies
- Copyright restrictions

The developers of this tool do not condone or support any illegal activities. Users should:
- Only access papers they have legal rights to access
- Respect intellectual property rights
- Use retrieved materials in accordance with fair use principles
- Consider supporting authors and publishers through legitimate channels

**By using this software, you acknowledge that you understand and will comply with all applicable laws and regulations regarding access to academic content.**

## Features

- 🔍 **Multi-Provider Search**: Comprehensive search across 14 academic sources:
  - **CrossRef** - Authoritative metadata for 130M+ papers
  - **Semantic Scholar** - AI-powered search with PDF access
  - **arXiv** - Physics, CS, and math preprints
  - **PubMed Central** - Biomedical and life science papers
  - **OpenReview** - ML conference papers (NeurIPS, ICLR, etc.)
  - **OpenAlex** - Open bibliographic database
  - **CORE** - 350M+ open access papers
  - **Unpaywall** - Legal free PDF discovery
  - **SSRN** - Social science working papers
  - **bioRxiv** - Biology preprints
  - **MDPI** - Open access journals
  - **ResearchGate** - Academic social network (ethical access)
  - **Sci-Hub** - Full-text fallback (lowest priority)

- 🧠 **Intelligent Routing**: Smart provider prioritization based on:
  - Academic domain detection (CS/ML, biomedical, physics, social sciences)
  - Search type optimization (DOI, author, title, keywords)
  - Content availability (PDF access, recent papers, open access)
  - Temporal relevance (recent vs. historical content)

- 📥 **Robust Downloads**: Multi-provider fallback with zero-byte protection and integrity verification
- 🔍 **Code Pattern Search**: Regex-powered search for algorithm implementations in research papers
- 📊 **Metadata Extraction**: Extract bibliographic information from PDFs with batch processing
- 📚 **Bibliography Generation**: Multi-format citations (BibTeX, APA, MLA, Chicago, IEEE, Harvard)
- 🏷️ **Smart Categorization**: Automatic paper categorization and organization
- 🤖 **MCP Integration**: Native support for Claude Desktop and Claude Code workflows
- ⚡ **High Performance**: Built with Rust for speed and reliability
- 🔄 **Resilient Architecture**: Circuit breakers, rate limiting, automatic retries, and graceful error handling
- 🛡️ **Security First**: HTTPS-only connections, certificate validation, and secure HTTP client factory
- 🔧 **Daemon Mode**: Background service with health monitoring and signal handling

## Installation

### Quick Start (Recommended)

**Build from source:**

```bash
# Prerequisites: Rust 1.70+ (install from https://rustup.rs/)
git clone https://github.com/Ladvien/sci_hub_mcp.git
cd sci_hub_mcp
cargo build --release

# Binary will be at ./target/release/rust-research-mcp
# Move to a permanent location
sudo cp target/release/rust-research-mcp /usr/local/bin/
```

### Alternative Installation Methods

**Using Cargo:**
```bash
cargo install rust-research-mcp
```

**Development Build:**
```bash
git clone https://github.com/Ladvien/sci_hub_mcp.git
cd sci_hub_mcp
cargo build --release
```

### Configuration for Claude Desktop

Add the following to your Claude Desktop configuration file:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
**Linux**: `~/.config/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "rust-research-mcp": {
      "command": "/usr/local/bin/rust-research-mcp",
      "args": [
        "--download-dir", "~/downloads/research_papers",
        "--log-level", "info"
      ],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

### Daemon Mode

For production deployments, you can run the server as a daemon:

```bash
# Start daemon with custom configuration
rust-research-mcp --daemon --pid-file /var/run/rust-research-mcp.pid --health-port 8090

# Check daemon status
curl http://localhost:8090/health

# Stop daemon (sends SIGTERM for graceful shutdown)
kill -TERM $(cat /var/run/rust-research-mcp.pid)
```

## Usage

Once configured, you can ask Claude to:

- **Search for papers**: "Search for recent papers on quantum computing"
- **Download papers**: "Download the first paper from the search results"
- **Extract metadata**: "Extract metadata from the PDF file"

### Command Line Options

```bash
rust-research-mcp [OPTIONS]

Options:
  -v, --verbose                    Enable verbose logging
  -c, --config <PATH>             Configuration file path
  -d, --daemon                    Run as daemon
      --pid-file <PATH>           PID file path (for daemon mode)
      --health-port <PORT>        Health check port [default: 8090]
      --port <PORT>               Override server port
      --host <HOST>               Override server host
      --log-level <LEVEL>         Override log level (trace, debug, info, warn, error)
      --profile <PROFILE>         Set environment profile (development, production)
      --download-dir <PATH>       Override download directory path
      --generate-schema           Generate JSON schema for configuration
  -h, --help                      Print help information
  -V, --version                   Print version information
```

### Environment Variables

- `RUST_RESEARCH_MCP_*`: Configuration variables (see config.toml for full list)
- `RUST_LOG`: Standard Rust logging configuration (debug, info, warn, error, trace)

## Available Tools

### Core Research Tools

#### search_papers
Search for academic papers across 14 different academic sources with intelligent provider routing.

**Parameters:**
- `query` (required): Search query (DOI, title, author, or keywords)
- `search_type` (optional): Search type (`auto`, `doi`, `title`, `author`, `author_year`)
- `limit` (optional): Maximum results to return (default: 10)
- `offset` (optional): Pagination offset (default: 0)

#### download_paper
Download a paper PDF with multi-provider fallback and integrity verification.

**Parameters:**
- `doi` (optional): DOI of the paper to download
- `url` (optional): Direct download URL (alternative to DOI)
- `filename` (optional): Custom filename for the downloaded PDF
- `directory` (optional): Target directory (uses default download directory if not specified)
- `category` (optional): Organization category (creates subdirectory)
- `overwrite` (optional): Whether to overwrite existing files (default: false)
- `verify_integrity` (optional): Verify file integrity after download (default: true)

#### extract_metadata
Extract bibliographic metadata from PDF files using multiple extraction methods.

**Parameters:**
- `file_path` (required): Path to the PDF file
- `extract_full_text` (optional): Also extract full text content (default: false)
- `extract_references` (optional): Extract reference list (default: false)

### Advanced Tools

#### search_code
Search for code patterns within downloaded research papers using regex patterns.

**Parameters:**
- `pattern` (required): Regex pattern to search for
- `search_dir` (optional): Directory to search in (defaults to download directory)
- `file_extensions` (optional): File extensions to search (default: [".pdf", ".txt"])
- `max_results` (optional): Maximum results to return (default: 50)
- `context_lines` (optional): Lines of context around matches (default: 2)

#### generate_bibliography
Generate formatted citations from paper metadata in multiple citation styles.

**Parameters:**
- `papers` (required): Array of paper metadata or DOIs
- `format` (optional): Citation format (`bibtex`, `apa`, `mla`, `chicago`, `ieee`) (default: bibtex)
- `sort_by` (optional): Sort order (`author`, `year`, `title`) (default: author)
- `include_abstracts` (optional): Include abstracts in output (default: false)

#### categorize_papers
Automatically categorize research papers based on content and metadata.

**Parameters:**
- `papers` (required): Array of paper metadata or file paths
- `category_scheme` (optional): Categorization scheme (`subject`, `methodology`, `custom`)
- `custom_categories` (optional): Custom category definitions
- `confidence_threshold` (optional): Minimum confidence for categorization (default: 0.7)

## Example Workflows

### Research Collection Workflow
```bash
# Step 1: Search for papers on a topic
"Search for recent papers on transformer architectures, limit 20"

# Step 2: Download selected papers
"Download the paper with DOI 10.1038/nature12373 to ~/research/transformers/"

# Step 3: Extract metadata for organization
"Extract metadata from ~/research/transformers/paper.pdf"

# Step 4: Search for code implementations
"Search for 'class Transformer' pattern in ~/research/transformers/"

# Step 5: Generate bibliography
"Create BibTeX bibliography from collected papers"
```

### Literature Review Workflow
```bash
# Search across multiple aspects of a topic
"Search for papers by author 'Yoshua Bengio' on deep learning"
"Search for papers on attention mechanisms in neural networks"

# Organize papers by category
"Categorize papers in ~/research/attention/ by methodology"

# Generate comprehensive bibliography
"Generate IEEE format bibliography from all categorized papers"
```

## Claude Code Integration

This MCP server is specifically enhanced for **Claude Code** workflows with advanced research capabilities:

### Key Benefits for Developers
- **Algorithm Discovery**: Find reference implementations in academic papers
- **Code Pattern Search**: Regex-powered search across research publications
- **Citation Management**: Generate properly formatted references for projects
- **Research Organization**: Automatic categorization and metadata extraction

### Integration Tips
1. **Configure download directory**: Set up dedicated research workspace
2. **Use search patterns**: Leverage regex for finding specific implementations
3. **Organize by categories**: Use automatic categorization for better organization
4. **Generate documentation**: Create proper citations and bibliographies

## Configuration File

Create a configuration file at `~/.config/knowledge_accumulator_mcp/config.toml`:

```toml
# Server configuration
[server]
port = 8080
host = "127.0.0.1"
graceful_shutdown_timeout_secs = 30

# Research source configuration
[research_source]
provider_timeout_secs = 30
max_results_per_provider = 50

# Download settings
[downloads]
directory = "~/downloads/research_papers"
max_concurrent_downloads = 5
max_file_size_mb = 100
verify_integrity = true

# Logging configuration
[logging]
level = "info"
format = "pretty"
output = "stderr"

# Resilience settings
[circuit_breaker]
failure_threshold = 5
timeout_duration_secs = 60
half_open_max_calls = 3

[rate_limiting]
requests_per_second = 2
burst_size = 10
```

## Development

### Running Tests

```bash
# Run all tests (parallel execution)
cargo nextest run

# Run specific test
cargo nextest run TEST_NAME

# Run with coverage report
cargo tarpaulin --out Html

# Run integration tests
cargo test --test comprehensive_e2e_scenarios
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter (must pass before commit)
cargo clippy -- -D warnings

# Security audit
cargo audit

# Build release version
cargo build --release
```

## Architecture

The project follows a clean, modular architecture with dependency injection:

```
src/
├── main.rs          # CLI entry point and configuration
├── lib.rs           # Public API and exports
├── server/          # MCP server implementation
│   ├── handler.rs   # MCP request handler
│   └── transport.rs # Transport layer validation
├── tools/           # MCP tool implementations
│   ├── search.rs    # Multi-provider search
│   ├── download.rs  # Paper download with fallback
│   ├── metadata.rs  # PDF metadata extraction
│   ├── code_search.rs # Code pattern search
│   ├── bibliography.rs # Citation generation
│   └── categorize.rs # Paper categorization
├── client/          # Research source integration
│   ├── meta_search.rs # Meta-search orchestration
│   ├── mirror.rs    # Mirror management
│   ├── rate_limiter.rs # Rate limiting
│   └── providers/   # Academic source implementations
│       ├── arxiv.rs
│       ├── crossref.rs
│       ├── semantic_scholar.rs
│       ├── pubmed_central.rs
│       ├── openreview.rs
│       ├── openalex.rs
│       └── ... (14 providers total)
├── resilience/      # Circuit breakers and retry logic
├── services/        # Business logic services
├── config/          # Configuration management
└── error.rs         # Centralized error handling
```

## Changelog

### Version 0.6.6 (Current)
- **🏗️ Complete Architecture Redesign**: Clean hexagonal architecture with dependency injection
- **🚀 14 Academic Providers**: Comprehensive coverage including OpenAlex, PubMed Central, OpenReview
- **🧠 Intelligent Provider Routing**: Context-aware selection based on domain, search type, and content availability
- **🔧 Enhanced MCP Integration**: Full rmcp framework integration with proper tool definitions
- **🛡️ Security Hardened**: HTTPS-only clients, certificate validation, secure HTTP factory
- **🔄 Resilience Features**: Circuit breakers, rate limiting, automatic retries, graceful degradation
- **🏷️ Smart Categorization**: Automatic paper categorization and organization
- **📊 Advanced Metadata Extraction**: Multiple extraction methods with batch processing
- **🔍 Code Pattern Search**: Regex-powered search across research publications
- **📚 Multi-format Citations**: BibTeX, APA, MLA, Chicago, IEEE format support
- **🔧 Daemon Mode**: Production-ready background service with health monitoring
- **📝 Comprehensive Testing**: E2E scenarios, integration tests, security auditing

## Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Troubleshooting

### Common Issues

**Issue**: Papers not downloading
- **Solution**: The tool uses 14 different providers with intelligent fallback. Check network connectivity and try alternative search terms.

**Issue**: MCP server not connecting
- **Solution**: Verify the binary path in `claude_desktop_config.json` is absolute and the binary has execute permissions (`chmod +x`).

**Issue**: High memory usage
- **Solution**: Configure appropriate concurrency limits in `config.toml`. Lower `max_concurrent_downloads` for systems with limited resources.

**Issue**: Provider timeout errors
- **Solution**: Increase `provider_timeout_secs` in configuration or check internet connectivity to academic databases.

**Issue**: Circuit breaker errors
- **Solution**: The system uses circuit breakers for resilience. Wait for the timeout period or check provider availability.

### Logs

**Daemon Mode Logs:**
- View with: `journalctl -u rust-research-mcp` (systemd)
- Or check: `/var/log/rust-research-mcp.log`

**Claude Desktop Logs:**
- **macOS**: `~/Library/Logs/Claude/mcp-server-rust-research-mcp.log`
- **Linux**: `~/.local/share/Claude/logs/`
- **Windows**: `%APPDATA%\Claude\logs\`

**Debug Mode:**
```bash
# Enable debug logging
RUST_LOG=debug rust-research-mcp --verbose
```

## License

This project is licensed under the GPL-3.0 License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [rmcp](https://github.com/anthropics/rmcp) - Rust SDK for Model Context Protocol
- Uses the [Model Context Protocol](https://modelcontextprotocol.io) specification
- Searches academic databases including [arXiv](https://arxiv.org) and [CrossRef](https://www.crossref.org)

## Disclaimer

This tool is provided "as is" without warranty of any kind. The authors and contributors are not responsible for any misuse or legal issues arising from the use of this software. Users must ensure they comply with all applicable laws, regulations, and terms of service when accessing academic content.

**For personal academic use only.**

## Support

For issues, questions, or suggestions, please [open an issue](https://github.com/Ladvien/sci_hub_mcp/issues) on GitHub.

---

Made with ❤️ for the academic community. Please use responsibly.
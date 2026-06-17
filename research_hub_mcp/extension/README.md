# Rust Research MCP Extension

A powerful Claude Desktop extension for academic research, providing seamless access to search, download, and manage research papers from multiple databases.

## 🚀 Features

- **Multi-Database Search**: Search across arXiv, PubMed Central, bioRxiv, CrossRef, Semantic Scholar, OpenReview, and MDPI
- **Smart Downloads**: Download papers by DOI or URL with automatic file organization
- **AI Categorization**: Automatically categorize papers using LLM-powered analysis
- **Metadata Extraction**: Extract detailed metadata from PDF files
- **Bibliography Generation**: Generate citations in APA, MLA, Chicago, and BibTeX formats
- **File Organization**: Organize papers by category, date, or keep them flat

## 📋 Prerequisites

- **Rust**: This extension requires Rust to be installed on your system
- **Claude Desktop**: Latest version of Claude Desktop application

## 🛠️ Installation

### Option 1: Automatic Installation (Recommended)
1. Download the `rust-research-mcp.dxt` file
2. Double-click the file or drag it into Claude Desktop
3. Follow the installation prompts

### Option 2: Manual Installation
```bash
# Install the MCP server
cargo install rust-research-mcp

# Create download directory
mkdir -p ~/Documents/Research-Papers
```

## 🔧 Configuration

The extension creates a configuration file at `~/.config/rust-research-mcp/config.toml` with the following default settings:

```toml
[downloads]
directory = "~/Documents/Research-Papers"
file_organization = "categorized"
concurrent_downloads = 3

[categorization]
enabled = true
default_category = "research_papers"

[research_source]
endpoints = [
    "https://sci-hub.se",
    "https://sci-hub.st", 
    "https://sci-hub.ru"
]
```

## 📚 Usage Examples

### Search for Papers
```
Search for papers about "quantum computing machine learning"
```

### Download a Paper
```
Download the paper with DOI: 10.1038/nature12373
```

### Extract Metadata
```
Extract metadata from the PDF file at ~/Downloads/paper.pdf
```

### Generate Bibliography
```
Generate a BibTeX bibliography for these DOIs: 10.1038/nature12373, 10.1126/science.1234567
```

### Categorize Papers
```
Categorize my downloaded papers in ~/Documents/Research-Papers
```

## 🎯 Available Tools

| Tool | Description |
|------|-------------|
| `search_papers` | Search academic databases by keywords, DOI, title, or author |
| `download_paper` | Download papers with automatic organization |
| `extract_metadata` | Extract detailed metadata from PDF files |
| `generate_bibliography` | Create formatted citations |
| `categorize_papers` | AI-powered paper categorization |

## 📁 File Organization

Papers can be organized in three ways:

1. **Categorized** (default): Papers sorted into topic-based folders
   ```
   ~/Documents/Research-Papers/
   ├── machine_learning/
   ├── quantum_physics/
   └── computer_science/
   ```

2. **By Date**: Papers organized by download date
   ```
   ~/Documents/Research-Papers/
   ├── 2024/01/
   ├── 2024/02/
   └── 2024/03/
   ```

3. **Flat**: All papers in the main directory

## 🔒 Privacy & Ethics

- **Local Processing**: All metadata extraction and categorization happens locally
- **Respectful Access**: Built-in rate limiting and circuit breakers
- **Personal Use**: Designed for personal research and academic use
- **No Data Collection**: No user data is collected or transmitted

## 🛠️ Troubleshooting

### Extension Not Loading
1. Ensure Rust is installed: `rustc --version`
2. Verify the MCP server is installed: `rust-research-mcp --version`
3. Check Claude Desktop logs for errors

### Download Issues
1. Verify the download directory exists and is writable
2. Check network connectivity
3. Ensure the DOI/URL is valid

### Permission Errors
1. Check folder permissions for the download directory
2. Grant Claude Desktop folder access in System Preferences (macOS)

## 🆘 Support

- **Issues**: Report bugs at [GitHub Issues](https://github.com/Ladvien/research_hub_mcp/issues)
- **Documentation**: [Full Documentation](https://github.com/Ladvien/research_hub_mcp)
- **License**: GPL-3.0

## 🏷️ Version

Current version: 0.6.2

Built with ❤️ for the research community.
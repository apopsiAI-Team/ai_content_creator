#!/bin/bash

# Rust Research MCP Installation Script
# Version: 0.6.6

set -e

echo "🔬 Installing Rust Research MCP Server..."
echo "========================================"

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo not found. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "✅ Rust found: $(rustc --version)"

# Check minimum Rust version
RUST_VERSION=$(rustc --version | cut -d' ' -f2)
MINIMUM_VERSION="1.70.0"

if ! printf '%s\n%s\n' "$MINIMUM_VERSION" "$RUST_VERSION" | sort -V -C; then
    echo "❌ Rust version $RUST_VERSION is too old. Minimum required: $MINIMUM_VERSION"
    echo "   Run: rustup update"
    exit 1
fi

echo "✅ Rust version check passed"

# Create download directory
DOWNLOAD_DIR="$HOME/Documents/Research-Papers"
echo "📁 Creating download directory: $DOWNLOAD_DIR"
mkdir -p "$DOWNLOAD_DIR"

# Create cache directory
CACHE_DIR="$HOME/.cache/rust-research-mcp"
echo "📁 Creating cache directory: $CACHE_DIR"
mkdir -p "$CACHE_DIR"

# Install the MCP server
echo "🛠️  Installing rust-research-mcp..."
if [ -d ".git" ]; then
    # Installing from local directory
    echo "📦 Installing from local source..."
    cargo install --path . --force
else
    # Installing from GitHub
    echo "📦 Installing from GitHub repository..."
    cargo install --git https://github.com/Ladvien/research_hub_mcp.git --force
fi

# Verify installation
if command -v rust-research-mcp &> /dev/null; then
    echo "✅ Installation successful!"
    echo "📍 Binary location: $(which rust-research-mcp)"
else
    echo "❌ Installation failed - binary not found in PATH"
    exit 1
fi

# Check Claude Desktop config
CLAUDE_CONFIG="$HOME/Library/Application Support/Claude/claude_desktop_config.json"
echo ""
echo "🖥️  Claude Desktop Configuration"
echo "================================"

if [ -f "$CLAUDE_CONFIG" ]; then
    echo "✅ Claude Desktop config found: $CLAUDE_CONFIG"

    # Check if already configured
    if grep -q "rust-research-mcp" "$CLAUDE_CONFIG"; then
        echo "✅ rust-research-mcp already configured in Claude Desktop"
    else
        echo "⚠️  rust-research-mcp not found in Claude Desktop config"
        echo ""
        echo "📋 Add this to your Claude Desktop configuration:"
        echo ""
        cat << EOF
{
  "mcpServers": {
    "rust-research-mcp": {
      "command": "$(which rust-research-mcp)",
      "args": [],
      "env": {
        "RUST_LOG": "info",
        "RSH_DOWNLOAD_DIRECTORY": "$DOWNLOAD_DIR",
        "RSH_CACHE_DIRECTORY": "$CACHE_DIR"
      }
    }
  }
}
EOF
    fi
else
    echo "⚠️  Claude Desktop config not found at: $CLAUDE_CONFIG"
    echo "   Please ensure Claude Desktop is installed"
fi

echo ""
echo "🎉 Installation Complete!"
echo "========================"
echo ""
echo "📁 Download directory: $DOWNLOAD_DIR"
echo "📁 Cache directory: $CACHE_DIR"
echo "🔧 Binary: $(which rust-research-mcp)"
echo ""
echo "🚀 Next steps:"
echo "1. Add the MCP server to your Claude Desktop configuration (see above)"
echo "2. Restart Claude Desktop"
echo "3. Try: 'Search for papers about machine learning'"
echo "4. Try: 'Download paper with DOI 10.1038/nature12373'"
echo ""
echo "📖 For more information: https://github.com/Ladvien/research_hub_mcp"
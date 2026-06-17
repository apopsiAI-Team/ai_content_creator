#!/bin/bash
# Script to update PATH for rust-research-mcp v0.4.2

echo "🔧 Updating PATH to use latest rust-research-mcp..."

# Add cargo bin to PATH if not already there
if [[ ":$PATH:" != *":$HOME/.cargo/bin:"* ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
    echo "✅ Added ~/.cargo/bin to PATH"
else
    echo "✅ ~/.cargo/bin already in PATH"
fi

# Verify version
echo "📋 Current version:"
rust-research-mcp --version

echo ""
echo "🚀 Ready to use! Available tools:"
echo "  • search_papers - Search academic papers across 12+ sources"
echo "  • download_paper - Download papers with fallback protection"
echo "  • extract_metadata - Extract bibliographic info from PDFs"
echo "  • search_code - Find code patterns in downloaded papers"
echo "  • generate_bibliography - Create citations (BibTeX, APA, MLA, etc.)"
echo ""
echo "💡 To make this permanent, add this to your ~/.zshrc or ~/.bashrc:"
echo '   export PATH="$HOME/.cargo/bin:$PATH"'
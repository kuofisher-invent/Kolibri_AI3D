#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────
#  CAD MCP Server — Setup & Build Script
#  Run this on your local machine
# ─────────────────────────────────────────────────────────────

set -e

echo "🦀 CAD MCP Server Setup"
echo "========================"

# 1. Install Rust if needed
if ! command -v cargo &> /dev/null; then
    echo "📦 Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

echo "✅ Rust $(rustc --version)"

# 2. Build
echo ""
echo "🔨 Building cad-mcp-server..."
cargo build --release

BINARY="$(pwd)/target/release/cad-mcp-server"
echo "✅ Binary: $BINARY"

# 3. Generate Claude Desktop config snippet
echo ""
echo "📋 Add this to your claude_desktop_config.json:"
echo ""
cat << CONFIG
{
  "mcpServers": {
    "cad-3d": {
      "command": "$BINARY",
      "env": {
        "RUST_LOG": "cad_mcp_server=info"
      }
    }
  }
}
CONFIG

echo ""
echo "📁 Config file locations:"
echo "  macOS:   ~/Library/Application Support/Claude/claude_desktop_config.json"
echo "  Windows: %APPDATA%\\Claude\\claude_desktop_config.json"
echo "  Linux:   ~/.config/Claude/claude_desktop_config.json"
echo ""
echo "🔄 Restart Claude Desktop after updating the config"
echo ""
echo "🎉 Done! Open Claude Desktop and say:"
echo "   「幫我建一個 5x4 公尺的客廳」"

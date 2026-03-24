mod scene;
mod mcp;

use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};
use tracing::info;

use scene::CadScene;
use mcp::{McpServer, JsonRpcRequest};

fn main() -> anyhow::Result<()> {
    // Log to stderr (stdout is reserved for MCP JSON-RPC)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "cad_mcp_server=info".into())
        )
        .init();

    info!("🚀 CAD MCP Server starting (stdio mode)");

    let scene  = Arc::new(Mutex::new(CadScene::new()));
    let server = McpServer::new(scene);

    let stdin  = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    // ── MCP stdio loop ────────────────────────────────────────────────────────
    // Protocol: each message is a JSON object on one line (newline-delimited)
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l)  => l,
            Err(e) => { tracing::error!("stdin read error: {}", e); break; }
        };

        let line = line.trim();
        if line.is_empty() { continue; }

        tracing::debug!("← {}", line);

        let response = match serde_json::from_str::<JsonRpcRequest>(line) {
            Ok(req) => server.handle(req),
            Err(e)  => {
                tracing::warn!("Parse error: {} | input: {}", e, line);
                mcp::JsonRpcResponse::err(
                    None, -32700,
                    format!("Parse error: {}", e)
                )
            }
        };

        let response_str = serde_json::to_string(&response)?;
        tracing::debug!("→ {}", response_str);

        writeln!(out, "{}", response_str)?;
        out.flush()?;
    }

    info!("CAD MCP Server shutting down");
    Ok(())
}

//! Kolibri MCP Server — 獨立執行檔
//!
//! 用法:
//!   kolibri-mcp-server              → stdio 模式（Claude Desktop）
//!   kolibri-mcp-server --http       → HTTP/SSE 模式（ChatGPT，預設 port 3001）
//!   kolibri-mcp-server --http 8080  → HTTP/SSE 模式（自訂 port）

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--http") {
        // HTTP/SSE mode
        let port = args.iter()
            .position(|a| a == "--http")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(3001);
        kolibri_mcp::transport_http::run_http(port).await;
    } else {
        // stdio mode (default)
        kolibri_mcp::transport_stdio::run_stdio();
    }
}

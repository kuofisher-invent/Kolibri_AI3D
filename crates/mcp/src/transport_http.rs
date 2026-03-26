//! Layer 3b: HTTP/SSE transport（ChatGPT MCP 相容）
//!
//! Endpoints:
//!   POST /mcp    — JSON-RPC 2.0 request → response（stateless）
//!   GET  /sse    — Server-Sent Events stream（streaming session）
//!   GET  /health — 健康檢查

use axum::{
    extract::State,
    http::StatusCode,
    response::{sse, Sse},
    routing::{get, post},
    Json, Router,
};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use crate::protocol::*;
use crate::adapter::KolibriAdapter;

type SharedAdapter = Arc<Mutex<KolibriAdapter>>;

/// 啟動 HTTP MCP server
pub async fn run_http(port: u16) {
    let adapter = Arc::new(Mutex::new(KolibriAdapter::new()));
    let (sse_tx, _) = broadcast::channel::<String>(100);
    let sse_tx = Arc::new(sse_tx);

    let app = Router::new()
        .route("/", get(handle_dashboard))
        .route("/mcp", post(handle_mcp_post))
        .route("/sse", get(handle_sse))
        .route("/health", get(handle_health))
        .layer(
            tower_http::cors::CorsLayer::permissive()
        )
        .with_state((adapter, sse_tx));

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("MCP HTTP server listening on {}", addr);
    eprintln!("[kolibri-mcp] HTTP server on http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(&addr).await
        .expect("無法綁定 HTTP port");
    axum::serve(listener, app).await
        .expect("HTTP server 錯誤");
}

/// POST /mcp — 標準 JSON-RPC 2.0 request
async fn handle_mcp_post(
    State((adapter, sse_tx)): State<(SharedAdapter, Arc<broadcast::Sender<String>>)>,
    Json(req): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    let id = req.id.clone();
    let response = match req.method.as_str() {
        "initialize" => initialize_response(id),
        "notifications/initialized" => JsonRpcResponse::ok(id, serde_json::json!({})),
        "tools/list" => {
            let adapter = adapter.lock().unwrap();
            let tools = adapter.tool_definitions();
            JsonRpcResponse::ok(id, serde_json::json!({ "tools": tools }))
        }
        "tools/call" => {
            let params = req.params.unwrap_or(serde_json::json!({}));
            let tool_name = params["name"].as_str().unwrap_or("").to_string();
            let args = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

            if tool_name == "shutdown" {
                // 延遲關閉
                tokio::spawn(async {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    std::process::exit(0);
                });
                tool_result(id, serde_json::json!({"message":"Shutting down..."}))
            } else {
                let result = {
                    let mut adapter = adapter.lock().unwrap();
                    adapter.execute_tool(&tool_name, &args)
                };
                // 推送 SSE 事件
                let event = serde_json::json!({
                    "tool": tool_name,
                    "result": result,
                }).to_string();
                let _ = sse_tx.send(event);
                tool_result(id, result)
            }
        }
        "resources/list" => JsonRpcResponse::ok(id, serde_json::json!({"resources": []})),
        "prompts/list" => JsonRpcResponse::ok(id, serde_json::json!({"prompts": []})),
        other => JsonRpcResponse::err(id, -32601, &format!("Method not found: {}", other)),
    };
    Json(response)
}

/// GET /sse — Server-Sent Events stream
async fn handle_sse(
    State((_, sse_tx)): State<(SharedAdapter, Arc<broadcast::Sender<String>>)>,
) -> Sse<impl futures_core::Stream<Item = Result<sse::Event, std::convert::Infallible>>> {
    let mut rx = sse_tx.subscribe();
    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(data) => yield Ok(sse::Event::default().data(data)),
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    };
    Sse::new(stream)
}

/// GET / — Dashboard UI
async fn handle_dashboard() -> axum::response::Html<&'static str> {
    axum::response::Html(crate::dashboard::DASHBOARD_HTML)
}

/// GET /health
async fn handle_health(
    State((adapter, _)): State<(SharedAdapter, Arc<broadcast::Sender<String>>)>,
) -> Json<serde_json::Value> {
    let count = adapter.lock().unwrap().scene.objects.len();
    Json(serde_json::json!({
        "status": "ok",
        "server": SERVER_NAME,
        "version": SERVER_VERSION,
        "object_count": count,
    }))
}

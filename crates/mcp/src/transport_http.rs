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
        .route("/scene_svg", get(handle_scene_svg))
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
        "resources/list" => JsonRpcResponse::ok(id, serde_json::json!({"resources": [
            {"uri": "kolibri://scene", "name": "Current Scene", "mimeType": "application/json"}
        ]})),
        "resources/read" => {
            let uri = req.params.as_ref()
                .and_then(|p| p["uri"].as_str())
                .unwrap_or("");
            if uri == "kolibri://scene" {
                let state = { adapter.lock().unwrap().execute_tool("get_scene_state", &serde_json::json!({})) };
                JsonRpcResponse::ok(id, serde_json::json!({
                    "contents": [{"uri": "kolibri://scene", "mimeType": "application/json",
                                  "text": serde_json::to_string_pretty(&state).unwrap_or_default()}]
                }))
            } else {
                JsonRpcResponse::err(id, -32602, &format!("Unknown resource: {}", uri))
            }
        }
        "prompts/list" => JsonRpcResponse::ok(id, serde_json::json!({"prompts": crate::adapter::prompt_templates()})),
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

/// GET /scene_svg — 場景 SVG 預覽（isometric wireframe）
async fn handle_scene_svg(
    State((adapter, _)): State<(SharedAdapter, Arc<broadcast::Sender<String>>)>,
) -> axum::response::Html<String> {
    let adapter = adapter.lock().unwrap();
    let mut lines = Vec::new();
    let w = 400.0_f32;
    let h = 300.0_f32;

    // Simple isometric projection
    let project = |x: f32, y: f32, z: f32| -> (f32, f32) {
        let scale = 0.05;
        let px = w / 2.0 + (x - z) * 0.866 * scale;
        let py = h / 2.0 - y * scale + (x + z) * 0.5 * scale;
        (px, py)
    };

    for obj in adapter.scene.objects.values() {
        let p = obj.position;
        let c = obj.material.color();
        let color = format!("rgb({},{},{})", (c[0]*255.0) as u8, (c[1]*255.0) as u8, (c[2]*255.0) as u8);
        match &obj.shape {
            kolibri_core::scene::Shape::Box { width, height, depth } => {
                let corners = [
                    (p[0],p[1],p[2]), (p[0]+width,p[1],p[2]),
                    (p[0]+width,p[1],p[2]+depth), (p[0],p[1],p[2]+depth),
                    (p[0],p[1]+height,p[2]), (p[0]+width,p[1]+height,p[2]),
                    (p[0]+width,p[1]+height,p[2]+depth), (p[0],p[1]+height,p[2]+depth),
                ];
                let edges = [(0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7)];
                for (a,b) in &edges {
                    let (x1,y1) = project(corners[*a].0, corners[*a].1, corners[*a].2);
                    let (x2,y2) = project(corners[*b].0, corners[*b].1, corners[*b].2);
                    lines.push(format!(r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="1.5" opacity="0.8"/>"#, x1,y1,x2,y2, color));
                }
            }
            kolibri_core::scene::Shape::Cylinder { radius, height, .. } => {
                let cx = p[0]+radius; let cz = p[2]+radius;
                let (px, py) = project(cx, p[1]+height/2.0, cz);
                let r_screen = radius * 0.05;
                lines.push(format!(r#"<ellipse cx="{:.1}" cy="{:.1}" rx="{:.1}" ry="{:.1}" fill="none" stroke="{}" stroke-width="1.5" opacity="0.8"/>"#, px, py, r_screen*0.866, r_screen*0.5, color));
            }
            kolibri_core::scene::Shape::SteelProfile { params, length, .. } => {
                // 以 bounding box 繪製線框
                let (w, d) = (params.b, params.h);
                let h = *length;
                let corners = [
                    (p[0],p[1],p[2]), (p[0]+w,p[1],p[2]),
                    (p[0]+w,p[1],p[2]+d), (p[0],p[1],p[2]+d),
                    (p[0],p[1]+h,p[2]), (p[0]+w,p[1]+h,p[2]),
                    (p[0]+w,p[1]+h,p[2]+d), (p[0],p[1]+h,p[2]+d),
                ];
                let edges = [(0,1),(1,2),(2,3),(3,0),(4,5),(5,6),(6,7),(7,4),(0,4),(1,5),(2,6),(3,7)];
                for (a,b) in &edges {
                    let (x1,y1) = project(corners[*a].0, corners[*a].1, corners[*a].2);
                    let (x2,y2) = project(corners[*b].0, corners[*b].1, corners[*b].2);
                    lines.push(format!(r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{}" stroke-width="1.5" opacity="0.8"/>"#, x1,y1,x2,y2, color));
                }
            }
            _ => {}
        }
    }

    let obj_count = adapter.scene.objects.len();
    let body = lines.join("\n");
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" style="background:#1a1b2e">
<text x="10" y="20" fill="rgb(136,136,170)" font-size="12">{obj_count} objects</text>
{body}
</svg>"#
    );
    axum::response::Html(svg)
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

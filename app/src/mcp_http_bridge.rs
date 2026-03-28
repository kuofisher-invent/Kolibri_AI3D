//! MCP HTTP server that bridges to APP's GUI scene via McpBridge channel
//! 所有操作直接影響 APP 視窗裡的場景

use crate::mcp_server::{McpCommand, McpResult};
use std::sync::mpsc::Sender;
use std::sync::Arc;

type CmdSender = Sender<(McpCommand, std::sync::mpsc::Sender<McpResult>)>;

/// 啟動橋接式 HTTP MCP server（操作 APP 的場景）
pub fn start_bridged_http(port: u16) -> CmdSender {
    let (bridge, cmd_tx) = crate::mcp_server::McpBridge::new();

    let cmd_tx_clone = cmd_tx.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(run_bridged_server(port, cmd_tx_clone));
    });

    // 回傳 bridge 讓 APP 接收指令
    // 但我們需要把 bridge 塞進 app — 改用不同做法
    // 實際上我們只需要 cmd_tx 給 HTTP server，bridge.cmd_rx 給 APP
    // 所以把 bridge 存到 APP 的 mcp_bridge 欄位

    // 這裡改成直接回傳，讓呼叫者設定 bridge
    cmd_tx
}

/// 建立 bridge 並回傳 (bridge 給 APP, cmd_tx 給 HTTP server)
pub fn create_bridge_and_start_http(port: u16) -> crate::mcp_server::McpBridge {
    let (bridge, cmd_tx) = crate::mcp_server::McpBridge::new();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(run_bridged_server(port, cmd_tx));
    });

    bridge
}

async fn run_bridged_server(port: u16, cmd_tx: CmdSender) {
    use axum::{extract::State, routing::{get, post}, Json, Router};
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};

    #[derive(Deserialize)]
    struct JsonRpcRequest {
        jsonrpc: String,
        id: Option<Value>,
        method: String,
        params: Option<Value>,
    }

    #[derive(Serialize)]
    struct JsonRpcResponse {
        jsonrpc: String,
        id: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<JsonRpcError>,
    }

    #[derive(Serialize)]
    struct JsonRpcError { code: i32, message: String }

    impl JsonRpcResponse {
        fn ok(id: Option<Value>, result: Value) -> Self {
            Self { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
        }
        fn err(id: Option<Value>, code: i32, msg: &str) -> Self {
            Self { jsonrpc: "2.0".into(), id, result: None,
                   error: Some(JsonRpcError { code, message: msg.into() }) }
        }
    }

    let tx = Arc::new(std::sync::Mutex::new(cmd_tx));

    let app = Router::new()
        .route("/", get(|| async { axum::response::Html(kolibri_mcp::dashboard::DASHBOARD_HTML) }))
        .route("/mcp", post({
            let tx = tx.clone();
            move |Json(req): Json<JsonRpcRequest>| {
                let tx = tx.clone();
                async move {
                    let id = req.id.clone();
                    let response = match req.method.as_str() {
                        "initialize" => JsonRpcResponse::ok(id, json!({
                            "protocolVersion": "2024-11-05",
                            "serverInfo": { "name": "kolibri-ai3d-gui", "version": "1.0.0" },
                            "capabilities": { "tools": {} }
                        })),
                        "notifications/initialized" => JsonRpcResponse::ok(id, json!({})),
                        "tools/list" => {
                            // 回傳 APP 支援的工具列表
                            JsonRpcResponse::ok(id, json!({
                                "tools": kolibri_mcp::adapter::KolibriAdapter::new().tool_definitions()
                            }))
                        }
                        "tools/call" => {
                            let params = req.params.unwrap_or(json!({}));
                            let tool = params["name"].as_str().unwrap_or("");
                            let args = params.get("arguments").cloned().unwrap_or(json!({}));

                            // 轉成 McpCommand 送到 GUI thread
                            match tool_to_command(tool, &args) {
                                Some(cmd) => {
                                    let (result_tx, result_rx) = std::sync::mpsc::channel();
                                    let send_ok = {
                                        let tx = tx.lock().unwrap();
                                        tx.send((cmd, result_tx)).is_ok()
                                    };
                                    if !send_ok {
                                        return Json(JsonRpcResponse::err(id, -32603, "GUI not responding"));
                                    }
                                    match result_rx.recv_timeout(std::time::Duration::from_secs(120)) {
                                        Ok(result) => {
                                            let text = serde_json::to_string_pretty(&result.data).unwrap_or_default();
                                            JsonRpcResponse::ok(id, json!({
                                                "content": [{"type": "text", "text": text}]
                                            }))
                                        }
                                        Err(_) => JsonRpcResponse::err(id, -32603, "Timeout waiting for GUI (10s)"),
                                    }
                                }
                                None => {
                                    // 不認識的工具，回傳錯誤
                                    JsonRpcResponse::ok(id, json!({
                                        "content": [{"type": "text", "text": format!("Unknown tool: {}", tool)}],
                                        "isError": true
                                    }))
                                }
                            }
                        }
                        "resources/list" => JsonRpcResponse::ok(id, json!({"resources": []})),
                        "prompts/list" => JsonRpcResponse::ok(id, json!({"prompts": []})),
                        _ => JsonRpcResponse::err(id, -32601, &format!("Method not found: {}", req.method)),
                    };
                    Json(response)
                }
            }
        }))
        .route("/health", get({
            let tx = tx.clone();
            move || {
                let tx = tx.clone();
                async move {
                    // 送 GetSceneState 取得物件數
                    let count = {
                        let (result_tx, result_rx) = std::sync::mpsc::channel();
                        let send_ok = tx.lock().unwrap().send((McpCommand::GetSceneState, result_tx)).is_ok();
                        if send_ok {
                            result_rx.recv_timeout(std::time::Duration::from_secs(2))
                                .ok()
                                .and_then(|r| r.data["count"].as_u64())
                                .unwrap_or(0)
                        } else { 0 }
                    };
                    Json(json!({
                        "status": "ok",
                        "server": "kolibri-ai3d-gui-bridge",
                        "mode": "gui",
                        "object_count": count,
                    }))
                }
            }
        }))
        .layer(tower_http::cors::CorsLayer::permissive());

    let addr = format!("0.0.0.0:{}", port);
    eprintln!("[kolibri-mcp] GUI Bridge HTTP on http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(&addr).await
        .expect("Cannot bind HTTP port");
    axum::serve(listener, app).await
        .expect("HTTP server error");
}

/// 將 MCP tool name + args 轉成 APP 的 McpCommand
fn tool_to_command(tool: &str, args: &serde_json::Value) -> Option<McpCommand> {
    use serde_json::Value;

    fn parse_pos(v: &Value) -> [f32; 3] {
        if let Some(arr) = v.as_array() {
            if arr.len() >= 3 {
                return [
                    arr[0].as_f64().unwrap_or(0.0) as f32,
                    arr[1].as_f64().unwrap_or(0.0) as f32,
                    arr[2].as_f64().unwrap_or(0.0) as f32,
                ];
            }
        }
        [0.0; 3]
    }

    match tool {
        "get_scene_state" => Some(McpCommand::GetSceneState),
        "create_box" => Some(McpCommand::CreateBox {
            name: args["name"].as_str().unwrap_or("Box").into(),
            position: parse_pos(&args["position"]),
            width: args["width"].as_f64().unwrap_or(1000.0) as f32,
            height: args["height"].as_f64().unwrap_or(1000.0) as f32,
            depth: args["depth"].as_f64().unwrap_or(1000.0) as f32,
            material: args["material"].as_str().unwrap_or("concrete").into(),
        }),
        "create_cylinder" => Some(McpCommand::CreateCylinder {
            name: args["name"].as_str().unwrap_or("Cylinder").into(),
            position: parse_pos(&args["position"]),
            radius: args["radius"].as_f64().unwrap_or(500.0) as f32,
            height: args["height"].as_f64().unwrap_or(1000.0) as f32,
            material: args["material"].as_str().unwrap_or("concrete").into(),
        }),
        "create_sphere" => Some(McpCommand::CreateSphere {
            name: args["name"].as_str().unwrap_or("Sphere").into(),
            position: parse_pos(&args["position"]),
            radius: args["radius"].as_f64().unwrap_or(500.0) as f32,
            material: args["material"].as_str().unwrap_or("concrete").into(),
        }),
        "delete_object" => Some(McpCommand::DeleteObject {
            id: args["id"].as_str().unwrap_or("").into(),
        }),
        "move_object" => Some(McpCommand::MoveObject {
            id: args["id"].as_str().unwrap_or("").into(),
            position: parse_pos(&args["position"]),
        }),
        "set_material" => Some(McpCommand::SetMaterial {
            id: args["id"].as_str().unwrap_or("").into(),
            material: args["material"].as_str().unwrap_or("concrete").into(),
        }),
        "rotate_object" => Some(McpCommand::RotateObject {
            id: args["id"].as_str().unwrap_or("").into(),
            angle_deg: args["angle_deg"].as_f64().unwrap_or(0.0) as f32,
        }),
        "scale_object" => Some(McpCommand::ScaleObject {
            id: args["id"].as_str().unwrap_or("").into(),
            factor: {
                let f = &args["factor"];
                if let Some(arr) = f.as_array() {
                    [arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                     arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                     arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32]
                } else { [1.0; 3] }
            },
        }),
        "duplicate_object" => Some(McpCommand::DuplicateObject {
            id: args["id"].as_str().unwrap_or("").into(),
            offset: parse_pos(&args.get("offset").cloned().unwrap_or(serde_json::json!([500,0,0]))),
        }),
        "get_object_info" => Some(McpCommand::GetObjectInfo {
            id: args["id"].as_str().unwrap_or("").into(),
        }),
        "clear_scene" => Some(McpCommand::ClearScene),
        "undo" => Some(McpCommand::Undo),
        "redo" => Some(McpCommand::Redo),
        "shutdown" => Some(McpCommand::Shutdown),
        "import_file" => Some(McpCommand::ImportFile {
            path: args["path"].as_str().unwrap_or("").into(),
        }),
        "screenshot" => Some(McpCommand::Screenshot {
            path: args["path"].as_str().unwrap_or("D:/AI_Design/Kolibri_Ai3D/app/screenshot.png").into(),
        }),
        "export_scene" => Some(McpCommand::ExportScene {
            path: args["path"].as_str().unwrap_or("D:/AI_Design/Kolibri_Ai3D/app/scene_export.json").into(),
        }),
        _ => None,
    }
}

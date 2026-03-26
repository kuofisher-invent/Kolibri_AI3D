//! Layer 3a: stdio transport（Claude Desktop 相容）
//! 每行一個 JSON-RPC 2.0 訊息，stdin → 處理 → stdout

use std::io::{BufRead, Write};
use crate::protocol::*;
use crate::adapter::KolibriAdapter;

/// 啟動 stdio MCP server（阻塞式，直到 stdin EOF）
pub fn run_stdio() {
    let mut adapter = KolibriAdapter::new();

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    eprintln!("[kolibri-mcp] stdio server started");

    for line in stdin.lock().lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        let line = line.trim().to_string();
        if line.is_empty() { continue; }

        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(req) => handle_request(req, &mut adapter),
            Err(e) => JsonRpcResponse::err(None, -32700, &format!("Parse error: {}", e)),
        };

        let resp_str = serde_json::to_string(&response).unwrap_or_default();
        let _ = writeln!(out, "{}", resp_str);
        let _ = out.flush();
    }

    eprintln!("[kolibri-mcp] stdio server stopped");
}

fn handle_request(req: JsonRpcRequest, adapter: &mut KolibriAdapter) -> JsonRpcResponse {
    let id = req.id.clone();
    match req.method.as_str() {
        "initialize" => initialize_response(id),
        "notifications/initialized" => JsonRpcResponse::ok(id, serde_json::json!({})),
        "tools/list" => {
            let tools = adapter.tool_definitions();
            JsonRpcResponse::ok(id, serde_json::json!({ "tools": tools }))
        }
        "tools/call" => {
            let params = req.params.unwrap_or(serde_json::json!({}));
            let tool_name = params["name"].as_str().unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

            if tool_name == "shutdown" {
                let _ = serde_json::to_string(&tool_result(id.clone(), serde_json::json!({"message":"Shutting down..."})));
                std::process::exit(0);
            }

            let result = adapter.execute_tool(tool_name, &args);
            tool_result(id, result)
        }
        "resources/list" => JsonRpcResponse::ok(id, serde_json::json!({"resources": []})),
        "prompts/list" => JsonRpcResponse::ok(id, serde_json::json!({"prompts": []})),
        other => JsonRpcResponse::err(id, -32601, &format!("Method not found: {}", other)),
    }
}

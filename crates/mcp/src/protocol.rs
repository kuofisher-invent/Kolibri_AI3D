//! Layer 1: MCP JSON-RPC 2.0 協定型別
//! 與傳輸層無關的純資料結構

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── JSON-RPC 2.0 ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn ok(id: Option<Value>, result: Value) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
    }
    pub fn err(id: Option<Value>, code: i32, msg: &str) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: None,
               error: Some(JsonRpcError { code, message: msg.into() }) }
    }
}

// ─── MCP Tool Definition ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

// ─── MCP Server Info ────────────────────────────────────────────────────────

pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
pub const SERVER_NAME: &str = "kolibri-ai3d";
pub const SERVER_VERSION: &str = "1.0.0";

pub fn initialize_response(id: Option<Value>) -> JsonRpcResponse {
    JsonRpcResponse::ok(id, serde_json::json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION },
        "capabilities": { "tools": {} }
    }))
}

// ─── Tool Result wrapper (MCP content format) ───────────────────────────────

pub fn tool_result(id: Option<Value>, data: Value) -> JsonRpcResponse {
    let text = serde_json::to_string_pretty(&data).unwrap_or_default();
    JsonRpcResponse::ok(id, serde_json::json!({
        "content": [{ "type": "text", "text": text }]
    }))
}

pub fn tool_error(id: Option<Value>, msg: &str) -> JsonRpcResponse {
    JsonRpcResponse::ok(id, serde_json::json!({
        "content": [{ "type": "text", "text": msg }],
        "isError": true
    }))
}

//! kolibri-mcp — 4 層 MCP Server 架構
//!
//! Layer 1: protocol   — MCP JSON-RPC 2.0 協定型別
//! Layer 2: adapter    — Kolibri 工具轉接器（tool name → Scene 操作）
//! Layer 3: transport  — stdio（Claude Desktop）+ HTTP/SSE（ChatGPT）
//! Layer 4: test       — Rust test harness

pub mod protocol;
pub mod adapter;
pub mod dashboard;
pub mod transport_stdio;
pub mod transport_http;

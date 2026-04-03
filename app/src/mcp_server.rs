//! Built-in MCP Server for Claude Desktop integration
//! Activated with --mcp flag or through the GUI menu

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
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
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn ok(id: Option<Value>, result: Value) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
    }
    fn err(id: Option<Value>, code: i32, msg: &str) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: None,
               error: Some(JsonRpcError { code, message: msg.into() }) }
    }
}

/// MCP command that can be sent from the MCP thread to the GUI thread
#[derive(Debug, Clone)]
pub enum McpCommand {
    GetSceneState,
    CreateBox { name: String, position: [f32; 3], width: f32, height: f32, depth: f32, material: String },
    CreateCylinder { name: String, position: [f32; 3], radius: f32, height: f32, material: String },
    CreateSphere { name: String, position: [f32; 3], radius: f32, material: String },
    DeleteObject { id: String },
    MoveObject { id: String, position: [f32; 3] },
    SetMaterial { id: String, material: String },
    ClearScene,
    RotateObject { id: String, angle_deg: f32 },
    ScaleObject { id: String, factor: [f32; 3] },
    DuplicateObject { id: String, offset: [f32; 3] },
    GetObjectInfo { id: String },
    Undo,
    Redo,
    Shutdown,
    ImportFile { path: String },
    Screenshot { path: String },
    ExportScene { path: String },
    SetLayoutMode { enabled: bool },
    // ── 鋼構 ──
    #[cfg(feature = "steel")]
    CreateSteelColumn { position: [f32; 3], profile: String, height: f32 },
    #[cfg(feature = "steel")]
    CreateSteelBeam { p1: [f32; 3], p2: [f32; 3], profile: String },
    #[cfg(feature = "steel")]
    CreateSteelConnection { member_ids: Vec<String>, conn_type: String },
    // ── Debug Trace ──
    StartTrace { interval_ms: u32 },
    StopTrace,
    GetTraceStatus,
    // ── 2D Drafting ──
    #[cfg(feature = "drafting")]
    DraftAddLine { p1: [f64; 2], p2: [f64; 2] },
    #[cfg(feature = "drafting")]
    DraftAddCircle { center: [f64; 2], radius: f64 },
    #[cfg(feature = "drafting")]
    DraftAddArc { center: [f64; 2], radius: f64, start_angle: f64, end_angle: f64 },
    #[cfg(feature = "drafting")]
    DraftAddRectangle { p1: [f64; 2], p2: [f64; 2] },
    #[cfg(feature = "drafting")]
    DraftAddPolyline { points: Vec<[f64; 2]>, closed: bool },
    #[cfg(feature = "drafting")]
    DraftAddText { position: [f64; 2], content: String, height: f64 },
    #[cfg(feature = "drafting")]
    DraftAddDimLinear { p1: [f64; 2], p2: [f64; 2], offset: f64 },
    #[cfg(feature = "drafting")]
    DraftDelete { id: u64 },
    #[cfg(feature = "drafting")]
    DraftClear,
    #[cfg(feature = "drafting")]
    DraftList,
    #[cfg(feature = "drafting")]
    DraftGetEntity { id: u64 },
    #[cfg(feature = "drafting")]
    DraftSetTool { tool: String },
    #[cfg(feature = "drafting")]
    DraftSelect { ids: Vec<u64> },
    #[cfg(feature = "drafting")]
    DraftImportFile { path: String },
    #[cfg(feature = "drafting")]
    DraftSetZoom { zoom: f32, offset_x: f32, offset_y: f32 },
}

/// MCP result sent back from GUI thread to MCP thread
#[derive(Debug, Clone)]
pub struct McpResult {
    pub success: bool,
    pub data: Value,
}

/// Channels for MCP <-> GUI communication
pub struct McpBridge {
    pub cmd_rx: std::sync::mpsc::Receiver<(McpCommand, std::sync::mpsc::Sender<McpResult>)>,
}

impl McpBridge {
    pub fn new() -> (Self, std::sync::mpsc::Sender<(McpCommand, std::sync::mpsc::Sender<McpResult>)>) {
        let (tx, rx) = std::sync::mpsc::channel();
        (Self { cmd_rx: rx }, tx)
    }
}

/// Run MCP stdio server on a background thread
pub fn run_mcp_stdio(cmd_tx: std::sync::mpsc::Sender<(McpCommand, std::sync::mpsc::Sender<McpResult>)>) {
    use std::io::{BufRead, Write};

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim().to_string();
        if line.is_empty() { continue; }

        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(req) => handle_mcp_request(req, &cmd_tx),
            Err(e) => JsonRpcResponse::err(None, -32700, &format!("Parse error: {}", e)),
        };

        let resp_str = serde_json::to_string(&response).unwrap_or_default();
        let _ = writeln!(out, "{}", resp_str);
        let _ = out.flush();
    }
}

fn handle_mcp_request(
    req: JsonRpcRequest,
    cmd_tx: &std::sync::mpsc::Sender<(McpCommand, std::sync::mpsc::Sender<McpResult>)>,
) -> JsonRpcResponse {
    let id = req.id.clone();
    match req.method.as_str() {
        "initialize" => JsonRpcResponse::ok(id, json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": { "name": "kolibri-ai3d", "version": "1.0.0" },
            "capabilities": { "tools": {} }
        })),
        "tools/list" => JsonRpcResponse::ok(id, json!({
            "tools": [
                {
                    "name": "get_scene_state",
                    "description": "\u{7372}\u{53d6}\u{7576}\u{524d}3D\u{5834}\u{666f}\u{5b8c}\u{6574}\u{72c0}\u{614b}",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "create_box",
                    "description": "\u{5efa}\u{7acb}\u{65b9}\u{584a}\u{3002}\u{55ae}\u{4f4d}mm\u{3002}",
                    "inputSchema": {
                        "type": "object",
                        "required": ["width", "height", "depth"],
                        "properties": {
                            "name": { "type": "string" },
                            "position": { "type": "array", "items": {"type": "number"}, "default": [0,0,0] },
                            "width": { "type": "number" },
                            "height": { "type": "number" },
                            "depth": { "type": "number" },
                            "material": { "type": "string", "default": "concrete" }
                        }
                    }
                },
                {
                    "name": "create_cylinder",
                    "description": "\u{5efa}\u{7acb}\u{5713}\u{67f1}\u{3002}",
                    "inputSchema": {
                        "type": "object",
                        "required": ["radius", "height"],
                        "properties": {
                            "name": { "type": "string" },
                            "position": { "type": "array", "items": {"type": "number"} },
                            "radius": { "type": "number" },
                            "height": { "type": "number" },
                            "material": { "type": "string" }
                        }
                    }
                },
                {
                    "name": "create_sphere",
                    "description": "\u{5efa}\u{7acb}\u{7403}\u{9ad4}\u{3002}",
                    "inputSchema": {
                        "type": "object", "required": ["radius"],
                        "properties": {
                            "name": {"type": "string"},
                            "position": {"type": "array", "items": {"type": "number"}},
                            "radius": {"type": "number"},
                            "material": {"type": "string"}
                        }
                    }
                },
                {
                    "name": "delete_object",
                    "description": "\u{522a}\u{9664}\u{7269}\u{4ef6}",
                    "inputSchema": { "type": "object", "required": ["id"], "properties": { "id": {"type": "string"} } }
                },
                {
                    "name": "move_object",
                    "description": "\u{79fb}\u{52d5}\u{7269}\u{4ef6}\u{5230}\u{6307}\u{5b9a}\u{4f4d}\u{7f6e}",
                    "inputSchema": { "type": "object", "required": ["id", "position"],
                        "properties": { "id": {"type":"string"}, "position": {"type":"array","items":{"type":"number"}} } }
                },
                {
                    "name": "set_material",
                    "description": "\u{8a2d}\u{5b9a}\u{6750}\u{8cea}\u{3002}\u{53ef}\u{7528}: concrete, wood, glass, metal, brick, white...",
                    "inputSchema": { "type": "object", "required": ["id", "material"],
                        "properties": { "id": {"type":"string"}, "material": {"type":"string"} } }
                },
                {
                    "name": "clear_scene",
                    "description": "清空場景",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "rotate_object",
                    "description": "旋轉物件（Y軸，角度制）",
                    "inputSchema": { "type": "object", "required": ["id", "angle_deg"],
                        "properties": { "id":{"type":"string"}, "angle_deg":{"type":"number"} } }
                },
                {
                    "name": "scale_object",
                    "description": "縮放物件。factor=[x,y,z] 倍率",
                    "inputSchema": { "type": "object", "required": ["id", "factor"],
                        "properties": { "id":{"type":"string"}, "factor":{"type":"array","items":{"type":"number"}} } }
                },
                {
                    "name": "duplicate_object",
                    "description": "複製物件。offset=[x,y,z] 偏移量(mm)",
                    "inputSchema": { "type": "object", "required": ["id"],
                        "properties": { "id":{"type":"string"}, "offset":{"type":"array","items":{"type":"number"},"default":[500,0,0]} } }
                },
                {
                    "name": "get_object_info",
                    "description": "取得單一物件詳細資訊",
                    "inputSchema": { "type": "object", "required": ["id"], "properties": { "id":{"type":"string"} } }
                },
                {
                    "name": "undo",
                    "description": "撤銷上一步",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "redo",
                    "description": "重做",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "shutdown",
                    "description": "關閉 Kolibri CAD 應用程式",
                    "inputSchema": { "type": "object", "properties": {} }
                }
            ]
        })),
        "tools/call" => {
            let params = req.params.unwrap_or(json!({}));
            let tool = params["name"].as_str().unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));

            let cmd = match tool {
                "get_scene_state" => McpCommand::GetSceneState,
                "create_box" => McpCommand::CreateBox {
                    name: args["name"].as_str().unwrap_or("Box").into(),
                    position: parse_pos(&args["position"]),
                    width: args["width"].as_f64().unwrap_or(1000.0) as f32,
                    height: args["height"].as_f64().unwrap_or(1000.0) as f32,
                    depth: args["depth"].as_f64().unwrap_or(1000.0) as f32,
                    material: args["material"].as_str().unwrap_or("concrete").into(),
                },
                "create_cylinder" => McpCommand::CreateCylinder {
                    name: args["name"].as_str().unwrap_or("Cylinder").into(),
                    position: parse_pos(&args["position"]),
                    radius: args["radius"].as_f64().unwrap_or(500.0) as f32,
                    height: args["height"].as_f64().unwrap_or(1000.0) as f32,
                    material: args["material"].as_str().unwrap_or("concrete").into(),
                },
                "create_sphere" => McpCommand::CreateSphere {
                    name: args["name"].as_str().unwrap_or("Sphere").into(),
                    position: parse_pos(&args["position"]),
                    radius: args["radius"].as_f64().unwrap_or(500.0) as f32,
                    material: args["material"].as_str().unwrap_or("concrete").into(),
                },
                "delete_object" => McpCommand::DeleteObject {
                    id: args["id"].as_str().unwrap_or("").into(),
                },
                "move_object" => McpCommand::MoveObject {
                    id: args["id"].as_str().unwrap_or("").into(),
                    position: parse_pos(&args["position"]),
                },
                "set_material" => McpCommand::SetMaterial {
                    id: args["id"].as_str().unwrap_or("").into(),
                    material: args["material"].as_str().unwrap_or("concrete").into(),
                },
                "clear_scene" => McpCommand::ClearScene,
                "rotate_object" => McpCommand::RotateObject {
                    id: args["id"].as_str().unwrap_or("").into(),
                    angle_deg: args["angle_deg"].as_f64().unwrap_or(0.0) as f32,
                },
                "scale_object" => McpCommand::ScaleObject {
                    id: args["id"].as_str().unwrap_or("").into(),
                    factor: {
                        let f = &args["factor"];
                        if let Some(arr) = f.as_array() {
                            [arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                             arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                             arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32]
                        } else { [1.0; 3] }
                    },
                },
                "duplicate_object" => McpCommand::DuplicateObject {
                    id: args["id"].as_str().unwrap_or("").into(),
                    offset: parse_pos(&args.get("offset").cloned().unwrap_or(json!([500,0,0]))),
                },
                "get_object_info" => McpCommand::GetObjectInfo {
                    id: args["id"].as_str().unwrap_or("").into(),
                },
                "undo" => McpCommand::Undo,
                "redo" => McpCommand::Redo,
                "shutdown" => McpCommand::Shutdown,
                other => return JsonRpcResponse::ok(id, json!({
                    "content": [{"type": "text", "text": format!("Unknown tool: {}", other)}],
                    "isError": true
                })),
            };

            // Send command to GUI thread and wait for result
            let (result_tx, result_rx) = std::sync::mpsc::channel();
            if cmd_tx.send((cmd, result_tx)).is_err() {
                return JsonRpcResponse::err(id, -32603, "GUI not responding");
            }
            match result_rx.recv_timeout(std::time::Duration::from_secs(5)) {
                Ok(result) => JsonRpcResponse::ok(id, json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&result.data).unwrap_or_default()}]
                })),
                Err(_) => JsonRpcResponse::err(id, -32603, "Timeout waiting for GUI"),
            }
        }
        "notifications/initialized" => JsonRpcResponse::ok(id, json!({})),
        "resources/list" => JsonRpcResponse::ok(id, json!({"resources": []})),
        "prompts/list" => JsonRpcResponse::ok(id, json!({"prompts": []})),
        other => JsonRpcResponse::err(id, -32601, &format!("Method not found: {}", other)),
    }
}

/// Standalone MCP mode: no GUI, own Scene, processes commands directly
pub fn run_mcp_standalone() {
    use std::io::{BufRead, Write};
    use crate::scene::{Scene, MaterialKind};

    let mut scene = Scene::default();
    let mut ai_log = crate::ai_log::AiLog::new();
    let actor = crate::ai_log::ActorId::claude();

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        let line = line.trim().to_string();
        if line.is_empty() { continue; }

        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(req) => {
                let id = req.id.clone();
                match req.method.as_str() {
                    "initialize" => JsonRpcResponse::ok(id, json!({
                        "protocolVersion": "2024-11-05",
                        "serverInfo": { "name": "kolibri-ai3d", "version": "1.0.0" },
                        "capabilities": { "tools": {} }
                    })),
                    "tools/list" => handle_tools_list(id),
                    "tools/call" => {
                        let params = req.params.unwrap_or(json!({}));
                        let tool = params["name"].as_str().unwrap_or("").to_string();
                        let args = params.get("arguments").cloned().unwrap_or(json!({}));
                        let result = execute_tool(&mut scene, &mut ai_log, &actor, &tool, &args);
                        JsonRpcResponse::ok(id, json!({
                            "content": [{"type": "text", "text": serde_json::to_string_pretty(&result).unwrap_or_default()}]
                        }))
                    }
                    "notifications/initialized" => JsonRpcResponse::ok(id, json!({})),
                    "resources/list" => JsonRpcResponse::ok(id, json!({"resources": []})),
                    "prompts/list" => JsonRpcResponse::ok(id, json!({"prompts": []})),
                    other => JsonRpcResponse::err(id, -32601, &format!("Method not found: {}", other)),
                }
            }
            Err(e) => JsonRpcResponse::err(None, -32700, &format!("Parse error: {}", e)),
        };

        let resp_str = serde_json::to_string(&response).unwrap_or_default();
        let _ = writeln!(out, "{}", resp_str);
        let _ = out.flush();
    }
}

fn handle_tools_list(id: Option<Value>) -> JsonRpcResponse {
    JsonRpcResponse::ok(id, json!({
        "tools": [
            { "name": "get_scene_state", "description": "獲取當前3D場景完整狀態，包含所有物件ID、尺寸、位置、材質。", "inputSchema": { "type": "object", "properties": {} } },
            { "name": "create_box", "description": "建立方塊。單位mm。", "inputSchema": { "type": "object", "required": ["width","height","depth"], "properties": { "name":{"type":"string"}, "position":{"type":"array","items":{"type":"number"},"default":[0,0,0]}, "width":{"type":"number"}, "height":{"type":"number"}, "depth":{"type":"number"}, "material":{"type":"string","default":"concrete"} } } },
            { "name": "create_cylinder", "description": "建立圓柱。", "inputSchema": { "type": "object", "required": ["radius","height"], "properties": { "name":{"type":"string"}, "position":{"type":"array","items":{"type":"number"}}, "radius":{"type":"number"}, "height":{"type":"number"}, "material":{"type":"string"} } } },
            { "name": "create_sphere", "description": "建立球體。", "inputSchema": { "type": "object", "required": ["radius"], "properties": { "name":{"type":"string"}, "position":{"type":"array","items":{"type":"number"}}, "radius":{"type":"number"}, "material":{"type":"string"} } } },
            { "name": "delete_object", "description": "刪除物件", "inputSchema": { "type": "object", "required": ["id"], "properties": { "id":{"type":"string"} } } },
            { "name": "move_object", "description": "移動物件到指定位置(mm)", "inputSchema": { "type": "object", "required": ["id","position"], "properties": { "id":{"type":"string"}, "position":{"type":"array","items":{"type":"number"}} } } },
            { "name": "set_material", "description": "設定材質。可用: concrete, wood, glass, metal, brick, white, marble, steel, aluminum, copper, gold, tile, asphalt, grass", "inputSchema": { "type": "object", "required": ["id","material"], "properties": { "id":{"type":"string"}, "material":{"type":"string"} } } },
            { "name": "clear_scene", "description": "清空場景", "inputSchema": { "type": "object", "properties": {} } },
            { "name": "push_pull", "description": "推拉物件的面。face: top/bottom/front/back/left/right。distance: mm正值向外。", "inputSchema": { "type": "object", "required": ["id","face","distance"], "properties": { "id":{"type":"string"}, "face":{"type":"string","enum":["top","bottom","front","back","left","right"]}, "distance":{"type":"number"} } } },
            { "name": "save_scene", "description": "儲存場景到檔案", "inputSchema": { "type": "object", "required": ["path"], "properties": { "path":{"type":"string"} } } },
            { "name": "load_scene", "description": "載入場景", "inputSchema": { "type": "object", "required": ["path"], "properties": { "path":{"type":"string"} } } },
            { "name": "rotate_object", "description": "旋轉物件（Y軸，角度制）", "inputSchema": { "type": "object", "required": ["id","angle_deg"], "properties": { "id":{"type":"string"}, "angle_deg":{"type":"number"} } } },
            { "name": "scale_object", "description": "縮放物件 factor=[x,y,z]倍率", "inputSchema": { "type": "object", "required": ["id","factor"], "properties": { "id":{"type":"string"}, "factor":{"type":"array","items":{"type":"number"}} } } },
            { "name": "duplicate_object", "description": "複製物件 offset=[x,y,z]mm", "inputSchema": { "type": "object", "required": ["id"], "properties": { "id":{"type":"string"}, "offset":{"type":"array","items":{"type":"number"},"default":[500,0,0]} } } },
            { "name": "get_object_info", "description": "取得單一物件詳細資訊", "inputSchema": { "type": "object", "required": ["id"], "properties": { "id":{"type":"string"} } } },
            { "name": "undo", "description": "撤銷上一步", "inputSchema": { "type": "object", "properties": {} } },
            { "name": "redo", "description": "重做", "inputSchema": { "type": "object", "properties": {} } },
            { "name": "shutdown", "description": "關閉應用程式", "inputSchema": { "type": "object", "properties": {} } }
        ]
    }))
}

fn execute_tool(scene: &mut crate::scene::Scene, ai_log: &mut crate::ai_log::AiLog, actor: &crate::ai_log::ActorId, tool: &str, args: &Value) -> Value {
    use crate::scene::{Shape, MaterialKind};

    let mat_from_str = |s: &str| -> MaterialKind {
        match s.to_lowercase().as_str() {
            "concrete" => MaterialKind::Concrete, "wood" => MaterialKind::Wood,
            "glass" => MaterialKind::Glass, "metal" => MaterialKind::Metal,
            "brick" => MaterialKind::Brick, "white" => MaterialKind::White,
            "marble" => MaterialKind::Marble, "steel" => MaterialKind::Steel,
            "aluminum" => MaterialKind::Aluminum, "copper" => MaterialKind::Copper,
            "gold" => MaterialKind::Gold, "tile" => MaterialKind::Tile,
            "asphalt" => MaterialKind::Asphalt, "grass" => MaterialKind::Grass,
            _ => MaterialKind::Concrete,
        }
    };

    match tool {
        "get_scene_state" => {
            let objs: Vec<Value> = scene.objects.values().map(|o| {
                let dims = match &o.shape {
                    Shape::Box { width, height, depth } => format!("{:.0}×{:.0}×{:.0}", width, height, depth),
                    Shape::Cylinder { radius, height, .. } => format!("r={:.0} h={:.0}", radius, height),
                    Shape::Sphere { radius, .. } => format!("r={:.0}", radius),
                    _ => "—".into(),
                };
                json!({ "id": o.id, "name": o.name, "shape": dims, "position": o.position, "material": o.material.label() })
            }).collect();
            json!({ "object_count": objs.len(), "objects": objs })
        }
        "create_box" => {
            let name = args["name"].as_str().unwrap_or("Box").to_string();
            let pos = parse_pos(&args["position"]);
            let w = args["width"].as_f64().unwrap_or(1000.0) as f32;
            let h = args["height"].as_f64().unwrap_or(1000.0) as f32;
            let d = args["depth"].as_f64().unwrap_or(1000.0) as f32;
            let mat = mat_from_str(args["material"].as_str().unwrap_or("concrete"));
            let id = scene.add_box(name.clone(), pos, w, h, d, mat);
            ai_log.log(actor, "建立方塊", &format!("{} {:.0}×{:.0}×{:.0}", name, w, h, d), vec![id.clone()]);
            json!({ "success": true, "id": id, "size": format!("{:.0}×{:.0}×{:.0}", w, h, d) })
        }
        "create_cylinder" => {
            let name = args["name"].as_str().unwrap_or("Cylinder").to_string();
            let pos = parse_pos(&args["position"]);
            let r = args["radius"].as_f64().unwrap_or(500.0) as f32;
            let h = args["height"].as_f64().unwrap_or(1000.0) as f32;
            let mat = mat_from_str(args["material"].as_str().unwrap_or("concrete"));
            let id = scene.add_cylinder(name.clone(), pos, r, h, 48, mat);
            ai_log.log(actor, "建立圓柱", &format!("{} r={:.0} h={:.0}", name, r, h), vec![id.clone()]);
            json!({ "success": true, "id": id })
        }
        "create_sphere" => {
            let name = args["name"].as_str().unwrap_or("Sphere").to_string();
            let pos = parse_pos(&args["position"]);
            let r = args["radius"].as_f64().unwrap_or(500.0) as f32;
            let mat = mat_from_str(args["material"].as_str().unwrap_or("concrete"));
            let id = scene.add_sphere(name.clone(), pos, r, 32, mat);
            ai_log.log(actor, "建立球體", &format!("{} r={:.0}", name, r), vec![id.clone()]);
            json!({ "success": true, "id": id })
        }
        "delete_object" => {
            let oid = args["id"].as_str().unwrap_or("");
            let ok = scene.delete(oid);
            if ok { ai_log.log(actor, "刪除物件", oid, vec![oid.into()]); }
            json!({ "success": ok })
        }
        "move_object" => {
            let oid = args["id"].as_str().unwrap_or("").to_string();
            let pos = parse_pos(&args["position"]);
            if let Some(obj) = scene.objects.get_mut(&oid) {
                obj.position = pos;
                scene.version += 1;
                ai_log.log(actor, "移動物件", &format!("{} → [{:.0},{:.0},{:.0}]", oid, pos[0], pos[1], pos[2]), vec![oid]);
                json!({ "success": true })
            } else {
                json!({ "success": false, "error": "Object not found" })
            }
        }
        "set_material" => {
            let oid = args["id"].as_str().unwrap_or("").to_string();
            let mat_name = args["material"].as_str().unwrap_or("concrete");
            if let Some(obj) = scene.objects.get_mut(&oid) {
                obj.material = mat_from_str(mat_name);
                scene.version += 1;
                ai_log.log(actor, "設定材質", &format!("{} → {}", oid, mat_name), vec![oid]);
                json!({ "success": true })
            } else {
                json!({ "success": false, "error": "Object not found" })
            }
        }
        "push_pull" => {
            let oid = args["id"].as_str().unwrap_or("").to_string();
            let face = args["face"].as_str().unwrap_or("top");
            let dist = args["distance"].as_f64().unwrap_or(0.0) as f32;
            if let Some(obj) = scene.objects.get_mut(&oid) {
                match (&mut obj.shape, face) {
                    (Shape::Box { height, .. }, "top") => *height = (*height + dist).max(10.0),
                    (Shape::Box { height, .. }, "bottom") => { let d = dist.min(*height - 10.0); *height -= d; obj.position[1] += d; }
                    (Shape::Box { width, .. }, "right") => *width = (*width + dist).max(10.0),
                    (Shape::Box { width, .. }, "left") => { let d = dist.min(*width - 10.0); *width -= d; obj.position[0] += d; }
                    (Shape::Box { depth, .. }, "back") => *depth = (*depth + dist).max(10.0),
                    (Shape::Box { depth, .. }, "front") => { let d = dist.min(*depth - 10.0); *depth -= d; obj.position[2] += d; }
                    _ => return json!({ "success": false, "error": "Unsupported shape/face" }),
                }
                scene.version += 1;
                ai_log.log(actor, "推拉", &format!("{}.{} {:.0}mm", oid, face, dist), vec![oid]);
                json!({ "success": true })
            } else {
                json!({ "success": false, "error": "Object not found" })
            }
        }
        "save_scene" => {
            let path = args["path"].as_str().unwrap_or("scene.k3d");
            match scene.save_to_file(path) {
                Ok(()) => json!({ "success": true, "path": path }),
                Err(e) => json!({ "success": false, "error": e.to_string() }),
            }
        }
        "load_scene" => {
            let path = args["path"].as_str().unwrap_or("scene.k3d");
            match scene.load_from_file(path) {
                Ok(count) => json!({ "success": true, "loaded": count }),
                Err(e) => json!({ "success": false, "error": e.to_string() }),
            }
        }
        "clear_scene" => {
            scene.clear();
            ai_log.log(actor, "清空場景", "", vec![]);
            json!({ "success": true })
        }
        "rotate_object" => {
            let oid = args["id"].as_str().unwrap_or("").to_string();
            let deg = args["angle_deg"].as_f64().unwrap_or(0.0) as f32;
            if let Some(obj) = scene.objects.get_mut(&oid) {
                obj.rotation_y += deg.to_radians();
                obj.rotation_xyz[1] = obj.rotation_y;
                obj.rotation_quat = glam::Quat::from_rotation_y(obj.rotation_y).to_array();
                scene.version += 1;
                json!({ "success": true, "rotated": oid })
            } else {
                json!({ "success": false, "error": "Object not found" })
            }
        }
        "scale_object" => {
            let oid = args["id"].as_str().unwrap_or("").to_string();
            let factor = {
                let f = &args["factor"];
                if let Some(arr) = f.as_array() {
                    [arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                     arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                     arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32]
                } else { [1.0; 3] }
            };
            if let Some(obj) = scene.objects.get_mut(&oid) {
                match &mut obj.shape {
                    Shape::Box { width, height, depth } => { *width *= factor[0]; *height *= factor[1]; *depth *= factor[2]; }
                    Shape::Cylinder { radius, height, .. } => { *radius *= factor[0]; *height *= factor[1]; }
                    Shape::Sphere { radius, .. } => { *radius *= factor[0]; }
                    _ => {}
                }
                scene.version += 1;
                json!({ "success": true, "scaled": oid })
            } else {
                json!({ "success": false, "error": "Object not found" })
            }
        }
        "duplicate_object" => {
            let oid = args["id"].as_str().unwrap_or("").to_string();
            let offset = parse_pos(&args.get("offset").cloned().unwrap_or(json!([500,0,0])));
            if let Some(obj) = scene.objects.get(&oid).cloned() {
                let mut clone = obj;
                clone.id = scene.next_id_pub();
                clone.name = format!("{}_copy", clone.name);
                clone.position[0] += offset[0];
                clone.position[1] += offset[1];
                clone.position[2] += offset[2];
                let nid = clone.id.clone();
                scene.objects.insert(nid.clone(), clone);
                scene.version += 1;
                json!({ "success": true, "copy_id": nid })
            } else {
                json!({ "success": false, "error": "Object not found" })
            }
        }
        "get_object_info" => {
            let oid = args["id"].as_str().unwrap_or("");
            if let Some(obj) = scene.objects.get(oid) {
                let shape_info = match &obj.shape {
                    Shape::Box { width, height, depth } => json!({"type":"box","width":width,"height":height,"depth":depth}),
                    Shape::Cylinder { radius, height, segments } => json!({"type":"cylinder","radius":radius,"height":height,"segments":segments}),
                    Shape::Sphere { radius, segments } => json!({"type":"sphere","radius":radius,"segments":segments}),
                    _ => json!({"type":"other"}),
                };
                json!({ "id": obj.id, "name": obj.name, "position": obj.position, "material": obj.material.label(), "shape": shape_info })
            } else {
                json!({ "success": false, "error": "Object not found" })
            }
        }
        "undo" => { let ok = scene.undo(); json!({ "success": ok }) }
        "redo" => { let ok = scene.redo(); json!({ "success": ok }) }
        "shutdown" => {
            std::process::exit(0);
        }
        _ => json!({ "success": false, "error": format!("Unknown tool: {}", tool) }),
    }
}

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

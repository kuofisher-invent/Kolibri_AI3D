use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use anyhow::Result;

use crate::scene::*;

// ─── MCP JSON-RPC Types ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id:      Option<Value>,
    pub method:  String,
    pub params:  Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id:      Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result:  Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error:   Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code:    i32,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn ok(id: Option<Value>, result: Value) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
    }
    pub fn err(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: None,
               error: Some(JsonRpcError { code, message: message.into() }) }
    }
}

// ─── MCP Server ───────────────────────────────────────────────────────────────

pub struct McpServer {
    scene: Arc<Mutex<CadScene>>,
}

impl McpServer {
    pub fn new(scene: Arc<Mutex<CadScene>>) -> Self {
        Self { scene }
    }

    pub fn handle(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let id = req.id.clone();
        match req.method.as_str() {
            "initialize"        => self.handle_initialize(id),
            "tools/list"        => self.handle_tools_list(id),
            "tools/call"        => self.handle_tool_call(id, req.params),
            "resources/list"    => JsonRpcResponse::ok(id, json!({"resources": []})),
            "prompts/list"      => JsonRpcResponse::ok(id, json!({"prompts": []})),
            "notifications/initialized" => JsonRpcResponse::ok(id, json!({})),
            other => {
                tracing::warn!("Unknown method: {}", other);
                JsonRpcResponse::err(id, -32601, format!("Method not found: {}", other))
            }
        }
    }

    // ── initialize ────────────────────────────────────────────────────────────

    fn handle_initialize(&self, id: Option<Value>) -> JsonRpcResponse {
        JsonRpcResponse::ok(id, json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name":    "cad-3d-server",
                "version": "0.1.0"
            },
            "capabilities": {
                "tools": {}
            }
        }))
    }

    // ── tools/list ────────────────────────────────────────────────────────────

    fn handle_tools_list(&self, id: Option<Value>) -> JsonRpcResponse {
        JsonRpcResponse::ok(id, json!({
            "tools": [
                {
                    "name": "get_scene_state",
                    "description": "獲取當前3D場景完整狀態，包含所有物件的ID、尺寸、位置、材質。在執行任何建模操作前應先呼叫此工具以了解場景現況。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "create_geometry",
                    "description": "在場景中創建一個3D幾何體（box/cylinder/sphere）。所有尺寸單位為毫米(mm)。",
                    "inputSchema": {
                        "type": "object",
                        "required": ["shape"],
                        "properties": {
                            "shape": {
                                "type": "string",
                                "enum": ["box", "cylinder", "sphere"],
                                "description": "幾何體類型"
                            },
                            "name": {
                                "type": "string",
                                "description": "物件名稱（選填）"
                            },
                            "origin": {
                                "type": "array",
                                "items": {"type": "number"},
                                "minItems": 3, "maxItems": 3,
                                "description": "[x, y, z] 原點座標(mm)",
                                "default": [0, 0, 0]
                            },
                            "width":    {"type": "number", "description": "X軸寬度(mm)，box必填"},
                            "height":   {"type": "number", "description": "Y軸高度(mm)，box/cylinder必填"},
                            "depth":    {"type": "number", "description": "Z軸深度(mm)，box必填"},
                            "radius":   {"type": "number", "description": "半徑(mm)，cylinder/sphere必填"},
                            "segments": {"type": "number", "description": "圓形細分數，預設32"}
                        }
                    }
                },
                {
                    "name": "push_pull",
                    "description": "對指定面執行Push/Pull擠出操作（SketchUp最核心功能）。正值向外擠出，負值向內縮。",
                    "inputSchema": {
                        "type": "object",
                        "required": ["face", "distance"],
                        "properties": {
                            "face": {
                                "type": "string",
                                "description": "目標面的引用，格式: 'obj_id.face.top'。可用面: top/bottom/front/back/left/right。從get_scene_state獲取可用faces列表。"
                            },
                            "distance": {
                                "type": "number",
                                "description": "擠出距離(mm)，正值=向外擴，負值=向內縮"
                            }
                        }
                    }
                },
                {
                    "name": "set_material",
                    "description": "設定物件材質。支援: wood, concrete, glass, metal, brick, white, black",
                    "inputSchema": {
                        "type": "object",
                        "required": ["obj_id", "material"],
                        "properties": {
                            "obj_id":   {"type": "string", "description": "物件ID"},
                            "material": {
                                "type": "string",
                                "enum": ["wood", "concrete", "glass", "metal", "brick", "white", "black", "default"]
                            }
                        }
                    }
                },
                {
                    "name": "move_object",
                    "description": "移動物件到新位置（相對偏移）",
                    "inputSchema": {
                        "type": "object",
                        "required": ["obj_id", "delta"],
                        "properties": {
                            "obj_id": {"type": "string"},
                            "delta": {
                                "type": "array",
                                "items": {"type": "number"},
                                "minItems": 3, "maxItems": 3,
                                "description": "[dx, dy, dz] 移動偏移量(mm)"
                            }
                        }
                    }
                },
                {
                    "name": "execute_batch",
                    "description": "批次執行多個CAD操作，效率更高。適合一次建立複雜模型（如整個房間）。",
                    "inputSchema": {
                        "type": "object",
                        "required": ["operations"],
                        "properties": {
                            "operations": {
                                "type": "array",
                                "description": "操作序列，每個元素需包含 'type' 欄位",
                                "items": {
                                    "type": "object",
                                    "required": ["type"],
                                    "properties": {
                                        "type": {
                                            "type": "string",
                                            "enum": ["create_box","create_cylinder","create_sphere",
                                                     "push_pull","set_material","move_object",
                                                     "delete_object","rename_object","clear_scene"]
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                {
                    "name": "delete_object",
                    "description": "從場景中刪除一個物件",
                    "inputSchema": {
                        "type": "object",
                        "required": ["obj_id"],
                        "properties": {
                            "obj_id": {"type": "string", "description": "要刪除的物件ID"}
                        }
                    }
                },
                {
                    "name": "check_collisions",
                    "description": "檢查場景中所有物件的碰撞狀況。AI生成模型後應呼叫此工具確認沒有物件互相穿插。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "obj_id": {
                                "type": "string",
                                "description": "選填：只檢查特定物件的碰撞。不填則檢查全場景。"
                            }
                        }
                    }
                },
                {
                    "name": "move_safe",
                    "description": "安全移動物件：自動偵測碰撞，若會碰到其他物件則停在邊界，不允許穿牆/穿地。",
                    "inputSchema": {
                        "type": "object",
                        "required": ["obj_id", "delta"],
                        "properties": {
                            "obj_id": {"type": "string"},
                            "delta": {
                                "type": "array",
                                "items": {"type": "number"},
                                "minItems": 3, "maxItems": 3,
                                "description": "[dx, dy, dz] 移動偏移量(mm)"
                            }
                        }
                    }
                },
                {
                    "name": "check_placement",
                    "description": "在放置物件前先檢查指定位置是否有效（不會碰撞）。若無效會建議附近的替代位置。",
                    "inputSchema": {
                        "type": "object",
                        "required": ["obj_id", "position"],
                        "properties": {
                            "obj_id": {"type": "string"},
                            "position": {
                                "type": "array",
                                "items": {"type": "number"},
                                "minItems": 3, "maxItems": 3,
                                "description": "[x, y, z] 目標放置座標(mm)"
                            }
                        }
                    }
                },
                {
                    "name": "measure_distance",
                    "description": "量測兩個物件之間的最近距離(mm)。距離為0表示剛好接觸，負值表示互相穿插。",
                    "inputSchema": {
                        "type": "object",
                        "required": ["obj_id_a", "obj_id_b"],
                        "properties": {
                            "obj_id_a": {"type": "string", "description": "第一個物件ID"},
                            "obj_id_b": {"type": "string", "description": "第二個物件ID"}
                        }
                    }
                },
                {
                    "name": "clear_scene",
                    "description": "清空整個場景，刪除所有物件。操作不可逆，執行前請確認。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "calculate_weight",
                    "description": "計算物件或整個場景的重量（根據材質密度與體積）。不指定obj_id時計算全場景。",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "obj_id": {
                                "type": "string",
                                "description": "選填：指定物件ID。不填則計算全場景重量。"
                            }
                        }
                    }
                },
                {
                    "name": "get_material_info",
                    "description": "查詢材質的詳細工程屬性（密度、強度、模數等）。可用材質: steel, concrete, wood, glass, aluminum, brick",
                    "inputSchema": {
                        "type": "object",
                        "required": ["material"],
                        "properties": {
                            "material": {
                                "type": "string",
                                "description": "材質名稱"
                            }
                        }
                    }
                }
            ]
        }))
    }

    // ── tools/call ────────────────────────────────────────────────────────────

    fn handle_tool_call(&self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None    => return JsonRpcResponse::err(id, -32602, "Missing params"),
        };

        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None    => return JsonRpcResponse::err(id, -32602, "Missing tool name"),
        };

        let args = params.get("arguments").cloned().unwrap_or(json!({}));

        let result = match tool_name.as_str() {
            "get_scene_state" => self.tool_get_scene_state(),
            "create_geometry" => self.tool_create_geometry(&args),
            "push_pull"       => self.tool_push_pull(&args),
            "set_material"    => self.tool_set_material(&args),
            "move_object"     => self.tool_move_object(&args),
            "execute_batch"   => self.tool_execute_batch(&args),
            "delete_object"    => self.tool_delete_object(&args),
            "check_collisions" => self.tool_check_collisions(&args),
            "move_safe"        => self.tool_move_safe(&args),
            "check_placement"  => self.tool_check_placement(&args),
            "measure_distance" => self.tool_measure_distance(&args),
            "clear_scene"      => self.tool_clear_scene(),
            "calculate_weight" => self.tool_calculate_weight(&args),
            "get_material_info" => self.tool_get_material_info(&args),
            other              => Err(anyhow::anyhow!("Unknown tool: {}", other)),
        };

        match result {
            Ok(content) => JsonRpcResponse::ok(id, json!({
                "content": [{"type": "text", "text": content}]
            })),
            Err(e) => JsonRpcResponse::ok(id, json!({
                "content": [{"type": "text", "text": format!("❌ Error: {}", e)}],
                "isError": true
            })),
        }
    }

    // ── Tool Implementations ──────────────────────────────────────────────────

    fn tool_get_scene_state(&self) -> Result<String> {
        let scene  = self.scene.lock().unwrap();
        let summary = scene.summarize();
        Ok(serde_json::to_string_pretty(&summary)?)
    }

    fn tool_create_geometry(&self, args: &Value) -> Result<String> {
        let mut scene = self.scene.lock().unwrap();
        let shape  = args["shape"].as_str().unwrap_or("box");
        let name   = args["name"].as_str().map(|s| s.to_string());
        let origin = parse_vec3(args.get("origin"), [0.0, 0.0, 0.0]);

        match shape {
            "box" => {
                let w = args["width"].as_f64().ok_or_else(|| anyhow::anyhow!("width required for box"))?;
                let h = args["height"].as_f64().ok_or_else(|| anyhow::anyhow!("height required for box"))?;
                let d = args["depth"].as_f64().ok_or_else(|| anyhow::anyhow!("depth required for box"))?;
                let id = scene.create_box(name, origin, w, h, d)?;
                Ok(format!("✅ Box created | ID: {id} | Size: {w}×{h}×{d}mm | Use '{id}.face.top' for push/pull"))
            }
            "cylinder" => {
                let r = args["radius"].as_f64().ok_or_else(|| anyhow::anyhow!("radius required for cylinder"))?;
                let h = args["height"].as_f64().ok_or_else(|| anyhow::anyhow!("height required for cylinder"))?;
                let s = args["segments"].as_u64().unwrap_or(32) as u32;
                let id = scene.create_cylinder(name, origin, r, h, s)?;
                Ok(format!("✅ Cylinder created | ID: {id} | r={r}mm h={h}mm"))
            }
            "sphere" => {
                let r = args["radius"].as_f64().ok_or_else(|| anyhow::anyhow!("radius required for sphere"))?;
                let s = args["segments"].as_u64().unwrap_or(32) as u32;
                let id = scene.create_sphere(name, origin, r, s)?;
                Ok(format!("✅ Sphere created | ID: {id} | r={r}mm"))
            }
            other => Err(anyhow::anyhow!("Unknown shape: '{}'", other))
        }
    }

    fn tool_push_pull(&self, args: &Value) -> Result<String> {
        let mut scene = self.scene.lock().unwrap();
        let face = args["face"].as_str()
            .ok_or_else(|| anyhow::anyhow!("'face' required, e.g. 'abc123.face.top'"))?;
        let dist = args["distance"].as_f64()
            .ok_or_else(|| anyhow::anyhow!("'distance' required (mm)"))?;
        let id = scene.push_pull(face, dist)?;
        Ok(format!("✅ Push/Pull applied | Object: {id} | Face: {face} | Distance: {dist}mm"))
    }

    fn tool_set_material(&self, args: &Value) -> Result<String> {
        let mut scene = self.scene.lock().unwrap();
        let obj_id   = args["obj_id"].as_str().ok_or_else(|| anyhow::anyhow!("obj_id required"))?;
        let material = args["material"].as_str().ok_or_else(|| anyhow::anyhow!("material required"))?;
        scene.set_material(obj_id, material)?;
        Ok(format!("✅ Material '{material}' applied to {obj_id}"))
    }

    fn tool_move_object(&self, args: &Value) -> Result<String> {
        let mut scene = self.scene.lock().unwrap();
        let obj_id = args["obj_id"].as_str().ok_or_else(|| anyhow::anyhow!("obj_id required"))?;
        let delta  = parse_vec3(args.get("delta"), [0.0, 0.0, 0.0]);
        scene.move_object(obj_id, delta)?;
        Ok(format!("✅ Moved {obj_id} by [{:.1}, {:.1}, {:.1}]mm", delta[0], delta[1], delta[2]))
    }

    fn tool_execute_batch(&self, args: &Value) -> Result<String> {
        let ops_raw = args["operations"].clone();
        let ops: Vec<CadOperation> = serde_json::from_value(ops_raw)
            .map_err(|e| anyhow::anyhow!("Failed to parse operations: {}", e))?;

        let count = ops.len();
        let mut scene = self.scene.lock().unwrap();
        let results = scene.execute_batch(ops);

        let ok_count  = results.iter().filter(|r| r.success).count();
        let err_count = results.iter().filter(|r| !r.success).count();

        let mut out = format!("✅ Batch complete: {ok_count}/{count} succeeded");
        if err_count > 0 {
            out.push_str(&format!(" ({err_count} errors)"));
            for r in results.iter().filter(|r| !r.success) {
                out.push_str(&format!("\n  ❌ Op[{}]: {}", r.op_index, r.message));
            }
        }
        // List created IDs
        let created: Vec<String> = results.iter()
            .filter_map(|r| r.obj_id.as_ref().map(|id| id.clone()))
            .filter(|id| !id.is_empty())
            .collect();
        if !created.is_empty() {
            out.push_str(&format!("\n  Created IDs: {}", created.join(", ")));
        }
        Ok(out)
    }

    fn tool_move_safe(&self, args: &Value) -> Result<String> {
        let mut scene = self.scene.lock().unwrap();
        let obj_id = args["obj_id"].as_str()
            .ok_or_else(|| anyhow::anyhow!("obj_id required"))?;
        let delta = parse_vec3(args.get("delta"), [0.0, 0.0, 0.0]);

        let original_pos = {
            let obj = scene.objects.get(obj_id)
                .ok_or_else(|| anyhow::anyhow!("Object '{}' not found", obj_id))?;
            obj.position
        };

        // Apply move
        {
            let obj = scene.objects.get_mut(obj_id).unwrap();
            for i in 0..3 { obj.position[i] += delta[i]; }
        }
        if let Some(obj) = scene.objects.get(obj_id) {
            scene.collision.update_object(obj);
        }

        // Check collisions at new position
        let collisions = scene.collision.check_object(obj_id);

        if collisions.is_empty() {
            let new_pos = scene.objects.get(obj_id).unwrap().position;
            scene.bump_version();
            Ok(format!(
                "✅ 移動成功 → [{:.1}, {:.1}, {:.1}]mm",
                new_pos[0], new_pos[1], new_pos[2]
            ))
        } else {
            // Revert move
            {
                let obj = scene.objects.get_mut(obj_id).unwrap();
                obj.position = original_pos;
            }
            if let Some(obj) = scene.objects.get(obj_id) {
                scene.collision.update_object(obj);
            }
            let blocked_by: Vec<String> = collisions.iter().map(|c| {
                if c.obj_a == obj_id { c.obj_b.clone() } else { c.obj_a.clone() }
            }).collect();
            Ok(format!(
                "⚠️  移動被阻擋（碰到 {}），已還原位置 [{:.1}, {:.1}, {:.1}]mm",
                blocked_by.join(", "),
                original_pos[0], original_pos[1], original_pos[2]
            ))
        }
    }

    fn tool_check_placement(&self, args: &Value) -> Result<String> {
        let mut scene = self.scene.lock().unwrap();
        let obj_id = args["obj_id"].as_str()
            .ok_or_else(|| anyhow::anyhow!("obj_id required"))?;
        let position = parse_vec3(args.get("position"), [0.0, 0.0, 0.0]);

        let original_pos = {
            let obj = scene.objects.get(obj_id)
                .ok_or_else(|| anyhow::anyhow!("Object '{}' not found", obj_id))?;
            obj.position
        };

        // Temporarily move to target position
        {
            let obj = scene.objects.get_mut(obj_id).unwrap();
            obj.position = position;
        }
        if let Some(obj) = scene.objects.get(obj_id) {
            scene.collision.update_object(obj);
        }

        let collisions = scene.collision.check_object(obj_id);

        // Revert to original position
        {
            let obj = scene.objects.get_mut(obj_id).unwrap();
            obj.position = original_pos;
        }
        if let Some(obj) = scene.objects.get(obj_id) {
            scene.collision.update_object(obj);
        }

        if collisions.is_empty() {
            Ok(format!(
                "✅ 位置 [{:.0},{:.0},{:.0}] 可以放置，無碰撞",
                position[0], position[1], position[2]
            ))
        } else {
            let names: Vec<String> = collisions.iter().map(|c| {
                if c.obj_a == obj_id { c.obj_b.clone() } else { c.obj_a.clone() }
            }).collect();
            Ok(format!(
                "❌ 位置 [{:.0},{:.0},{:.0}] 會碰到：{}",
                position[0], position[1], position[2],
                names.join(", ")
            ))
        }
    }

    fn tool_measure_distance(&self, args: &Value) -> Result<String> {
        let scene = self.scene.lock().unwrap();
        let id_a = args["obj_id_a"].as_str()
            .ok_or_else(|| anyhow::anyhow!("obj_id_a required"))?;
        let id_b = args["obj_id_b"].as_str()
            .ok_or_else(|| anyhow::anyhow!("obj_id_b required"))?;

        let obj_a = scene.objects.get(id_a)
            .ok_or_else(|| anyhow::anyhow!("Object '{}' not found", id_a))?;
        let obj_b = scene.objects.get(id_b)
            .ok_or_else(|| anyhow::anyhow!("Object '{}' not found", id_b))?;

        let aabb_a = Aabb::from_object(obj_a);
        let aabb_b = Aabb::from_object(obj_b);

        if aabb_a.overlaps(&aabb_b) {
            let ox = (aabb_a.max[0].min(aabb_b.max[0]) - aabb_a.min[0].max(aabb_b.min[0])).max(0.0);
            let oy = (aabb_a.max[1].min(aabb_b.max[1]) - aabb_a.min[1].max(aabb_b.min[1])).max(0.0);
            let oz = (aabb_a.max[2].min(aabb_b.max[2]) - aabb_a.min[2].max(aabb_b.min[2])).max(0.0);
            let depth = ox.min(oy).min(oz);
            Ok(format!("⚠️  穿插 {:.2}mm\n物件 {} ↔ {}", depth, id_a, id_b))
        } else {
            let dist = aabb_a.distance_to(&aabb_b);
            if dist < 1.0 {
                Ok(format!("✅ 剛好接觸\n物件 {} ↔ {}", id_a, id_b))
            } else {
                Ok(format!("📏 間距 {:.2}mm\n物件 {} ↔ {}", dist, id_a, id_b))
            }
        }
    }

    fn tool_delete_object(&self, args: &Value) -> Result<String> {
        let mut scene = self.scene.lock().unwrap();
        let obj_id = args["obj_id"].as_str().ok_or_else(|| anyhow::anyhow!("obj_id required"))?;
        scene.collision.remove_object(obj_id);
        scene.delete_object(obj_id)?;
        Ok(format!("✅ Deleted object {obj_id}"))
    }

    fn tool_clear_scene(&self) -> Result<String> {
        let mut scene = self.scene.lock().unwrap();
        scene.clear();
        Ok("✅ Scene cleared".into())
    }

    fn tool_check_collisions(&self, args: &Value) -> Result<String> {
        let scene = self.scene.lock().unwrap();
        let pairs = if let Some(id) = args["obj_id"].as_str() {
            scene.collision.check_object(id)
        } else {
            scene.collision.check_all()
        };
        if pairs.is_empty() {
            return Ok(format!("✅ 無碰撞，場景乾淨（共 {} 個物件）", scene.objects.len()));
        }
        let mut out = format!("⚠️ 發現 {} 組碰撞：\n", pairs.len());
        for p in &pairs {
            let sev = match p.severity {
                crate::scene::CollisionSeverity::Touch     => "輕微接觸",
                crate::scene::CollisionSeverity::Overlap   => "明顯重疊",
                crate::scene::CollisionSeverity::Contained => "完全包含",
            };
            out.push_str(&format!("  [{sev}] {} ↔ {} | 重疊: [{:.1},{:.1},{:.1}]mm\n",
                p.obj_a, p.obj_b, p.overlap_mm[0], p.overlap_mm[1], p.overlap_mm[2]));
        }
        Ok(out)
    }

    fn tool_calculate_weight(&self, args: &Value) -> Result<String> {
        let scene = self.scene.lock().unwrap();
        let obj_id = args["obj_id"].as_str();
        if let Some(id) = obj_id {
            let obj = scene.objects.get(id)
                .ok_or_else(|| anyhow::anyhow!("Object {id} not found"))?;
            let mat = MaterialLibrary::get(&obj.material.name);
            let vol = VolumeCalc::volume_mm3(&obj.shape);
            let kg  = VolumeCalc::weight_kg(&obj.shape, mat.physical.density);
            let kn  = VolumeCalc::weight_kn(&obj.shape, mat.physical.density);
            Ok(format!("⚖️ {}\n  體積：{:.0} mm³\n  密度：{:.0} kg/m³\n  重量：{:.2} kg ({:.4} kN)",
                obj.name, vol, mat.physical.density, kg, kn))
        } else {
            let mut total = 0.0f64;
            let mut lines = vec!["⚖️ 場景重量報告：".to_string()];
            for obj in scene.objects.values() {
                let mat = MaterialLibrary::get(&obj.material.name);
                let kg  = VolumeCalc::weight_kg(&obj.shape, mat.physical.density);
                total += kg;
                lines.push(format!("  {} → {:.2} kg", obj.name, kg));
            }
            lines.push(format!("  總計：{:.2} kg ({:.4} kN)", total, total*9.81/1000.0));
            Ok(lines.join("\n"))
        }
    }

    fn tool_get_material_info(&self, args: &Value) -> Result<String> {
        let name = args["material"].as_str()
            .ok_or_else(|| anyhow::anyhow!("material name required"))?;
        let mat = MaterialLibrary::get(name);
        let mut out = format!("📦 {}\n  密度：{} kg/m³\n", mat.name, mat.physical.density);
        if let Some(s) = &mat.structural {
            out.push_str(&format!(
                "  楊氏模數：{:.0} GPa | 降伏強度：{:.0} MPa | 抗拉：{:.0} MPa | 泊松比：{}\n",
                s.youngs_modulus/1e9, s.yield_strength/1e6, s.tensile_strength/1e6, s.poissons_ratio));
        }
        Ok(out)
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn parse_vec3(v: Option<&Value>, default: [f64; 3]) -> [f64; 3] {
    v.and_then(|v| v.as_array())
     .and_then(|arr| {
         if arr.len() >= 3 {
             Some([
                 arr[0].as_f64().unwrap_or(default[0]),
                 arr[1].as_f64().unwrap_or(default[1]),
                 arr[2].as_f64().unwrap_or(default[2]),
             ])
         } else { None }
     })
     .unwrap_or(default)
}

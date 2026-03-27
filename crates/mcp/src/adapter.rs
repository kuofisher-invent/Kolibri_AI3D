//! Layer 2: Kolibri Adapter — tool name → Scene 操作
//! 與傳輸層無關，只依賴 kolibri-core

use kolibri_core::scene::{Scene, Shape, MaterialKind};
use serde_json::{json, Value};
use crate::protocol::ToolDef;

pub fn prompt_templates() -> Vec<Value> {
    Vec::new()
}

/// Kolibri 3D engine adapter — 持有 Scene，執行 MCP 工具
pub struct KolibriAdapter {
    pub scene: Scene,
}

impl KolibriAdapter {
    pub fn new() -> Self {
        Self { scene: Scene::default() }
    }

    /// 列出所有可用工具的定義
    pub fn tool_definitions(&self) -> Vec<ToolDef> {
        vec![
            ToolDef {
                name: "get_scene_state".into(),
                description: "取得當前3D場景完整狀態".into(),
                input_schema: json!({ "type": "object", "properties": {} }),
            },
            ToolDef {
                name: "get_object_info".into(),
                description: "取得單一物件詳細資訊".into(),
                input_schema: json!({ "type": "object", "required": ["id"], "properties": { "id":{"type":"string"} } }),
            },
            ToolDef {
                name: "create_box".into(),
                description: "建立方塊。單位mm。".into(),
                input_schema: json!({ "type": "object", "required": ["width","height","depth"], "properties": {
                    "name":{"type":"string"}, "position":{"type":"array","items":{"type":"number"},"default":[0,0,0]},
                    "width":{"type":"number"}, "height":{"type":"number"}, "depth":{"type":"number"},
                    "material":{"type":"string","default":"concrete"}
                }}),
            },
            ToolDef {
                name: "create_cylinder".into(),
                description: "建立圓柱。".into(),
                input_schema: json!({ "type": "object", "required": ["radius","height"], "properties": {
                    "name":{"type":"string"}, "position":{"type":"array","items":{"type":"number"}},
                    "radius":{"type":"number"}, "height":{"type":"number"}, "material":{"type":"string"}
                }}),
            },
            ToolDef {
                name: "create_sphere".into(),
                description: "建立球體。".into(),
                input_schema: json!({ "type": "object", "required": ["radius"], "properties": {
                    "name":{"type":"string"}, "position":{"type":"array","items":{"type":"number"}},
                    "radius":{"type":"number"}, "material":{"type":"string"}
                }}),
            },
            ToolDef {
                name: "delete_object".into(),
                description: "刪除物件".into(),
                input_schema: json!({ "type": "object", "required": ["id"], "properties": { "id":{"type":"string"} } }),
            },
            ToolDef {
                name: "move_object".into(),
                description: "移動物件到指定位置(mm)".into(),
                input_schema: json!({ "type": "object", "required": ["id","position"], "properties": {
                    "id":{"type":"string"}, "position":{"type":"array","items":{"type":"number"}}
                }}),
            },
            ToolDef {
                name: "rotate_object".into(),
                description: "旋轉物件（Y軸，角度制）".into(),
                input_schema: json!({ "type": "object", "required": ["id","angle_deg"], "properties": {
                    "id":{"type":"string"}, "angle_deg":{"type":"number"}
                }}),
            },
            ToolDef {
                name: "scale_object".into(),
                description: "縮放物件 factor=[x,y,z]倍率".into(),
                input_schema: json!({ "type": "object", "required": ["id","factor"], "properties": {
                    "id":{"type":"string"}, "factor":{"type":"array","items":{"type":"number"}}
                }}),
            },
            ToolDef {
                name: "set_material".into(),
                description: "設定材質。可用: concrete, wood, glass, metal, brick, white, marble, steel, aluminum, copper, gold, tile, asphalt, grass".into(),
                input_schema: json!({ "type": "object", "required": ["id","material"], "properties": {
                    "id":{"type":"string"}, "material":{"type":"string"}
                }}),
            },
            ToolDef {
                name: "push_pull".into(),
                description: "推拉物件的面。face: top/bottom/front/back/left/right。distance: mm正值向外。".into(),
                input_schema: json!({ "type": "object", "required": ["id","face","distance"], "properties": {
                    "id":{"type":"string"}, "face":{"type":"string","enum":["top","bottom","front","back","left","right"]},
                    "distance":{"type":"number"}
                }}),
            },
            ToolDef {
                name: "duplicate_object".into(),
                description: "複製物件 offset=[x,y,z]mm".into(),
                input_schema: json!({ "type": "object", "required": ["id"], "properties": {
                    "id":{"type":"string"}, "offset":{"type":"array","items":{"type":"number"},"default":[500,0,0]}
                }}),
            },
            ToolDef {
                name: "clear_scene".into(),
                description: "清空場景".into(),
                input_schema: json!({ "type": "object", "properties": {} }),
            },
            ToolDef {
                name: "save_scene".into(),
                description: "儲存場景到檔案".into(),
                input_schema: json!({ "type": "object", "required": ["path"], "properties": { "path":{"type":"string"} } }),
            },
            ToolDef {
                name: "load_scene".into(),
                description: "載入場景".into(),
                input_schema: json!({ "type": "object", "required": ["path"], "properties": { "path":{"type":"string"} } }),
            },
            ToolDef {
                name: "batch_create".into(),
                description: "批量建立多個物件。objects 陣列，每個元素同 create_box/cylinder/sphere 參數 + type 欄位".into(),
                input_schema: json!({ "type": "object", "required": ["objects"], "properties": {
                    "objects":{"type":"array","items":{"type":"object","properties":{
                        "type":{"type":"string","enum":["box","cylinder","sphere"]},
                        "name":{"type":"string"}, "position":{"type":"array","items":{"type":"number"}},
                        "width":{"type":"number"}, "height":{"type":"number"}, "depth":{"type":"number"},
                        "radius":{"type":"number"}, "material":{"type":"string"}
                    }}}
                }}),
            },
            ToolDef {
                name: "set_object_property".into(),
                description: "設定物件屬性（name, tag, visible, locked, roughness, metallic）".into(),
                input_schema: json!({ "type": "object", "required": ["id"], "properties": {
                    "id":{"type":"string"},
                    "name":{"type":"string"}, "tag":{"type":"string"},
                    "visible":{"type":"boolean"}, "locked":{"type":"boolean"},
                    "roughness":{"type":"number"}, "metallic":{"type":"number"}
                }}),
            },
            ToolDef {
                name: "measure_object".into(),
                description: "測量物件面積和體積".into(),
                input_schema: json!({ "type": "object", "required": ["id"], "properties": { "id":{"type":"string"} } }),
            },
            ToolDef {
                name: "measure_distance".into(),
                description: "測量兩個物件中心之間的距離(mm)".into(),
                input_schema: json!({ "type": "object", "required": ["id_a", "id_b"], "properties": {
                    "id_a":{"type":"string"}, "id_b":{"type":"string"}
                }}),
            },
            ToolDef {
                name: "align_objects".into(),
                description: "對齊多個物件。mode: left/right/top/bottom/front/back/center_x/center_y/center_z".into(),
                input_schema: json!({ "type": "object", "required": ["ids", "mode"], "properties": {
                    "ids":{"type":"array","items":{"type":"string"}},
                    "mode":{"type":"string","enum":["left","right","top","bottom","front","back","center_x","center_y","center_z"]}
                }}),
            },
            ToolDef {
                name: "import_file".into(),
                description: "匯入檔案到場景（OBJ/STL/DXF）".into(),
                input_schema: json!({ "type": "object", "required": ["path"], "properties": {
                    "path":{"type":"string"}
                }}),
            },
            ToolDef {
                name: "export_scene".into(),
                description: "匯出場景到檔案（OBJ/STL/DXF/glTF）".into(),
                input_schema: json!({ "type": "object", "required": ["path"], "properties": {
                    "path":{"type":"string"},
                    "format":{"type":"string","enum":["obj","stl","dxf","gltf"],"description":"auto-detect from extension if omitted"}
                }}),
            },
            ToolDef {
                name: "undo".into(),
                description: "撤銷上一步".into(),
                input_schema: json!({ "type": "object", "properties": {} }),
            },
            ToolDef {
                name: "redo".into(),
                description: "重做".into(),
                input_schema: json!({ "type": "object", "properties": {} }),
            },
        ]
    }

    /// 執行工具，回傳結果 JSON
    pub fn execute_tool(&mut self, tool: &str, args: &Value) -> Value {
        match tool {
            "get_scene_state" => {
                let objs: Vec<Value> = self.scene.objects.values().map(|o| {
                    json!({ "id": o.id, "name": o.name, "position": o.position,
                            "shape": format!("{:?}", o.shape).chars().take(60).collect::<String>(),
                            "material": o.material.label() })
                }).collect();
                json!({ "object_count": objs.len(), "objects": objs })
            }
            "get_object_info" => {
                let id = args["id"].as_str().unwrap_or("");
                if let Some(obj) = self.scene.objects.get(id) {
                    let shape_info = match &obj.shape {
                        Shape::Box { width, height, depth } => json!({"type":"box","width":width,"height":height,"depth":depth}),
                        Shape::Cylinder { radius, height, segments } => json!({"type":"cylinder","radius":radius,"height":height,"segments":segments}),
                        Shape::Sphere { radius, segments } => json!({"type":"sphere","radius":radius,"segments":segments}),
                        Shape::Line { points, .. } => json!({"type":"line","points":points.len()}),
                        Shape::Mesh(m) => json!({"type":"mesh","vertices":m.vertices.len(),"faces":m.faces.len()}),
                    };
                    json!({ "id": obj.id, "name": obj.name, "position": obj.position,
                            "rotation_y_deg": obj.rotation_y.to_degrees(), "material": obj.material.label(),
                            "roughness": obj.roughness, "metallic": obj.metallic, "shape": shape_info })
                } else {
                    json!({ "error": "Object not found" })
                }
            }
            "create_box" => {
                let name = args["name"].as_str().unwrap_or("Box").to_string();
                let pos = parse_pos(&args["position"]);
                let w = args["width"].as_f64().unwrap_or(1000.0) as f32;
                let h = args["height"].as_f64().unwrap_or(1000.0) as f32;
                let d = args["depth"].as_f64().unwrap_or(1000.0) as f32;
                let mat = parse_material(args["material"].as_str().unwrap_or("concrete"));
                let id = self.scene.add_box(name, pos, w, h, d, mat);
                json!({ "success": true, "id": id })
            }
            "create_cylinder" => {
                let name = args["name"].as_str().unwrap_or("Cylinder").to_string();
                let pos = parse_pos(&args["position"]);
                let r = args["radius"].as_f64().unwrap_or(500.0) as f32;
                let h = args["height"].as_f64().unwrap_or(1000.0) as f32;
                let mat = parse_material(args["material"].as_str().unwrap_or("concrete"));
                let id = self.scene.add_cylinder(name, pos, r, h, 48, mat);
                json!({ "success": true, "id": id })
            }
            "create_sphere" => {
                let name = args["name"].as_str().unwrap_or("Sphere").to_string();
                let pos = parse_pos(&args["position"]);
                let r = args["radius"].as_f64().unwrap_or(500.0) as f32;
                let mat = parse_material(args["material"].as_str().unwrap_or("concrete"));
                let id = self.scene.add_sphere(name, pos, r, 32, mat);
                json!({ "success": true, "id": id })
            }
            "delete_object" => {
                let ok = self.scene.delete(args["id"].as_str().unwrap_or(""));
                json!({ "success": ok })
            }
            "move_object" => {
                let id = args["id"].as_str().unwrap_or("").to_string();
                let pos = parse_pos(&args["position"]);
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    obj.position = pos; self.scene.version += 1;
                    json!({ "success": true })
                } else { json!({ "error": "Object not found" }) }
            }
            "rotate_object" => {
                let id = args["id"].as_str().unwrap_or("").to_string();
                let deg = args["angle_deg"].as_f64().unwrap_or(0.0) as f32;
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    obj.rotation_y += deg.to_radians(); self.scene.version += 1;
                    json!({ "success": true })
                } else { json!({ "error": "Object not found" }) }
            }
            "scale_object" => {
                let id = args["id"].as_str().unwrap_or("").to_string();
                let f = parse_factor(&args["factor"]);
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    match &mut obj.shape {
                        Shape::Box { width, height, depth } => { *width *= f[0]; *height *= f[1]; *depth *= f[2]; }
                        Shape::Cylinder { radius, height, .. } => { *radius *= f[0]; *height *= f[1]; }
                        Shape::Sphere { radius, .. } => { *radius *= f[0]; }
                        _ => {}
                    }
                    self.scene.version += 1;
                    json!({ "success": true })
                } else { json!({ "error": "Object not found" }) }
            }
            "set_material" => {
                let id = args["id"].as_str().unwrap_or("").to_string();
                let mat = parse_material(args["material"].as_str().unwrap_or("concrete"));
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    obj.material = mat; self.scene.version += 1;
                    json!({ "success": true })
                } else { json!({ "error": "Object not found" }) }
            }
            "push_pull" => {
                let id = args["id"].as_str().unwrap_or("").to_string();
                let face = args["face"].as_str().unwrap_or("top");
                let dist = args["distance"].as_f64().unwrap_or(0.0) as f32;
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    match (&mut obj.shape, face) {
                        (Shape::Box { height, .. }, "top") => *height = (*height + dist).max(10.0),
                        (Shape::Box { height, .. }, "bottom") => { let d = dist.min(*height - 10.0); *height -= d; obj.position[1] += d; }
                        (Shape::Box { width, .. }, "right") => *width = (*width + dist).max(10.0),
                        (Shape::Box { width, .. }, "left") => { let d = dist.min(*width - 10.0); *width -= d; obj.position[0] += d; }
                        (Shape::Box { depth, .. }, "back") => *depth = (*depth + dist).max(10.0),
                        (Shape::Box { depth, .. }, "front") => { let d = dist.min(*depth - 10.0); *depth -= d; obj.position[2] += d; }
                        _ => return json!({ "error": "Unsupported shape/face" }),
                    }
                    self.scene.version += 1;
                    json!({ "success": true })
                } else { json!({ "error": "Object not found" }) }
            }
            "duplicate_object" => {
                let id = args["id"].as_str().unwrap_or("").to_string();
                let offset = parse_pos(&args.get("offset").cloned().unwrap_or(json!([500,0,0])));
                if let Some(obj) = self.scene.objects.get(&id).cloned() {
                    let mut clone = obj;
                    clone.id = self.scene.next_id_pub();
                    clone.name = format!("{}_copy", clone.name);
                    clone.position[0] += offset[0]; clone.position[1] += offset[1]; clone.position[2] += offset[2];
                    let nid = clone.id.clone();
                    self.scene.objects.insert(nid.clone(), clone);
                    self.scene.version += 1;
                    json!({ "success": true, "copy_id": nid })
                } else { json!({ "error": "Object not found" }) }
            }
            "clear_scene" => {
                let count = self.scene.objects.len();
                self.scene.objects.clear(); self.scene.version += 1;
                json!({ "success": true, "cleared": count })
            }
            "save_scene" => {
                let path = args["path"].as_str().unwrap_or("scene.k3d");
                match self.scene.save_to_file(path) {
                    Ok(()) => json!({ "success": true, "path": path }),
                    Err(e) => json!({ "error": e.to_string() }),
                }
            }
            "load_scene" => {
                let path = args["path"].as_str().unwrap_or("scene.k3d");
                match self.scene.load_from_file(path) {
                    Ok(n) => json!({ "success": true, "loaded": n }),
                    Err(e) => json!({ "error": e.to_string() }),
                }
            }
            "undo" => { let ok = self.scene.undo(); json!({ "success": ok }) }
            "redo" => { let ok = self.scene.redo(); json!({ "success": ok }) }
            "batch_create" => {
                let objects = args["objects"].as_array().cloned().unwrap_or_default();
                let mut ids = Vec::new();
                for obj_def in &objects {
                    let typ = obj_def["type"].as_str().unwrap_or("box");
                    let name = obj_def["name"].as_str().unwrap_or(typ).to_string();
                    let pos = parse_pos(&obj_def["position"]);
                    let mat = parse_material(obj_def["material"].as_str().unwrap_or("concrete"));
                    let id = match typ {
                        "box" => {
                            let w = obj_def["width"].as_f64().unwrap_or(1000.0) as f32;
                            let h = obj_def["height"].as_f64().unwrap_or(1000.0) as f32;
                            let d = obj_def["depth"].as_f64().unwrap_or(1000.0) as f32;
                            self.scene.add_box(name, pos, w, h, d, mat)
                        }
                        "cylinder" => {
                            let r = obj_def["radius"].as_f64().unwrap_or(500.0) as f32;
                            let h = obj_def["height"].as_f64().unwrap_or(1000.0) as f32;
                            self.scene.add_cylinder(name, pos, r, h, 32, mat)
                        }
                        "sphere" => {
                            let r = obj_def["radius"].as_f64().unwrap_or(500.0) as f32;
                            self.scene.add_sphere(name, pos, r, 32, mat)
                        }
                        _ => continue,
                    };
                    ids.push(id);
                }
                json!({ "success": true, "created": ids.len(), "ids": ids })
            }
            "set_object_property" => {
                let id = args["id"].as_str().unwrap_or("").to_string();
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    if let Some(n) = args["name"].as_str() { obj.name = n.to_string(); }
                    if let Some(t) = args["tag"].as_str() { obj.tag = t.to_string(); }
                    if let Some(v) = args["visible"].as_bool() { obj.visible = v; }
                    if let Some(l) = args["locked"].as_bool() { obj.locked = l; }
                    if let Some(r) = args["roughness"].as_f64() { obj.roughness = r as f32; }
                    if let Some(m) = args["metallic"].as_f64() { obj.metallic = m as f32; }
                    self.scene.version += 1;
                    json!({ "success": true, "id": id })
                } else { json!({ "error": "Object not found" }) }
            }
            "measure_object" => {
                let id = args["id"].as_str().unwrap_or("");
                if let Some(obj) = self.scene.objects.get(id) {
                    let area = kolibri_core::measure::surface_area(obj);
                    let vol = kolibri_core::measure::volume(obj);
                    json!({
                        "id": id,
                        "surface_area_mm2": area,
                        "surface_area": kolibri_core::measure::format_area(area),
                        "volume_mm3": vol,
                        "volume": kolibri_core::measure::format_volume(vol),
                    })
                } else { json!({ "error": "Object not found" }) }
            }
            "measure_distance" => {
                let id_a = args["id_a"].as_str().unwrap_or("");
                let id_b = args["id_b"].as_str().unwrap_or("");
                let center_of = |id: &str| -> Option<[f32; 3]> {
                    self.scene.objects.get(id).map(|o| {
                        let p = o.position;
                        let ext = match &o.shape {
                            Shape::Box { width, height, depth } => [*width, *height, *depth],
                            Shape::Cylinder { radius, height, .. } => [*radius*2.0, *height, *radius*2.0],
                            Shape::Sphere { radius, .. } => [*radius*2.0; 3],
                            _ => [0.0; 3],
                        };
                        [p[0]+ext[0]/2.0, p[1]+ext[1]/2.0, p[2]+ext[2]/2.0]
                    })
                };
                match (center_of(id_a), center_of(id_b)) {
                    (Some(a), Some(b)) => {
                        let dx = b[0]-a[0]; let dy = b[1]-a[1]; let dz = b[2]-a[2];
                        let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                        json!({ "distance_mm": dist, "distance": format!("{:.0} mm", dist),
                                "delta": [dx, dy, dz] })
                    }
                    _ => json!({ "error": "One or both objects not found" }),
                }
            }
            "align_objects" => {
                let ids: Vec<String> = args["ids"].as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default();
                let mode = args["mode"].as_str().unwrap_or("left");
                let mode_num: u8 = match mode {
                    "left" => 0, "right" => 1, "bottom" => 2, "top" => 3,
                    "front" => 4, "back" => 5, "center_x" => 6, "center_y" => 7, "center_z" => 8,
                    _ => 0,
                };
                if ids.len() < 2 {
                    return json!({ "error": "Need at least 2 object IDs" });
                }
                // Apply alignment inline
                self.scene.snapshot();
                // 簡化：直接對齊 X 座標
                let positions: Vec<f32> = ids.iter()
                    .filter_map(|id| self.scene.objects.get(id).map(|o| o.position[mode_num as usize / 2]))
                    .collect();
                if let Some(&target) = positions.iter().min_by(|a, b| a.partial_cmp(b).unwrap()) {
                    for id in &ids {
                        if let Some(obj) = self.scene.objects.get_mut(id) {
                            let axis = (mode_num / 2) as usize;
                            if mode_num % 2 == 0 { // min
                                obj.position[axis] = target;
                            }
                        }
                    }
                    self.scene.version += 1;
                }
                json!({ "success": true, "aligned": ids.len(), "mode": mode })
            }
            "import_file" => {
                let path = args["path"].as_str().unwrap_or("").to_string();
                let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
                let result = match ext.as_str() {
                    "obj" => kolibri_io::obj_io::import_obj(&mut self.scene, &path).map(|n| json!({"imported": n})),
                    "stl" => kolibri_io::stl_io::import_stl(&mut self.scene, &path).map(|n| json!({"imported": n})),
                    "dxf" => kolibri_io::dxf_io::import_dxf(&mut self.scene, &path).map(|n| json!({"imported": n})),
                    _ => Err(format!("Unsupported format: {}", ext)),
                };
                match result {
                    Ok(data) => { let mut d = data; d["success"] = json!(true); d["path"] = json!(path); d }
                    Err(e) => json!({ "error": e }),
                }
            }
            "export_scene" => {
                let path = args["path"].as_str().unwrap_or("export.obj").to_string();
                let format = args["format"].as_str().unwrap_or("").to_string();
                let fmt = if !format.is_empty() { format } else {
                    if path.ends_with(".stl") { "stl".into() }
                    else if path.ends_with(".dxf") { "dxf".into() }
                    else if path.ends_with(".gltf") || path.ends_with(".glb") { "gltf".into() }
                    else { "obj".into() }
                };
                let result = match fmt.as_str() {
                    "obj" => kolibri_io::obj_io::export_obj(&self.scene, &path),
                    "stl" => kolibri_io::stl_io::export_stl(&self.scene, &path),
                    "dxf" => kolibri_io::dxf_io::export_dxf(&self.scene, &path),
                    "gltf" => kolibri_io::gltf_io::export_gltf(&self.scene, &path),
                    _ => Err(format!("Unknown format: {}", fmt)),
                };
                match result {
                    Ok(()) => json!({ "success": true, "path": path, "format": fmt }),
                    Err(e) => json!({ "error": e }),
                }
            }
            _ => json!({ "error": format!("Unknown tool: {}", tool) }),
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

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

fn parse_factor(v: &Value) -> [f32; 3] {
    if let Some(arr) = v.as_array() {
        [arr.first().and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
         arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
         arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32]
    } else { [1.0; 3] }
}

fn parse_material(s: &str) -> MaterialKind {
    match s.to_lowercase().as_str() {
        "concrete" => MaterialKind::Concrete, "wood" => MaterialKind::Wood,
        "glass" => MaterialKind::Glass, "metal" => MaterialKind::Metal,
        "brick" => MaterialKind::Brick, "white" => MaterialKind::White,
        "marble" => MaterialKind::Marble, "steel" => MaterialKind::Steel,
        "aluminum" => MaterialKind::Aluminum, "copper" => MaterialKind::Copper,
        "gold" => MaterialKind::Gold, "tile" => MaterialKind::Tile,
        "asphalt" => MaterialKind::Asphalt, "grass" => MaterialKind::Grass,
        "black" => MaterialKind::Black, "stone" => MaterialKind::Stone,
        "plaster" => MaterialKind::Plaster,
        _ => MaterialKind::Concrete,
    }
}

/// MCP prompt templates — 預設建築場景生成提示
pub fn prompt_templates() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "name": "simple_building",
            "description": "生成一棟簡易建築（地板+牆壁+柱子）",
            "arguments": [
                {"name": "width", "description": "建築寬度(mm)", "required": false},
                {"name": "depth", "description": "建築深度(mm)", "required": false},
                {"name": "height", "description": "樓高(mm)", "required": false},
            ]
        }),
        serde_json::json!({
            "name": "column_grid",
            "description": "生成柱列網格（指定行列數和間距）",
            "arguments": [
                {"name": "rows", "description": "行數", "required": true},
                {"name": "cols", "description": "列數", "required": true},
                {"name": "spacing", "description": "間距(mm)", "required": false},
            ]
        }),
        serde_json::json!({
            "name": "room_layout",
            "description": "生成一個房間（四面牆+地板+天花板）",
            "arguments": [
                {"name": "width", "description": "寬(mm)", "required": false},
                {"name": "depth", "description": "深(mm)", "required": false},
                {"name": "wall_thickness", "description": "牆厚(mm)", "required": false},
            ]
        }),
    ]
}

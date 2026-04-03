use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use eframe::{egui, wgpu};
use eframe::epaint::mutex::RwLock;
use serde::Serialize;

use crate::camera::{self, OrbitCamera};
use crate::renderer::ViewportRenderer;
use crate::scene::{MaterialKind, Scene, Shape};
use crate::app::{KolibriApp, Tool, WorkMode, DrawState, ScaleHandle, PullFace, SnapType, SnapResult, AiSuggestion, SuggestionAction, RightTab, CursorHint, EditorState, SelectionMode, RenderMode, ViewerState, BackgroundTaskResult, BackgroundSceneBuild, SpatialEntry, parse_material_name};

impl KolibriApp {
    pub(crate) fn handle_mcp_command(&mut self, cmd: crate::mcp_server::McpCommand) -> crate::mcp_server::McpResult {
        use crate::mcp_server::{McpCommand, McpResult};
        use serde_json::json;

        let actor = crate::ai_log::ActorId::claude();

        match cmd {
            McpCommand::GetSceneState => {
                let objects: Vec<serde_json::Value> = self.scene.objects.values().map(|obj| {
                    json!({
                        "id": obj.id,
                        "name": obj.name,
                        "position": obj.position,
                        "shape": format!("{:?}", obj.shape),
                        "material": format!("{:?}", obj.material),
                    })
                }).collect();
                McpResult { success: true, data: json!({ "objects": objects, "count": objects.len() }) }
            }
            McpCommand::CreateBox { name, position, width, height, depth, material } => {
                self.scene.snapshot();
                let mat = parse_material_name(&material);
                let n = if name.is_empty() { self.next_name("Box") } else { name };
                let id = self.scene.add_box(n, position, width, height, depth, mat);
                self.ai_log.log(&actor, "\u{5efa}\u{7acb}\u{65b9}\u{584a}", &format!("{:.0}\u{00d7}{:.0}\u{00d7}{:.0}", width, height, depth), vec![id.clone()]);
                McpResult { success: true, data: json!({ "id": id }) }
            }
            McpCommand::CreateCylinder { name, position, radius, height, material } => {
                self.scene.snapshot();
                let mat = parse_material_name(&material);
                let n = if name.is_empty() { self.next_name("Cylinder") } else { name };
                let id = self.scene.add_cylinder(n, position, radius, height, 48, mat);
                self.ai_log.log(&actor, "\u{5efa}\u{7acb}\u{5713}\u{67f1}", &format!("r={:.0} h={:.0}", radius, height), vec![id.clone()]);
                McpResult { success: true, data: json!({ "id": id }) }
            }
            McpCommand::CreateSphere { name, position, radius, material } => {
                self.scene.snapshot();
                let mat = parse_material_name(&material);
                let n = if name.is_empty() { self.next_name("Sphere") } else { name };
                let id = self.scene.add_sphere(n, position, radius, 32, mat);
                self.ai_log.log(&actor, "\u{5efa}\u{7acb}\u{7403}\u{9ad4}", &format!("r={:.0}", radius), vec![id.clone()]);
                McpResult { success: true, data: json!({ "id": id }) }
            }
            McpCommand::DeleteObject { id } => {
                // 查物件是否屬於群組 → 整組刪除
                let parent_group = self.scene.objects.get(&id)
                    .and_then(|o| o.parent_id.clone());
                if let Some(gid) = parent_group {
                    self.scene.delete_group(&gid);
                    self.ai_log.log(&actor, "\u{522a}\u{9664}\u{7fa4}\u{7d44}", &gid, vec![gid.clone()]);
                    McpResult { success: true, data: json!({ "deleted_group": gid }) }
                } else if self.scene.delete_group(&id) {
                    self.ai_log.log(&actor, "\u{522a}\u{9664}\u{7fa4}\u{7d44}", &id, vec![id.clone()]);
                    McpResult { success: true, data: json!({ "deleted_group": id }) }
                } else {
                    self.scene.delete(&id);
                    self.ai_log.log(&actor, "\u{522a}\u{9664}\u{7269}\u{4ef6}", &id, vec![id.clone()]);
                    McpResult { success: true, data: json!({ "deleted": id }) }
                }
            }
            McpCommand::MoveObject { id, position } => {
                self.scene.snapshot();
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    obj.position = position;
                    obj.obj_version += 1;
                    self.scene.version += 1;
                    self.ai_log.log(&actor, "\u{79fb}\u{52d5}\u{7269}\u{4ef6}", &format!("{:?}", position), vec![id.clone()]);
                    McpResult { success: true, data: json!({ "moved": id }) }
                } else {
                    McpResult { success: false, data: json!({ "error": "Object not found" }) }
                }
            }
            McpCommand::SetMaterial { id, material } => {
                self.scene.snapshot();
                let mat = parse_material_name(&material);
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    obj.material = mat;
                    self.scene.version += 1;
                    self.ai_log.log(&actor, "\u{8a2d}\u{5b9a}\u{6750}\u{8cea}", &material, vec![id.clone()]);
                    McpResult { success: true, data: json!({ "updated": id }) }
                } else {
                    McpResult { success: false, data: json!({ "error": "Object not found" }) }
                }
            }
            McpCommand::ClearScene => {
                self.scene.snapshot();
                let count = self.scene.objects.len();
                self.scene.objects.clear();
                self.import_object_debug.clear();
                self.scene.version += 1;
                self.editor.selected_ids.clear();
                self.ai_log.log(&actor, "清空場景", &format!("{} objects removed", count), vec![]);
                McpResult { success: true, data: json!({ "cleared": count }) }
            }
            McpCommand::RotateObject { id, angle_deg } => {
                self.scene.snapshot_ids(&[&id], "MCP旋轉");
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    obj.rotation_y += angle_deg.to_radians();
                    obj.rotation_xyz[1] = obj.rotation_y;
                    let qy = glam::Quat::from_rotation_y(obj.rotation_y);
                    obj.rotation_quat = qy.to_array();
                    obj.obj_version += 1;
                    self.scene.version += 1;
                    self.ai_log.log(&actor, "旋轉物件", &format!("{} {:.0}°", id, angle_deg), vec![id.clone()]);
                    McpResult { success: true, data: json!({ "rotated": id, "angle_deg": angle_deg }) }
                } else {
                    McpResult { success: false, data: json!({ "error": "Object not found" }) }
                }
            }
            McpCommand::ScaleObject { id, factor } => {
                self.scene.snapshot_ids(&[&id], "MCP縮放");
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    match &mut obj.shape {
                        Shape::Box { width, height, depth } => {
                            *width *= factor[0]; *height *= factor[1]; *depth *= factor[2];
                        }
                        Shape::Cylinder { radius, height, .. } => {
                            *radius *= factor[0]; *height *= factor[1];
                        }
                        Shape::Sphere { radius, .. } => {
                            *radius *= factor[0];
                        }
                        _ => {}
                    }
                    obj.obj_version += 1;
                    self.scene.version += 1;
                    self.ai_log.log(&actor, "縮放物件", &format!("{} x[{:.2},{:.2},{:.2}]", id, factor[0], factor[1], factor[2]), vec![id.clone()]);
                    McpResult { success: true, data: json!({ "scaled": id }) }
                } else {
                    McpResult { success: false, data: json!({ "error": "Object not found" }) }
                }
            }
            McpCommand::DuplicateObject { id, offset } => {
                if let Some(obj) = self.scene.objects.get(&id).cloned() {
                    self.scene.snapshot();
                    let mut clone = obj;
                    clone.id = self.scene.next_id_pub();
                    clone.name = format!("{}_copy", clone.name);
                    clone.position[0] += offset[0];
                    clone.position[1] += offset[1];
                    clone.position[2] += offset[2];
                    let new_id = clone.id.clone();
                    self.scene.objects.insert(new_id.clone(), clone);
                    self.scene.version += 1;
                    self.ai_log.log(&actor, "複製物件", &format!("{} → {}", id, new_id), vec![new_id.clone()]);
                    McpResult { success: true, data: json!({ "original": id, "copy_id": new_id }) }
                } else {
                    McpResult { success: false, data: json!({ "error": "Object not found" }) }
                }
            }
            McpCommand::GetObjectInfo { id } => {
                if let Some(obj) = self.scene.objects.get(&id) {
                    let shape_info = match &obj.shape {
                        Shape::Box { width, height, depth } => json!({"type":"box","width":width,"height":height,"depth":depth}),
                        Shape::Cylinder { radius, height, segments } => json!({"type":"cylinder","radius":radius,"height":height,"segments":segments}),
                        Shape::Sphere { radius, segments } => json!({"type":"sphere","radius":radius,"segments":segments}),
                        Shape::Line { points, thickness, .. } => json!({"type":"line","point_count":points.len(),"thickness":thickness}),
                        Shape::Mesh(mesh) => json!({"type":"mesh","vertices":mesh.vertices.len(),"faces":mesh.faces.len(),"edges":mesh.edges.len()}),
                    };
                    McpResult { success: true, data: json!({
                        "id": obj.id, "name": obj.name, "position": obj.position,
                        "rotation_y_deg": obj.rotation_y.to_degrees(),
                        "material": obj.material.label(),
                        "tag": obj.tag, "visible": obj.visible,
                        "roughness": obj.roughness, "metallic": obj.metallic,
                        "shape": shape_info,
                    }) }
                } else {
                    McpResult { success: false, data: json!({ "error": "Object not found" }) }
                }
            }
            McpCommand::Undo => {
                let ok = self.scene.undo();
                McpResult { success: ok, data: json!({ "undo": ok, "undo_count": self.scene.undo_count() }) }
            }
            McpCommand::Redo => {
                let ok = self.scene.redo();
                McpResult { success: ok, data: json!({ "redo": ok, "redo_count": self.scene.redo_count() }) }
            }
            McpCommand::Shutdown => {
                self.ai_log.log(&actor, "關閉應用", "MCP shutdown", vec![]);
                // 關閉前自動儲存 trace
                if self.editor.debug_trace_active {
                    self.stop_debug_trace();
                } else {
                    self.flush_debug_trace();
                }
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    std::process::exit(0);
                });
                McpResult { success: true, data: json!({ "message": "Shutting down..." }) }
            }
            // ── 鋼構 MCP 命令 ──
            #[cfg(feature = "steel")]
            McpCommand::CreateSteelColumn { position, profile, height } => {
                self.scene.snapshot();
                let (h_sec, b_sec, tw, tf) = crate::tools::geometry_ops::parse_h_profile(&profile);
                let name_base = self.next_name("COL");
                let cx = position[0];
                let cz = position[2];
                let base_y = position[1];

                let f1_id = self.scene.insert_box_raw(
                    format!("{}_F1", name_base),
                    [cx - b_sec / 2.0, base_y, cz - h_sec / 2.0],
                    b_sec, height, tf, crate::scene::MaterialKind::Steel,
                );
                let f2_id = self.scene.insert_box_raw(
                    format!("{}_F2", name_base),
                    [cx - b_sec / 2.0, base_y, cz + h_sec / 2.0 - tf],
                    b_sec, height, tf, crate::scene::MaterialKind::Steel,
                );
                let web_id = self.scene.insert_box_raw(
                    format!("{}_W", name_base),
                    [cx - tw / 2.0, base_y, cz - h_sec / 2.0 + tf],
                    tw, height, h_sec - 2.0 * tf, crate::scene::MaterialKind::Steel,
                );
                for id in [&f1_id, &f2_id, &web_id] {
                    if let Some(obj) = self.scene.objects.get_mut(id) {
                        obj.component_kind = crate::collision::ComponentKind::Column;
                    }
                }
                let child_ids = vec![f1_id.clone(), f2_id.clone(), web_id.clone()];
                self.scene.create_group(name_base.clone(), child_ids);
                self.scene.version += 1;
                McpResult { success: true, data: json!({ "column": name_base, "profile": profile, "height": height }) }
            }
            #[cfg(feature = "steel")]
            McpCommand::CreateSteelBeam { p1, p2, profile } => {
                self.scene.snapshot();
                let (h_sec, b_sec, tw, tf) = crate::tools::geometry_ops::parse_h_profile(&profile);
                let name_base = self.next_name("BM");
                let dx = p2[0] - p1[0];
                let dz = p2[2] - p1[2];
                let length = (dx * dx + dz * dz).sqrt();
                let beam_y = p1[1]; // 梁底 Y
                let is_x_dir = dx.abs() > dz.abs();

                let ids = if is_x_dir {
                    let min_x = p1[0].min(p2[0]);
                    let cz = p1[2];
                    let f1 = self.scene.insert_box_raw(
                        format!("{}_TF", name_base),
                        [min_x, beam_y + h_sec - tf, cz - b_sec / 2.0],
                        length, tf, b_sec, crate::scene::MaterialKind::Steel,
                    );
                    let f2 = self.scene.insert_box_raw(
                        format!("{}_BF", name_base),
                        [min_x, beam_y, cz - b_sec / 2.0],
                        length, tf, b_sec, crate::scene::MaterialKind::Steel,
                    );
                    let w = self.scene.insert_box_raw(
                        format!("{}_W", name_base),
                        [min_x, beam_y + tf, cz - tw / 2.0],
                        length, h_sec - 2.0 * tf, tw, crate::scene::MaterialKind::Steel,
                    );
                    vec![f1, f2, w]
                } else {
                    let min_z = p1[2].min(p2[2]);
                    let cx = p1[0];
                    let f1 = self.scene.insert_box_raw(
                        format!("{}_TF", name_base),
                        [cx - b_sec / 2.0, beam_y + h_sec - tf, min_z],
                        b_sec, tf, length, crate::scene::MaterialKind::Steel,
                    );
                    let f2 = self.scene.insert_box_raw(
                        format!("{}_BF", name_base),
                        [cx - b_sec / 2.0, beam_y, min_z],
                        b_sec, tf, length, crate::scene::MaterialKind::Steel,
                    );
                    let w = self.scene.insert_box_raw(
                        format!("{}_W", name_base),
                        [cx - tw / 2.0, beam_y + tf, min_z],
                        tw, h_sec - 2.0 * tf, length, crate::scene::MaterialKind::Steel,
                    );
                    vec![f1, f2, w]
                };
                for id in &ids {
                    if let Some(obj) = self.scene.objects.get_mut(id) {
                        obj.component_kind = crate::collision::ComponentKind::Beam;
                    }
                }
                self.scene.create_group(name_base.clone(), ids);
                self.scene.version += 1;
                McpResult { success: true, data: json!({ "beam": name_base, "profile": profile, "length": length }) }
            }
            #[cfg(feature = "steel")]
            McpCommand::CreateSteelConnection { member_ids, conn_type } => {
                // 選取指定構件後建立接頭
                self.editor.selected_ids = member_ids.clone();
                self.expand_selection_to_groups();
                match conn_type.as_str() {
                    "end_plate" | "endplate" => {
                        self.create_end_plate_connection();
                    }
                    "shear_tab" | "sheartab" => {
                        self.create_shear_tab_connection();
                    }
                    "base_plate" | "baseplate" => {
                        self.create_base_plate_connection();
                    }
                    "web_doubler" | "doubler" => {
                        self.create_web_doubler_connection();
                    }
                    "double_angle" | "framed" => {
                        self.create_double_angle_connection();
                    }
                    _ => {
                        self.create_end_plate_connection(); // 預設端板
                    }
                }
                let count = self.scene.objects.len();
                McpResult { success: true, data: json!({ "connection": conn_type, "total_objects": count }) }
            }

            // ── Debug Trace 遠端控制 ──
            McpCommand::StartTrace { interval_ms } => {
                self.editor.debug_trace_interval_ms = interval_ms.max(10).min(1500);
                self.start_debug_trace();
                McpResult { success: true, data: json!({ "started": true, "interval_ms": self.editor.debug_trace_interval_ms }) }
            }
            McpCommand::StopTrace => {
                let count = self.editor.debug_trace_records.len();
                self.stop_debug_trace();
                McpResult { success: true, data: json!({ "stopped": true, "records": count, "path": self.editor.debug_trace_path }) }
            }
            McpCommand::GetTraceStatus => {
                McpResult { success: true, data: json!({
                    "active": self.editor.debug_trace_active,
                    "records": self.editor.debug_trace_records.len(),
                    "interval_ms": self.editor.debug_trace_interval_ms,
                    "path": self.editor.debug_trace_path,
                }) }
            }

            McpCommand::ImportFile { path } => {
                let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
                // 根據目前模式決定 DXF/DWG 匯入路徑
                let is_2d_mode = {
                    #[cfg(feature = "drafting")]
                    { self.viewer.layout_mode }
                    #[cfg(not(feature = "drafting"))]
                    { false }
                };
                let result = match ext.as_str() {
                    "obj" => crate::obj_io::import_obj(&mut self.scene, &path).map(|n| json!({"imported": n, "mode": "3d"})),
                    "stl" => crate::stl_io::import_stl(&mut self.scene, &path).map(|n| json!({"imported": n, "mode": "3d"})),
                    "dxf" | "dwg" if is_2d_mode => {
                        #[cfg(feature = "drafting")]
                        {
                            self.import_cad_to_2d_tab(&path)
                                .map(|n| json!({"imported": n, "mode": "2d"}))
                        }
                        #[cfg(not(feature = "drafting"))]
                        { Err("drafting feature not enabled".to_string()) }
                    }
                    "dxf" | "dwg" => {
                        // 非 2D 模式：DWG/DXF 匯入到 2D 畫布（自動切換）
                        #[cfg(feature = "drafting")]
                        {
                            self.import_cad_to_2d_tab(&path)
                                .map(|n| json!({"imported": n, "mode": "2d", "auto_switch": true}))
                        }
                        #[cfg(not(feature = "drafting"))]
                        {
                            crate::dxf_io::import_dxf(&mut self.scene, &path).map(|n| json!({"imported": n, "mode": "3d"}))
                        }
                    }
                    "skp" => {
                        // SKP SDK 匯入（子進程隔離，避免 DLL 崩潰影響主 APP）
                        if kolibri_skp::sdk_available() {
                            match kolibri_skp::import_skp_subprocess(&path) {
                                Ok(skp_scene) => {
                                    let ir = crate::import::skp_sdk_import::skp_scene_to_ir(&skp_scene, &path);
                                    let stats = ir.stats.clone();
                                    // 建立 mesh_id → mesh 查詢表
                                    let build = crate::import::import_manager::build_scene_from_ir(&mut self.scene, &ir);
                                    self.import_object_debug = build.object_debug.clone();
                                    Self::write_import_source_debug(&self.import_object_debug);
                                    // 匯入後自動 zoom extents
                                    self.zoom_extents();
                                    Ok(json!({
                                        "imported": stats.instance_count,
                                        "meshes": stats.mesh_count,
                                        "groups": stats.group_count,
                                        "components": stats.component_count,
                                        "materials": stats.material_count,
                                        "built_meshes": build.meshes,
                                        "source_debug_path": "logs/import_source_debug.json",
                                    }))
                                    /*
                                    // 按 instance 匯入（帶 transform）
                                    for inst in &skp_scene.instances {
                                        let mesh = match mesh_map.get(inst.mesh_id.as_str()) {
                                            Some(m) => m,
                                            None => continue,
                                        };
                                        if mesh.vertices.len() < 3 || mesh.indices.len() < 3 { continue; }
                                        // 套用 instance transform 到頂點
                                        let m = inst.transform;
                                        let transform_pt = |v: [f32; 3]| -> [f32; 3] {
                                            [m[0]*v[0] + m[4]*v[1] + m[8]*v[2] + m[12],
                                             m[1]*v[0] + m[5]*v[1] + m[9]*v[2] + m[13],
                                             m[2]*v[0] + m[6]*v[1] + m[10]*v[2] + m[14]]
                                        };
                                        let mut he = crate::halfedge::HeMesh::new();
                                        let xf_verts: Vec<[f32; 3]> = mesh.vertices.iter().map(|v| transform_pt(*v)).collect();
                                        for v in &xf_verts { he.add_vertex(*v); }
                                        for tri in mesh.indices.chunks(3) {
                                            if tri.len() < 3 { continue; }
                                            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
                                            if i0 >= xf_verts.len() || i1 >= xf_verts.len() || i2 >= xf_verts.len() { continue; }
                                            let v0 = xf_verts[i0]; let v1 = xf_verts[i1]; let v2 = xf_verts[i2];
                                            let a = [v1[0]-v0[0], v1[1]-v0[1], v1[2]-v0[2]];
                                            let b = [v2[0]-v0[0], v2[1]-v0[1], v2[2]-v0[2]];
                                            let nx = a[1]*b[2]-a[2]*b[1]; let ny = a[2]*b[0]-a[0]*b[2]; let nz = a[0]*b[1]-a[1]*b[0];
                                            let len = (nx*nx+ny*ny+nz*nz).sqrt().max(1e-10);
                                            let fid = he.next_fid();
                                            let vid0 = i0 as u32 + 1; let vid1 = i1 as u32 + 1; let vid2 = i2 as u32 + 1;
                                            he.faces.insert(fid, crate::halfedge::HeFace { edge: 0, normal: [nx/len, ny/len, nz/len], vert_ids: Some(vec![vid0, vid1, vid2]), source_face_label: None });
                                        }
                                        // SDK 原始邊線（也套用 instance transform）
                                        he.sdk_edge_segments = mesh.edges.iter().map(|(p1, p2)| {
                                            (transform_pt(*p1), transform_pt(*p2))
                                        }).collect();
                                        // 底部對齊地面
                                        let min_y = xf_verts.iter().map(|v| v[1]).fold(f32::MAX, f32::min);
                                        self.scene.insert_mesh_raw(inst.name.clone(), [0.0, -min_y, 0.0], he, MaterialKind::White);
                                        imported += 1;
                                    }
                                    self.scene.version += 1;
                                    Ok(json!({"imported": imported, "meshes": mesh_count, "groups": group_count, "components": comp_count}))
                                    */
                                }
                                Err(e) => {
                                    tracing::warn!("SKP SDK subprocess failed: {}, trying unified import", e);
                                    // Fallback: 走統一匯入管線（bridge/heuristic）
                                    match crate::import::import_manager::import_file(&path) {
                                        Ok(ir) => {
                                            let build = crate::import::import_manager::build_scene_from_ir(&mut self.scene, &ir);
                                            Ok(json!({
                                                "imported": build.meshes,
                                                "fallback": true,
                                                "sdk_error": format!("{}", e),
                                            }))
                                        }
                                        Err(e2) => Err(format!("SKP SDK: {} | Fallback: {}", e, e2)),
                                    }
                                }
                            }
                        } else {
                            // 沒有 SDK DLL，直接走統一匯入
                            match crate::import::import_manager::import_file(&path) {
                                Ok(ir) => {
                                    let build = crate::import::import_manager::build_scene_from_ir(&mut self.scene, &ir);
                                    Ok(json!({
                                        "imported": build.meshes,
                                        "fallback": true,
                                    }))
                                }
                                Err(e) => Err(format!("SKP import: {}", e)),
                            }
                        }
                    }
                    _ => Err(format!("Unsupported: {}", ext)),
                };
                match result {
                    Ok(data) => McpResult { success: true, data },
                    Err(e) => McpResult { success: false, data: json!({"error": e}) },
                }
            }
            McpCommand::Screenshot { path } => {
                self.viewport.save_screenshot(&self.device, &self.queue, &path);
                let exists = std::path::Path::new(&path).exists();
                McpResult {
                    success: exists,
                    data: json!({"path": path, "saved": exists}),
                }
            }
            McpCommand::ExportScene { path } => {
                // 匯出完整場景為 JSON
                let mut objects = Vec::new();
                for obj in self.scene.objects.values() {
                    let shape_info = match &obj.shape {
                        Shape::Mesh(m) => json!({
                            "type": "Mesh",
                            "vertices": m.vertices.len(),
                            "faces": m.faces.len(),
                        }),
                        Shape::Box { width, height, depth } => json!({
                            "type": "Box", "width": width, "height": height, "depth": depth
                        }),
                        Shape::Cylinder { radius, height, .. } => json!({
                            "type": "Cylinder", "radius": radius, "height": height
                        }),
                        Shape::Sphere { radius, .. } => json!({
                            "type": "Sphere", "radius": radius
                        }),
                        Shape::Line { .. } => json!({"type": "Line"}),
                    };
                    objects.push(json!({
                        "id": obj.id,
                        "name": obj.name,
                        "position": obj.position,
                        "material": format!("{:?}", obj.material),
                        "shape": shape_info,
                        "visible": obj.visible,
                        "rotation_y": obj.rotation_y,
                    }));
                }
                let export = json!({
                    "object_count": self.scene.objects.len(),
                    "objects": objects,
                    "groups": self.scene.groups.len(),
                    "version": self.scene.version,
                });
                match std::fs::write(&path, serde_json::to_string_pretty(&export).unwrap_or_default()) {
                    Ok(_) => McpResult { success: true, data: json!({"path": path, "objects": self.scene.objects.len()}) },
                    Err(e) => McpResult { success: false, data: json!({"error": format!("{}", e)}) },
                }
            }
            McpCommand::SetLayoutMode { enabled } => {
                if enabled { self.enter_layout_mode(); } else { self.exit_layout_mode(); }
                McpResult { success: true, data: json!({"layout_mode": enabled}) }
            }
            // ── 2D Drafting ──
            #[cfg(feature = "drafting")]
            McpCommand::DraftAddLine { p1, p2 } => {
                if !self.viewer.layout_mode { self.enter_layout_mode(); }
                let id = self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Line { start: p1, end: p2 });
                McpResult { success: true, data: json!({ "id": id }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftAddCircle { center, radius } => {
                if !self.viewer.layout_mode { self.enter_layout_mode(); }
                let id = self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Circle { center, radius });
                McpResult { success: true, data: json!({ "id": id }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftAddArc { center, radius, start_angle, end_angle } => {
                if !self.viewer.layout_mode { self.enter_layout_mode(); }
                let id = self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Arc { center, radius, start_angle, end_angle });
                McpResult { success: true, data: json!({ "id": id }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftAddRectangle { p1, p2 } => {
                if !self.viewer.layout_mode { self.enter_layout_mode(); }
                let id = self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Rectangle { p1, p2 });
                McpResult { success: true, data: json!({ "id": id }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftAddPolyline { points, closed } => {
                if !self.viewer.layout_mode { self.enter_layout_mode(); }
                let id = self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Polyline { points, closed });
                McpResult { success: true, data: json!({ "id": id }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftAddText { position, content, height } => {
                if !self.viewer.layout_mode { self.enter_layout_mode(); }
                let id = self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Text { position, content, height, rotation: 0.0 });
                McpResult { success: true, data: json!({ "id": id }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftAddDimLinear { p1, p2, offset } => {
                if !self.viewer.layout_mode { self.enter_layout_mode(); }
                let id = self.editor.draft_doc.add(kolibri_drafting::DraftEntity::DimLinear { p1, p2, offset, text_override: None });
                McpResult { success: true, data: json!({ "id": id }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftDelete { id } => {
                let ok = self.editor.draft_doc.remove(id);
                self.editor.draft_selected.retain(|&s| s != id);
                McpResult { success: ok, data: json!({ "deleted": ok }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftClear => {
                let count = self.editor.draft_doc.objects.len();
                self.editor.draft_doc = kolibri_drafting::DraftDocument::new();
                self.editor.draft_selected.clear();
                McpResult { success: true, data: json!({ "cleared": count }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftList => {
                let entities: Vec<serde_json::Value> = self.editor.draft_doc.objects.iter().map(|obj| {
                    let etype = match &obj.entity {
                        kolibri_drafting::DraftEntity::Line { .. } => "line",
                        kolibri_drafting::DraftEntity::Circle { .. } => "circle",
                        kolibri_drafting::DraftEntity::Arc { .. } => "arc",
                        kolibri_drafting::DraftEntity::Rectangle { .. } => "rectangle",
                        kolibri_drafting::DraftEntity::Polyline { .. } => "polyline",
                        kolibri_drafting::DraftEntity::Text { .. } => "text",
                        kolibri_drafting::DraftEntity::DimLinear { .. } => "dim_linear",
                        kolibri_drafting::DraftEntity::DimAligned { .. } => "dim_aligned",
                        _ => "other",
                    };
                    json!({ "id": obj.id, "type": etype, "layer": obj.layer, "visible": obj.visible })
                }).collect();
                McpResult { success: true, data: json!({ "count": entities.len(), "entities": entities }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftGetEntity { id } => {
                if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                    let data = serde_json::to_value(obj).unwrap_or(json!({}));
                    McpResult { success: true, data }
                } else {
                    McpResult { success: false, data: json!({ "error": "Entity not found" }) }
                }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftSetTool { tool } => {
                if !self.viewer.layout_mode { self.enter_layout_mode(); }
                let t = match tool.as_str() {
                    "select" => Tool::DraftSelect, "line" => Tool::DraftLine,
                    "arc" => Tool::DraftArc, "circle" => Tool::DraftCircle,
                    "rectangle" => Tool::DraftRectangle, "polyline" => Tool::DraftPolyline,
                    "text" => Tool::DraftText, "move" => Tool::DraftMove,
                    "rotate" => Tool::DraftRotate, "mirror" => Tool::DraftMirror,
                    "trim" => Tool::DraftTrim, "offset" => Tool::DraftOffset,
                    "dim_linear" => Tool::DraftDimLinear, "dim_aligned" => Tool::DraftDimAligned,
                    "erase" => Tool::DraftErase, "copy" => Tool::DraftCopy,
                    "zoom_all" => Tool::DraftZoomAll, "pan" => Tool::DraftPan,
                    "zoom_window" => Tool::DraftZoomWindow,
                    _ => Tool::DraftSelect,
                };
                self.editor.tool = t;
                McpResult { success: true, data: json!({ "tool": tool }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftSelect { ids } => {
                self.editor.draft_selected = ids.clone();
                McpResult { success: true, data: json!({ "selected": ids.len() }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftSetZoom { zoom, offset_x, offset_y } => {
                if !self.viewer.layout_mode { self.enter_layout_mode(); }
                self.editor.draft_zoom = zoom;
                self.editor.draft_offset = egui::vec2(offset_x, offset_y);
                self.console_push("INFO", format!("MCP Zoom: {:.2}x offset=({:.0},{:.0})", zoom, offset_x, offset_y));
                McpResult { success: true, data: json!({ "zoom": zoom, "offset_x": offset_x, "offset_y": offset_y }) }
            }
            #[cfg(feature = "drafting")]
            McpCommand::DraftImportFile { path } => {
                match self.import_cad_to_2d_tab(&path) {
                    Ok(count) => {
                        McpResult { success: true, data: json!({ "imported": count, "path": path }) }
                    }
                    Err(e) => {
                        self.console_push("ERROR", format!("[2D] 匯入失敗: {}", e));
                        McpResult { success: false, data: json!({ "error": e }) }
                    }
                }
            }
        }
    }
}

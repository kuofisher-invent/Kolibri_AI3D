use eframe::egui;

use crate::app::{
    compute_arc, DrawState, KolibriApp, PullFace, RenderMode, RightTab, ScaleHandle, SelectionMode, Tool,
};
use crate::camera;
use crate::scene::{MaterialKind, Shape};

impl KolibriApp {
    /// Execute a menu action without unsaved-changes check (used after confirmation).
    pub(crate) fn force_menu_action(&mut self, action: crate::menu::MenuAction) {
        use crate::menu::MenuAction;
        match action {
            MenuAction::NewScene => {
                self.scene.clear();
                self.editor.selected_ids.clear();
                self.editor.draw_state = DrawState::Idle;
                self.current_file = None;
                self.last_saved_version = self.scene.version;
                self.file_message = Some(("新建場景".to_string(), std::time::Instant::now()));
            }
            MenuAction::OpenScene => self.open_scene(),
            MenuAction::Revert => {
                if let Some(ref path) = self.current_file.clone() {
                    match self.scene.load_from_file(path) {
                        Ok(count) => {
                            self.editor.selected_ids.clear();
                            self.last_saved_version = self.scene.version;
                            self.file_message = Some((format!("已回復: {} 個物件", count), std::time::Instant::now()));
                        }
                        Err(e) => self.file_message = Some((format!("回復失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            _ => self.handle_menu_action(action),
        }
    }

    pub(crate) fn handle_menu_action(&mut self, action: crate::menu::MenuAction) {
        use crate::menu::MenuAction;
        match action {
            MenuAction::None => {}
            MenuAction::NewScene | MenuAction::Revert => {
                if self.has_unsaved_changes() {
                    self.pending_action = Some(action);
                } else {
                    self.force_menu_action(action);
                }
            }
            MenuAction::OpenScene => {
                if self.has_unsaved_changes() {
                    self.pending_action = Some(action);
                } else {
                    self.open_scene();
                }
            }
            MenuAction::OpenRecent(ref path) => {
                let path = path.clone();
                if path.ends_with(".obj") {
                    match crate::obj_io::import_obj(&mut self.scene, &path) {
                        Ok(count) => {
                            self.editor.selected_ids.clear();
                            self.file_message = Some((format!("已匯入 {} 個物件", count), std::time::Instant::now()));
                        }
                        Err(e) => self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now())),
                    }
                } else {
                    match self.scene.load_from_file(&path) {
                        Ok(count) => {
                            self.current_file = Some(path.clone());
                            self.add_recent_file(&path);
                            self.editor.selected_ids.clear();
                            self.last_saved_version = self.scene.version;
                            self.file_message = Some((format!("已載入 {} 個物件", count), std::time::Instant::now()));
                        }
                        Err(e) => self.file_message = Some((format!("載入失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::SaveScene => self.save_scene(),
            MenuAction::SaveAs => {
                self.current_file = None; // force dialog
                self.save_scene();
            }
            MenuAction::Undo => { self.scene.undo(); }
            MenuAction::Redo => { self.scene.redo(); }
            MenuAction::Delete => {
                for id in self.editor.selected_ids.drain(..).collect::<Vec<_>>() {
                    self.scene.delete(&id);
                }
            }
            MenuAction::SelectAll => {
                self.editor.selected_ids = self.scene.objects.keys().cloned().collect();
            }
            MenuAction::ViewFront => self.viewer.animate_camera_to(|c| c.set_front()),
            MenuAction::ViewBack => self.viewer.animate_camera_to(|c| c.set_back()),
            MenuAction::ViewLeft => self.viewer.animate_camera_to(|c| c.set_left()),
            MenuAction::ViewRight => self.viewer.animate_camera_to(|c| c.set_right()),
            MenuAction::ViewTop => self.viewer.animate_camera_to(|c| c.set_top()),
            MenuAction::ViewBottom => self.viewer.animate_camera_to(|c| c.set_bottom()),
            MenuAction::ViewIso => self.viewer.animate_camera_to(|c| c.set_iso()),
            MenuAction::ZoomExtents => self.zoom_extents(),
            MenuAction::Duplicate => {
                let mut new_ids = Vec::new();
                for id in &self.editor.selected_ids.clone() {
                    if let Some(obj) = self.scene.objects.get(id) {
                        let mut clone = obj.clone();
                        clone.id = self.scene.next_id_pub();
                        clone.name = format!("{}_copy", clone.name);
                        clone.position[0] += 500.0;
                        let new_id = clone.id.clone();
                        self.scene.objects.insert(new_id.clone(), clone);
                        new_ids.push(new_id);
                    }
                }
                if !new_ids.is_empty() {
                    self.scene.version += 1;
                    self.editor.selected_ids = new_ids;
                }
            }
            MenuAction::GroupSelected => {
                if self.editor.selected_ids.len() >= 2 {
                    self.scene.snapshot();
                    let name = format!("Group_{}", self.scene.groups.len() + 1);
                    let gid = self.scene.create_group(name, self.editor.selected_ids.clone());
                    self.file_message = Some((format!("已建立群組: {}", gid), std::time::Instant::now()));
                } else {
                    self.file_message = Some(("需要選取至少2個物件".to_string(), std::time::Instant::now()));
                }
            }
            MenuAction::ComponentSelected => {
                for id in &self.editor.selected_ids {
                    if let Some(obj) = self.scene.objects.get_mut(id) {
                        if !obj.name.contains("[元件]") {
                            obj.name = format!("[元件] {}", obj.name);
                        }
                    }
                }
            }
            MenuAction::Properties => {
                self.right_tab = RightTab::Properties;
            }
            MenuAction::ExportObj => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 OBJ")
                    .add_filter("OBJ 模型", &["obj"])
                    .set_file_name("export.obj")
                    .save_file();
                if let Some(path) = file {
                    let path_str = path.to_string_lossy().to_string();
                    match crate::obj_io::export_obj(&self.scene, &path_str) {
                        Ok(()) => self.file_message = Some((format!("已匯出: {}", path_str), std::time::Instant::now())),
                        Err(e) => self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ImportObj => {
                self.open_scene(); // reuse open_scene which now handles OBJ too
            }
            MenuAction::CsgUnion | MenuAction::CsgSubtract | MenuAction::CsgIntersect => {
                let op = match action {
                    MenuAction::CsgUnion => crate::csg::CsgOp::Union,
                    MenuAction::CsgSubtract => crate::csg::CsgOp::Subtract,
                    MenuAction::CsgIntersect => crate::csg::CsgOp::Intersect,
                    _ => unreachable!(),
                };

                if self.editor.selected_ids.len() >= 2 {
                    let id_a = self.editor.selected_ids[0].clone();
                    let id_b = self.editor.selected_ids[1].clone();

                    if let (Some(a), Some(b)) = (
                        self.scene.objects.get(&id_a).cloned(),
                        self.scene.objects.get(&id_b).cloned(),
                    ) {
                        if matches!(a.shape, Shape::Box{..}) && matches!(b.shape, Shape::Box{..}) {
                            self.scene.snapshot();
                            self.scene.objects.remove(&id_a);
                            self.scene.objects.remove(&id_b);

                            let results = crate::csg::box_csg(&a, &b, op);
                            let mut new_ids = Vec::new();
                            for obj in results {
                                let id = obj.id.clone();
                                self.scene.objects.insert(id.clone(), obj);
                                new_ids.push(id);
                            }
                            self.scene.version += 1;
                            self.editor.selected_ids = new_ids.clone();

                            let op_name = match op {
                                crate::csg::CsgOp::Union => "聯集",
                                crate::csg::CsgOp::Subtract => "差集",
                                crate::csg::CsgOp::Intersect => "交集",
                            };
                            self.file_message = Some((format!("布林{}: 產生 {} 個物件", op_name, new_ids.len()), std::time::Instant::now()));
                        } else {
                            self.file_message = Some(("布林運算僅支援方塊物件".to_string(), std::time::Instant::now()));
                        }
                    }
                } else {
                    self.file_message = Some(("請先選取兩個方塊物件".to_string(), std::time::Instant::now()));
                }
            }
            MenuAction::SetRenderMode(mode) => {
                self.viewer.render_mode = match mode {
                    0 => RenderMode::Shaded,
                    1 => RenderMode::Wireframe,
                    2 => RenderMode::XRay,
                    3 => RenderMode::HiddenLine,
                    5 => RenderMode::Sketch,
                    _ => RenderMode::Monochrome,
                };
            }
            MenuAction::ToggleBackground => {
                if self.viewer.sky_color[0] > 0.5 {
                    // Switch to dark
                    self.viewer.sky_color = [0.12, 0.12, 0.15];
                    self.viewer.ground_color = [0.2, 0.2, 0.22];
                } else {
                    // Switch to light
                    self.viewer.sky_color = [0.53, 0.72, 0.9];
                    self.viewer.ground_color = [0.65, 0.63, 0.60];
                }
            }
            MenuAction::SaveTemplate => {
                let file = rfd::FileDialog::new()
                    .set_title("存為範本")
                    .add_filter("Kolibri 範本", &["k3d"])
                    .set_directory("D:\\AI_Design\\Kolibri_Ai3D\\app\\templates")
                    .set_file_name("template.k3d")
                    .save_file();
                if let Some(path) = file {
                    let p = path.to_string_lossy().to_string();
                    match self.scene.save_to_file(&p) {
                        Ok(()) => self.file_message = Some((format!("範本已儲存: {}", p), std::time::Instant::now())),
                        Err(e) => self.file_message = Some((format!("儲存失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ExportPng => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 PNG 截圖")
                    .add_filter("PNG 圖片", &["png"])
                    .set_file_name("screenshot.png")
                    .save_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    self.viewport.save_screenshot(&self.device, &self.queue, &ps);
                    self.file_message = Some((format!("已匯出 PNG: {}", ps), std::time::Instant::now()));
                }
            }
            MenuAction::ExportJpg => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 JPG 截圖")
                    .add_filter("JPG 圖片", &["jpg", "jpeg"])
                    .set_file_name("screenshot.jpg")
                    .save_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    if let Some((w, h, rgb)) = self.viewport.capture_rgb(&self.device, &self.queue) {
                        if let Some(img) = image::RgbImage::from_raw(w, h, rgb) {
                            match img.save(&ps) {
                                Ok(_) => self.file_message = Some((format!("已匯出 JPG: {}", ps), std::time::Instant::now())),
                                Err(e) => self.file_message = Some((format!("JPG 匯出失敗: {}", e), std::time::Instant::now())),
                            }
                        }
                    }
                }
            }
            MenuAction::ExportPdf => {
                self.file_message = Some(("PDF 匯出功能開發中，請先使用 PNG 匯出".to_string(), std::time::Instant::now()));
            }
            MenuAction::ImportImage => {
                self.file_message = Some(("圖片參考底圖功能開發中".to_string(), std::time::Instant::now()));
            }
            MenuAction::ExportStl => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 STL")
                    .add_filter("STL 模型", &["stl"])
                    .set_file_name("export.stl")
                    .save_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    match crate::stl_io::export_stl(&self.scene, &ps) {
                        Ok(()) => self.file_message = Some((format!("已匯出 STL: {}", ps), std::time::Instant::now())),
                        Err(e) => self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ImportStl => {
                let file = rfd::FileDialog::new()
                    .set_title("匯入 STL")
                    .add_filter("STL 模型", &["stl"])
                    .pick_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    match crate::stl_io::import_stl(&mut self.scene, &ps) {
                        Ok(count) => {
                            self.editor.selected_ids.clear();
                            self.file_message = Some((format!("已匯入 {} 個物件: {}", count, ps), std::time::Instant::now()));
                        }
                        Err(e) => self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ExportGltf => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 GLTF")
                    .add_filter("GLTF 模型", &["gltf"])
                    .set_file_name("export.gltf")
                    .save_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    match crate::gltf_io::export_gltf(&self.scene, &ps) {
                        Ok(()) => self.file_message = Some((format!("已匯出 GLTF: {}", ps), std::time::Instant::now())),
                        Err(e) => self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ImportGltf => {
                self.file_message = Some(("GLTF 匯入尚未支援".to_string(), std::time::Instant::now()));
            }
            MenuAction::ExportDxf => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 DXF")
                    .add_filter("DXF 圖面", &["dxf", "DXF"])
                    .set_file_name("export.dxf")
                    .save_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    match crate::dxf_io::export_dxf(&self.scene, &ps) {
                        Ok(()) => self.file_message = Some((format!("已匯出 DXF: {}", ps), std::time::Instant::now())),
                        Err(e) => self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ImportDxf => {
                let file = rfd::FileDialog::new()
                    .set_title("匯入 DXF")
                    .add_filter("DXF 圖面", &["dxf", "DXF"])
                    .pick_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    match crate::dxf_io::import_dxf(&mut self.scene, &ps) {
                        Ok(count) => {
                            self.editor.selected_ids.clear();
                            self.file_message = Some((format!("已匯入 {} 個物件: {}", count, ps), std::time::Instant::now()));
                        }
                        Err(e) => self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ImportDxfSmart => {
                let file = rfd::FileDialog::new()
                    .set_title("智慧匯入 (DXF/DWG/PDF)")
                    .add_filter("CAD 圖面", &["dxf", "DXF", "dwg", "DWG", "pdf", "PDF"])
                    .add_filter("所有檔案", &["*"])
                    .pick_file();
                if let Some(path) = file {
                    let ps = path.to_string_lossy().to_string();
                    let ext = ps.rsplit('.').next().unwrap_or("").to_lowercase();
                    self.console_push("INFO", format!("[CAD] 開始解析: {} ({})", ps, ext));

                    if ext == "dxf" {
                        // DXF: full entity parsing
                        match crate::cad_import::import_dxf_to_ir(&ps) {
                            Ok(ir) => {
                                // Push full debug report to console
                                for line in &ir.debug_report {
                                    self.console_push("INFO", line.clone());
                                }
                                self.viewer.show_console = true;
                                self.console_push("INFO", format!("[DXF] Grids: X={} Y={} | Columns: {} | Beams: {} | Levels: {}",
                                    ir.grids.x_grids.len(), ir.grids.y_grids.len(),
                                    ir.columns.len(), ir.beams.len(), ir.levels.len()));
                                // Show review panel for user confirmation instead of auto-building
                                let entity_count = ir.columns.len() + ir.beams.len() + ir.base_plates.len();
                                let debug = ir.debug_report.clone();
                                self.import_review = Some(crate::import_review::ImportReview::from_drawing_ir(
                                    &ir, &ps, entity_count, debug,
                                ));
                                self.file_message = Some(("解析完成 — 請確認偵測結果".into(), std::time::Instant::now()));
                            }
                            Err(e) => {
                                self.console_push("ERROR", format!("[DXF] Parse failed: {}", e));
                                self.file_message = Some((format!("Parse failed: {}", e), std::time::Instant::now()));
                            }
                        }
                    } else {
                        // DWG/PDF: use smart import pipeline (unified IR)
                        self.console_push("INFO", format!("[CAD] 使用統一匯入管線"));
                        match crate::import::import_manager::import_file(&ps) {
                            Ok(ir) => {
                                for line in &ir.debug_report {
                                    let level = if line.contains("❌") { "WARN" } else { "INFO" };
                                    self.console_push(level, line.clone());
                                }
                                self.viewer.show_console = true;
                                self.pending_unified_ir = Some(ir);
                            }
                            Err(e) => {
                                self.console_push("ERROR", format!("[CAD] 匯入失敗: {}", e));
                                self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now()));
                            }
                        }
                    }
                }
            }
            MenuAction::SmartImport => {
                let file = rfd::FileDialog::new()
                    .set_title("智慧匯入")
                    .add_filter("所有支援格式", &["dxf", "DXF", "dwg", "DWG", "skp", "SKP", "obj", "OBJ", "stl", "STL", "pdf", "PDF"])
                    .add_filter("DXF 圖面", &["dxf", "DXF"])
                    .add_filter("DWG 圖面", &["dwg"])
                    .add_filter("PDF 圖面", &["pdf"])
                    .add_filter("SketchUp 模型", &["skp"])
                    .add_filter("OBJ 模型", &["obj"])
                    .pick_file();
                if let Some(path) = file {
                    let ps = path.to_string_lossy().to_string();
                    self.console_push("INFO", format!("[Import] 開始匯入: {}", ps));
                    self.start_import_task(ps.clone());
                }
            }
            // ── 2D CAD DXF Import/Export ──
            #[cfg(feature = "drafting")]
            MenuAction::ImportDxfToDraft => {
                let file = rfd::FileDialog::new()
                    .set_title("匯入 DXF/DWG → 2D CAD")
                    .add_filter("CAD 圖面", &["dxf", "DXF", "dwg", "DWG"])
                    .add_filter("DXF", &["dxf", "DXF"])
                    .add_filter("DWG", &["dwg", "DWG"])
                    .pick_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    self.console_push("INFO", format!("[2D] 正在匯入: {}...", ps));
                    // 切換到 2D 模式
                    self.enter_layout_mode();
                    match crate::dxf_io::import_cad_to_draft(&mut self.editor.draft_doc, &ps) {
                        Ok(count) => {
                            self.console_push("ACTION", format!("[2D] 匯入完成: {} 個圖元", count));
                            self.file_message = Some((format!("已匯入 {} 個 2D 圖元", count), std::time::Instant::now()));
                        }
                        Err(e) => {
                            self.console_push("ERROR", format!("[2D] 匯入失敗: {}", e));
                            self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now()));
                        }
                    }
                }
            }
            #[cfg(feature = "drafting")]
            MenuAction::ExportDraftDxf => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 2D CAD → DXF")
                    .add_filter("DXF 圖面", &["dxf", "DXF"])
                    .set_file_name("drawing.dxf")
                    .save_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    match crate::dxf_io::export_draft_to_dxf(&self.editor.draft_doc, &ps) {
                        Ok(count) => {
                            self.console_push("ACTION", format!("[2D] 匯出 {} 個圖元: {}", count, ps));
                            self.file_message = Some((format!("已匯出 {} 個 2D 圖元到 DXF", count), std::time::Instant::now()));
                        }
                        Err(e) => {
                            self.console_push("ERROR", format!("[2D] 匯出失敗: {}", e));
                            self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now()));
                        }
                    }
                }
            }
            MenuAction::SplitObject => {
                if let Some(id) = self.editor.selected_ids.first().cloned() {
                    if let Some(obj) = self.scene.objects.get(&id) {
                        if let Shape::Box { width, height, depth } = &obj.shape {
                            let p = obj.position;
                            let (w, h, d) = (*width, *height, *depth);
                            // Split along the longest axis at midpoint
                            let (axis, split_pos) = if w >= h && w >= d {
                                (0u8, p[0] + w / 2.0)
                            } else if h >= d {
                                (1u8, p[1] + h / 2.0)
                            } else {
                                (2u8, p[2] + d / 2.0)
                            };
                            if let Some((a, b)) = self.scene.split_box(&id, axis, split_pos) {
                                self.editor.selected_ids = vec![a, b];
                                self.file_message = Some(("物件已分割".to_string(), std::time::Instant::now()));
                            }
                        }
                    }
                }
            }
            MenuAction::ReverseFace => {
                let mut count = 0usize;
                for id in &self.editor.selected_ids.clone() {
                    if let Some(obj) = self.scene.objects.get_mut(id) {
                        if let Shape::Mesh(ref mut mesh) = obj.shape {
                            for face in mesh.faces.values_mut() {
                                face.normal = [-face.normal[0], -face.normal[1], -face.normal[2]];
                            }
                            count += 1;
                        }
                    }
                }
                if count > 0 {
                    self.scene.version += 1;
                    self.file_message = Some((format!("已反轉 {} 個網格的面法線", count), std::time::Instant::now()));
                } else {
                    self.file_message = Some(("所選物件無可反轉的網格面".to_string(), std::time::Instant::now()));
                }
            }
            // Camera/view actions handled in app.rs update() before dispatch
            _ => {}
        }
    }
}

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
                let mut deleted_groups = std::collections::HashSet::new();
                for id in self.editor.selected_ids.drain(..).collect::<Vec<_>>() {
                    let parent_group = self.scene.objects.get(&id)
                        .and_then(|o| o.parent_id.clone());
                    if let Some(gid) = parent_group {
                        if deleted_groups.insert(gid.clone()) {
                            self.scene.delete_group(&gid);
                        }
                    } else if self.scene.delete_group(&id) {
                        deleted_groups.insert(id);
                    } else {
                        self.scene.delete(&id);
                    }
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
                    .set_title("匯入 DXF/DWG")
                    .add_filter("CAD 圖面", &["dxf", "DXF", "dwg", "DWG"])
                    .pick_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    // 儲存路徑，顯示選擇對話框讓使用者決定匯入到 2D 還是 3D
                    #[cfg(feature = "drafting")]
                    {
                        self.pending_import_path = Some(ps);
                        self.show_import_mode_dialog = true;
                    }
                    #[cfg(not(feature = "drafting"))]
                    {
                        match crate::dxf_io::import_dxf(&mut self.scene, &ps) {
                            Ok(count) => {
                                self.editor.selected_ids.clear();
                                self.file_message = Some((format!("已匯入 {} 個物件: {}", count, ps), std::time::Instant::now()));
                            }
                            Err(e) => self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now())),
                        }
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

                    // 2D 模式下：DXF/DWG 自動路由到 2D DraftDocument
                    let routed_to_2d = {
                        #[cfg(feature = "drafting")]
                        { self.viewer.layout_mode && matches!(ext.as_str(), "dxf" | "dwg") }
                        #[cfg(not(feature = "drafting"))]
                        { false }
                    };
                    if routed_to_2d {
                        #[cfg(feature = "drafting")]
                        {
                            match self.import_cad_to_2d_tab(&ps) {
                                Ok(count) => {
                                    self.file_message = Some((format!("已匯入 {} 個 2D 圖元", count), std::time::Instant::now()));
                                }
                                Err(e) => {
                                    self.console_push("ERROR", format!("[2D] 匯入失敗: {}", e));
                                    self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now()));
                                }
                            }
                        }
                    } else if ext == "dxf" {
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
                    let ext = ps.rsplit('.').next().unwrap_or("").to_lowercase();
                    // 2D 模式下：DXF/DWG 自動路由到 2D DraftDocument
                    let route_2d = {
                        #[cfg(feature = "drafting")]
                        { self.viewer.layout_mode && matches!(ext.as_str(), "dxf" | "dwg") }
                        #[cfg(not(feature = "drafting"))]
                        { false }
                    };
                    if route_2d {
                        #[cfg(feature = "drafting")]
                        {
                            match self.import_cad_to_2d_tab(&ps) {
                                Ok(count) => {
                                    self.file_message = Some((format!("已匯入 {} 個 2D 圖元", count), std::time::Instant::now()));
                                }
                                Err(e) => {
                                    self.console_push("ERROR", format!("[2D] 匯入失敗: {}", e));
                                    self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now()));
                                }
                            }
                        }
                    } else {
                        self.console_push("INFO", format!("[Import] 開始匯入: {}", ps));
                        self.start_import_task(ps.clone());
                    }
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
                    match self.import_cad_to_2d_tab(&ps) {
                        Ok(count) => {
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
            // ── DWG 匯出（3D Scene → DWG via DXF 中繼轉換）──
            MenuAction::ExportDwg => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 DWG")
                    .add_filter("DWG 圖面", &["dwg", "DWG"])
                    .set_file_name("export.dwg")
                    .save_file();
                if let Some(p) = file {
                    let dwg_path = p.to_string_lossy().to_string();
                    // 檢查可用工具
                    let tools = crate::dwg_parser::available_dwg_tools();
                    if tools.is_empty() {
                        // 沒有轉換工具，匯出 DXF 作為替代
                        let dxf_fallback = dwg_path.replace(".dwg", ".dxf").replace(".DWG", ".dxf");
                        match crate::dxf_io::export_dxf(&self.scene, &dxf_fallback) {
                            Ok(()) => {
                                self.console_push("WARN", "無 DWG 轉換工具（LibreDWG/ODA/ZWCAD），已改匯出 DXF".into());
                                self.file_message = Some((format!("已匯出 DXF（無 DWG 工具）: {}", dxf_fallback), std::time::Instant::now()));
                            }
                            Err(e) => self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now())),
                        }
                    } else {
                        let scene_ref = &self.scene;
                        match crate::dwg_parser::export_dwg_via_dxf(
                            |tmp_dxf| crate::dxf_io::export_dxf(scene_ref, tmp_dxf),
                            &dwg_path,
                        ) {
                            Ok(path) => {
                                self.console_push("ACTION", format!("DWG 匯出成功: {}", path));
                                self.file_message = Some((format!("已匯出 DWG: {}", path), std::time::Instant::now()));
                            }
                            Err(e) => {
                                self.console_push("ERROR", format!("DWG 匯出失敗: {}", e));
                                // 自動匯出 DXF 作為替代
                                let dxf_fallback = dwg_path.replace(".dwg", ".dxf").replace(".DWG", ".dxf");
                                if crate::dxf_io::export_dxf(&self.scene, &dxf_fallback).is_ok() {
                                    self.file_message = Some((format!("DWG 轉換失敗，已改匯出 DXF: {}", dxf_fallback), std::time::Instant::now()));
                                } else {
                                    self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now()));
                                }
                            }
                        }
                    }
                }
            }
            // ── DWG 匯出（2D DraftDocument → DWG via DXF 中繼轉換）──
            #[cfg(feature = "drafting")]
            MenuAction::ExportDraftDwg => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 2D CAD → DWG")
                    .add_filter("DWG 圖面", &["dwg", "DWG"])
                    .set_file_name("drawing.dwg")
                    .save_file();
                if let Some(p) = file {
                    let dwg_path = p.to_string_lossy().to_string();
                    let tools = crate::dwg_parser::available_dwg_tools();
                    if tools.is_empty() {
                        // 匯出 DXF 替代
                        let dxf_fallback = dwg_path.replace(".dwg", ".dxf").replace(".DWG", ".dxf");
                        match crate::dxf_io::export_draft_to_dxf(&self.editor.draft_doc, &dxf_fallback) {
                            Ok(count) => {
                                self.console_push("WARN", format!("無 DWG 轉換工具，已匯出 {} 個圖元到 DXF", count));
                                self.file_message = Some((format!("已匯出 DXF（無 DWG 工具）: {}", dxf_fallback), std::time::Instant::now()));
                            }
                            Err(e) => self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now())),
                        }
                    } else {
                        let doc_ref = &self.editor.draft_doc;
                        match crate::dwg_parser::export_dwg_via_dxf(
                            |tmp_dxf| crate::dxf_io::export_draft_to_dxf(doc_ref, tmp_dxf).map(|_| ()),
                            &dwg_path,
                        ) {
                            Ok(path) => {
                                self.console_push("ACTION", format!("[2D] DWG 匯出成功: {}", path));
                                self.file_message = Some((format!("已匯出 2D DWG: {}", path), std::time::Instant::now()));
                            }
                            Err(e) => {
                                self.console_push("ERROR", format!("[2D] DWG 匯出失敗: {}", e));
                                let dxf_fallback = dwg_path.replace(".dwg", ".dxf").replace(".DWG", ".dxf");
                                if let Ok(count) = crate::dxf_io::export_draft_to_dxf(&self.editor.draft_doc, &dxf_fallback) {
                                    self.file_message = Some((format!("DWG 轉換失敗，已匯出 {} 個圖元到 DXF: {}", count, dxf_fallback), std::time::Instant::now()));
                                } else {
                                    self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now()));
                                }
                            }
                        }
                    }
                }
            }
            // ── BOM 料表匯出 ──
            MenuAction::ExportBom => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 BOM 料表 (CSV)")
                    .add_filter("CSV", &["csv"])
                    .set_file_name("bom.csv")
                    .save_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    match self.export_bom_csv(&ps) {
                        Ok(count) => {
                            self.console_push("ACTION", format!("BOM 匯出: {} 個構件 → {}", count, ps));
                            self.file_message = Some((format!("已匯出 BOM: {} 個構件", count), std::time::Instant::now()));
                        }
                        Err(e) => {
                            self.console_push("ERROR", format!("BOM 匯出失敗: {}", e));
                            self.file_message = Some((format!("BOM 匯出失敗: {}", e), std::time::Instant::now()));
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

    /// 匯出 BOM 料表到 CSV
    pub(crate) fn export_bom_csv(&self, path: &str) -> Result<usize, String> {
        use std::io::Write;
        let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;

        // CSV BOM header (UTF-8 BOM for Excel compatibility)
        write!(file, "\u{FEFF}").map_err(|e| e.to_string())?;
        writeln!(file, "編號,類型,名稱,規格,長度(mm),寬度(mm),高度(mm),材料,標籤,數量,單重(kg/m),備註")
            .map_err(|e| e.to_string())?;

        let mut count = 0;
        // 統計相同規格的構件數量
        let mut bom_map: std::collections::HashMap<String, (String, String, String, f32, f32, f32, String, String, usize, f32)>
            = std::collections::HashMap::new();

        for obj in self.scene.objects.values() {
            if !obj.visible { continue; }
            let tag = &obj.tag;
            if tag.is_empty() { continue; }

            let (w, h, d) = match &obj.shape {
                crate::scene::Shape::Box { width, height, depth } => (*width, *height, *depth),
                crate::scene::Shape::Cylinder { radius, height, .. } => (*radius * 2.0, *height, *radius * 2.0),
                crate::scene::Shape::Line { points, .. } => {
                    if points.len() >= 2 {
                        let dx = points.last().unwrap()[0] - points[0][0];
                        let dy = points.last().unwrap()[1] - points[0][1];
                        let dz = points.last().unwrap()[2] - points[0][2];
                        ((dx*dx+dy*dy+dz*dz).sqrt(), 0.0, 0.0)
                    } else { (0.0, 0.0, 0.0) }
                }
                _ => (0.0, 0.0, 0.0),
            };

            let cat = if tag.starts_with("管線") { "管線" }
                else if tag.starts_with("管件") { "管件" }
                else if tag.contains("鋼構") || obj.component_kind != Default::default() { "鋼構" }
                else { "其他" };

            let spec = &obj.name;
            let mat = format!("{:?}", obj.material);
            let key = format!("{}|{}|{}", cat, spec, tag);

            let entry = bom_map.entry(key).or_insert_with(|| {
                (cat.to_string(), spec.clone(), tag.clone(), w, h, d, mat.clone(), String::new(), 0, 0.0)
            });
            entry.8 += 1;
            // 累加長度（管線）
            if cat == "管線" { entry.9 += w; }
        }

        let mut entries: Vec<_> = bom_map.into_values().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        for (idx, (cat, name, tag, w, h, d, mat, note, qty, total_len)) in entries.iter().enumerate() {
            let len_str = if *total_len > 0.0 { format!("{:.0}", total_len) } else { format!("{:.0}", w) };
            writeln!(file, "{},{},{},{},{},{:.0},{:.0},{},{},{},{},",
                idx + 1, cat, name, tag, len_str, h, d, mat, tag, qty, "")
                .map_err(|e| e.to_string())?;
            count += 1;
        }

        Ok(count)
    }
}

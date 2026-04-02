//! 鋼構輸出功能：報表、施工圖、NC、IFC、碰撞偵測、自動編號
//! Phase B/C/D UI 按鈕的後端實作

use crate::app::KolibriApp;
use kolibri_core::collision::ComponentKind;

impl KolibriApp {
    /// Phase B: 匯出完整鋼構報表（CSV）
    pub(crate) fn export_steel_report(&mut self) {
        let file = rfd::FileDialog::new()
            .set_title("匯出鋼構報表 (CSV)")
            .add_filter("CSV", &["csv"])
            .set_file_name("steel_report.csv")
            .save_file();

        if let Some(path) = file {
            let ps = path.to_string_lossy().to_string();
            // 目前場景中沒有持久化的 connections，用空列表
            let connections = Vec::new();
            let report = kolibri_core::steel_report::generate_report(&self.scene, &connections);

            match kolibri_core::steel_report::export_report_csv(&report, &ps) {
                Ok(()) => {
                    let msg = format!(
                        "報表已匯出: {} 種構件, {:.0}kg, {} 螺栓, {:.0}mm 焊接 → {}",
                        report.numbering.stats.unique_marks,
                        report.grand_total_weight,
                        report.total_bolt_count,
                        report.total_weld_length,
                        ps,
                    );
                    self.console_push("ACTION", msg.clone());
                    self.file_message = Some((msg, std::time::Instant::now()));
                }
                Err(e) => {
                    self.console_push("ERROR", format!("報表匯出失敗: {}", e));
                    self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now()));
                }
            }
        }
    }

    /// Phase C: 匯出施工圖 DXF
    pub(crate) fn export_steel_drawings(&mut self) {
        let file = rfd::FileDialog::new()
            .set_title("匯出施工圖 (DXF)")
            .add_filter("DXF", &["dxf"])
            .set_file_name("GA_drawing.dxf")
            .save_file();

        if let Some(path) = file {
            let ps = path.to_string_lossy().to_string();
            let numbering = kolibri_core::steel_numbering::auto_number(&self.scene);
            let drawing = kolibri_core::steel_drawing::generate_ga_drawing(&self.scene, &numbering);

            match kolibri_core::steel_drawing::export_drawing_dxf(&drawing, &ps) {
                Ok(()) => {
                    let view_count = drawing.views.len();
                    let elem_count: usize = drawing.views.iter().map(|v| v.elements.len()).sum();
                    let msg = format!("施工圖已匯出: {} 個視圖, {} 個元素 → {}", view_count, elem_count, ps);
                    self.console_push("ACTION", msg.clone());
                    self.file_message = Some((msg, std::time::Instant::now()));
                }
                Err(e) => {
                    self.console_push("ERROR", format!("施工圖匯出失敗: {}", e));
                    self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now()));
                }
            }
        }
    }

    /// Phase D: 匯出 DSTV NC1 檔案
    pub(crate) fn export_nc_files(&mut self) {
        let folder = rfd::FileDialog::new()
            .set_title("選擇 NC 輸出資料夾")
            .pick_folder();

        if let Some(dir) = folder {
            let dir_str = dir.to_string_lossy().to_string();
            let numbering = kolibri_core::steel_numbering::auto_number(&self.scene);
            let connections = Vec::new();
            let programs = kolibri_io::nc_export::generate_nc_programs(&self.scene, &connections, &numbering);

            match kolibri_io::nc_export::export_all_nc(&programs, &dir_str) {
                Ok(count) => {
                    let msg = format!("NC 已匯出: {} 個構件 → {}", count, dir_str);
                    self.console_push("ACTION", msg.clone());
                    self.file_message = Some((msg, std::time::Instant::now()));
                }
                Err(e) => {
                    self.console_push("ERROR", format!("NC 匯出失敗: {}", e));
                    self.file_message = Some((format!("NC 匯出失敗: {}", e), std::time::Instant::now()));
                }
            }
        }
    }

    /// Phase D: 匯出 IFC 2x3
    pub(crate) fn export_ifc_file(&mut self) {
        let file = rfd::FileDialog::new()
            .set_title("匯出 IFC 檔案")
            .add_filter("IFC", &["ifc"])
            .set_file_name("model.ifc")
            .save_file();

        if let Some(path) = file {
            let ps = path.to_string_lossy().to_string();
            let numbering = kolibri_core::steel_numbering::auto_number(&self.scene);
            let connections = Vec::new();

            match kolibri_io::ifc_export::export_ifc(&self.scene, &connections, &numbering, &ps) {
                Ok(count) => {
                    let msg = format!("IFC 已匯出: {} 個實體 → {}", count, ps);
                    self.console_push("ACTION", msg.clone());
                    self.file_message = Some((msg, std::time::Instant::now()));
                }
                Err(e) => {
                    self.console_push("ERROR", format!("IFC 匯出失敗: {}", e));
                    self.file_message = Some((format!("IFC 匯出失敗: {}", e), std::time::Instant::now()));
                }
            }
        }
    }

    /// Phase D: 碰撞偵測
    pub(crate) fn run_collision_check(&mut self) {
        let config = kolibri_core::collision::CollisionConfig::default();
        let report = kolibri_core::collision::check_scene_collisions(&self.scene, &config);

        if report.all_clear {
            let msg = format!("碰撞偵測完成: 無碰撞問題 ({} 個構件)", self.scene.objects.len());
            self.console_push("ACTION", msg.clone());
            self.file_message = Some((msg, std::time::Instant::now()));
        } else {
            for w in &report.warnings {
                self.console_push("WARN", w.message.clone());
            }
            let msg = format!("碰撞偵測: 發現 {} 個問題", report.warnings.len());
            self.console_push("ACTION", msg.clone());
            self.file_message = Some((msg, std::time::Instant::now()));
            self.editor.collision_warning = Some(format!("{} 個碰撞", report.warnings.len()));
        }
    }

    /// Phase B: 自動編號
    pub(crate) fn run_auto_numbering(&mut self) {
        let result = kolibri_core::steel_numbering::auto_number(&self.scene);
        let stats = &result.stats;

        // 更新物件名稱為編號
        for (id, mark) in &result.marks {
            if let Some(obj) = self.scene.objects.get_mut(id) {
                if !obj.name.contains(&*mark) {
                    obj.name = format!("{} [{}]", obj.name, mark);
                }
            }
        }
        self.scene.version += 1;

        let msg = format!(
            "自動編號完成: {} 種編號 (柱{} 梁{} 撐{} 板{}) {} 個組裝件",
            stats.unique_marks, stats.total_columns, stats.total_beams,
            stats.total_braces, stats.total_plates, stats.assembly_count,
        );
        self.console_push("ACTION", msg.clone());
        self.file_message = Some((msg, std::time::Instant::now()));
    }

    /// 統計鋼構構件數量
    pub(crate) fn count_steel_members(&self) -> (u32, u32, u32, u32) {
        let mut cols = 0_u32;
        let mut beams = 0_u32;
        let mut braces = 0_u32;
        let mut plates = 0_u32;
        for obj in self.scene.objects.values() {
            match obj.component_kind {
                ComponentKind::Column => cols += 1,
                ComponentKind::Beam => beams += 1,
                ComponentKind::Brace => braces += 1,
                ComponentKind::Plate => plates += 1,
                _ => {}
            }
        }
        (cols, beams, braces, plates)
    }

    /// 改 Level 標高後，更新所有綁定構件的位置
    /// 邏輯：
    ///   柱：base_level → position.Y = level_elev, height = top_level_elev - base_level_elev
    ///   梁：base_level → position.Y = level_elev - beam_height
    pub(crate) fn update_levels(&mut self) {
        let levels = self.editor.floor_levels.clone();
        let gl = self.editor.ground_level;
        let mut updated = 0_u32;

        // 收集群組資訊（群組的子物件要一起動）
        let groups: Vec<(String, Vec<String>)> = self.scene.groups.values()
            .map(|g| (g.id.clone(), g.children.clone()))
            .collect();

        // 處理群組單位（柱/梁是群組，子物件一起更新）
        for (gid, children) in &groups {
            if children.is_empty() { continue; }

            // 取第一個子物件的 level 綁定
            let first = match self.scene.objects.get(&children[0]) {
                Some(o) => o.clone(),
                None => continue,
            };
            let base_idx = match first.base_level_idx {
                Some(i) => i,
                None => continue, // 沒綁定 level
            };
            let base_elev = levels.get(base_idx).map_or(0.0, |f| f.1) + gl;

            match first.component_kind {
                ComponentKind::Column => {
                    // 柱頂 = top_level 標高
                    let top_idx = first.top_level_idx.unwrap_or(base_idx);
                    let top_elev = levels.get(top_idx).map_or(base_elev + 4200.0, |f| f.1) + gl;
                    let new_height = (top_elev - base_elev).max(100.0);

                    // 計算原始位置的 XZ 中心（不改 XZ）
                    for cid in children {
                        if let Some(obj) = self.scene.objects.get_mut(cid) {
                            // 更新 Y 位置到新的 base_elev
                            obj.position[1] = base_elev;
                            // 更新高度
                            if let crate::scene::Shape::Box { ref mut height, .. } = obj.shape {
                                *height = new_height;
                            }
                        }
                    }
                    updated += 1;
                }
                ComponentKind::Beam => {
                    // 梁頂 = 綁定 level 標高
                    let level_elev = base_elev;
                    // 取梁高（從第一個子物件推斷）
                    let beam_h = {
                        let mut max_h = 0.0_f32;
                        for cid in children {
                            if let Some(obj) = self.scene.objects.get(cid) {
                                if let crate::scene::Shape::Box { height, .. } = &obj.shape {
                                    max_h = max_h.max(*height);
                                }
                            }
                        }
                        max_h
                    };
                    // 取截面高（所有子物件的 Y 跨度）
                    let mut min_y = f32::MAX;
                    let mut max_y = f32::MIN;
                    for cid in children {
                        if let Some(obj) = self.scene.objects.get(cid) {
                            if let crate::scene::Shape::Box { height, .. } = &obj.shape {
                                min_y = min_y.min(obj.position[1]);
                                max_y = max_y.max(obj.position[1] + height);
                            }
                        }
                    }
                    let section_h = max_y - min_y;
                    let new_beam_y = level_elev - section_h;

                    // 計算 Y 偏移量
                    let dy = new_beam_y - min_y;
                    if dy.abs() > 0.1 {
                        for cid in children {
                            if let Some(obj) = self.scene.objects.get_mut(cid) {
                                obj.position[1] += dy;
                            }
                        }
                        updated += 1;
                    }
                }
                _ => {}
            }
        }

        if updated > 0 {
            self.scene.version += 1;
            self.file_message = Some((
                format!("已更新 {} 個構件位置", updated),
                std::time::Instant::now(),
            ));
            self.console_push("LEVEL", format!("樓層標高變更 → {} 構件已更新", updated));
        }
    }
}

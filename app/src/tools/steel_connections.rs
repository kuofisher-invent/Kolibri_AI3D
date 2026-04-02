//! 鋼構接頭工具 — 選取構件 → 自動生成接頭（板件+螺栓+焊接）
//! Phase A: 端板式、腹板式、底板、螺栓放置、焊接標記、肋板

use crate::app::KolibriApp;
use crate::scene::{MaterialKind, SceneObject, Shape};
use kolibri_core::collision::ComponentKind;
use kolibri_core::steel_connection::*;

/// 計算物件 AABB 邊界 (min, max)
fn obj_bounds(obj: &SceneObject) -> ([f32; 3], [f32; 3]) {
    let p = obj.position;
    match &obj.shape {
        Shape::Box { width, height, depth } => {
            (p, [p[0] + width, p[1] + height, p[2] + depth])
        }
        Shape::Cylinder { radius, height, .. } => {
            ([p[0] - radius, p[1], p[2] - radius],
             [p[0] + radius, p[1] + height, p[2] + radius])
        }
        _ => (p, p),
    }
}

/// 計算物件幾何中心
fn obj_center(obj: &SceneObject) -> [f32; 3] {
    let (min, max) = obj_bounds(obj);
    [(min[0]+max[0])/2.0, (min[1]+max[1])/2.0, (min[2]+max[2])/2.0]
}

impl KolibriApp {
    /// 將 pick 到的子物件 ID 追溯到群組 ID（如果屬於群組）
    pub(crate) fn resolve_to_group(&self, obj_id: &str) -> String {
        if let Some(obj) = self.scene.objects.get(obj_id) {
            if let Some(ref pid) = obj.parent_id {
                if self.scene.groups.contains_key(pid) {
                    return pid.clone();
                }
            }
        }
        // 也檢查是否本身就是群組 ID
        if self.scene.groups.contains_key(obj_id) {
            return obj_id.to_string();
        }
        obj_id.to_string()
    }

    /// 接頭建立後執行 AISC 驗算並輸出到 Console
    fn check_and_report_connection(&mut self, conn: &SteelConnection, material_name: &str) {
        let mat = SteelMaterial::from_name(material_name);
        let check = check_connection(conn, &mat, DesignMethod::LRFD);

        self.console_push("AISC", format!(
            "{} | 螺栓抗剪:{:.0}kN 抗拉:{:.0}kN | 焊接:{:.0}kN",
            conn.conn_type.label(),
            check.total_bolt_shear, check.total_bolt_tension,
            check.total_weld_capacity,
        ));

        if !check.pass {
            for w in &check.warnings {
                self.console_push("WARN", format!("AISC: {}", w));
            }
        } else if !check.warnings.is_empty() {
            for w in &check.warnings {
                self.console_push("INFO", format!("AISC: {}", w));
            }
        }
    }

    /// AISC 智慧建議：根據已選取的兩構件自動分析並建議接頭
    pub(crate) fn show_aisc_suggestion(&mut self) {
        let ids = self.editor.selected_ids.clone();
        if ids.len() < 2 {
            self.file_message = Some(("請先選取兩個構件再按 AISC 建議".into(), std::time::Instant::now()));
            return;
        }

        // 辨識梁/柱
        let (beam_id, col_id) = match self.identify_beam_column(&ids) {
            Some(pair) => pair,
            None => {
                // 嘗試柱底板
                let has_col = ids.iter().any(|id| {
                    let cids = self.get_group_member_ids(id);
                    cids.iter().any(|cid| {
                        self.scene.objects.get(cid).map_or(false, |o| o.component_kind == ComponentKind::Column)
                    })
                });
                if has_col {
                    self.console_push("AISC", "偵測到柱 → 建議底板接頭".into());
                    let col_id = ids[0].clone();
                    let col_section = self.get_member_section(&col_id);
                    let suggestions = kolibri_core::steel_connection::suggest_connection(
                        col_section, col_section,
                        ConnectionIntent::ColumnBase,
                        &self.editor.steel_material,
                    );
                    self.display_suggestions(&suggestions);
                    return;
                }
                self.file_message = Some(("未偵測到梁+柱或柱組合".into(), std::time::Instant::now()));
                return;
            }
        };

        let beam_section = self.get_member_section(&beam_id);
        let col_section = self.get_member_section(&col_id);

        let suggestions = kolibri_core::steel_connection::suggest_connection(
            beam_section, col_section,
            ConnectionIntent::BeamToColumn,
            &self.editor.steel_material,
        );

        self.display_suggestions(&suggestions);

        // 自動採用第一個建議的螺栓尺寸
        if let Some(s) = suggestions.first() {
            self.editor.conn_bolt_size = s.bolt_size;
            self.editor.conn_bolt_grade = s.bolt_grade;
            self.editor.conn_add_stiffeners = s.need_stiffeners;
        }
    }

    /// 在 Console 顯示 AISC 建議結果
    fn display_suggestions(&mut self, suggestions: &[ConnectionSuggestion]) {
        self.console_push("AISC", "═══ AISC 360-22 接頭建議 ═══".into());
        for (i, s) in suggestions.iter().enumerate() {
            self.console_push("AISC", format!(
                "方案{}: {} — {}",
                i + 1, s.conn_type.label(), s.reason
            ));
            self.console_push("AISC", format!(
                "  螺栓: {} {} | 端板厚: {:.0}mm | 加勁板: {}",
                s.bolt_size.label(), s.bolt_grade.label(),
                s.plate_thickness,
                if s.need_stiffeners { "需要" } else { "不需" },
            ));
            if s.need_stiffeners {
                self.console_push("AISC", format!("  加勁板原因: {}", s.stiffener_reason));
            }

            // 孔位資訊
            let hole_d = s.bolt_size.hole_diameter();
            let min_edge = s.bolt_size.min_edge();
            let min_spacing = s.bolt_size.min_spacing();
            self.console_push("AISC", format!(
                "  孔徑: Ø{:.0}mm (標準孔=Ø{}+2) | 邊距≥{:.0}mm | 間距≥{:.0}mm",
                hole_d, s.bolt_size.label(), min_edge, min_spacing,
            ));

            let cap = &s.estimated_capacity;
            self.console_push("AISC", format!(
                "  強度: 螺栓抗剪{:.0}kN 抗拉{:.0}kN | 焊接{:.0}kN | {}",
                cap.total_bolt_shear, cap.total_bolt_tension,
                cap.total_weld_capacity,
                if cap.pass { "PASS" } else { "FAIL" },
            ));
            self.console_push("AISC", format!("  依據: {}", s.aisc_ref));
        }
        self.console_push("AISC", "═══════════════════════════".into());

        if let Some(s) = suggestions.first() {
            self.file_message = Some((
                format!("AISC 建議: {} | {} {} | 板厚{:.0}mm | {}",
                    s.conn_type.label(), s.bolt_size.label(), s.bolt_grade.label(),
                    s.plate_thickness,
                    if s.need_stiffeners { "需加勁板" } else { "不需加勁板" },
                ),
                std::time::Instant::now(),
            ));
        }
    }

    /// 鋼構專用 pick：先嘗試精確 pick，失敗則擴大搜尋範圍（±5px）
    /// 並自動追溯到群組
    pub(crate) fn steel_pick_member(&mut self) -> Option<String> {
        let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
        let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);

        // 先嘗試精確 pick
        if let Some(raw_id) = self.pick(mx, my, vw, vh) {
            return Some(self.resolve_to_group(&raw_id));
        }

        // 擴大搜尋範圍（H型鋼翼板很薄，容易 miss）
        let offsets: &[(f32, f32)] = &[
            (-5.0, 0.0), (5.0, 0.0), (0.0, -5.0), (0.0, 5.0),
            (-10.0, 0.0), (10.0, 0.0), (0.0, -10.0), (0.0, 10.0),
            (-5.0, -5.0), (5.0, 5.0), (-5.0, 5.0), (5.0, -5.0),
        ];
        for &(dx, dy) in offsets {
            if let Some(raw_id) = self.pick(mx + dx, my + dy, vw, vh) {
                return Some(self.resolve_to_group(&raw_id));
            }
        }

        self.file_message = Some(("未選到構件 — 請點擊柱或梁".into(), std::time::Instant::now()));
        None
    }

    /// 端板式接頭（梁-柱剛接）：選取梁+柱 → 自動計算 → 生成 3D 物件
    pub(crate) fn create_end_plate_connection(&mut self) {
        let ids = self.editor.selected_ids.clone();
        if ids.len() < 2 {
            self.file_message = Some(("請選取梁和柱（兩個構件）".into(), std::time::Instant::now()));
            return;
        }

        // 辨識梁和柱
        let (beam_id, col_id) = match self.identify_beam_column(&ids) {
            Some(pair) => pair,
            None => {
                self.file_message = Some(("未偵測到梁+柱組合".into(), std::time::Instant::now()));
                return;
            }
        };

        // 取得截面參數
        let beam_section = self.get_member_section(&beam_id);
        let col_section = self.get_member_section(&col_id);

        // 計算接頭位置（梁端與柱的交接處）
        let conn_pos = self.calc_connection_position(&beam_id, &col_id);

        // 計算端板接頭
        let params = EndPlateParams {
            beam_section,
            col_section,
            bolt_size: self.editor.conn_bolt_size,
            bolt_grade: self.editor.conn_bolt_grade,
            plate_thickness: None,
            add_stiffeners: self.editor.conn_add_stiffeners,
        };
        let conn = calc_end_plate(&params);

        // 生成 3D 物件
        self.scene.snapshot();
        let mut child_ids = Vec::new();
        let name_base = self.next_name("EP");
        let (is_x_dir, sign) = self.beam_direction(&beam_id, &col_id);

        // 生成板件（端板 + 肋板）
        for (i, plate) in conn.plates.iter().enumerate() {
            let plate_name = match plate.plate_type {
                PlateType::EndPlate => format!("{}_plate", name_base),
                PlateType::Stiffener => format!("{}_stiff_{}", name_base, i),
                _ => format!("{}_pl_{}", name_base, i),
            };

            let (pos, bw, bh, bd) = self.calc_plate_world_pos(conn_pos, plate, is_x_dir, sign);
            let id = self.scene.insert_box_raw(plate_name, pos, bw, bh, bd, MaterialKind::Metal);
            if let Some(obj) = self.scene.objects.get_mut(&id) {
                obj.component_kind = ComponentKind::Plate;
            }
            child_ids.push(id);
        }

        // 生成螺栓（位置用本地→世界轉換）
        for bg in &conn.bolts {
            let bolt_ids = self.create_bolt_group_world(bg, conn_pos, is_x_dir, sign);
            child_ids.extend(bolt_ids);
        }

        // 生成焊接標記
        for (i, weld) in conn.welds.iter().enumerate() {
            let weld_name = format!("{}_weld_{}", name_base, i);
            let w_start = self.conn_local_to_world(conn_pos, weld.start, is_x_dir, sign);
            let w_end = self.conn_local_to_world(conn_pos, weld.end, is_x_dir, sign);
            let id = self.scene.insert_weld_line(weld_name, w_start, w_end, weld.size);
            if let Some(obj) = self.scene.objects.get_mut(&id) {
                obj.component_kind = ComponentKind::Weld;
            }
            child_ids.push(id);
        }

        // 群組
        self.scene.create_group(
            format!("{} 端板接頭", name_base),
            child_ids.clone(),
        );
        self.scene.version += 1;
        self.editor.selected_ids = child_ids.clone();

        self.ai_log.log(
            &self.current_actor, "建立端板接頭",
            &format!("梁={} 柱={} 螺栓={}", beam_id, col_id, conn.bolts[0].bolt_size.label()),
            child_ids,
        );
        self.file_message = Some((
            format!("端板接頭已建立: {} + {} 螺栓", params.bolt_size.label(),
                    if params.add_stiffeners { "含肋板" } else { "無肋板" }),
            std::time::Instant::now(),
        ));

        // AISC 驗算
        self.check_and_report_connection(&conn, &self.editor.steel_material.clone());
    }

    /// 腹板式接頭（梁-柱鉸接）
    pub(crate) fn create_shear_tab_connection(&mut self) {
        let ids = self.editor.selected_ids.clone();
        if ids.len() < 2 {
            self.file_message = Some(("請選取梁和柱（兩個構件）".into(), std::time::Instant::now()));
            return;
        }

        let (beam_id, col_id) = match self.identify_beam_column(&ids) {
            Some(pair) => pair,
            None => {
                self.file_message = Some(("未偵測到梁+柱組合".into(), std::time::Instant::now()));
                return;
            }
        };

        let beam_section = self.get_member_section(&beam_id);
        let conn_pos = self.calc_connection_position(&beam_id, &col_id);

        let conn = calc_shear_tab(beam_section, self.editor.conn_bolt_size, self.editor.conn_bolt_grade);

        self.scene.snapshot();
        let mut child_ids = Vec::new();
        let name_base = self.next_name("ST");
        let (is_x_dir, sign) = self.beam_direction(&beam_id, &col_id);

        // 剪力板
        for (i, plate) in conn.plates.iter().enumerate() {
            let plate_name = format!("{}_tab_{}", name_base, i);
            let (pos, bw, bh, bd) = self.calc_plate_world_pos(conn_pos, plate, is_x_dir, sign);
            let id = self.scene.insert_box_raw(plate_name, pos, bw, bh, bd, MaterialKind::Metal);
            if let Some(obj) = self.scene.objects.get_mut(&id) {
                obj.component_kind = ComponentKind::Plate;
            }
            child_ids.push(id);
        }

        // 螺栓
        for bg in &conn.bolts {
            let bolt_ids = self.create_bolt_group_world(bg, conn_pos, is_x_dir, sign);
            child_ids.extend(bolt_ids);
        }

        // 焊接
        for (i, weld) in conn.welds.iter().enumerate() {
            let weld_name = format!("{}_weld_{}", name_base, i);
            let w_start = self.conn_local_to_world(conn_pos, weld.start, is_x_dir, sign);
            let w_end = self.conn_local_to_world(conn_pos, weld.end, is_x_dir, sign);
            let id = self.scene.insert_weld_line(weld_name, w_start, w_end, weld.size);
            if let Some(obj) = self.scene.objects.get_mut(&id) {
                obj.component_kind = ComponentKind::Weld;
            }
            child_ids.push(id);
        }

        self.scene.create_group(format!("{} 腹板接頭", name_base), child_ids.clone());
        self.scene.version += 1;
        self.editor.selected_ids = child_ids.clone();
        self.file_message = Some(("腹板接頭已建立".into(), std::time::Instant::now()));
        self.check_and_report_connection(&conn, &self.editor.steel_material.clone());
    }

    /// 底板接頭（柱底+錨栓）
    pub(crate) fn create_base_plate_connection(&mut self) {
        let ids = self.editor.selected_ids.clone();
        if ids.is_empty() {
            self.file_message = Some(("請選取柱".into(), std::time::Instant::now()));
            return;
        }

        // 找柱（先 resolve 到群組）
        let resolved_ids: Vec<String> = ids.iter().map(|id| self.resolve_to_group(id)).collect();
        let col_id = resolved_ids.iter().find(|id| {
            let cids = self.get_group_member_ids(id);
            cids.iter().any(|cid| {
                self.scene.objects.get(cid).map_or(false, |o| o.component_kind == ComponentKind::Column)
            })
        }).cloned().unwrap_or_else(|| resolved_ids[0].clone());

        let col_section = self.get_member_section(&col_id);
        // 取柱群組中心作為底板位置
        let col_center = self.get_group_center(&col_id);
        // 底板放置在柱底部（Y = 群組最低點）
        let (col_min, _) = self.get_group_bounds(&col_id);
        let col_pos = [col_center[0], col_min[1], col_center[2]];

        let conn = calc_base_plate(col_section, self.editor.conn_bolt_size, self.editor.conn_bolt_grade);

        self.scene.snapshot();
        let mut child_ids = Vec::new();
        let name_base = self.next_name("BP");

        // 底板
        for plate in &conn.plates {
            let plate_pos = [col_pos[0] - plate.width / 2.0, col_pos[1] - plate.thickness, col_pos[2] - plate.height / 2.0];
            let id = self.scene.insert_box_raw(
                format!("{}_plate", name_base), plate_pos,
                plate.width, plate.thickness, plate.height, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&id) {
                obj.component_kind = ComponentKind::Plate;
            }
            child_ids.push(id);
        }

        // 錨栓（圓柱，從底板往下延伸）
        for (i, bg) in conn.bolts.iter().enumerate() {
            for (j, bp) in bg.positions.iter().enumerate() {
                let bolt_name = format!("{}_anchor_{}_{}", name_base, i, j);
                // 錨栓嵌入深度 = 12 × 螺栓直徑（ACI 318 經驗值）
                let embed_depth = bg.bolt_size.diameter() * 12.0;
                let plate_t = conn.plates[0].thickness;
                let bolt_pos = [col_pos[0] + bp[0], col_pos[1] - plate_t - embed_depth, col_pos[2] + bp[2]];
                let bolt_r = bg.bolt_size.diameter() / 2.0;
                let bolt_h = embed_depth + plate_t;

                let id = self.scene.insert_cylinder_raw(
                    bolt_name, bolt_pos, bolt_r, bolt_h, 12, MaterialKind::Metal,
                );
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    obj.component_kind = ComponentKind::Bolt;
                }
                child_ids.push(id);
            }
        }

        self.scene.create_group(format!("{} 底板接頭", name_base), child_ids.clone());
        self.scene.version += 1;
        self.editor.selected_ids = child_ids.clone();
        self.file_message = Some(("底板接頭已建立".into(), std::time::Instant::now()));
        self.check_and_report_connection(&conn, &self.editor.steel_material.clone());
    }

    // ─── 輔助函式 ──────────────────────────────────────────────────────────────

    /// 辨識選取物件中的梁和柱
    pub(crate) fn identify_beam_column(&self, ids: &[String]) -> Option<(String, String)> {
        let mut beam_id = None;
        let mut col_id = None;

        for id in ids {
            // 先找 group 的子物件
            let check_ids = self.get_group_member_ids(id);
            for cid in &check_ids {
                if let Some(obj) = self.scene.objects.get(cid) {
                    match obj.component_kind {
                        ComponentKind::Beam => { beam_id = Some(id.clone()); }
                        ComponentKind::Column => { col_id = Some(id.clone()); }
                        _ => {}
                    }
                }
            }
            // 也檢查物件本身
            if let Some(obj) = self.scene.objects.get(id) {
                match obj.component_kind {
                    ComponentKind::Beam => { beam_id = Some(id.clone()); }
                    ComponentKind::Column => { col_id = Some(id.clone()); }
                    _ => {}
                }
            }
        }

        // 如果只有兩個物件且無法辨識，用名稱推斷
        if beam_id.is_none() || col_id.is_none() {
            if ids.len() >= 2 {
                for id in ids {
                    let name = self.scene.objects.get(id).map_or(String::new(), |o| o.name.to_uppercase());
                    if name.contains("BM") || name.contains("BEAM") {
                        beam_id = Some(id.clone());
                    } else if name.contains("COL") || name.contains("COLUMN") {
                        col_id = Some(id.clone());
                    }
                }
            }
        }

        // 最後嘗試：依位置推斷（Y 方向高的 = 柱，寬的 = 梁）
        if beam_id.is_none() || col_id.is_none() {
            if ids.len() >= 2 {
                let h0 = self.get_member_height(&ids[0]);
                let h1 = self.get_member_height(&ids[1]);
                if h0 > h1 {
                    col_id = Some(ids[0].clone());
                    beam_id = Some(ids[1].clone());
                } else {
                    col_id = Some(ids[1].clone());
                    beam_id = Some(ids[0].clone());
                }
            }
        }

        match (beam_id, col_id) {
            (Some(b), Some(c)) => Some((b, c)),
            _ => None,
        }
    }

    /// 取得構件的 H 截面參數 (H, B, tw, tf)
    pub(crate) fn get_member_section(&self, id: &str) -> (f32, f32, f32, f32) {
        // 查找群組子物件來推斷截面
        let child_ids = self.get_group_member_ids(id);
        if child_ids.len() >= 3 {
            // H 型鋼 = 2 翼板 + 1 腹板
            let mut flanges = Vec::new();
            let mut web = None;
            for cid in &child_ids {
                if let Some(obj) = self.scene.objects.get(cid) {
                    if let crate::scene::Shape::Box { width, height, depth } = &obj.shape {
                        // 翼板較寬，腹板較窄
                        if *width > *depth * 2.0 || *depth > *width * 2.0 {
                            // 可能是翼板或腹板
                            let min_dim = width.min(*depth);
                            let max_dim = width.max(*depth);
                            if min_dim < 30.0 { // 薄的 = 翼板或腹板
                                if flanges.len() < 2 && max_dim > min_dim * 3.0 {
                                    flanges.push((*width, *height, *depth));
                                } else {
                                    web = Some((*width, *height, *depth));
                                }
                            }
                        } else {
                            web = Some((*width, *height, *depth));
                        }
                    }
                }
            }
            if let (Some((ww, _wh, wd)), true) = (web, flanges.len() >= 1) {
                let (fw, _fh, fd) = flanges[0];
                let b = fw.max(fd); // 翼板寬
                let tf = fw.min(fd); // 翼板厚
                let tw = ww.min(wd); // 腹板厚
                let h = wd.max(ww) + 2.0 * tf; // 截面高 ≈ 腹板深 + 2×翼板厚
                return (h, b, tw, tf);
            }
        }

        // 嘗試直接從物件 shape 推斷
        if let Some(obj) = self.scene.objects.get(id) {
            if let crate::scene::Shape::Box { width, height, depth } = &obj.shape {
                return (*height, width.max(*depth), 8.0, 12.0);
            }
        }

        // 預設 H300x150x6x9
        (300.0, 150.0, 6.0, 9.0)
    }

    /// 取得構件高度（Y 方向）
    pub(crate) fn get_member_height(&self, id: &str) -> f32 {
        let child_ids = self.get_group_member_ids(id);
        let mut max_h = 0.0_f32;
        for cid in &child_ids {
            if let Some(obj) = self.scene.objects.get(cid) {
                if let crate::scene::Shape::Box { height, .. } = &obj.shape {
                    max_h = max_h.max(*height);
                }
            }
        }
        if max_h == 0.0 {
            if let Some(obj) = self.scene.objects.get(id) {
                if let crate::scene::Shape::Box { height, .. } = &obj.shape {
                    return *height;
                }
            }
        }
        max_h
    }

    /// 取得群組的子物件 ID（如果是群組的話）
    pub(crate) fn get_group_member_ids(&self, id: &str) -> Vec<String> {
        // 檢查是否為群組
        if let Some(group) = self.scene.groups.get(id) {
            return group.children.clone();
        }
        // 檢查物件是否屬於某群組
        if let Some(obj) = self.scene.objects.get(id) {
            if let Some(ref pid) = obj.parent_id {
                if let Some(group) = self.scene.groups.get(pid) {
                    return group.children.clone();
                }
            }
        }
        // 不是群組，回傳自身
        vec![id.to_string()]
    }

    /// 取得群組或物件的 AABB 幾何中心（正確處理 Box position=左下角）
    pub(crate) fn get_group_center(&self, id: &str) -> [f32; 3] {
        let child_ids = self.get_group_member_ids(id);
        if child_ids.is_empty() {
            if let Some(obj) = self.scene.objects.get(id) {
                return obj_center(obj);
            }
            return [0.0; 3];
        }

        // 計算所有子物件的 AABB → 取中心
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for cid in &child_ids {
            if let Some(obj) = self.scene.objects.get(cid) {
                let (obj_min, obj_max) = obj_bounds(obj);
                for i in 0..3 {
                    min[i] = min[i].min(obj_min[i]);
                    max[i] = max[i].max(obj_max[i]);
                }
            }
        }
        [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0, (min[2] + max[2]) / 2.0]
    }

    /// 取得群組或物件的 AABB 邊界 (min, max)
    pub(crate) fn get_group_bounds(&self, id: &str) -> ([f32; 3], [f32; 3]) {
        let child_ids = self.get_group_member_ids(id);
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        let ids = if child_ids.is_empty() { vec![id.to_string()] } else { child_ids };
        for cid in &ids {
            if let Some(obj) = self.scene.objects.get(cid) {
                let (obj_min, obj_max) = obj_bounds(obj);
                for i in 0..3 {
                    min[i] = min[i].min(obj_min[i]);
                    max[i] = max[i].max(obj_max[i]);
                }
            }
        }
        (min, max)
    }

    /// 計算接頭位置（梁端靠近柱的那一端）
    fn calc_connection_position(&self, beam_id: &str, col_id: &str) -> [f32; 3] {
        let col_center = self.get_group_center(col_id);
        let (beam_min, beam_max) = self.get_group_bounds(beam_id);
        let beam_center = self.get_group_center(beam_id);

        // 判斷梁沿哪個軸延伸
        let span_x = beam_max[0] - beam_min[0];
        let span_z = beam_max[2] - beam_min[2];

        if span_x > span_z {
            // 梁沿 X 方向 — 找最近的 X 端
            let beam_end_x = if (beam_min[0] - col_center[0]).abs() < (beam_max[0] - col_center[0]).abs() {
                beam_min[0]
            } else {
                beam_max[0]
            };
            // 接頭 X = 梁端 X，Y = 梁中心 Y，Z = 梁中心 Z（與柱對齊）
            [beam_end_x, beam_center[1], beam_center[2]]
        } else {
            // 梁沿 Z 方向
            let beam_end_z = if (beam_min[2] - col_center[2]).abs() < (beam_max[2] - col_center[2]).abs() {
                beam_min[2]
            } else {
                beam_max[2]
            };
            [beam_center[0], beam_center[1], beam_end_z]
        }
    }

    /// 判斷梁相對於柱的方向（回傳 true=X方向, false=Z方向）和方向符號
    fn beam_direction(&self, beam_id: &str, col_id: &str) -> (bool, f32) {
        let col_center = self.get_group_center(col_id);
        let beam_center = self.get_group_center(beam_id);
        let dx = beam_center[0] - col_center[0];
        let dz = beam_center[2] - col_center[2];
        if dx.abs() > dz.abs() {
            (true, if dx > 0.0 { 1.0 } else { -1.0 })
        } else {
            (false, if dz > 0.0 { 1.0 } else { -1.0 })
        }
    }

    /// 把接頭本地座標 (local_x, local_y, local_z) 轉換為世界座標
    /// 本地座標系：X=板件水平, Y=板件垂直(高度), Z=板件法線(沿梁方向)
    fn conn_local_to_world(
        &self, conn_pos: [f32; 3], local: [f32; 3],
        is_x_dir: bool, sign: f32,
    ) -> [f32; 3] {
        if is_x_dir {
            // 梁沿 X → 端板面在 YZ 平面 → local_x→Z, local_y→Y, local_z→X
            [
                conn_pos[0] + local[2] * sign,
                conn_pos[1] + local[1],
                conn_pos[2] + local[0],
            ]
        } else {
            // 梁沿 Z → 端板面在 XY 平面 → local_x→X, local_y→Y, local_z→Z
            [
                conn_pos[0] + local[0],
                conn_pos[1] + local[1],
                conn_pos[2] + local[2] * sign,
            ]
        }
    }

    /// 計算板件在世界座標的位置（Box 左下角）
    fn calc_plate_world_pos(
        &self, conn_pos: [f32; 3], plate: &ConnectionPlate,
        is_x_dir: bool, sign: f32,
    ) -> ([f32; 3], f32, f32, f32) {
        // 板件中心在本地座標
        let center_local = plate.position; // [local_x, local_y, local_z]
        let center_world = self.conn_local_to_world(conn_pos, center_local, is_x_dir, sign);

        // Box 尺寸：width=板寬, height=板高, depth=板厚
        // 在世界座標中，根據方向分配 w/h/d
        if is_x_dir {
            // 端板在 YZ 平面：Box(厚=X, 高=Y, 寬=Z)
            let bw = plate.thickness; // X 方向
            let bh = plate.height;    // Y 方向
            let bd = plate.width;     // Z 方向
            let pos = [
                center_world[0] - bw / 2.0,
                center_world[1] - bh / 2.0,
                center_world[2] - bd / 2.0,
            ];
            (pos, bw, bh, bd)
        } else {
            // 端板在 XY 平面：Box(寬=X, 高=Y, 厚=Z)
            let bw = plate.width;
            let bh = plate.height;
            let bd = plate.thickness;
            let pos = [
                center_world[0] - bw / 2.0,
                center_world[1] - bh / 2.0,
                center_world[2] - bd / 2.0,
            ];
            (pos, bw, bh, bd)
        }
    }

    /// 生成螺栓群組的 3D mesh（含桿身+頭+墊圈+螺帽+孔徑標記）
    pub(crate) fn create_bolt_group_meshes(
        &mut self, bg: &BoltGroup, conn_pos: [f32; 3],
        _beam_id: &str, _col_id: &str,
    ) -> Vec<String> {
        let mut ids = Vec::new();
        let bolt_r = bg.bolt_size.diameter() / 2.0;
        let hole_r = bg.hole_diameter / 2.0;       // 孔徑半徑
        let head_r = bg.bolt_size.head_across_flats() / 2.0;
        let head_t = bg.bolt_size.head_thickness();
        let washer_r = head_r + 2.0;               // 墊圈比螺栓頭大 2mm
        let washer_t = 3.0;                         // 墊圈厚 3mm
        let nut_t = bg.bolt_size.diameter() * 0.8;  // 螺帽厚 ≈ 0.8d
        let grip = 50.0;                             // 夾持長度（板厚總和）

        // 輸出孔位資訊到 Console
        self.console_push("BOLT", format!(
            "螺栓組 {} {} | {}×{} = {} 顆 | 孔Ø{:.0}mm | 邊距{:.0}mm | 間距{:.0}mm",
            bg.bolt_size.label(), bg.bolt_grade.label(),
            bg.rows, bg.cols, bg.positions.len(),
            bg.hole_diameter, bg.edge_dist, bg.row_spacing,
        ));

        for (i, bp) in bg.positions.iter().enumerate() {
            let bolt_name = format!("{}_{}", bg.bolt_size.label(), i + 1);
            let bolt_pos = [
                conn_pos[0] + bp[0],
                conn_pos[1] + bp[1],
                conn_pos[2] + bp[2],
            ];

            // 1. 螺栓孔標記（透明圓柱，比螺栓大，代表孔徑）
            let hole_id = self.scene.insert_cylinder_raw(
                format!("{}_hole", bolt_name),
                [bolt_pos[0], bolt_pos[1] - 1.0, bolt_pos[2]],
                hole_r, grip + 2.0, 12,
                MaterialKind::Custom([0.2, 0.2, 0.2, 0.3]), // 深灰半透明
            );
            if let Some(obj) = self.scene.objects.get_mut(&hole_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(hole_id);

            // 2. 螺栓桿身（實心）
            let shank_id = self.scene.insert_cylinder_raw(
                format!("{}_shank", bolt_name),
                bolt_pos,
                bolt_r, grip + head_t + nut_t, 8, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&shank_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(shank_id);

            // 3. 螺栓頭（上方）
            let head_pos = [bolt_pos[0], bolt_pos[1] + grip, bolt_pos[2]];
            let head_id = self.scene.insert_cylinder_raw(
                format!("{}_head", bolt_name),
                head_pos,
                head_r, head_t, 6, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&head_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(head_id);

            // 4. 墊圈（螺栓頭下方）
            let washer_pos = [bolt_pos[0], bolt_pos[1] + grip - washer_t, bolt_pos[2]];
            let washer_id = self.scene.insert_cylinder_raw(
                format!("{}_washer", bolt_name),
                washer_pos,
                washer_r, washer_t, 12, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&washer_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(washer_id);

            // 5. 螺帽（底部）
            let nut_pos = [bolt_pos[0], bolt_pos[1] - nut_t, bolt_pos[2]];
            let nut_id = self.scene.insert_cylinder_raw(
                format!("{}_nut", bolt_name),
                nut_pos,
                head_r, nut_t, 6, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&nut_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(nut_id);
        }

        ids
    }

    /// 生成螺栓群組（使用本地→世界座標轉換）
    /// bolt positions 是相對於接頭中心的本地座標 [local_x, local_y, 0]
    pub(crate) fn create_bolt_group_world(
        &mut self, bg: &BoltGroup, conn_pos: [f32; 3],
        is_x_dir: bool, sign: f32,
    ) -> Vec<String> {
        let mut ids = Vec::new();
        let bolt_r = bg.bolt_size.diameter() / 2.0;
        let hole_r = bg.hole_diameter / 2.0;
        let head_r = bg.bolt_size.head_across_flats() / 2.0;
        let head_t = bg.bolt_size.head_thickness();
        let washer_r = head_r + 2.0;
        let washer_t = 3.0;
        let nut_t = bg.bolt_size.diameter() * 0.8;
        let grip = 50.0;

        self.console_push("BOLT", format!(
            "螺栓 {} {} | {}×{} = {} 顆 | 孔Ø{:.0} | 邊距{:.0} | 間距{:.0}",
            bg.bolt_size.label(), bg.bolt_grade.label(),
            bg.rows, bg.cols, bg.positions.len(),
            bg.hole_diameter, bg.edge_dist, bg.row_spacing,
        ));

        for (i, bp) in bg.positions.iter().enumerate() {
            let bolt_name = format!("{}_{}", bg.bolt_size.label(), i + 1);

            // bp = [local_x, local_y, 0] → 轉世界座標
            // 螺栓軸向沿板件法線（local_z）
            let bolt_center = self.conn_local_to_world(conn_pos, *bp, is_x_dir, sign);

            // 螺栓沿 local_z 方向延伸 — 在世界座標中是哪個軸？
            let bolt_axis_offset = if is_x_dir {
                // local_z → world X (× sign)
                [sign, 0.0, 0.0]
            } else {
                // local_z → world Z (× sign)
                [0.0, 0.0, sign]
            };

            // 孔標記（圓柱沿 Y 軸，穿透板件）
            let hole_id = self.scene.insert_cylinder_raw(
                format!("{}_hole", bolt_name),
                [bolt_center[0], bolt_center[1] - grip / 2.0, bolt_center[2]],
                hole_r, grip, 12,
                MaterialKind::Custom([0.2, 0.2, 0.2, 0.3]),
            );
            if let Some(obj) = self.scene.objects.get_mut(&hole_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(hole_id);

            // 螺栓桿身（沿板法線方向 = 用 bolt_axis_offset）
            // 簡化：螺栓都沿 Y 軸放（垂直），因為端板也是垂直的
            // 實際上螺栓穿透端板方向是水平的，但 Cylinder 只能沿 Y 軸
            // 所以用多個小 Box 代替或直接用 Y 軸 cylinder
            let shank_id = self.scene.insert_cylinder_raw(
                format!("{}_shank", bolt_name),
                [bolt_center[0] - bolt_axis_offset[0] * grip / 2.0,
                 bolt_center[1],
                 bolt_center[2] - bolt_axis_offset[2] * grip / 2.0],
                bolt_r, grip, 8, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&shank_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(shank_id);

            // 螺栓頭（板外側）
            let head_pos = [
                bolt_center[0] + bolt_axis_offset[0] * (grip / 2.0 + head_t / 2.0),
                bolt_center[1],
                bolt_center[2] + bolt_axis_offset[2] * (grip / 2.0 + head_t / 2.0),
            ];
            let head_id = self.scene.insert_cylinder_raw(
                format!("{}_head", bolt_name),
                head_pos,
                head_r, head_t, 6, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&head_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(head_id);
        }

        ids
    }

    // ─── AISC 接頭對話框流程 ──────────────────────────────────────────────────

    /// 開啟 AISC 接頭確認對話框（選取兩構件後觸發）
    pub(crate) fn open_connection_dialog(&mut self) {
        let raw_ids = self.editor.selected_ids.clone();
        if raw_ids.is_empty() {
            self.file_message = Some(("請先選取構件（選取兩個鋼構件後再按接頭功能鍵）".into(), std::time::Instant::now()));
            return;
        }

        // 先全部 resolve 到群組
        let ids: Vec<String> = raw_ids.iter()
            .map(|id| self.resolve_to_group(id))
            .collect::<std::collections::HashSet<_>>() // 去重
            .into_iter().collect();

        // 辨識梁/柱
        let (beam_id, col_id, intent) = if let Some((b, c)) = self.identify_beam_column(&ids) {
            (b, c, ConnectionIntent::BeamToColumn)
        } else {
            // 嘗試柱底板
            let col = self.resolve_to_group(&ids[0]);
            (col.clone(), col, ConnectionIntent::ColumnBase)
        };

        let beam_section = self.get_member_section(&beam_id);
        let col_section = self.get_member_section(&col_id);

        // AISC 自動建議
        let suggestions = kolibri_core::steel_connection::suggest_connection(
            beam_section, col_section, intent, &self.editor.steel_material,
        );

        // 設定預設參數（取第一方案）
        let (bs, bg, pt, stiff, ws) = if let Some(s) = suggestions.first() {
            (s.bolt_size, s.bolt_grade, s.plate_thickness, s.need_stiffeners,
             minimum_fillet_weld_size(s.plate_thickness))
        } else {
            (BoltSize::M20, BoltGrade::F10T, 20.0, true, 6.0)
        };

        self.editor.conn_dialog = Some(crate::editor::ConnectionDialogState {
            member_ids: vec![beam_id, col_id],
            beam_section,
            col_section,
            intent,
            suggestions,
            selected_idx: 0,
            bolt_size: bs,
            bolt_grade: bg,
            plate_thickness: pt,
            add_stiffeners: stiff,
            weld_size: ws,
        });
    }

    /// 使用者在對話框按「確認」後，執行接頭生成
    pub(crate) fn execute_connection_from_dialog(&mut self, selected_idx: usize) {
        let dialog = match &self.editor.conn_dialog {
            Some(d) => d.clone(),
            None => return,
        };

        let conn_type = dialog.suggestions.get(selected_idx)
            .map(|s| s.conn_type)
            .unwrap_or(ConnectionType::EndPlate);

        // 用對話框中使用者調整的參數
        self.editor.conn_bolt_size = dialog.bolt_size;
        self.editor.conn_bolt_grade = dialog.bolt_grade;
        self.editor.conn_add_stiffeners = dialog.add_stiffeners;
        self.editor.conn_weld_size = dialog.weld_size;

        self.editor.selected_ids = dialog.member_ids.clone();

        match dialog.intent {
            ConnectionIntent::BeamToColumn => {
                match conn_type {
                    ConnectionType::EndPlate => self.create_end_plate_connection(),
                    ConnectionType::ShearTab => self.create_shear_tab_connection(),
                    _ => self.create_end_plate_connection(),
                }
            }
            ConnectionIntent::ColumnBase => {
                self.create_base_plate_connection();
            }
            ConnectionIntent::BeamToBeam => {
                self.create_end_plate_connection();
            }
            ConnectionIntent::BraceToGusset => {
                self.create_shear_tab_connection();
            }
        }
    }
}

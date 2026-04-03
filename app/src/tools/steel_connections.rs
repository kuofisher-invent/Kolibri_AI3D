//! 鋼構接頭工具 — 選取構件 → 自動生成接頭（板件+螺栓+焊接）
//! Phase A: 端板式、腹板式、底板、螺栓放置、焊接標記、肋板

use crate::app::KolibriApp;
use crate::scene::{MaterialKind, SceneObject, Shape};
use kolibri_core::collision::ComponentKind;
use kolibri_core::steel_connection::*;


use super::steel_conn_helpers::{obj_bounds, obj_center};

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

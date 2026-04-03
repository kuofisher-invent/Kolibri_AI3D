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
        let col_center = self.get_group_center(&col_id);
        let (col_min, col_max) = self.get_group_bounds(&col_id);
        for (i, plate) in conn.plates.iter().enumerate() {
            let plate_name = match plate.plate_type {
                PlateType::EndPlate => format!("{}_plate", name_base),
                PlateType::Stiffener => format!("{}_stiff_{}", name_base, i),
                _ => format!("{}_pl_{}", name_base, i),
            };

            if plate.plate_type == PlateType::Stiffener {
                // 肋板（continuity plate）：水平板在柱內部，對齊梁翼板
                // plate.position[1] = ±bh/2（梁翼板相對於接頭中心的 Y 偏移）
                let stiff_y = conn_pos[1] + plate.position[1];
                // 柱內部範圍：X = col_min..col_max, Z = col_min..col_max
                // 肋板寬 = 柱翼板內淨寬（Z 方向），深 = 柱翼板寬（X 方向）
                let stiff_depth = col_max[0] - col_min[0]; // 柱 X 方向寬
                let stiff_width = col_max[2] - col_min[2]; // 柱 Z 方向深
                let stiff_t = plate.thickness;
                let pos = [col_min[0], stiff_y - stiff_t / 2.0, col_min[2]];
                let id = self.scene.insert_box_raw(
                    plate_name, pos,
                    stiff_depth, stiff_t, stiff_width,
                    MaterialKind::Metal,
                );
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    obj.component_kind = ComponentKind::Plate;
                }
                child_ids.push(id);
            } else {
                // 端板：用原本的座標轉換
                let (pos, bw, bh, bd) = self.calc_plate_world_pos(conn_pos, plate, is_x_dir, sign);
                let id = self.scene.insert_box_raw(plate_name, pos, bw, bh, bd, MaterialKind::Metal);
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    obj.component_kind = ComponentKind::Plate;
                }
                child_ids.push(id);
            }
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
    /// 接合板平行梁腹板(XY平面)、焊在柱翼板面、螺栓沿Z軸穿過接合板+梁腹板
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
        let conn = calc_shear_tab(beam_section, self.editor.conn_bolt_size, self.editor.conn_bolt_grade);

        // 取柱翼板面位置和梁中心
        let (col_min, col_max) = self.get_group_bounds(&col_id);
        let beam_center = self.get_group_center(&beam_id);
        let (beam_min, beam_max) = self.get_group_bounds(&beam_id);
        let (is_x_dir, sign) = self.beam_direction(&beam_id, &col_id);

        self.scene.snapshot();
        let mut child_ids = Vec::new();
        let name_base = self.next_name("ST");

        let tab = &conn.plates[0];
        let tab_w = tab.width;   // 板寬（從柱面向外延伸）
        let tab_h = tab.height;  // 板高（垂直）
        let tab_t = tab.thickness; // 板厚

        // 接合板位置：貼在柱翼板外面，平行梁腹板
        // 梁沿 X 方向：板在 XY 平面，X=柱翼板面起，Z=梁腹板中心
        // 梁沿 Z 方向：板在 ZY 平面，Z=柱翼板面起，X=梁腹板中心
        let beam_web_z = (beam_min[2] + beam_max[2]) / 2.0; // 梁腹板 Z 中心
        let beam_web_x = (beam_min[0] + beam_max[0]) / 2.0; // 梁腹板 X 中心

        let (tab_pos, tab_bw, tab_bh, tab_bd) = if is_x_dir {
            // 梁沿 X：板在 XY 平面
            let x_start = if sign > 0.0 { col_max[0] } else { col_min[0] - tab_w };
            let pos = [x_start, beam_center[1] - tab_h / 2.0, beam_web_z - tab_t / 2.0];
            (pos, tab_w, tab_h, tab_t) // Box: X=寬, Y=高, Z=厚
        } else {
            // 梁沿 Z：板在 ZY 平面
            let z_start = if sign > 0.0 { col_max[2] } else { col_min[2] - tab_w };
            let pos = [beam_web_x - tab_t / 2.0, beam_center[1] - tab_h / 2.0, z_start];
            (pos, tab_t, tab_h, tab_w) // Box: X=厚, Y=高, Z=寬
        };

        // 不穿入檢查
        let beam_flange_gap = 5.0; // 離翼板 5mm 間隙
        let tab_y_min = tab_pos[1];
        let tab_y_max = tab_pos[1] + tab_bh;
        let beam_inner_bot = beam_min[1] + beam_section.3 + beam_flange_gap; // 底翼板內面+間隙
        let beam_inner_top = beam_max[1] - beam_section.3 - beam_flange_gap; // 頂翼板內面-間隙

        self.console_push("CONN", format!(
            "剪力板: 板[{:.0}×{:.0}×{:.0}] pos=[{:.0},{:.0},{:.0}] | 梁內Y=[{:.0}~{:.0}] 板Y=[{:.0}~{:.0}]",
            tab_w, tab_h, tab_t, tab_pos[0], tab_pos[1], tab_pos[2],
            beam_inner_bot, beam_inner_top, tab_y_min, tab_y_max,
        ));

        if tab_y_min < beam_inner_bot || tab_y_max > beam_inner_top {
            self.console_push("WARN", "剪力板高度超出梁翼板範圍，已裁切".into());
        }

        let id = self.scene.insert_box_raw(
            format!("{}_tab", name_base), tab_pos, tab_bw, tab_bh, tab_bd, MaterialKind::Metal,
        );
        if let Some(obj) = self.scene.objects.get_mut(&id) {
            obj.component_kind = ComponentKind::Plate;
        }
        child_ids.push(id);

        // 螺栓：垂直於接合板最大面
        // 板在 XY 平面(梁沿X) → 最薄=Z → 螺栓沿 Z
        // 板在 ZY 平面(梁沿Z) → 最薄=X → 螺栓沿 X
        let bolt_rot = if is_x_dir {
            [std::f32::consts::FRAC_PI_2, 0.0, 0.0] // Y→Z
        } else {
            [0.0, 0.0, std::f32::consts::FRAC_PI_2] // Y→X
        };
        let bolt_dir: [f32; 3] = if is_x_dir {
            [0.0, 0.0, 1.0] // 沿 Z（穿過板+梁腹板）
        } else {
            [1.0, 0.0, 0.0] // 沿 X
        };

        for bg in &conn.bolts {
            let bolt_r = bg.bolt_size.diameter() / 2.0;
            let head_r = bg.bolt_size.head_across_flats() / 2.0;
            let head_t = bg.bolt_size.head_thickness();
            let grip = tab_t + beam_section.2 + 10.0; // 接合板厚 + 梁腹板厚 + 餘量

            for (j, bp) in bg.positions.iter().enumerate() {
                let bolt_name = format!("{}_{}", bg.bolt_size.label(), j + 1);

                // 螺栓中心世界座標（在接合板面上）
                let bolt_center = if is_x_dir {
                    [tab_pos[0] + bp[0], tab_pos[1] + tab_h / 2.0 + bp[1], beam_web_z]
                } else {
                    [beam_web_x, tab_pos[1] + tab_h / 2.0 + bp[1], tab_pos[2] + bp[0]]
                };

                // Cylinder position = 底面圓心，不需 -radius 偏移
                let shank_pos = [bolt_center[0], bolt_center[1] - grip / 2.0, bolt_center[2]];
                let shank_id = self.scene.insert_cylinder_raw(
                    format!("{}_shank", bolt_name), shank_pos,
                    bolt_r, grip, 8, MaterialKind::Metal,
                );
                if let Some(obj) = self.scene.objects.get_mut(&shank_id) {
                    obj.component_kind = ComponentKind::Bolt;
                    obj.rotation_xyz = bolt_rot;
                }
                child_ids.push(shank_id);

                // 螺栓頭：在桿身外端
                let head_offset = grip / 2.0 + head_t / 2.0;
                let head_center = [
                    bolt_center[0] + bolt_dir[0] * head_offset,
                    bolt_center[1] + bolt_dir[1] * head_offset,
                    bolt_center[2] + bolt_dir[2] * head_offset,
                ];
                let head_pos = [head_center[0], head_center[1] - head_t / 2.0, head_center[2]];
                let head_id = self.scene.insert_cylinder_raw(
                    format!("{}_head", bolt_name), head_pos,
                    head_r, head_t, 6, MaterialKind::Metal,
                );
                if let Some(obj) = self.scene.objects.get_mut(&head_id) {
                    obj.component_kind = ComponentKind::Bolt;
                    obj.rotation_xyz = bolt_rot;
                }
                child_ids.push(head_id);
            }
        }

        // 焊接：接合板焊在柱翼板面上（沿 Y 方向全長角焊）
        let weld_x = if is_x_dir {
            if sign > 0.0 { col_max[0] } else { col_min[0] }
        } else { beam_web_x };
        let weld_z = if is_x_dir { beam_web_z } else {
            if sign > 0.0 { col_max[2] } else { col_min[2] }
        };
        let w_start = [weld_x, tab_pos[1], weld_z];
        let w_end = [weld_x, tab_pos[1] + tab_bh, weld_z];
        let weld_size = (tab_t * 0.7).max(6.0);
        let weld_id = self.scene.insert_weld_line(
            format!("{}_weld", name_base), w_start, w_end, weld_size,
        );
        if let Some(obj) = self.scene.objects.get_mut(&weld_id) {
            obj.component_kind = ComponentKind::Weld;
        }
        child_ids.push(weld_id);

        self.scene.create_group(format!("{} 腹板接頭", name_base), child_ids.clone());
        self.scene.version += 1;
        self.editor.selected_ids = child_ids.clone();
        self.console_push("CONN", format!(
            "剪力板: pos=[{:.0},{:.0},{:.0}] 板[{:.0}×{:.0}×{:.0}] 螺栓{}顆",
            tab_pos[0], tab_pos[1], tab_pos[2],
            tab_w, tab_h, tab_t, conn.bolts[0].positions.len(),
        ));
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
        self.console_push("CONN", format!(
            "底板接頭位置: X={:.0} Y={:.0} Z={:.0} | 板 {:.0}×{:.0}×{:.0}mm | 錨栓 {}×{} 顆",
            col_pos[0], col_pos[1], col_pos[2],
            conn.plates[0].width, conn.plates[0].height, conn.plates[0].thickness,
            conn.bolts[0].rows, conn.bolts[0].cols,
        ));
        self.file_message = Some(("底板接頭已建立".into(), std::time::Instant::now()));
        self.check_and_report_connection(&conn, &self.editor.steel_material.clone());
    }



    /// 腹板加厚板（Web Doubler Plate）— AISC J10.6
    /// 焊接於柱腹板面板區，增加抗剪強度
    pub(crate) fn create_web_doubler_connection(&mut self) {
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
        let col_section = self.get_member_section(&col_id);
        let conn = kolibri_core::steel_connection::calc_web_doubler(
            beam_section, col_section, None,
        );

        // 加厚板位置：柱腹板中心，Y = 梁中心高度
        let (col_min, col_max) = self.get_group_bounds(&col_id);
        let beam_center = self.get_group_center(&beam_id);
        let col_center = self.get_group_center(&col_id);
        let (is_x_dir, _sign) = self.beam_direction(&beam_id, &col_id);

        self.scene.snapshot();
        let mut child_ids = Vec::new();
        let name_base = self.next_name("WD");

        let plate = &conn.plates[0];
        let plate_w = plate.width;   // panel zone 寬度
        let plate_h = plate.height;  // 柱翼板間淨高
        let plate_t = plate.thickness;

        // 加厚板貼在柱腹板側面
        // 梁沿 X 方向時：加厚板在 XY 平面，Z = 柱腹板中心 + 偏移
        // 梁沿 Z 方向時：加厚板在 ZY 平面，X = 柱腹板中心 + 偏移
        let ctw = col_section.2; // 柱腹板厚
        let pos = if is_x_dir {
            // 加厚板在柱腹板 Z 方向外側
            [
                col_center[0] - plate_w / 2.0,
                beam_center[1] - plate_h / 2.0,
                col_center[2] + ctw / 2.0,
            ]
        } else {
            [
                col_center[0] + ctw / 2.0,
                beam_center[1] - plate_h / 2.0,
                col_center[2] - plate_w / 2.0,
            ]
        };

        let (bw, bh, bd) = if is_x_dir {
            (plate_w, plate_h, plate_t)
        } else {
            (plate_t, plate_h, plate_w)
        };

        let id = self.scene.insert_box_raw(
            format!("{}_doubler", name_base), pos, bw, bh, bd, MaterialKind::Metal,
        );
        if let Some(obj) = self.scene.objects.get_mut(&id) {
            obj.component_kind = ComponentKind::Plate;
        }
        child_ids.push(id);

        // 焊接標記（四邊角焊）
        for (i, weld) in conn.welds.iter().enumerate() {
            let weld_name = format!("{}_weld_{}", name_base, i);
            let w_start = if is_x_dir {
                [pos[0] + plate_w / 2.0 + weld.start[0], pos[1] + plate_h / 2.0 + weld.start[1], pos[2]]
            } else {
                [pos[0], pos[1] + plate_h / 2.0 + weld.start[1], pos[2] + plate_w / 2.0 + weld.start[0]]
            };
            let w_end = if is_x_dir {
                [pos[0] + plate_w / 2.0 + weld.end[0], pos[1] + plate_h / 2.0 + weld.end[1], pos[2]]
            } else {
                [pos[0], pos[1] + plate_h / 2.0 + weld.end[1], pos[2] + plate_w / 2.0 + weld.end[0]]
            };
            let wid = self.scene.insert_weld_line(weld_name, w_start, w_end, weld.size);
            if let Some(obj) = self.scene.objects.get_mut(&wid) {
                obj.component_kind = ComponentKind::Weld;
            }
            child_ids.push(wid);
        }

        self.scene.create_group(format!("{} 腹板加厚板", name_base), child_ids.clone());
        self.scene.version += 1;
        self.editor.selected_ids = child_ids.clone();
        self.console_push("CONN", format!(
            "腹板加厚板: {:.0}×{:.0}×{:.0}mm | 四邊角焊 {}mm",
            plate_w, plate_h, plate_t, conn.welds.first().map_or(0.0, |w| w.size),
        ));
        self.file_message = Some(("腹板加厚板已建立 (AISC J10.6)".into(), std::time::Instant::now()));
        self.check_and_report_connection(&conn, &self.editor.steel_material.clone());
    }

    /// 雙角鋼接頭（Double Angle / Framed Connection）
    /// 兩片角鋼夾住梁腹板，螺栓穿過梁腹板+角鋼，另一肢鎖柱翼板
    pub(crate) fn create_double_angle_connection(&mut self) {
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
        let col_section = self.get_member_section(&col_id);
        let params = kolibri_core::steel_connection::DoubleAngleParams {
            beam_section,
            col_section,
            bolt_size: self.editor.conn_bolt_size,
            bolt_grade: self.editor.conn_bolt_grade,
            angle_leg: None,
            angle_thickness: None,
        };
        let conn = kolibri_core::steel_connection::calc_double_angle(&params);

        let (col_min, col_max) = self.get_group_bounds(&col_id);
        let beam_center = self.get_group_center(&beam_id);
        let (beam_min, beam_max) = self.get_group_bounds(&beam_id);
        let (is_x_dir, sign) = self.beam_direction(&beam_id, &col_id);

        self.scene.snapshot();
        let mut child_ids = Vec::new();
        let name_base = self.next_name("DA");

        let btw = beam_section.2; // 梁腹板厚
        let beam_web_z = (beam_min[2] + beam_max[2]) / 2.0;
        let beam_web_x = (beam_min[0] + beam_max[0]) / 2.0;

        // 角鋼尺寸（從 conn.plates 取）
        let angle_h = conn.plates[0].height;
        let angle_leg = conn.plates[0].width;
        let angle_t = conn.plates[0].thickness;

        // 角鋼垂直肢：貼在梁腹板兩側，從柱翼板面延伸
        // 水平肢：垂直於垂直肢，貼在柱翼板上
        let col_face = if is_x_dir {
            if sign > 0.0 { col_max[0] } else { col_min[0] }
        } else {
            if sign > 0.0 { col_max[2] } else { col_min[2] }
        };

        // ── 生成 4 片角鋼板件 ──
        // 垂直肢（左/右各一片，夾住梁腹板）
        for side in [-1.0_f32, 1.0] {
            let vert_name = format!("{}_vert_{}", name_base, if side < 0.0 { "L" } else { "R" });

            let pos = if is_x_dir {
                let x_start = if sign > 0.0 { col_face } else { col_face - angle_leg };
                let z = beam_web_z + side * (btw / 2.0);
                [x_start, beam_center[1] - angle_h / 2.0, z]
            } else {
                let z_start = if sign > 0.0 { col_face } else { col_face - angle_leg };
                let x = beam_web_x + side * (btw / 2.0);
                [x, beam_center[1] - angle_h / 2.0, z_start]
            };

            let (bw, bh, bd) = if is_x_dir {
                (angle_leg, angle_h, angle_t) // X=肢寬, Y=高, Z=厚
            } else {
                (angle_t, angle_h, angle_leg) // X=厚, Y=高, Z=肢寬
            };

            let id = self.scene.insert_box_raw(vert_name, pos, bw, bh, bd, MaterialKind::Metal);
            if let Some(obj) = self.scene.objects.get_mut(&id) {
                obj.component_kind = ComponentKind::Plate;
            }
            child_ids.push(id);
        }

        // 水平肢（左/右各一片，貼柱翼板面）
        for side in [-1.0_f32, 1.0] {
            let horiz_name = format!("{}_horiz_{}", name_base, if side < 0.0 { "L" } else { "R" });

            let pos = if is_x_dir {
                let x = col_face - if sign > 0.0 { 0.0 } else { angle_t };
                let z = beam_web_z + side * (btw / 2.0 + angle_t);
                let z_start = if side < 0.0 { z - angle_leg + angle_t } else { z };
                [x, beam_center[1] - angle_h / 2.0, z_start]
            } else {
                let z = col_face - if sign > 0.0 { 0.0 } else { angle_t };
                let x = beam_web_x + side * (btw / 2.0 + angle_t);
                let x_start = if side < 0.0 { x - angle_leg + angle_t } else { x };
                [x_start, beam_center[1] - angle_h / 2.0, z]
            };

            let (bw, bh, bd) = if is_x_dir {
                (angle_t, angle_h, angle_leg) // X=厚, Y=高, Z=肢寬
            } else {
                (angle_leg, angle_h, angle_t) // X=肢寬, Y=高, Z=厚
            };

            let id = self.scene.insert_box_raw(horiz_name, pos, bw, bh, bd, MaterialKind::Metal);
            if let Some(obj) = self.scene.objects.get_mut(&id) {
                obj.component_kind = ComponentKind::Plate;
            }
            child_ids.push(id);
        }

        // ── 生成螺栓（梁腹板側 — 穿過角鋼+梁腹板）──
        let web_bg = &conn.bolts[0];
        let bolt_r = web_bg.bolt_size.diameter() / 2.0;
        let head_r = web_bg.bolt_size.head_across_flats() / 2.0;
        let head_t = web_bg.bolt_size.head_thickness();
        let grip = angle_t * 2.0 + btw + 5.0; // 左角鋼 + 梁腹板 + 右角鋼 + 餘量

        let bolt_rot = if is_x_dir {
            [std::f32::consts::FRAC_PI_2, 0.0, 0.0] // 螺栓沿 Z
        } else {
            [0.0, 0.0, std::f32::consts::FRAC_PI_2] // 螺栓沿 X
        };

        for (j, bp) in web_bg.positions.iter().enumerate() {
            let bolt_name = format!("{}_wb_{}", name_base, j + 1);

            let bolt_center = if is_x_dir {
                [col_face + sign * bp[0], beam_center[1] + bp[1], beam_web_z]
            } else {
                [beam_web_x, beam_center[1] + bp[1], col_face + sign * bp[0]]
            };

            // 螺栓桿
            let shank_pos = [bolt_center[0], bolt_center[1] - bolt_r, bolt_center[2]];
            let shank_id = self.scene.insert_cylinder_raw(
                format!("{}_shank", bolt_name), shank_pos,
                bolt_r, grip, 8, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&shank_id) {
                obj.component_kind = ComponentKind::Bolt;
                obj.rotation_xyz = bolt_rot;
            }
            child_ids.push(shank_id);

            // 螺栓頭
            let head_pos = [bolt_center[0], bolt_center[1] - head_r, bolt_center[2]];
            let head_id = self.scene.insert_cylinder_raw(
                format!("{}_head", bolt_name), head_pos,
                head_r, head_t, 6, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&head_id) {
                obj.component_kind = ComponentKind::Bolt;
                obj.rotation_xyz = bolt_rot;
            }
            child_ids.push(head_id);
        }

        // ── 柱翼板側螺栓 ──
        if conn.bolts.len() > 1 {
            let col_bg = &conn.bolts[1];
            let col_grip = angle_t + col_section.3 + 5.0; // 角鋼水平肢 + 柱翼板 + 餘量

            for (j, bp) in col_bg.positions.iter().enumerate() {
                let bolt_name = format!("{}_cb_{}", name_base, j + 1);

                // 柱側螺栓沿梁方向（穿過水平肢+柱翼板）
                let bolt_center = if is_x_dir {
                    [col_face, beam_center[1] + bp[1], beam_web_z + bp[2]]
                } else {
                    [beam_web_x + bp[2], beam_center[1] + bp[1], col_face]
                };

                let col_bolt_rot = if is_x_dir {
                    [0.0, 0.0, std::f32::consts::FRAC_PI_2] // 沿 X
                } else {
                    [std::f32::consts::FRAC_PI_2, 0.0, 0.0] // 沿 Z
                };

                let shank_pos = [bolt_center[0], bolt_center[1] - bolt_r, bolt_center[2]];
                let shank_id = self.scene.insert_cylinder_raw(
                    format!("{}_shank", bolt_name), shank_pos,
                    bolt_r, col_grip, 8, MaterialKind::Metal,
                );
                if let Some(obj) = self.scene.objects.get_mut(&shank_id) {
                    obj.component_kind = ComponentKind::Bolt;
                    obj.rotation_xyz = col_bolt_rot;
                }
                child_ids.push(shank_id);

                let head_pos = [bolt_center[0], bolt_center[1] - head_r, bolt_center[2]];
                let head_id = self.scene.insert_cylinder_raw(
                    format!("{}_head", bolt_name), head_pos,
                    head_r, head_t, 6, MaterialKind::Metal,
                );
                if let Some(obj) = self.scene.objects.get_mut(&head_id) {
                    obj.component_kind = ComponentKind::Bolt;
                    obj.rotation_xyz = col_bolt_rot;
                }
                child_ids.push(head_id);
            }
        }

        self.scene.create_group(format!("{} 雙角鋼接頭", name_base), child_ids.clone());
        self.scene.version += 1;
        self.editor.selected_ids = child_ids.clone();

        let total_bolts = conn.bolts.iter().map(|b| b.positions.len()).sum::<usize>();
        self.console_push("CONN", format!(
            "雙角鋼接頭: L{:.0}×{:.0}×{:.0} h={:.0}mm | 螺栓 {} 顆（梁側{}+柱側{}）",
            angle_leg, angle_leg, angle_t, angle_h,
            total_bolts, conn.bolts[0].positions.len(),
            conn.bolts.get(1).map_or(0, |b| b.positions.len()),
        ));
        self.file_message = Some(("雙角鋼接頭已建立 (AISC Table 10-1)".into(), std::time::Instant::now()));
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
                    ConnectionType::WebDoubler => self.create_web_doubler_connection(),
                    ConnectionType::DoubleAngle => self.create_double_angle_connection(),
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

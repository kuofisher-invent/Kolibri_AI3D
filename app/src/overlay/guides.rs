//! Guide overlays: floating material picker, push/pull reference lines, protractor

use eframe::egui;

use crate::app::{KolibriApp, CursorHint, DrawState, PullFace, RenderMode, ScaleHandle, SelectionMode, SnapType, Tool, WorkMode};
use crate::scene::Shape;

/// 3D 量角器上一點的世界座標
/// center: 旋轉中心, r: 半徑, angle: 弧度, axis: 0=X 1=Y 2=Z
fn protractor_3d_point(center: [f32; 3], r: f32, angle: f32, axis: u8) -> [f32; 3] {
    let (s, c) = angle.sin_cos();
    match axis {
        0 => [center[0], center[1] + r * c, center[2] + r * s],       // X軸: YZ平面
        2 => [center[0] + r * c, center[1] + r * s, center[2]],       // Z軸: XY平面
        _ => [center[0] + r * c, center[1], center[2] + r * s],       // Y軸: XZ平面（地面）
    }
}

impl KolibriApp {
    /// Draw guide overlays (material picker, push/pull ref, protractor)
    pub(crate) fn draw_guide_overlays(
        &mut self,
        ui: &mut egui::Ui,
        vp: glam::Mat4,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
                // ── Floating material picker for PaintBucket ──
                if self.editor.tool == Tool::PaintBucket {
                    let swatch = 32.0_f32;
                    let gap = 4.0_f32;
                    let cols = 8_usize;
                    let all_mats = crate::scene::MaterialKind::ALL;
                    let rows = (all_mats.len() + cols - 1) / cols;
                    let panel_w = cols as f32 * (swatch + gap) + gap + 16.0;
                    let panel_h = rows as f32 * (swatch + gap) + gap + 36.0;
                    let panel_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.center().x - panel_w / 2.0, rect.bottom() - panel_h - 50.0),
                        egui::vec2(panel_w, panel_h),
                    );
                    ui.painter().rect_filled(panel_rect, 16.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 235));
                    ui.painter().rect_stroke(panel_rect, 16.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));
                    // Title
                    ui.painter().text(
                        egui::pos2(panel_rect.center().x, panel_rect.top() + 14.0),
                        egui::Align2::CENTER_CENTER,
                        format!("油漆桶 — 目前: {}", self.create_mat.label()),
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgb(110, 118, 135),
                    );
                    // Swatches
                    let start_x = panel_rect.left() + 8.0 + gap;
                    let start_y = panel_rect.top() + 28.0;
                    for (i, mat) in all_mats.iter().enumerate() {
                        let row = i / cols;
                        let col = i % cols;
                        let sx = start_x + col as f32 * (swatch + gap);
                        let sy = start_y + row as f32 * (swatch + gap);
                        let sr = egui::Rect::from_min_size(egui::pos2(sx, sy), egui::vec2(swatch, swatch));
                        let resp = ui.allocate_rect(sr, egui::Sense::click());
                        crate::panels::draw_material_swatch(
                            ui.painter(), sr, mat,
                            self.create_mat == *mat,
                            resp.hovered(),
                        );
                        if resp.clicked() {
                            self.create_mat = *mat;
                        }
                        resp.on_hover_text(mat.label());
                    }
                }

                // ── C3: Push/Pull reference dashed lines ──
                if self.editor.selected_face.is_some() && self.editor.drag_snapshot_taken {
                    if let Some((obj_id, _face)) = self.editor.selected_face.clone() {
                        if let (Some(orig_pos), Some(orig_dims), Some(obj)) = (
                            self.editor.pull_original_pos,
                            self.editor.pull_original_dims,
                            self.scene.objects.get(&obj_id),
                        ) {
                            if let Shape::Box { width, height, depth } = &obj.shape {
                                // 4 corners of the pulled face — compute in original and current positions
                                let ow = orig_dims[0];
                                let oh = orig_dims[1];
                                let od = orig_dims[2];
                                let op = orig_pos;
                                let cp = obj.position;
                                let cw = *width;
                                let ch = *height;
                                let cd = *depth;

                                // Draw lines from each original corner to current corner
                                let orig_corners = [
                                    [op[0], op[1], op[2]],
                                    [op[0]+ow, op[1], op[2]],
                                    [op[0]+ow, op[1]+oh, op[2]],
                                    [op[0], op[1]+oh, op[2]],
                                    [op[0], op[1], op[2]+od],
                                    [op[0]+ow, op[1], op[2]+od],
                                    [op[0]+ow, op[1]+oh, op[2]+od],
                                    [op[0], op[1]+oh, op[2]+od],
                                ];
                                let curr_corners = [
                                    [cp[0], cp[1], cp[2]],
                                    [cp[0]+cw, cp[1], cp[2]],
                                    [cp[0]+cw, cp[1]+ch, cp[2]],
                                    [cp[0], cp[1]+ch, cp[2]],
                                    [cp[0], cp[1], cp[2]+cd],
                                    [cp[0]+cw, cp[1], cp[2]+cd],
                                    [cp[0]+cw, cp[1]+ch, cp[2]+cd],
                                    [cp[0], cp[1]+ch, cp[2]+cd],
                                ];
                                let dash_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(200, 100, 100, 150));
                                for i in 0..8 {
                                    if let (Some(s1), Some(s2)) = (
                                        Self::world_to_screen_vp(orig_corners[i], &vp, &rect),
                                        Self::world_to_screen_vp(curr_corners[i], &vp, &rect),
                                    ) {
                                        let dist = ((s2.x-s1.x).powi(2) + (s2.y-s1.y).powi(2)).sqrt();
                                        if dist > 3.0 {
                                            super::draw_dashed_line(ui.painter(), s1, s2, dash_stroke, 6.0, 4.0);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ── D1: Protractor overlay during Rotate (3-step) ──
                // SU 風格 3D 旋轉盤 — 根據旋轉軸正交擺放
                // Step 1 (RotateRef): 3D 量角器 + 虛線到滑鼠
                if let DrawState::RotateRef { center, rotate_axis, .. } = &self.editor.draw_state {
                    let axis = *rotate_axis;
                    let world_r = 800.0; // 世界空間半徑 (mm)
                    let segments = 48;
                    let axis_color = match axis {
                        0 => egui::Color32::from_rgba_unmultiplied(220, 60, 60, 140),  // X=紅
                        2 => egui::Color32::from_rgba_unmultiplied(60, 60, 220, 140),  // Z=藍
                        _ => egui::Color32::from_rgba_unmultiplied(60, 180, 60, 140),  // Y=綠
                    };

                    // 3D 圓盤：在旋轉軸的正交平面上畫圓
                    for i in 0..segments {
                        let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                        let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
                        // 圓上兩點的 3D 世界座標
                        let p0 = protractor_3d_point(*center, world_r, a0, axis);
                        let p1 = protractor_3d_point(*center, world_r, a1, axis);
                        if let (Some(s0), Some(s1)) = (
                            Self::world_to_screen_vp(p0, &vp, &rect),
                            Self::world_to_screen_vp(p1, &vp, &rect),
                        ) {
                            ui.painter().line_segment([s0, s1], egui::Stroke::new(1.5, axis_color));
                        }
                    }
                    // 15° 刻度線
                    for tick in 0..24 {
                        let angle = tick as f32 * 15.0_f32.to_radians();
                        let inner = protractor_3d_point(*center, world_r * 0.92, angle, axis);
                        let outer = protractor_3d_point(*center, world_r * (if tick % 6 == 0 { 1.08 } else { 1.04 }), angle, axis);
                        if let (Some(si), Some(so)) = (
                            Self::world_to_screen_vp(inner, &vp, &rect),
                            Self::world_to_screen_vp(outer, &vp, &rect),
                        ) {
                            let tc = if tick % 6 == 0 { axis_color } else {
                                egui::Color32::from_rgba_unmultiplied(axis_color.r(), axis_color.g(), axis_color.b(), 60)
                            };
                            ui.painter().line_segment([si, so], egui::Stroke::new(1.0, tc));
                        }
                    }
                    // 中心十字
                    if let Some(sc) = Self::world_to_screen_vp(*center, &vp, &rect) {
                        let cs = 6.0;
                        ui.painter().line_segment([egui::pos2(sc.x-cs,sc.y),egui::pos2(sc.x+cs,sc.y)], egui::Stroke::new(2.0, axis_color));
                        ui.painter().line_segment([egui::pos2(sc.x,sc.y-cs),egui::pos2(sc.x,sc.y+cs)], egui::Stroke::new(2.0, axis_color));
                        // 虛線到滑鼠
                        let mouse = egui::pos2(self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                        super::draw_dashed_line(ui.painter(), sc, mouse,
                            egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(200, 200, 200, 150)), 6.0, 4.0);
                        // 軸名稱
                        let axis_name = ["X (紅)", "Y (綠)", "Z (藍)"][axis.min(2) as usize];
                        ui.painter().text(egui::pos2(sc.x + 15.0, sc.y - 15.0),
                            egui::Align2::LEFT_CENTER, format!("旋轉軸: {} | ←→↑切換", axis_name),
                            egui::FontId::proportional(11.0), axis_color);
                    }
                }
                // Step 2 (RotateAngle): 3D 量角器 + 掃過弧 + 角度標籤
                if let DrawState::RotateAngle { center, ref_angle, current_angle, rotate_axis, .. } = &self.editor.draw_state {
                    let axis = *rotate_axis;
                    let world_r = 800.0;
                    let segments = 48;
                    let delta = current_angle - ref_angle;
                    let axis_color = match axis {
                        0 => egui::Color32::from_rgba_unmultiplied(220, 60, 60, 120),
                        2 => egui::Color32::from_rgba_unmultiplied(60, 60, 220, 120),
                        _ => egui::Color32::from_rgba_unmultiplied(60, 180, 60, 120),
                    };

                    // 3D 圓盤
                    for i in 0..segments {
                        let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                        let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
                        let p0 = protractor_3d_point(*center, world_r, a0, axis);
                        let p1 = protractor_3d_point(*center, world_r, a1, axis);
                        if let (Some(s0), Some(s1)) = (
                            Self::world_to_screen_vp(p0, &vp, &rect),
                            Self::world_to_screen_vp(p1, &vp, &rect),
                        ) {
                            ui.painter().line_segment([s0, s1], egui::Stroke::new(1.5, axis_color));
                        }
                    }
                    // 掃過弧（填色）
                    let arc_segments = (delta.abs() / std::f32::consts::TAU * 48.0).max(2.0) as usize;
                    let arc_color = egui::Color32::from_rgba_unmultiplied(axis_color.r(), axis_color.g(), axis_color.b(), 60);
                    if let Some(sc) = Self::world_to_screen_vp(*center, &vp, &rect) {
                        for i in 0..arc_segments {
                            let t0 = i as f32 / arc_segments as f32;
                            let t1 = (i + 1) as f32 / arc_segments as f32;
                            let a0 = *ref_angle + delta * t0;
                            let a1 = *ref_angle + delta * t1;
                            let p0 = protractor_3d_point(*center, world_r, a0, axis);
                            let p1 = protractor_3d_point(*center, world_r, a1, axis);
                            if let (Some(s0), Some(s1)) = (
                                Self::world_to_screen_vp(p0, &vp, &rect),
                                Self::world_to_screen_vp(p1, &vp, &rect),
                            ) {
                                // 三角形扇形
                                ui.painter().add(egui::Shape::convex_polygon(
                                    vec![sc, s0, s1],
                                    arc_color,
                                    egui::Stroke::NONE,
                                ));
                                ui.painter().line_segment([s0, s1], egui::Stroke::new(2.0, axis_color));
                            }
                        }
                        // 參考線（灰白）
                        let ref_end = protractor_3d_point(*center, world_r, *ref_angle, axis);
                        if let Some(sr) = Self::world_to_screen_vp(ref_end, &vp, &rect) {
                            ui.painter().line_segment([sc, sr],
                                egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(200, 200, 200, 180)));
                        }
                        // 目標線（藍色）
                        let mouse = egui::pos2(self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                        ui.painter().line_segment([sc, mouse],
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(76, 139, 245)),
                        );
                        // 角度標籤
                        let delta_deg = delta.to_degrees();
                        let snap_deg = (delta_deg / 15.0).round() * 15.0;
                        let is_snapped = (delta_deg - snap_deg).abs() < 3.0;
                        let label = if is_snapped {
                            format!("{:.0}\u{00b0} \u{25cf}", snap_deg)
                        } else {
                            format!("{:.1}\u{00b0}", delta_deg)
                        };
                        let label_color = if is_snapped {
                            egui::Color32::from_rgb(60, 200, 60)
                        } else { axis_color };
                        let axis_name = ["X","Y","Z"][axis.min(2) as usize];
                        ui.painter().text(
                            egui::pos2(sc.x + 15.0, sc.y - 15.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{} ({}軸)", label, axis_name),
                            egui::FontId::proportional(if is_snapped { 15.0 } else { 13.0 }),
                            label_color,
                        );
                    }
                }
    }
}

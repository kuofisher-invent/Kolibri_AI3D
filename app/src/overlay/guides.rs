//! Guide overlays: floating material picker, push/pull reference lines, protractor

use eframe::egui;

use crate::app::{KolibriApp, CursorHint, DrawState, PullFace, RenderMode, ScaleHandle, SelectionMode, SnapType, Tool, WorkMode};
use crate::scene::Shape;

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
                // Step 1 (RotateRef): 量角器跟隨中心，虛線延伸到滑鼠
                if let DrawState::RotateRef { center, .. } = &self.editor.draw_state {
                    if let Some(sc) = Self::world_to_screen_vp(*center, &vp, &rect) {
                        let radius = 70.0;
                        let segments = 48;
                        // 量角器圓
                        let circle_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(76, 139, 245, 120));
                        for i in 0..segments {
                            let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                            let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
                            ui.painter().line_segment(
                                [egui::pos2(sc.x + radius * a0.cos(), sc.y + radius * a0.sin()),
                                 egui::pos2(sc.x + radius * a1.cos(), sc.y + radius * a1.sin())],
                                circle_stroke,
                            );
                        }
                        // 15° 刻度
                        for tick in 0..24 {
                            let angle = tick as f32 * 15.0_f32.to_radians();
                            let inner_r = radius - 4.0;
                            let outer_r = if tick % 6 == 0 { radius + 6.0 } else { radius + 3.0 };
                            let tick_color = if tick % 6 == 0 {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 180)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 60)
                            };
                            ui.painter().line_segment(
                                [egui::pos2(sc.x + inner_r * angle.cos(), sc.y + inner_r * angle.sin()),
                                 egui::pos2(sc.x + outer_r * angle.cos(), sc.y + outer_r * angle.sin())],
                                egui::Stroke::new(1.0, tick_color),
                            );
                        }
                        // 虛線到滑鼠位置（reference preview）
                        let mouse = egui::pos2(self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                        super::draw_dashed_line(ui.painter(), sc, mouse,
                            egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(200, 200, 200, 150)),
                            6.0, 4.0);
                        // 中心十字
                        let cs = 6.0;
                        let cc = egui::Color32::from_rgb(76, 139, 245);
                        ui.painter().line_segment([egui::pos2(sc.x - cs, sc.y), egui::pos2(sc.x + cs, sc.y)], egui::Stroke::new(2.0, cc));
                        ui.painter().line_segment([egui::pos2(sc.x, sc.y - cs), egui::pos2(sc.x, sc.y + cs)], egui::Stroke::new(2.0, cc));
                        // 提示文字
                        ui.painter().text(
                            egui::pos2(sc.x + radius + 10.0, sc.y - 10.0),
                            egui::Align2::LEFT_CENTER,
                            "設定參考方向",
                            egui::FontId::proportional(12.0),
                            egui::Color32::from_rgb(76, 139, 245),
                        );
                    }
                }
                // Step 2 (RotateAngle): 量角器 + 參考線 + 掃過弧 + 角度標籤
                if let DrawState::RotateAngle { center, ref_angle, current_angle, .. } = &self.editor.draw_state {
                    if let Some(sc) = Self::world_to_screen_vp(*center, &vp, &rect) {
                        let radius = 70.0;
                        let segments = 48;
                        let delta = current_angle - ref_angle;

                        // 量角器圓
                        let circle_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(76, 139, 245, 100));
                        for i in 0..segments {
                            let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                            let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
                            ui.painter().line_segment(
                                [egui::pos2(sc.x + radius * a0.cos(), sc.y + radius * a0.sin()),
                                 egui::pos2(sc.x + radius * a1.cos(), sc.y + radius * a1.sin())],
                                circle_stroke,
                            );
                        }
                        // 15° 刻度
                        for tick in 0..24 {
                            let angle = tick as f32 * 15.0_f32.to_radians();
                            let inner_r = radius - 4.0;
                            let outer_r = if tick % 6 == 0 { radius + 6.0 } else { radius + 3.0 };
                            let tick_color = if tick % 6 == 0 {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 180)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 60)
                            };
                            ui.painter().line_segment(
                                [egui::pos2(sc.x + inner_r * angle.cos(), sc.y + inner_r * angle.sin()),
                                 egui::pos2(sc.x + outer_r * angle.cos(), sc.y + outer_r * angle.sin())],
                                egui::Stroke::new(1.0, tick_color),
                            );
                        }
                        // 參考線（實線，灰白）— 從 center 沿 ref_angle 方向
                        // 注意：世界空間的 atan2(dz, dx) 需要投影到螢幕空間
                        // 簡化：用 ref_angle 在螢幕上畫（XZ 平面對應螢幕 X 方向）
                        let ref_end = egui::pos2(sc.x + radius * ref_angle.cos(), sc.y - radius * ref_angle.sin());
                        ui.painter().line_segment(
                            [sc, ref_end],
                            egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(200, 200, 200, 180)),
                        );
                        // 目標線（實線，藍色）— 從 center 到滑鼠
                        let mouse = egui::pos2(self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                        ui.painter().line_segment(
                            [sc, mouse],
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(76, 139, 245)),
                        );
                        // 掃過弧
                        if delta.abs() > 0.001 {
                            let arc_segments = 32;
                            let arc_stroke = egui::Stroke::new(3.0, egui::Color32::from_rgba_unmultiplied(76, 139, 245, 200));
                            for i in 0..arc_segments {
                                let t0 = ref_angle + delta * (i as f32 / arc_segments as f32);
                                let t1 = ref_angle + delta * ((i + 1) as f32 / arc_segments as f32);
                                ui.painter().line_segment(
                                    [egui::pos2(sc.x + radius * t0.cos(), sc.y - radius * t0.sin()),
                                     egui::pos2(sc.x + radius * t1.cos(), sc.y - radius * t1.sin())],
                                    arc_stroke,
                                );
                            }
                        }
                        // 中心十字
                        let cs = 6.0;
                        let cc = egui::Color32::from_rgb(76, 139, 245);
                        ui.painter().line_segment([egui::pos2(sc.x - cs, sc.y), egui::pos2(sc.x + cs, sc.y)], egui::Stroke::new(2.0, cc));
                        ui.painter().line_segment([egui::pos2(sc.x, sc.y - cs), egui::pos2(sc.x, sc.y + cs)], egui::Stroke::new(2.0, cc));
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
                        } else {
                            egui::Color32::from_rgb(76, 139, 245)
                        };
                        ui.painter().text(
                            egui::pos2(sc.x + radius + 10.0, sc.y - 10.0),
                            egui::Align2::LEFT_CENTER,
                            &label,
                            egui::FontId::proportional(if is_snapped { 15.0 } else { 13.0 }),
                            label_color,
                        );
                    }
                }
    }
}

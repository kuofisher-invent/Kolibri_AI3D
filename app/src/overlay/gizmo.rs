//! Gizmo overlays: selection outline, object pivot, move gizmo, scale handles

use eframe::egui;

use crate::app::{KolibriApp, CursorHint, DrawState, PullFace, RenderMode, ScaleHandle, SelectionMode, SnapType, Tool, WorkMode};
use crate::scene::Shape;

impl KolibriApp {
    /// Draw gizmo overlays (selection outline, pivot, move gizmo, scale handles)
    pub(crate) fn draw_gizmo_overlays(
        &mut self,
        ui: &mut egui::Ui,
        vp: glam::Mat4,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
                // ── Selection outline（螢幕空間 AABB 描邊）──
                // 群組物件合併成一個大 bounding box，不分開畫
                {
                    let mut min_s = egui::pos2(f32::MAX, f32::MAX);
                    let mut max_s = egui::pos2(f32::MIN, f32::MIN);
                    let mut visible = 0u32;
                    // 3D 世界空間的整體 AABB（用於尺寸標籤）
                    let mut world_min = [f32::MAX; 3];
                    let mut world_max = [f32::MIN; 3];

                    for sel_id in &self.editor.selected_ids {
                        if let Some(obj) = self.scene.objects.get(sel_id) {
                            // 使用旋轉感知的角落計算
                            let corners = crate::tools::steel_conn_helpers::rotated_obj_corners(obj);
                            for c in &corners {
                                for i in 0..3 {
                                    world_min[i] = world_min[i].min(c[i]);
                                    world_max[i] = world_max[i].max(c[i]);
                                }
                                if let Some(sp) = Self::world_to_screen_vp(*c, &vp, &rect) {
                                    min_s.x = min_s.x.min(sp.x); min_s.y = min_s.y.min(sp.y);
                                    max_s.x = max_s.x.max(sp.x); max_s.y = max_s.y.max(sp.y);
                                    visible += 1;
                                }
                            }
                        }
                    }
                    if visible >= 2 {
                        let outline_rect = egui::Rect::from_min_max(min_s, max_s).expand(3.0);
                        ui.painter().rect_stroke(outline_rect, 4.0,
                            egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(76, 139, 245, 140)));
                        // 尺寸標籤（整體 AABB 尺寸）
                        let dim_color = egui::Color32::from_rgba_unmultiplied(76, 139, 245, 200);
                        let dim_font = egui::FontId::proportional(9.0);
                        let fmt = |v: f32| if v >= 1000.0 { format!("{:.2}m", v/1000.0) } else { format!("{:.0}", v) };
                        let total_w = world_max[0] - world_min[0];
                        let total_h = world_max[1] - world_min[1];
                        ui.painter().text(
                            egui::pos2(outline_rect.center().x, outline_rect.max.y + 10.0),
                            egui::Align2::CENTER_TOP, &fmt(total_w),
                            dim_font.clone(), dim_color);
                        ui.painter().text(
                            egui::pos2(outline_rect.max.x + 8.0, outline_rect.center().y),
                            egui::Align2::LEFT_CENTER, &fmt(total_h),
                            dim_font, dim_color);
                    }
                }

                // ── Object center pivot（選取物件中心十字）──
                if self.editor.selected_ids.len() == 1 {
                    if let Some(obj) = self.editor.selected_ids.first()
                        .and_then(|id| self.scene.objects.get(id))
                    {
                        let center = match &obj.shape {
                            Shape::Box { width, height, depth } =>
                                [obj.position[0]+width/2.0, obj.position[1]+height/2.0, obj.position[2]+depth/2.0],
                            Shape::Cylinder { radius, height, .. } =>
                                [obj.position[0]+radius, obj.position[1]+height/2.0, obj.position[2]+radius],
                            Shape::Sphere { radius, .. } =>
                                [obj.position[0]+radius, obj.position[1]+radius, obj.position[2]+radius],
                            _ => obj.position,
                        };
                        if let Some(sc) = Self::world_to_screen_vp(center, &vp, &rect) {
                            let cs = 5.0;
                            let pivot_color = egui::Color32::from_rgba_unmultiplied(255, 200, 60, 200);
                            ui.painter().line_segment(
                                [egui::pos2(sc.x-cs, sc.y), egui::pos2(sc.x+cs, sc.y)],
                                egui::Stroke::new(1.5, pivot_color));
                            ui.painter().line_segment(
                                [egui::pos2(sc.x, sc.y-cs), egui::pos2(sc.x, sc.y+cs)],
                                egui::Stroke::new(1.5, pivot_color));
                            ui.painter().circle_stroke(sc, 3.0,
                                egui::Stroke::new(1.0, pivot_color));
                        }
                    }
                }

                // ── Move gizmo: 3D XYZ arrows with interactive hover/drag ──
                if (self.editor.tool == Tool::Move || self.editor.tool == Tool::Select)
                    && !self.editor.selected_ids.is_empty()
                    && matches!(self.editor.draw_state, DrawState::Idle)
                {
                    if let Some(obj) = self.editor.selected_ids.first()
                        .and_then(|id| self.scene.objects.get(id))
                    {
                        let center = match &obj.shape {
                            Shape::Box { width, height, depth } =>
                                [obj.position[0] + width / 2.0, obj.position[1] + height / 2.0, obj.position[2] + depth / 2.0],
                            Shape::Cylinder { radius, height, .. } =>
                                [obj.position[0] + radius, obj.position[1] + height / 2.0, obj.position[2] + radius],
                            Shape::Sphere { radius, .. } =>
                                [obj.position[0] + radius, obj.position[1] + radius, obj.position[2] + radius],
                            _ => obj.position,
                        };
                        if let Some(sc) = Self::world_to_screen_vp(center, &vp, &rect) {
                            let axis_len = 55.0;
                            let head_sz = 10.0;
                            let hit_radius = 12.0; // 滑鼠靠近箭頭多近算 hover
                            let mouse = egui::pos2(self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                            let axes: [([ f32; 3], egui::Color32, &str, u8); 3] = [
                                ([1.0, 0.0, 0.0], egui::Color32::from_rgb(220, 60, 60), "X", 0),
                                ([0.0, 1.0, 0.0], egui::Color32::from_rgb(60, 180, 60), "Y", 1),
                                ([0.0, 0.0, 1.0], egui::Color32::from_rgb(60, 60, 220), "Z", 2),
                            ];
                            let mut new_hover: Option<u8> = None;
                            // 中心方塊（自由移動）
                            let center_rect = egui::Rect::from_center_size(sc, egui::vec2(10.0, 10.0));
                            let center_hovered = center_rect.contains(mouse);
                            ui.painter().rect_filled(center_rect, 2.0,
                                if center_hovered { egui::Color32::from_rgb(255, 255, 255) }
                                else { egui::Color32::from_rgba_unmultiplied(200, 200, 200, 150) });

                            for (dir, base_color, label, axis_idx) in &axes {
                                let end_world = [
                                    center[0] + dir[0] * 800.0,
                                    center[1] + dir[1] * 800.0,
                                    center[2] + dir[2] * 800.0,
                                ];
                                if let Some(ep) = Self::world_to_screen_vp(end_world, &vp, &rect) {
                                    let dx = ep.x - sc.x;
                                    let dy = ep.y - sc.y;
                                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                                    let nx = dx / len;
                                    let ny = dy / len;
                                    let tip = egui::pos2(sc.x + nx * axis_len, sc.y + ny * axis_len);

                                    // Hit test: 滑鼠到箭桿線段的距離
                                    let mx = mouse.x - sc.x;
                                    let my = mouse.y - sc.y;
                                    let proj = (mx * nx + my * ny).clamp(0.0, axis_len);
                                    let closest = egui::pos2(sc.x + nx * proj, sc.y + ny * proj);
                                    let dist = ((mouse.x - closest.x).powi(2) + (mouse.y - closest.y).powi(2)).sqrt();
                                    let is_hovered = dist < hit_radius;
                                    let is_active = self.editor.gizmo_drag_axis == Some(*axis_idx);

                                    if is_hovered { new_hover = Some(*axis_idx); }

                                    // 顏色：hover/active 時加亮，非 hover 時半透明
                                    let color = if is_active || is_hovered {
                                        egui::Color32::from_rgb(
                                            (base_color.r() as u16 + 60).min(255) as u8,
                                            (base_color.g() as u16 + 60).min(255) as u8,
                                            (base_color.b() as u16 + 60).min(255) as u8,
                                        )
                                    } else { *base_color };
                                    let thickness = if is_active || is_hovered { 4.0 } else { 2.5 };

                                    // 箭桿
                                    ui.painter().line_segment([sc, tip], egui::Stroke::new(thickness, color));
                                    // 箭頭（圓錐）
                                    let perp_x = -ny;
                                    let perp_y = nx;
                                    let cone_w = if is_hovered { head_sz * 0.5 } else { head_sz * 0.4 };
                                    let h1 = egui::pos2(tip.x - nx * head_sz + perp_x * cone_w, tip.y - ny * head_sz + perp_y * cone_w);
                                    let h2 = egui::pos2(tip.x - nx * head_sz - perp_x * cone_w, tip.y - ny * head_sz - perp_y * cone_w);
                                    ui.painter().add(egui::Shape::convex_polygon(vec![tip, h1, h2], color, egui::Stroke::NONE));
                                    // 軸標籤
                                    let label_size = if is_hovered { 13.0 } else { 10.0 };
                                    ui.painter().text(
                                        egui::pos2(tip.x + nx * 8.0, tip.y + ny * 8.0),
                                        egui::Align2::CENTER_CENTER, label,
                                        egui::FontId::proportional(label_size), color,
                                    );
                                }
                            }
                            self.editor.gizmo_hovered_axis = new_hover;
                        }
                    }
                }

                // ── Scale handles: visible grip squares on bounding box ──
                if self.editor.tool == Tool::Scale && !self.editor.selected_ids.is_empty() {
                    if let Some(obj) = self.editor.selected_ids.first()
                        .and_then(|id| self.scene.objects.get(id))
                    {
                        let pos = glam::Vec3::from(obj.position);
                        let (sx, sy, sz) = match &obj.shape {
                            Shape::Box { width, height, depth } => (*width, *height, *depth),
                            Shape::Cylinder { radius, height, .. } => (*radius * 2.0, *height, *radius * 2.0),
                            Shape::Sphere { radius, .. } => (*radius * 2.0, *radius * 2.0, *radius * 2.0),
                            _ => (0.0, 0.0, 0.0),
                        };
                        if sx > 0.0 {
                            // 旋轉函式：將局部偏移繞物件中心旋轉
                            let rot_fn = {
                                let [rx, ry, rz] = obj.rotation_xyz;
                                let has_rot = rx.abs() > 1e-6 || ry.abs() > 1e-6 || rz.abs() > 1e-6 || obj.rotation_y.abs() > 1e-6;
                                let eff_ry = if rx.abs() < 1e-6 && rz.abs() < 1e-6 && ry.abs() < 1e-6 { obj.rotation_y } else { ry };
                                let half = [sx / 2.0, sy / 2.0, sz / 2.0];
                                let (sin_x, cos_x) = rx.sin_cos();
                                let (sin_y, cos_y) = eff_ry.sin_cos();
                                let (sin_z, cos_z) = rz.sin_cos();
                                let r00 = cos_y*cos_z + sin_y*sin_x*sin_z;
                                let r01 = -cos_y*sin_z + sin_y*sin_x*cos_z;
                                let r02 = sin_y*cos_x;
                                let r10 = cos_x*sin_z;
                                let r11 = cos_x*cos_z;
                                let r12 = -sin_x;
                                let r20 = -sin_y*cos_z + cos_y*sin_x*sin_z;
                                let r21 = sin_y*sin_z + cos_y*sin_x*cos_z;
                                let r22 = cos_y*cos_x;
                                move |local: [f32; 3]| -> [f32; 3] {
                                    if !has_rot {
                                        return [pos.x + local[0], pos.y + local[1], pos.z + local[2]];
                                    }
                                    let dx = local[0] - half[0];
                                    let dy = local[1] - half[1];
                                    let dz = local[2] - half[2];
                                    [
                                        pos.x + half[0] + r00*dx + r01*dy + r02*dz,
                                        pos.y + half[1] + r10*dx + r11*dy + r12*dz,
                                        pos.z + half[2] + r20*dx + r21*dy + r22*dz,
                                    ]
                                }
                            };
                            // 8 corners + 6 face centers = 14 grip points
                            let corners_local = [
                                [0.0, 0.0, 0.0], [sx, 0.0, 0.0], [sx, 0.0, sz], [0.0, 0.0, sz],
                                [0.0, sy, 0.0], [sx, sy, 0.0], [sx, sy, sz], [0.0, sy, sz],
                            ];
                            let corners: Vec<[f32; 3]> = corners_local.iter().map(|c| rot_fn(*c)).collect();
                            let face_centers_local = [
                                [sx / 2.0, sy / 2.0, 0.0],  // Front
                                [sx / 2.0, sy / 2.0, sz],   // Back
                                [0.0, sy / 2.0, sz / 2.0],  // Left
                                [sx, sy / 2.0, sz / 2.0],   // Right
                                [sx / 2.0, 0.0, sz / 2.0],  // Bottom
                                [sx / 2.0, sy, sz / 2.0],   // Top
                            ];
                            let face_centers: Vec<[f32; 3]> = face_centers_local.iter().map(|c| rot_fn(*c)).collect();
                            let grip_size = 4.0;
                            let corner_color = egui::Color32::from_rgb(80, 200, 120); // green = uniform
                            // Face center grip colors: axis-coded (R=X, G=Y, B=Z)
                            let face_colors = [
                                egui::Color32::from_rgb(220, 80, 80),   // Front (Z)
                                egui::Color32::from_rgb(220, 80, 80),   // Back (Z)
                                egui::Color32::from_rgb(80, 80, 220),   // Left (X)
                                egui::Color32::from_rgb(80, 80, 220),   // Right (X)
                                egui::Color32::from_rgb(80, 200, 80),   // Bottom (Y)
                                egui::Color32::from_rgb(80, 200, 80),   // Top (Y)
                            ];
                            // 角落 grip (uniform scale)
                            for c in &corners {
                                let wp = *c; // 已經是世界座標
                                if let Some(sp) = Self::world_to_screen_vp(wp, &vp, &rect) {
                                    let r = egui::Rect::from_center_size(sp, egui::vec2(grip_size * 2.0, grip_size * 2.0));
                                    ui.painter().rect_filled(r, 0.0, corner_color);
                                    ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                                }
                            }
                            // 面中心 grip（per-axis scale, color-coded）
                            for (fi, fc) in face_centers.iter().enumerate() {
                                let wp = *fc; // 已經是世界座標
                                if let Some(sp) = Self::world_to_screen_vp(wp, &vp, &rect) {
                                    let r = egui::Rect::from_center_size(sp, egui::vec2(grip_size * 1.5, grip_size * 1.5));
                                    ui.painter().rect_filled(r, 0.0, face_colors[fi]);
                                    ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                                }
                            }
                            // 邊框連線
                            let edges: [(usize, usize); 12] = [
                                (0,1),(1,2),(2,3),(3,0), // bottom
                                (4,5),(5,6),(6,7),(7,4), // top
                                (0,4),(1,5),(2,6),(3,7), // vertical
                            ];
                            let edge_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(80, 200, 120, 120));
                            for (a, b) in &edges {
                                let wa = corners[*a]; // 已經是世界座標
                                let wb = corners[*b];
                                if let (Some(sa), Some(sb)) = (
                                    Self::world_to_screen_vp(wa, &vp, &rect),
                                    Self::world_to_screen_vp(wb, &vp, &rect),
                                ) {
                                    ui.painter().line_segment([sa, sb], edge_stroke);
                                }
                            }
                        }
                    }
                }

                // ── Scale mode badge ──
                if self.editor.tool == Tool::Scale {
                    if let DrawState::Scaling { handle, .. } = &self.editor.draw_state {
                        let effective = if self.editor.shift_held { &ScaleHandle::Uniform } else { handle };
                        let (label, color) = match effective {
                            ScaleHandle::Uniform => ("Uniform (Shift)", egui::Color32::from_rgb(80, 200, 120)),
                            ScaleHandle::AxisX => ("X-axis only", egui::Color32::from_rgb(220, 80, 80)),
                            ScaleHandle::AxisY => ("Y-axis only", egui::Color32::from_rgb(80, 200, 80)),
                            ScaleHandle::AxisZ => ("Z-axis only", egui::Color32::from_rgb(80, 80, 220)),
                        };
                        let badge_pos = egui::pos2(rect.min.x + 10.0, rect.max.y - 40.0);
                        let badge_rect = egui::Rect::from_min_size(badge_pos, egui::vec2(130.0, 22.0));
                        ui.painter().rect_filled(badge_rect, 6.0, egui::Color32::from_rgba_unmultiplied(30, 30, 30, 200));
                        ui.painter().rect_stroke(badge_rect, 6.0, egui::Stroke::new(1.0, color));
                        ui.painter().text(badge_rect.center(), egui::Align2::CENTER_CENTER,
                            label, egui::FontId::proportional(11.0), color);
                    } else if !self.editor.selected_ids.is_empty() {
                        // Hint when Scale tool active but not yet dragging
                        let hint_pos = egui::pos2(rect.min.x + 10.0, rect.max.y - 40.0);
                        let hint_rect = egui::Rect::from_min_size(hint_pos, egui::vec2(200.0, 22.0));
                        ui.painter().rect_filled(hint_rect, 6.0, egui::Color32::from_rgba_unmultiplied(30, 30, 30, 180));
                        let hint_color = egui::Color32::from_rgb(180, 180, 180);
                        ui.painter().text(hint_rect.center(), egui::Align2::CENTER_CENTER,
                            "Face=Axis  Corner=Uniform  Shift=Uniform", egui::FontId::proportional(10.0), hint_color);
                    }
                }
    }
}

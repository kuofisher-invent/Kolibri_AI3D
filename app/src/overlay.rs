//! 2D overlay 繪圖工具（虛線、圓弧計算）
//! 從 app.rs 拆分出來

use eframe::egui;

// ─── Arc geometry（真圓弧，非 Bezier 近似）──────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct ArcInfo {
    pub center: [f32; 3],
    pub radius: f32,
    pub start_angle: f32,
    pub end_angle: f32,
    pub normal: [f32; 3],
    pub u_axis: [f32; 3],
    pub v_axis: [f32; 3],
}

impl ArcInfo {
    pub fn sweep_angle(&self) -> f32 {
        let mut sweep = self.end_angle - self.start_angle;
        if sweep < 0.0 { sweep += std::f32::consts::TAU; }
        sweep
    }
    pub fn sweep_degrees(&self) -> f32 {
        self.sweep_angle().to_degrees()
    }
    pub fn arc_length(&self) -> f32 {
        self.radius * self.sweep_angle()
    }
    pub fn is_semicircle(&self) -> bool {
        let deg = self.sweep_degrees();
        deg > 170.0 && deg < 190.0
    }
    pub fn points(&self, segments: usize) -> Vec<[f32; 3]> {
        let sweep = self.sweep_angle();
        let mut pts = Vec::with_capacity(segments + 1);
        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let angle = self.start_angle + sweep * t;
            let (sin_a, cos_a) = angle.sin_cos();
            pts.push([
                self.center[0] + self.radius * (cos_a * self.u_axis[0] + sin_a * self.v_axis[0]),
                self.center[1] + self.radius * (cos_a * self.u_axis[1] + sin_a * self.v_axis[1]),
                self.center[2] + self.radius * (cos_a * self.u_axis[2] + sin_a * self.v_axis[2]),
            ]);
        }
        pts
    }
}

/// 從兩端點 + 凸度點計算真圓弧（circumscribed circle）
pub(crate) fn compute_arc_info(p1: [f32; 3], p2: [f32; 3], p3: [f32; 3]) -> Option<ArcInfo> {
    let a = glam::Vec3::from(p1);
    let b = glam::Vec3::from(p2);
    let c = glam::Vec3::from(p3);

    let ab = b - a;
    let ac = c - a;
    let normal = ab.cross(ac);
    if normal.length_squared() < 1e-6 {
        return None;
    }
    let normal = normal.normalize();

    let mid_ab = (a + b) * 0.5;
    let mid_ac = (a + c) * 0.5;
    let dir_ab = ab.cross(normal).normalize();
    let dir_ac = ac.cross(normal).normalize();

    let d = mid_ac - mid_ab;
    let denom = dir_ab.cross(dir_ac).length_squared();
    if denom < 1e-10 { return None; }
    let t1 = d.cross(dir_ac).dot(dir_ab.cross(dir_ac)) / denom;
    let center = mid_ab + dir_ab * t1;
    let radius = (a - center).length();

    let u_axis = (a - center).normalize();
    let v_axis = normal.cross(u_axis).normalize();

    let angle_of = |p: glam::Vec3| -> f32 {
        let d = p - center;
        let u = d.dot(u_axis);
        let v = d.dot(v_axis);
        v.atan2(u)
    };

    let angle_a = angle_of(a);
    let angle_b = angle_of(b);
    let angle_c = angle_of(c);

    let mut end_angle = angle_b - angle_a;
    let mut mid_check = angle_c - angle_a;
    if mid_check < 0.0 { mid_check += std::f32::consts::TAU; }
    if end_angle < 0.0 { end_angle += std::f32::consts::TAU; }

    if mid_check > end_angle {
        end_angle = end_angle - std::f32::consts::TAU;
    }

    Some(ArcInfo {
        center: center.into(),
        radius,
        start_angle: angle_a,
        end_angle: angle_a + end_angle,
        normal: normal.into(),
        u_axis: u_axis.into(),
        v_axis: v_axis.into(),
    })
}

/// 向下相容包裝：回傳點陣列
pub(crate) fn compute_arc(p1: [f32; 3], p2: [f32; 3], p3: [f32; 3], segments: usize) -> Vec<[f32; 3]> {
    if let Some(info) = compute_arc_info(p1, p2, p3) {
        info.points(segments)
    } else {
        vec![p1, p2]
    }
}

// ─── Dashed line helper ──────────────────────────────────────────────────────

pub(crate) fn draw_dashed_line(
    painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2,
    stroke: egui::Stroke, dash: f32, gap: f32,
) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 { return; }
    let nx = dx / len;
    let ny = dy / len;
    let mut t = 0.0;
    while t < len {
        let t1 = t;
        let t2 = (t + dash).min(len);
        painter.line_segment(
            [
                egui::pos2(from.x + nx * t1, from.y + ny * t1),
                egui::pos2(from.x + nx * t2, from.y + ny * t2),
            ],
            stroke,
        );
        t += dash + gap;
    }
}

// ─── Viewport overlay drawing (extracted from app.rs) ────────────────────────

use crate::app::{KolibriApp, CursorHint, DrawState, PullFace, RenderMode, ScaleHandle, SelectionMode, SnapType, Tool, WorkMode};
use crate::scene::Shape;

impl KolibriApp {
    /// Draw all 2D overlays on the 3D viewport (guide lines, snap indicators,
    /// protractor, gizmo, scale handles, dimension editing, import review, etc.)
    pub(crate) fn draw_viewport_overlays(
        &mut self,
        ui: &mut egui::Ui,
        vp: glam::Mat4,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
                // ── 渲染模式 Pill（右上角快速切換）──
                {
                    let modes = [
                        (RenderMode::Shaded, "著色"),
                        (RenderMode::Wireframe, "線框"),
                        (RenderMode::XRay, "X光"),
                        (RenderMode::HiddenLine, "隱藏線"),
                        (RenderMode::Sketch, "草稿"),
                    ];
                    let pill_h = 22.0;
                    let pill_w = 42.0;
                    let total_w = modes.len() as f32 * pill_w + 2.0;
                    let pill_x = rect.max.x - total_w - 8.0;
                    let pill_y = rect.min.y + 8.0;

                    // 背景膠囊
                    let bg_rect = egui::Rect::from_min_size(
                        egui::pos2(pill_x, pill_y),
                        egui::vec2(total_w, pill_h),
                    );
                    ui.painter().rect_filled(bg_rect, pill_h / 2.0,
                        egui::Color32::from_rgba_unmultiplied(30, 32, 45, 200));

                    for (i, (mode, label)) in modes.iter().enumerate() {
                        let active = self.viewer.render_mode == *mode;
                        let btn_rect = egui::Rect::from_min_size(
                            egui::pos2(pill_x + 1.0 + i as f32 * pill_w, pill_y + 1.0),
                            egui::vec2(pill_w - 1.0, pill_h - 2.0),
                        );
                        let mouse = egui::pos2(self.editor.mouse_screen[0] + rect.min.x,
                                               self.editor.mouse_screen[1] + rect.min.y);
                        let hovered = btn_rect.contains(mouse);

                        if active {
                            ui.painter().rect_filled(btn_rect, pill_h / 2.0 - 1.0,
                                egui::Color32::from_rgb(76, 139, 245));
                        } else if hovered {
                            ui.painter().rect_filled(btn_rect, pill_h / 2.0 - 1.0,
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 60));
                        }

                        let text_color = if active {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_rgb(180, 185, 200)
                        };
                        ui.painter().text(
                            btn_rect.center(), egui::Align2::CENTER_CENTER,
                            label, egui::FontId::proportional(10.0), text_color,
                        );

                        // 點擊切換
                        if hovered && response.clicked() {
                            self.viewer.render_mode = *mode;
                        }
                    }
                }

                // ── Camera bookmarks（左上角小按鈕）──
                if !self.viewer.saved_cameras.is_empty() {
                    let bk_size = 20.0;
                    let bk_x = rect.min.x + 8.0;
                    let bk_y = rect.min.y + 8.0;
                    for (i, (name, _cam)) in self.viewer.saved_cameras.iter().enumerate() {
                        if i >= 6 { break; } // 最多顯示 6 個
                        let br = egui::Rect::from_min_size(
                            egui::pos2(bk_x + i as f32 * (bk_size + 4.0), bk_y),
                            egui::vec2(bk_size, bk_size),
                        );
                        let mouse = egui::pos2(self.editor.mouse_screen[0] + rect.min.x,
                                               self.editor.mouse_screen[1] + rect.min.y);
                        let hovered = br.contains(mouse);
                        ui.painter().rect_filled(br, 4.0,
                            if hovered { egui::Color32::from_rgba_unmultiplied(76, 139, 245, 200) }
                            else { egui::Color32::from_rgba_unmultiplied(40, 42, 55, 180) });
                        ui.painter().text(br.center(), egui::Align2::CENTER_CENTER,
                            &format!("{}", i + 1),
                            egui::FontId::proportional(11.0),
                            if hovered { egui::Color32::WHITE } else { egui::Color32::from_rgb(180, 185, 200) });
                        if hovered && response.clicked() {
                            if let Some((_, cam)) = self.viewer.saved_cameras.get(i) {
                                self.viewer.camera = cam.clone();
                            }
                        }
                    }
                    // + 按鈕儲存新 bookmark
                    let plus_rect = egui::Rect::from_min_size(
                        egui::pos2(bk_x + self.viewer.saved_cameras.len().min(6) as f32 * (bk_size + 4.0), bk_y),
                        egui::vec2(bk_size, bk_size),
                    );
                    let mouse = egui::pos2(self.editor.mouse_screen[0] + rect.min.x,
                                           self.editor.mouse_screen[1] + rect.min.y);
                    let plus_hovered = plus_rect.contains(mouse);
                    ui.painter().rect_filled(plus_rect, 4.0,
                        egui::Color32::from_rgba_unmultiplied(40, 42, 55, if plus_hovered { 220 } else { 120 }));
                    ui.painter().text(plus_rect.center(), egui::Align2::CENTER_CENTER, "+",
                        egui::FontId::proportional(14.0),
                        egui::Color32::from_rgba_unmultiplied(180, 185, 200, if plus_hovered { 255 } else { 150 }));
                    if plus_hovered && response.clicked() {
                        let name = format!("View {}", self.viewer.saved_cameras.len() + 1);
                        self.viewer.saved_cameras.push((name, self.viewer.camera.clone()));
                    }
                }

                // ── Draw guide/construction lines ──
                if !self.scene.guide_lines.is_empty() {
                    let guide_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(150, 50, 50, 180));
                    for (start, end) in &self.scene.guide_lines {
                        let p1 = Self::world_to_screen_vp(*start, &vp, &rect);
                        let p2 = Self::world_to_screen_vp(*end, &vp, &rect);
                        if let (Some(s1), Some(s2)) = (p1, p2) {
                            // Dashed guide line
                            let dir: egui::Vec2 = s2 - s1;
                            let total = dir.length();
                            if total > 1.0 {
                                let norm = dir / total;
                                let step: f32 = 10.0;
                                let mut d_val: f32 = 0.0;
                                while d_val < total {
                                    let a_pt = s1 + norm * d_val;
                                    let b_pt = s1 + norm * (d_val + step * 0.6).min(total);
                                    ui.painter().line_segment([a_pt, b_pt], guide_stroke);
                                    d_val += step;
                                }
                            }
                        }
                    }
                }

                // ── A6: Track move origin for rubber band ──
                if self.editor.tool == Tool::Move && self.editor.drag_snapshot_taken && !self.editor.selected_ids.is_empty() {
                    // Move drag is in progress; capture origin if not yet set
                    if self.editor.move_origin.is_none() {
                        // Use the undo stack's last snapshot to get the original position
                        if let Some((prev_objects, _)) = self.scene.undo_stack.last() {
                            if let Some(obj) = prev_objects.get(&self.editor.selected_ids[0]) {
                                self.editor.move_origin = Some(obj.position);
                            }
                        }
                    }
                } else {
                    // Not in a move drag; clear origin
                    self.editor.move_origin = None;
                }

                // ── A2/A3: Cursor-following dimension during drag/push-pull ──
                self.editor.cursor_dimension = match &self.editor.draw_state {
                    DrawState::Pulling { obj_id, face, original_dim } => {
                        if let Some(obj) = self.scene.objects.get(obj_id) {
                            let current_dim = match (&obj.shape, face) {
                                (Shape::Box { height, .. }, PullFace::Top | PullFace::Bottom) => *height,
                                (Shape::Box { depth, .. }, PullFace::Front | PullFace::Back) => *depth,
                                (Shape::Box { width, .. }, PullFace::Left | PullFace::Right) => *width,
                                (Shape::Cylinder { height, .. }, _) => *height,
                                _ => 0.0,
                            };
                            let delta = current_dim - original_dim;
                            if delta.abs() > 0.5 {
                                Some((self.editor.mouse_screen[0] + 20.0, self.editor.mouse_screen[1] - 20.0,
                                    format!("拉伸 {:.0} mm", delta)))
                            } else { None }
                        } else { None }
                    }
                    DrawState::Scaling { ref obj_id, handle, original_dims } => {
                        if let Some(obj) = self.scene.objects.get(obj_id) {
                            let current = match &obj.shape {
                                Shape::Box { width, height, depth } => [*width, *height, *depth],
                                Shape::Cylinder { radius, height, .. } => [*radius * 2.0, *height, *radius * 2.0],
                                Shape::Sphere { radius, .. } => [*radius * 2.0, *radius * 2.0, *radius * 2.0],
                                _ => [0.0; 3],
                            };
                            let text = match handle {
                                ScaleHandle::Uniform => {
                                    let ratio = if original_dims[0] > 0.1 { current[0] / original_dims[0] } else { 1.0 };
                                    format!("\u{00d7}{:.2}", ratio)
                                }
                                ScaleHandle::AxisX => format!("W: {:.0} mm (\u{00d7}{:.2})", current[0], current[0] / original_dims[0].max(1.0)),
                                ScaleHandle::AxisY => format!("H: {:.0} mm (\u{00d7}{:.2})", current[1], current[1] / original_dims[1].max(1.0)),
                                ScaleHandle::AxisZ => format!("D: {:.0} mm (\u{00d7}{:.2})", current[2], current[2] / original_dims[2].max(1.0)),
                            };
                            Some((self.editor.mouse_screen[0] + 20.0, self.editor.mouse_screen[1] - 20.0, text))
                        } else { None }
                    }
                    _ => None,
                };

                // ── Build cursor hint ──
                {
                    // Detect tool change => trigger fade
                    if self.editor.tool != self.editor.prev_tool_for_hint {
                        self.editor.cursor_hint_fade = Some(std::time::Instant::now());
                        self.editor.prev_tool_for_hint = self.editor.tool;
                    }
                    // Fade-out after tool change
                    if let Some(fade_time) = self.editor.cursor_hint_fade {
                        if fade_time.elapsed().as_millis() > 300 {
                            self.editor.cursor_hint.active = false;
                            self.editor.cursor_hint_fade = None;
                        }
                    }

                    self.editor.cursor_hint = CursorHint::default();

                    let is_drawing = !matches!(self.editor.draw_state, DrawState::Idle)
                        || matches!(self.editor.tool, Tool::Line | Tool::Arc | Tool::Rectangle | Tool::Circle
                            | Tool::CreateBox | Tool::CreateCylinder | Tool::CreateSphere
                            | Tool::PushPull | Tool::Move | Tool::Rotate | Tool::Scale | Tool::TapeMeasure
                            | Tool::Dimension | Tool::Text);

                    if is_drawing {
                        self.editor.cursor_hint.active = true;

                        // Layer 1: Inference source
                        if let Some(ref snap) = self.editor.snap_result {
                            if snap.snap_type != SnapType::None && snap.snap_type != SnapType::Grid {
                                let (dot, label) = match snap.snap_type {
                                    SnapType::Endpoint => ("\u{1f7e2}", "Endpoint"),
                                    SnapType::Midpoint => ("\u{1f535}", "Midpoint"),
                                    SnapType::Origin => ("\u{1f7e0}", "Origin"),
                                    SnapType::AxisX => ("\u{1f534}", "On Red Axis"),
                                    SnapType::AxisY => ("\u{1f7e2}", "On Green Axis"),
                                    SnapType::AxisZ => ("\u{1f535}", "On Blue Axis"),
                                    SnapType::OnFace => ("\u{1f7e1}", "On Face"),
                                    SnapType::FaceCenter => ("\u{2795}", "Face Center"),
                                    SnapType::OnEdge => ("\u{1f534}", "On Edge"),
                                    SnapType::Perpendicular => ("\u{1f7e3}", "Perpendicular"),
                                    SnapType::Parallel => ("\u{1f7e3}", "Parallel to Edge"),
                                    SnapType::Intersection => ("\u{26aa}", "Intersection"),
                                    _ => ("", ""),
                                };
                                self.editor.cursor_hint.inference_label = format!("{} {}", dot, label);
                                self.editor.cursor_hint.inference_color = snap.snap_type.color();
                            }
                        }

                        // Layer 2: Distance
                        if let Some((_, _, ref text)) = self.editor.cursor_dimension {
                            self.editor.cursor_hint.distance_text = text.clone();
                        } else if let Some(ref snap) = self.editor.snap_result {
                            if let Some(from) = snap.from_point {
                                let p = snap.position;
                                let dx = p[0] - from[0];
                                let dy = p[1] - from[1];
                                let dz = p[2] - from[2];
                                let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                                if dist > 1.0 {
                                    self.editor.cursor_hint.distance_text = if dist >= 1000.0 {
                                        format!("\u{2194} {:.2} m", dist / 1000.0)
                                    } else {
                                        format!("\u{2194} {:.0} mm", dist)
                                    };
                                }
                            }
                        }

                        // Layer 3: Chips
                        if let Some(ref snap) = self.editor.snap_result {
                            if snap.snap_type != SnapType::None && snap.snap_type != SnapType::Grid {
                                self.editor.cursor_hint.chips.push((snap.snap_type.label().to_string(), false));
                            }
                        }
                        // Working plane chip
                        match self.editor.inference_ctx.working_plane {
                            crate::inference::WorkingPlane::Ground => {
                                self.editor.cursor_hint.chips.push(("Ground".to_string(), false));
                            }
                            crate::inference::WorkingPlane::FaceXZ(y) => {
                                self.editor.cursor_hint.chips.push((format!("Plane Y:{:.0}", y), false));
                            }
                            _ => {}
                        }
                        // AI inference chip
                        if let Some((ref label, ref source)) = self.editor.inference_label {
                            if *source != crate::inference::InferenceSource::Geometry {
                                self.editor.cursor_hint.chips.push((format!("\u{1f916} {}", label), true));
                                self.editor.cursor_hint.ai_suggestion = Some(label.clone());
                            }
                        }

                        // ── Inference Engine 2.0: score snap through formal pipeline ──
                        if let Some(ref snap) = self.editor.snap_result {
                            let engine_ctx = crate::inference_engine::InferenceContext {
                                current_tool: crate::inference_engine::tool_to_kind(self.editor.tool),
                                current_mode: if self.editor.work_mode == WorkMode::Steel {
                                    crate::inference_engine::AppMode::Steel
                                } else {
                                    crate::inference_engine::AppMode::Modeling
                                },
                                selected_ids: self.editor.selected_ids.clone(),
                                hover_id: self.editor.hovered_id.clone(),
                                last_direction: self.editor.last_line_dir,
                                last_action: self.editor.last_action_name.clone(),
                                working_plane_y: 0.0,
                                locked_axis: self.editor.locked_axis,
                                is_drawing: !matches!(self.editor.draw_state, DrawState::Idle),
                                consecutive_same_tool: 1,
                            };

                            let candidate = crate::inference_engine::InferenceCandidate {
                                id: "snap_0".into(),
                                inference_type: crate::inference_engine::snap_type_to_inference_type(&snap.snap_type),
                                position: snap.position,
                                source_object_id: None,
                                raw_distance: 5.0,
                            };

                            let scored = self.editor.inference_engine.score_candidates(&[candidate], &engine_ctx);
                            if let Some(top) = scored.first() {
                                if let Some(reason) = top.breakdown.reasons.first() {
                                    self.editor.cursor_hint.inference_label = format!(
                                        "{} [S:{:.0}]",
                                        reason,
                                        top.breakdown.total,
                                    );
                                }
                            }
                        }

                        // Ghost line: predicted direction from last drawn line
                        if let Some(dir) = self.editor.inference_ctx.last_direction {
                            if let Some(from) = self.get_drawing_origin() {
                                let extend = 2000.0;
                                let to = [from[0] + dir[0] * extend, from[1], from[2] + dir[1] * extend];
                                self.editor.cursor_hint.ghost_dir = Some((from, to));
                            }
                        }
                    }
                }

                // ── Cursor Hint Card ──
                if self.editor.cursor_hint.active && (!self.editor.cursor_hint.inference_label.is_empty() || !self.editor.cursor_hint.distance_text.is_empty()) {
                    let mx = self.editor.mouse_screen[0];
                    let my = self.editor.mouse_screen[1];
                    let card_x = rect.min.x + mx + 40.0;  // farther from cursor to avoid blocking
                    let card_y = rect.min.y + my - 60.0;  // above cursor, not overlapping

                    let painter = ui.painter();
                    let font_small = egui::FontId::proportional(11.0);
                    let font_chip = egui::FontId::proportional(10.0);

                    let text_dark = egui::Color32::from_rgb(31, 36, 48);
                    let text_muted = egui::Color32::from_rgb(110, 118, 135);
                    let brand = egui::Color32::from_rgb(76, 139, 245);
                    let bg = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 235);
                    let border = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200);

                    // Calculate card height
                    let mut card_h = 8.0; // top padding
                    let has_inference = !self.editor.cursor_hint.inference_label.is_empty();
                    let has_distance = !self.editor.cursor_hint.distance_text.is_empty();
                    let has_chips = !self.editor.cursor_hint.chips.is_empty();
                    let has_tab = self.editor.cursor_hint.ai_suggestion.is_some();

                    if has_inference { card_h += 18.0; }
                    if has_distance { card_h += 26.0; }
                    if has_chips { card_h += 22.0; }
                    if has_tab { card_h += 22.0; }
                    card_h += 8.0; // bottom padding

                    let card_w = 240.0;

                    // Clamp to viewport
                    let cx = card_x.min(rect.max.x - card_w - 10.0);
                    let cy = card_y.min(rect.max.y - card_h - 10.0).max(rect.min.y + 10.0);

                    let card_rect = egui::Rect::from_min_size(egui::pos2(cx, cy), egui::vec2(card_w, card_h));

                    // Shadow
                    painter.rect_filled(card_rect.translate(egui::vec2(2.0, 3.0)), 14.0,
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 20));
                    // Background
                    painter.rect_filled(card_rect, 14.0, bg);
                    painter.rect_stroke(card_rect, 14.0, egui::Stroke::new(1.0, border));

                    let mut y_pos = cy + 10.0;
                    let lx = cx + 14.0;

                    // Layer 1: Inference label
                    if has_inference {
                        painter.text(egui::pos2(lx, y_pos), egui::Align2::LEFT_TOP,
                            &self.editor.cursor_hint.inference_label, font_small.clone(), text_muted);
                        y_pos += 18.0;
                    }

                    // Layer 2: Distance (big, bold, brand color)
                    if has_distance {
                        painter.text(egui::pos2(lx, y_pos), egui::Align2::LEFT_TOP,
                            &self.editor.cursor_hint.distance_text,
                            egui::FontId { size: 18.0, family: egui::FontFamily::Proportional },
                            brand);
                        y_pos += 26.0;
                    }

                    // Layer 3: Chips
                    if has_chips {
                        let mut chip_x = lx;
                        for (label, is_ai) in &self.editor.cursor_hint.chips {
                            let galley = painter.layout_no_wrap(label.clone(), font_chip.clone(), text_dark);
                            let chip_w = galley.size().x + 12.0;
                            let chip_h = 18.0;
                            let chip_rect = egui::Rect::from_min_size(egui::pos2(chip_x, y_pos), egui::vec2(chip_w, chip_h));

                            let chip_bg = if *is_ai {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 30)
                            } else {
                                egui::Color32::from_rgb(240, 242, 248)
                            };
                            let chip_border = if *is_ai {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 80)
                            } else {
                                egui::Color32::from_rgb(229, 231, 239)
                            };

                            painter.rect_filled(chip_rect, 9.0, chip_bg);
                            painter.rect_stroke(chip_rect, 9.0, egui::Stroke::new(0.5, chip_border));
                            painter.galley(egui::pos2(chip_x + 6.0, y_pos + 1.0), galley,
                                if *is_ai { brand } else { text_muted });

                            chip_x += chip_w + 4.0;
                            if chip_x > cx + card_w - 20.0 { break; }
                        }
                        y_pos += 22.0;
                    }

                    // TAB hint (only when AI suggestion exists)
                    if has_tab {
                        let tab_text = "\u{6309} TAB \u{5957}\u{7528} AI \u{5efa}\u{8b70}";
                        let tab_bg = egui::Color32::from_rgba_unmultiplied(76, 139, 245, 20);
                        let tab_rect = egui::Rect::from_min_size(egui::pos2(lx, y_pos), egui::vec2(card_w - 28.0, 18.0));
                        painter.rect_filled(tab_rect, 9.0, tab_bg);
                        painter.text(tab_rect.center(), egui::Align2::CENTER_CENTER,
                            tab_text, font_chip.clone(), brand);
                    }
                }

                // ── Ghost line (predicted direction) ──
                if let Some((from, to)) = self.editor.cursor_hint.ghost_dir {
                    if let (Some(s1), Some(s2)) = (
                        self.world_to_screen(from, &rect),
                        self.world_to_screen(to, &rect),
                    ) {
                        let ghost_color = if let Some(ref snap) = self.editor.snap_result {
                            match snap.snap_type {
                                SnapType::AxisX => egui::Color32::from_rgba_unmultiplied(220, 60, 60, 80),
                                SnapType::AxisZ => egui::Color32::from_rgba_unmultiplied(60, 100, 220, 80),
                                _ => egui::Color32::from_rgba_unmultiplied(150, 200, 255, 60),
                            }
                        } else {
                            egui::Color32::from_rgba_unmultiplied(150, 200, 255, 60)
                        };
                        draw_dashed_line(ui.painter(), s1, s2, egui::Stroke::new(1.5, ghost_color), 8.0, 6.0);
                    }
                }

                // ── Collision warning overlay near cursor ──
                if let Some(ref warning) = self.editor.collision_warning {
                    let warn_pos = egui::pos2(
                        rect.min.x + self.editor.mouse_screen[0] + 20.0,
                        rect.min.y + self.editor.mouse_screen[1] + 30.0,
                    );
                    let font = egui::FontId::proportional(12.0);
                    let galley = ui.painter().layout_no_wrap(warning.clone(), font, egui::Color32::from_rgb(240, 70, 50));
                    let bg = egui::Rect::from_min_size(warn_pos, galley.size()).expand(4.0);
                    ui.painter().rect_filled(bg, 6.0, egui::Color32::from_rgba_unmultiplied(60, 10, 10, 220));
                    ui.painter().galley(warn_pos + egui::vec2(4.0, 4.0), galley, egui::Color32::from_rgb(255, 100, 80));
                }
                // Clear collision warning each frame
                self.editor.collision_warning = None;

                // ── A6: Move rubber band line from original position ──
                if let Some(origin) = self.editor.move_origin {
                    if !self.editor.selected_ids.is_empty() {
                        if let Some(obj) = self.scene.objects.get(&self.editor.selected_ids[0]) {
                            let p1 = self.world_to_screen(origin, &rect);
                            let p2 = self.world_to_screen(obj.position, &rect);
                            if let (Some(s1), Some(s2)) = (p1, p2) {
                                // Draw dashed rubber band line
                                let dir = s2 - s1;
                                let total = dir.length();
                                if total > 2.0 {
                                    let norm = dir / total;
                                    let step = 8.0;
                                    let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100, 150, 255, 150));
                                    let mut d_val = 0.0;
                                    while d_val < total {
                                        let a_pt = s1 + norm * d_val;
                                        let b_pt = s1 + norm * (d_val + step * 0.6).min(total);
                                        ui.painter().line_segment([a_pt, b_pt], stroke);
                                        d_val += step;
                                    }
                                }
                                // Show distance label at midpoint
                                let dx = obj.position[0] - origin[0];
                                let dy = obj.position[1] - origin[1];
                                let dz = obj.position[2] - origin[2];
                                let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                                if dist > 1.0 {
                                    let mid = egui::pos2((s1.x+s2.x)*0.5, (s1.y+s2.y)*0.5 - 10.0);
                                    let label = if dist >= 1000.0 {
                                        format!("{:.2} m", dist / 1000.0)
                                    } else {
                                        format!("{:.0} mm", dist)
                                    };
                                    // Background
                                    let font = egui::FontId::proportional(11.0);
                                    let galley = ui.painter().layout_no_wrap(label, font, egui::Color32::from_gray(200));
                                    let bg_rect = egui::Rect::from_center_size(mid, galley.size()).expand(3.0);
                                    ui.painter().rect_filled(bg_rect, 2.0, egui::Color32::from_rgba_unmultiplied(30, 30, 40, 180));
                                    ui.painter().galley(bg_rect.min, galley, egui::Color32::from_gray(200));
                                }
                            }
                        }
                    }
                }

                // Show locked axis indicator with visual axis line through object
                if let Some(axis) = self.editor.locked_axis {
                    let (label, color) = match axis {
                        0 => ("X軸 ──", egui::Color32::from_rgb(240, 60, 60)),
                        1 => ("Y軸 │", egui::Color32::from_rgb(60, 200, 60)),
                        2 => ("Z軸 ──", egui::Color32::from_rgb(60, 60, 240)),
                        _ => ("", egui::Color32::WHITE),
                    };

                    // Text indicator at bottom-left
                    let pos = egui::pos2(rect.min.x + 10.0, rect.max.y - 30.0);
                    ui.painter().text(pos, egui::Align2::LEFT_BOTTOM, label,
                        egui::FontId::proportional(16.0), color);

                    // Draw axis line through selected object
                    if let Some(ref id) = self.editor.selected_ids.first() {
                        if let Some(obj) = self.scene.objects.get(*id) {
                            let p = obj.position;
                            let len = 5000.0;
                            let (a, b) = match axis {
                                0 => ([p[0]-len, p[1], p[2]], [p[0]+len, p[1], p[2]]),
                                1 => ([p[0], p[1]-len, p[2]], [p[0], p[1]+len, p[2]]),
                                2 => ([p[0], p[1], p[2]-len], [p[0], p[1], p[2]+len]),
                                _ => (p, p),
                            };
                            if let (Some(sa), Some(sb)) = (
                                self.world_to_screen(a, &rect),
                                self.world_to_screen(b, &rect),
                            ) {
                                let stroke = egui::Stroke::new(2.5, color);
                                // Dashed line
                                let dir = sb - sa;
                                let total = dir.length();
                                if total > 1.0 {
                                    let norm = dir / total;
                                    let step = 12.0;
                                    let mut d_val = 0.0;
                                    while d_val < total {
                                        let a_pt = sa + norm * d_val;
                                        let b_pt = sa + norm * (d_val + step * 0.6).min(total);
                                        ui.painter().line_segment([a_pt, b_pt], stroke);
                                        d_val += step;
                                    }
                                }
                            }
                        }
                    }
                }

                // Group isolation mode indicator + F3 exit button
                if let Some(ref gid) = self.editor.editing_group_id.clone() {
                    let label = if let Some(obj) = self.scene.objects.get(gid) {
                        format!("\u{1f512} 群組編輯: {}", obj.name)
                    } else {
                        "\u{1f512} 群組編輯模式".to_string()
                    };
                    let pos = egui::pos2(rect.center().x, rect.min.y + 25.0);
                    ui.painter().text(pos, egui::Align2::CENTER_TOP, &label,
                        egui::FontId::proportional(15.0),
                        egui::Color32::from_rgb(255, 200, 80));

                    // F3: "退出群組" floating button
                    let exit_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.center().x - 60.0, rect.top() + 50.0),
                        egui::vec2(120.0, 32.0),
                    );
                    ui.painter().rect_filled(exit_rect, 16.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 230));
                    ui.painter().rect_stroke(exit_rect, 16.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));
                    let exit_response = ui.allocate_rect(exit_rect, egui::Sense::click());
                    ui.painter().text(exit_rect.center(), egui::Align2::CENTER_CENTER,
                        "\u{21a9} \u{9000}\u{51fa}\u{7fa4}\u{7d44}", egui::FontId::proportional(12.0),
                        egui::Color32::from_rgb(76, 139, 245));
                    if exit_response.clicked() {
                        self.editor.editing_group_id = None;
                    }
                }

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
                                            draw_dashed_line(ui.painter(), s1, s2, dash_stroke, 6.0, 4.0);
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
                        draw_dashed_line(ui.painter(), sc, mouse,
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

                // ── Selection outline（螢幕空間 AABB 描邊）──
                for sel_id in &self.editor.selected_ids {
                    if let Some(obj) = self.scene.objects.get(sel_id) {
                        let p = obj.position;
                        let ext = match &obj.shape {
                            Shape::Box { width, height, depth } => [*width, *height, *depth],
                            Shape::Cylinder { radius, height, .. } => [*radius*2.0, *height, *radius*2.0],
                            Shape::Sphere { radius, .. } => [*radius*2.0; 3],
                            _ => continue,
                        };
                        // 8 corners → screen → 2D bounding rect
                        let corners = [
                            [p[0],p[1],p[2]], [p[0]+ext[0],p[1],p[2]],
                            [p[0],p[1]+ext[1],p[2]], [p[0]+ext[0],p[1]+ext[1],p[2]],
                            [p[0],p[1],p[2]+ext[2]], [p[0]+ext[0],p[1],p[2]+ext[2]],
                            [p[0],p[1]+ext[1],p[2]+ext[2]], [p[0]+ext[0],p[1]+ext[1],p[2]+ext[2]],
                        ];
                        let mut min_s = egui::pos2(f32::MAX, f32::MAX);
                        let mut max_s = egui::pos2(f32::MIN, f32::MIN);
                        let mut visible = 0;
                        for c in &corners {
                            if let Some(sp) = Self::world_to_screen_vp(*c, &vp, &rect) {
                                min_s.x = min_s.x.min(sp.x); min_s.y = min_s.y.min(sp.y);
                                max_s.x = max_s.x.max(sp.x); max_s.y = max_s.y.max(sp.y);
                                visible += 1;
                            }
                        }
                        if visible >= 2 {
                            let outline_rect = egui::Rect::from_min_max(min_s, max_s).expand(3.0);
                            ui.painter().rect_stroke(outline_rect, 4.0,
                                egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(76, 139, 245, 140)));
                            // 尺寸標籤（寬×高×深）
                            let dim_color = egui::Color32::from_rgba_unmultiplied(76, 139, 245, 200);
                            let dim_font = egui::FontId::proportional(9.0);
                            let fmt = |v: f32| if v >= 1000.0 { format!("{:.2}m", v/1000.0) } else { format!("{:.0}", v) };
                            // 底部：寬度
                            ui.painter().text(
                                egui::pos2(outline_rect.center().x, outline_rect.max.y + 10.0),
                                egui::Align2::CENTER_TOP, &fmt(ext[0]),
                                dim_font.clone(), dim_color);
                            // 右側：高度
                            ui.painter().text(
                                egui::pos2(outline_rect.max.x + 8.0, outline_rect.center().y),
                                egui::Align2::LEFT_CENTER, &fmt(ext[1]),
                                dim_font.clone(), dim_color);
                        }
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
                            // 8 corners + 6 face centers = 14 grip points
                            let corners = [
                                [0.0, 0.0, 0.0], [sx, 0.0, 0.0], [sx, 0.0, sz], [0.0, 0.0, sz],
                                [0.0, sy, 0.0], [sx, sy, 0.0], [sx, sy, sz], [0.0, sy, sz],
                            ];
                            let face_centers = [
                                [sx / 2.0, sy / 2.0, 0.0],  // Front
                                [sx / 2.0, sy / 2.0, sz],   // Back
                                [0.0, sy / 2.0, sz / 2.0],  // Left
                                [sx, sy / 2.0, sz / 2.0],   // Right
                                [sx / 2.0, 0.0, sz / 2.0],  // Bottom
                                [sx / 2.0, sy, sz / 2.0],   // Top
                            ];
                            let grip_size = 4.0;
                            let corner_color = egui::Color32::from_rgb(80, 200, 120);
                            let face_color = egui::Color32::from_rgb(80, 200, 120);
                            // 角落 grip
                            for c in &corners {
                                let wp = [pos.x + c[0], pos.y + c[1], pos.z + c[2]];
                                if let Some(sp) = Self::world_to_screen_vp(wp, &vp, &rect) {
                                    let r = egui::Rect::from_center_size(sp, egui::vec2(grip_size * 2.0, grip_size * 2.0));
                                    ui.painter().rect_filled(r, 0.0, corner_color);
                                    ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                                }
                            }
                            // 面中心 grip（稍小）
                            for fc in &face_centers {
                                let wp = [pos.x + fc[0], pos.y + fc[1], pos.z + fc[2]];
                                if let Some(sp) = Self::world_to_screen_vp(wp, &vp, &rect) {
                                    let r = egui::Rect::from_center_size(sp, egui::vec2(grip_size * 1.5, grip_size * 1.5));
                                    ui.painter().rect_filled(r, 0.0, face_color);
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
                                let wa = [pos.x + corners[*a][0], pos.y + corners[*a][1], pos.z + corners[*a][2]];
                                let wb = [pos.x + corners[*b][0], pos.y + corners[*b][1], pos.z + corners[*b][2]];
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

                // ── DXF Smart Import confirmation panel (legacy, redirects to review) ──
                if let Some(ir) = self.pending_ir.take() {
                    // Convert pending_ir into the new review panel
                    let entity_count = ir.columns.len() + ir.beams.len() + ir.base_plates.len();
                    let debug = ir.debug_report.clone();
                    self.import_review = Some(crate::import_review::ImportReview::from_drawing_ir(
                        &ir, &"DXF", entity_count, debug,
                    ));
                }

                // ── Import Review Panel (full-screen overlay) ──
                if let Some(ref mut review) = self.import_review {
                    if review.active {
                        let action = crate::import_review::draw_review_panel(ui, review, rect);
                        match action {
                            crate::import_review::ReviewAction::Confirm => {
                                let ir = review.to_drawing_ir();
                                self.scene.snapshot();
                                let result = crate::builders::steel_builder::build_from_ir(&mut self.scene, &ir);
                                self.editor.selected_ids.clear();
                                self.editor.selected_ids.extend(result.ids);
                                self.zoom_extents();
                                let msg = format!("建模完成: {} 柱 + {} 梁 + {} 底板",
                                    result.columns_created, result.beams_created, result.plates_created);
                                self.file_message = Some((msg, std::time::Instant::now()));
                                self.import_review = None;
                            }
                            crate::import_review::ReviewAction::Cancel => {
                                self.import_review = None;
                            }
                            _ => {}
                        }
                    }
                }

                // ── Unified Smart Import confirmation panel ──
                if let Some(ref ir) = self.pending_unified_ir.clone() {
                    let panel_w = 420.0;
                    let panel_h = 380.0;
                    let panel_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(panel_w, panel_h));

                    ui.painter().rect_filled(panel_rect, 16.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 245));
                    ui.painter().rect_stroke(panel_rect, 16.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));

                    let mut y_ir = panel_rect.top() + 20.0;
                    let x_ir = panel_rect.left() + 20.0;

                    ui.painter().text(egui::pos2(panel_rect.center().x, y_ir), egui::Align2::CENTER_TOP,
                        format!("智慧匯入結果 ({})", ir.source_format.to_uppercase()),
                        egui::FontId::proportional(16.0), egui::Color32::from_rgb(31, 36, 48));
                    y_ir += 30.0;

                    let info_lines = [
                        format!("來源檔案: {}", std::path::Path::new(&ir.source_file).file_name()
                            .map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
                        format!("網格數: {}", ir.stats.mesh_count),
                        format!("頂點數: {}", ir.stats.vertex_count),
                        format!("面數: {}", ir.stats.face_count),
                        format!("群組數: {}", ir.stats.group_count),
                        format!("構件數: {}", ir.stats.member_count),
                        format!("材質數: {}", ir.stats.material_count),
                    ];

                    for line_text in &info_lines {
                        ui.painter().text(egui::pos2(x_ir, y_ir), egui::Align2::LEFT_TOP,
                            line_text, egui::FontId::proportional(12.0), egui::Color32::from_rgb(60, 65, 80));
                        y_ir += 20.0;
                    }

                    y_ir += 15.0;

                    // Confirm button
                    let btn_confirm = egui::Rect::from_min_size(egui::pos2(panel_rect.center().x - 80.0, y_ir), egui::vec2(70.0, 32.0));
                    let btn_cancel = egui::Rect::from_min_size(egui::pos2(panel_rect.center().x + 10.0, y_ir), egui::vec2(70.0, 32.0));

                    ui.painter().rect_filled(btn_confirm, 8.0, egui::Color32::from_rgb(76, 139, 245));
                    ui.painter().text(btn_confirm.center(), egui::Align2::CENTER_CENTER, "確認建模",
                        egui::FontId::proportional(12.0), egui::Color32::WHITE);

                    ui.painter().rect_filled(btn_cancel, 8.0, egui::Color32::from_rgb(200, 200, 200));
                    ui.painter().text(btn_cancel.center(), egui::Align2::CENTER_CENTER, "取消",
                        egui::FontId::proportional(12.0), egui::Color32::from_rgb(60, 60, 60));

                    let confirm_resp = ui.allocate_rect(btn_confirm, egui::Sense::click());
                    let cancel_resp = ui.allocate_rect(btn_cancel, egui::Sense::click());

                    if confirm_resp.clicked() {
                        let ir_data = self.pending_unified_ir.take().unwrap();
                        self.start_scene_build_task(ir_data);
                    }
                    if cancel_resp.clicked() {
                        self.pending_unified_ir = None;
                    }
                }

                // Unsaved changes confirmation overlay
                if self.pending_action.is_some() {
                    let popup_rect = egui::Rect::from_center_size(
                        rect.center(),
                        egui::vec2(350.0, 100.0),
                    );
                    ui.painter().rect_filled(popup_rect, 8.0, egui::Color32::from_rgba_unmultiplied(30, 30, 40, 240));
                    ui.painter().rect_stroke(popup_rect, 8.0, egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 150, 220)));
                    ui.painter().text(popup_rect.center_top() + egui::vec2(0.0, 20.0),
                        egui::Align2::CENTER_TOP, "場景有未儲存的修改",
                        egui::FontId::proportional(15.0), egui::Color32::WHITE);
                    ui.painter().text(popup_rect.center() + egui::vec2(0.0, 5.0),
                        egui::Align2::CENTER_CENTER, "按 Y 繼續（放棄修改）/ N 取消",
                        egui::FontId::proportional(13.0), egui::Color32::from_gray(180));
                }

                // ── Floating view buttons (top-center of viewport) ──
                {
                    let view_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.center().x - 120.0, rect.top() + 8.0),
                        egui::vec2(240.0, 44.0),
                    );

                    // Background pill
                    ui.painter().rect_filled(view_rect, 18.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 225));
                    ui.painter().rect_stroke(view_rect, 18.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200)));

                    // View buttons
                    let views = ["\u{900f}\u{8996}", "\u{6b63}\u{8996}", "\u{4fef}\u{8996}", "\u{5de6}\u{8996}"];
                    let btn_w = 50.0;
                    let padding = 8.0;
                    for (i, label) in views.iter().enumerate() {
                        let x = view_rect.left() + padding + i as f32 * (btn_w + 6.0);
                        let btn_rect = egui::Rect::from_min_size(
                            egui::pos2(x, view_rect.top() + 6.0),
                            egui::vec2(btn_w, 32.0),
                        );

                        let is_active = match i {
                            0 => !self.viewer.use_ortho,  // 透視
                            1 => self.viewer.use_ortho && self.viewer.camera.pitch.abs() < 0.1, // 正視
                            2 => self.viewer.use_ortho && self.viewer.camera.pitch < -1.0, // 俯視
                            3 => self.viewer.use_ortho && (self.viewer.camera.yaw + std::f32::consts::FRAC_PI_2).abs() < 0.1, // 左視
                            _ => false,
                        };

                        let response = ui.allocate_rect(btn_rect, egui::Sense::click());
                        let bg = if is_active {
                            egui::Color32::from_rgba_unmultiplied(76, 139, 245, 30)
                        } else if response.hovered() {
                            egui::Color32::from_rgb(240, 242, 248)
                        } else {
                            egui::Color32::WHITE
                        };
                        let text_color = if is_active {
                            egui::Color32::from_rgb(76, 139, 245)
                        } else {
                            egui::Color32::from_rgb(110, 118, 135)
                        };

                        ui.painter().rect_filled(btn_rect, 12.0, bg);
                        ui.painter().rect_stroke(btn_rect, 12.0,
                            egui::Stroke::new(1.0, if is_active {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 90)
                            } else {
                                egui::Color32::from_rgb(229, 231, 239)
                            }));
                        ui.painter().text(btn_rect.center(), egui::Align2::CENTER_CENTER,
                            label, egui::FontId::proportional(12.0), text_color);

                        if response.clicked() {
                            match i {
                                0 => self.viewer.use_ortho = false,
                                1 => { self.viewer.use_ortho = true; self.viewer.camera.set_front(); }
                                2 => { self.viewer.use_ortho = true; self.viewer.camera.set_top(); }
                                3 => { self.viewer.use_ortho = true; self.viewer.camera.set_left(); }
                                _ => {}
                            }
                        }
                    }
                }

                // ── Tool info card (top-left of viewport) ──
                {
                    let card_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + 16.0, rect.top() + 16.0),
                        egui::vec2(280.0, 60.0),
                    );
                    ui.painter().rect_filled(card_rect, 18.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 225));
                    ui.painter().rect_stroke(card_rect, 18.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200)));

                    let tool_name = match self.editor.tool {
                        Tool::Select => "\u{9078}\u{53d6} / Move-Ready",
                        Tool::Move => "\u{79fb}\u{52d5}\u{5de5}\u{5177}",
                        Tool::Rotate => "\u{65cb}\u{8f49}\u{5de5}\u{5177}",
                        Tool::CreateBox => "\u{65b9}\u{584a}\u{5de5}\u{5177}",
                        Tool::PushPull => "\u{63a8}\u{62c9}\u{5de5}\u{5177}",
                        Tool::Line => "\u{7dda}\u{6bb5}\u{5de5}\u{5177}",
                        _ => "\u{5de5}\u{5177}",
                    };
                    ui.painter().text(
                        egui::pos2(card_rect.left() + 14.0, card_rect.top() + 16.0),
                        egui::Align2::LEFT_TOP,
                        format!("\u{76ee}\u{524d}\u{5de5}\u{5177}\u{ff1a}{}", tool_name),
                        egui::FontId::proportional(13.0),
                        egui::Color32::from_rgb(31, 36, 48),
                    );
                    ui.painter().text(
                        egui::pos2(card_rect.left() + 14.0, card_rect.top() + 36.0),
                        egui::Align2::LEFT_TOP,
                        &self.status_text(),
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgb(110, 118, 135),
                    );
                }

                // ── Navigation pad (bottom-left of viewport) ──
                {
                    let pad_size = 130.0;
                    let pad_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + 16.0, rect.bottom() - pad_size - 60.0),
                        egui::vec2(pad_size, pad_size + 24.0),
                    );
                    ui.painter().rect_filled(pad_rect, 22.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 225));
                    ui.painter().rect_stroke(pad_rect, 22.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210)));

                    // Title
                    ui.painter().text(
                        egui::pos2(pad_rect.left() + 12.0, pad_rect.top() + 10.0),
                        egui::Align2::LEFT_TOP, "\u{8996}\u{89d2} / \u{5e73}\u{79fb}",
                        egui::FontId::proportional(11.0), egui::Color32::from_rgb(110, 118, 135));

                    // 3x3 button grid
                    let arrows = ["", "\u{2191}", "", "\u{2190}", "\u{29bf}", "\u{2192}", "", "\u{2193}", ""];
                    let btn_size = 32.0;
                    let gap = 6.0;
                    let grid_start_x = pad_rect.center().x - (btn_size * 1.5 + gap);
                    let grid_start_y = pad_rect.top() + 28.0;

                    for (i, label) in arrows.iter().enumerate() {
                        if label.is_empty() { continue; }
                        let row = i / 3;
                        let col = i % 3;
                        let btn_rect = egui::Rect::from_min_size(
                            egui::pos2(
                                grid_start_x + col as f32 * (btn_size + gap),
                                grid_start_y + row as f32 * (btn_size + gap),
                            ),
                            egui::vec2(btn_size, btn_size),
                        );

                        let response = ui.allocate_rect(btn_rect, egui::Sense::click());
                        let bg = if response.hovered() {
                            egui::Color32::from_rgb(240, 242, 248)
                        } else {
                            egui::Color32::WHITE
                        };
                        ui.painter().rect_filled(btn_rect, 12.0, bg);
                        ui.painter().rect_stroke(btn_rect, 12.0,
                            egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));
                        ui.painter().text(btn_rect.center(), egui::Align2::CENTER_CENTER,
                            label, egui::FontId::proportional(14.0), egui::Color32::from_rgb(110, 118, 135));

                        if response.clicked() {
                            let step = self.viewer.camera.distance * 0.1;
                            match i {
                                1 => self.viewer.camera.walk_forward(step),
                                3 => self.viewer.camera.walk_strafe(-step),
                                4 => self.viewer.camera.set_iso(),
                                5 => self.viewer.camera.walk_strafe(step),
                                7 => self.viewer.camera.walk_forward(-step),
                                _ => {}
                            }
                        }
                    }
                }

                // ── Coordinate chips (bottom-center of viewport) ──
                {
                    let chips_y = rect.bottom() - 40.0;
                    let chip_data = [
                        format!("X: {:.0}", self.editor.mouse_ground.map(|p| p[0]).unwrap_or(0.0)),
                        format!("Y: {:.0}", self.editor.mouse_ground.map(|p| p[1]).unwrap_or(0.0)),
                        format!("Z: {:.0}", self.editor.mouse_ground.map(|p| p[2]).unwrap_or(0.0)),
                        "Snap: ON".to_string(),
                        "Units: mm".to_string(),
                    ];

                    let chip_w = 70.0;
                    let total_w = chip_data.len() as f32 * (chip_w + 8.0);
                    let start_x = rect.center().x - total_w / 2.0;

                    for (i, text) in chip_data.iter().enumerate() {
                        let chip_rect = egui::Rect::from_min_size(
                            egui::pos2(start_x + i as f32 * (chip_w + 8.0), chips_y),
                            egui::vec2(chip_w, 28.0),
                        );
                        ui.painter().rect_filled(chip_rect, 999.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 215));
                        ui.painter().rect_stroke(chip_rect, 999.0,
                            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200)));
                        ui.painter().text(chip_rect.center(), egui::Align2::CENTER_CENTER,
                            text, egui::FontId::proportional(11.0), egui::Color32::from_rgb(31, 36, 48));
                    }
                }

                // ── Tool cursor icon (small tool icon follows mouse) ──
                if response.hovered() {
                    let mx = rect.min.x + self.editor.mouse_screen[0];
                    let my = rect.min.y + self.editor.mouse_screen[1];

                    // Draw mini tool icon (20x20) at cursor offset
                    let icon_size = 20.0;
                    let icon_rect = egui::Rect::from_min_size(
                        egui::pos2(mx + 14.0, my + 14.0),  // bottom-right of cursor
                        egui::vec2(icon_size, icon_size),
                    );

                    // Semi-transparent background circle per tool category
                    let bg_color = match self.editor.tool {
                        Tool::Select => egui::Color32::from_rgba_unmultiplied(76, 139, 245, 180),
                        Tool::Move => egui::Color32::from_rgba_unmultiplied(245, 166, 35, 180),
                        Tool::Rotate => egui::Color32::from_rgba_unmultiplied(180, 80, 220, 180),
                        Tool::Scale => egui::Color32::from_rgba_unmultiplied(80, 200, 120, 180),
                        Tool::Line | Tool::Arc | Tool::Rectangle | Tool::Circle => egui::Color32::from_rgba_unmultiplied(60, 60, 60, 180),
                        Tool::CreateBox | Tool::CreateCylinder | Tool::CreateSphere => egui::Color32::from_rgba_unmultiplied(76, 139, 245, 180),
                        Tool::PushPull => egui::Color32::from_rgba_unmultiplied(245, 100, 60, 180),
                        Tool::PaintBucket => egui::Color32::from_rgba_unmultiplied(220, 80, 160, 180),
                        Tool::Eraser => egui::Color32::from_rgba_unmultiplied(220, 50, 50, 180),
                        Tool::TapeMeasure | Tool::Dimension => egui::Color32::from_rgba_unmultiplied(100, 180, 100, 180),
                        Tool::Text => egui::Color32::from_rgba_unmultiplied(180, 140, 60, 180),
                        _ => egui::Color32::from_rgba_unmultiplied(100, 100, 100, 160),
                    };

                    // Draw circular background
                    let center = icon_rect.center();
                    ui.painter().circle_filled(center, icon_size * 0.55, bg_color);

                    // Draw the tool icon inside (shrunk)
                    let inner_rect = icon_rect.shrink(3.0);
                    crate::icons::draw_tool_icon(ui.painter(), inner_rect, self.editor.tool, egui::Color32::WHITE);
                }

                // ── 樓層指示線 ──
                if self.viewer.current_floor != 0 {
                    let floor_y = self.viewer.current_floor as f32 * self.viewer.floor_height;
                    // 畫水平虛線標示樓層
                    let left_world = [-10000.0, floor_y, 0.0];
                    let right_world = [10000.0, floor_y, 0.0];
                    if let (Some(sl), Some(sr)) = (
                        Self::world_to_screen_vp(left_world, &vp, &rect),
                        Self::world_to_screen_vp(right_world, &vp, &rect),
                    ) {
                        let floor_color = egui::Color32::from_rgba_unmultiplied(76, 139, 245, 80);
                        draw_dashed_line(ui.painter(), sl, sr, egui::Stroke::new(1.0, floor_color), 8.0, 6.0);
                        let floor_name = match self.viewer.current_floor {
                            f if f < 0 => format!("B{}", -f),
                            0 => "GF".to_string(),
                            f => format!("{}F", f),
                        };
                        ui.painter().text(
                            egui::pos2(rect.min.x + 8.0, sl.y - 10.0),
                            egui::Align2::LEFT_BOTTOM,
                            &format!("── {} ({:.0}m) ──", floor_name, floor_y / 1000.0),
                            egui::FontId::proportional(10.0),
                            egui::Color32::from_rgba_unmultiplied(76, 139, 245, 120),
                        );
                    }
                }

                // ── Viewport axes indicator（右下角 XYZ 方向立方）──
                {
                    let ax_size = 40.0;
                    let ax_center = egui::pos2(rect.max.x - ax_size - 12.0, rect.max.y - ax_size - 40.0);
                    let ax_len = ax_size * 0.4;
                    // 從相機 view matrix 提取軸向（螢幕空間投影）
                    let view = self.viewer.camera.view();
                    let axes_3d = [
                        (glam::Vec3::X, egui::Color32::from_rgb(220, 60, 60), "X"),
                        (glam::Vec3::Y, egui::Color32::from_rgb(60, 180, 60), "Y"),
                        (glam::Vec3::Z, egui::Color32::from_rgb(60, 60, 220), "Z"),
                    ];
                    for (dir, color, label) in &axes_3d {
                        let view_dir = view.transform_vector3(*dir);
                        let sx = view_dir.x * ax_len;
                        let sy = -view_dir.y * ax_len; // screen Y is inverted
                        let tip = egui::pos2(ax_center.x + sx, ax_center.y + sy);
                        ui.painter().line_segment([ax_center, tip], egui::Stroke::new(2.0, *color));
                        ui.painter().text(
                            egui::pos2(tip.x + sx * 0.15, tip.y + sy * 0.15),
                            egui::Align2::CENTER_CENTER, label,
                            egui::FontId::proportional(9.0), *color,
                        );
                    }
                    // 中心圓
                    ui.painter().circle_filled(ax_center, 3.0, egui::Color32::from_rgba_unmultiplied(200, 200, 200, 150));
                    // 方位指示（N/S 對應 -Z/+Z）
                    let north_dir = view.transform_vector3(-glam::Vec3::Z); // -Z = North
                    let compass_len = ax_size * 0.52;
                    let nx = north_dir.x * compass_len;
                    let ny = -north_dir.y * compass_len;
                    let n_tip = egui::pos2(ax_center.x + nx, ax_center.y + ny);
                    ui.painter().text(n_tip, egui::Align2::CENTER_CENTER, "N",
                        egui::FontId::proportional(8.0), egui::Color32::from_rgba_unmultiplied(255, 80, 80, 180));
                }

                // ── Scale bar（左下角比例尺）──
                {
                    // 用兩個已知距離的 3D 點投影到螢幕，計算 pixel/mm
                    let origin = [0.0_f32, 0.0, 0.0];
                    let x1000 = [1000.0, 0.0, 0.0]; // 1m
                    if let (Some(sp0), Some(sp1)) = (
                        Self::world_to_screen_vp(origin, &vp, &rect),
                        Self::world_to_screen_vp(x1000, &vp, &rect),
                    ) {
                        let px_per_mm = ((sp1.x - sp0.x).powi(2) + (sp1.y - sp0.y).powi(2)).sqrt() / 1000.0;
                        if px_per_mm > 0.001 {
                            // 選擇適合的比例尺長度（50-150px）
                            let target_px = 100.0;
                            let mm_at_target = target_px / px_per_mm;
                            // 取整到好看的數字
                            let nice = [100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0];
                            let scale_mm = nice.iter().copied()
                                .min_by_key(|v| ((v - mm_at_target).abs() * 100.0) as i64)
                                .unwrap_or(1000.0);
                            let bar_px = scale_mm * px_per_mm;
                            let bar_y = rect.max.y - 32.0;
                            let bar_x = rect.min.x + 8.0;

                            let bar_color = egui::Color32::from_rgba_unmultiplied(160, 165, 180, 150);
                            // 橫線
                            ui.painter().line_segment(
                                [egui::pos2(bar_x, bar_y), egui::pos2(bar_x + bar_px, bar_y)],
                                egui::Stroke::new(2.0, bar_color),
                            );
                            // 左端帽
                            ui.painter().line_segment(
                                [egui::pos2(bar_x, bar_y - 4.0), egui::pos2(bar_x, bar_y + 4.0)],
                                egui::Stroke::new(1.5, bar_color),
                            );
                            // 右端帽
                            ui.painter().line_segment(
                                [egui::pos2(bar_x + bar_px, bar_y - 4.0), egui::pos2(bar_x + bar_px, bar_y + 4.0)],
                                egui::Stroke::new(1.5, bar_color),
                            );
                            // 標籤
                            let label = if scale_mm >= 1000.0 {
                                format!("{:.0} m", scale_mm / 1000.0)
                            } else {
                                format!("{:.0} mm", scale_mm)
                            };
                            ui.painter().text(
                                egui::pos2(bar_x + bar_px / 2.0, bar_y - 6.0),
                                egui::Align2::CENTER_BOTTOM, &label,
                                egui::FontId::proportional(9.0), bar_color,
                            );
                        }
                    }
                }

                // ── Viewport 資訊欄（左下角）──
                {
                    let obj_count = self.scene.objects.len();
                    let mode_name = match self.viewer.render_mode {
                        RenderMode::Shaded => "著色",
                        RenderMode::Wireframe => "線框",
                        RenderMode::XRay => "X光",
                        RenderMode::HiddenLine => "隱藏線",
                        RenderMode::Monochrome => "單色",
                        RenderMode::Sketch => "草稿",
                    };
                    let plane = match self.viewer.work_plane {
                        1 => "XY", 2 => "YZ", _ => "XZ",
                    };
                    // 選取物件的面積/體積
                    let measure_info = if self.editor.selected_ids.len() == 1 {
                        if let Some(obj) = self.editor.selected_ids.first()
                            .and_then(|id| self.scene.objects.get(id))
                        {
                            let area = kolibri_core::measure::surface_area(obj);
                            let vol = kolibri_core::measure::volume(obj);
                            format!(" | {} | {}", kolibri_core::measure::format_area(area), kolibri_core::measure::format_volume(vol))
                        } else { String::new() }
                    } else { String::new() };
                    let info = format!("{} 物件 | {} | 平面:{}{}", obj_count, mode_name, plane, measure_info);
                    ui.painter().text(
                        egui::pos2(rect.min.x + 8.0, rect.max.y - 8.0),
                        egui::Align2::LEFT_BOTTOM,
                        &info,
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgba_unmultiplied(160, 165, 180, 150),
                    );
                }

                // ── Toast 通知（右下角堆疊）──
                {
                    let now = std::time::Instant::now();
                    self.toasts.retain(|(_, t)| now.duration_since(*t).as_secs_f32() < 4.0);
                    let toast_w = 250.0;
                    let toast_h = 28.0;
                    let margin = 12.0;
                    for (i, (msg, when)) in self.toasts.iter().rev().enumerate() {
                        let age = now.duration_since(*when).as_secs_f32();
                        let alpha = if age > 3.0 { ((4.0 - age) * 255.0) as u8 } else { 230 };
                        let y = rect.max.y - margin - (i as f32) * (toast_h + 6.0) - toast_h;
                        let toast_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.max.x - margin - toast_w, y),
                            egui::vec2(toast_w, toast_h),
                        );
                        ui.painter().rect_filled(toast_rect, 8.0,
                            egui::Color32::from_rgba_unmultiplied(40, 42, 55, alpha));
                        ui.painter().rect_stroke(toast_rect, 8.0,
                            egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(76, 139, 245, alpha)));
                        ui.painter().text(
                            toast_rect.left_center() + egui::vec2(10.0, 0.0),
                            egui::Align2::LEFT_CENTER, msg,
                            egui::FontId::proportional(12.0),
                            egui::Color32::from_rgba_unmultiplied(230, 235, 245, alpha),
                        );
                    }
                }
    }
}

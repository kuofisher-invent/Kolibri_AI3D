//! Cursor-related overlays: camera bookmarks, guide lines, cursor hint, ghost line,
//! collision warning, move rubber band, group edit indicator

use eframe::egui;

use crate::app::{KolibriApp, CursorHint, DrawState, PullFace, RenderMode, ScaleHandle, SelectionMode, SnapType, Tool, WorkMode};
use crate::scene::Shape;

impl KolibriApp {
    /// Draw cursor-related overlays (bookmarks, guides, cursor hint, ghost line, etc.)
    pub(crate) fn draw_cursor_overlays(
        &mut self,
        ui: &mut egui::Ui,
        vp: glam::Mat4,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
                // 渲染模式 Pill 已移至右側面板 DISPLAY 區塊

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
                if let DrawState::MoveFrom { from, .. } = &self.editor.draw_state {
                    // SU-style click-click Move: 用 from 作為 move_origin
                    self.editor.move_origin = Some(*from);
                } else if self.editor.tool == Tool::Move && self.editor.drag_snapshot_taken && !self.editor.selected_ids.is_empty() {
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

                // ── A2/A3: Cursor-following dimension during drag/push-pull/move/pullclick ──
                self.editor.cursor_dimension = match &self.editor.draw_state {
                    // SU-style MoveFrom: 顯示移動距離
                    DrawState::MoveFrom { from, .. } => {
                        if let Some(to) = self.editor.mouse_ground {
                            let dx = to[0] - from[0];
                            let dy = to[1] - from[1];
                            let dz = to[2] - from[2];
                            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                            if dist > 1.0 {
                                Some((self.editor.mouse_screen[0] + 20.0, self.editor.mouse_screen[1] - 20.0,
                                    format!("移動 {}", if dist >= 1000.0 { format!("{:.2} m", dist / 1000.0) } else { format!("{:.0} mm", dist) })))
                            } else { None }
                        } else { None }
                    }
                    // SU-style PullClick: 顯示推拉距離
                    DrawState::PullClick { ref obj_id, face, original_dim } => {
                        if let Some(obj) = self.scene.objects.get(obj_id) {
                            let current_dim = match (&obj.shape, face) {
                                (Shape::Box { height, .. }, PullFace::Top | PullFace::Bottom) => *height,
                                (Shape::Box { depth, .. }, PullFace::Front | PullFace::Back) => *depth,
                                (Shape::Box { width, .. }, PullFace::Left | PullFace::Right) => *width,
                                (Shape::Cylinder { height, .. }, _) => *height,
                                (Shape::SteelProfile { length, .. }, PullFace::Top | PullFace::Bottom) => *length,
                                (Shape::SteelProfile { params, .. }, PullFace::Front | PullFace::Back) => params.h,
                                (Shape::SteelProfile { params, .. }, PullFace::Left | PullFace::Right) => params.b,
                                _ => 0.0,
                            };
                            let delta = current_dim - original_dim;
                            if delta.abs() > 0.5 {
                                Some((self.editor.mouse_screen[0] + 20.0, self.editor.mouse_screen[1] - 20.0,
                                    format!("推拉 {:.0} mm", delta)))
                            } else { None }
                        } else { None }
                    }
                    DrawState::Pulling { obj_id, face, original_dim, .. } => {
                        if let Some(obj) = self.scene.objects.get(obj_id) {
                            let current_dim = match (&obj.shape, face) {
                                (Shape::Box { height, .. }, PullFace::Top | PullFace::Bottom) => *height,
                                (Shape::Box { depth, .. }, PullFace::Front | PullFace::Back) => *depth,
                                (Shape::Box { width, .. }, PullFace::Left | PullFace::Right) => *width,
                                (Shape::Cylinder { height, .. }, _) => *height,
                                (Shape::SteelProfile { length, .. }, PullFace::Top | PullFace::Bottom) => *length,
                                (Shape::SteelProfile { params, .. }, PullFace::Front | PullFace::Back) => params.h,
                                (Shape::SteelProfile { params, .. }, PullFace::Left | PullFace::Right) => params.b,
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
                            // Shift held = force uniform mode
                            let effective_handle = if self.editor.shift_held { ScaleHandle::Uniform } else { *handle };
                            let mode_prefix = match effective_handle {
                                ScaleHandle::Uniform => "[Uniform] ",
                                ScaleHandle::AxisX => "[X] ",
                                ScaleHandle::AxisY => "[Y] ",
                                ScaleHandle::AxisZ => "[Z] ",
                            };
                            let text = match effective_handle {
                                ScaleHandle::Uniform => {
                                    let ratio = if original_dims[0] > 0.1 { current[0] / original_dims[0] } else { 1.0 };
                                    format!("{}\u{00d7}{:.2}", mode_prefix, ratio)
                                }
                                ScaleHandle::AxisX => format!("{}W: {:.0} mm (\u{00d7}{:.2})", mode_prefix, current[0], current[0] / original_dims[0].max(1.0)),
                                ScaleHandle::AxisY => format!("{}H: {:.0} mm (\u{00d7}{:.2})", mode_prefix, current[1], current[1] / original_dims[1].max(1.0)),
                                ScaleHandle::AxisZ => format!("{}D: {:.0} mm (\u{00d7}{:.2})", mode_prefix, current[2], current[2] / original_dims[2].max(1.0)),
                            };
                            Some((self.editor.mouse_screen[0] + 20.0, self.editor.mouse_screen[1] - 20.0, text))
                        } else { None }
                    }
                    // SU-style Measuring: 即時顯示量測距離
                    DrawState::Measuring { start } => {
                        if let Some(end) = self.editor.mouse_ground {
                            let dx = end[0] - start[0];
                            let dy = end[1] - start[1];
                            let dz = end[2] - start[2];
                            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                            if dist > 1.0 {
                                let text = if dist >= 1000.0 {
                                    format!("距離 {:.2} m", dist / 1000.0)
                                } else {
                                    format!("距離 {:.0} mm", dist)
                                };
                                Some((self.editor.mouse_screen[0] + 20.0, self.editor.mouse_screen[1] - 20.0, text))
                            } else { None }
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
                        super::draw_dashed_line(ui.painter(), s1, s2, egui::Stroke::new(1.5, ghost_color), 8.0, 6.0);
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
    }
}

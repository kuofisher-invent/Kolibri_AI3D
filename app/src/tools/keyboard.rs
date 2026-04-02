use eframe::egui;

use crate::app::{
    DrawState, KolibriApp, Tool,
};
use crate::scene::Shape;

impl KolibriApp {
    /// 鍵盤快捷鍵處理 — 從 handle_viewport() 中提取
    pub(crate) fn handle_keyboard(&mut self, response: &egui::Response, ui: &egui::Ui) {
        let shift = ui.input(|i| i.modifiers.shift);

        if response.has_focus() || response.hovered() {
            ui.input(|i| {
                if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                    let ids = std::mem::take(&mut self.editor.selected_ids);
                    for id in &ids {
                        self.ai_log.log(&self.current_actor.clone(), "刪除���件", id, vec![id.clone()]);
                        self.scene.delete(id);
                    }
                }
                if i.key_pressed(egui::Key::Escape) {
                    // MoveFrom: ESC cancels → restore original positions
                    if let DrawState::MoveFrom { ref obj_ids, ref original_positions, .. } = self.editor.draw_state.clone() {
                        for (i, id) in obj_ids.iter().enumerate() {
                            if let Some(obj) = self.scene.objects.get_mut(id) {
                                obj.position = original_positions[i];
                            }
                        }
                        self.scene.undo();
                        self.editor.draw_state = DrawState::Idle;
                    }
                    // PullClick: ESC cancels → undo snapshot
                    else if matches!(self.editor.draw_state, DrawState::PullClick { .. }) {
                        self.scene.undo();
                        self.editor.draw_state = DrawState::Idle;
                        self.editor.selected_face = None;
                    }
                    // RotateAngle: ESC cancels — restore original rotations
                    else if let DrawState::RotateAngle { ref obj_ids, ref original_rotations, .. } = self.editor.draw_state.clone() {
                        for (i, id) in obj_ids.iter().enumerate() {
                            let orig = original_rotations.get(i).copied().unwrap_or(0.0);
                            if let Some(obj) = self.scene.objects.get_mut(id) {
                                obj.rotation_y = orig;
                            }
                        }
                        self.scene.undo(); // 撤銷 snapshot
                        self.editor.draw_state = DrawState::Idle;
                    }
                    // FollowPath: ESC finishes the path and creates extrusion
                    else if let DrawState::FollowPath { ref source_id, ref path_points } = self.editor.draw_state.clone() {
                        if path_points.len() >= 2 {
                            let pid = source_id.clone();
                            let pts = path_points.clone();
                            self.extrude_along_path(&pid, &pts);
                        }
                        self.editor.draw_state = DrawState::Idle;
                    }
                    else {
                        // Piping: ESC cancels pipe drawing
                        #[cfg(feature = "piping")]
                        {
                            if matches!(self.editor.tool, Tool::PipeDraw | Tool::PipeFitting) {
                                self.editor.piping.cancel();
                            }
                        }
                        // SU-style ESC 兩段式：
                        // 1. 如果有選取，先清除選取（保持目前工具）
                        // 2. 如果在非 Idle 狀態，回到 Idle（保持目前工具）
                        // 3. 如果已經沒選取且 Idle，才切到 Select
                        if !self.editor.selected_ids.is_empty() || self.editor.selected_face.is_some() {
                            // 第一段：清除選取
                            self.editor.selected_ids.clear();
                            self.editor.selected_face = None;
                            self.editor.locked_axis = None;
                            self.editor.sticky_axis = None;
                            self.editor.suggestion = None;
                        } else if self.editor.editing_group_id.is_some() {
                            // 退出群組編輯
                            self.editor.editing_group_id = None;
                        } else if self.editor.tool != Tool::Select {
                            // 第二段：切到 Select
                            self.editor.tool = Tool::Select;
                            self.editor.draw_state = DrawState::Idle;
                            // Inference 2.0: reset context on ESC
                            crate::inference::reset_context(&mut self.editor.inference_ctx);
                            self.editor.inference_ctx.current_tool = Tool::Select;
                        }
                    }
                }
                // Enter key: finish FollowPath extrusion
                if let DrawState::FollowPath { ref source_id, ref path_points } = self.editor.draw_state.clone() {
                    if i.key_pressed(egui::Key::Enter) && path_points.len() >= 2 {
                        let pid = source_id.clone();
                        let pts = path_points.clone();
                        self.extrude_along_path(&pid, &pts);
                        self.editor.draw_state = DrawState::Idle;
                    }
                }
                // Undo / Redo (Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z)
                let ctrl = i.modifiers.ctrl || i.modifiers.mac_cmd;
                if ctrl && i.key_pressed(egui::Key::Z) {
                    if i.modifiers.shift {
                        self.scene.redo();
                    } else {
                        self.scene.undo();
                    }
                }
                if ctrl && i.key_pressed(egui::Key::Y) {
                    self.scene.redo();
                }
                // Save / Open (Ctrl+S, Ctrl+O)
                if ctrl && i.key_pressed(egui::Key::S) {
                    self.save_scene();
                }
                if ctrl && i.key_pressed(egui::Key::O) {
                    self.open_scene();
                }
                if ctrl && i.key_pressed(egui::Key::A) {
                    self.editor.selected_ids = self.scene.objects.keys().cloned().collect();
                }
                // Copy (Ctrl+C)
                if ctrl && i.key_pressed(egui::Key::C) && !self.editor.selected_ids.is_empty() {
                    self.editor.clipboard = self.editor.selected_ids.iter()
                        .filter_map(|id| self.scene.objects.get(id).cloned())
                        .collect();
                    self.file_message = Some((
                        format!("已複製 {} 個物件", self.editor.clipboard.len()),
                        std::time::Instant::now(),
                    ));
                }
                // Paste (Ctrl+V)
                if ctrl && i.key_pressed(egui::Key::V) && !self.editor.clipboard.is_empty() {
                    self.scene.snapshot();
                    let mut new_ids = Vec::new();
                    let offset = [500.0_f32, 0.0, 500.0];
                    for obj in &self.editor.clipboard {
                        let mut clone = obj.clone();
                        clone.id = self.scene.next_id_pub();
                        clone.name = format!("{}_paste", clone.name);
                        clone.position[0] += offset[0];
                        clone.position[2] += offset[2];
                        new_ids.push(clone.id.clone());
                        self.scene.objects.insert(clone.id.clone(), clone);
                    }
                    self.scene.version += 1;
                    self.editor.selected_ids = new_ids;
                    self.file_message = Some((
                        format!("已貼上 {} ���物件", self.editor.clipboard.len()),
                        std::time::Instant::now(),
                    ));
                }
                // Cut (Ctrl+X)
                if ctrl && i.key_pressed(egui::Key::X) && !self.editor.selected_ids.is_empty() {
                    self.editor.clipboard = self.editor.selected_ids.iter()
                        .filter_map(|id| self.scene.objects.get(id).cloned())
                        .collect();
                    self.scene.snapshot();
                    let ids = std::mem::take(&mut self.editor.selected_ids);
                    for id in &ids {
                        self.scene.delete(id);
                    }
                    self.file_message = Some((
                        format!("已剪下 {} 個物件", self.editor.clipboard.len()),
                        std::time::Instant::now(),
                    ));
                }

                // Duplicate (Ctrl+D)
                if ctrl && i.key_pressed(egui::Key::D) && !self.editor.selected_ids.is_empty() {
                    self.scene.snapshot();
                    let mut new_ids = Vec::new();
                    for id in &self.editor.selected_ids.clone() {
                        if let Some(obj) = self.scene.objects.get(id).cloned() {
                            let mut clone = obj;
                            clone.id = self.scene.next_id_pub();
                            clone.name = format!("{}_dup", clone.name);
                            clone.position[0] += 300.0;
                            clone.position[2] += 300.0;
                            new_ids.push(clone.id.clone());
                            self.scene.objects.insert(clone.id.clone(), clone);
                        }
                    }
                    self.scene.version += 1;
                    self.editor.selected_ids = new_ids.clone();
                    self.file_message = Some((
                        format!("已��製 {} 個物件", new_ids.len()),
                        std::time::Instant::now(),
                    ));
                }
                // Invert Selection (Ctrl+I)
                if ctrl && i.key_pressed(egui::Key::I) {
                    let all: std::collections::HashSet<String> = self.scene.objects.keys().cloned().collect();
                    let sel: std::collections::HashSet<String> = self.editor.selected_ids.iter().cloned().collect();
                    self.editor.selected_ids = all.difference(&sel).cloned().collect();
                }

                // Mirror (Ctrl+M = X, Ctrl+Shift+M 循環 Y/Z)
                if ctrl && i.key_pressed(egui::Key::M) && !self.editor.selected_ids.is_empty() {
                    let axis = if shift { 2 } else { 0 }; // Shift+Ctrl+M = Z, Ctrl+M = X
                    self.mirror_selected(axis, true); // true = 建立副本
                }

                // Copy properties (Ctrl+Shift+C)
                if ctrl && shift && i.key_pressed(egui::Key::C) {
                    if let Some(obj) = self.editor.selected_ids.first()
                        .and_then(|id| self.scene.objects.get(id))
                    {
                        self.editor.property_clipboard = Some((obj.material, obj.roughness, obj.metallic));
                        self.file_message = Some(("已複製屬性".into(), std::time::Instant::now()));
                    }
                }
                // Paste properties (Ctrl+Shift+V)
                if ctrl && shift && i.key_pressed(egui::Key::V) {
                    if let Some((mat, rough, metal)) = self.editor.property_clipboard {
                        let ids: Vec<&str> = self.editor.selected_ids.iter().map(|s| s.as_str()).collect();
                        if !ids.is_empty() {
                            self.scene.snapshot_ids(&ids, "貼��屬性");
                            for id in &self.editor.selected_ids.clone() {
                                if let Some(obj) = self.scene.objects.get_mut(id) {
                                    obj.material = mat;
                                    obj.roughness = rough;
                                    obj.metallic = metal;
                                }
                            }
                            self.scene.version += 1;
                            self.file_message = Some((
                                format!("已貼上屬性到 {} 個物件", ids.len()),
                                std::time::Instant::now(),
                            ));
                        }
                    }
                }

                // Hide selected (Alt+H)
                let alt = i.modifiers.alt;
                if alt && i.key_pressed(egui::Key::H) {
                    if shift {
                        for obj in self.scene.objects.values_mut() { obj.visible = true; }
                        self.scene.version += 1;
                        self.file_message = Some(("全部顯示".into(), std::time::Instant::now()));
                    } else if !self.editor.selected_ids.is_empty() {
                        for id in &self.editor.selected_ids.clone() {
                            if let Some(obj) = self.scene.objects.get_mut(id) { obj.visible = false; }
                        }
                        self.scene.version += 1;
                        self.editor.selected_ids.clear();
                        self.file_message = Some(("已隱藏選取物件".into(), std::time::Instant::now()));
                    }
                }
                // Isolate selected (Alt+I)
                if alt && i.key_pressed(egui::Key::I) && !self.editor.selected_ids.is_empty() {
                    let sel_set: std::collections::HashSet<&str> = self.editor.selected_ids.iter().map(|s| s.as_str()).collect();
                    for obj in self.scene.objects.values_mut() {
                        obj.visible = sel_set.contains(obj.id.as_str());
                    }
                    self.scene.version += 1;
                    self.file_message = Some(("已隔離顯示選取物件".into(), std::time::Instant::now()));
                }

                // Unsaved changes confirmation Y/N (takes priority)
                if self.pending_action.is_some() {
                    if i.key_pressed(egui::Key::Y) {
                        if let Some(action) = self.pending_action.take() {
                            self.force_menu_action(action);
                        }
                    }
                    if i.key_pressed(egui::Key::N) {
                        self.pending_action = None;
                    }
                }
                // AI suggestion Y/N
                else if self.editor.suggestion.is_some() {
                    if i.key_pressed(egui::Key::Y) {
                        if let Some(suggestion) = self.editor.suggestion.take() {
                            self.apply_suggestion(suggestion.action);
                        }
                    }
                    if i.key_pressed(egui::Key::N) {
                        self.editor.suggestion = None;
                    }
                }

                // 弧線工具啟用時，按 Ctrl 循環切換模式
                if matches!(self.editor.tool, Tool::Arc | Tool::Arc3Point | Tool::Pie) {
                    if ctrl && !self.editor.ctrl_was_down {
                        let next = match self.editor.tool {
                            Tool::Arc       => Tool::Arc3Point,
                            Tool::Arc3Point => Tool::Pie,
                            Tool::Pie       => Tool::Arc,
                            _ => Tool::Arc,
                        };
                        let label = match next {
                            Tool::Arc3Point => "三點弧",
                            Tool::Pie       => "扇形",
                            _               => "兩點弧",
                        };
                        self.console_push("TOOL", format!("弧線模式: {}", label));
                        self.editor.tool = next;
                        self.editor.draw_state = DrawState::Idle;
                    }
                    self.editor.ctrl_was_down = ctrl;
                }

                // Shortcut keys (SketchUp-style) — only when Ctrl is NOT held
                let set = |tool: Tool, this: &mut Self| {
                    this.console_push("TOOL", format!("切換工具: {:?}", tool));
                    this.editor.tool = tool;
                    this.editor.draw_state = DrawState::Idle;
                };
                if !ctrl {
                    if i.key_pressed(egui::Key::Space) { set(Tool::Select, self); }
                    if i.key_pressed(egui::Key::M) { set(Tool::Move, self); }
                    if i.key_pressed(egui::Key::Q) { set(Tool::Rotate, self); }
                    if i.key_pressed(egui::Key::L) { set(Tool::Line, self); }
                    if i.key_pressed(egui::Key::A) { set(Tool::Arc, self); }
                    if i.key_pressed(egui::Key::R) { set(Tool::Rectangle, self); }
                    if i.key_pressed(egui::Key::C) { set(Tool::Circle, self); }
                    if i.key_pressed(egui::Key::B) { set(Tool::CreateBox, self); }
                    if i.key_pressed(egui::Key::S) { set(Tool::CreateSphere, self); }
                    if i.key_pressed(egui::Key::P) { set(Tool::PushPull, self); }
                    if i.key_pressed(egui::Key::F) { set(Tool::Offset, self); }
                    if i.key_pressed(egui::Key::T) { set(Tool::TapeMeasure, self); }
                    if i.key_pressed(egui::Key::D) { set(Tool::Dimension, self); }
                    if i.key_pressed(egui::Key::O) { set(Tool::Orbit, self); }
                    if i.key_pressed(egui::Key::H) { set(Tool::Pan, self); }
                    if i.key_pressed(egui::Key::Z) { self.zoom_extents(); }
                    if i.key_pressed(egui::Key::G) { set(Tool::Group, self); }
                    if i.key_pressed(egui::Key::E) { set(Tool::Eraser, self); }
                    if i.key_pressed(egui::Key::W) && !ctrl { set(Tool::Wall, self); }

                    // ── 鋼構接頭快捷鍵（Shift+數字）──
                    #[cfg(feature = "steel")]
                    if shift {
                        // Shift+1: AISC 接頭對話框（需 ≥1 構件）
                        if i.key_pressed(egui::Key::Num1) {
                            if !self.editor.selected_ids.is_empty() {
                                self.open_connection_dialog();
                            } else {
                                self.file_message = Some(("Shift+1: 請先選取構件".into(), std::time::Instant::now()));
                            }
                        }
                        // Shift+2: AISC 建議（Console）
                        if i.key_pressed(egui::Key::Num2) {
                            if !self.editor.selected_ids.is_empty() {
                                self.show_aisc_suggestion();
                            } else {
                                self.file_message = Some(("Shift+2: 請先選取構件".into(), std::time::Instant::now()));
                            }
                        }
                        // Shift+3: 自動編號
                        if i.key_pressed(egui::Key::Num3) {
                            self.run_auto_numbering();
                        }
                        // Shift+4: 碰撞偵測
                        if i.key_pressed(egui::Key::Num4) {
                            self.run_collision_check();
                        }
                    }

                    // Standard view shortcuts (number row + numpad)
                    if i.key_pressed(egui::Key::Num1) { self.viewer.animate_camera_to(|c| c.set_front()); }
                    if i.key_pressed(egui::Key::Num2) { self.viewer.animate_camera_to(|c| c.set_top()); }
                    if i.key_pressed(egui::Key::Num3) { self.viewer.animate_camera_to(|c| c.set_iso()); }
                    if i.key_pressed(egui::Key::Num4) { self.viewer.animate_camera_to(|c| c.set_left()); }
                    if i.key_pressed(egui::Key::Num6) { self.viewer.animate_camera_to(|c| c.set_right()); }
                    if i.key_pressed(egui::Key::Num8) { self.viewer.animate_camera_to(|c| c.set_back()); }
                    // Zoom to selected (period / numpad decimal)
                    if i.key_pressed(egui::Key::Period) && !self.editor.selected_ids.is_empty() {
                        if let Some(obj) = self.editor.selected_ids.first()
                            .and_then(|id| self.scene.objects.get(id))
                        {
                            let p = glam::Vec3::from(obj.position);
                            let ext = match &obj.shape {
                                Shape::Box { width, height, depth } => glam::Vec3::new(*width, *height, *depth),
                                Shape::Cylinder { radius, height, .. } => glam::Vec3::new(*radius*2.0, *height, *radius*2.0),
                                Shape::Sphere { radius, .. } => glam::Vec3::splat(*radius * 2.0),
                                _ => glam::Vec3::splat(500.0),
                            };
                            self.viewer.camera.target = p + ext * 0.5;
                            self.viewer.camera.distance = ext.length() * 2.0;
                        }
                    }

                    // Axis locking / Rotation axis switching
                    if i.key_pressed(egui::Key::ArrowRight) {
                        // → = X 軸 (紅)
                        self.editor.locked_axis = Some(0);
                        // 旋轉工具：切換旋轉軸
                        if let DrawState::RotateRef { ref mut rotate_axis, .. } = self.editor.draw_state {
                            *rotate_axis = 0;
                            self.file_message = Some(("旋轉軸: X (紅)".into(), std::time::Instant::now()));
                        }
                    }
                    if i.key_pressed(egui::Key::ArrowUp) {
                        // ↑ = Y 軸 (綠) — SU 預設
                        self.editor.locked_axis = Some(1);
                        if let DrawState::RotateRef { ref mut rotate_axis, .. } = self.editor.draw_state {
                            *rotate_axis = 1;
                            self.file_message = Some(("旋轉軸: Y (綠)".into(), std::time::Instant::now()));
                        }
                    }
                    if i.key_pressed(egui::Key::ArrowLeft) {
                        // ← = Z 軸 (藍)
                        self.editor.locked_axis = Some(2);
                        if let DrawState::RotateRef { ref mut rotate_axis, .. } = self.editor.draw_state {
                            *rotate_axis = 2;
                            self.file_message = Some(("旋轉軸: Z (藍)".into(), std::time::Instant::now()));
                        }
                    }
                    if i.key_pressed(egui::Key::ArrowDown) {
                        self.editor.locked_axis = None; // 清除鎖定
                    }
                }

                // Collect digit input for measurement
                if !matches!(self.editor.draw_state, DrawState::Idle) {
                    for ev in &i.events {
                        if let egui::Event::Text(t) = ev {
                            if t.chars().all(|c| c.is_ascii_digit() || c == ',' || c == '.' || c == 'x' || c == 'X' || c == 'r' || c == 'R' || c == '%' || c == 'm' || c == 'c' || c == 'f' || c == 't' || c == '\'' || c == '"') {
                                self.editor.measure_input.push_str(t);
                            }
                        }
                        if let egui::Event::Key { key: egui::Key::Enter, pressed: true, .. } = ev {
                            self.apply_measure();
                        }
                    }
                }
            });
        }
    }
}

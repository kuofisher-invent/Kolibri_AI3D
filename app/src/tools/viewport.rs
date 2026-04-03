use eframe::egui;

use crate::app::{
    compute_arc, DrawState, KolibriApp, PullFace, RenderMode, RightTab, ScaleHandle, SelectionMode, Tool,
};
use crate::camera;
use crate::scene::{MaterialKind, Shape};

impl KolibriApp {
    // ── Viewport interaction ────────────────────────────────────────────────

    pub(crate) fn handle_viewport(&mut self, response: &egui::Response, ui: &egui::Ui) {
        let shift = ui.input(|i| i.modifiers.shift);
        self.editor.shift_held = shift;

        // Track mouse position on ground
        if let Some(hp) = response.hover_pos() {
            let local = hp - response.rect.min;
            self.editor.mouse_screen = [local.x, local.y];
            self.viewer.viewport_size = [response.rect.width(), response.rect.height()];
            let (origin, dir) = self.viewer.camera.screen_ray(local.x, local.y, response.rect.width(), response.rect.height());
            // 工作平面交點（0=Ground XZ, 1=Front XY, 2=Side YZ）
            self.editor.mouse_ground = match self.viewer.work_plane {
                1 => { // XY 平面 (Z = offset)
                    let z = self.viewer.work_plane_offset;
                    if dir.z.abs() > 1e-6 {
                        let t = (z - origin.z) / dir.z;
                        if t > 0.0 { Some([origin.x + dir.x * t, origin.y + dir.y * t, z]) } else { None }
                    } else { None }
                }
                2 => { // YZ 平面 (X = offset)
                    let x = self.viewer.work_plane_offset;
                    if dir.x.abs() > 1e-6 {
                        let t = (x - origin.x) / dir.x;
                        if t > 0.0 { Some([x, origin.y + dir.y * t, origin.z + dir.z * t]) } else { None }
                    } else { None }
                }
                _ => {
                    // Ground XZ，偏移樓層高度
                    let floor_y = self.viewer.current_floor as f32 * self.viewer.floor_height;
                    if dir.y.abs() > 1e-6 {
                        let t = (floor_y - origin.y) / dir.y;
                        if t > 0.0 { Some([origin.x + dir.x * t, floor_y, origin.z + dir.z * t]) } else { None }
                    } else { None }
                }
            };

            // Shift-lock axis (SketchUp-style: hold Shift to lock detected axis)
            // Only when actively drawing or moving, not when idle (Shift+Middle = Pan)
            let in_active_state = !matches!(self.editor.draw_state, DrawState::Idle)
                || (matches!(self.editor.tool, Tool::Move) && !self.editor.selected_ids.is_empty() && response.dragged());
            if shift && in_active_state {
                if let Some(ref snap) = self.editor.snap_result {
                    match snap.snap_type {
                        crate::app::SnapType::AxisX => self.editor.locked_axis = Some(0),
                        crate::app::SnapType::AxisY => self.editor.locked_axis = Some(1),
                        crate::app::SnapType::AxisZ => self.editor.locked_axis = Some(2),
                        _ => {} // keep current lock if any
                    }
                }
            } else if !shift && !ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd) {
                // Release Shift-lock when Shift is released
                // 但不清除 Ctrl 循環設定的軸鎖定
                if self.editor.locked_axis.is_some() && !self.editor.axis_locked_by_ctrl {
                    self.editor.locked_axis = None;
                }
            }

            // Compute smart snap
            // 大場景（> 500 物件）跳過 snap 計算（遍歷所有物件太慢）
            let skip_snap = self.scene.objects.len() > 500;
            // For Line/Arc tools, try face snap first
            if !skip_snap {
                let is_draw_tool = matches!(self.editor.tool, Tool::Line | Tool::Arc | Tool::Arc3Point | Tool::Pie);
                if is_draw_tool {
                    if let Some(face_pt) = self.snap_to_face(local.x, local.y, response.rect.width(), response.rect.height()) {
                        self.editor.mouse_ground = Some(face_pt);
                        self.editor.snap_result = Some(crate::app::SnapResult {
                            position: face_pt,
                            snap_type: crate::app::SnapType::OnFace,
                            from_point: self.get_drawing_origin(),
                        });
                    } else if let Some(raw) = self.editor.mouse_ground {
                        let from = self.get_drawing_origin();
                        let result = self.smart_snap(raw, from);
                        self.editor.mouse_ground = Some(result.position);
                        self.editor.snap_result = Some(result);
                    } else {
                        self.editor.snap_result = None;
                    }
                } else {
                    let raw = self.editor.mouse_ground.unwrap_or([0.0, 0.0, 0.0]);
                    let from = self.get_drawing_origin();
                    let result = self.smart_snap(raw, from);
                    if result.snap_type != crate::app::SnapType::None {
                        self.editor.mouse_ground = Some(result.position);
                    }
                    self.editor.snap_result = Some(result);
                }
            }

            // ── MoveFrom: Ctrl 循環切換軸鎖定 ──
            if matches!(self.editor.draw_state, DrawState::MoveFrom { .. }) {
                let ctrl_now = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
                if ctrl_now && !self.editor.ctrl_was_down {
                    self.editor.locked_axis = match self.editor.locked_axis {
                        None => Some(0),
                        Some(0) => Some(1),
                        Some(1) => Some(2),
                        Some(2) => None,
                        Some(_) => None,
                    };
                    self.editor.axis_locked_by_ctrl = self.editor.locked_axis.is_some();
                    let axis_name = match self.editor.locked_axis {
                        Some(0) => "X (紅)",
                        Some(1) => "Y (綠)",
                        Some(2) => "Z (藍)",
                        _ => "自由",
                    };
                    self.file_message = Some((
                        format!("Move 鎖定軸: {} (Ctrl 切換)", axis_name),
                        std::time::Instant::now(),
                    ));
                }
                self.editor.ctrl_was_down = ctrl_now;
            }

            // ── SU-style Move click-click 即時預覽 ──
            if let DrawState::MoveFrom { from, ref obj_ids, ref original_positions } = self.editor.draw_state.clone() {
                // Y 軸鎖定時用垂直投影，否則用 ground plane XZ
                let (mut dx, mut dy, mut dz) = if self.editor.locked_axis == Some(1) {
                    let y = self.current_vertical_y(from);
                    (0.0, y - from[1], 0.0)
                } else if let Some(to) = self.editor.mouse_ground {
                    (to[0] - from[0], 0.0, to[2] - from[2])
                } else {
                    (0.0, 0.0, 0.0)
                };

                // 鎖定軸
                match self.editor.locked_axis {
                    Some(0) => { dy = 0.0; dz = 0.0; }
                    Some(1) => { dx = 0.0; dz = 0.0; }
                    Some(2) => { dx = 0.0; dy = 0.0; }
                    _ => {}
                }

                if dx.abs() > 0.01 || dy.abs() > 0.01 || dz.abs() > 0.01 {
                    for (i, id) in obj_ids.iter().enumerate() {
                        if let Some(obj) = self.scene.objects.get_mut(id) {
                            let orig = original_positions[i];
                            obj.position = [orig[0] + dx, orig[1] + dy, orig[2] + dz];
                            obj.obj_version += 1;
                        }
                    }
                    self.scene.version += 1;

                    let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                    self.editor.cursor_dimension = Some((dist, 0.0, format!("{:.0}mm", dist)));
                }
            }

            // ── SU-style PushPull click-click 即時預覽 ──
            if let DrawState::PullClick { ref obj_id, face, original_dim } = self.editor.draw_state.clone() {
                let d = response.drag_delta();
                let scale = self.viewer.camera.distance * 0.0015;
                // 用垂直拖曳量計算推拉距離
                let amount = -d.y * scale;
                if amount.abs() > 0.01 {
                    if let Some(obj) = self.scene.objects.get_mut(obj_id.as_str()) {
                        match (&mut obj.shape, face) {
                            (Shape::Box { height, .. }, PullFace::Top) =>
                                *height = (*height + amount).max(10.0),
                            (Shape::Box { height, .. }, PullFace::Bottom) => {
                                let delta = amount.min(*height - 10.0);
                                *height = (*height - delta).max(10.0);
                                obj.position[1] += delta;
                            }
                            (Shape::Box { width, .. }, PullFace::Right) =>
                                *width = (*width + amount).max(10.0),
                            (Shape::Box { width, .. }, PullFace::Left) => {
                                let delta = amount.min(*width - 10.0);
                                *width = (*width - delta).max(10.0);
                                obj.position[0] += delta;
                            }
                            (Shape::Box { depth, .. }, PullFace::Back) =>
                                *depth = (*depth + amount).max(10.0),
                            (Shape::Box { depth, .. }, PullFace::Front) => {
                                let delta = amount.min(*depth - 10.0);
                                *depth = (*depth - delta).max(10.0);
                                obj.position[2] += delta;
                            }
                            (Shape::Cylinder { height, .. }, PullFace::Top) =>
                                *height = (*height + amount).max(10.0),
                            _ => {}
                        }
                    }
                    self.scene.version += 1;
                }
            }

            // Hover pick — 大場景跳過（遍歷所有物件 raycasting 太慢）
            let interactive = !skip_snap && matches!(self.editor.tool,
                Tool::Select | Tool::PushPull | Tool::Move | Tool::Rotate |
                Tool::Scale | Tool::Eraser | Tool::PaintBucket | Tool::Offset |
                Tool::FollowMe
            );
            if interactive && matches!(self.editor.draw_state, DrawState::Idle) {
                self.editor.hovered_id = self.pick(local.x, local.y, response.rect.width(), response.rect.height());
                self.editor.hovered_face = self.pick_face(local.x, local.y, response.rect.width(), response.rect.height());
            } else {
                self.editor.hovered_id = None;
                self.editor.hovered_face = None;
            }
        }

        // Camera controls — SketchUp-style:
        // Middle drag = Orbit, Shift+Middle drag = Pan
        if response.dragged_by(egui::PointerButton::Middle) {
            let d = response.drag_delta();
            if shift {
                self.viewer.camera.pan(d.x, d.y);
            } else {
                self.viewer.camera.orbit(d.x, d.y);
            }
        }
        // Middle click (no drag) = center view on cursor point
        if response.clicked_by(egui::PointerButton::Middle) {
            if let Some(ground) = self.editor.mouse_ground {
                self.viewer.camera.target = glam::Vec3::new(ground[0], ground[1], ground[2]);
                self.file_message = Some(("視角已居中".to_string(), std::time::Instant::now()));
            }
        }
        // Right-drag no longer orbits (right-click is for context menu)
        if response.dragged_by(egui::PointerButton::Primary) && matches!(self.editor.draw_state, DrawState::Idle) {
            let d = response.drag_delta();
            match self.editor.tool {
                Tool::Orbit => self.viewer.camera.orbit(d.x, d.y),
                Tool::Pan => self.viewer.camera.pan(d.x, d.y),
                Tool::Select => {
                    if shift {
                        self.viewer.camera.pan(d.x, d.y);
                    } else if self.editor.rubber_band.is_some() {
                        // Continue rubber band drag
                        if let Some(hp) = response.hover_pos() {
                            if let Some((_, ref mut end)) = self.editor.rubber_band {
                                *end = hp;
                            }
                        }
                    } else if self.editor.selected_ids.is_empty() && self.editor.hovered_id.is_none() {
                        // 只有在沒有選取物件且沒有 hover 物件時才 orbit
                        // 大場景下 hovered_id 可能是 None 但有選取物件，此時不 orbit
                        self.viewer.camera.orbit(d.x, d.y);
                    } else if self.editor.hovered_id.is_none() && self.scene.objects.len() > 500 {
                        // 大場景：沒有 hover 功能，左鍵拖曳 = orbit（用中鍵替代）
                        self.viewer.camera.orbit(d.x, d.y);
                    }
                }
                Tool::Move => {
                    // Move selected objects by dragging
                    // Gizmo 互動：如果 hover 在某軸上，拖曳開始時鎖定該軸
                    if !self.editor.selected_ids.is_empty() {
                        if !self.editor.drag_snapshot_taken {
                            // 設定 gizmo axis lock
                            if let Some(axis) = self.editor.gizmo_hovered_axis {
                                self.editor.gizmo_drag_axis = Some(axis);
                                self.editor.locked_axis = Some(axis);
                            } else {
                                self.editor.gizmo_drag_axis = None;
                            }
                            // Diff undo: 只備份即將被修改的物件
                            let ids: Vec<&str> = self.editor.selected_ids.iter().map(|s| s.as_str()).collect();
                            self.scene.snapshot_ids(&ids, "移動");
                            // If Ctrl held at drag start, duplicate objects first
                            let ctrl_at_start = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
                            if ctrl_at_start {
                                let mut new_ids = Vec::new();
                                for id in &self.editor.selected_ids.clone() {
                                    if let Some(obj) = self.scene.objects.get(id).cloned() {
                                        let mut clone = obj;
                                        clone.id = self.scene.next_id_pub();
                                        clone.name = format!("{}_copy", clone.name);
                                        let new_id = clone.id.clone();
                                        self.scene.objects.insert(new_id.clone(), clone);
                                        new_ids.push(new_id);
                                    }
                                }
                                if !new_ids.is_empty() {
                                    self.editor.selected_ids = new_ids;
                                    self.editor.move_is_copy = true;
                                }
                            }
                            self.editor.drag_snapshot_taken = true;
                        }

                        // Detect Ctrl press EDGE to cycle axis lock (only when not in copy mode)
                        let ctrl_now = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
                        if ctrl_now && !self.editor.ctrl_was_down {
                            // Ctrl just pressed → cycle: None → X(red) → Y(green) → Z(blue) → None
                            self.editor.locked_axis = match self.editor.locked_axis {
                                None => Some(0),
                                Some(0) => Some(1),
                                Some(1) => Some(2),
                                Some(2) => None,
                                Some(_) => None,
                            };
                        }
                        self.editor.ctrl_was_down = ctrl_now;

                        // 用地面投影計算精確的世界座標差（不受相機距離影響）
                        let mx = self.editor.mouse_screen[0];
                        let my = self.editor.mouse_screen[1];
                        let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                        let prev_mx = mx - d.x;
                        let prev_my = my - d.y;

                        // 當前和上一幀的地面投影
                        let (orig_cur, dir_cur) = self.viewer.camera.screen_ray(mx, my, vw, vh);
                        let (orig_prev, dir_prev) = self.viewer.camera.screen_ray(prev_mx, prev_my, vw, vh);

                        // 取物件 Y 高度作為投影平面（而非 Y=0 地面）
                        let obj_y = self.editor.selected_ids.first()
                            .and_then(|id| self.scene.objects.get(id))
                            .map(|o| o.position[1])
                            .unwrap_or(0.0);

                        let hit_cur = {
                            let t = if dir_cur.y.abs() > 1e-6 { (obj_y - orig_cur.y) / dir_cur.y } else { 0.0 };
                            if t > 0.0 { orig_cur + dir_cur * t } else { orig_cur }
                        };
                        let hit_prev = {
                            let t = if dir_prev.y.abs() > 1e-6 { (obj_y - orig_prev.y) / dir_prev.y } else { 0.0 };
                            if t > 0.0 { orig_prev + dir_prev * t } else { orig_prev }
                        };

                        let raw_delta = hit_cur - hit_prev;

                        // 安全限制：每幀最大移動量 = 相機距離 × 0.05
                        // 防止相機接近水平時投影到無窮遠
                        let max_per_frame = self.viewer.camera.distance * 0.05;
                        let raw_delta = glam::Vec3::new(
                            raw_delta.x.clamp(-max_per_frame, max_per_frame),
                            raw_delta.y,
                            raw_delta.z.clamp(-max_per_frame, max_per_frame),
                        );

                        let vert_scale = self.viewer.camera.distance * 0.001;
                        let vert_delta = -d.y * vert_scale;

                        // Apply movement based on locked axis
                        let (dx, dy, dz) = match self.editor.locked_axis {
                            Some(0) => (raw_delta.x, 0.0, 0.0),              // X only (red)
                            Some(1) => (0.0, vert_delta, 0.0),                // Y only (green)
                            Some(2) => (0.0, 0.0, raw_delta.z),              // Z only (blue)
                            _ => (raw_delta.x, 0.0, raw_delta.z),             // Free XZ
                        };

                        let ids = self.editor.selected_ids.clone();
                        // Collision check during move
                        let components = crate::scene::scene_to_collision_components(&self.scene);
                        let config = crate::collision::CollisionConfig::default();
                        for id in &ids {
                            if let Some(obj) = self.scene.objects.get(id) {
                                let (center, size) = crate::scene::obj_collision_center_size(obj);
                                let moving = crate::collision::Component::new(id.clone(), obj.component_kind, center, size);
                                let new_center = [center[0] + dx, center[1] + dy, center[2] + dz];
                                let report = crate::collision::can_move_component(&moving, new_center, &components, &config);
                                if !report.is_allowed {
                                    let note = report.blocking_pairs.first()
                                        .and_then(|p| p.note.as_deref())
                                        .unwrap_or("Illegal geometric penetration");
                                    self.editor.collision_warning = Some(format!("碰撞: {}", note));
                                } else if !report.warning_pairs.is_empty() {
                                    let note = report.warning_pairs.first()
                                        .and_then(|p| p.note.as_deref())
                                        .unwrap_or("Contact detected");
                                    self.editor.collision_warning = Some(format!("警告: {}", note));
                                }
                            }
                            // Always apply the move (warning only, never block)
                            if let Some(obj) = self.scene.objects.get_mut(id) {
                                obj.position[0] += dx;
                                obj.position[1] += dy;
                                obj.position[2] += dz;
                                obj.obj_version += 1;
                            }
                        }
                        self.scene.version += 1; // 拖曳時即時更新渲染
                    } else {
                        self.viewer.camera.orbit(d.x, d.y);
                    }
                }
                Tool::Eraser => {
                    // Drag over objects to continuously delete them
                    if let Some(id) = self.editor.hovered_id.clone() {
                        if !self.editor.drag_snapshot_taken {
                            self.scene.snapshot_ids(&[&id], "橡皮擦");
                            self.editor.drag_snapshot_taken = true;
                        }
                        self.scene.objects.remove(&id);
                        self.scene.version += 1;
                        self.editor.selected_ids.retain(|s| s != &id);
                        self.editor.hovered_id = None;
                    }
                }
                _ => {} // Drawing tools handle clicks, not drags
            }
        }
        // Reset drag snapshot flag when not dragging
        if !response.dragged() {
            // B8: Save move delta for array copy before clearing move state
            if self.editor.drag_snapshot_taken && self.editor.move_is_copy {
                if let Some(origin) = self.editor.move_origin {
                    if let Some(obj) = self.editor.selected_ids.first().and_then(|id| self.scene.objects.get(id)) {
                        self.editor.last_move_delta = Some([
                            obj.position[0] - origin[0],
                            obj.position[1] - origin[1],
                            obj.position[2] - origin[2],
                        ]);
                        self.editor.last_move_was_copy = true;
                    }
                }
            }
            self.editor.drag_snapshot_taken = false;
            self.editor.move_is_copy = false;
            // 清除 gizmo drag lock
            if self.editor.gizmo_drag_axis.is_some() {
                self.editor.gizmo_drag_axis = None;
                self.editor.locked_axis = None;
            }
        }

        // Rubber band selection: start on drag start in Select mode when nothing hovered
        if response.drag_started_by(egui::PointerButton::Primary)
            && matches!(self.editor.tool, Tool::Select)
            && matches!(self.editor.draw_state, DrawState::Idle)
            && !shift
            && self.editor.hovered_id.is_none()
        {
            if let Some(hp) = response.interact_pointer_pos() {
                self.editor.rubber_band = Some((hp, hp));
            }
        }

        // Rubber band selection: finish on drag stop
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            if let Some((start, end)) = self.editor.rubber_band.take() {
                let rect = egui::Rect::from_two_pos(start, end);
                if rect.width() > 3.0 || rect.height() > 3.0 {
                    // CAD 標準：左→右 = Window（完全包含），右→左 = Crossing（交叉）
                    let is_crossing = start.x > end.x;
                    let viewport_rect = response.rect;
                    let mut selected = if shift { self.editor.selected_ids.clone() } else { Vec::new() };
                    for obj in self.scene.objects.values() {
                        let p = obj.position;
                        let (min_p, max_p) = match &obj.shape {
                            Shape::Box { width, height, depth } =>
                                (p, [p[0] + width, p[1] + height, p[2] + depth]),
                            Shape::Cylinder { radius, height, .. } =>
                                (p, [p[0] + radius * 2.0, p[1] + height, p[2] + radius * 2.0]),
                            Shape::Sphere { radius, .. } =>
                                (p, [p[0] + radius * 2.0, p[1] + radius * 2.0, p[2] + radius * 2.0]),
                            Shape::Line { points, thickness, .. } => {
                                let mut mx = p;
                                for pt in points {
                                    mx[0] = mx[0].max(pt[0] + thickness);
                                    mx[1] = mx[1].max(pt[1] + thickness);
                                    mx[2] = mx[2].max(pt[2] + thickness);
                                }
                                (p, mx)
                            }
                            Shape::Mesh(ref mesh) => {
                                let (min, max) = mesh.aabb();
                                ([min[0]+p[0], min[1]+p[1], min[2]+p[2]],
                                 [max[0]+p[0], max[1]+p[1], max[2]+p[2]])
                            }
                        };
                        let corners = [
                            [min_p[0], min_p[1], min_p[2]],
                            [max_p[0], min_p[1], min_p[2]],
                            [min_p[0], max_p[1], min_p[2]],
                            [max_p[0], max_p[1], min_p[2]],
                            [min_p[0], min_p[1], max_p[2]],
                            [max_p[0], min_p[1], max_p[2]],
                            [min_p[0], max_p[1], max_p[2]],
                            [max_p[0], max_p[1], max_p[2]],
                        ];
                        let screen_pts: Vec<bool> = corners.iter().map(|c| {
                            self.world_to_screen(*c, &viewport_rect)
                                .map_or(false, |sp| rect.contains(sp))
                        }).collect();
                        let hit = if is_crossing {
                            screen_pts.iter().any(|&v| v) // 交叉：任一角在內
                        } else {
                            screen_pts.iter().all(|&v| v) // 窗選：全部角在內
                        };
                        if hit && !selected.contains(&obj.id) {
                            selected.push(obj.id.clone());
                        }
                    }
                    self.editor.selected_ids = selected;
                    if !self.editor.selected_ids.is_empty() {
                        self.right_tab = RightTab::Properties;
                    }
                }
            }
        }

        // Zoom toward cursor position (SketchUp-style)
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                let world_point = self.editor.mouse_ground.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
                self.viewer.camera.zoom_toward(scroll, world_point);
            }
        }

        // Scale drag with axis-constrained handles
        // Shift = force uniform, Ctrl = cycle axis mode
        if let DrawState::Scaling { ref obj_id, handle, original_dims: _ } = self.editor.draw_state.clone() {
            if response.dragged_by(egui::PointerButton::Primary) {
                if !self.editor.drag_snapshot_taken {
                    self.scene.snapshot_ids(&[&obj_id], "縮放");
                    self.editor.drag_snapshot_taken = true;
                }
                let dy = -response.drag_delta().y;
                let factor = 1.0 + dy * 0.005;
                // Shift held = force uniform regardless of face-detected handle
                let effective_handle = if shift { ScaleHandle::Uniform } else { handle };
                if let Some(obj) = self.scene.objects.get_mut(obj_id.as_str()) {
                    match (&mut obj.shape, effective_handle) {
                        // Box: uniform scaling
                        (Shape::Box { width, height, depth }, ScaleHandle::Uniform) => {
                            *width = (*width * factor).max(10.0);
                            *height = (*height * factor).max(10.0);
                            *depth = (*depth * factor).max(10.0);
                        }
                        // Box: X-axis only (width)
                        (Shape::Box { width, .. }, ScaleHandle::AxisX) => {
                            *width = (*width * factor).max(10.0);
                        }
                        // Box: Y-axis only (height)
                        (Shape::Box { height, .. }, ScaleHandle::AxisY) => {
                            *height = (*height * factor).max(10.0);
                        }
                        // Box: Z-axis only (depth)
                        (Shape::Box { depth, .. }, ScaleHandle::AxisZ) => {
                            *depth = (*depth * factor).max(10.0);
                        }
                        // Cylinder: uniform
                        (Shape::Cylinder { radius, height, .. }, ScaleHandle::Uniform) => {
                            *radius = (*radius * factor).max(10.0);
                            *height = (*height * factor).max(10.0);
                        }
                        // Cylinder: Y = height only
                        (Shape::Cylinder { height, .. }, ScaleHandle::AxisY) => {
                            *height = (*height * factor).max(10.0);
                        }
                        // Cylinder: X or Z = radius
                        (Shape::Cylinder { radius, .. }, ScaleHandle::AxisX | ScaleHandle::AxisZ) => {
                            *radius = (*radius * factor).max(10.0);
                        }
                        // Sphere: always uniform
                        (Shape::Sphere { radius, .. }, _) => {
                            *radius = (*radius * factor).max(10.0);
                        }
                        // Line: thickness
                        (Shape::Line { thickness, .. }, _) => {
                            *thickness = (*thickness * factor).max(1.0);
                        }
                        // Mesh: not yet implemented
                        (Shape::Mesh(_), _) => {}
                    }
                }
            }
            if response.drag_stopped() || response.clicked() {
                // 元件同步：縮放完成後同步所有同一元件的實例
                let oid = obj_id.clone();
                self.editor.draw_state = DrawState::Idle;
                self.editor.drag_snapshot_taken = false;
                self.scene.auto_sync_component(&oid);
            }
        }

        // ── Ctrl 切換旋轉軸（璇盤定位階段 RotateRef）──
        // 按 Ctrl 循環: Y(1,綠) → X(0,紅) → Z(2,藍) → Y
        if matches!(self.editor.draw_state, DrawState::RotateRef { .. }) {
            let ctrl_now = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
            if ctrl_now && !self.editor.ctrl_was_down {
                if let DrawState::RotateRef { ref mut rotate_axis, .. } = self.editor.draw_state {
                    *rotate_axis = match *rotate_axis {
                        1 => 0, // Y→X
                        0 => 2, // X→Z
                        2 => 1, // Z→Y
                        _ => 1,
                    };
                    let axis_name = ["X (紅)","Y (綠)","Z (藍)"][(*rotate_axis).min(2) as usize];
                    self.file_message = Some((
                        format!("旋轉軸: {} — Ctrl 切換", axis_name),
                        std::time::Instant::now(),
                    ));
                }
            }
            self.editor.ctrl_was_down = ctrl_now;
        }

        // D1: Rotate — live preview (支援 XYZ 軸)
        // 統一用螢幕空間角度：把 3D 璇盤中心投影到螢幕，用滑鼠到投影中心的角度
        // 修正：A) 正確公轉 B) 方向一致 C) 角度連續化避免閃爍
        if let DrawState::RotateAngle { ref obj_ids, center, ref_angle, ref mut current_angle, ref original_rotations, ref original_positions, rotate_axis } = self.editor.draw_state.clone() {
            // 統一用螢幕空間角度（所有軸都一致）
            let aspect = self.viewer.viewport_size[0] / self.viewer.viewport_size[1].max(1.0);
            let vp_mat = self.viewer.camera.view_proj(aspect);
            let vp_rect = eframe::egui::Rect::from_min_size(
                eframe::egui::pos2(0.0, 0.0),
                eframe::egui::vec2(self.viewer.viewport_size[0], self.viewer.viewport_size[1]),
            );
            let mouse_angle_opt = KolibriApp::world_to_screen_vp(center, &vp_mat, &vp_rect).map(|c_scr| {
                let mx = self.editor.mouse_screen[0];
                let my = self.editor.mouse_screen[1];
                let dx = mx - c_scr.x;
                let dy = -(my - c_scr.y); // 螢幕 Y 向上
                dy.atan2(dx)
            });
            if let Some(mouse_angle) = mouse_angle_opt {
                // 角度連續化：把 delta 正規化到 [-π, π]，避免 atan2 邊界跳 2π (修 C 閃爍)
                let mut delta = mouse_angle - ref_angle;
                while delta > std::f32::consts::PI { delta -= 2.0 * std::f32::consts::PI; }
                while delta < -std::f32::consts::PI { delta += 2.0 * std::f32::consts::PI; }

                // 15° snap
                let snap_angle = 15.0_f32.to_radians();
                let snapped = (delta / snap_angle).round() * snap_angle;
                if (delta - snapped).abs() < 3.0_f32.to_radians() { delta = snapped; }

                let cos_d = delta.cos();
                let sin_d = delta.sin();

                // 先算群組幾何中心（所有被旋轉物件的平均中心）
                let mut group_center = [0.0_f32; 3];
                let mut group_count = 0u32;
                for (i, id) in obj_ids.iter().enumerate() {
                    let orig_pos = original_positions.get(i).copied().unwrap_or([0.0; 3]);
                    if let Some(obj) = self.scene.objects.get(id) {
                        let half = crate::renderer::mesh_builder::shape_half_size(&obj.shape);
                        group_center[0] += orig_pos[0] + half[0];
                        group_center[1] += orig_pos[1] + half[1];
                        group_center[2] += orig_pos[2] + half[2];
                        group_count += 1;
                    }
                }
                if group_count > 0 {
                    group_center[0] /= group_count as f32;
                    group_center[1] /= group_count as f32;
                    group_center[2] /= group_count as f32;
                }

                // 群組中心繞璇盤中心公轉
                let new_gc = {
                    let (pa, pb) = match rotate_axis {
                        0 => (group_center[1] - group_center[1], group_center[2] - center[2]),
                        2 => (group_center[0] - center[0], group_center[1] - group_center[1]),
                        _ => (group_center[0] - center[0], group_center[2] - center[2]),
                    };
                    let na = pa * cos_d - pb * sin_d;
                    let nb = pa * sin_d + pb * cos_d;
                    match rotate_axis {
                        0 => [group_center[0], group_center[1] + na, center[2] + nb],
                        2 => [center[0] + na, group_center[1] + nb, group_center[2]],
                        _ => [center[0] + na, group_center[1], center[2] + nb],
                    }
                };

                for (i, id) in obj_ids.iter().enumerate() {
                    let orig_rot = original_rotations.get(i).copied().unwrap_or(0.0);
                    let orig_pos = original_positions.get(i).copied().unwrap_or([0.0; 3]);
                    if let Some(obj) = self.scene.objects.get_mut(id) {
                        obj.rotation_xyz[rotate_axis.min(2) as usize] = orig_rot + delta;
                        // 永遠同步 rotation_y（mesh_builder Y-only 快速路徑需要）
                        obj.rotation_y = obj.rotation_xyz[1];

                        // 角點繞群組中心公轉：P_new = new_gc + R(P+half - gc) - half
                        let half = crate::renderer::mesh_builder::shape_half_size(&obj.shape);
                        let d = [
                            (orig_pos[0] + half[0]) - group_center[0],
                            (orig_pos[1] + half[1]) - group_center[1],
                            (orig_pos[2] + half[2]) - group_center[2],
                        ];
                        let rd = match rotate_axis {
                            0 => [d[0], d[1]*cos_d - d[2]*sin_d, d[1]*sin_d + d[2]*cos_d],
                            2 => [d[0]*cos_d - d[1]*sin_d, d[0]*sin_d + d[1]*cos_d, d[2]],
                            _ => [d[0]*cos_d - d[2]*sin_d, d[1], d[0]*sin_d + d[2]*cos_d],
                        };
                        obj.position[0] = new_gc[0] + rd[0] - half[0];
                        obj.position[1] = new_gc[1] + rd[1] - half[1];
                        obj.position[2] = new_gc[2] + rd[2] - half[2];
                        obj.obj_version += 1;
                    }
                }

                let axis_name = ["X","Y","Z"][rotate_axis.min(2) as usize];
                let deg = delta.to_degrees();
                self.editor.cursor_dimension = Some((deg, 0.0, format!("{:.1}° ({}軸)", deg, axis_name)));

                self.editor.draw_state = DrawState::RotateAngle {
                    obj_ids: obj_ids.to_vec(), center, ref_angle, current_angle: mouse_angle,
                    original_rotations: original_rotations.to_vec(),
                    original_positions: original_positions.to_vec(),
                    rotate_axis,
                };
                self.scene.version += 1;
            } // if let Some(mouse_angle)
        }

        // Offset drag — face edge inset/outset with live preview (SketchUp-style)
        // Drag right = inset (positive distance), drag left = outset (negative distance)
        if let DrawState::Offsetting { ref obj_id, face, distance: _ } = self.editor.draw_state.clone() {
            if response.dragged_by(egui::PointerButton::Primary) {
                if !self.editor.drag_snapshot_taken {
                    self.scene.snapshot_ids(&[&obj_id], "偏移");
                    self.editor.drag_snapshot_taken = true;
                }
                let scale = self.viewer.camera.distance * 0.001;
                let delta = response.drag_delta().x * scale;
                let cur_d = match &self.editor.draw_state {
                    DrawState::Offsetting { distance, .. } => *distance,
                    _ => 0.0,
                };
                // Allow both positive (inset) and negative (outset) offset
                let new_d = cur_d + delta;
                self.editor.draw_state = DrawState::Offsetting { obj_id: obj_id.clone(), face, distance: new_d };
            }
            if response.drag_stopped() {
                let d = match &self.editor.draw_state {
                    DrawState::Offsetting { distance, .. } => *distance,
                    _ => 0.0,
                };
                if d.abs() > 1.0 {
                    if let Some(obj) = self.scene.objects.get(obj_id.as_str()).cloned() {
                        if let Shape::Box { width, height, depth } = &obj.shape {
                            let p = obj.position;
                            let mat = obj.material;
                            // d > 0 = inset (shrink), d < 0 = outset (expand)
                            // For inset: position moves inward by d, dimensions shrink by 2*d
                            // For outset: position moves outward by |d|, dimensions grow by 2*|d|
                            let (new_pos, new_w, new_h, new_d) = match face {
                                PullFace::Top => (
                                    [p[0] + d, p[1] + height, p[2] + d],
                                    (*width - 2.0 * d).max(1.0), 0.1, (*depth - 2.0 * d).max(1.0),
                                ),
                                PullFace::Bottom => (
                                    [p[0] + d, p[1] - 0.1, p[2] + d],
                                    (*width - 2.0 * d).max(1.0), 0.1, (*depth - 2.0 * d).max(1.0),
                                ),
                                PullFace::Front => (
                                    [p[0] + d, p[1] + d, p[2]],
                                    (*width - 2.0 * d).max(1.0), (*height - 2.0 * d).max(1.0), 0.1,
                                ),
                                PullFace::Back => (
                                    [p[0] + d, p[1] + d, p[2] + depth],
                                    (*width - 2.0 * d).max(1.0), (*height - 2.0 * d).max(1.0), 0.1,
                                ),
                                PullFace::Right => (
                                    [p[0] + width, p[1] + d, p[2] + d],
                                    0.1, (*height - 2.0 * d).max(1.0), (*depth - 2.0 * d).max(1.0),
                                ),
                                PullFace::Left => (
                                    [p[0], p[1] + d, p[2] + d],
                                    0.1, (*height - 2.0 * d).max(1.0), (*depth - 2.0 * d).max(1.0),
                                ),
                            };
                            let label = if d > 0.0 { "內縮" } else { "外擴" };
                            let name = format!("{}_offset", obj.name);
                            let new_id = self.scene.add_box(name, new_pos, new_w, new_h, new_d, mat);
                            self.editor.selected_ids = vec![new_id.clone()];
                            self.editor.selected_face = Some((new_id, face));
                            self.editor.tool = Tool::PushPull;
                            self.file_message = Some((format!("偏移{} {:.0}mm — 可推拉", label, d.abs()), std::time::Instant::now()));
                        }
                    }
                }
                self.editor.draw_state = DrawState::Idle;
                self.editor.drag_snapshot_taken = false;
            }
        }

        // Push/Pull drag — only when a face is click-selected (selected_face)
        if self.editor.tool == Tool::PushPull {
            if let Some((ref obj_id, face)) = self.editor.selected_face.clone() {
                if response.dragged_by(egui::PointerButton::Primary) {
                    if !self.editor.drag_snapshot_taken {
                        self.scene.snapshot_ids(&[&obj_id], "推拉");
                        self.editor.last_pull_distance = 0.0; // reset accumulator at drag start
                        // C3: Save original position & dims for dashed reference lines
                        if let Some(obj) = self.scene.objects.get(&*obj_id) {
                            self.editor.pull_original_pos = Some(obj.position);
                            self.editor.pull_original_dims = match &obj.shape {
                                Shape::Box { width, height, depth } => Some([*width, *height, *depth]),
                                Shape::Cylinder { radius, height, .. } => Some([*radius * 2.0, *height, *radius * 2.0]),
                                _ => None,
                            };
                        }
                        self.editor.drag_snapshot_taken = true;
                    }
                    let d = response.drag_delta();
                    let scale = self.viewer.camera.distance * 0.0015;

                    // Get face normal in world space
                    let normal = match face {
                        PullFace::Top    => glam::Vec3::Y,
                        PullFace::Bottom => glam::Vec3::NEG_Y,
                        PullFace::Front  => glam::Vec3::NEG_Z,
                        PullFace::Back   => glam::Vec3::Z,
                        PullFace::Left   => glam::Vec3::NEG_X,
                        PullFace::Right  => glam::Vec3::X,
                    };

                    // Project face normal to screen space direction
                    // Get two points along the normal in world space, project both to screen
                    let obj_center = if let Some(o) = self.scene.objects.get(obj_id.as_str()) {
                        let p = o.position;
                        match &o.shape {
                            Shape::Box { width, height, depth } =>
                                glam::Vec3::new(p[0] + width / 2.0, p[1] + height / 2.0, p[2] + depth / 2.0),
                            _ => glam::Vec3::from(p),
                        }
                    } else { glam::Vec3::ZERO };

                    let vp = self.viewer.camera.view_proj(self.viewer.viewport_size[0] / self.viewer.viewport_size[1].max(1.0));

                    // Project center and center+normal to clip space
                    let p0_clip = vp * glam::Vec4::from((obj_center, 1.0));
                    let p1_clip = vp * glam::Vec4::from((obj_center + normal * 100.0, 1.0));

                    if p0_clip.w > 0.0 && p1_clip.w > 0.0 {
                        let p0_ndc = p0_clip.truncate() / p0_clip.w;
                        let p1_ndc = p1_clip.truncate() / p1_clip.w;

                        // Screen direction of the normal (flip Y because screen Y is down)
                        let screen_normal_x = p1_ndc.x - p0_ndc.x;
                        let screen_normal_y = -(p1_ndc.y - p0_ndc.y);
                        let len = (screen_normal_x * screen_normal_x + screen_normal_y * screen_normal_y).sqrt();

                        if len > 0.001 {
                            let sn_x = screen_normal_x / len;
                            let sn_y = screen_normal_y / len;

                            // Project drag delta onto screen normal direction
                            let amount = (d.x * sn_x + d.y * sn_y) * scale;

                            self.editor.last_pull_distance += amount;

                            if let Some(obj) = self.scene.objects.get_mut(obj_id.as_str()) {
                                match (&mut obj.shape, face) {
                                    (Shape::Box { height, .. }, PullFace::Top) =>
                                        *height = (*height + amount).max(10.0),
                                    (Shape::Box { height, .. }, PullFace::Bottom) => {
                                        let delta = amount.min(*height - 10.0);
                                        *height = (*height - delta).max(10.0);
                                        obj.position[1] += delta;
                                    }
                                    (Shape::Box { width, .. }, PullFace::Right) =>
                                        *width = (*width + amount).max(10.0),
                                    (Shape::Box { width, .. }, PullFace::Left) => {
                                        let delta = amount.min(*width - 10.0);
                                        *width = (*width - delta).max(10.0);
                                        obj.position[0] += delta;
                                    }
                                    (Shape::Box { depth, .. }, PullFace::Back) =>
                                        *depth = (*depth + amount).max(10.0),
                                    (Shape::Box { depth, .. }, PullFace::Front) => {
                                        let delta = amount.min(*depth - 10.0);
                                        *depth = (*depth - delta).max(10.0);
                                        obj.position[2] += delta;
                                    }
                                    (Shape::Cylinder { height, .. }, PullFace::Top) =>
                                        *height = (*height + amount).max(10.0),
                                    (Shape::Cylinder { height, .. }, PullFace::Bottom) => {
                                        let delta = amount.min(*height - 10.0);
                                        *height = (*height - delta).max(10.0);
                                        obj.position[1] += delta;
                                    }
                                    _ => {}
                                }
                            }
                            // Collision check after push/pull resize
                            if let Some(obj) = self.scene.objects.get(obj_id.as_str()) {
                                let (center, size) = crate::scene::obj_collision_center_size(obj);
                                let moving = crate::collision::Component::new(obj_id.clone(), obj.component_kind, center, size);
                                let components = crate::scene::scene_to_collision_components(&self.scene);
                                let report = crate::collision::can_place_component(&moving, &components, &crate::collision::CollisionConfig::default());
                                if !report.is_allowed || !report.warning_pairs.is_empty() {
                                    self.editor.collision_warning = Some("推拉導致碰撞".to_string());
                                }
                            }
                        }
                    }
                }
                if response.drag_stopped() {
                    // B5: Adjust adjacent objects that were touching the pulled face
                    {
                        let pull_delta = self.editor.last_pull_distance;
                        if pull_delta.abs() > 0.1 {
                            Self::adjust_adjacent_after_pull(&mut self.scene, &obj_id, face, pull_delta);
                        }
                    }
                    // 元件同步：推拉完成後同步所有同一元件的實例
                    self.scene.auto_sync_component(&obj_id);
                    // Store pull face for double-click repeat (A4)
                    // last_pull_distance was accumulated during drag
                    self.editor.last_pull_face = Some((obj_id.clone(), face));
                    self.editor.last_pull_click_time = std::time::Instant::now();
                    self.editor.drag_snapshot_taken = false;
                    // C3: clear original pos/dims
                    self.editor.pull_original_pos = None;
                    self.editor.pull_original_dims = None;
                    self.ai_log.log(&self.current_actor.clone(), "\u{63a8}\u{62c9}\u{9762}", &format!("{:?} {:.0}mm", face, self.editor.last_pull_distance), vec![obj_id.clone()]);
                    // Face stays selected after drag — user can pull again or click to deselect
                }
            }
        }

        // Push/Pull drag on free mesh face
        if let DrawState::PullingFreeMesh { face_id } = self.editor.draw_state {
            if response.dragged_by(egui::PointerButton::Primary) {
                if !self.editor.drag_snapshot_taken {
                    self.scene.snapshot();
                    self.editor.drag_snapshot_taken = true;
                }
                let drag = response.drag_delta();
                let scale = self.viewer.camera.distance * 0.002;
                // Use vertical drag mapped to face normal direction
                let amount = -drag.y * scale;
                if amount.abs() > 0.1 {
                    self.scene.free_mesh.push_pull_face(face_id, amount);
                    self.scene.version += 1;
                }
            }
            if response.drag_stopped() || response.clicked() {
                self.editor.draw_state = DrawState::Idle;
                self.editor.drag_snapshot_taken = false;
                self.file_message = Some((
                    "\u{9762}\u{5df2}\u{63a8}\u{62c9}\u{5b8c}\u{6210}".to_string(),
                    std::time::Instant::now(),
                ));
            }
        }

        // Double-click: enter/exit group isolation mode
        if response.double_clicked() {
            let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
            let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
            if self.editor.editing_group_id.is_some() {
                // 已在群組內 → 雙擊退出
                self.editor.editing_group_id = None;
                self.editor.selected_ids.clear();
            } else if let Some(id) = self.pick(mx, my, vw, vh) {
                // 用 GroupDef 檢查是否為群組（取代字串比對）
                let is_group = self.scene.groups.contains_key(&id)
                    || self.scene.objects.get(&id)
                        .map(|o| o.name.contains("[群組]") || o.name.contains("[元件]"))
                        .unwrap_or(false);
                // 也檢查物件的 parent_id 是否指向一個群組
                let parent_group = self.scene.objects.get(&id)
                    .and_then(|o| o.parent_id.clone())
                    .filter(|pid| self.scene.groups.contains_key(pid));
                if is_group {
                    self.editor.editing_group_id = Some(id.clone());
                    self.editor.selected_ids = vec![id];
                } else if let Some(gid) = parent_group {
                    // 雙擊群組成員 → 進入該群組的隔離編輯
                    self.editor.editing_group_id = Some(gid);
                    self.editor.selected_ids = vec![id];
                }
            }
        }

        // Click
        if response.clicked() {
            self.on_click();
        }

        // Right-click context menu (擴充版：對齊/分佈/鏡射)
        response.context_menu(|ui| {
            let has_sel = !self.editor.selected_ids.is_empty();
            let (action, cmd) = crate::menu::draw_context_menu_ext(ui, has_sel);
            self.handle_menu_action(action);
            if let Some(cmd_name) = cmd {
                self.execute_command_by_name(&cmd_name);
            }
        });

        // Keyboard shortcuts (extracted to keyboard.rs)
        self.handle_keyboard(response, ui);
    }
}

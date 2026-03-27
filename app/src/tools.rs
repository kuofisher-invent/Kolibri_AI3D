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
                        crate::app::SnapType::AxisZ => self.editor.locked_axis = Some(2),
                        _ => {} // keep current lock if any
                    }
                }
            } else if !shift && !ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd) {
                // Release Shift-lock when Shift is released (unless Ctrl-locked)
                // Only clear if it was a Shift-lock (not a Ctrl-cycle lock)
                if self.editor.locked_axis.is_some() && !self.editor.ctrl_was_down {
                    self.editor.locked_axis = None;
                }
            }

            // Compute smart snap
            // For Line/Arc tools, try face snap first
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
                // Run snap for ALL tools (not just drawing) — use ground point or fallback
                let raw = self.editor.mouse_ground.unwrap_or([0.0, 0.0, 0.0]);
                let from = self.get_drawing_origin();
                let result = self.smart_snap(raw, from);
                if result.snap_type != crate::app::SnapType::None {
                    self.editor.mouse_ground = Some(result.position);
                }
                self.editor.snap_result = Some(result);
            }

            // Hover pick — highlight objects/faces for all interactive tools
            let interactive = matches!(self.editor.tool,
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
                        // Continue rubber band drag (don't break on hover)
                        if let Some(hp) = response.hover_pos() {
                            if let Some((_, ref mut end)) = self.editor.rubber_band {
                                *end = hp;
                            }
                        }
                    } else if self.editor.hovered_id.is_none() {
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

                        let scale = self.viewer.camera.distance * 0.001;
                        let (sy, cy) = self.viewer.camera.yaw.sin_cos();
                        let right = glam::Vec3::new(-sy, 0.0, cy);
                        let fwd = glam::Vec3::new(-cy, 0.0, -sy);
                        let raw_delta = right * (-d.x) * scale + fwd * (d.y * scale);
                        let vert_delta = -d.y * scale;

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
                            }
                        }
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
                                (min, max)
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
        if let DrawState::Scaling { ref obj_id, handle, original_dims: _ } = self.editor.draw_state.clone() {
            if response.dragged_by(egui::PointerButton::Primary) {
                if !self.editor.drag_snapshot_taken {
                    self.scene.snapshot_ids(&[&obj_id], "縮放");
                    self.editor.drag_snapshot_taken = true;
                }
                let dy = -response.drag_delta().y;
                let factor = 1.0 + dy * 0.005;
                if let Some(obj) = self.scene.objects.get_mut(obj_id.as_str()) {
                    match (&mut obj.shape, handle) {
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

        // D1: Rotate — live preview during step 3 (hover updates rotation)
        if let DrawState::RotateAngle { ref obj_ids, center, ref_angle, ref mut current_angle, ref original_rotations } = self.editor.draw_state.clone() {
            if let Some(pt) = self.editor.mouse_ground {
                let dx = pt[0] - center[0];
                let dz = pt[2] - center[2];
                let mouse_angle = dz.atan2(dx);
                let mut delta = mouse_angle - ref_angle;

                // Snap to 15° increments when within 3°
                let snap_angle = 15.0_f32.to_radians();
                let snapped = (delta / snap_angle).round() * snap_angle;
                if (delta - snapped).abs() < 3.0_f32.to_radians() {
                    delta = snapped;
                }

                // 即時套用旋轉到所有選取物件
                for (i, id) in obj_ids.iter().enumerate() {
                    let orig = original_rotations.get(i).copied().unwrap_or(0.0);
                    if let Some(obj) = self.scene.objects.get_mut(id) {
                        obj.rotation_y = orig + delta;
                    }
                }
                self.editor.draw_state = DrawState::RotateAngle {
                    obj_ids: obj_ids.to_vec(), center, ref_angle, current_angle: mouse_angle, original_rotations: original_rotations.to_vec(),
                };
            }
        }

        // Offset drag — face edge inset with live preview
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
                let new_d = (cur_d + delta).abs().max(10.0);
                self.editor.draw_state = DrawState::Offsetting { obj_id: obj_id.clone(), face, distance: new_d };
            }
            if response.drag_stopped() {
                let d = match &self.editor.draw_state {
                    DrawState::Offsetting { distance, .. } => *distance,
                    _ => 0.0,
                };
                if d > 10.0 {
                    if let Some(obj) = self.scene.objects.get(obj_id.as_str()).cloned() {
                        if let Shape::Box { width, height, depth } = &obj.shape {
                            let p = obj.position;
                            let mat = obj.material;
                            let (new_pos, new_w, new_h, new_d) = match face {
                                PullFace::Top => (
                                    [p[0] + d, p[1] + height, p[2] + d],
                                    (*width - 2.0 * d).max(10.0), 0.1, (*depth - 2.0 * d).max(10.0),
                                ),
                                PullFace::Bottom => (
                                    [p[0] + d, p[1] - 0.1, p[2] + d],
                                    (*width - 2.0 * d).max(10.0), 0.1, (*depth - 2.0 * d).max(10.0),
                                ),
                                PullFace::Front => (
                                    [p[0] + d, p[1] + d, p[2]],
                                    (*width - 2.0 * d).max(10.0), (*height - 2.0 * d).max(10.0), 0.1,
                                ),
                                PullFace::Back => (
                                    [p[0] + d, p[1] + d, p[2] + depth],
                                    (*width - 2.0 * d).max(10.0), (*height - 2.0 * d).max(10.0), 0.1,
                                ),
                                PullFace::Right => (
                                    [p[0] + width, p[1] + d, p[2] + d],
                                    0.1, (*height - 2.0 * d).max(10.0), (*depth - 2.0 * d).max(10.0),
                                ),
                                PullFace::Left => (
                                    [p[0], p[1] + d, p[2] + d],
                                    0.1, (*height - 2.0 * d).max(10.0), (*depth - 2.0 * d).max(10.0),
                                ),
                            };
                            let name = format!("{}_offset", obj.name);
                            let new_id = self.scene.add_box(name, new_pos, new_w, new_h, new_d, mat);
                            self.editor.selected_ids = vec![new_id.clone()];
                            self.editor.selected_face = Some((new_id, face));
                            self.editor.tool = Tool::PushPull;
                            self.file_message = Some((format!("偏移 {:.0}mm — 可推拉", d), std::time::Instant::now()));
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

        // Keyboard
        if response.has_focus() || response.hovered() {
            ui.input(|i| {
                if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                    let ids = std::mem::take(&mut self.editor.selected_ids);
                    for id in &ids {
                        self.ai_log.log(&self.current_actor.clone(), "\u{522a}\u{9664}\u{7269}\u{4ef6}", id, vec![id.clone()]);
                        self.scene.delete(id);
                    }
                }
                if i.key_pressed(egui::Key::Escape) {
                    // RotateAngle: ESC cancels — restore original rotations
                    if let DrawState::RotateAngle { ref obj_ids, ref original_rotations, .. } = self.editor.draw_state.clone() {
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
                    } else {
                        self.editor.tool = Tool::Select;
                        self.editor.draw_state = DrawState::Idle;
                        self.editor.selected_ids.clear();
                        self.editor.selected_face = None;
                        self.editor.locked_axis = None;
                        self.editor.sticky_axis = None;
                        self.editor.editing_group_id = None;
                        self.editor.suggestion = None;
                        // Inference 2.0: reset context on ESC
                        crate::inference::reset_context(&mut self.editor.inference_ctx);
                        self.editor.inference_ctx.current_tool = Tool::Select;
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
                    let offset = [500.0_f32, 0.0, 500.0]; // 偏移貼上
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
                        format!("已貼上 {} 個物件", self.editor.clipboard.len()),
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
                        format!("已複製 {} 個物件", new_ids.len()),
                        std::time::Instant::now(),
                    ));
                }
                // Invert Selection (Ctrl+I)
                if ctrl && i.key_pressed(egui::Key::I) {
                    let all: std::collections::HashSet<String> = self.scene.objects.keys().cloned().collect();
                    let sel: std::collections::HashSet<String> = self.editor.selected_ids.iter().cloned().collect();
                    self.editor.selected_ids = all.difference(&sel).cloned().collect();
                }

                // Mirror X (Ctrl+M) — 鏡射選取物件沿 X 軸
                if ctrl && i.key_pressed(egui::Key::M) && !self.editor.selected_ids.is_empty() {
                    self.scene.snapshot();
                    // 計算選取物件的中心 X
                    let selected_objs: Vec<_> = self.editor.selected_ids.iter()
                        .filter_map(|id| self.scene.objects.get(id).cloned())
                        .collect();
                    let cx = if !selected_objs.is_empty() {
                        let sum: f32 = selected_objs.iter().map(|o| {
                            let w = match &o.shape {
                                Shape::Box { width, .. } => *width,
                                Shape::Cylinder { radius, .. } => *radius * 2.0,
                                Shape::Sphere { radius, .. } => *radius * 2.0,
                                _ => 0.0,
                            };
                            o.position[0] + w / 2.0
                        }).sum();
                        sum / selected_objs.len() as f32
                    } else { 0.0 };

                    let mut new_ids = Vec::new();
                    for obj in &selected_objs {
                        let mut clone = obj.clone();
                        clone.id = self.scene.next_id_pub();
                        clone.name = format!("{}_mirror", clone.name);
                        // 鏡射 X：新位置 = center - (original - center)
                        let obj_cx = clone.position[0] + match &clone.shape {
                            Shape::Box { width, .. } => *width / 2.0,
                            Shape::Cylinder { radius, .. } => *radius,
                            Shape::Sphere { radius, .. } => *radius,
                            _ => 0.0,
                        };
                        let mirrored_cx = 2.0 * cx - obj_cx;
                        let half_w = match &clone.shape {
                            Shape::Box { width, .. } => *width / 2.0,
                            Shape::Cylinder { radius, .. } => *radius,
                            Shape::Sphere { radius, .. } => *radius,
                            _ => 0.0,
                        };
                        clone.position[0] = mirrored_cx - half_w;
                        new_ids.push(clone.id.clone());
                        self.scene.objects.insert(clone.id.clone(), clone);
                    }
                    self.scene.version += 1;
                    self.editor.selected_ids.extend(new_ids);
                    self.file_message = Some((
                        format!("已鏡射 {} 個物件", selected_objs.len()),
                        std::time::Instant::now(),
                    ));
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
                            self.scene.snapshot_ids(&ids, "貼上屬性");
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
                        // Alt+Shift+H: 顯示全部
                        for obj in self.scene.objects.values_mut() { obj.visible = true; }
                        self.scene.version += 1;
                        self.file_message = Some(("全部顯示".into(), std::time::Instant::now()));
                    } else if !self.editor.selected_ids.is_empty() {
                        // Alt+H: 隱藏選取
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

                // 弧線工具啟用時，按 Ctrl 循環切換模式（兩點弧→三點弧→扇形）
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

                    // Standard view shortcuts
                    if i.key_pressed(egui::Key::Num1) { self.viewer.camera.set_front(); }
                    if i.key_pressed(egui::Key::Num2) { self.viewer.camera.set_top(); }
                    if i.key_pressed(egui::Key::Num3) { self.viewer.camera.set_iso(); }

                    // Axis locking
                    if i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowRight) {
                        self.editor.locked_axis = if self.editor.locked_axis == Some(0) { None } else { Some(0) };
                    }
                    if i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowDown) {
                        self.editor.locked_axis = if self.editor.locked_axis == Some(2) { None } else { Some(2) };
                    }
                }

                // Collect digit input for measurement
                if !matches!(self.editor.draw_state, DrawState::Idle) {
                    for ev in &i.events {
                        if let egui::Event::Text(t) = ev {
                            if t.chars().all(|c| c.is_ascii_digit() || c == ',' || c == '.' || c == 'x') {
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

    pub(crate) fn on_click(&mut self) {
        // ── Console: log every click with tool + position ──
        {
            let tool_name = format!("{:?}", self.editor.tool);
            let pos = self.editor.mouse_ground.map(|p| format!("[{:.0}, {:.0}, {:.0}]", p[0], p[1], p[2])).unwrap_or("(no ground)".into());
            let state = format!("{:?}", self.editor.draw_state).chars().take(40).collect::<String>();
            let hover = self.editor.hovered_id.as_deref().unwrap_or("none");
            self.console_push("CLICK", format!("{} @ {} | state={} | hover={}", tool_name, pos, state, hover));
        }

        match self.editor.tool {
            Tool::Select => {
                let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);

                match self.editor.selection_mode {
                    SelectionMode::Face => {
                        // 面選取模式：選取被點擊的面
                        if let Some((id, face)) = self.pick_face(mx, my, vw, vh) {
                            self.editor.selected_ids = vec![id.clone()];
                            self.editor.selected_face = Some((id.clone(), face));
                            self.clog(format!("選取面: {:?} on {}", face, id));
                        } else {
                            self.editor.selected_face = None;
                            self.editor.selected_ids.clear();
                        }
                    }
                    SelectionMode::Edge => {
                        // 邊選取模式：選取物件（用 hovered_face 的邊指示）
                        let picked = self.pick(mx, my, vw, vh);
                        if let Some(ref id) = picked {
                            self.editor.selected_ids = vec![id.clone()];
                            self.clog(format!("選取邊: {}", id));
                        } else {
                            self.editor.selected_ids.clear();
                        }
                    }
                    SelectionMode::Object => {
                        // 物件選取模式（原始行為）
                        let picked = self.pick(mx, my, vw, vh);
                        if self.editor.shift_held {
                            if let Some(id) = picked {
                                if let Some(pos) = self.editor.selected_ids.iter().position(|s| s == &id) {
                                    self.editor.selected_ids.remove(pos);
                                    self.clog(format!("取消選取: {}", id));
                                } else {
                                    self.editor.selected_ids.push(id.clone());
                                    let name = self.scene.objects.get(&id).map(|o| o.name.as_str()).unwrap_or("?");
                                    self.clog(format!("加選: {} ({})", name, id));
                                }
                            }
                        } else {
                            if let Some(ref id) = picked {
                                let name = self.scene.objects.get(id).map(|o| o.name.as_str()).unwrap_or("?");
                                self.clog(format!("選取: {} ({})", name, id));
                            }
                            self.editor.selected_ids = picked.into_iter().collect();
                        }
                        self.expand_selection_to_groups();
                    }
                }
                if !self.editor.selected_ids.is_empty() { self.right_tab = RightTab::Properties; }
            }

            Tool::CreateBox => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::BoxBase { p1: p };
                        }
                    }
                    DrawState::BoxBase { p1 } => {
                        if let Some(p2) = self.ground_snapped() {
                            let p1 = *p1;
                            self.editor.draw_state = DrawState::BoxHeight { p1, p2 };
                        }
                    }
                    DrawState::BoxHeight { p1, p2 } => {
                        let p1 = *p1;
                        let p2 = *p2;
                        let x0 = p1[0].min(p2[0]);
                        let z0 = p1[2].min(p2[2]);
                        let w = (p1[0]-p2[0]).abs().max(10.0);
                        let d = (p1[2]-p2[2]).abs().max(10.0);
                        let center = [(p1[0]+p2[0])*0.5, 0.0, (p1[2]+p2[2])*0.5];
                        let h = self.current_height(center).max(10.0);
                        let name = self.next_name("Box");
                        let id = self.scene.add_box(name.clone(), [x0, 0.0, z0], w, h, d, self.create_mat);
                        self.console_push("ACTION", format!("建立方塊 {} ({:.0}x{:.0}x{:.0}) mat={}", name, w, h, d, self.create_mat.label()));
                        // Collision check on creation (warning only)
                        {
                            let components = crate::scene::scene_to_collision_components(&self.scene);
                            let col_center = [x0 + w / 2.0, h / 2.0, z0 + d / 2.0];
                            let new_comp = crate::collision::Component::new(id.clone(), crate::collision::ComponentKind::Generic, col_center, [w, h, d]);
                            let report = crate::collision::can_place_component(&new_comp, &components, &crate::collision::CollisionConfig::default());
                            if !report.is_allowed || !report.warning_pairs.is_empty() {
                                self.editor.collision_warning = Some("放置位置與現有物件碰撞".to_string());
                            }
                        }
                        self.ai_log.log(&self.current_actor.clone(), "\u{5efa}\u{7acb}\u{65b9}\u{584a}", &format!("{:.0}\u{00d7}{:.0}\u{00d7}{:.0}", w, h, d), vec![id.clone()]);
                        self.editor.selected_ids = vec![id.clone()];
                        self.editor.draw_state = DrawState::Idle;
                        self.right_tab = RightTab::Properties;

                        // Check for alignment suggestion
                        self.check_alignment_suggestion(&id);
                    }
                    _ => {}
                }
            }

            Tool::CreateCylinder => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::CylBase { center: p };
                        }
                    }
                    DrawState::CylBase { center } => {
                        let c = *center;
                        if let Some(mouse) = self.ground_snapped() {
                            let r = ((mouse[0]-c[0]).powi(2)+(mouse[2]-c[2]).powi(2)).sqrt().max(10.0);
                            self.editor.draw_state = DrawState::CylHeight { center: c, radius: r };
                        }
                    }
                    DrawState::CylHeight { center, radius } => {
                        let c = *center;
                        let r = *radius;
                        let h = self.current_height(c).max(10.0);
                        let name = self.next_name("Cylinder");
                        let id = self.scene.add_cylinder(name.clone(), c, r, h, 48, self.create_mat);
                        self.console_push("ACTION", format!("建立圓柱 {} (r={:.0} h={:.0}) mat={}", name, r, h, self.create_mat.label()));
                        // Collision check on creation (warning only)
                        {
                            let components = crate::scene::scene_to_collision_components(&self.scene);
                            let col_center = [c[0] + r, c[1] + h / 2.0, c[2] + r];
                            let new_comp = crate::collision::Component::new(id.clone(), crate::collision::ComponentKind::Generic, col_center, [r * 2.0, h, r * 2.0]);
                            let report = crate::collision::can_place_component(&new_comp, &components, &crate::collision::CollisionConfig::default());
                            if !report.is_allowed || !report.warning_pairs.is_empty() {
                                self.editor.collision_warning = Some("放置位置與現有物件碰撞".to_string());
                            }
                        }
                        self.ai_log.log(&self.current_actor.clone(), "\u{5efa}\u{7acb}\u{5713}\u{67f1}", &format!("r={:.0} h={:.0}", r, h), vec![id.clone()]);
                        self.editor.selected_ids = vec![id];
                        self.editor.draw_state = DrawState::Idle;
                        self.right_tab = RightTab::Properties;
                    }
                    _ => {}
                }
            }

            Tool::CreateSphere => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::SphRadius { center: p };
                        }
                    }
                    DrawState::SphRadius { center } => {
                        let c = *center;
                        if let Some(mouse) = self.ground_snapped() {
                            let r = ((mouse[0]-c[0]).powi(2)+(mouse[2]-c[2]).powi(2)).sqrt().max(10.0);
                            let name = self.next_name("Sphere");
                            let id = self.scene.add_sphere(name.clone(), c, r, 32, self.create_mat);
                            self.console_push("ACTION", format!("建立球體 {} (r={:.0}) mat={}", name, r, self.create_mat.label()));
                            // Collision check on creation (warning only)
                            {
                                let components = crate::scene::scene_to_collision_components(&self.scene);
                                let col_center = [c[0] + r, c[1] + r, c[2] + r];
                                let new_comp = crate::collision::Component::new(id.clone(), crate::collision::ComponentKind::Generic, col_center, [r * 2.0, r * 2.0, r * 2.0]);
                                let report = crate::collision::can_place_component(&new_comp, &components, &crate::collision::CollisionConfig::default());
                                if !report.is_allowed || !report.warning_pairs.is_empty() {
                                    self.editor.collision_warning = Some("放置位置與現有物件碰撞".to_string());
                                }
                            }
                            self.ai_log.log(&self.current_actor.clone(), "\u{5efa}\u{7acb}\u{7403}\u{9ad4}", &format!("r={:.0}", r), vec![id.clone()]);
                            self.editor.selected_ids = vec![id];
                            self.editor.draw_state = DrawState::Idle;
                            self.right_tab = RightTab::Properties;
                        }
                    }
                    _ => {}
                }
            }

            // Rectangle = 2D flat rectangle only (use Push/Pull to extrude)
            Tool::Rectangle => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::BoxBase { p1: p };
                        }
                    }
                    DrawState::BoxBase { p1 } => {
                        if let Some(p2) = self.ground_snapped() {
                            let p1 = *p1;
                            let x0 = p1[0].min(p2[0]);
                            let z0 = p1[2].min(p2[2]);
                            let w = (p1[0] - p2[0]).abs().max(10.0);
                            let d = (p1[2] - p2[2]).abs().max(10.0);
                            let name = self.next_name("Rect");
                            // Create flat rectangle (1mm height) — use Push/Pull to extrude
                            let id = self.scene.add_box(name, [x0, 0.0, z0], w, 1.0, d, self.create_mat);
                            self.editor.selected_ids = vec![id];
                            self.editor.draw_state = DrawState::Idle;
                            self.right_tab = RightTab::Properties;
                            self.editor.tool = Tool::PushPull; // auto-switch to Push/Pull
                            self.file_message = Some(("矩形已建立 — 用推拉拉出高度".to_string(), std::time::Instant::now()));
                        }
                    }
                    _ => {}
                }
            }

            // Circle = same as CreateCylinder flow
            Tool::Circle => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::CylBase { center: p };
                        }
                    }
                    DrawState::CylBase { center } => {
                        let c = *center;
                        if let Some(mouse) = self.ground_snapped() {
                            let r = ((mouse[0]-c[0]).powi(2)+(mouse[2]-c[2]).powi(2)).sqrt().max(10.0);
                            self.editor.draw_state = DrawState::CylHeight { center: c, radius: r };
                        }
                    }
                    DrawState::CylHeight { center, radius } => {
                        let (c, r) = (*center, *radius);
                        let h = self.current_height(c).max(10.0);
                        let name = self.next_name("Circle");
                        let id = self.scene.add_cylinder(name, c, r, h, 48, self.create_mat);
                        self.editor.selected_ids = vec![id];
                        self.editor.draw_state = DrawState::Idle;
                        self.right_tab = RightTab::Properties;
                    }
                    _ => {}
                }
            }

            // Move click = select for moving (only when highlighted)
            Tool::Move => {
                if let Some(ref id) = self.editor.hovered_id.clone() {
                    self.editor.selected_ids = vec![id.clone()];
                    // Expand selection to include all group members
                    self.expand_selection_to_groups();
                    self.right_tab = RightTab::Properties;
                }
            }

            // Eraser = click to delete (only when highlighted)
            Tool::Eraser => {
                if let Some(ref id) = self.editor.hovered_id.clone() {
                    self.ai_log.log(&self.current_actor.clone(), "\u{522a}\u{9664}\u{7269}\u{4ef6}", id, vec![id.clone()]);
                    self.scene.delete(id);
                    self.editor.selected_ids.retain(|s| s != id);
                }
            }

            // Paint Bucket = apply material on click (hovered or picked)
            Tool::PaintBucket => {
                let target_id = self.editor.hovered_id.clone().or_else(|| {
                    let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                    let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                    self.pick(mx, my, vw, vh)
                });
                if let Some(ref id) = target_id {
                    // 單一物件上色
                    self.scene.snapshot_ids(&[id], "材質");
                    if let Some(obj) = self.scene.objects.get_mut(id) {
                        obj.material = self.create_mat;
                        self.scene.version += 1;
                    }
                    self.file_message = Some((format!("已套用材質: {}", self.create_mat.label()), std::time::Instant::now()));
                    self.editor.selected_ids.clear();
                } else if !self.editor.selected_ids.is_empty() {
                    // 批量上色：所有選取物件
                    let ids: Vec<&str> = self.editor.selected_ids.iter().map(|s| s.as_str()).collect();
                    self.scene.snapshot_ids(&ids, "批量材質");
                    let count = self.editor.selected_ids.len();
                    for id in &self.editor.selected_ids.clone() {
                        if let Some(obj) = self.scene.objects.get_mut(id) {
                            obj.material = self.create_mat;
                        }
                    }
                    self.scene.version += 1;
                    self.file_message = Some((
                        format!("已批量套用 {} 到 {} 個物件", self.create_mat.label(), count),
                        std::time::Instant::now(),
                    ));
                }
            }

            // TapeMeasure = snap-aware point-to-point measurement (like SketchUp)
            Tool::TapeMeasure => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        // Always use snap position first (endpoint/midpoint/edge/face)
                        let p = if let Some(ref snap) = self.editor.snap_result {
                            if snap.snap_type != crate::app::SnapType::None {
                                snap.position
                            } else if let Some(g) = self.ground_snapped() {
                                g
                            } else { return; }
                        } else if let Some(g) = self.ground_snapped() {
                            g
                        } else { return; };

                        // Show what we snapped to
                        if let Some(ref snap) = self.editor.snap_result {
                            if snap.snap_type != crate::app::SnapType::None && snap.snap_type != crate::app::SnapType::Grid {
                                self.file_message = Some((
                                    format!("量測起點: {} [{:.0}, {:.0}, {:.0}]", snap.snap_type.label(), p[0], p[1], p[2]),
                                    std::time::Instant::now()
                                ));
                            }
                        }
                        self.editor.draw_state = DrawState::Measuring { start: p };
                    }
                    DrawState::Measuring { start } => {
                        let s = *start;
                        let p = if let Some(ref snap) = self.editor.snap_result {
                            if snap.snap_type != crate::app::SnapType::None {
                                snap.position
                            } else if let Some(g) = self.ground_snapped() {
                                g
                            } else { return; }
                        } else if let Some(g) = self.ground_snapped() {
                            g
                        } else { return; };

                        // Compute and display distance
                        let dx = p[0] - s[0];
                        let dy = p[1] - s[1];
                        let dz = p[2] - s[2];
                        let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                        let dist_text = if dist >= 1000.0 {
                            format!("{:.2} m", dist / 1000.0)
                        } else {
                            format!("{:.0} mm", dist)
                        };
                        self.file_message = Some((
                            format!("距離: {} | ΔX={:.0} ΔY={:.0} ΔZ={:.0}", dist_text, dx.abs(), dy.abs(), dz.abs()),
                            std::time::Instant::now()
                        ));

                        self.dimensions.push(crate::dimensions::Dimension::new(s, p));
                        self.editor.draw_state = DrawState::Idle;
                    }
                    _ => {}
                }
            }

            // Dimension = persistent two-point annotation (same measuring flow, no object info)
            Tool::Dimension => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        let p = if let Some(ref snap) = self.editor.snap_result {
                            if snap.snap_type != crate::app::SnapType::None {
                                snap.position
                            } else if let Some(g) = self.ground_snapped() {
                                g
                            } else { return; }
                        } else if let Some(g) = self.ground_snapped() {
                            g
                        } else { return; };

                        self.file_message = Some((
                            format!("標註起點: [{:.0}, {:.0}, {:.0}]", p[0], p[1], p[2]),
                            std::time::Instant::now()
                        ));
                        self.editor.draw_state = DrawState::Measuring { start: p };
                    }
                    DrawState::Measuring { start } => {
                        let s = *start;
                        let p = if let Some(ref snap) = self.editor.snap_result {
                            if snap.snap_type != crate::app::SnapType::None {
                                snap.position
                            } else if let Some(g) = self.ground_snapped() {
                                g
                            } else { return; }
                        } else if let Some(g) = self.ground_snapped() {
                            g
                        } else { return; };

                        let dx = p[0] - s[0];
                        let dy = p[1] - s[1];
                        let dz = p[2] - s[2];
                        let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                        let dist_text = if dist >= 1000.0 {
                            format!("{:.2} m", dist / 1000.0)
                        } else {
                            format!("{:.0} mm", dist)
                        };
                        self.file_message = Some((
                            format!("標註: {}", dist_text),
                            std::time::Instant::now()
                        ));
                        self.dimensions.push(crate::dimensions::Dimension::new(s, p));
                        self.editor.draw_state = DrawState::Idle;
                    }
                    _ => {}
                }
            }

            // Text = click to place a text label
            Tool::Text => {
                let p = if let Some(ref snap) = self.editor.snap_result {
                    if snap.snap_type != crate::app::SnapType::None {
                        snap.position
                    } else if let Some(g) = self.ground_snapped() {
                        g
                    } else { return; }
                } else if let Some(g) = self.ground_snapped() {
                    g
                } else { return; };

                // Create a small box named "Text" as a label marker
                let name = format!("Text_{}", self.scene.objects.len() + 1);
                self.scene.snapshot();
                let mat = crate::scene::MaterialKind::White;
                self.scene.add_box(name, p, 50.0, 10.0, 50.0, mat);
                self.file_message = Some((
                    format!("文字標籤已放置 @ [{:.0}, {:.0}, {:.0}]", p[0], p[1], p[2]),
                    std::time::Instant::now()
                ));
            }

            // Camera tools: click does nothing (drag handled above)
            Tool::Orbit | Tool::Pan | Tool::ZoomExtents => {}

            // Rotate: 3-step SU-style protractor (D1)
            // Step 1: click to place center (on ground or snap point)
            // Step 2: click to set reference direction (0° line)
            // Step 3: click to set target angle (or type degrees + Enter)
            Tool::Rotate => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        // Step 1: place rotation center
                        // 需要先有選取物件
                        let ids = if self.editor.selected_ids.is_empty() {
                            let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                            let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                            if let Some(id) = self.pick(mx, my, vw, vh) {
                                vec![id]
                            } else {
                                vec![]
                            }
                        } else {
                            self.editor.selected_ids.clone()
                        };
                        if !ids.is_empty() {
                            self.editor.selected_ids = ids.clone();
                            if let Some(pt) = self.editor.mouse_ground {
                                self.editor.draw_state = DrawState::RotateRef {
                                    obj_ids: ids,
                                    center: pt,
                                };
                            }
                        }
                    }
                    DrawState::RotateRef { obj_ids, center } => {
                        // Step 2: set reference direction
                        let obj_ids = obj_ids.clone();
                        let center = *center;
                        if let Some(pt) = self.editor.mouse_ground {
                            let dx = pt[0] - center[0];
                            let dz = pt[2] - center[2];
                            let ref_angle = dz.atan2(dx);
                            // 記錄所有物件的原始旋轉角
                            let original_rotations: Vec<f32> = obj_ids.iter().map(|id| {
                                self.scene.objects.get(id).map_or(0.0, |o| o.rotation_y)
                            }).collect();
                            let ids: Vec<&str> = obj_ids.iter().map(|s| s.as_str()).collect();
                            self.scene.snapshot_ids(&ids, "旋轉");
                            self.editor.draw_state = DrawState::RotateAngle {
                                obj_ids,
                                center,
                                ref_angle,
                                current_angle: ref_angle,
                                original_rotations,
                            };
                        }
                    }
                    DrawState::RotateAngle { obj_ids, center, ref_angle, current_angle, original_rotations } => {
                        // Step 3: confirm target angle
                        let delta = *current_angle - *ref_angle;
                        // 已經在 hover 時即時套用，直接結束
                        let _ = (obj_ids, center, delta, original_rotations);
                        self.editor.draw_state = DrawState::Idle;
                        self.editor.drag_snapshot_taken = false;
                    }
                    _ => {}
                }
            }

            // Scale: click to select, determine handle from face
            Tool::Scale => {
                let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                if let Some(id) = self.pick(mx, my, vw, vh) {
                    self.editor.selected_ids = vec![id.clone()];

                    // Determine handle type from click position on bounding box
                    let handle = if let Some((_, face)) = self.pick_face(mx, my, vw, vh) {
                        match face {
                            PullFace::Left | PullFace::Right => ScaleHandle::AxisX,
                            PullFace::Top | PullFace::Bottom => ScaleHandle::AxisY,
                            PullFace::Front | PullFace::Back => ScaleHandle::AxisZ,
                        }
                    } else {
                        ScaleHandle::Uniform
                    };

                    // Store original dimensions
                    let orig = if let Some(obj) = self.scene.objects.get(&id) {
                        match &obj.shape {
                            Shape::Box { width, height, depth } => [*width, *height, *depth],
                            Shape::Cylinder { radius, height, .. } => [*radius * 2.0, *height, *radius * 2.0],
                            Shape::Sphere { radius, .. } => [*radius * 2.0, *radius * 2.0, *radius * 2.0],
                            _ => [1000.0; 3],
                        }
                    } else { [1000.0; 3] };

                    self.editor.draw_state = DrawState::Scaling { obj_id: id, handle, original_dims: orig };
                }
            }

            // Line: click-click chain drawing → adds edges to the shared free mesh
            Tool::Line => {
                let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        // Try face snap first, then fall back to ground
                        let pos = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(p) = pos {
                            self.editor.draw_state = DrawState::LineFrom { p1: p };
                        }
                    }
                    DrawState::LineFrom { p1 } => {
                        let p1 = *p1;
                        let p2_opt = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(p2) = p2_opt {
                            self.scene.snapshot();

                            // Add to free mesh topology
                            let tol = 50.0; // 50mm snap tolerance
                            let v1 = self.scene.free_mesh.find_vertex_near(p1, tol)
                                .unwrap_or_else(|| self.scene.free_mesh.add_vertex(p1));
                            let v2 = self.scene.free_mesh.find_vertex_near(p2, tol)
                                .unwrap_or_else(|| self.scene.free_mesh.add_vertex(p2));

                            self.scene.free_mesh.add_edge_between(v1, v2);

                            // Try to detect new faces from closed loops
                            let face_count_before = self.scene.free_mesh.faces.len();
                            self.scene.free_mesh.detect_faces();
                            let new_faces = self.scene.free_mesh.faces.len() - face_count_before;

                            if new_faces > 0 {
                                self.file_message = Some((
                                    format!("\u{2713} \u{5075}\u{6e2c}\u{5230} {} \u{500b}\u{65b0}\u{9762}\u{ff01}\u{53ef}\u{7528}\u{63a8}\u{62c9}\u{5de5}\u{5177}\u{62c9}\u{4f38}", new_faces),
                                    std::time::Instant::now(),
                                ));
                            }

                            self.scene.version += 1;

                            // Store last drawn edge direction for perpendicular/parallel inference
                            let edge_dir = [p2[0] - p1[0], p2[2] - p1[2]];
                            let edge_len = (edge_dir[0] * edge_dir[0] + edge_dir[1] * edge_dir[1]).sqrt();
                            if edge_len > 1.0 {
                                self.editor.last_line_dir = Some([edge_dir[0] / edge_len, edge_dir[1] / edge_len]);
                            }

                            // Inference 2.0: update context after line drawn
                            crate::inference::update_context_after_line(&mut self.editor.inference_ctx, p1, p2);

                            // Try to split a box face if line lies on it
                            self.try_split_face(p1, p2);

                            // Also try crossing-based split (line crosses box footprint)
                            self.try_split_face_on_line(p1, p2);

                            // Chain: start new line from p2
                            self.editor.draw_state = DrawState::LineFrom { p1: p2 };
                        }
                    }
                    _ => {}
                }
            }

            // Arc: 3-click (start, end, bulge)
            Tool::Arc => {
                let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        let pos = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(p) = pos {
                            self.editor.draw_state = DrawState::ArcP1 { p1: p };
                            self.clog(format!("圓弧: 起點 [{:.0}, {:.0}, {:.0}]", p[0], p[1], p[2]));
                        }
                    }
                    DrawState::ArcP1 { p1 } => {
                        let p1 = *p1;
                        let pos = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(p2) = pos {
                            let chord = ((p2[0]-p1[0]).powi(2) + (p2[2]-p1[2]).powi(2)).sqrt();
                            self.clog(format!("圓弧: 終點 [{:.0}, {:.0}, {:.0}] 弦長={:.0}mm", p2[0], p2[1], p2[2], chord));
                            self.editor.draw_state = DrawState::ArcP2 { p1, p2 };
                        }
                    }
                    DrawState::ArcP2 { p1, p2 } => {
                        let (p1, p2) = (*p1, *p2);
                        let pos = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(mut p3) = pos {
                            // 半圓鎖定：凸度點接近弦長中垂線上的半圓位置時自動吸附
                            if let Some(ref info) = crate::app::compute_arc_info(p1, p2, p3) {
                                if info.is_semicircle() {
                                    // 強制半圓：調整 p3 到精確半圓位置
                                    let mid = [(p1[0]+p2[0])*0.5, (p1[1]+p2[1])*0.5, (p1[2]+p2[2])*0.5];
                                    let chord_dir = [p2[0]-p1[0], p2[1]-p1[1], p2[2]-p1[2]];
                                    let chord_len = (chord_dir[0]*chord_dir[0] + chord_dir[2]*chord_dir[2]).sqrt();
                                    let perp = [-chord_dir[2] / chord_len, 0.0, chord_dir[0] / chord_len];
                                    let r = chord_len * 0.5;
                                    let sign = if (p3[0]-mid[0])*perp[0] + (p3[2]-mid[2])*perp[2] > 0.0 { 1.0 } else { -1.0 };
                                    p3 = [mid[0] + perp[0]*r*sign, mid[1], mid[2] + perp[2]*r*sign];
                                    self.clog("圓弧: 半圓鎖定!".to_string());
                                }
                            }

                            let seg = 32_usize; // TODO: 可調細分數
                            if let Some(info) = crate::app::compute_arc_info(p1, p2, p3) {
                                let arc_pts = info.points(seg);
                                let name = self.next_name("Arc");
                                let id = self.scene.add_line(name.clone(), arc_pts, 20.0, self.create_mat);
                                // 儲存圓弧資訊到 Shape::Line
                                if let Some(obj) = self.scene.objects.get_mut(&id) {
                                    if let crate::scene::Shape::Line { ref mut arc_center, ref mut arc_radius, ref mut arc_angle_deg, .. } = obj.shape {
                                        *arc_center = Some(info.center);
                                        *arc_radius = Some(info.radius);
                                        *arc_angle_deg = Some(info.sweep_degrees());
                                    }
                                }
                                self.console_push("ACTION", format!(
                                    "建立圓弧 {} R={:.0}mm 角度={:.1}° 弧長={:.0}mm",
                                    name, info.radius, info.sweep_degrees(), info.arc_length()
                                ));
                                self.editor.selected_ids = vec![id];
                            } else {
                                // 退回直線
                                let name = self.next_name("Line");
                                let id = self.scene.add_line(name, vec![p1, p2], 20.0, self.create_mat);
                                self.editor.selected_ids = vec![id];
                                self.console_push("WARN", "圓弧: 三點共線，建立直線".to_string());
                            }
                            self.editor.draw_state = DrawState::Idle;
                        }
                    }
                    _ => {}
                }
            }

            // Arc3Point: 任意三點定圓弧（等同 Arc 但語意更清楚）
            Tool::Arc3Point => {
                let pos = self.snap_to_face(self.editor.mouse_screen[0], self.editor.mouse_screen[1],
                    self.viewer.viewport_size[0], self.viewer.viewport_size[1])
                    .or_else(|| self.ground_snapped());
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = pos {
                            self.editor.draw_state = DrawState::ArcP1 { p1: p };
                            self.clog(format!("三點圓弧: P1 [{:.0}, {:.0}, {:.0}]", p[0], p[1], p[2]));
                        }
                    }
                    DrawState::ArcP1 { p1 } => {
                        let p1 = *p1;
                        if let Some(p2) = pos {
                            self.editor.draw_state = DrawState::ArcP2 { p1, p2 };
                            self.clog(format!("三點圓弧: P2 [{:.0}, {:.0}, {:.0}]", p2[0], p2[1], p2[2]));
                        }
                    }
                    DrawState::ArcP2 { p1, p2 } => {
                        let (p1, p2) = (*p1, *p2);
                        if let Some(p3) = pos {
                            if let Some(info) = crate::app::compute_arc_info(p1, p2, p3) {
                                let arc_pts = info.points(32);
                                let name = self.next_name("Arc3P");
                                let id = self.scene.add_line(name.clone(), arc_pts, 20.0, self.create_mat);
                                if let Some(obj) = self.scene.objects.get_mut(&id) {
                                    if let crate::scene::Shape::Line { ref mut arc_center, ref mut arc_radius, ref mut arc_angle_deg, .. } = obj.shape {
                                        *arc_center = Some(info.center);
                                        *arc_radius = Some(info.radius);
                                        *arc_angle_deg = Some(info.sweep_degrees());
                                    }
                                }
                                self.console_push("ACTION", format!("建立三點圓弧 {} R={:.0} {:.1}°", name, info.radius, info.sweep_degrees()));
                                self.editor.selected_ids = vec![id];
                            }
                            self.editor.draw_state = DrawState::Idle;
                        }
                    }
                    _ => {}
                }
            }

            // Pie: 扇形工具 — 中心→邊緣定半徑→第二邊緣定角度
            Tool::Pie => {
                let pos = self.ground_snapped();
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = pos {
                            self.editor.draw_state = DrawState::PieCenter { center: p };
                            self.clog(format!("扇形: 中心 [{:.0}, {:.0}, {:.0}]", p[0], p[1], p[2]));
                        }
                    }
                    DrawState::PieCenter { center } => {
                        let c = *center;
                        if let Some(e1) = pos {
                            let r = ((e1[0]-c[0]).powi(2) + (e1[2]-c[2]).powi(2)).sqrt();
                            if r > 10.0 {
                                self.editor.draw_state = DrawState::PieRadius { center: c, edge1: e1 };
                                self.clog(format!("扇形: 半徑={:.0}mm", r));
                            }
                        }
                    }
                    DrawState::PieRadius { center, edge1 } => {
                        let c = *center;
                        let e1 = *edge1;
                        if let Some(e2) = pos {
                            let r = ((e1[0]-c[0]).powi(2) + (e1[2]-c[2]).powi(2)).sqrt();
                            let a1 = (e1[2]-c[2]).atan2(e1[0]-c[0]);
                            let a2 = (e2[2]-c[2]).atan2(e2[0]-c[0]);
                            let mut sweep = a2 - a1;
                            if sweep < 0.0 { sweep += std::f32::consts::TAU; }

                            // 生成扇形邊緣弧線 + 兩條半徑線
                            let seg = 32;
                            let mut pts = vec![c]; // 中心
                            for i in 0..=seg {
                                let t = i as f32 / seg as f32;
                                let a = a1 + sweep * t;
                                pts.push([c[0] + r * a.cos(), c[1], c[2] + r * a.sin()]);
                            }
                            pts.push(c); // 閉合回中心

                            let name = self.next_name("Pie");
                            let id = self.scene.add_line(name.clone(), pts, 20.0, self.create_mat);
                            if let Some(obj) = self.scene.objects.get_mut(&id) {
                                if let crate::scene::Shape::Line { ref mut arc_center, ref mut arc_radius, ref mut arc_angle_deg, .. } = obj.shape {
                                    *arc_center = Some(c);
                                    *arc_radius = Some(r);
                                    *arc_angle_deg = Some(sweep.to_degrees());
                                }
                            }
                            self.console_push("ACTION", format!("建立扇形 {} R={:.0} {:.1}°", name, r, sweep.to_degrees()));
                            self.editor.selected_ids = vec![id];
                            self.editor.draw_state = DrawState::Idle;
                        }
                    }
                    _ => {}
                }
            }

            // Offset: face edge inset — click a box face to enter drag-to-inset mode
            Tool::Offset => {
                let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);

                if let Some((id, face)) = self.pick_face(mx, my, vw, vh) {
                    if let Some(obj) = self.scene.objects.get(&id) {
                        match &obj.shape {
                            Shape::Box { .. } => {
                                self.editor.selected_ids = vec![id.clone()];
                                self.editor.draw_state = DrawState::Offsetting {
                                    obj_id: id,
                                    face,
                                    distance: 0.0,
                                };
                            }
                            _ => {
                                self.file_message = Some(("偏移目前僅支援方塊面".to_string(), std::time::Instant::now()));
                            }
                        }
                    }
                } else {
                    self.file_message = Some(("請點擊方塊的面來偏移".to_string(), std::time::Instant::now()));
                }
            }

            // Group: tag selected objects as group (add _group suffix)
            Tool::Group => {
                let ids = self.editor.selected_ids.clone();
                for id in &ids {
                    let needs_tag = self.scene.objects.get(id)
                        .map(|o| !o.name.contains("[群組]"))
                        .unwrap_or(false);
                    if needs_tag {
                        self.scene.snapshot();
                        if let Some(obj) = self.scene.objects.get_mut(id) {
                            obj.name = format!("[群組] {}", obj.name);
                        }
                    }
                }
            }

            // Component: tag selected object as component (reusable)
            Tool::Component => {
                if let Some(ref id) = self.editor.selected_ids.first().cloned() {
                    let needs_tag = self.scene.objects.get(id)
                        .map(|o| !o.name.contains("[元件]"))
                        .unwrap_or(false);
                    if needs_tag {
                        self.scene.snapshot();
                        if let Some(obj) = self.scene.objects.get_mut(id) {
                            obj.name = format!("[元件] {}", obj.name);
                        }
                    }
                }
            }

            // FollowMe: path extrusion — select profile, then click points to define path
            Tool::FollowMe => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if !self.editor.selected_ids.is_empty() {
                            // Profile already selected, start path with first click
                            if let Some(p) = self.ground_snapped() {
                                self.editor.draw_state = DrawState::FollowPath {
                                    source_id: self.editor.selected_ids[0].clone(),
                                    path_points: vec![p],
                                };
                                self.file_message = Some(("路徑第一點已設定 — 繼續點擊加入路徑點, ESC 完成".to_string(), std::time::Instant::now()));
                            }
                        } else {
                            // No selection: pick an object first
                            let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                            let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                            if let Some(id) = self.pick(mx, my, vw, vh) {
                                self.editor.selected_ids = vec![id];
                                self.file_message = Some(("已選取截面 — 點擊地面設定路徑起點".to_string(), std::time::Instant::now()));
                            } else {
                                self.file_message = Some(("請先選取要沿路徑擠出的物件".to_string(), std::time::Instant::now()));
                            }
                        }
                    }
                    DrawState::FollowPath { source_id, path_points } => {
                        let src = source_id.clone();
                        let mut pts = path_points.clone();
                        if let Some(p) = self.ground_snapped() {
                            pts.push(p);

                            // If this point is very close to the first point (closed path), finish
                            if pts.len() >= 3 {
                                let first = pts[0];
                                let last = pts.last().unwrap();
                                let dist = ((last[0] - first[0]).powi(2) + (last[2] - first[2]).powi(2)).sqrt();
                                if dist < 200.0 {
                                    self.extrude_along_path(&src, &pts);
                                    self.editor.draw_state = DrawState::Idle;
                                    return;
                                }
                            }

                            self.editor.draw_state = DrawState::FollowPath { source_id: src, path_points: pts.clone() };
                            self.file_message = Some((
                                format!("路徑 {} 點 — 繼續點擊或按 Enter/ESC 完成擠出", pts.len()),
                                std::time::Instant::now(),
                            ));
                        }
                    }
                    _ => {}
                }
            }

            // ── Architecture Tools ──
            Tool::Wall => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::WallFrom { p1: p };
                        }
                    }
                    DrawState::WallFrom { p1 } => {
                        let p1 = *p1;
                        if let Some(p2) = self.ground_snapped() {
                            // 計算牆的方向和法線
                            let dx = p2[0] - p1[0];
                            let dz = p2[2] - p1[2];
                            let len = (dx * dx + dz * dz).sqrt();
                            if len > 10.0 {
                                let t = self.editor.wall_thickness;
                                let h = self.editor.wall_height;
                                // 法線方向（垂直於牆線段，在 XZ 平面上）
                                let nx = -dz / len * (t / 2.0);
                                let nz = dx / len * (t / 2.0);

                                self.scene.snapshot();
                                let name = self.next_name("Wall");
                                // 牆體：以 Box 表示，位置和尺寸根據兩端點計算
                                let min_x = p1[0].min(p2[0]) - nx.abs();
                                let min_z = p1[2].min(p2[2]) - nz.abs();
                                let w = (p2[0] - p1[0]).abs() + t;
                                let d = (p2[2] - p1[2]).abs() + t;
                                self.scene.add_box(name.clone(), [min_x, 0.0, min_z], w, h, d, MaterialKind::Concrete);

                                self.file_message = Some((
                                    format!("牆 {} — {:.0}mm × {:.0}mm × {:.0}mm", name, len, t, h),
                                    std::time::Instant::now(),
                                ));
                                // 繼續畫下一面牆（連續）
                                self.editor.draw_state = DrawState::WallFrom { p1: p2 };
                            }
                        }
                    }
                    _ => {}
                }
            }
            Tool::Slab => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::SlabCorner { p1: p };
                        }
                    }
                    DrawState::SlabCorner { p1 } => {
                        let p1 = *p1;
                        if let Some(p2) = self.ground_snapped() {
                            let w = (p2[0] - p1[0]).abs().max(10.0);
                            let d = (p2[2] - p1[2]).abs().max(10.0);
                            let t = self.editor.slab_thickness;
                            let min_x = p1[0].min(p2[0]);
                            let min_z = p1[2].min(p2[2]);
                            let y = p1[1]; // 板底標高

                            self.scene.snapshot();
                            let name = self.next_name("Slab");
                            self.scene.add_box(name.clone(), [min_x, y, min_z], w, t, d, MaterialKind::Concrete);
                            self.file_message = Some((
                                format!("板 {} — {:.0}×{:.0}×{:.0}mm", name, w, d, t),
                                std::time::Instant::now(),
                            ));
                            self.editor.draw_state = DrawState::Idle;
                        }
                    }
                    _ => {}
                }
            }

            // ── Steel Mode Tools ──
            Tool::SteelColumn => {
                if let Some(p) = self.ground_snapped() {
                    self.scene.snapshot();
                    let member_h = self.editor.steel_height;
                    let (h_sec, b_sec, tw, tf) = parse_h_profile(&self.editor.steel_profile);
                    let name_base = self.next_name("COL");

                    let cx = p[0]; // column center X
                    let cz = p[2]; // column center Z

                    // Front flange (Z-)
                    let f1_id = self.scene.insert_box_raw(
                        format!("{}_F1", name_base),
                        [cx - b_sec / 2.0, 0.0, cz - h_sec / 2.0],
                        b_sec, member_h, tf, MaterialKind::Steel,
                    );
                    // Back flange (Z+)
                    let f2_id = self.scene.insert_box_raw(
                        format!("{}_F2", name_base),
                        [cx - b_sec / 2.0, 0.0, cz + h_sec / 2.0 - tf],
                        b_sec, member_h, tf, MaterialKind::Steel,
                    );
                    // Web (center)
                    let web_id = self.scene.insert_box_raw(
                        format!("{}_W", name_base),
                        [cx - tw / 2.0, 0.0, cz - h_sec / 2.0 + tf],
                        tw, member_h, h_sec - 2.0 * tf, MaterialKind::Steel,
                    );

                    // Set component kinds
                    for id in [&f1_id, &f2_id, &web_id] {
                        if let Some(obj) = self.scene.objects.get_mut(id) {
                            obj.component_kind = crate::collision::ComponentKind::Column;
                        }
                    }

                    // Group them
                    let child_ids = vec![f1_id.clone(), f2_id.clone(), web_id.clone()];
                    self.scene.create_group(name_base.clone(), child_ids.clone());
                    self.scene.version += 1;

                    self.editor.selected_ids = child_ids.clone();
                    self.ai_log.log(
                        &self.current_actor, "\u{5efa}\u{7acb}\u{67f1}",
                        &format!("{} H={:.0}", self.editor.steel_profile, member_h),
                        child_ids,
                    );
                    self.file_message = Some((
                        format!("\u{67f1}\u{5df2}\u{5efa}\u{7acb}: {} @ [{:.0},{:.0}]", self.editor.steel_profile, cx, cz),
                        std::time::Instant::now(),
                    ));
                }
            }

            Tool::SteelBeam => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::LineFrom { p1: [p[0], self.editor.steel_height, p[2]] };
                        }
                    }
                    DrawState::LineFrom { p1 } => {
                        let p1 = *p1;
                        if let Some(p2) = self.ground_snapped() {
                            self.scene.snapshot();
                            let (h_sec, b_sec, tw, tf) = parse_h_profile(&self.editor.steel_profile);
                            let beam_y = self.editor.steel_height - h_sec; // beam bottom

                            let dx = p2[0] - p1[0];
                            let dz = p2[2] - p1[2];
                            let length = (dx * dx + dz * dz).sqrt();
                            let name_base = self.next_name("BM");

                            let is_x_dir = dx.abs() > dz.abs();

                            let ids = if is_x_dir {
                                let min_x = p1[0].min(p2[0]);
                                let cz = p1[2];
                                // Top flange
                                let f1 = self.scene.insert_box_raw(
                                    format!("{}_TF", name_base),
                                    [min_x, beam_y + h_sec - tf, cz - b_sec / 2.0],
                                    length, tf, b_sec, MaterialKind::Steel,
                                );
                                // Bottom flange
                                let f2 = self.scene.insert_box_raw(
                                    format!("{}_BF", name_base),
                                    [min_x, beam_y, cz - b_sec / 2.0],
                                    length, tf, b_sec, MaterialKind::Steel,
                                );
                                // Web
                                let w = self.scene.insert_box_raw(
                                    format!("{}_W", name_base),
                                    [min_x, beam_y + tf, cz - tw / 2.0],
                                    length, h_sec - 2.0 * tf, tw, MaterialKind::Steel,
                                );
                                vec![f1, f2, w]
                            } else {
                                let min_z = p1[2].min(p2[2]);
                                let cx = p1[0];
                                // Top flange
                                let f1 = self.scene.insert_box_raw(
                                    format!("{}_TF", name_base),
                                    [cx - b_sec / 2.0, beam_y + h_sec - tf, min_z],
                                    b_sec, tf, length, MaterialKind::Steel,
                                );
                                // Bottom flange
                                let f2 = self.scene.insert_box_raw(
                                    format!("{}_BF", name_base),
                                    [cx - b_sec / 2.0, beam_y, min_z],
                                    b_sec, tf, length, MaterialKind::Steel,
                                );
                                // Web
                                let w = self.scene.insert_box_raw(
                                    format!("{}_W", name_base),
                                    [cx - tw / 2.0, beam_y + tf, min_z],
                                    tw, h_sec - 2.0 * tf, length, MaterialKind::Steel,
                                );
                                vec![f1, f2, w]
                            };

                            for id in &ids {
                                if let Some(obj) = self.scene.objects.get_mut(id) {
                                    obj.component_kind = crate::collision::ComponentKind::Beam;
                                }
                            }

                            self.scene.create_group(name_base.clone(), ids.clone());
                            self.scene.version += 1;

                            self.editor.selected_ids = ids.clone();
                            self.editor.draw_state = DrawState::Idle;
                            self.ai_log.log(
                                &self.current_actor, "\u{5efa}\u{7acb}\u{6881}",
                                &format!("{} L={:.0}", self.editor.steel_profile, length),
                                ids,
                            );
                        }
                    }
                    _ => {}
                }
            }

            Tool::SteelBrace => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::LineFrom { p1: p };
                        }
                    }
                    DrawState::LineFrom { p1 } => {
                        let p1 = *p1;
                        if let Some(p2) = self.ground_snapped() {
                            self.scene.snapshot();
                            let name = self.next_name("BR");
                            let id = self.scene.add_line(name, vec![p1, [p2[0], self.editor.steel_height, p2[2]]], 50.0, MaterialKind::Steel);
                            self.editor.selected_ids = vec![id.clone()];
                            self.editor.draw_state = DrawState::Idle;
                            self.ai_log.log(&self.current_actor, "建立斜撐", "", vec![id]);
                        }
                    }
                    _ => {}
                }
            }

            Tool::SteelPlate => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::BoxBase { p1: p };
                        }
                    }
                    DrawState::BoxBase { p1 } => {
                        let p1 = *p1;
                        if let Some(p2) = self.ground_snapped() {
                            self.scene.snapshot();
                            let x0 = p1[0].min(p2[0]);
                            let z0 = p1[2].min(p2[2]);
                            let w = (p1[0] - p2[0]).abs().max(10.0);
                            let d = (p1[2] - p2[2]).abs().max(10.0);
                            let thickness = 12.0;
                            let name = self.next_name("PL");
                            let id = self.scene.add_box(name, [x0, 0.0, z0], w, thickness, d, MaterialKind::Metal);
                            if let Some(obj) = self.scene.objects.get_mut(&id) {
                                obj.component_kind = crate::collision::ComponentKind::Plate;
                            }
                            self.editor.selected_ids = vec![id.clone()];
                            self.editor.draw_state = DrawState::Idle;
                            self.editor.tool = Tool::PushPull;
                            self.file_message = Some(("鋼板已建立 — 用推拉設定厚度".into(), std::time::Instant::now()));
                        }
                    }
                    _ => {}
                }
            }

            Tool::SteelGrid => {
                if let Some(p) = self.ground_snapped() {
                    let half = 20000.0;
                    self.scene.guide_lines.push(([p[0], 0.0, -half], [p[0], 0.0, half]));
                    self.scene.guide_lines.push(([-half, 0.0, p[2]], [half, 0.0, p[2]]));
                    self.file_message = Some((format!("軸線已建立 @ [{:.0}, {:.0}]", p[0], p[2]), std::time::Instant::now()));
                }
            }

            Tool::SteelConnection => {
                let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                if let Some(id) = self.pick(mx, my, vw, vh) {
                    if !self.editor.selected_ids.contains(&id) {
                        self.editor.selected_ids.push(id);
                    }
                    if self.editor.selected_ids.len() >= 2 {
                        self.file_message = Some(("接頭已標記（選取兩構件）".into(), std::time::Instant::now()));
                    } else {
                        self.file_message = Some(("選取第二個構件".into(), std::time::Instant::now()));
                    }
                }
            }

            Tool::PushPull => {
                if matches!(self.editor.draw_state, DrawState::Idle) {
                    let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                    let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                    let clicked_face = self.pick_face(mx, my, vw, vh);

                    // A4: Double-click repeats last pull distance
                    if let Some((ref id, face)) = clicked_face {
                        let is_double = self.editor.last_pull_click_time.elapsed().as_millis() < 500
                            && self.editor.last_pull_distance.abs() > 0.1
                            && self.editor.last_pull_face.as_ref()
                                .map(|(lid, lf)| lid == id && *lf == face)
                                .unwrap_or(false);

                        if is_double {
                            // Apply last pull distance instantly
                            self.scene.snapshot();
                            let dist = self.editor.last_pull_distance;
                            if let Some(obj) = self.scene.objects.get_mut(id.as_str()) {
                                match (&mut obj.shape, face) {
                                    (Shape::Box { height, .. }, PullFace::Top) =>
                                        *height = (*height + dist).max(10.0),
                                    (Shape::Box { height, .. }, PullFace::Bottom) => {
                                        let delta = dist.min(*height - 10.0);
                                        *height = (*height - delta).max(10.0);
                                        obj.position[1] += delta;
                                    }
                                    (Shape::Box { width, .. }, PullFace::Right) =>
                                        *width = (*width + dist).max(10.0),
                                    (Shape::Box { width, .. }, PullFace::Left) => {
                                        let delta = dist.min(*width - 10.0);
                                        *width = (*width - delta).max(10.0);
                                        obj.position[0] += delta;
                                    }
                                    (Shape::Box { depth, .. }, PullFace::Back) =>
                                        *depth = (*depth + dist).max(10.0),
                                    (Shape::Box { depth, .. }, PullFace::Front) => {
                                        let delta = dist.min(*depth - 10.0);
                                        *depth = (*depth - delta).max(10.0);
                                        obj.position[2] += delta;
                                    }
                                    (Shape::Cylinder { height, .. }, PullFace::Top) =>
                                        *height = (*height + dist).max(10.0),
                                    (Shape::Cylinder { height, .. }, PullFace::Bottom) => {
                                        let delta = dist.min(*height - 10.0);
                                        *height = (*height - delta).max(10.0);
                                        obj.position[1] += delta;
                                    }
                                    _ => {}
                                }
                            }
                            self.file_message = Some((
                                format!("\u{91cd}\u{8907}\u{63a8}\u{62c9} {:.0}mm", dist),
                                std::time::Instant::now(),
                            ));
                            self.editor.last_pull_click_time = std::time::Instant::now();
                            // Don't change selection, keep face selected
                        } else {
                            // Check if clicking the SAME face that's already selected → toggle off
                            let same = self.editor.selected_face.as_ref()
                                .map(|(sid, sf)| sid == id && *sf == face)
                                .unwrap_or(false);

                            if same {
                                // Deselect face
                                self.editor.selected_face = None;
                            } else {
                                // Select this face (highlight + show properties)
                                self.editor.selected_face = Some((id.clone(), face));
                                self.editor.selected_ids = vec![id.clone()];
                                self.right_tab = RightTab::Properties;
                            }
                        }
                    } else if let Some(fid) = self.pick_free_mesh_face(mx, my, vw, vh) {
                        self.editor.selected_ids.clear();
                        self.editor.selected_face = None;
                        self.editor.draw_state = DrawState::PullingFreeMesh { face_id: fid };
                    } else {
                        // Clicked empty space → deselect face
                        self.editor.selected_face = None;
                    }
                }
            }
        }

        // Fallback: any click on an object selects it (like SketchUp)
        // Only when idle (not mid-draw) and nothing was selected by the tool handler
        if matches!(self.editor.draw_state, DrawState::Idle) && self.editor.selected_ids.is_empty() {
            let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
            let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
            if let Some(id) = self.pick(mx, my, vw, vh) {
                self.editor.selected_ids = vec![id];
                // Expand selection to include all group members
                self.expand_selection_to_groups();
                self.right_tab = RightTab::Properties;
            }
        }

        self.editor.measure_input.clear();
    }

    pub(crate) fn apply_measure(&mut self) {
        // Array creation: "3x" or "5X" after Ctrl+Move copy
        // Uses persisted last_move_delta / last_move_was_copy (B8) since
        // move_is_copy and move_origin are cleared when drag stops.
        if (self.editor.measure_input.ends_with('x') || self.editor.measure_input.ends_with('X'))
            && self.editor.measure_input.len() > 1
        {
            let count_str = &self.editor.measure_input[..self.editor.measure_input.len() - 1];
            if let Ok(count) = count_str.parse::<usize>() {
                // Try live drag state first, then fall back to persisted delta
                let is_copy = self.editor.move_is_copy || self.editor.last_move_was_copy;
                let delta_opt: Option<[f32; 3]> = if self.editor.move_is_copy {
                    // Still dragging (unlikely but handle it)
                    self.editor.move_origin.and_then(|orig| {
                        self.editor.selected_ids.first().and_then(|id| {
                            self.scene.objects.get(id).map(|obj| [
                                obj.position[0] - orig[0],
                                obj.position[1] - orig[1],
                                obj.position[2] - orig[2],
                            ])
                        })
                    })
                } else {
                    self.editor.last_move_delta
                };

                if count >= 2 && count <= 100 && is_copy && !self.editor.selected_ids.is_empty() {
                    if let Some(delta) = delta_opt {
                        // Get current positions of selected objects as base
                        let base_objs: Vec<_> = self.editor.selected_ids.iter()
                            .filter_map(|id| self.scene.objects.get(id).cloned())
                            .collect();
                        if !base_objs.is_empty() {
                            self.scene.snapshot();
                            // Create (count-1) more copies at equal intervals from the first copy
                            for i in 1..count {
                                for base in &base_objs {
                                    let mut clone = base.clone();
                                    clone.id = self.scene.next_id_pub();
                                    clone.position = [
                                        base.position[0] + delta[0] * i as f32,
                                        base.position[1] + delta[1] * i as f32,
                                        base.position[2] + delta[2] * i as f32,
                                    ];
                                    self.scene.objects.insert(clone.id.clone(), clone);
                                }
                            }
                            self.scene.version += 1;
                            self.file_message = Some((
                                format!("\u{5df2}\u{5efa}\u{7acb} {} \u{500b}\u{526f}\u{672c}", count),
                                std::time::Instant::now(),
                            ));
                            self.editor.last_move_delta = None;
                            self.editor.last_move_was_copy = false;
                            self.editor.measure_input.clear();
                            return;
                        }
                    }
                }
            }
        }

        // Polar array: "6r" or "4R" — N copies rotated equally around Y at selection center
        if (self.editor.measure_input.ends_with('r') || self.editor.measure_input.ends_with('R'))
            && self.editor.measure_input.len() > 1
            && !self.editor.selected_ids.is_empty()
        {
            let count_str = &self.editor.measure_input[..self.editor.measure_input.len() - 1];
            if let Ok(count) = count_str.parse::<usize>() {
                if count >= 2 && count <= 100 {
                    // 計算選取物件的中心
                    let selected_objs: Vec<_> = self.editor.selected_ids.iter()
                        .filter_map(|id| self.scene.objects.get(id).cloned())
                        .collect();
                    if !selected_objs.is_empty() {
                        let mut cx = 0.0_f32;
                        let mut cz = 0.0_f32;
                        for o in &selected_objs {
                            let (w, _, d) = match &o.shape {
                                Shape::Box { width, depth, .. } => (*width, 0.0, *depth),
                                Shape::Cylinder { radius, .. } => (*radius * 2.0, 0.0, *radius * 2.0),
                                _ => (0.0, 0.0, 0.0),
                            };
                            cx += o.position[0] + w / 2.0;
                            cz += o.position[2] + d / 2.0;
                        }
                        cx /= selected_objs.len() as f32;
                        cz /= selected_objs.len() as f32;

                        self.scene.snapshot();
                        let angle_step = std::f32::consts::TAU / count as f32;
                        let mut new_ids = Vec::new();
                        for i in 1..count {
                            let angle = angle_step * i as f32;
                            let (sin_a, cos_a) = angle.sin_cos();
                            for base in &selected_objs {
                                let mut clone = base.clone();
                                clone.id = self.scene.next_id_pub();
                                // 繞 (cx, cz) 旋轉
                                let dx = clone.position[0] - cx;
                                let dz = clone.position[2] - cz;
                                clone.position[0] = cx + dx * cos_a - dz * sin_a;
                                clone.position[2] = cz + dx * sin_a + dz * cos_a;
                                clone.rotation_y += angle;
                                new_ids.push(clone.id.clone());
                                self.scene.objects.insert(clone.id.clone(), clone);
                            }
                        }
                        self.scene.version += 1;
                        self.editor.selected_ids.extend(new_ids);
                        self.file_message = Some((
                            format!("極座標陣列：{} 個副本（{:.0}° 間隔）",
                                count, angle_step.to_degrees()),
                            std::time::Instant::now(),
                        ));
                        self.editor.measure_input.clear();
                        return;
                    }
                }
            }
        }

        let parts: Vec<f32> = self.editor.measure_input
            .split(|c: char| c == ',' || c == 'x')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if parts.is_empty() { return; }

        match &self.editor.draw_state {
            DrawState::BoxBase { p1 } => {
                if parts.len() >= 2 {
                    let p1 = *p1;
                    let p2 = [p1[0]+parts[0], 0.0, p1[2]+parts[1]];
                    self.editor.draw_state = DrawState::BoxHeight { p1, p2 };
                }
            }
            DrawState::BoxHeight { p1, p2 } => {
                let p1 = *p1; let p2 = *p2;
                let x0 = p1[0].min(p2[0]);
                let z0 = p1[2].min(p2[2]);
                let w = (p1[0]-p2[0]).abs().max(10.0);
                let d = (p1[2]-p2[2]).abs().max(10.0);
                let h = parts[0].max(10.0);
                let name = self.next_name("Box");
                let id = self.scene.add_box(name, [x0, 0.0, z0], w, h, d, self.create_mat);
                self.editor.selected_ids = vec![id];
                self.editor.draw_state = DrawState::Idle;
            }
            DrawState::CylBase { center } => {
                let c = *center;
                let r = parts[0].max(10.0);
                self.editor.draw_state = DrawState::CylHeight { center: c, radius: r };
            }
            DrawState::CylHeight { center, radius } => {
                let c = *center; let r = *radius;
                let h = parts[0].max(10.0);
                let name = self.next_name("Cylinder");
                let id = self.scene.add_cylinder(name, c, r, h, 48, self.create_mat);
                self.editor.selected_ids = vec![id];
                self.editor.draw_state = DrawState::Idle;
            }
            DrawState::SphRadius { center } => {
                let c = *center;
                let r = parts[0].max(10.0);
                let name = self.next_name("Sphere");
                let id = self.scene.add_sphere(name, c, r, 32, self.create_mat);
                self.editor.selected_ids = vec![id];
                self.editor.draw_state = DrawState::Idle;
            }
            DrawState::PullingFreeMesh { face_id } => {
                let fid = *face_id;
                let height = parts[0];
                self.scene.snapshot();
                self.scene.free_mesh.push_pull_face(fid, height);
                self.scene.version += 1;
                self.editor.draw_state = DrawState::Idle;
                self.file_message = Some((
                    format!("\u{9762}\u{5df2}\u{63a8}\u{62c9} {}mm", height),
                    std::time::Instant::now(),
                ));
            }
            DrawState::Scaling { ref obj_id, handle, original_dims } => {
                let obj_id = obj_id.clone();
                let original_dims = *original_dims;
                let handle = *handle;
                let input = &self.editor.measure_input;

                // Parse as scale factor (e.g., "1.5", "x1.5", "150%") or absolute dimension (e.g., "2000")
                let value: Option<f32> = if input.ends_with('%') {
                    input.trim_end_matches('%').parse::<f32>().ok().map(|v| v / 100.0)
                } else if input.starts_with('x') || input.starts_with('X') {
                    input[1..].parse::<f32>().ok()
                } else {
                    input.parse::<f32>().ok()
                };

                if let Some(val) = value {
                    self.scene.snapshot();
                    let is_factor = input.contains('%') || input.starts_with('x') || input.starts_with('X') || val < 10.0;

                    if let Some(obj) = self.scene.objects.get_mut(&obj_id) {
                        match (&mut obj.shape, handle) {
                            (Shape::Box { width, height, depth }, ScaleHandle::Uniform) => {
                                if is_factor {
                                    *width = (original_dims[0] * val).max(10.0);
                                    *height = (original_dims[1] * val).max(10.0);
                                    *depth = (original_dims[2] * val).max(10.0);
                                }
                            }
                            (Shape::Box { width, .. }, ScaleHandle::AxisX) => {
                                *width = if is_factor { (original_dims[0] * val).max(10.0) } else { val.max(10.0) };
                            }
                            (Shape::Box { height, .. }, ScaleHandle::AxisY) => {
                                *height = if is_factor { (original_dims[1] * val).max(10.0) } else { val.max(10.0) };
                            }
                            (Shape::Box { depth, .. }, ScaleHandle::AxisZ) => {
                                *depth = if is_factor { (original_dims[2] * val).max(10.0) } else { val.max(10.0) };
                            }
                            (Shape::Cylinder { radius, height, .. }, ScaleHandle::Uniform) => {
                                if is_factor {
                                    *radius = (original_dims[0] / 2.0 * val).max(10.0);
                                    *height = (original_dims[1] * val).max(10.0);
                                }
                            }
                            (Shape::Cylinder { height, .. }, ScaleHandle::AxisY) => {
                                *height = if is_factor { (original_dims[1] * val).max(10.0) } else { val.max(10.0) };
                            }
                            (Shape::Cylinder { radius, .. }, ScaleHandle::AxisX | ScaleHandle::AxisZ) => {
                                *radius = if is_factor { (original_dims[0] / 2.0 * val).max(10.0) } else { (val / 2.0).max(10.0) };
                            }
                            (Shape::Sphere { radius, .. }, _) => {
                                *radius = if is_factor { (original_dims[0] / 2.0 * val).max(10.0) } else { (val / 2.0).max(10.0) };
                            }
                            _ => {}
                        }
                    }
                    self.editor.draw_state = DrawState::Idle;
                }
            }
            // D1: Rotate step 3 — type angle in degrees for precise rotation
            DrawState::RotateAngle { ref obj_ids, ref_angle: _, ref original_rotations, .. } => {
                if let Ok(angle) = self.editor.measure_input.parse::<f32>() {
                    let delta = angle.to_radians();
                    for (i, id) in obj_ids.iter().enumerate() {
                        let orig = original_rotations.get(i).copied().unwrap_or(0.0);
                        if let Some(obj) = self.scene.objects.get_mut(id) {
                            obj.rotation_y = orig + delta;
                        }
                    }
                    self.editor.draw_state = DrawState::Idle;
                }
            }
            _ => {}
        }
        self.editor.measure_input.clear();
    }

    /// 重建空間索引（場景變更時呼叫）
    pub(crate) fn rebuild_spatial_index(&mut self) {
        if self.scene.version == self.spatial_index_version { return; }
        use crate::app::SpatialEntry;
        let entries: Vec<SpatialEntry> = self.scene.objects.values().map(|obj| {
            let p = obj.position;
            let (mn, mx) = match &obj.shape {
                Shape::Box { width, height, depth } => (p, [p[0]+width, p[1]+height, p[2]+depth]),
                Shape::Cylinder { radius, height, .. } => (p, [p[0]+radius*2.0, p[1]+height, p[2]+radius*2.0]),
                Shape::Sphere { radius, .. } => (p, [p[0]+radius*2.0, p[1]+radius*2.0, p[2]+radius*2.0]),
                Shape::Line { points, thickness, .. } => {
                    let mut mx = p;
                    for pt in points { mx[0] = mx[0].max(pt[0]+thickness); mx[1] = mx[1].max(pt[1]+thickness); mx[2] = mx[2].max(pt[2]+thickness); }
                    (p, mx)
                }
                Shape::Mesh(ref mesh) => { let (a,b) = mesh.aabb(); ([p[0]+a[0],p[1]+a[1],p[2]+a[2]], [p[0]+b[0],p[1]+b[1],p[2]+b[2]]) }
            };
            SpatialEntry { id: obj.id.clone(), min: mn, max: mx }
        }).collect();
        self.spatial_index = Some(rstar::RTree::bulk_load(entries));
        self.spatial_index_version = self.scene.version;
    }

    pub(crate) fn pick(&mut self, mx: f32, my: f32, vw: f32, vh: f32) -> Option<String> {
        self.rebuild_spatial_index();
        let (origin, dir) = self.viewer.camera.screen_ray(mx, my, vw, vh);
        let mut best: Option<(f32, String)> = None;
        let editing_gid = &self.editor.editing_group_id;

        // 使用空間索引：沿射線採樣多個點，查詢附近的候選物件
        let candidates: Vec<String> = if let Some(ref tree) = self.spatial_index {
            use rstar::RTreeObject;
            let mut ids = std::collections::HashSet::new();
            // 沿射線採樣 10 個點（距離 100-50000mm）
            for i in 0..10 {
                let t = 100.0 + (i as f32) * 5000.0;
                let pt = origin + dir * t;
                let query_pt = [pt.x, pt.y, pt.z];
                // 查詢最近的 5 個 AABB
                for entry in tree.nearest_neighbor_iter(&query_pt).take(5) {
                    ids.insert(entry.id.clone());
                }
            }
            ids.into_iter().collect()
        } else {
            self.scene.objects.keys().cloned().collect()
        };

        for id in &candidates {
            let obj = match self.scene.objects.get(id) { Some(o) => o, None => continue };
            if obj.locked { continue; } // 鎖定物件不可選取
            if let Some(gid) = editing_gid {
                if obj.parent_id.as_deref() != Some(gid.as_str()) && &obj.id != gid { continue; }
            }
            let pos = glam::Vec3::from(obj.position);
            let (pick_min, pick_max) = match &obj.shape {
                Shape::Box { width, height, depth } => {
                    let pick_sz = 30.0_f32;
                    let center = pos + glam::Vec3::new(*width, *height, *depth) * 0.5;
                    let half = glam::Vec3::new(width.max(pick_sz), height.max(pick_sz), depth.max(pick_sz)) * 0.5;
                    (center - half, center + half)
                }
                Shape::Cylinder { radius, height, .. } => (pos, pos + glam::Vec3::new(*radius*2.0, *height, *radius*2.0)),
                Shape::Sphere { radius, .. } => (pos, pos + glam::Vec3::splat(*radius*2.0)),
                Shape::Line { points, thickness, .. } => {
                    let mut mx = pos;
                    for pt in points { mx = mx.max(glam::Vec3::from(*pt) + glam::Vec3::splat(*thickness)); }
                    (pos, mx)
                }
                Shape::Mesh(ref mesh) => {
                    let (_, aabb_max) = mesh.aabb();
                    (pos, glam::Vec3::from(aabb_max))
                }
            };
            if let Some(t) = camera::ray_aabb(origin, dir, pick_min, pick_max) {
                if best.as_ref().map_or(true, |(bt,_)| t < *bt) { best = Some((t, obj.id.clone())); }
            }
        }
        best.map(|(_, id)| id)
    }

    /// B5: After push/pull, move adjacent objects that were touching the pulled face
    /// to maintain contact. Only moves (does not resize) adjacent boxes.
    fn adjust_adjacent_after_pull(scene: &mut crate::scene::Scene, pulled_id: &str, face: PullFace, delta: f32) {
        let pulled = match scene.objects.get(pulled_id).cloned() {
            Some(o) => o,
            None => return,
        };
        let (pw, ph, pd) = match &pulled.shape {
            Shape::Box { width, height, depth } => (*width, *height, *depth),
            _ => return,
        };
        let pp = pulled.position;
        let tol = 5.0; // 5mm tolerance for face adjacency

        let ids: Vec<String> = scene.objects.keys().cloned().collect();
        for id in &ids {
            if id == pulled_id { continue; }
            if let Some(other) = scene.objects.get_mut(id) {
                if let Shape::Box { .. } = &other.shape {
                    match face {
                        PullFace::Right => {
                            // Right face of pulled obj grew rightward; push adjacent objects right
                            // Before pull, right face was at pp[0] + pw - delta
                            if (pp[0] + pw - delta - other.position[0]).abs() < tol {
                                other.position[0] += delta;
                            }
                        }
                        PullFace::Left => {
                            // Left face moved leftward (position decreased)
                            // Before pull, left face was at pp[0] + delta (since position moved by -delta)
                            // Check objects whose right face touched original left face
                            if let Shape::Box { width: ow, .. } = &other.shape {
                                if (other.position[0] + ow - (pp[0] - delta)).abs() < tol {
                                    other.position[0] += delta; // delta is negative for left pull
                                }
                            }
                        }
                        PullFace::Top => {
                            if (pp[1] + ph - delta - other.position[1]).abs() < tol {
                                other.position[1] += delta;
                            }
                        }
                        PullFace::Bottom => {
                            if let Shape::Box { height: oh, .. } = &other.shape {
                                if (other.position[1] + oh - (pp[1] - delta)).abs() < tol {
                                    other.position[1] += delta;
                                }
                            }
                        }
                        PullFace::Back => {
                            if (pp[2] + pd - delta - other.position[2]).abs() < tol {
                                other.position[2] += delta;
                            }
                        }
                        PullFace::Front => {
                            if let Shape::Box { depth: od, .. } = &other.shape {
                                if (other.position[2] + od - (pp[2] - delta)).abs() < tol {
                                    other.position[2] += delta;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Pick which face of which object the ray hits (for Push/Pull)
    pub(crate) fn pick_face(&self, mx: f32, my: f32, vw: f32, vh: f32) -> Option<(String, PullFace)> {
        let (origin, dir) = self.viewer.camera.screen_ray(mx, my, vw, vh);
        let mut best: Option<(f32, String, PullFace)> = None;

        for obj in self.scene.objects.values() {
            let p = glam::Vec3::from(obj.position);
            let (faces, obj_max): (Vec<(PullFace, glam::Vec3, glam::Vec3)>, _) = match &obj.shape {
                Shape::Box { width, height, depth } => {
                    let max = p + glam::Vec3::new(*width, *height, *depth);
                    let faces = vec![
                        (PullFace::Top,    glam::Vec3::new(p.x, max.y, p.z), glam::Vec3::new(max.x, max.y, max.z)),
                        (PullFace::Bottom, glam::Vec3::new(p.x, p.y, p.z),   glam::Vec3::new(max.x, p.y, max.z)),
                        (PullFace::Right,  glam::Vec3::new(max.x, p.y, p.z), glam::Vec3::new(max.x, max.y, max.z)),
                        (PullFace::Left,   glam::Vec3::new(p.x, p.y, p.z),   glam::Vec3::new(p.x, max.y, max.z)),
                        (PullFace::Back,   glam::Vec3::new(p.x, p.y, max.z), glam::Vec3::new(max.x, max.y, max.z)),
                        (PullFace::Front,  glam::Vec3::new(p.x, p.y, p.z),   glam::Vec3::new(max.x, max.y, p.z)),
                    ];
                    (faces, max)
                }
                Shape::Cylinder { radius, height, .. } => {
                    let max = p + glam::Vec3::new(*radius * 2.0, *height, *radius * 2.0);
                    let faces = vec![
                        (PullFace::Top,    glam::Vec3::new(p.x, max.y, p.z), max),
                        (PullFace::Bottom, p, glam::Vec3::new(max.x, p.y, max.z)),
                    ];
                    (faces, max)
                }
                _ => continue,
            };

            // Test each face quad as a plane
            for (face, fmin, fmax) in &faces {
                let normal = match face {
                    PullFace::Top    => glam::Vec3::Y,
                    PullFace::Bottom => glam::Vec3::NEG_Y,
                    PullFace::Right  => glam::Vec3::X,
                    PullFace::Left   => glam::Vec3::NEG_X,
                    PullFace::Back   => glam::Vec3::Z,
                    PullFace::Front  => glam::Vec3::NEG_Z,
                };
                let denom = dir.dot(normal);
                if denom.abs() < 1e-6 { continue; } // parallel

                // fmin is a known point on the face plane — correct for all orientations
                let t = (*fmin - origin).dot(normal) / denom;
                if t < 0.0 { continue; }

                let hit = origin + dir * t;

                // Check if hit is within face bounds
                let in_bounds = hit.x >= fmin.x - 1.0 && hit.x <= fmax.x + 1.0
                    && hit.y >= fmin.y - 1.0 && hit.y <= fmax.y + 1.0
                    && hit.z >= fmin.z - 1.0 && hit.z <= fmax.z + 1.0;
                if !in_bounds { continue; }

                if best.as_ref().map_or(true, |(bt, _, _)| t < *bt) {
                    best = Some((t, obj.id.clone(), *face));
                }
            }
            let _ = obj_max;
        }

        best.map(|(_, id, face)| (id, face))
    }

    /// Pick a face on the shared free mesh by ray-casting.
    pub(crate) fn pick_free_mesh_face(&self, mx: f32, my: f32, vw: f32, vh: f32) -> Option<u32> {
        let (origin, dir) = self.viewer.camera.screen_ray(mx, my, vw, vh);
        let mut best: Option<(f32, u32)> = None;

        for (&fid, face) in &self.scene.free_mesh.faces {
            let verts = self.scene.free_mesh.face_vertices(fid);
            if verts.len() < 3 { continue; }

            let normal = glam::Vec3::from(face.normal);
            let v0 = glam::Vec3::from(verts[0]);

            // Ray-plane intersection
            let denom = dir.dot(normal);
            if denom.abs() < 1e-6 { continue; }
            let t = (v0 - origin).dot(normal) / denom;
            if t < 0.0 { continue; }

            let hit = origin + dir * t;

            // Point-in-polygon test using winding number on the projected plane
            let mut inside = false;
            let n = verts.len();
            // Use a simple crossing-number (ray-casting) test projected onto the
            // dominant plane (drop the axis with the largest normal component).
            let (ax1, ax2) = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
                (1usize, 2usize) // drop X, use Y/Z
            } else if normal.y.abs() >= normal.z.abs() {
                (0, 2) // drop Y, use X/Z
            } else {
                (0, 1) // drop Z, use X/Y
            };
            let hx = [hit.x, hit.y, hit.z][ax1];
            let hy = [hit.x, hit.y, hit.z][ax2];
            let mut j = n - 1;
            for i in 0..n {
                let yi = verts[i][ax2]; let yj = verts[j][ax2];
                let xi = verts[i][ax1]; let xj = verts[j][ax1];
                if ((yi > hy) != (yj > hy))
                    && (hx < (xj - xi) * (hy - yi) / (yj - yi) + xi)
                {
                    inside = !inside;
                }
                j = i;
            }
            if !inside { continue; }

            if best.as_ref().map_or(true, |(bt, _)| t < *bt) {
                best = Some((t, fid));
            }
        }

        best.map(|(_, fid)| fid)
    }

    /// Execute a menu action without unsaved-changes check (used after confirmation).
    pub(crate) fn force_menu_action(&mut self, action: crate::menu::MenuAction) {
        use crate::menu::MenuAction;
        match action {
            MenuAction::NewScene => {
                self.scene.clear();
                self.editor.selected_ids.clear();
                self.editor.draw_state = DrawState::Idle;
                self.current_file = None;
                self.last_saved_version = self.scene.version;
                self.file_message = Some(("新建場景".to_string(), std::time::Instant::now()));
            }
            MenuAction::OpenScene => self.open_scene(),
            MenuAction::Revert => {
                if let Some(ref path) = self.current_file.clone() {
                    match self.scene.load_from_file(path) {
                        Ok(count) => {
                            self.editor.selected_ids.clear();
                            self.last_saved_version = self.scene.version;
                            self.file_message = Some((format!("已回復: {} 個物件", count), std::time::Instant::now()));
                        }
                        Err(e) => self.file_message = Some((format!("回復失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            _ => self.handle_menu_action(action),
        }
    }

    pub(crate) fn handle_menu_action(&mut self, action: crate::menu::MenuAction) {
        use crate::menu::MenuAction;
        match action {
            MenuAction::None => {}
            MenuAction::NewScene | MenuAction::Revert => {
                if self.has_unsaved_changes() {
                    self.pending_action = Some(action);
                } else {
                    self.force_menu_action(action);
                }
            }
            MenuAction::OpenScene => {
                if self.has_unsaved_changes() {
                    self.pending_action = Some(action);
                } else {
                    self.open_scene();
                }
            }
            MenuAction::OpenRecent(ref path) => {
                let path = path.clone();
                if path.ends_with(".obj") {
                    match crate::obj_io::import_obj(&mut self.scene, &path) {
                        Ok(count) => {
                            self.editor.selected_ids.clear();
                            self.file_message = Some((format!("已匯入 {} 個物件", count), std::time::Instant::now()));
                        }
                        Err(e) => self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now())),
                    }
                } else {
                    match self.scene.load_from_file(&path) {
                        Ok(count) => {
                            self.current_file = Some(path.clone());
                            self.add_recent_file(&path);
                            self.editor.selected_ids.clear();
                            self.last_saved_version = self.scene.version;
                            self.file_message = Some((format!("已載入 {} 個物件", count), std::time::Instant::now()));
                        }
                        Err(e) => self.file_message = Some((format!("載入失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::SaveScene => self.save_scene(),
            MenuAction::SaveAs => {
                self.current_file = None; // force dialog
                self.save_scene();
            }
            MenuAction::Undo => { self.scene.undo(); }
            MenuAction::Redo => { self.scene.redo(); }
            MenuAction::Delete => {
                for id in self.editor.selected_ids.drain(..).collect::<Vec<_>>() {
                    self.scene.delete(&id);
                }
            }
            MenuAction::SelectAll => {
                self.editor.selected_ids = self.scene.objects.keys().cloned().collect();
            }
            MenuAction::ViewFront => self.viewer.camera.set_front(),
            MenuAction::ViewBack => self.viewer.camera.set_back(),
            MenuAction::ViewLeft => self.viewer.camera.set_left(),
            MenuAction::ViewRight => self.viewer.camera.set_right(),
            MenuAction::ViewTop => self.viewer.camera.set_top(),
            MenuAction::ViewBottom => self.viewer.camera.set_bottom(),
            MenuAction::ViewIso => self.viewer.camera.set_iso(),
            MenuAction::ZoomExtents => self.zoom_extents(),
            MenuAction::Duplicate => {
                let mut new_ids = Vec::new();
                for id in &self.editor.selected_ids.clone() {
                    if let Some(obj) = self.scene.objects.get(id) {
                        let mut clone = obj.clone();
                        clone.id = self.scene.next_id_pub();
                        clone.name = format!("{}_copy", clone.name);
                        clone.position[0] += 500.0;
                        let new_id = clone.id.clone();
                        self.scene.objects.insert(new_id.clone(), clone);
                        new_ids.push(new_id);
                    }
                }
                if !new_ids.is_empty() {
                    self.scene.version += 1;
                    self.editor.selected_ids = new_ids;
                }
            }
            MenuAction::GroupSelected => {
                if self.editor.selected_ids.len() >= 2 {
                    self.scene.snapshot();
                    let name = format!("Group_{}", self.scene.groups.len() + 1);
                    let gid = self.scene.create_group(name, self.editor.selected_ids.clone());
                    self.file_message = Some((format!("已建立群組: {}", gid), std::time::Instant::now()));
                } else {
                    self.file_message = Some(("需要選取至少2個物件".to_string(), std::time::Instant::now()));
                }
            }
            MenuAction::ComponentSelected => {
                for id in &self.editor.selected_ids {
                    if let Some(obj) = self.scene.objects.get_mut(id) {
                        if !obj.name.contains("[元件]") {
                            obj.name = format!("[元件] {}", obj.name);
                        }
                    }
                }
            }
            MenuAction::Properties => {
                self.right_tab = RightTab::Properties;
            }
            MenuAction::ExportObj => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 OBJ")
                    .add_filter("OBJ 模型", &["obj"])
                    .set_file_name("export.obj")
                    .save_file();
                if let Some(path) = file {
                    let path_str = path.to_string_lossy().to_string();
                    match crate::obj_io::export_obj(&self.scene, &path_str) {
                        Ok(()) => self.file_message = Some((format!("已匯出: {}", path_str), std::time::Instant::now())),
                        Err(e) => self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ImportObj => {
                self.open_scene(); // reuse open_scene which now handles OBJ too
            }
            MenuAction::CsgUnion | MenuAction::CsgSubtract | MenuAction::CsgIntersect => {
                let op = match action {
                    MenuAction::CsgUnion => crate::csg::CsgOp::Union,
                    MenuAction::CsgSubtract => crate::csg::CsgOp::Subtract,
                    MenuAction::CsgIntersect => crate::csg::CsgOp::Intersect,
                    _ => unreachable!(),
                };

                if self.editor.selected_ids.len() >= 2 {
                    let id_a = self.editor.selected_ids[0].clone();
                    let id_b = self.editor.selected_ids[1].clone();

                    if let (Some(a), Some(b)) = (
                        self.scene.objects.get(&id_a).cloned(),
                        self.scene.objects.get(&id_b).cloned(),
                    ) {
                        if matches!(a.shape, Shape::Box{..}) && matches!(b.shape, Shape::Box{..}) {
                            self.scene.snapshot();
                            self.scene.objects.remove(&id_a);
                            self.scene.objects.remove(&id_b);

                            let results = crate::csg::box_csg(&a, &b, op);
                            let mut new_ids = Vec::new();
                            for obj in results {
                                let id = obj.id.clone();
                                self.scene.objects.insert(id.clone(), obj);
                                new_ids.push(id);
                            }
                            self.scene.version += 1;
                            self.editor.selected_ids = new_ids.clone();

                            let op_name = match op {
                                crate::csg::CsgOp::Union => "聯集",
                                crate::csg::CsgOp::Subtract => "差集",
                                crate::csg::CsgOp::Intersect => "交集",
                            };
                            self.file_message = Some((format!("布林{}: 產生 {} 個物件", op_name, new_ids.len()), std::time::Instant::now()));
                        } else {
                            self.file_message = Some(("布林運算僅支援方塊物件".to_string(), std::time::Instant::now()));
                        }
                    }
                } else {
                    self.file_message = Some(("請先選取兩個方塊物件".to_string(), std::time::Instant::now()));
                }
            }
            MenuAction::SetRenderMode(mode) => {
                self.viewer.render_mode = match mode {
                    0 => RenderMode::Shaded,
                    1 => RenderMode::Wireframe,
                    2 => RenderMode::XRay,
                    3 => RenderMode::HiddenLine,
                    5 => RenderMode::Sketch,
                    _ => RenderMode::Monochrome,
                };
            }
            MenuAction::ToggleBackground => {
                if self.viewer.sky_color[0] > 0.5 {
                    // Switch to dark
                    self.viewer.sky_color = [0.12, 0.12, 0.15];
                    self.viewer.ground_color = [0.2, 0.2, 0.22];
                } else {
                    // Switch to light
                    self.viewer.sky_color = [0.53, 0.72, 0.9];
                    self.viewer.ground_color = [0.65, 0.63, 0.60];
                }
            }
            MenuAction::SaveTemplate => {
                let file = rfd::FileDialog::new()
                    .set_title("存為範本")
                    .add_filter("Kolibri 範本", &["k3d"])
                    .set_directory("D:\\AI_Design\\Kolibri_Ai3D\\app\\templates")
                    .set_file_name("template.k3d")
                    .save_file();
                if let Some(path) = file {
                    let p = path.to_string_lossy().to_string();
                    match self.scene.save_to_file(&p) {
                        Ok(()) => self.file_message = Some((format!("範本已儲存: {}", p), std::time::Instant::now())),
                        Err(e) => self.file_message = Some((format!("儲存失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ExportPng => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 PNG 截圖")
                    .add_filter("PNG 圖片", &["png"])
                    .set_file_name("screenshot.png")
                    .save_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    self.viewport.save_screenshot(&self.device, &self.queue, &ps);
                    self.file_message = Some((format!("已匯出 PNG: {}", ps), std::time::Instant::now()));
                }
            }
            MenuAction::ExportJpg => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 JPG 截圖")
                    .add_filter("JPG 圖片", &["jpg", "jpeg"])
                    .set_file_name("screenshot.jpg")
                    .save_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    if let Some((w, h, rgb)) = self.viewport.capture_rgb(&self.device, &self.queue) {
                        if let Some(img) = image::RgbImage::from_raw(w, h, rgb) {
                            match img.save(&ps) {
                                Ok(_) => self.file_message = Some((format!("已匯出 JPG: {}", ps), std::time::Instant::now())),
                                Err(e) => self.file_message = Some((format!("JPG 匯出失敗: {}", e), std::time::Instant::now())),
                            }
                        }
                    }
                }
            }
            MenuAction::ExportPdf => {
                self.file_message = Some(("PDF 匯出功能開發中，請先使用 PNG 匯出".to_string(), std::time::Instant::now()));
            }
            MenuAction::ImportImage => {
                self.file_message = Some(("圖片參考底圖功能開發中".to_string(), std::time::Instant::now()));
            }
            MenuAction::ExportStl => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 STL")
                    .add_filter("STL 模型", &["stl"])
                    .set_file_name("export.stl")
                    .save_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    match crate::stl_io::export_stl(&self.scene, &ps) {
                        Ok(()) => self.file_message = Some((format!("已匯出 STL: {}", ps), std::time::Instant::now())),
                        Err(e) => self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ImportStl => {
                let file = rfd::FileDialog::new()
                    .set_title("匯入 STL")
                    .add_filter("STL 模型", &["stl"])
                    .pick_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    match crate::stl_io::import_stl(&mut self.scene, &ps) {
                        Ok(count) => {
                            self.editor.selected_ids.clear();
                            self.file_message = Some((format!("已匯入 {} 個物件: {}", count, ps), std::time::Instant::now()));
                        }
                        Err(e) => self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ExportGltf => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 GLTF")
                    .add_filter("GLTF 模型", &["gltf"])
                    .set_file_name("export.gltf")
                    .save_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    match crate::gltf_io::export_gltf(&self.scene, &ps) {
                        Ok(()) => self.file_message = Some((format!("已匯出 GLTF: {}", ps), std::time::Instant::now())),
                        Err(e) => self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ImportGltf => {
                self.file_message = Some(("GLTF 匯入尚未支援".to_string(), std::time::Instant::now()));
            }
            MenuAction::ExportDxf => {
                let file = rfd::FileDialog::new()
                    .set_title("匯出 DXF")
                    .add_filter("DXF 圖面", &["dxf", "DXF"])
                    .set_file_name("export.dxf")
                    .save_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    match crate::dxf_io::export_dxf(&self.scene, &ps) {
                        Ok(()) => self.file_message = Some((format!("已匯出 DXF: {}", ps), std::time::Instant::now())),
                        Err(e) => self.file_message = Some((format!("匯出失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ImportDxf => {
                let file = rfd::FileDialog::new()
                    .set_title("匯入 DXF")
                    .add_filter("DXF 圖面", &["dxf", "DXF"])
                    .pick_file();
                if let Some(p) = file {
                    let ps = p.to_string_lossy().to_string();
                    match crate::dxf_io::import_dxf(&mut self.scene, &ps) {
                        Ok(count) => {
                            self.editor.selected_ids.clear();
                            self.file_message = Some((format!("已匯入 {} 個物件: {}", count, ps), std::time::Instant::now()));
                        }
                        Err(e) => self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now())),
                    }
                }
            }
            MenuAction::ImportDxfSmart => {
                let file = rfd::FileDialog::new()
                    .set_title("智慧匯入 (DXF/DWG/PDF)")
                    .add_filter("CAD 圖面", &["dxf", "DXF", "dwg", "DWG", "pdf", "PDF"])
                    .add_filter("所有檔案", &["*"])
                    .pick_file();
                if let Some(path) = file {
                    let ps = path.to_string_lossy().to_string();
                    let ext = ps.rsplit('.').next().unwrap_or("").to_lowercase();
                    self.console_push("INFO", format!("[CAD] 開始解析: {} ({})", ps, ext));

                    if ext == "dxf" {
                        // DXF: full entity parsing
                        match crate::cad_import::import_dxf_to_ir(&ps) {
                            Ok(ir) => {
                                // Push full debug report to console
                                for line in &ir.debug_report {
                                    self.console_push("INFO", line.clone());
                                }
                                self.viewer.show_console = true;
                                self.console_push("INFO", format!("[DXF] Grids: X={} Y={} | Columns: {} | Beams: {} | Levels: {}",
                                    ir.grids.x_grids.len(), ir.grids.y_grids.len(),
                                    ir.columns.len(), ir.beams.len(), ir.levels.len()));
                                // Show review panel for user confirmation instead of auto-building
                                let entity_count = ir.columns.len() + ir.beams.len() + ir.base_plates.len();
                                let debug = ir.debug_report.clone();
                                self.import_review = Some(crate::import_review::ImportReview::from_drawing_ir(
                                    &ir, &ps, entity_count, debug,
                                ));
                                self.file_message = Some(("解析完成 — 請確認偵測結果".into(), std::time::Instant::now()));
                            }
                            Err(e) => {
                                self.console_push("ERROR", format!("[DXF] Parse failed: {}", e));
                                self.file_message = Some((format!("Parse failed: {}", e), std::time::Instant::now()));
                            }
                        }
                    } else {
                        // DWG/PDF: use smart import pipeline (unified IR)
                        self.console_push("INFO", format!("[CAD] 使用統一匯入管線"));
                        match crate::import::import_manager::import_file(&ps) {
                            Ok(ir) => {
                                for line in &ir.debug_report {
                                    let level = if line.contains("❌") { "WARN" } else { "INFO" };
                                    self.console_push(level, line.clone());
                                }
                                self.viewer.show_console = true;
                                self.pending_unified_ir = Some(ir);
                            }
                            Err(e) => {
                                self.console_push("ERROR", format!("[CAD] 匯入失敗: {}", e));
                                self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now()));
                            }
                        }
                    }
                }
            }
            MenuAction::SmartImport => {
                let file = rfd::FileDialog::new()
                    .set_title("智慧匯入")
                    .add_filter("所有支援格式", &["dxf", "DXF", "dwg", "DWG", "skp", "SKP", "obj", "OBJ", "stl", "STL", "pdf", "PDF"])
                    .add_filter("DXF 圖面", &["dxf", "DXF"])
                    .add_filter("DWG 圖面", &["dwg"])
                    .add_filter("PDF 圖面", &["pdf"])
                    .add_filter("SketchUp 模型", &["skp"])
                    .add_filter("OBJ 模型", &["obj"])
                    .pick_file();
                if let Some(path) = file {
                    let ps = path.to_string_lossy().to_string();
                    self.console_push("INFO", format!("[Import] 開始匯入: {}", ps));
                    match crate::import::import_manager::import_file(&ps) {
                        Ok(ir) => {
                            // Push structured debug report to console
                            for line in &ir.debug_report {
                                let level = if line.contains("❌") || line.contains("ERROR") { "WARN" }
                                    else if line.contains("⚠") { "WARN" }
                                    else { "INFO" };
                                self.console_push(level, line.clone());
                            }
                            if ir.debug_report.is_empty() {
                                self.console_push("INFO", format!("[Import] 格式: {} | 頂點: {} | 面: {} | 網格: {} | 構件: {} | 材質: {}",
                                    ir.source_format.to_uppercase(),
                                    ir.stats.vertex_count, ir.stats.face_count,
                                    ir.stats.mesh_count, ir.stats.member_count, ir.stats.material_count));
                            }
                            let summary = format!(
                                "匯入解析完成 ({})\n\n頂點: {}\n面: {}\n網格: {}\n群組: {}\n構件: {}\n材質: {}",
                                ir.source_format.to_uppercase(),
                                ir.stats.vertex_count, ir.stats.face_count,
                                ir.stats.mesh_count, ir.stats.group_count,
                                ir.stats.member_count, ir.stats.material_count,
                            );
                            self.viewer.show_console = true; // auto-open console on import
                            self.pending_unified_ir = Some(ir);
                            self.file_message = Some((summary, std::time::Instant::now()));
                        }
                        Err(e) => {
                            self.console_push("ERROR", format!("[Import] 匯入失敗: {}", e));
                            self.file_message = Some((format!("匯入失敗:\n{}", e), std::time::Instant::now()));
                        }
                    }
                }
            }
            MenuAction::SplitObject => {
                if let Some(id) = self.editor.selected_ids.first().cloned() {
                    if let Some(obj) = self.scene.objects.get(&id) {
                        if let Shape::Box { width, height, depth } = &obj.shape {
                            let p = obj.position;
                            let (w, h, d) = (*width, *height, *depth);
                            // Split along the longest axis at midpoint
                            let (axis, split_pos) = if w >= h && w >= d {
                                (0u8, p[0] + w / 2.0)
                            } else if h >= d {
                                (1u8, p[1] + h / 2.0)
                            } else {
                                (2u8, p[2] + d / 2.0)
                            };
                            if let Some((a, b)) = self.scene.split_box(&id, axis, split_pos) {
                                self.editor.selected_ids = vec![a, b];
                                self.file_message = Some(("物件已分割".to_string(), std::time::Instant::now()));
                            }
                        }
                    }
                }
            }
            // Camera/view actions handled in app.rs update() before dispatch
            _ => {}
        }
    }

    /// When a line segment is drawn across a Box face, try to split the box
    /// into two boxes along the cut line. Detects which face the line lies on
    /// and determines the split axis. Handles axis-aligned cuts on all 6 faces.
    fn try_split_face(&mut self, p1: [f32; 3], p2: [f32; 3]) {
        let face_tol = 10.0_f32; // tolerance for "on the face" (mm)
        let margin = 50.0_f32;   // line must be away from edges to trigger split

        // Collect candidate (obj_id, axis, split_pos) to avoid borrow issues
        let mut split_info: Option<(String, u8, f32)> = None;

        for (id, obj) in &self.scene.objects {
            if let Shape::Box { width, height, depth } = &obj.shape {
                let pos = obj.position;
                let max = [pos[0] + width, pos[1] + height, pos[2] + depth];

                // Both endpoints must be within the box bounding volume (with tolerance)
                let in_x = p1[0] >= pos[0] - face_tol && p1[0] <= max[0] + face_tol
                         && p2[0] >= pos[0] - face_tol && p2[0] <= max[0] + face_tol;
                let in_y = p1[1] >= pos[1] - face_tol && p1[1] <= max[1] + face_tol
                         && p2[1] >= pos[1] - face_tol && p2[1] <= max[1] + face_tol;
                let in_z = p1[2] >= pos[2] - face_tol && p1[2] <= max[2] + face_tol
                         && p2[2] >= pos[2] - face_tol && p2[2] <= max[2] + face_tol;
                if !in_x || !in_y || !in_z { continue; }

                // Check which face both endpoints lie on
                let on_front = (p1[2] - pos[2]).abs() < face_tol && (p2[2] - pos[2]).abs() < face_tol;
                let on_back  = (p1[2] - max[2]).abs() < face_tol && (p2[2] - max[2]).abs() < face_tol;
                let on_left  = (p1[0] - pos[0]).abs() < face_tol && (p2[0] - pos[0]).abs() < face_tol;
                let on_right = (p1[0] - max[0]).abs() < face_tol && (p2[0] - max[0]).abs() < face_tol;
                let on_top   = (p1[1] - max[1]).abs() < face_tol && (p2[1] - max[1]).abs() < face_tol;
                let on_bot   = (p1[1] - pos[1]).abs() < face_tol && (p2[1] - pos[1]).abs() < face_tol;

                if !(on_front || on_back || on_left || on_right || on_top || on_bot) { continue; }

                let mid = [
                    (p1[0] + p2[0]) * 0.5,
                    (p1[1] + p2[1]) * 0.5,
                    (p1[2] + p2[2]) * 0.5,
                ];

                // On front/back face (XY plane): horizontal line splits Y, vertical splits X
                if on_front || on_back {
                    if (p1[1] - p2[1]).abs() < face_tol && mid[1] > pos[1] + margin && mid[1] < max[1] - margin {
                        // Horizontal line on front/back → split height (Y axis = 1)
                        split_info = Some((id.clone(), 1, mid[1]));
                        break;
                    }
                    if (p1[0] - p2[0]).abs() < face_tol && mid[0] > pos[0] + margin && mid[0] < max[0] - margin {
                        // Vertical line on front/back → split width (X axis = 0)
                        split_info = Some((id.clone(), 0, mid[0]));
                        break;
                    }
                }

                // On left/right face (YZ plane): horizontal line splits Y, vertical splits Z
                if on_left || on_right {
                    if (p1[1] - p2[1]).abs() < face_tol && mid[1] > pos[1] + margin && mid[1] < max[1] - margin {
                        split_info = Some((id.clone(), 1, mid[1]));
                        break;
                    }
                    if (p1[2] - p2[2]).abs() < face_tol && mid[2] > pos[2] + margin && mid[2] < max[2] - margin {
                        split_info = Some((id.clone(), 2, mid[2]));
                        break;
                    }
                }

                // On top/bottom face (XZ plane): line along X splits Z, line along Z splits X
                if on_top || on_bot {
                    if (p1[2] - p2[2]).abs() < face_tol && mid[0] > pos[0] + margin && mid[0] < max[0] - margin {
                        split_info = Some((id.clone(), 0, mid[0]));
                        break;
                    }
                    if (p1[0] - p2[0]).abs() < face_tol && mid[2] > pos[2] + margin && mid[2] < max[2] - margin {
                        split_info = Some((id.clone(), 2, mid[2]));
                        break;
                    }
                }
            }
        }

        if let Some((obj_id, axis, split_pos)) = split_info {
            if let Some((a, b)) = self.scene.split_box(&obj_id, axis, split_pos) {
                self.editor.selected_ids = vec![a, b];
                self.file_message = Some(("\u{9762}\u{5df2}\u{88ab}\u{7dda}\u{6bb5}\u{5207}\u{5272}".to_string(), std::time::Instant::now()));
            }
        }
    }

    /// Check if a drawn line segment crosses any Box face, and split if so.
    /// Unlike try_split_face (which requires endpoints ON a face), this detects
    /// lines that pass *through* a box's footprint in the XZ or Y planes.
    fn try_split_face_on_line(&mut self, p1: [f32; 3], p2: [f32; 3]) {
        let dx = (p2[0] - p1[0]).abs();
        let dz = (p2[2] - p1[2]).abs();

        let ids: Vec<String> = self.scene.objects.keys().cloned().collect();

        for id in &ids {
            let obj = match self.scene.objects.get(id) {
                Some(o) => o.clone(),
                None => continue,
            };

            let (w, h, d) = match &obj.shape {
                Shape::Box { width, height, depth } => (*width, *height, *depth),
                _ => continue,
            };
            let p = obj.position;

            let box_min_x = p[0];
            let box_max_x = p[0] + w;
            let box_min_z = p[2];
            let box_max_z = p[2] + d;

            // Line goes through the box in X direction (cut along Z)
            if dx > dz * 3.0 {
                let line_z = (p1[2] + p2[2]) / 2.0;
                let line_x_min = p1[0].min(p2[0]);
                let line_x_max = p1[0].max(p2[0]);

                if line_z > box_min_z + 10.0 && line_z < box_max_z - 10.0
                    && line_x_min < box_max_x && line_x_max > box_min_x
                {
                    self.scene.split_box(id, 2, line_z);
                    self.file_message = Some(("線段切割：物件已沿 Z 軸分割".into(), std::time::Instant::now()));
                    return;
                }
            }

            // Line goes through the box in Z direction (cut along X)
            if dz > dx * 3.0 {
                let line_x = (p1[0] + p2[0]) / 2.0;
                let line_z_min = p1[2].min(p2[2]);
                let line_z_max = p1[2].max(p2[2]);

                if line_x > box_min_x + 10.0 && line_x < box_max_x - 10.0
                    && line_z_min < box_max_z && line_z_max > box_min_z
                {
                    self.scene.split_box(id, 0, line_x);
                    self.file_message = Some(("線段切割：物件已沿 X 軸分割".into(), std::time::Instant::now()));
                    return;
                }
            }

            // Handle Y-direction cuts (vertical lines on a wall face)
            if p1[1].abs() > 10.0 || p2[1].abs() > 10.0 {
                let line_y = (p1[1] + p2[1]) / 2.0;
                if line_y > p[1] + 10.0 && line_y < p[1] + h - 10.0 {
                    let mx = (p1[0] + p2[0]) / 2.0;
                    let mz = (p1[2] + p2[2]) / 2.0;
                    if mx >= box_min_x - 100.0 && mx <= box_max_x + 100.0
                        && mz >= box_min_z - 100.0 && mz <= box_max_z + 100.0
                    {
                        self.scene.split_box(id, 1, line_y);
                        self.file_message = Some(("線段切割：物件已沿 Y 軸分割".into(), std::time::Instant::now()));
                        return;
                    }
                }
            }
        }
    }

    /// Extrude a profile object along a path, creating stretched copies at each segment.
    fn extrude_along_path(&mut self, profile_id: &str, path: &[[f32; 3]]) {
        let profile = match self.scene.objects.get(profile_id).cloned() {
            Some(o) => o,
            None => return,
        };

        if path.len() < 2 { return; }

        self.scene.snapshot();
        let mut created_ids = Vec::new();

        let (_pw, _ph, pd) = match &profile.shape {
            Shape::Box { width, height, depth } => (*width, *height, *depth),
            _ => {
                // For non-box shapes, fall back to simple copy placement
                for (i, point) in path.iter().enumerate() {
                    if i == 0 { continue; }
                    let delta = [
                        point[0] - path[0][0],
                        point[1] - path[0][1],
                        point[2] - path[0][2],
                    ];
                    let new_pos = [
                        profile.position[0] + delta[0],
                        profile.position[1] + delta[1],
                        profile.position[2] + delta[2],
                    ];
                    match &profile.shape {
                        Shape::Cylinder { radius, height, segments } => {
                            let nid = self.scene.add_cylinder(
                                format!("{}_{}", profile.name, i),
                                new_pos, *radius, *height, *segments, profile.material,
                            );
                            created_ids.push(nid);
                        }
                        Shape::Sphere { radius, segments } => {
                            let nid = self.scene.add_sphere(
                                format!("{}_{}", profile.name, i),
                                new_pos, *radius, *segments, profile.material,
                            );
                            created_ids.push(nid);
                        }
                        _ => {}
                    }
                }
                if !created_ids.is_empty() {
                    self.scene.version += 1;
                    self.editor.selected_ids = created_ids.clone();
                    self.file_message = Some((
                        format!("沿路徑擠出 {} 段", created_ids.len()),
                        std::time::Instant::now(),
                    ));
                    self.editor.tool = Tool::Select;
                }
                return;
            }
        };

        // For Box profiles: create stretched boxes along each path segment
        for (i, point) in path.iter().enumerate() {
            if i == 0 { continue; }

            let prev = path[i - 1];
            let dir_x = point[0] - prev[0];
            let dir_z = point[2] - prev[2];
            let segment_len = (dir_x * dir_x + dir_z * dir_z).sqrt();

            if segment_len < 1.0 { continue; }

            let angle = dir_z.atan2(dir_x);

            // Place stretched box at midpoint of segment
            let mid_x = (prev[0] + point[0]) / 2.0 - segment_len / 2.0;
            let mid_z = (prev[2] + point[2]) / 2.0 - pd / 2.0;

            let mut clone = profile.clone();
            clone.id = self.scene.next_id_pub();
            clone.name = format!("{}_{}", profile.name, i);
            clone.position = [mid_x, profile.position[1], mid_z];
            clone.rotation_y = angle;

            // Stretch width to fill the segment length
            if let Shape::Box { ref mut width, .. } = clone.shape {
                *width = segment_len;
            }

            let cid = clone.id.clone();
            self.scene.objects.insert(cid.clone(), clone);
            created_ids.push(cid);
        }

        // Also add path visualization lines
        for i in 0..path.len() - 1 {
            self.scene.add_line(
                format!("path_{}", i),
                vec![path[i], path[i + 1]], 5.0, profile.material,
            );
        }

        self.scene.version += 1;
        self.editor.selected_ids = created_ids.clone();
        self.file_message = Some((
            format!("沿路徑擠出 {} 段", created_ids.len()),
            std::time::Instant::now(),
        ));
        self.editor.tool = Tool::Select;
    }

    /// Expand selection to include all group members for any selected object
    pub(crate) fn expand_selection_to_groups(&mut self) {
        let mut expanded = self.editor.selected_ids.clone();
        for id in &self.editor.selected_ids {
            for g in self.scene.groups.values() {
                if g.children.contains(id) {
                    for child in &g.children {
                        if !expanded.contains(child) {
                            expanded.push(child.clone());
                        }
                    }
                }
            }
        }
        self.editor.selected_ids = expanded;
    }
}

/// Parse H-section profile string like "H300x150x6x9" -> (H, B, tw, tf) in mm
fn parse_h_profile(profile: &str) -> (f32, f32, f32, f32) {
    let parts: Vec<f32> = profile
        .replace("H", "").replace("h", "")
        .split('x')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    match parts.len() {
        4 => (parts[0], parts[1], parts[2], parts[3]),
        3 => (parts[0], parts[1], parts[2], parts[2]), // assume tf = tw
        2 => (parts[0], parts[1], 8.0, 12.0),          // defaults
        1 => (parts[0], parts[0] * 0.5, 8.0, 12.0),
        _ => (300.0, 150.0, 6.0, 9.0),                 // H300x150x6x9 default
    }
}

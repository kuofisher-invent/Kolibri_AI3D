use eframe::egui;

use crate::app::{
    compute_arc, DrawState, KolibriApp, PullFace, RenderMode, RightTab, ScaleHandle, Tool,
};
use crate::camera;
use crate::scene::{MaterialKind, Shape};

impl KolibriApp {
    // ── Viewport interaction ────────────────────────────────────────────────

    pub(crate) fn handle_viewport(&mut self, response: &egui::Response, ui: &egui::Ui) {
        let shift = ui.input(|i| i.modifiers.shift);
        self.shift_held = shift;

        // Track mouse position on ground
        if let Some(hp) = response.hover_pos() {
            let local = hp - response.rect.min;
            self.mouse_screen = [local.x, local.y];
            self.viewport_size = [response.rect.width(), response.rect.height()];
            let (origin, dir) = self.camera.screen_ray(local.x, local.y, response.rect.width(), response.rect.height());
            self.mouse_ground = camera::ray_ground(origin, dir).map(|p| [p.x, p.y, p.z]);

            // Shift-lock axis (SketchUp-style: hold Shift to lock detected axis)
            // Only when actively drawing or moving, not when idle (Shift+Middle = Pan)
            let in_active_state = !matches!(self.draw_state, DrawState::Idle)
                || (matches!(self.tool, Tool::Move) && !self.selected_ids.is_empty() && response.dragged());
            if shift && in_active_state {
                if let Some(ref snap) = self.snap_result {
                    match snap.snap_type {
                        crate::app::SnapType::AxisX => self.locked_axis = Some(0),
                        crate::app::SnapType::AxisZ => self.locked_axis = Some(2),
                        _ => {} // keep current lock if any
                    }
                }
            } else if !shift && !ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd) {
                // Release Shift-lock when Shift is released (unless Ctrl-locked)
                // Only clear if it was a Shift-lock (not a Ctrl-cycle lock)
                if self.locked_axis.is_some() && !self.ctrl_was_down {
                    self.locked_axis = None;
                }
            }

            // Compute smart snap
            // For Line/Arc tools, try face snap first
            let is_draw_tool = matches!(self.tool, Tool::Line | Tool::Arc);
            if is_draw_tool {
                if let Some(face_pt) = self.snap_to_face(local.x, local.y, response.rect.width(), response.rect.height()) {
                    self.mouse_ground = Some(face_pt);
                    self.snap_result = Some(crate::app::SnapResult {
                        position: face_pt,
                        snap_type: crate::app::SnapType::OnFace,
                        from_point: self.get_drawing_origin(),
                    });
                } else if let Some(raw) = self.mouse_ground {
                    let from = self.get_drawing_origin();
                    let result = self.smart_snap(raw, from);
                    self.mouse_ground = Some(result.position);
                    self.snap_result = Some(result);
                } else {
                    self.snap_result = None;
                }
            } else {
                // Run snap for ALL tools (not just drawing) — use ground point or fallback
                let raw = self.mouse_ground.unwrap_or([0.0, 0.0, 0.0]);
                let from = self.get_drawing_origin();
                let result = self.smart_snap(raw, from);
                if result.snap_type != crate::app::SnapType::None {
                    self.mouse_ground = Some(result.position);
                }
                self.snap_result = Some(result);
            }

            // Hover pick — highlight objects/faces for all interactive tools
            let interactive = matches!(self.tool,
                Tool::Select | Tool::PushPull | Tool::Move | Tool::Rotate |
                Tool::Scale | Tool::Eraser | Tool::PaintBucket | Tool::Offset |
                Tool::FollowMe
            );
            if interactive && matches!(self.draw_state, DrawState::Idle) {
                self.hovered_id = self.pick(local.x, local.y, response.rect.width(), response.rect.height());
                self.hovered_face = self.pick_face(local.x, local.y, response.rect.width(), response.rect.height());
            } else {
                self.hovered_id = None;
                self.hovered_face = None;
            }
        }

        // Camera controls — SketchUp-style:
        // Middle drag = Orbit, Shift+Middle drag = Pan
        if response.dragged_by(egui::PointerButton::Middle) {
            let d = response.drag_delta();
            if shift {
                self.camera.pan(d.x, d.y);
            } else {
                self.camera.orbit(d.x, d.y);
            }
        }
        // Middle click (no drag) = center view on cursor point
        if response.clicked_by(egui::PointerButton::Middle) {
            if let Some(ground) = self.mouse_ground {
                self.camera.target = glam::Vec3::new(ground[0], ground[1], ground[2]);
                self.file_message = Some(("視角已居中".to_string(), std::time::Instant::now()));
            }
        }
        // Right-drag no longer orbits (right-click is for context menu)
        if response.dragged_by(egui::PointerButton::Primary) && matches!(self.draw_state, DrawState::Idle) {
            let d = response.drag_delta();
            match self.tool {
                Tool::Orbit => self.camera.orbit(d.x, d.y),
                Tool::Pan => self.camera.pan(d.x, d.y),
                Tool::Select => {
                    if shift {
                        self.camera.pan(d.x, d.y);
                    } else if self.rubber_band.is_some() {
                        // Continue rubber band drag (don't break on hover)
                        if let Some(hp) = response.hover_pos() {
                            if let Some((_, ref mut end)) = self.rubber_band {
                                *end = hp;
                            }
                        }
                    } else if self.hovered_id.is_none() {
                        self.camera.orbit(d.x, d.y);
                    }
                }
                Tool::Move => {
                    // Move selected objects by dragging
                    // Ctrl held at drag START: duplicate objects first, then move clones
                    if !self.selected_ids.is_empty() {
                        if !self.drag_snapshot_taken {
                            self.scene.snapshot();
                            // If Ctrl held at drag start, duplicate objects first
                            let ctrl_at_start = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
                            if ctrl_at_start {
                                let mut new_ids = Vec::new();
                                for id in &self.selected_ids.clone() {
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
                                    self.selected_ids = new_ids;
                                    self.move_is_copy = true;
                                }
                            }
                            self.drag_snapshot_taken = true;
                        }

                        // Detect Ctrl press EDGE to cycle axis lock (only when not in copy mode)
                        let ctrl_now = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
                        if ctrl_now && !self.ctrl_was_down {
                            // Ctrl just pressed → cycle: None → X(red) → Y(green) → Z(blue) → None
                            self.locked_axis = match self.locked_axis {
                                None => Some(0),
                                Some(0) => Some(1),
                                Some(1) => Some(2),
                                Some(2) => None,
                                Some(_) => None,
                            };
                        }
                        self.ctrl_was_down = ctrl_now;

                        let scale = self.camera.distance * 0.001;
                        let (sy, cy) = self.camera.yaw.sin_cos();
                        let right = glam::Vec3::new(-sy, 0.0, cy);
                        let fwd = glam::Vec3::new(-cy, 0.0, -sy);
                        let raw_delta = right * (-d.x) * scale + fwd * (d.y * scale);
                        let vert_delta = -d.y * scale;

                        // Apply movement based on locked axis
                        let (dx, dy, dz) = match self.locked_axis {
                            Some(0) => (raw_delta.x, 0.0, 0.0),              // X only (red)
                            Some(1) => (0.0, vert_delta, 0.0),                // Y only (green)
                            Some(2) => (0.0, 0.0, raw_delta.z),              // Z only (blue)
                            _ => (raw_delta.x, 0.0, raw_delta.z),             // Free XZ
                        };

                        let ids = self.selected_ids.clone();
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
                                    self.collision_warning = Some(format!("碰撞: {}", note));
                                } else if !report.warning_pairs.is_empty() {
                                    let note = report.warning_pairs.first()
                                        .and_then(|p| p.note.as_deref())
                                        .unwrap_or("Contact detected");
                                    self.collision_warning = Some(format!("警告: {}", note));
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
                        self.camera.orbit(d.x, d.y);
                    }
                }
                Tool::Eraser => {
                    // Drag over objects to continuously delete them
                    if let Some(id) = self.hovered_id.clone() {
                        if !self.drag_snapshot_taken {
                            self.scene.snapshot();
                            self.drag_snapshot_taken = true;
                        }
                        self.scene.objects.remove(&id);
                        self.scene.version += 1;
                        self.selected_ids.retain(|s| s != &id);
                        self.hovered_id = None;
                    }
                }
                _ => {} // Drawing tools handle clicks, not drags
            }
        }
        // Reset drag snapshot flag when not dragging
        if !response.dragged() {
            self.drag_snapshot_taken = false;
            self.move_is_copy = false;
        }

        // Rubber band selection: start on drag start in Select mode when nothing hovered
        if response.drag_started_by(egui::PointerButton::Primary)
            && matches!(self.tool, Tool::Select)
            && matches!(self.draw_state, DrawState::Idle)
            && !shift
            && self.hovered_id.is_none()
        {
            if let Some(hp) = response.interact_pointer_pos() {
                self.rubber_band = Some((hp, hp));
            }
        }

        // Rubber band selection: finish on drag stop
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            if let Some((start, end)) = self.rubber_band.take() {
                let rect = egui::Rect::from_two_pos(start, end);
                if rect.width() > 3.0 || rect.height() > 3.0 {
                    // Find all objects whose projected AABB overlaps the rubber band rect
                    let viewport_rect = response.rect;
                    let mut selected = if shift { self.selected_ids.clone() } else { Vec::new() };
                    for obj in self.scene.objects.values() {
                        let p = obj.position;
                        let (min_p, max_p) = match &obj.shape {
                            Shape::Box { width, height, depth } =>
                                (p, [p[0] + width, p[1] + height, p[2] + depth]),
                            Shape::Cylinder { radius, height, .. } =>
                                (p, [p[0] + radius * 2.0, p[1] + height, p[2] + radius * 2.0]),
                            Shape::Sphere { radius, .. } =>
                                (p, [p[0] + radius * 2.0, p[1] + radius * 2.0, p[2] + radius * 2.0]),
                            Shape::Line { points, thickness } => {
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
                        // Project 8 AABB corners to screen, check if any is inside rubber band
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
                        let any_inside = corners.iter().any(|c| {
                            if let Some(sp) = self.world_to_screen(*c, &viewport_rect) {
                                rect.contains(sp)
                            } else {
                                false
                            }
                        });
                        if any_inside && !selected.contains(&obj.id) {
                            selected.push(obj.id.clone());
                        }
                    }
                    self.selected_ids = selected;
                    if !self.selected_ids.is_empty() {
                        self.right_tab = RightTab::Properties;
                    }
                }
            }
        }

        // Zoom toward cursor position (SketchUp-style)
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                let world_point = self.mouse_ground.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
                self.camera.zoom_toward(scroll, world_point);
            }
        }

        // Scale drag with axis-constrained handles
        if let DrawState::Scaling { ref obj_id, handle, original_dims: _ } = self.draw_state.clone() {
            if response.dragged_by(egui::PointerButton::Primary) {
                if !self.drag_snapshot_taken {
                    self.scene.snapshot();
                    self.drag_snapshot_taken = true;
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
                self.draw_state = DrawState::Idle;
                self.drag_snapshot_taken = false;
            }
        }

        // D1: Rotate drag handler — interactive rotation with protractor
        if let DrawState::Rotating { ref obj_id, center, start_angle: _, ref mut accumulated } = self.draw_state.clone() {
            if response.dragged_by(egui::PointerButton::Primary) {
                if !self.drag_snapshot_taken {
                    self.scene.snapshot();
                    self.drag_snapshot_taken = true;
                }
                // Horizontal drag = rotation
                let delta_angle = response.drag_delta().x * 0.005;
                let obj_id = obj_id.clone();
                let new_accumulated = *accumulated + delta_angle;

                // Snap to 15-degree increments when within 3 degrees
                let mut new_rotation = {
                    if let Some(obj) = self.scene.objects.get(&obj_id) {
                        obj.rotation_y + delta_angle
                    } else {
                        0.0
                    }
                };
                let snap_angle = 15.0_f32.to_radians();
                let snapped = (new_rotation / snap_angle).round() * snap_angle;
                if (new_rotation - snapped).abs() < 3.0_f32.to_radians() {
                    new_rotation = snapped;
                }
                if let Some(obj) = self.scene.objects.get_mut(&obj_id) {
                    obj.rotation_y = new_rotation;
                }
                // Update draw state with new accumulated
                self.draw_state = DrawState::Rotating {
                    obj_id: obj_id.clone(), center, start_angle: 0.0,
                    accumulated: new_accumulated,
                };
            }
            if response.drag_stopped() {
                self.draw_state = DrawState::Idle;
                self.drag_snapshot_taken = false;
            }
        }

        // Offset drag — face edge inset with live preview
        if let DrawState::Offsetting { ref obj_id, face, distance: _ } = self.draw_state.clone() {
            if response.dragged_by(egui::PointerButton::Primary) {
                if !self.drag_snapshot_taken {
                    self.scene.snapshot();
                    self.drag_snapshot_taken = true;
                }
                let scale = self.camera.distance * 0.001;
                let delta = response.drag_delta().x * scale;
                let cur_d = match &self.draw_state {
                    DrawState::Offsetting { distance, .. } => *distance,
                    _ => 0.0,
                };
                let new_d = (cur_d + delta).abs().max(10.0);
                self.draw_state = DrawState::Offsetting { obj_id: obj_id.clone(), face, distance: new_d };
            }
            if response.drag_stopped() {
                let d = match &self.draw_state {
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
                            self.selected_ids = vec![new_id];
                            self.file_message = Some((format!("偏移 {:.0}mm — 可用推拉工具拉伸", d), std::time::Instant::now()));
                        }
                    }
                }
                self.draw_state = DrawState::Idle;
                self.drag_snapshot_taken = false;
            }
        }

        // Push/Pull drag — only when a face is click-selected (selected_face)
        if self.tool == Tool::PushPull {
            if let Some((ref obj_id, face)) = self.selected_face.clone() {
                if response.dragged_by(egui::PointerButton::Primary) {
                    if !self.drag_snapshot_taken {
                        self.scene.snapshot();
                        self.last_pull_distance = 0.0; // reset accumulator at drag start
                        // C3: Save original position & dims for dashed reference lines
                        if let Some(obj) = self.scene.objects.get(&*obj_id) {
                            self.pull_original_pos = Some(obj.position);
                            self.pull_original_dims = match &obj.shape {
                                Shape::Box { width, height, depth } => Some([*width, *height, *depth]),
                                Shape::Cylinder { radius, height, .. } => Some([*radius * 2.0, *height, *radius * 2.0]),
                                _ => None,
                            };
                        }
                        self.drag_snapshot_taken = true;
                    }
                    let d = response.drag_delta();
                    let scale = self.camera.distance * 0.0015;

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

                    let vp = self.camera.view_proj(self.viewport_size[0] / self.viewport_size[1].max(1.0));

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

                            self.last_pull_distance += amount;

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
                                    self.collision_warning = Some("推拉導致碰撞".to_string());
                                }
                            }
                        }
                    }
                }
                if response.drag_stopped() {
                    // Store pull face for double-click repeat (A4)
                    // last_pull_distance was accumulated during drag
                    self.last_pull_face = Some((obj_id.clone(), face));
                    self.last_pull_click_time = std::time::Instant::now();
                    self.drag_snapshot_taken = false;
                    // C3: clear original pos/dims
                    self.pull_original_pos = None;
                    self.pull_original_dims = None;
                    self.ai_log.log(&self.current_actor.clone(), "\u{63a8}\u{62c9}\u{9762}", &format!("{:?} {:.0}mm", face, self.last_pull_distance), vec![obj_id.clone()]);
                    // Face stays selected after drag — user can pull again or click to deselect
                }
            }
        }

        // Push/Pull drag on free mesh face
        if let DrawState::PullingFreeMesh { face_id } = self.draw_state {
            if response.dragged_by(egui::PointerButton::Primary) {
                if !self.drag_snapshot_taken {
                    self.scene.snapshot();
                    self.drag_snapshot_taken = true;
                }
                let drag = response.drag_delta();
                let scale = self.camera.distance * 0.002;
                // Use vertical drag mapped to face normal direction
                let amount = -drag.y * scale;
                if amount.abs() > 0.1 {
                    self.scene.free_mesh.push_pull_face(face_id, amount);
                    self.scene.version += 1;
                }
            }
            if response.drag_stopped() || response.clicked() {
                self.draw_state = DrawState::Idle;
                self.drag_snapshot_taken = false;
                self.file_message = Some((
                    "\u{9762}\u{5df2}\u{63a8}\u{62c9}\u{5b8c}\u{6210}".to_string(),
                    std::time::Instant::now(),
                ));
            }
        }

        // Double-click: enter group isolation mode
        if response.double_clicked() {
            let (mx, my) = (self.mouse_screen[0], self.mouse_screen[1]);
            let (vw, vh) = (self.viewport_size[0], self.viewport_size[1]);
            if let Some(id) = self.pick(mx, my, vw, vh) {
                let is_group = self.scene.objects.get(&id)
                    .map(|o| o.name.contains("[群組]") || o.name.contains("[元件]"))
                    .unwrap_or(false);
                if is_group {
                    self.editing_group_id = Some(id.clone());
                    self.selected_ids = vec![id];
                } else {
                    self.editing_group_id = None;
                }
            }
        }

        // Click
        if response.clicked() {
            self.on_click();
        }

        // Right-click context menu
        response.context_menu(|ui| {
            let has_sel = !self.selected_ids.is_empty();
            let action = crate::menu::draw_context_menu(ui, has_sel);
            self.handle_menu_action(action);
        });

        // Keyboard
        if response.has_focus() || response.hovered() {
            ui.input(|i| {
                if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                    let ids = std::mem::take(&mut self.selected_ids);
                    for id in &ids {
                        self.ai_log.log(&self.current_actor.clone(), "\u{522a}\u{9664}\u{7269}\u{4ef6}", id, vec![id.clone()]);
                        self.scene.delete(id);
                    }
                }
                if i.key_pressed(egui::Key::Escape) {
                    // FollowPath: ESC finishes the path and creates extrusion
                    if let DrawState::FollowPath { ref source_id, ref path_points } = self.draw_state.clone() {
                        if path_points.len() >= 2 {
                            self.scene.snapshot();
                            if let Some(source) = self.scene.objects.get(&source_id.clone()).cloned() {
                                let mut new_ids = Vec::new();
                                for idx in 1..path_points.len() {
                                    let delta = [
                                        path_points[idx][0] - path_points[0][0],
                                        path_points[idx][1] - path_points[0][1],
                                        path_points[idx][2] - path_points[0][2],
                                    ];
                                    let new_pos = [
                                        source.position[0] + delta[0],
                                        source.position[1] + delta[1],
                                        source.position[2] + delta[2],
                                    ];
                                    match &source.shape {
                                        Shape::Box { width, height, depth } => {
                                            let nid = self.scene.add_box(
                                                format!("{}_{}", source.name, idx),
                                                new_pos, *width, *height, *depth, source.material,
                                            );
                                            new_ids.push(nid);
                                        }
                                        Shape::Cylinder { radius, height, segments } => {
                                            let nid = self.scene.add_cylinder(
                                                format!("{}_{}", source.name, idx),
                                                new_pos, *radius, *height, *segments, source.material,
                                            );
                                            new_ids.push(nid);
                                        }
                                        Shape::Sphere { radius, segments } => {
                                            let nid = self.scene.add_sphere(
                                                format!("{}_{}", source.name, idx),
                                                new_pos, *radius, *segments, source.material,
                                            );
                                            new_ids.push(nid);
                                        }
                                        _ => {}
                                    }
                                }
                                // Connect path with thin line objects
                                for idx in 0..path_points.len() - 1 {
                                    let p1 = path_points[idx];
                                    let p2 = path_points[idx + 1];
                                    self.scene.add_line(
                                        format!("path_{}", idx),
                                        vec![p1, p2], 5.0, source.material,
                                    );
                                }
                                self.selected_ids = new_ids;
                                self.file_message = Some((
                                    format!("沿路徑擠出 {} 個副本", path_points.len() - 1),
                                    std::time::Instant::now(),
                                ));
                            }
                        }
                        self.draw_state = DrawState::Idle;
                    } else {
                        self.tool = Tool::Select;
                        self.draw_state = DrawState::Idle;
                        self.selected_ids.clear();
                        self.selected_face = None;
                        self.locked_axis = None;
                        self.sticky_axis = None;
                        self.editing_group_id = None;
                        self.suggestion = None;
                        // Inference 2.0: reset context on ESC
                        crate::inference::reset_context(&mut self.inference_ctx);
                        self.inference_ctx.current_tool = Tool::Select;
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
                    self.selected_ids = self.scene.objects.keys().cloned().collect();
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
                else if self.suggestion.is_some() {
                    if i.key_pressed(egui::Key::Y) {
                        if let Some(suggestion) = self.suggestion.take() {
                            self.apply_suggestion(suggestion.action);
                        }
                    }
                    if i.key_pressed(egui::Key::N) {
                        self.suggestion = None;
                    }
                }

                // Shortcut keys (SketchUp-style) — only when Ctrl is NOT held
                let set = |tool: Tool, this: &mut Self| { this.tool = tool; this.draw_state = DrawState::Idle; };
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

                    // Standard view shortcuts
                    if i.key_pressed(egui::Key::Num1) { self.camera.set_front(); }
                    if i.key_pressed(egui::Key::Num2) { self.camera.set_top(); }
                    if i.key_pressed(egui::Key::Num3) { self.camera.set_iso(); }

                    // Axis locking
                    if i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowRight) {
                        self.locked_axis = if self.locked_axis == Some(0) { None } else { Some(0) };
                    }
                    if i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowDown) {
                        self.locked_axis = if self.locked_axis == Some(2) { None } else { Some(2) };
                    }
                }

                // Collect digit input for measurement
                if !matches!(self.draw_state, DrawState::Idle) {
                    for ev in &i.events {
                        if let egui::Event::Text(t) = ev {
                            if t.chars().all(|c| c.is_ascii_digit() || c == ',' || c == '.' || c == 'x') {
                                self.measure_input.push_str(t);
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
        match self.tool {
            Tool::Select => {
                let (mx, my) = (self.mouse_screen[0], self.mouse_screen[1]);
                let (vw, vh) = (self.viewport_size[0], self.viewport_size[1]);
                let picked = self.pick(mx, my, vw, vh);
                // We need shift state - store it from the last handle_viewport call
                // Since on_click is called from handle_viewport where shift is available,
                // we check if shift key is held via a stored flag
                if self.shift_held {
                    if let Some(id) = picked {
                        if let Some(pos) = self.selected_ids.iter().position(|s| s == &id) {
                            self.selected_ids.remove(pos);
                        } else {
                            self.selected_ids.push(id);
                        }
                    }
                } else {
                    self.selected_ids = picked.into_iter().collect();
                }
                // Expand selection to include all group members
                self.expand_selection_to_groups();
                if !self.selected_ids.is_empty() { self.right_tab = RightTab::Properties; }
            }

            Tool::CreateBox => {
                match &self.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.draw_state = DrawState::BoxBase { p1: p };
                        }
                    }
                    DrawState::BoxBase { p1 } => {
                        if let Some(p2) = self.ground_snapped() {
                            let p1 = *p1;
                            self.draw_state = DrawState::BoxHeight { p1, p2 };
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
                        let id = self.scene.add_box(name, [x0, 0.0, z0], w, h, d, self.create_mat);
                        // Collision check on creation (warning only)
                        {
                            let components = crate::scene::scene_to_collision_components(&self.scene);
                            let col_center = [x0 + w / 2.0, h / 2.0, z0 + d / 2.0];
                            let new_comp = crate::collision::Component::new(id.clone(), crate::collision::ComponentKind::Generic, col_center, [w, h, d]);
                            let report = crate::collision::can_place_component(&new_comp, &components, &crate::collision::CollisionConfig::default());
                            if !report.is_allowed || !report.warning_pairs.is_empty() {
                                self.collision_warning = Some("放置位置與現有物件碰撞".to_string());
                            }
                        }
                        self.ai_log.log(&self.current_actor.clone(), "\u{5efa}\u{7acb}\u{65b9}\u{584a}", &format!("{:.0}\u{00d7}{:.0}\u{00d7}{:.0}", w, h, d), vec![id.clone()]);
                        self.selected_ids = vec![id.clone()];
                        self.draw_state = DrawState::Idle;
                        self.right_tab = RightTab::Properties;

                        // Check for alignment suggestion
                        self.check_alignment_suggestion(&id);
                    }
                    _ => {}
                }
            }

            Tool::CreateCylinder => {
                match &self.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.draw_state = DrawState::CylBase { center: p };
                        }
                    }
                    DrawState::CylBase { center } => {
                        let c = *center;
                        if let Some(mouse) = self.ground_snapped() {
                            let r = ((mouse[0]-c[0]).powi(2)+(mouse[2]-c[2]).powi(2)).sqrt().max(10.0);
                            self.draw_state = DrawState::CylHeight { center: c, radius: r };
                        }
                    }
                    DrawState::CylHeight { center, radius } => {
                        let c = *center;
                        let r = *radius;
                        let h = self.current_height(c).max(10.0);
                        let name = self.next_name("Cylinder");
                        let id = self.scene.add_cylinder(name, c, r, h, 48, self.create_mat);
                        // Collision check on creation (warning only)
                        {
                            let components = crate::scene::scene_to_collision_components(&self.scene);
                            let col_center = [c[0] + r, c[1] + h / 2.0, c[2] + r];
                            let new_comp = crate::collision::Component::new(id.clone(), crate::collision::ComponentKind::Generic, col_center, [r * 2.0, h, r * 2.0]);
                            let report = crate::collision::can_place_component(&new_comp, &components, &crate::collision::CollisionConfig::default());
                            if !report.is_allowed || !report.warning_pairs.is_empty() {
                                self.collision_warning = Some("放置位置與現有物件碰撞".to_string());
                            }
                        }
                        self.ai_log.log(&self.current_actor.clone(), "\u{5efa}\u{7acb}\u{5713}\u{67f1}", &format!("r={:.0} h={:.0}", r, h), vec![id.clone()]);
                        self.selected_ids = vec![id];
                        self.draw_state = DrawState::Idle;
                        self.right_tab = RightTab::Properties;
                    }
                    _ => {}
                }
            }

            Tool::CreateSphere => {
                match &self.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.draw_state = DrawState::SphRadius { center: p };
                        }
                    }
                    DrawState::SphRadius { center } => {
                        let c = *center;
                        if let Some(mouse) = self.ground_snapped() {
                            let r = ((mouse[0]-c[0]).powi(2)+(mouse[2]-c[2]).powi(2)).sqrt().max(10.0);
                            let name = self.next_name("Sphere");
                            let id = self.scene.add_sphere(name, c, r, 32, self.create_mat);
                            // Collision check on creation (warning only)
                            {
                                let components = crate::scene::scene_to_collision_components(&self.scene);
                                let col_center = [c[0] + r, c[1] + r, c[2] + r];
                                let new_comp = crate::collision::Component::new(id.clone(), crate::collision::ComponentKind::Generic, col_center, [r * 2.0, r * 2.0, r * 2.0]);
                                let report = crate::collision::can_place_component(&new_comp, &components, &crate::collision::CollisionConfig::default());
                                if !report.is_allowed || !report.warning_pairs.is_empty() {
                                    self.collision_warning = Some("放置位置與現有物件碰撞".to_string());
                                }
                            }
                            self.ai_log.log(&self.current_actor.clone(), "\u{5efa}\u{7acb}\u{7403}\u{9ad4}", &format!("r={:.0}", r), vec![id.clone()]);
                            self.selected_ids = vec![id];
                            self.draw_state = DrawState::Idle;
                            self.right_tab = RightTab::Properties;
                        }
                    }
                    _ => {}
                }
            }

            // Rectangle = 2D flat rectangle only (use Push/Pull to extrude)
            Tool::Rectangle => {
                match &self.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.draw_state = DrawState::BoxBase { p1: p };
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
                            self.selected_ids = vec![id];
                            self.draw_state = DrawState::Idle;
                            self.right_tab = RightTab::Properties;
                            self.tool = Tool::PushPull; // auto-switch to Push/Pull
                            self.file_message = Some(("矩形已建立 — 用推拉拉出高度".to_string(), std::time::Instant::now()));
                        }
                    }
                    _ => {}
                }
            }

            // Circle = same as CreateCylinder flow
            Tool::Circle => {
                match &self.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.draw_state = DrawState::CylBase { center: p };
                        }
                    }
                    DrawState::CylBase { center } => {
                        let c = *center;
                        if let Some(mouse) = self.ground_snapped() {
                            let r = ((mouse[0]-c[0]).powi(2)+(mouse[2]-c[2]).powi(2)).sqrt().max(10.0);
                            self.draw_state = DrawState::CylHeight { center: c, radius: r };
                        }
                    }
                    DrawState::CylHeight { center, radius } => {
                        let (c, r) = (*center, *radius);
                        let h = self.current_height(c).max(10.0);
                        let name = self.next_name("Circle");
                        let id = self.scene.add_cylinder(name, c, r, h, 48, self.create_mat);
                        self.selected_ids = vec![id];
                        self.draw_state = DrawState::Idle;
                        self.right_tab = RightTab::Properties;
                    }
                    _ => {}
                }
            }

            // Move click = select for moving (only when highlighted)
            Tool::Move => {
                if let Some(ref id) = self.hovered_id.clone() {
                    self.selected_ids = vec![id.clone()];
                    // Expand selection to include all group members
                    self.expand_selection_to_groups();
                    self.right_tab = RightTab::Properties;
                }
            }

            // Eraser = click to delete (only when highlighted)
            Tool::Eraser => {
                if let Some(ref id) = self.hovered_id.clone() {
                    self.ai_log.log(&self.current_actor.clone(), "\u{522a}\u{9664}\u{7269}\u{4ef6}", id, vec![id.clone()]);
                    self.scene.delete(id);
                    self.selected_ids.retain(|s| s != id);
                }
            }

            // Paint Bucket = apply material on click (hovered or picked)
            Tool::PaintBucket => {
                let target_id = self.hovered_id.clone().or_else(|| {
                    let (mx, my) = (self.mouse_screen[0], self.mouse_screen[1]);
                    let (vw, vh) = (self.viewport_size[0], self.viewport_size[1]);
                    self.pick(mx, my, vw, vh)
                });
                if let Some(ref id) = target_id {
                    self.scene.snapshot();
                    if let Some(obj) = self.scene.objects.get_mut(id) {
                        let old_mat = obj.material.label().to_string();
                        obj.material = self.create_mat;
                        self.ai_log.log(&self.current_actor.clone(), "設定材質", &format!("{} → {}", old_mat, self.create_mat.label()), vec![id.clone()]);
                        self.file_message = Some((format!("已套用材質: {}", self.create_mat.label()), std::time::Instant::now()));
                    }
                }
            }

            // TapeMeasure = snap-aware point-to-point measurement (like SketchUp)
            Tool::TapeMeasure => {
                match &self.draw_state {
                    DrawState::Idle => {
                        // Always use snap position first (endpoint/midpoint/edge/face)
                        let p = if let Some(ref snap) = self.snap_result {
                            if snap.snap_type != crate::app::SnapType::None {
                                snap.position
                            } else if let Some(g) = self.ground_snapped() {
                                g
                            } else { return; }
                        } else if let Some(g) = self.ground_snapped() {
                            g
                        } else { return; };

                        // Show what we snapped to
                        if let Some(ref snap) = self.snap_result {
                            if snap.snap_type != crate::app::SnapType::None && snap.snap_type != crate::app::SnapType::Grid {
                                self.file_message = Some((
                                    format!("量測起點: {} [{:.0}, {:.0}, {:.0}]", snap.snap_type.label(), p[0], p[1], p[2]),
                                    std::time::Instant::now()
                                ));
                            }
                        }
                        self.draw_state = DrawState::Measuring { start: p };
                    }
                    DrawState::Measuring { start } => {
                        let s = *start;
                        let p = if let Some(ref snap) = self.snap_result {
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
                        self.draw_state = DrawState::Idle;
                    }
                    _ => {}
                }
            }

            // Dimension = persistent two-point annotation (same measuring flow, no object info)
            Tool::Dimension => {
                match &self.draw_state {
                    DrawState::Idle => {
                        let p = if let Some(ref snap) = self.snap_result {
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
                        self.draw_state = DrawState::Measuring { start: p };
                    }
                    DrawState::Measuring { start } => {
                        let s = *start;
                        let p = if let Some(ref snap) = self.snap_result {
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
                        self.draw_state = DrawState::Idle;
                    }
                    _ => {}
                }
            }

            // Text = click to place a text label
            Tool::Text => {
                let p = if let Some(ref snap) = self.snap_result {
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

            // Rotate: click on object to enter interactive rotation (D1)
            Tool::Rotate => {
                let (mx, my) = (self.mouse_screen[0], self.mouse_screen[1]);
                let (vw, vh) = (self.viewport_size[0], self.viewport_size[1]);
                if let Some(id) = self.pick(mx, my, vw, vh) {
                    self.selected_ids = vec![id.clone()];
                    if let Some(obj) = self.scene.objects.get(&id) {
                        let center = match &obj.shape {
                            Shape::Box { width, height, depth } =>
                                [obj.position[0] + width / 2.0, obj.position[1] + height / 2.0, obj.position[2] + depth / 2.0],
                            Shape::Cylinder { radius, height, .. } =>
                                [obj.position[0] + radius, obj.position[1] + height / 2.0, obj.position[2] + radius],
                            Shape::Sphere { radius, .. } =>
                                [obj.position[0] + radius, obj.position[1] + radius, obj.position[2] + radius],
                            _ => obj.position,
                        };
                        // Project object center to screen for correct angle calculation
                        let vp_rect = egui::Rect::from_min_size(
                            egui::pos2(0.0, 0.0),
                            egui::vec2(self.viewport_size[0], self.viewport_size[1]),
                        );
                        let (cx, cy) = if let Some(screen_center) = self.world_to_screen(center, &vp_rect) {
                            (screen_center.x, screen_center.y)
                        } else {
                            (self.viewport_size[0] / 2.0, self.viewport_size[1] / 2.0)
                        };
                        let start_angle = (mx - cx).atan2(my - cy);
                        self.draw_state = DrawState::Rotating {
                            obj_id: id, center, start_angle, accumulated: 0.0,
                        };
                    }
                }
            }

            // Scale: click to select, determine handle from face
            Tool::Scale => {
                let (mx, my) = (self.mouse_screen[0], self.mouse_screen[1]);
                let (vw, vh) = (self.viewport_size[0], self.viewport_size[1]);
                if let Some(id) = self.pick(mx, my, vw, vh) {
                    self.selected_ids = vec![id.clone()];

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

                    self.draw_state = DrawState::Scaling { obj_id: id, handle, original_dims: orig };
                }
            }

            // Line: click-click chain drawing → adds edges to the shared free mesh
            Tool::Line => {
                let (mx, my) = (self.mouse_screen[0], self.mouse_screen[1]);
                let (vw, vh) = (self.viewport_size[0], self.viewport_size[1]);
                match &self.draw_state {
                    DrawState::Idle => {
                        // Try face snap first, then fall back to ground
                        let pos = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(p) = pos {
                            self.draw_state = DrawState::LineFrom { p1: p };
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
                                self.last_line_dir = Some([edge_dir[0] / edge_len, edge_dir[1] / edge_len]);
                            }

                            // Inference 2.0: update context after line drawn
                            crate::inference::update_context_after_line(&mut self.inference_ctx, p1, p2);

                            // Try to split a box face if line lies on it
                            self.try_split_face(p1, p2);

                            // Chain: start new line from p2
                            self.draw_state = DrawState::LineFrom { p1: p2 };
                        }
                    }
                    _ => {}
                }
            }

            // Arc: 3-click (start, end, bulge)
            Tool::Arc => {
                let (mx, my) = (self.mouse_screen[0], self.mouse_screen[1]);
                let (vw, vh) = (self.viewport_size[0], self.viewport_size[1]);
                match &self.draw_state {
                    DrawState::Idle => {
                        let pos = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(p) = pos {
                            self.draw_state = DrawState::ArcP1 { p1: p };
                        }
                    }
                    DrawState::ArcP1 { p1 } => {
                        let p1 = *p1;
                        let pos = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(p2) = pos {
                            self.draw_state = DrawState::ArcP2 { p1, p2 };
                        }
                    }
                    DrawState::ArcP2 { p1, p2 } => {
                        let (p1, p2) = (*p1, *p2);
                        let pos = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(p3) = pos {
                            let arc_pts = compute_arc(p1, p2, p3, 32);
                            let name = self.next_name("Arc");
                            let id = self.scene.add_line(name, arc_pts, 20.0, self.create_mat);
                            self.selected_ids = vec![id];
                            self.draw_state = DrawState::Idle;
                        }
                    }
                    _ => {}
                }
            }

            // Offset: face edge inset — click a box face to enter drag-to-inset mode
            Tool::Offset => {
                let (mx, my) = (self.mouse_screen[0], self.mouse_screen[1]);
                let (vw, vh) = (self.viewport_size[0], self.viewport_size[1]);

                if let Some((id, face)) = self.pick_face(mx, my, vw, vh) {
                    if let Some(obj) = self.scene.objects.get(&id) {
                        match &obj.shape {
                            Shape::Box { .. } => {
                                self.selected_ids = vec![id.clone()];
                                self.draw_state = DrawState::Offsetting {
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
                let ids = self.selected_ids.clone();
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
                if let Some(ref id) = self.selected_ids.first().cloned() {
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
                match &self.draw_state {
                    DrawState::Idle => {
                        if !self.selected_ids.is_empty() {
                            // Profile already selected, start path with first click
                            if let Some(p) = self.ground_snapped() {
                                self.draw_state = DrawState::FollowPath {
                                    source_id: self.selected_ids[0].clone(),
                                    path_points: vec![p],
                                };
                                self.file_message = Some(("路徑第一點已設定 — 繼續點擊加入路徑點, ESC 完成".to_string(), std::time::Instant::now()));
                            }
                        } else {
                            // No selection: pick an object first
                            let (mx, my) = (self.mouse_screen[0], self.mouse_screen[1]);
                            let (vw, vh) = (self.viewport_size[0], self.viewport_size[1]);
                            if let Some(id) = self.pick(mx, my, vw, vh) {
                                self.selected_ids = vec![id];
                                self.file_message = Some(("已選取截面 — 點擊地面設定路徑起點".to_string(), std::time::Instant::now()));
                            } else {
                                self.file_message = Some(("請先選取要沿路徑擠出的物件".to_string(), std::time::Instant::now()));
                            }
                        }
                    }
                    DrawState::FollowPath { source_id, path_points } => {
                        if let Some(p) = self.ground_snapped() {
                            let mut pts = path_points.clone();
                            let src = source_id.clone();
                            pts.push(p);
                            self.draw_state = DrawState::FollowPath { source_id: src, path_points: pts.clone() };
                            self.file_message = Some((
                                format!("路徑 {} 點 — 繼續點擊或按 ESC 完成擠出", pts.len()),
                                std::time::Instant::now(),
                            ));
                        }
                    }
                    _ => {}
                }
            }

            // ── Steel Mode Tools ──
            Tool::SteelColumn => {
                if let Some(p) = self.ground_snapped() {
                    self.scene.snapshot();
                    let member_h = self.steel_height;
                    let (h_sec, b_sec, tw, tf) = parse_h_profile(&self.steel_profile);
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

                    self.selected_ids = child_ids.clone();
                    self.ai_log.log(
                        &self.current_actor, "\u{5efa}\u{7acb}\u{67f1}",
                        &format!("{} H={:.0}", self.steel_profile, member_h),
                        child_ids,
                    );
                    self.file_message = Some((
                        format!("\u{67f1}\u{5df2}\u{5efa}\u{7acb}: {} @ [{:.0},{:.0}]", self.steel_profile, cx, cz),
                        std::time::Instant::now(),
                    ));
                }
            }

            Tool::SteelBeam => {
                match &self.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.draw_state = DrawState::LineFrom { p1: [p[0], self.steel_height, p[2]] };
                        }
                    }
                    DrawState::LineFrom { p1 } => {
                        let p1 = *p1;
                        if let Some(p2) = self.ground_snapped() {
                            self.scene.snapshot();
                            let (h_sec, b_sec, tw, tf) = parse_h_profile(&self.steel_profile);
                            let beam_y = self.steel_height - h_sec; // beam bottom

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

                            self.selected_ids = ids.clone();
                            self.draw_state = DrawState::Idle;
                            self.ai_log.log(
                                &self.current_actor, "\u{5efa}\u{7acb}\u{6881}",
                                &format!("{} L={:.0}", self.steel_profile, length),
                                ids,
                            );
                        }
                    }
                    _ => {}
                }
            }

            Tool::SteelBrace => {
                match &self.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.draw_state = DrawState::LineFrom { p1: p };
                        }
                    }
                    DrawState::LineFrom { p1 } => {
                        let p1 = *p1;
                        if let Some(p2) = self.ground_snapped() {
                            self.scene.snapshot();
                            let name = self.next_name("BR");
                            let id = self.scene.add_line(name, vec![p1, [p2[0], self.steel_height, p2[2]]], 50.0, MaterialKind::Steel);
                            self.selected_ids = vec![id.clone()];
                            self.draw_state = DrawState::Idle;
                            self.ai_log.log(&self.current_actor, "建立斜撐", "", vec![id]);
                        }
                    }
                    _ => {}
                }
            }

            Tool::SteelPlate => {
                match &self.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.draw_state = DrawState::BoxBase { p1: p };
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
                            self.selected_ids = vec![id.clone()];
                            self.draw_state = DrawState::Idle;
                            self.tool = Tool::PushPull;
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
                let (mx, my) = (self.mouse_screen[0], self.mouse_screen[1]);
                let (vw, vh) = (self.viewport_size[0], self.viewport_size[1]);
                if let Some(id) = self.pick(mx, my, vw, vh) {
                    if !self.selected_ids.contains(&id) {
                        self.selected_ids.push(id);
                    }
                    if self.selected_ids.len() >= 2 {
                        self.file_message = Some(("接頭已標記（選取兩構件）".into(), std::time::Instant::now()));
                    } else {
                        self.file_message = Some(("選取第二個構件".into(), std::time::Instant::now()));
                    }
                }
            }

            Tool::PushPull => {
                if matches!(self.draw_state, DrawState::Idle) {
                    let (mx, my) = (self.mouse_screen[0], self.mouse_screen[1]);
                    let (vw, vh) = (self.viewport_size[0], self.viewport_size[1]);
                    let clicked_face = self.pick_face(mx, my, vw, vh);

                    // A4: Double-click repeats last pull distance
                    if let Some((ref id, face)) = clicked_face {
                        let is_double = self.last_pull_click_time.elapsed().as_millis() < 500
                            && self.last_pull_distance.abs() > 0.1
                            && self.last_pull_face.as_ref()
                                .map(|(lid, lf)| lid == id && *lf == face)
                                .unwrap_or(false);

                        if is_double {
                            // Apply last pull distance instantly
                            self.scene.snapshot();
                            let dist = self.last_pull_distance;
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
                            self.last_pull_click_time = std::time::Instant::now();
                            // Don't change selection, keep face selected
                        } else {
                            // Check if clicking the SAME face that's already selected → toggle off
                            let same = self.selected_face.as_ref()
                                .map(|(sid, sf)| sid == id && *sf == face)
                                .unwrap_or(false);

                            if same {
                                // Deselect face
                                self.selected_face = None;
                            } else {
                                // Select this face (highlight + show properties)
                                self.selected_face = Some((id.clone(), face));
                                self.selected_ids = vec![id.clone()];
                                self.right_tab = RightTab::Properties;
                            }
                        }
                    } else if let Some(fid) = self.pick_free_mesh_face(mx, my, vw, vh) {
                        self.selected_ids.clear();
                        self.selected_face = None;
                        self.draw_state = DrawState::PullingFreeMesh { face_id: fid };
                    } else {
                        // Clicked empty space → deselect face
                        self.selected_face = None;
                    }
                }
            }
        }

        // Fallback: any click on an object selects it (like SketchUp)
        // Only when idle (not mid-draw) and nothing was selected by the tool handler
        if matches!(self.draw_state, DrawState::Idle) && self.selected_ids.is_empty() {
            let (mx, my) = (self.mouse_screen[0], self.mouse_screen[1]);
            let (vw, vh) = (self.viewport_size[0], self.viewport_size[1]);
            if let Some(id) = self.pick(mx, my, vw, vh) {
                self.selected_ids = vec![id];
                // Expand selection to include all group members
                self.expand_selection_to_groups();
                self.right_tab = RightTab::Properties;
            }
        }

        self.measure_input.clear();
    }

    pub(crate) fn apply_measure(&mut self) {
        // Array creation: "3x" or "5X" after Ctrl+Move copy
        if (self.measure_input.ends_with('x') || self.measure_input.ends_with('X'))
            && self.measure_input.len() > 1
        {
            let count_str = &self.measure_input[..self.measure_input.len() - 1];
            if let Ok(count) = count_str.parse::<usize>() {
                if count >= 2 && count <= 100 && self.move_is_copy && !self.selected_ids.is_empty() {
                    if let Some(orig) = self.move_origin {
                        // Get the displacement from original to current position
                        if let Some(first_obj) = self.scene.objects.get(&self.selected_ids[0]).cloned() {
                            let delta = [
                                first_obj.position[0] - orig[0],
                                first_obj.position[1] - orig[1],
                                first_obj.position[2] - orig[2],
                            ];
                            self.scene.snapshot();
                            // Create (count-1) more copies at equal intervals
                            for i in 2..=count {
                                for id in &self.selected_ids.clone() {
                                    if let Some(obj) = self.scene.objects.get(id).cloned() {
                                        let mut clone = obj;
                                        clone.id = self.scene.next_id_pub();
                                        clone.position = [
                                            orig[0] + delta[0] * i as f32,
                                            orig[1] + delta[1] * i as f32,
                                            orig[2] + delta[2] * i as f32,
                                        ];
                                        self.scene.objects.insert(clone.id.clone(), clone);
                                    }
                                }
                            }
                            self.scene.version += 1;
                            self.file_message = Some((
                                format!("\u{5df2}\u{5efa}\u{7acb} {} \u{500b}\u{526f}\u{672c}", count),
                                std::time::Instant::now(),
                            ));
                            self.measure_input.clear();
                            return;
                        }
                    }
                }
            }
        }

        let parts: Vec<f32> = self.measure_input
            .split(|c: char| c == ',' || c == 'x')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if parts.is_empty() { return; }

        match &self.draw_state {
            DrawState::BoxBase { p1 } => {
                if parts.len() >= 2 {
                    let p1 = *p1;
                    let p2 = [p1[0]+parts[0], 0.0, p1[2]+parts[1]];
                    self.draw_state = DrawState::BoxHeight { p1, p2 };
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
                self.selected_ids = vec![id];
                self.draw_state = DrawState::Idle;
            }
            DrawState::CylBase { center } => {
                let c = *center;
                let r = parts[0].max(10.0);
                self.draw_state = DrawState::CylHeight { center: c, radius: r };
            }
            DrawState::CylHeight { center, radius } => {
                let c = *center; let r = *radius;
                let h = parts[0].max(10.0);
                let name = self.next_name("Cylinder");
                let id = self.scene.add_cylinder(name, c, r, h, 48, self.create_mat);
                self.selected_ids = vec![id];
                self.draw_state = DrawState::Idle;
            }
            DrawState::SphRadius { center } => {
                let c = *center;
                let r = parts[0].max(10.0);
                let name = self.next_name("Sphere");
                let id = self.scene.add_sphere(name, c, r, 32, self.create_mat);
                self.selected_ids = vec![id];
                self.draw_state = DrawState::Idle;
            }
            DrawState::PullingFreeMesh { face_id } => {
                let fid = *face_id;
                let height = parts[0];
                self.scene.snapshot();
                self.scene.free_mesh.push_pull_face(fid, height);
                self.scene.version += 1;
                self.draw_state = DrawState::Idle;
                self.file_message = Some((
                    format!("\u{9762}\u{5df2}\u{63a8}\u{62c9} {}mm", height),
                    std::time::Instant::now(),
                ));
            }
            DrawState::Scaling { ref obj_id, handle, original_dims } => {
                let obj_id = obj_id.clone();
                let original_dims = *original_dims;
                let handle = *handle;
                let input = &self.measure_input;

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
                    self.draw_state = DrawState::Idle;
                }
            }
            // D1: Rotating — type angle in degrees to set precise rotation
            DrawState::Rotating { ref obj_id, .. } => {
                if let Ok(angle) = self.measure_input.parse::<f32>() {
                    let obj_id = obj_id.clone();
                    self.scene.snapshot();
                    if let Some(obj) = self.scene.objects.get_mut(&obj_id) {
                        obj.rotation_y = angle.to_radians();
                    }
                    self.draw_state = DrawState::Idle;
                }
            }
            _ => {}
        }
        self.measure_input.clear();
    }

    pub(crate) fn pick(&self, mx: f32, my: f32, vw: f32, vh: f32) -> Option<String> {
        let (origin, dir) = self.camera.screen_ray(mx, my, vw, vh);
        let mut best: Option<(f32, String)> = None;
        for obj in self.scene.objects.values() {
            let pos = glam::Vec3::from(obj.position);
            let (pick_min, pick_max) = match &obj.shape {
                Shape::Box { width, height, depth } => {
                    // Expand thin dimensions for easier picking (min 30mm pick size)
                    let pick_sz = 30.0_f32;
                    let center = pos + glam::Vec3::new(*width, *height, *depth) * 0.5;
                    let half = glam::Vec3::new(width.max(pick_sz), height.max(pick_sz), depth.max(pick_sz)) * 0.5;
                    (center - half, center + half)
                }
                Shape::Cylinder { radius, height, .. } => (pos, pos + glam::Vec3::new(*radius*2.0, *height, *radius*2.0)),
                Shape::Sphere { radius, .. } => (pos, pos + glam::Vec3::splat(*radius*2.0)),
                Shape::Line { points, thickness } => {
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

    /// Pick which face of which object the ray hits (for Push/Pull)
    pub(crate) fn pick_face(&self, mx: f32, my: f32, vw: f32, vh: f32) -> Option<(String, PullFace)> {
        let (origin, dir) = self.camera.screen_ray(mx, my, vw, vh);
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
        let (origin, dir) = self.camera.screen_ray(mx, my, vw, vh);
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
                self.selected_ids.clear();
                self.draw_state = DrawState::Idle;
                self.current_file = None;
                self.last_saved_version = self.scene.version;
                self.file_message = Some(("新建場景".to_string(), std::time::Instant::now()));
            }
            MenuAction::OpenScene => self.open_scene(),
            MenuAction::Revert => {
                if let Some(ref path) = self.current_file.clone() {
                    match self.scene.load_from_file(path) {
                        Ok(count) => {
                            self.selected_ids.clear();
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
                            self.selected_ids.clear();
                            self.file_message = Some((format!("已匯入 {} 個物件", count), std::time::Instant::now()));
                        }
                        Err(e) => self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now())),
                    }
                } else {
                    match self.scene.load_from_file(&path) {
                        Ok(count) => {
                            self.current_file = Some(path.clone());
                            self.add_recent_file(&path);
                            self.selected_ids.clear();
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
                for id in self.selected_ids.drain(..).collect::<Vec<_>>() {
                    self.scene.delete(&id);
                }
            }
            MenuAction::SelectAll => {
                self.selected_ids = self.scene.objects.keys().cloned().collect();
            }
            MenuAction::ViewFront => self.camera.set_front(),
            MenuAction::ViewBack => self.camera.set_back(),
            MenuAction::ViewLeft => self.camera.set_left(),
            MenuAction::ViewRight => self.camera.set_right(),
            MenuAction::ViewTop => self.camera.set_top(),
            MenuAction::ViewBottom => self.camera.set_bottom(),
            MenuAction::ViewIso => self.camera.set_iso(),
            MenuAction::ZoomExtents => self.zoom_extents(),
            MenuAction::Duplicate => {
                let mut new_ids = Vec::new();
                for id in &self.selected_ids.clone() {
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
                    self.selected_ids = new_ids;
                }
            }
            MenuAction::GroupSelected => {
                if self.selected_ids.len() >= 2 {
                    self.scene.snapshot();
                    let name = format!("Group_{}", self.scene.groups.len() + 1);
                    let gid = self.scene.create_group(name, self.selected_ids.clone());
                    self.file_message = Some((format!("已建立群組: {}", gid), std::time::Instant::now()));
                } else {
                    self.file_message = Some(("需要選取至少2個物件".to_string(), std::time::Instant::now()));
                }
            }
            MenuAction::ComponentSelected => {
                for id in &self.selected_ids {
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

                if self.selected_ids.len() >= 2 {
                    let id_a = self.selected_ids[0].clone();
                    let id_b = self.selected_ids[1].clone();

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
                            self.selected_ids = new_ids.clone();

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
                self.render_mode = match mode {
                    0 => RenderMode::Shaded,
                    1 => RenderMode::Wireframe,
                    2 => RenderMode::XRay,
                    3 => RenderMode::HiddenLine,
                    _ => RenderMode::Monochrome,
                };
            }
            MenuAction::ToggleBackground => {
                if self.sky_color[0] > 0.5 {
                    // Switch to dark
                    self.sky_color = [0.12, 0.12, 0.15];
                    self.ground_color = [0.2, 0.2, 0.22];
                } else {
                    // Switch to light
                    self.sky_color = [0.53, 0.72, 0.9];
                    self.ground_color = [0.65, 0.63, 0.60];
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
                            self.selected_ids.clear();
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
                            self.selected_ids.clear();
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
                                self.show_console = true;
                                self.console_push("INFO", format!("[DXF] Grids: X={} Y={} | Columns: {} | Beams: {} | Levels: {}",
                                    ir.grids.x_grids.len(), ir.grids.y_grids.len(),
                                    ir.columns.len(), ir.beams.len(), ir.levels.len()));
                                self.pending_ir = Some(ir);
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
                                self.show_console = true;
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
                            self.show_console = true; // auto-open console on import
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
                if let Some(id) = self.selected_ids.first().cloned() {
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
                                self.selected_ids = vec![a, b];
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
                self.selected_ids = vec![a, b];
                self.file_message = Some(("\u{9762}\u{5df2}\u{88ab}\u{7dda}\u{6bb5}\u{5207}\u{5272}".to_string(), std::time::Instant::now()));
            }
        }
    }

    /// Expand selection to include all group members for any selected object
    pub(crate) fn expand_selection_to_groups(&mut self) {
        let mut expanded = self.selected_ids.clone();
        for id in &self.selected_ids {
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
        self.selected_ids = expanded;
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

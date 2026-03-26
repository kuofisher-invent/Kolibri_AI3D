use crate::app::{DrawState, KolibriApp, PullFace, Tool};
use crate::renderer::Vertex;
use crate::scene::Shape;

impl KolibriApp {
    // ── Preview geometry generation ─────────────────────────────────────────

    pub(crate) fn build_preview(&mut self) -> (Vec<Vertex>, Vec<u32>) {
        let mut v = Vec::new();
        let mut idx = Vec::new();
        let ghost = [0.35, 0.55, 0.95, 0.35]; // semi-transparent blue

        match &self.editor.draw_state {
            DrawState::Idle => {}

            DrawState::BoxBase { p1 } => {
                if let Some(p2) = self.ground_snapped() {
                    let x0 = p1[0].min(p2[0]);
                    let z0 = p1[2].min(p2[2]);
                    let w = (p1[0] - p2[0]).abs().max(1.0);
                    let d = (p1[2] - p2[2]).abs().max(1.0);
                    crate::renderer::push_box_pub(&mut v, &mut idx, [x0, 0.0, z0], w, 5.0, d, ghost);
                }
            }

            DrawState::BoxHeight { p1, p2 } => {
                let x0 = p1[0].min(p2[0]);
                let z0 = p1[2].min(p2[2]);
                let w = (p1[0] - p2[0]).abs().max(1.0);
                let d = (p1[2] - p2[2]).abs().max(1.0);
                let center = [(p1[0]+p2[0])*0.5, 0.0, (p1[2]+p2[2])*0.5];
                let h = self.current_height(center);
                crate::renderer::push_box_pub(&mut v, &mut idx, [x0, 0.0, z0], w, h, d, ghost);
            }

            DrawState::CylBase { center } => {
                if let Some(mouse) = self.ground_snapped() {
                    let dx = mouse[0] - center[0];
                    let dz = mouse[2] - center[2];
                    let r = (dx*dx + dz*dz).sqrt().max(10.0);
                    crate::renderer::push_cylinder_pub(&mut v, &mut idx, *center, r, 5.0, 48, ghost);
                }
            }

            DrawState::CylHeight { center, radius } => {
                let h = self.current_height(*center);
                crate::renderer::push_cylinder_pub(&mut v, &mut idx, *center, *radius, h, 48, ghost);
            }

            DrawState::SphRadius { center } => {
                if let Some(mouse) = self.ground_snapped() {
                    let dx = mouse[0] - center[0];
                    let dz = mouse[2] - center[2];
                    let r = (dx*dx + dz*dz).sqrt().max(10.0);
                    crate::renderer::push_sphere_pub(&mut v, &mut idx, *center, r, 32, ghost);
                }
            }

            DrawState::Pulling { ref obj_id, face, .. } => {
                // Highlight the face being pulled with a translucent overlay
                let face_color = [0.95, 0.55, 0.15, 0.30]; // orange highlight
                if let Some(obj) = self.scene.objects.get(obj_id) {
                    let p = obj.position;
                    match &obj.shape {
                        Shape::Box { width, height, depth } => {
                            // Draw a thin slab on the pulled face to highlight it
                            let thickness = 5.0;
                            match face {
                                PullFace::Top => crate::renderer::push_box_pub(
                                    &mut v, &mut idx,
                                    [p[0], p[1] + height - thickness, p[2]],
                                    *width, thickness, *depth, face_color,
                                ),
                                PullFace::Bottom => crate::renderer::push_box_pub(
                                    &mut v, &mut idx,
                                    [p[0], p[1], p[2]],
                                    *width, thickness, *depth, face_color,
                                ),
                                PullFace::Right => crate::renderer::push_box_pub(
                                    &mut v, &mut idx,
                                    [p[0] + width - thickness, p[1], p[2]],
                                    thickness, *height, *depth, face_color,
                                ),
                                PullFace::Left => crate::renderer::push_box_pub(
                                    &mut v, &mut idx,
                                    [p[0], p[1], p[2]],
                                    thickness, *height, *depth, face_color,
                                ),
                                PullFace::Back => crate::renderer::push_box_pub(
                                    &mut v, &mut idx,
                                    [p[0], p[1], p[2] + depth - thickness],
                                    *width, *height, thickness, face_color,
                                ),
                                PullFace::Front => crate::renderer::push_box_pub(
                                    &mut v, &mut idx,
                                    [p[0], p[1], p[2]],
                                    *width, *height, thickness, face_color,
                                ),
                            }
                        }
                        Shape::Cylinder { radius, height, .. } => {
                            let thickness = 5.0;
                            match face {
                                PullFace::Top => crate::renderer::push_cylinder_pub(
                                    &mut v, &mut idx,
                                    [p[0], p[1] + height - thickness, p[2]],
                                    *radius, thickness, 48, face_color,
                                ),
                                PullFace::Bottom => crate::renderer::push_cylinder_pub(
                                    &mut v, &mut idx, p, *radius, thickness, 48, face_color,
                                ),
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }

            DrawState::Rotating { .. } => {}

            // F2: Show ghost of original shape during push/pull — not a DrawState,
            // handled below after the match

            DrawState::Scaling { ref obj_id, .. } => {
                // Show ghost of object at current (being-scaled) dimensions
                let scale_ghost = [0.45, 0.85, 0.45, 0.30]; // green ghost
                if let Some(obj) = self.scene.objects.get(obj_id) {
                    let p = obj.position;
                    match &obj.shape {
                        Shape::Box { width, height, depth } =>
                            crate::renderer::push_box_pub(&mut v, &mut idx, p, *width, *height, *depth, scale_ghost),
                        Shape::Cylinder { radius, height, segments } =>
                            crate::renderer::push_cylinder_pub(&mut v, &mut idx, p, *radius, *height, *segments, scale_ghost),
                        Shape::Sphere { radius, segments } =>
                            crate::renderer::push_sphere_pub(&mut v, &mut idx, p, *radius, *segments, scale_ghost),
                        Shape::Line { points, thickness, .. } =>
                            crate::renderer::push_line_pub(&mut v, &mut idx, points, *thickness, scale_ghost),
                        Shape::Mesh(_) => {} // TODO: mesh ghost preview
                    }
                }
            }

            DrawState::Offsetting { ref obj_id, face, distance } => {
                // Show ghost of the inset rectangle on the face being offset
                let offset_ghost = [0.85, 0.65, 0.35, 0.45]; // warm orange ghost
                let d = *distance;
                if let Some(obj) = self.scene.objects.get(obj_id) {
                    if let Shape::Box { width, height, depth } = &obj.shape {
                        let p = obj.position;
                        let (ghost_pos, gw, gh, gd) = match face {
                            PullFace::Top => (
                                [p[0] + d, p[1] + height, p[2] + d],
                                (*width - 2.0 * d).max(10.0), 2.0, (*depth - 2.0 * d).max(10.0),
                            ),
                            PullFace::Bottom => (
                                [p[0] + d, p[1] - 2.0, p[2] + d],
                                (*width - 2.0 * d).max(10.0), 2.0, (*depth - 2.0 * d).max(10.0),
                            ),
                            PullFace::Front => (
                                [p[0] + d, p[1] + d, p[2] - 2.0],
                                (*width - 2.0 * d).max(10.0), (*height - 2.0 * d).max(10.0), 2.0,
                            ),
                            PullFace::Back => (
                                [p[0] + d, p[1] + d, p[2] + depth],
                                (*width - 2.0 * d).max(10.0), (*height - 2.0 * d).max(10.0), 2.0,
                            ),
                            PullFace::Right => (
                                [p[0] + width, p[1] + d, p[2] + d],
                                2.0, (*height - 2.0 * d).max(10.0), (*depth - 2.0 * d).max(10.0),
                            ),
                            PullFace::Left => (
                                [p[0] - 2.0, p[1] + d, p[2] + d],
                                2.0, (*height - 2.0 * d).max(10.0), (*depth - 2.0 * d).max(10.0),
                            ),
                        };
                        crate::renderer::push_box_pub(&mut v, &mut idx, ghost_pos, gw, gh, gd, offset_ghost);
                    }
                }
            }

            DrawState::LineFrom { p1 } => {
                // Use face snap or ground snap for preview
                let p2_opt = self.snap_to_face(self.editor.mouse_screen[0], self.editor.mouse_screen[1],
                    self.viewer.viewport_size[0], self.viewer.viewport_size[1])
                    .or_else(|| self.ground_snapped());
                if let Some(p2) = p2_opt {
                    crate::renderer::push_line_pub(&mut v, &mut idx, &[*p1, p2], 20.0, ghost);
                }
            }

            DrawState::ArcP1 { p1 } => {
                let p2_opt = self.snap_to_face(self.editor.mouse_screen[0], self.editor.mouse_screen[1],
                    self.viewer.viewport_size[0], self.viewer.viewport_size[1])
                    .or_else(|| self.ground_snapped());
                if let Some(p2) = p2_opt {
                    crate::renderer::push_line_pub(&mut v, &mut idx, &[*p1, p2], 20.0, ghost);
                }
            }

            DrawState::ArcP2 { p1, p2 } => {
                let p3_opt = self.snap_to_face(self.editor.mouse_screen[0], self.editor.mouse_screen[1],
                    self.viewer.viewport_size[0], self.viewer.viewport_size[1])
                    .or_else(|| self.ground_snapped());
                if let Some(p3) = p3_opt {
                    let pts = crate::app::compute_arc(*p1, *p2, p3, 32);
                    crate::renderer::push_line_pub(&mut v, &mut idx, &pts, 20.0, ghost);

                    // 即時顯示圓弧資訊（半徑、角度、弧長）
                    if let Some(info) = crate::app::compute_arc_info(*p1, *p2, p3) {
                        let semi = if info.is_semicircle() { " [半圓]" } else { "" };
                        self.editor.cursor_dimension = Some((
                            self.editor.mouse_screen[0] + 20.0,
                            self.editor.mouse_screen[1] - 40.0,
                            format!("R{:.0} {:.1}° L{:.0}{}", info.radius, info.sweep_degrees(), info.arc_length(), semi),
                        ));
                    }
                }
            }

            DrawState::PieCenter { center } => {
                // 從中心到滑鼠畫半徑線
                if let Some(edge) = self.ground_snapped() {
                    crate::renderer::push_line_pub(&mut v, &mut idx, &[*center, edge], 15.0, ghost);
                    let r = ((edge[0]-center[0]).powi(2) + (edge[2]-center[2]).powi(2)).sqrt();
                    self.editor.cursor_dimension = Some((
                        self.editor.mouse_screen[0] + 20.0, self.editor.mouse_screen[1] - 20.0,
                        format!("R{:.0}", r),
                    ));
                }
            }

            DrawState::PieRadius { center, edge1 } => {
                // 畫扇形預覽
                if let Some(e2) = self.ground_snapped() {
                    let r = ((edge1[0]-center[0]).powi(2) + (edge1[2]-center[2]).powi(2)).sqrt();
                    let a1 = (edge1[2]-center[2]).atan2(edge1[0]-center[0]);
                    let a2 = (e2[2]-center[2]).atan2(e2[0]-center[0]);
                    let mut sweep = a2 - a1;
                    if sweep < 0.0 { sweep += std::f32::consts::TAU; }
                    let seg = 32;
                    let mut pts = vec![*center];
                    for i in 0..=seg {
                        let t = i as f32 / seg as f32;
                        let a = a1 + sweep * t;
                        pts.push([center[0] + r * a.cos(), center[1], center[2] + r * a.sin()]);
                    }
                    pts.push(*center);
                    crate::renderer::push_line_pub(&mut v, &mut idx, &pts, 15.0, ghost);
                    self.editor.cursor_dimension = Some((
                        self.editor.mouse_screen[0] + 20.0, self.editor.mouse_screen[1] - 20.0,
                        format!("R{:.0} {:.1}°", r, sweep.to_degrees()),
                    ));
                }
            }

            DrawState::Measuring { start } => {
                if let Some(p2) = self.ground_snapped() {
                    crate::renderer::push_line_pub(&mut v, &mut idx, &[*start, p2], 10.0, ghost);
                }
            }

            DrawState::PullingFreeMesh { .. } => {
                // Preview handled by tools.rs
            }

            DrawState::FollowPath { ref source_id, ref path_points } => {
                // Show path line preview during Follow Me extrusion
                let path_color = [0.9, 0.5, 0.2, 0.6]; // orange path preview
                let mut all_pts: Vec<[f32; 3]> = path_points.clone();
                if let Some(p) = self.ground_snapped() {
                    all_pts.push(p);
                }
                if all_pts.len() >= 2 {
                    crate::renderer::push_line_pub(&mut v, &mut idx, &all_pts, 15.0, path_color);
                }
                // Show ghost of source object at last path point (or cursor)
                if let Some(last_pt) = all_pts.last() {
                    if let Some(obj) = self.scene.objects.get(source_id) {
                        let follow_ghost = [0.9, 0.6, 0.3, 0.3];
                        match &obj.shape {
                            Shape::Box { width, height, depth } =>
                                crate::renderer::push_box_pub(&mut v, &mut idx, *last_pt, *width, *height, *depth, follow_ghost),
                            Shape::Cylinder { radius, height, segments } =>
                                crate::renderer::push_cylinder_pub(&mut v, &mut idx, *last_pt, *radius, *height, *segments, follow_ghost),
                            Shape::Sphere { radius, segments } =>
                                crate::renderer::push_sphere_pub(&mut v, &mut idx, *last_pt, *radius, *segments, follow_ghost),
                            _ => {}
                        }
                    }
                }
            }
        }

        // ── F2: Ghost original shape during push/pull ──
        if self.editor.selected_face.is_some() && self.editor.drag_snapshot_taken {
            if let (Some(orig_pos), Some(orig_dims)) = (self.editor.pull_original_pos, self.editor.pull_original_dims) {
                let ghost_color = [0.5, 0.5, 0.5, 0.15];
                crate::renderer::push_box_pub(
                    &mut v, &mut idx,
                    orig_pos, orig_dims[0], orig_dims[1], orig_dims[2],
                    ghost_color,
                );
            }
        }

        // ── Scale handles: 8 corner cubes when Scale tool is active ──
        if matches!(self.editor.tool, Tool::Scale) {
            for id in &self.editor.selected_ids {
                if let Some(obj) = self.scene.objects.get(id) {
                    let p = obj.position;
                    let (w, h, d) = match &obj.shape {
                        Shape::Box { width, height, depth } => (*width, *height, *depth),
                        Shape::Cylinder { radius, height, .. } => (*radius * 2.0, *height, *radius * 2.0),
                        Shape::Sphere { radius, .. } => (*radius * 2.0, *radius * 2.0, *radius * 2.0),
                        _ => continue,
                    };
                    let handle_size = (w + h + d) / 3.0 * 0.03; // 3% of average dimension
                    let handle_size = handle_size.max(10.0); // minimum 10mm
                    let corners = [
                        [p[0],     p[1],     p[2]],
                        [p[0] + w, p[1],     p[2]],
                        [p[0],     p[1] + h, p[2]],
                        [p[0] + w, p[1] + h, p[2]],
                        [p[0],     p[1],     p[2] + d],
                        [p[0] + w, p[1],     p[2] + d],
                        [p[0],     p[1] + h, p[2] + d],
                        [p[0] + w, p[1] + h, p[2] + d],
                    ];
                    let handle_color = [0.2, 0.8, 0.2, 1.0]; // green handles
                    let hs = handle_size / 2.0;
                    for corner in &corners {
                        crate::renderer::push_box_pub(
                            &mut v, &mut idx,
                            [corner[0] - hs, corner[1] - hs, corner[2] - hs],
                            handle_size, handle_size, handle_size,
                            handle_color,
                        );
                    }

                    // Face center handles (axis-colored)
                    let face_hs = handle_size * 0.8;
                    let fhs = face_hs / 2.0;
                    let face_handles: [([f32; 3], [f32; 4]); 6] = [
                        // Right face = X axis (red)
                        ([p[0] + w, p[1] + h / 2.0, p[2] + d / 2.0], [0.9, 0.2, 0.2, 1.0]),
                        // Left face = X axis (red)
                        ([p[0],     p[1] + h / 2.0, p[2] + d / 2.0], [0.9, 0.2, 0.2, 1.0]),
                        // Top face = Y axis (green-bright)
                        ([p[0] + w / 2.0, p[1] + h, p[2] + d / 2.0], [0.2, 0.85, 0.2, 1.0]),
                        // Bottom face = Y axis (green-bright)
                        ([p[0] + w / 2.0, p[1],     p[2] + d / 2.0], [0.2, 0.85, 0.2, 1.0]),
                        // Back face = Z axis (blue)
                        ([p[0] + w / 2.0, p[1] + h / 2.0, p[2] + d], [0.2, 0.4, 0.95, 1.0]),
                        // Front face = Z axis (blue)
                        ([p[0] + w / 2.0, p[1] + h / 2.0, p[2]],     [0.2, 0.4, 0.95, 1.0]),
                    ];
                    for (center, color) in &face_handles {
                        crate::renderer::push_box_pub(
                            &mut v, &mut idx,
                            [center[0] - fhs, center[1] - fhs, center[2] - fhs],
                            face_hs, face_hs, face_hs,
                            *color,
                        );
                    }
                }
            }
        }

        (v, idx)
    }
}

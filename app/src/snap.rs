use eframe::egui;

use crate::app::{
    AiSuggestion, DrawState, KolibriApp, SnapResult, SnapType, SuggestionAction, Tool,
};
use crate::scene::Shape;

/// Find the closest point on a 3D edge segment to the mouse cursor in screen space.
/// Samples the edge at `samples` points, projects each to screen, and returns the
/// world-space point closest to the mouse if within `max_screen_dist` pixels.
fn closest_point_on_edge_to_mouse(
    edge_a: [f32; 3],
    edge_b: [f32; 3],
    mouse: egui::Pos2,
    vp: &glam::Mat4,
    rect: &egui::Rect,
) -> Option<([f32; 3], f32)> {
    let samples = 10;
    let mut best_t = 0.0_f32;
    let mut best_dist = f32::MAX;

    for i in 0..=samples {
        let t = i as f32 / samples as f32;
        let world = [
            edge_a[0] + (edge_b[0] - edge_a[0]) * t,
            edge_a[1] + (edge_b[1] - edge_a[1]) * t,
            edge_a[2] + (edge_b[2] - edge_a[2]) * t,
        ];
        let clip = *vp * glam::Vec4::new(world[0], world[1], world[2], 1.0);
        if clip.w <= 0.0 {
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        let sx = rect.min.x + (ndc.x * 0.5 + 0.5) * rect.width();
        let sy = rect.min.y + (0.5 - ndc.y * 0.5) * rect.height();
        let d = ((sx - mouse.x).powi(2) + (sy - mouse.y).powi(2)).sqrt();
        if d < best_dist {
            best_dist = d;
            best_t = t;
        }
    }

    if best_dist < 25.0 {
        let pos = [
            edge_a[0] + (edge_b[0] - edge_a[0]) * best_t,
            edge_a[1] + (edge_b[1] - edge_a[1]) * best_t,
            edge_a[2] + (edge_b[2] - edge_a[2]) * best_t,
        ];
        Some((pos, best_dist))
    } else {
        None
    }
}

impl KolibriApp {
    pub(crate) fn ground_snapped(&self) -> Option<[f32; 3]> {
        self.editor.snap_result.as_ref().map(|s| s.position)
    }

    /// Raycast against all visible object faces and return the closest hit point.
    pub(crate) fn snap_to_face(&self, mx: f32, my: f32, vw: f32, vh: f32) -> Option<[f32; 3]> {
        let (origin, dir) = self.viewer.camera.screen_ray(mx, my, vw, vh);
        let origin = glam::Vec3::new(origin.x, origin.y, origin.z);
        let dir = glam::Vec3::new(dir.x, dir.y, dir.z);
        let mut best_t = f32::MAX;
        let mut best_pos: Option<[f32; 3]> = None;

        for obj in self.scene.objects.values() {
            if !obj.visible { continue; }
            let p = glam::Vec3::from(obj.position);

            match &obj.shape {
                crate::scene::Shape::Box { width, height, depth } => {
                    let max = p + glam::Vec3::new(*width, *height, *depth);
                    // 6 faces: (normal, point_on_plane, min_bound, max_bound)
                    let faces: [(glam::Vec3, glam::Vec3); 6] = [
                        (glam::Vec3::Y, glam::Vec3::new(p.x, max.y, p.z)),       // top
                        (glam::Vec3::NEG_Y, p),                                   // bottom
                        (glam::Vec3::X, glam::Vec3::new(max.x, p.y, p.z)),       // right
                        (glam::Vec3::NEG_X, p),                                   // left
                        (glam::Vec3::Z, glam::Vec3::new(p.x, p.y, max.z)),       // back
                        (glam::Vec3::NEG_Z, p),                                   // front
                    ];
                    for (normal, face_pt) in &faces {
                        let denom = dir.dot(*normal);
                        if denom.abs() < 1e-6 { continue; }
                        let t = (*face_pt - origin).dot(*normal) / denom;
                        if t < 0.0 || t >= best_t { continue; }
                        let hit = origin + dir * t;
                        // Check if hit is within the box bounds (with small tolerance)
                        let tol = 1.0;
                        if hit.x >= p.x - tol && hit.x <= max.x + tol
                        && hit.y >= p.y - tol && hit.y <= max.y + tol
                        && hit.z >= p.z - tol && hit.z <= max.z + tol {
                            best_t = t;
                            best_pos = Some([hit.x, hit.y, hit.z]);
                        }
                    }
                }
                crate::scene::Shape::Cylinder { radius, height, .. } => {
                    // Top and bottom circle faces
                    let center_top = p + glam::Vec3::new(*radius, *height, *radius);
                    let center_bot = p + glam::Vec3::new(*radius, 0.0, *radius);
                    for (normal, center, y_val) in [
                        (glam::Vec3::Y, center_top, p.y + height),
                        (glam::Vec3::NEG_Y, center_bot, p.y),
                    ] {
                        let denom = dir.dot(normal);
                        if denom.abs() < 1e-6 { continue; }
                        let t = (center - origin).dot(normal) / denom;
                        if t < 0.0 || t >= best_t { continue; }
                        let hit = origin + dir * t;
                        let dx = hit.x - (p.x + radius);
                        let dz = hit.z - (p.z + radius);
                        if dx * dx + dz * dz <= radius * radius {
                            best_t = t;
                            best_pos = Some([hit.x, y_val, hit.z]);
                        }
                    }
                }
                _ => {}
            }
        }
        best_pos
    }

    pub(crate) fn smart_snap(&mut self, raw_ground: [f32; 3], from_point: Option<[f32; 3]>) -> SnapResult {
        let grid = 500.0;
        let screen_threshold = self.editor.snap_threshold;

        // Build view-projection matrix and viewport rect for screen-space distance checks
        let aspect = if self.viewer.viewport_size[1] > 0.0 { self.viewer.viewport_size[0] / self.viewer.viewport_size[1] } else { 1.0 };
        let vp_mat = if self.viewer.use_ortho {
            self.viewer.camera.proj_ortho(aspect) * self.viewer.camera.view()
        } else {
            self.viewer.camera.view_proj(aspect)
        };
        let vp_rect = egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(self.viewer.viewport_size[0], self.viewer.viewport_size[1]),
        );
        let mouse_screen = egui::pos2(self.editor.mouse_screen[0], self.editor.mouse_screen[1]);

        // Default: grid snap on ground
        let mut best_pos = [Self::snap(raw_ground[0], grid), 0.0, Self::snap(raw_ground[2], grid)];
        let mut best_type = SnapType::Grid;
        let mut best_screen_dist = f32::MAX;

        // Collect all snap candidates as (world_pos, snap_type) then pick by screen distance.
        // Using a Vec avoids borrow-checker issues with a closure that borrows &mut self.
        let mut snap_candidates: Vec<([f32; 3], SnapType)> = Vec::new();

        // ── Origin ──
        snap_candidates.push(([0.0, 0.0, 0.0], SnapType::Origin));

        // ── Object snap points (ALL in full 3D) ──
        let objects: Vec<_> = self.scene.objects.values().cloned().collect();
        // Also collect all 3D edges for on-edge and intersection snapping
        let mut all_box_edges_3d: Vec<([f32; 3], [f32; 3])> = Vec::new();

        for obj in &objects {
            if !obj.visible { continue; }
            let p = obj.position;

            match &obj.shape {
                Shape::Box { width, height, depth } => {
                    let (w, h, d) = (*width, *height, *depth);

                    // 8 corner endpoints (full 3D)
                    let corners = [
                        [p[0],   p[1],   p[2]],
                        [p[0]+w, p[1],   p[2]],
                        [p[0]+w, p[1],   p[2]+d],
                        [p[0],   p[1],   p[2]+d],
                        [p[0],   p[1]+h, p[2]],
                        [p[0]+w, p[1]+h, p[2]],
                        [p[0]+w, p[1]+h, p[2]+d],
                        [p[0],   p[1]+h, p[2]+d],
                    ];
                    for c in &corners {
                        snap_candidates.push((*c, SnapType::Endpoint));
                    }

                    // 12 edge midpoints (full 3D)
                    let edge_pairs: [(usize, usize); 12] = [
                        (0,1),(1,2),(2,3),(3,0),  // bottom
                        (4,5),(5,6),(6,7),(7,4),  // top
                        (0,4),(1,5),(2,6),(3,7),  // vertical
                    ];
                    for (a, b) in &edge_pairs {
                        let mid = [
                            (corners[*a][0] + corners[*b][0]) / 2.0,
                            (corners[*a][1] + corners[*b][1]) / 2.0,
                            (corners[*a][2] + corners[*b][2]) / 2.0,
                        ];
                        snap_candidates.push((mid, SnapType::Midpoint));
                    }

                    // 6 face centers
                    let face_centers = [
                        [p[0]+w/2.0, p[1]+h,     p[2]+d/2.0], // top
                        [p[0]+w/2.0, p[1],       p[2]+d/2.0], // bottom
                        [p[0]+w/2.0, p[1]+h/2.0, p[2]],       // front
                        [p[0]+w/2.0, p[1]+h/2.0, p[2]+d],     // back
                        [p[0],       p[1]+h/2.0, p[2]+d/2.0], // left
                        [p[0]+w,     p[1]+h/2.0, p[2]+d/2.0], // right
                    ];
                    for fc in &face_centers {
                        snap_candidates.push((*fc, SnapType::FaceCenter));
                    }

                    // Collect 3D edges for on-edge snap and intersection
                    for (a, b) in &edge_pairs {
                        all_box_edges_3d.push((corners[*a], corners[*b]));
                    }
                }
                Shape::Cylinder { radius, height, .. } => {
                    let r = *radius;
                    let h = *height;
                    let cx = p[0] + r;
                    let cz = p[2] + r;
                    let center_b = [cx, p[1], cz];
                    let center_t = [cx, p[1] + h, cz];
                    snap_candidates.push((center_b, SnapType::Endpoint));
                    snap_candidates.push((center_t, SnapType::Endpoint));
                    let mid = [cx, p[1] + h / 2.0, cz];
                    snap_candidates.push((mid, SnapType::Midpoint));
                    for &dy in &[0.0, h] {
                        snap_candidates.push(([p[0],       p[1]+dy, cz],     SnapType::Endpoint));
                        snap_candidates.push(([p[0]+r*2.0, p[1]+dy, cz],     SnapType::Endpoint));
                        snap_candidates.push(([cx,         p[1]+dy, p[2]],   SnapType::Endpoint));
                        snap_candidates.push(([cx,         p[1]+dy, p[2]+r*2.0], SnapType::Endpoint));
                    }
                    // 切線吸附：從 from_point 到圓的切點（XZ 平面）
                    if let Some(from) = from_point {
                        let dx = from[0] - cx;
                        let dz = from[2] - cz;
                        let dist_sq = dx * dx + dz * dz;
                        if dist_sq > r * r * 1.01 {
                            // 外切點公式
                            let dist = dist_sq.sqrt();
                            let tang_len = (dist_sq - r * r).sqrt();
                            let angle = dz.atan2(dx);
                            let half_angle = (r / dist).asin();
                            for &sign in &[1.0_f32, -1.0] {
                                let ta = angle + sign * (std::f32::consts::FRAC_PI_2 + half_angle);
                                let tx = cx + r * ta.cos();
                                let tz = cz + r * ta.sin();
                                for &dy in &[0.0, h] {
                                    snap_candidates.push(([tx, p[1]+dy, tz], SnapType::Tangent));
                                }
                            }
                        }
                    }
                }
                Shape::Sphere { radius, .. } => {
                    let r = *radius;
                    let center = [p[0]+r, p[1]+r, p[2]+r];
                    snap_candidates.push((center, SnapType::FaceCenter));
                    // 6 cardinal points
                    snap_candidates.push(([p[0],       p[1]+r,     p[2]+r],     SnapType::Endpoint));
                    snap_candidates.push(([p[0]+r*2.0, p[1]+r,     p[2]+r],     SnapType::Endpoint));
                    snap_candidates.push(([p[0]+r,     p[1],       p[2]+r],     SnapType::Endpoint));
                    snap_candidates.push(([p[0]+r,     p[1]+r*2.0, p[2]+r],     SnapType::Endpoint));
                    snap_candidates.push(([p[0]+r,     p[1]+r,     p[2]],       SnapType::Endpoint));
                    snap_candidates.push(([p[0]+r,     p[1]+r,     p[2]+r*2.0], SnapType::Endpoint));
                }
                _ => {
                    snap_candidates.push((p, SnapType::Endpoint));
                }
            }
        }

        // ── Evaluate all point candidates in screen space ──
        // SU-style: 收集附近所有 snap 候選點（在 nearby_radius 內的），用於被動顯示小圓點
        let nearby_radius = 120.0_f32; // 像素範圍內的都顯示小圓點
        let mut nearby: Vec<([f32; 3], SnapType)> = Vec::new();
        for (world_pos, snap_type) in &snap_candidates {
            if let Some(screen_pos) = Self::world_to_screen_vp(*world_pos, &vp_mat, &vp_rect) {
                let dx = screen_pos.x - mouse_screen.x;
                let dy = screen_pos.y - mouse_screen.y;
                let screen_dist = (dx * dx + dy * dy).sqrt();
                // 收集附近所有端點/中點/面中心（被動圓點）
                if screen_dist < nearby_radius && matches!(snap_type,
                    SnapType::Endpoint | SnapType::Midpoint | SnapType::FaceCenter | SnapType::Origin)
                {
                    nearby.push((*world_pos, *snap_type));
                }
                if screen_dist < screen_threshold && screen_dist < best_screen_dist {
                    best_pos = *world_pos;
                    best_type = *snap_type;
                    best_screen_dist = screen_dist;
                }
            }
        }
        self.editor.nearby_snaps = nearby;

        // ── On-edge snap: find closest point on each 3D edge to mouse in screen space ──
        for (ea, eb) in &all_box_edges_3d {
            if let Some((closest, sd)) = closest_point_on_edge_to_mouse(
                *ea, *eb, mouse_screen, &vp_mat, &vp_rect,
            ) {
                if sd < screen_threshold && sd < best_screen_dist {
                    best_pos = closest;
                    best_type = SnapType::OnEdge;
                    best_screen_dist = sd;
                }
            }
        }

        // ── Intersection snap: 2D screen-space edge crossing ──
        // 限制最大邊數以避免 O(N²) 效能問題（>200 邊跳過）
        {
            let edge_count = all_box_edges_3d.len();
            let max_edges = 200; // 超過此數跳過交叉檢測
            if edge_count <= max_edges {
            for i in 0..edge_count {
                for j in (i + 1)..edge_count {
                    let (a1, a2) = all_box_edges_3d[i];
                    let (b1, b2) = all_box_edges_3d[j];
                    // Project all 4 endpoints to screen
                    let sa1 = Self::world_to_screen_vp(a1, &vp_mat, &vp_rect);
                    let sa2 = Self::world_to_screen_vp(a2, &vp_mat, &vp_rect);
                    let sb1 = Self::world_to_screen_vp(b1, &vp_mat, &vp_rect);
                    let sb2 = Self::world_to_screen_vp(b2, &vp_mat, &vp_rect);
                    if let (Some(sa1), Some(sa2), Some(sb1), Some(sb2)) = (sa1, sa2, sb1, sb2) {
                        // 2D segment intersection in screen space
                        let d1 = [sa2.x - sa1.x, sa2.y - sa1.y];
                        let d2 = [sb2.x - sb1.x, sb2.y - sb1.y];
                        let cross = d1[0] * d2[1] - d1[1] * d2[0];
                        if cross.abs() > 1e-6 {
                            let diff = [sb1.x - sa1.x, sb1.y - sa1.y];
                            let t = (diff[0] * d2[1] - diff[1] * d2[0]) / cross;
                            let u = (diff[0] * d1[1] - diff[1] * d1[0]) / cross;
                            if t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0 {
                                let screen_pt = egui::pos2(sa1.x + t * d1[0], sa1.y + t * d1[1]);
                                let sd = ((screen_pt.x - mouse_screen.x).powi(2) + (screen_pt.y - mouse_screen.y).powi(2)).sqrt();
                                if sd < screen_threshold && sd < best_screen_dist {
                                    // Compute the 3D intersection point (use t on edge A)
                                    best_pos = [
                                        a1[0] + t * (a2[0] - a1[0]),
                                        a1[1] + t * (a2[1] - a1[1]),
                                        a1[2] + t * (a2[2] - a1[2]),
                                    ];
                                    best_type = SnapType::Intersection;
                                    best_screen_dist = sd;
                                }
                            }
                        }
                    }
                }
            }
            } // end if edge_count <= max_edges
        }

        // Check axis alignment from starting point using SCREEN-SPACE distance
        // Priority: Endpoint > Midpoint > Axis > Perpendicular > Parallel > Grid
        let has_object_snap = matches!(best_type, SnapType::Endpoint | SnapType::Midpoint | SnapType::FaceCenter | SnapType::Origin | SnapType::Intersection);
        if let Some(from) = from_point {
            // If locked axis is set, force it (even over object snaps)
            if let Some(axis) = self.editor.locked_axis {
                match axis {
                    0 => { // X axis
                        best_pos = [raw_ground[0], 0.0, from[2]];
                        best_type = SnapType::AxisX;
                    }
                    1 => { // Y axis (vertical)
                        let height = self.current_height(from);
                        best_pos = [from[0], height, from[2]];
                        best_type = SnapType::AxisY;
                    }
                    2 => { // Z axis
                        best_pos = [from[0], 0.0, raw_ground[2]];
                        best_type = SnapType::AxisZ;
                    }
                    _ => {}
                }
            } else if !has_object_snap {
                // Screen-space axis detection: zoom-independent, always in pixels
                let axis_threshold_px = 20.0_f32; // pixels
                let sticky_release_px = 30.0_f32; // hysteresis: must move this far to unlock

                // Project points onto each axis from the origin point
                let on_x = [raw_ground[0], from[1], from[2]]; // constrained to X axis
                let on_z = [from[0], from[1], raw_ground[2]]; // constrained to Z axis

                let cursor_scr = Self::world_to_screen_vp(raw_ground, &vp_mat, &vp_rect);
                let on_x_scr = Self::world_to_screen_vp(on_x, &vp_mat, &vp_rect);
                let on_z_scr = Self::world_to_screen_vp(on_z, &vp_mat, &vp_rect);

                // Compute screen distances from cursor to axis-projected points
                let dist_to_x = match (cursor_scr, on_x_scr) {
                    (Some(c), Some(x)) => ((c.x - x.x).powi(2) + (c.y - x.y).powi(2)).sqrt(),
                    _ => f32::MAX,
                };
                let dist_to_z = match (cursor_scr, on_z_scr) {
                    (Some(c), Some(z)) => ((c.x - z.x).powi(2) + (c.y - z.y).powi(2)).sqrt(),
                    _ => f32::MAX,
                };

                // Sticky axis hysteresis: if already sticky, keep unless cursor is far enough away
                if let Some(sticky) = self.editor.sticky_axis {
                    let release_dist = match sticky {
                        0 => dist_to_x,
                        2 => dist_to_z,
                        _ => f32::MAX,
                    };
                    if release_dist < sticky_release_px {
                        // Stay locked to sticky axis
                        match sticky {
                            0 => {
                                best_pos = [raw_ground[0], 0.0, from[2]];
                                best_type = SnapType::AxisX;
                            }
                            2 => {
                                best_pos = [from[0], 0.0, raw_ground[2]];
                                best_type = SnapType::AxisZ;
                            }
                            _ => {}
                        }
                    } else {
                        // Release sticky axis
                        self.editor.sticky_axis = None;
                    }
                }

                // If not sticky-locked, try to detect axis from screen distance
                if self.editor.sticky_axis.is_none() && !matches!(best_type, SnapType::AxisX | SnapType::AxisZ) {
                    if dist_to_x < axis_threshold_px && dist_to_x < dist_to_z {
                        best_pos = [raw_ground[0], 0.0, from[2]];
                        best_type = SnapType::AxisX;
                        self.editor.sticky_axis = Some(0);
                    } else if dist_to_z < axis_threshold_px {
                        best_pos = [from[0], 0.0, raw_ground[2]];
                        best_type = SnapType::AxisZ;
                        self.editor.sticky_axis = Some(2);
                    }
                }
            }
        }

        // Parallel / Perpendicular inference when drawing a line
        // Only upgrade from Grid (never overwrite Endpoint, Midpoint, Origin, etc.)
        if let Some(from) = from_point {
            let dir = [raw_ground[0] - from[0], raw_ground[2] - from[2]];
            let dir_len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
            if dir_len > 100.0 {
                let dir_n = [dir[0] / dir_len, dir[1] / dir_len];

                // Collect reference edge directions: box edges + last drawn edge
                let mut ref_dirs: Vec<[f32; 2]> = Vec::new();
                for obj in self.scene.objects.values() {
                    if let Shape::Box { .. } = &obj.shape {
                        ref_dirs.push([1.0, 0.0]); // X-aligned
                        ref_dirs.push([0.0, 1.0]); // Z-aligned
                    }
                }
                // Also check against last drawn edge direction (SketchUp-style)
                if let Some(last_dir) = self.editor.last_line_dir {
                    ref_dirs.push(last_dir);
                }

                for edge_dir in &ref_dirs {
                    let dot = dir_n[0] * edge_dir[0] + dir_n[1] * edge_dir[1];
                    if dot.abs() > 0.98 {
                        // Parallel — only upgrade from Grid
                        if best_type == SnapType::Grid {
                            best_type = SnapType::Parallel;
                        }
                    } else if dot.abs() < 0.02 {
                        // Perpendicular — only upgrade from Grid
                        if best_type == SnapType::Grid {
                            best_type = SnapType::Perpendicular;
                        }
                    }
                }
            }
        }

        // ── Object boundary snapping（物件間 AABB 邊界吸附）──
        // 當 Move 工具拖曳時，吸附到其他物件的 min/max 邊界
        if matches!(self.editor.tool, Tool::Move) && !self.editor.selected_ids.is_empty() {
            let snap_dist = 30.0_f32; // mm 容差
            let selected_set: std::collections::HashSet<&str> = self.editor.selected_ids.iter().map(|s| s.as_str()).collect();
            for obj in self.scene.objects.values() {
                if selected_set.contains(obj.id.as_str()) || !obj.visible { continue; }
                let p = obj.position;
                let (mx, my, mz) = match &obj.shape {
                    Shape::Box { width, height, depth } => (p[0] + width, p[1] + height, p[2] + depth),
                    Shape::Cylinder { radius, height, .. } => (p[0] + radius * 2.0, p[1] + height, p[2] + radius * 2.0),
                    _ => continue,
                };
                // X 軸邊界
                if (best_pos[0] - p[0]).abs() < snap_dist { best_pos[0] = p[0]; }
                else if (best_pos[0] - mx).abs() < snap_dist { best_pos[0] = mx; }
                // Y 軸邊界
                if (best_pos[1] - p[1]).abs() < snap_dist { best_pos[1] = p[1]; }
                else if (best_pos[1] - my).abs() < snap_dist { best_pos[1] = my; }
                // Z 軸邊界
                if (best_pos[2] - p[2]).abs() < snap_dist { best_pos[2] = p[2]; }
                else if (best_pos[2] - mz).abs() < snap_dist { best_pos[2] = mz; }
            }
        }

        // Grid snap the final position (unless endpoint/midpoint/origin/intersection snap)
        if matches!(best_type, SnapType::Grid | SnapType::AxisX | SnapType::AxisZ
                    | SnapType::Parallel | SnapType::Perpendicular) {
            if matches!(best_type, SnapType::Grid | SnapType::Parallel | SnapType::Perpendicular) {
                best_pos[0] = Self::snap(best_pos[0], grid);
                best_pos[2] = Self::snap(best_pos[2], grid);
            } else {
                // For axis-aligned, snap the moving axis to grid
                match best_type {
                    SnapType::AxisX => best_pos[0] = Self::snap(best_pos[0], grid),
                    SnapType::AxisZ => best_pos[2] = Self::snap(best_pos[2], grid),
                    _ => {}
                }
            }
        }

        // ── Inference 2.0: Score-based ranking ──
        let mut candidates: Vec<crate::inference::InferenceCandidate> = Vec::new();

        // Add the current best as a candidate
        candidates.push(crate::inference::InferenceCandidate {
            position: best_pos,
            snap_type: best_type,
            score: match best_type {
                SnapType::Endpoint => 80.0,
                SnapType::Midpoint => 70.0,
                SnapType::Origin => 90.0,
                SnapType::Intersection => 85.0,
                SnapType::AxisX | SnapType::AxisZ | SnapType::AxisY => 60.0,
                SnapType::OnFace => 65.0,
                SnapType::FaceCenter => 68.0,
                SnapType::OnEdge => 62.0,
                SnapType::Perpendicular => 55.0,
                SnapType::Parallel => 50.0,
                SnapType::Grid => 20.0,
                SnapType::Tangent => 72.0,
                SnapType::None => 0.0,
            },
            label: best_type.label().to_string(),
            source: crate::inference::InferenceSource::Geometry,
        });

        // Also add the raw ground position as a "no snap" candidate
        candidates.push(crate::inference::InferenceCandidate {
            position: raw_ground,
            snap_type: SnapType::None,
            score: 10.0,
            label: "\u{81ea}\u{7531}".to_string(), // 自由
            source: crate::inference::InferenceSource::Geometry,
        });

        // Score all candidates
        crate::inference::rank_candidates(
            &mut candidates,
            &self.editor.inference_ctx,
            raw_ground,
            from_point,
            &self.scene,
        );

        // Pick the winner
        if let Some(winner) = candidates.first() {
            best_pos = winner.position;
            best_type = winner.snap_type;
        }

        // ── InferenceEngine 2.0: 4-layer scoring re-rank ──
        {
            use crate::inference_engine::{
                InferenceCandidate as IE2Cand, InferenceType as IE2Type, ResolveConfig,
            };
            let snap_to_ie2 = |st: SnapType| -> IE2Type {
                match st {
                    SnapType::Endpoint | SnapType::FaceCenter => IE2Type::Endpoint,
                    SnapType::Midpoint => IE2Type::Midpoint,
                    SnapType::Origin => IE2Type::Origin,
                    SnapType::Intersection => IE2Type::Intersection,
                    SnapType::OnEdge => IE2Type::OnEdge,
                    SnapType::OnFace => IE2Type::OnFace,
                    SnapType::AxisX => IE2Type::AxisLockX,
                    SnapType::AxisY => IE2Type::AxisLockY,
                    SnapType::AxisZ => IE2Type::AxisLockZ,
                    SnapType::Parallel => IE2Type::Parallel,
                    SnapType::Perpendicular => IE2Type::Perpendicular,
                    SnapType::Grid => IE2Type::Grid,
                    SnapType::Tangent => IE2Type::Custom,
                    SnapType::None => IE2Type::Custom,
                }
            };
            let ie2_to_snap = |it: IE2Type| -> SnapType {
                match it {
                    IE2Type::Endpoint => SnapType::Endpoint,
                    IE2Type::Midpoint => SnapType::Midpoint,
                    IE2Type::Origin => SnapType::Origin,
                    IE2Type::Intersection => SnapType::Intersection,
                    IE2Type::OnEdge => SnapType::OnEdge,
                    IE2Type::OnFace => SnapType::OnFace,
                    IE2Type::AxisLockX => SnapType::AxisX,
                    IE2Type::AxisLockY => SnapType::AxisY,
                    IE2Type::AxisLockZ => SnapType::AxisZ,
                    IE2Type::Parallel => SnapType::Parallel,
                    IE2Type::Perpendicular => SnapType::Perpendicular,
                    IE2Type::Grid | IE2Type::GridLine => SnapType::Grid,
                    _ => SnapType::None,
                }
            };
            let ie2_cands: Vec<IE2Cand> = candidates.iter().map(|c| {
                IE2Cand {
                    id: c.label.clone(),
                    inference_type: snap_to_ie2(c.snap_type),
                    position: c.position,
                    source_object_id: None,
                    raw_distance: best_screen_dist.min(100.0),
                }
            }).collect();
            // 使用 EditorState 上的 inference context（已由 tools.rs 更新）
            let ie2_ctx = &self.editor.inference_engine.context_from_editor(
                &self.editor.inference_ctx,
            );
            let scored = self.editor.inference_engine.score_candidates(&ie2_cands, ie2_ctx);
            let config = ResolveConfig::default();
            if let Some(top) = self.editor.inference_engine.resolve_primary(&scored, &config) {
                if top.breakdown.total > 30.0 {
                    best_pos = top.candidate.position;
                    best_type = ie2_to_snap(top.candidate.inference_type);
                }
            }
        }

        // Store inference label for overlay display
        self.editor.inference_label = candidates.first().map(|c| (c.label.clone(), c.source.clone()));

        SnapResult {
            position: best_pos,
            snap_type: best_type,
            from_point,
        }
    }

    pub(crate) fn get_drawing_origin(&self) -> Option<[f32; 3]> {
        match &self.editor.draw_state {
            DrawState::BoxBase { p1 } => Some(*p1),
            DrawState::LineFrom { p1 } => Some(*p1),
            DrawState::ArcP1 { p1 } => Some(*p1),
            DrawState::ArcP2 { p1, .. } => Some(*p1),
            DrawState::CylBase { center } => Some(*center),
            _ => None,
        }
    }

    /// Project world position to screen using a pre-computed view-projection matrix (static version).
    pub(crate) fn world_to_screen_vp(world_pos: [f32; 3], vp: &glam::Mat4, rect: &egui::Rect) -> Option<egui::Pos2> {
        let clip = *vp * glam::Vec4::new(world_pos[0], world_pos[1], world_pos[2], 1.0);
        if clip.w <= 0.0 { return None; }
        let ndc = clip.truncate() / clip.w;
        if ndc.x < -1.0 || ndc.x > 1.0 || ndc.y < -1.0 || ndc.y > 1.0 { return None; }
        let x = rect.min.x + (ndc.x * 0.5 + 0.5) * rect.width();
        let y = rect.min.y + (0.5 - ndc.y * 0.5) * rect.height();
        Some(egui::pos2(x, y))
    }

    /// 不裁切 NDC 範圍的投影（給旋轉盤等需要畫到視口外的 overlay 用）
    pub(crate) fn world_to_screen_unclipped(world_pos: [f32; 3], vp: &glam::Mat4, rect: &egui::Rect) -> Option<egui::Pos2> {
        let clip = *vp * glam::Vec4::new(world_pos[0], world_pos[1], world_pos[2], 1.0);
        if clip.w <= 0.0 { return None; } // 相機後方仍然要裁
        let ndc = clip.truncate() / clip.w;
        let x = rect.min.x + (ndc.x * 0.5 + 0.5) * rect.width();
        let y = rect.min.y + (0.5 - ndc.y * 0.5) * rect.height();
        Some(egui::pos2(x, y))
    }

    pub(crate) fn world_to_screen(&self, world_pos: [f32; 3], rect: &egui::Rect) -> Option<egui::Pos2> {
        let aspect = rect.width() / rect.height();
        let vp = if self.viewer.use_ortho {
            self.viewer.camera.proj_ortho(aspect) * self.viewer.camera.view()
        } else {
            self.viewer.camera.view_proj(aspect)
        };
        let clip = vp * glam::Vec4::new(world_pos[0], world_pos[1], world_pos[2], 1.0);
        if clip.w <= 0.0 { return None; }
        let ndc = clip.truncate() / clip.w;
        if ndc.x < -1.0 || ndc.x > 1.0 || ndc.y < -1.0 || ndc.y > 1.0 { return None; }
        let x = rect.min.x + (ndc.x * 0.5 + 0.5) * rect.width();
        let y = rect.min.y + (0.5 - ndc.y * 0.5) * rect.height();
        Some(egui::pos2(x, y))
    }

    pub(crate) fn check_alignment_suggestion(&mut self, id: &str) {
        let (new_p, new_w, new_d) = {
            if let Some(new_obj) = self.scene.objects.get(id) {
                if let Shape::Box { width, depth, .. } = &new_obj.shape {
                    (new_obj.position, *width, *depth)
                } else {
                    return;
                }
            } else {
                return;
            }
        };
        let edges = [new_p[0], new_p[0] + new_w, new_p[2], new_p[2] + new_d];
        let id_owned = id.to_string();
        for other in self.scene.objects.values() {
            if other.id == id_owned { continue; }
            if let Shape::Box { width: ow, depth: od, .. } = &other.shape {
                let other_edges = [other.position[0], other.position[0] + ow,
                                   other.position[2], other.position[2] + od];
                for (i, &e) in edges.iter().enumerate() {
                    for &oe in &other_edges {
                        let diff = (e - oe).abs();
                        if diff > 0.1 && diff < 200.0 {
                            let axis = if i < 2 { 0 } else { 2 };
                            self.editor.suggestion = Some(AiSuggestion {
                                message: format!("是否對齊到 {} 邊? (距離 {:.0}mm)", other.name, diff),
                                action: SuggestionAction::AlignToEdge {
                                    obj_id: id_owned.clone(),
                                    edge_pos: oe,
                                    axis,
                                },
                            });
                            return;
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn apply_suggestion(&mut self, action: SuggestionAction) {
        self.scene.snapshot();
        match action {
            SuggestionAction::AlignToEdge { ref obj_id, edge_pos, axis } => {
                if let Some(obj) = self.scene.objects.get_mut(obj_id) {
                    match axis {
                        0 => {
                            let w = match &obj.shape { Shape::Box { width, .. } => *width, _ => 0.0 };
                            let left = obj.position[0];
                            let right = left + w;
                            if (left - edge_pos).abs() < (right - edge_pos).abs() {
                                obj.position[0] = edge_pos;
                            } else {
                                obj.position[0] = edge_pos - w;
                            }
                        }
                        2 => {
                            let d = match &obj.shape { Shape::Box { depth, .. } => *depth, _ => 0.0 };
                            let front = obj.position[2];
                            let back = front + d;
                            if (front - edge_pos).abs() < (back - edge_pos).abs() {
                                obj.position[2] = edge_pos;
                            } else {
                                obj.position[2] = edge_pos - d;
                            }
                        }
                        _ => {}
                    }
                }
            }
            SuggestionAction::CompleteRectangle { .. } => {}
            SuggestionAction::SnapToGrid { ref obj_id, snapped_pos } => {
                if let Some(obj) = self.scene.objects.get_mut(obj_id) {
                    obj.position = snapped_pos;
                }
            }
        }
    }
}

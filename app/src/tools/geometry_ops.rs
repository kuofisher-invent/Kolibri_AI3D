use eframe::egui;

use crate::app::{
    compute_arc, DrawState, KolibriApp, PullFace, RenderMode, RightTab, ScaleHandle, SelectionMode, Tool,
};
use crate::camera;
use crate::scene::{MaterialKind, Shape};

impl KolibriApp {
    pub(crate) fn try_split_face(&mut self, p1: [f32; 3], p2: [f32; 3]) {
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
    pub(crate) fn try_split_face_on_line(&mut self, p1: [f32; 3], p2: [f32; 3]) {
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
    pub(crate) fn extrude_along_path(&mut self, profile_id: &str, path: &[[f32; 3]]) {
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
pub(crate) fn parse_h_profile(profile: &str) -> (f32, f32, f32, f32) {
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

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
            clone.rotation_xyz[1] = angle;
            clone.rotation_quat = glam::Quat::from_rotation_y(angle).to_array();

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

impl KolibriApp {
    /// 鏡射選取物件（axis: 0=X, 1=Y, 2=Z）；copy=true 時建立副本
    pub(crate) fn mirror_selected(&mut self, axis: usize, copy: bool) {
        if self.editor.selected_ids.is_empty() { return; }
        self.scene.snapshot();

        // 計算選取物件中心
        let ids: Vec<String> = self.editor.selected_ids.clone();
        let mut center = [0.0f32; 3];
        let mut count = 0usize;
        for id in &ids {
            if let Some(obj) = self.scene.objects.get(id) {
                for i in 0..3 { center[i] += obj.position[i]; }
                count += 1;
            }
        }
        if count == 0 { return; }
        for i in 0..3 { center[i] /= count as f32; }

        let mut new_ids = Vec::new();
        for id in &ids {
            if let Some(obj) = self.scene.objects.get(id).cloned() {
                let mut mirrored = obj.clone();
                if copy {
                    mirrored.id = format!("{}_mirror_{}", obj.id, axis);
                    mirrored.name = format!("{} (鏡射)", obj.name);
                }
                // 以中心鏡射
                let delta = mirrored.position[axis] - center[axis];
                mirrored.position[axis] = center[axis] - delta;

                if copy {
                    let mid = mirrored.id.clone();
                    self.scene.objects.insert(mid.clone(), mirrored);
                    new_ids.push(mid);
                } else {
                    self.scene.objects.insert(id.clone(), mirrored);
                }
            }
        }
        if copy && !new_ids.is_empty() {
            self.editor.selected_ids = new_ids;
        }
        self.scene.version += 1;
        let axis_name = ["X", "Y", "Z"][axis.min(2)];
        let action = if copy { "鏡射複製" } else { "鏡射" };
        self.file_message = Some((format!("{} {} 軸 ({} 個物件)", action, axis_name, count), std::time::Instant::now()));
    }
}

/// CNS 386 H型鋼規格表: (name, H, B, tw, tf, weight_kg_per_m)
pub(crate) const H_PROFILES: &[(&str, f32, f32, f32, f32, f32)] = &[
    ("H100x50x5x7",   100.0,  50.0, 5.0,  7.0,  9.30),
    ("H125x60x6x8",   125.0,  60.0, 6.0,  8.0, 13.10),
    ("H150x75x5x7",   150.0,  75.0, 5.0,  7.0, 14.00),
    ("H175x90x5x8",   175.0,  90.0, 5.0,  8.0, 18.20),
    ("H200x100x5.5x8", 200.0, 100.0, 5.5, 8.0, 21.30),
    ("H250x125x6x9",  250.0, 125.0, 6.0,  9.0, 29.60),
    ("H300x150x6x9",  300.0, 150.0, 6.0,  9.0, 36.70),
    ("H350x175x7x11", 350.0, 175.0, 7.0, 11.0, 49.60),
    ("H400x200x8x13", 400.0, 200.0, 8.0, 13.0, 66.00),
    ("H450x200x9x14", 450.0, 200.0, 9.0, 14.0, 76.00),
    ("H500x200x10x16",500.0, 200.0,10.0, 16.0, 89.70),
    ("H600x200x11x17",600.0, 200.0,11.0, 17.0,106.00),
    ("H700x300x13x24",700.0, 300.0,13.0, 24.0,185.00),
    ("H800x300x14x26",800.0, 300.0,14.0, 26.0,210.00),
    ("H900x300x16x28",900.0, 300.0,16.0, 28.0,243.00),
];

/// 方管規格表: (name, side, wall_thickness, weight_kg_per_m)
pub(crate) const TUBE_SQUARE_PROFILES: &[(&str, f32, f32, f32)] = &[
    ("□50x50x2.3",    50.0,  2.3,  3.45),
    ("□60x60x2.3",    60.0,  2.3,  4.21),
    ("□75x75x3.2",    75.0,  3.2,  7.10),
    ("□100x100x3.2", 100.0,  3.2,  9.62),
    ("□100x100x4.5", 100.0,  4.5, 13.30),
    ("□125x125x4.5", 125.0,  4.5, 16.80),
    ("□150x150x4.5", 150.0,  4.5, 20.30),
    ("□150x150x6.0", 150.0,  6.0, 26.60),
    ("□175x175x6.0", 175.0,  6.0, 31.40),
    ("□200x200x6.0", 200.0,  6.0, 36.20),
    ("□200x200x8.0", 200.0,  8.0, 47.40),
    ("□250x250x9.0", 250.0,  9.0, 67.20),
    ("□300x300x9.0", 300.0,  9.0, 81.00),
    ("□300x300x12.0",300.0, 12.0,106.00),
    ("□350x350x12.0",350.0, 12.0,125.00),
    ("□400x400x12.0",400.0, 12.0,143.00),
];

/// 圓管規格表: (name, outer_diameter, wall_thickness, weight_kg_per_m)
pub(crate) const TUBE_ROUND_PROFILES: &[(&str, f32, f32, f32)] = &[
    ("Ø42.7x2.3",    42.7,  2.3,  2.29),
    ("Ø48.6x2.3",    48.6,  2.3,  2.63),
    ("Ø60.5x2.3",    60.5,  2.3,  3.31),
    ("Ø76.3x3.2",    76.3,  3.2,  5.77),
    ("Ø89.1x3.2",    89.1,  3.2,  6.78),
    ("Ø101.6x3.2",  101.6,  3.2,  7.77),
    ("Ø114.3x3.5",  114.3,  3.5,  9.56),
    ("Ø139.8x4.0",  139.8,  4.0, 13.40),
    ("Ø165.2x5.0",  165.2,  5.0, 19.80),
    ("Ø190.7x5.3",  190.7,  5.3, 24.20),
    ("Ø216.3x5.8",  216.3,  5.8, 30.10),
    ("Ø267.4x6.6",  267.4,  6.6, 42.40),
    ("Ø318.5x6.9",  318.5,  6.9, 53.00),
    ("Ø355.6x7.9",  355.6,  7.9, 67.70),
    ("Ø406.4x7.9",  406.4,  7.9, 77.60),
];

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

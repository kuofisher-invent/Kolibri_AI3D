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

/// CNS 386 H型鋼規格表: (name, H, B, tw, tf, R, weight_kg_per_m)
/// 資料來源: Steel_dimension.xlsx — H 鋼 sheet
pub(crate) const H_PROFILES: &[(&str, f32, f32, f32, f32, f32, f32)] = &[
    // ── 100 系列 ──
    ("H100x50x5x7",     100.0,  50.0,  5.0,  7.0,  8.0,   9.3),
    ("H100x100x6x8",    100.0, 100.0,  6.0,  8.0,  8.0,  16.9),
    // ── 125 系列 ──
    ("H125x60x6x8",     125.0,  60.0,  6.0,  8.0,  8.0,  13.1),
    ("H125x125x6.5x9",  125.0, 125.0,  6.5,  9.0,  8.0,  23.6),
    // ── 150 系列 ──
    ("H150x75x5x7",     150.0,  75.0,  5.0,  7.0,  8.0,  14.0),
    ("H148x100x6x9",    148.0, 100.0,  6.0,  9.0,  8.0,  20.7),
    ("H150x150x7x10",   150.0, 150.0,  7.0, 10.0,  8.0,  31.1),
    // ── 175 系列 ──
    ("H175x90x5x8",     175.0,  90.0,  5.0,  8.0,  8.0,  18.0),
    ("H169x125x5.5x8",  169.0, 125.0,  5.5,  8.0, 12.0,  23.3),
    ("H175x175x7.5x11", 175.0, 175.0,  7.5, 11.0, 13.0,  40.4),
    // ── 200 系列 ──
    ("H198x99x4.5x7",   198.0,  99.0,  4.5,  7.0,  8.0,  17.8),
    ("H200x100x5.5x8",  200.0, 100.0,  5.5,  8.0,  8.0,  20.9),
    ("H194x150x6x9",    194.0, 150.0,  6.0,  9.0,  8.0,  29.9),
    ("H200x200x8x12",   200.0, 200.0,  8.0, 12.0, 13.0,  49.9),
    ("H208x208x10x16",  208.0, 208.0, 10.0, 16.0, 13.0,  65.7),
    ("H200x204x12x12",  200.0, 204.0, 12.0, 12.0, 13.0,  56.2),
    // ── 250 系列 ──
    ("H250x125x6x9",    250.0, 125.0,  6.0,  9.0,  8.0,  29.0),
    ("H244x175x7x11",   244.0, 175.0,  7.0, 11.0, 13.0,  43.6),
    ("H248x249x8x13",   248.0, 249.0,  8.0, 13.0, 13.0,  65.9),
    ("H250x250x9x14",   250.0, 250.0,  9.0, 14.0, 13.0,  71.8),
    ("H244x252x11x11",  244.0, 252.0, 11.0, 11.0, 13.0,  63.8),
    ("H250x255x14x14",  250.0, 255.0, 14.0, 14.0, 13.0,  81.6),
    // ── 300 系列 ──
    ("H298x149x5.5x8",  298.0, 149.0,  5.5,  8.0, 13.0,  32.0),
    ("H300x150x6.5x9",  300.0, 150.0,  6.5,  9.0, 13.0,  36.7),
    ("H298x299x9x14",   298.0, 299.0,  9.0, 14.0, 13.0,  65.9),
    ("H300x300x10x15",  300.0, 300.0, 10.0, 15.0, 13.0,  93.0),
    ("H300x300x11x12",  300.0, 300.0, 11.0, 12.0, 18.0,  82.5),
    ("H304x301x11x17",  304.0, 301.0, 11.0, 17.0, 13.0, 105.0),
    ("H294x302x12x12",  294.0, 302.0, 12.0, 12.0, 13.0,  83.4),
    ("H294x200x8x12",   294.0, 200.0,  8.0, 12.0, 13.0,  55.8),
    ("H298x201x9x14",   298.0, 201.0,  9.0, 14.0, 13.0,  64.4),
    ("H300x305x15x15",  300.0, 305.0, 15.0, 15.0, 13.0, 105.0),
    // ── 350 系列 ──
    ("H346x174x6x9",    346.0, 174.0,  6.0,  9.0, 13.0,  41.2),
    ("H350x175x7x11",   350.0, 175.0,  7.0, 11.0, 13.0,  49.4),
    ("H336x249x8x12",   336.0, 249.0,  8.0, 12.0, 13.0,  67.6),
    ("H340x250x9x14",   340.0, 250.0,  9.0, 14.0, 13.0,  78.1),
    ("H344x348x10x16",  344.0, 348.0, 10.0, 16.0, 13.0, 113.0),
    ("H350x350x12x19",  350.0, 350.0, 12.0, 19.0, 13.0, 135.0),
    ("H338x351x13x13",  338.0, 351.0, 13.0, 13.0, 13.0, 105.0),
    ("H356x352x14x22",  356.0, 352.0, 14.0, 22.0, 13.0, 157.0),
    ("H344x354x16x16",  344.0, 354.0, 16.0, 16.0, 13.0, 129.0),
    ("H350x357x19x19",  350.0, 357.0, 19.0, 19.0, 13.0, 154.0),
    // ── 400 系列 ──
    ("H369x199x7x11",   369.0, 199.0,  7.0, 11.0, 13.0,  56.1),
    ("H400x200x8x13",   400.0, 200.0,  8.0, 13.0, 13.0,  65.4),
    ("H386x299x9x14",   386.0, 299.0,  9.0, 14.0, 13.0,  92.2),
    ("H390x300x10x16",  390.0, 300.0, 10.0, 16.0, 13.0, 105.0),
    ("H394x398x11x18",  394.0, 398.0, 11.0, 18.0, 22.0, 147.0),
    ("H400x400x13x21",  400.0, 400.0, 13.0, 21.0, 22.0, 172.0),
    ("H388x402x15x15",  388.0, 402.0, 15.0, 15.0, 22.0, 140.0),
    ("H406x403x16x24",  406.0, 403.0, 16.0, 24.0, 22.0, 200.0),
    ("H394x405x18x18",  394.0, 405.0, 18.0, 18.0, 22.0, 168.0),
    ("H414x405x18x28",  414.0, 405.0, 18.0, 28.0, 22.0, 232.0),
    ("H428x407x20x35",  428.0, 407.0, 20.0, 35.0, 22.0, 283.0),
    ("H400x408x21x21",  400.0, 408.0, 21.0, 21.0, 22.0, 197.0),
    ("H458x417x30x50",  458.0, 417.0, 30.0, 50.0, 22.0, 415.0),
    ("H498x432x45x70",  498.0, 432.0, 45.0, 70.0, 22.0, 605.0),
    // ── 450 系列 ──
    ("H446x199x8x12",   446.0, 199.0,  8.0, 12.0, 13.0,  65.1),
    ("H450x200x9x14",   450.0, 200.0,  9.0, 14.0, 13.0,  74.9),
    ("H434x299x10x15",  434.0, 299.0, 10.0, 15.0, 13.0, 103.0),
    ("H440x300x11x18",  440.0, 300.0, 11.0, 18.0, 13.0, 121.0),
    // ── 500 系列 ──
    ("H496x199x9x14",   496.0, 199.0,  9.0, 14.0, 13.0,  77.9),
    ("H500x200x10x16",  500.0, 200.0, 10.0, 16.0, 13.0,  88.2),
    ("H506x201x11x19",  506.0, 201.0, 11.0, 19.0, 13.0, 102.0),
    ("H482x300x11x15",  482.0, 300.0, 11.0, 15.0, 13.0, 111.0),
    ("H488x300x11x18",  488.0, 300.0, 11.0, 18.0, 13.0, 125.0),
    ("H500x500x25x25",  500.0, 500.0, 25.0, 25.0, 26.0, 289.0),
    // ── 550 系列 ──
    ("H546x199x10x14",  546.0, 199.0, 10.0, 14.0, 13.0,  85.5),
    ("H550x200x10x18",  550.0, 200.0, 10.0, 18.0, 13.0,  98.0),
    ("H554x201x11x18",  554.0, 201.0, 11.0, 18.0, 22.0, 105.0),
    ("H560x202x12x21",  560.0, 202.0, 12.0, 21.0, 22.0, 119.0),
    ("H564x203x13x23",  564.0, 203.0, 13.0, 23.0, 13.0, 127.0),
    // ── 600 系列 ──
    ("H596x199x10x15",  596.0, 199.0, 10.0, 15.0, 13.0,  92.5),
    ("H600x200x11x17",  600.0, 200.0, 11.0, 17.0, 13.0, 103.0),
    ("H606x201x12x20",  606.0, 201.0, 12.0, 20.0, 13.0, 118.0),
    ("H612x202x13x23",  612.0, 202.0, 13.0, 23.0, 13.0, 132.0),
    ("H582x300x12x17",  582.0, 300.0, 12.0, 17.0, 13.0, 133.0),
    ("H588x300x12x20",  588.0, 300.0, 12.0, 20.0, 13.0, 147.0),
    ("H594x302x14x23",  594.0, 302.0, 14.0, 23.0, 13.0, 170.0),
    // ── 700 系列 ──
    ("H692x300x13x20",  692.0, 300.0, 13.0, 20.0, 18.0, 163.0),
    ("H700x300x13x24",  700.0, 300.0, 13.0, 24.0, 18.0, 182.0),
    ("H708x302x15x28",  708.0, 302.0, 15.0, 28.0, 18.0, 212.0),
    // ── 800 系列 ──
    ("H792x300x14x22",  792.0, 300.0, 14.0, 22.0, 18.0, 188.0),
    ("H800x300x14x26",  800.0, 300.0, 14.0, 26.0, 18.0, 207.0),
    // ── 900 系列 ──
    ("H890x299x15x23",  890.0, 299.0, 15.0, 23.0, 18.0, 210.0),
    ("H900x300x16x28",  900.0, 300.0, 16.0, 28.0, 18.0, 240.0),
    ("H912x302x18x34",  912.0, 302.0, 18.0, 34.0, 18.0, 283.0),
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

/// CNS 槽鋼規格表: (name, H, B, tw, tf, R, weight_kg_per_m)
/// 資料來源: Steel_dimension.xlsx — 槽鋼 sheet
pub(crate) const C_PROFILES: &[(&str, f32, f32, f32, f32, f32, f32)] = &[
    // ── 50~75 系列 ──
    ("C50x25x5",         50.0,  25.0,  5.0,  5.0,  6.0,   3.66),
    ("C65x32x4x6",       65.0,  32.0,  4.0,  6.0,  7.0,   4.78),
    ("C75x40x5x7",       75.0,  40.0,  5.0,  7.0,  8.0,   6.92),
    // ── 100 系列 ──
    ("C100x50x5x7.5",   100.0,  50.0,  5.0,  7.5,  8.0,   9.36),
    // ── 125 系列 ──
    ("C125x65x6x8",     125.0,  65.0,  6.0,  8.0,  8.0,  13.4),
    // ── 150 系列 ──
    ("C150x70x6x8.5",   150.0,  70.0,  6.0,  8.5,  9.0,  15.8),
    ("C150x75x6.5x10",  150.0,  75.0,  6.5, 10.0, 10.0,  18.6),
    ("C150x75x9x12.5",  150.0,  75.0,  9.0, 12.5, 15.0,  24.0),
    ("C150x90x6x10.5",  150.0,  90.0,  6.0, 10.5,  9.0,  21.1),
    ("C150x90x9x14",    150.0,  90.0,  9.0, 14.0, 16.0,  28.9),
    ("C150x90x12x17.5", 150.0,  90.0, 12.0, 17.5, 23.0,  36.6),
    // ── 180 系列 ──
    ("C180x75x7x10.5",  180.0,  75.0,  7.0, 10.5, 11.0,  21.4),
    ("C180x75x10x13.5", 180.0,  75.0, 10.0, 13.5, 17.5,  28.6),
    ("C180x75x12x15.5", 180.0,  75.0, 12.0, 15.5, 21.0,  33.2),
    ("C180x90x7.5x12.5",180.0,  90.0,  7.5, 12.5, 13.0,  27.1),
    ("C180x90x10x15.5", 180.0,  90.0, 10.0, 15.5, 19.0,  34.3),
    ("C180x90x12.5x18.5",180.0, 90.0, 12.5, 18.5, 25.0,  41.5),
    // ── 200 系列 ──
    ("C200x70x7x10",    200.0,  70.0,  7.0, 10.0, 11.0,  21.1),
    ("C200x80x7.5x11",  200.0,  80.0,  7.5, 11.0, 12.0,  24.6),
    ("C200x80x9x13.5",  200.0,  80.0,  9.0, 13.5, 16.0,  29.7),
    ("C200x80x10.5x15.5",200.0, 80.0, 10.5, 15.5, 20.5,  34.3),
    ("C200x90x8x13.5",  200.0,  90.0,  8.0, 13.5, 14.0,  30.3),
    ("C200x90x10x15.5", 200.0,  90.0, 10.0, 15.5, 19.0,  35.9),
    ("C200x90x12x18",   200.0,  90.0, 12.0, 18.0, 24.0,  42.1),
    // ── 230 系列 ──
    ("C230x80x8x12",    230.0,  80.0,  8.0, 12.0, 13.0,  28.4),
    ("C230x80x10x15",   230.0,  80.0, 10.0, 15.0, 19.5,  35.3),
    ("C230x80x12x17.5", 230.0,  80.0, 12.0, 17.5, 24.5,  41.6),
    ("C230x90x8.5x13.5",230.0,  90.0,  8.5, 13.5, 15.0,  33.1),
    ("C230x90x10.5x16", 230.0,  90.0, 10.5, 16.0, 20.0,  39.8),
    ("C230x90x12.5x18.5",230.0, 90.0, 12.5, 18.5, 25.0,  46.4),
    // ── 250 系列 ──
    ("C250x80x8x12.5",  250.0,  80.0,  8.0, 12.5, 14.0,  30.2),
    ("C250x80x10x15",   250.0,  80.0, 10.0, 15.0, 19.5,  36.9),
    ("C250x80x12x17.5", 250.0,  80.0, 12.0, 17.5, 24.5,  43.5),
    ("C250x90x9x13",    250.0,  90.0,  9.0, 13.0, 14.0,  34.6),
    ("C250x90x11x14.5", 250.0,  90.0, 11.0, 14.5, 17.0,  40.2),
    ("C250x90x13x18",   250.0,  90.0, 13.0, 18.0, 24.0,  48.5),
    // ── 280 系列 ──
    ("C280x100x9x13",   280.0, 100.0,  9.0, 13.0, 14.0,  38.8),
    ("C280x100x11.5x16",280.0, 100.0, 11.5, 16.0, 18.0,  48.2),
    ("C280x100x13x18",  280.0, 100.0, 13.0, 18.0, 21.0,  54.1),
    // ── 300 系列 ──
    ("C300x90x9x13",    300.0,  90.0,  9.0, 13.0, 14.0,  38.1),
    ("C300x90x10x15.5", 300.0,  90.0, 10.0, 15.5, 19.0,  43.8),
    ("C300x90x12x16",   300.0,  90.0, 12.0, 16.0, 19.0,  48.6),
    ("C300x90x13x19.5", 300.0,  90.0, 13.0, 19.5, 23.0,  55.3),
    ("C300x100x10x16",  300.0, 100.0, 10.0, 16.0, 17.0,  46.8),
    ("C300x100x12x18",  300.0, 100.0, 12.0, 18.0, 21.0,  54.0),
    ("C300x100x14x20",  300.0, 100.0, 14.0, 20.0, 24.0,  61.2),
    // ── 380 系列 ──
    ("C380x100x10.5x16",380.0, 100.0, 10.5, 16.0, 18.0,  54.5),
    ("C380x100x13x16.5",380.0, 100.0, 13.0, 16.5, 18.0,  62.0),
    ("C380x100x13x20",  380.0, 100.0, 13.0, 20.0, 24.0,  67.3),
    ("C380x100x15.5x23.5",380.0,100.0,15.5, 23.5, 29.0,  79.1),
    // ── 425 系列 ──
    ("C425x100x10.5x16",425.0, 100.0, 10.5, 16.0, 18.0,  58.2),
    ("C425x100x13x19.5",425.0, 100.0, 13.0, 19.5, 24.0,  71.2),
    ("C425x100x15.5x23.5",425.0,100.0,15.5, 23.5, 29.0,  84.6),
];

/// CNS 等邊角鋼(L型鋼)規格表: (name, leg, thickness, R, weight_kg_per_m)
/// 資料來源: Steel_dimension.xlsx — 角鐵 sheet
pub(crate) const L_PROFILES: &[(&str, f32, f32, f32, f32)] = &[
    // ── 20~30 系列 ──
    ("L20x20x3",      20.0,  3.0,  4.0,  0.885),
    ("L25x25x3",      25.0,  3.0,  4.0,  1.12),
    ("L25x25x5",      25.0,  5.0,  4.0,  1.76),
    ("L30x30x3",      30.0,  3.0,  4.0,  1.36),
    ("L30x30x5",      30.0,  5.0,  4.0,  2.16),
    // ── 35~40 系列 ──
    ("L35x35x3",      35.0,  3.0,  4.5,  1.60),
    ("L35x35x5",      35.0,  5.0,  4.5,  2.56),
    ("L40x40x3",      40.0,  3.0,  4.5,  1.83),
    ("L40x40x5",      40.0,  5.0,  4.5,  2.95),
    // ── 45~50 系列 ──
    ("L45x45x4",      45.0,  4.0,  6.5,  2.74),
    ("L45x45x6",      45.0,  6.0,  6.5,  3.96),
    ("L50x50x4",      50.0,  4.0,  6.5,  3.06),
    ("L50x50x6",      50.0,  6.0,  6.5,  4.43),
    ("L50x50x8",      50.0,  8.0,  6.5,  5.78),
    // ── 60 系列 ──
    ("L60x60x4",      60.0,  4.0,  6.5,  3.68),
    ("L60x60x5",      60.0,  5.0,  6.5,  4.55),
    ("L60x60x7",      60.0,  7.0,  6.5,  6.21),
    ("L60x60x9",      60.0,  9.0,  6.5,  7.85),
    // ── 65 系列 ──
    ("L65x65x6",      65.0,  6.0,  8.5,  5.91),
    ("L65x65x8",      65.0,  8.0,  8.5,  7.66),
    ("L65x65x10",     65.0, 10.0,  8.5,  9.42),
    // ── 70 系列 ──
    ("L70x70x6",      70.0,  6.0,  8.5,  6.38),
    ("L70x70x8",      70.0,  8.0,  8.5,  8.29),
    ("L70x70x10",     70.0, 10.0,  8.5, 10.2),
    // ── 75 系列 ──
    ("L75x75x6",      75.0,  6.0,  8.5,  6.85),
    ("L75x75x9",      75.0,  9.0,  8.5,  9.96),
    ("L75x75x12",     75.0, 12.0,  8.5, 13.0),
    ("L75x75x14",     75.0, 14.0,  8.5, 14.9),
    // ── 80 系列 ──
    ("L80x80x6",      80.0,  6.0,  8.5,  7.32),
    ("L80x80x9",      80.0,  9.0,  8.5, 10.7),
    ("L80x80x12",     80.0, 12.0,  8.5, 13.9),
    // ── 90 系列 ──
    ("L90x90x6",      90.0,  6.0, 10.0,  8.28),
    ("L90x90x7",      90.0,  7.0, 10.0,  9.59),
    ("L90x90x10",     90.0, 10.0, 10.0, 13.3),
    ("L90x90x13",     90.0, 13.0, 10.0, 17.0),
    ("L90x90x15",     90.0, 15.0, 10.0, 19.4),
    // ── 100 系列 ──
    ("L100x100x7",   100.0,  7.0, 10.0, 10.7),
    ("L100x100x10",  100.0, 10.0, 10.0, 14.9),
    ("L100x100x13",  100.0, 13.0, 10.0, 19.1),
    ("L100x100x15",  100.0, 15.0, 10.0, 21.8),
    ("L100x100x17",  100.0, 17.0, 10.0, 24.4),
    // ── 120~130 系列 ──
    ("L120x120x8",   120.0,  8.0, 12.0, 14.7),
    ("L130x130x9",   130.0,  9.0, 12.0, 17.9),
    ("L130x130x12",  130.0, 12.0, 12.0, 23.4),
    ("L130x130x15",  130.0, 15.0, 12.0, 28.8),
    ("L130x130x17",  130.0, 17.0, 12.0, 32.4),
    // ── 150 系列 ──
    ("L150x150x11",  150.0, 11.0, 14.0, 25.1),
    ("L150x150x12",  150.0, 12.0, 14.0, 27.3),
    ("L150x150x15",  150.0, 15.0, 14.0, 33.6),
    ("L150x150x19",  150.0, 19.0, 14.0, 41.9),
    ("L150x150x22",  150.0, 22.0, 14.0, 48.0),
    // ── 175 系列 ──
    ("L175x175x12",  175.0, 12.0, 15.0, 31.8),
    ("L175x175x15",  175.0, 15.0, 15.0, 39.4),
    // ── 200 系列 ──
    ("L200x200x15",  200.0, 15.0, 17.0, 45.3),
    ("L200x200x20",  200.0, 20.0, 17.0, 59.7),
    ("L200x200x25",  200.0, 25.0, 17.0, 73.6),
    ("L200x200x27",  200.0, 27.0, 17.0, 79.0),
    ("L200x200x29",  200.0, 29.0, 17.0, 84.5),
    // ── 250 系列 ──
    ("L250x250x25",  250.0, 25.0, 24.0, 93.7),
    ("L250x250x35",  250.0, 35.0, 24.0, 128.0),
];

/// 鋼構截面類型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SteelSectionType {
    /// H型鋼（寬翼 I 形）
    H,
    /// C型鋼（槽鋼，U 形）
    C,
    /// L型鋼（角鋼）
    L,
}

impl SteelSectionType {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::H => "H",
            Self::C => "C",
            Self::L => "L",
        }
    }
}

/// Parse H-section profile string like "H300x150x6.5x9" -> (H, B, tw, tf, R) in mm
/// 先查表取 R，查不到用預設公式
pub(crate) fn parse_h_profile(profile: &str) -> (f32, f32, f32, f32, f32) {
    // 先從表中查找精確匹配
    for &(name, h, b, tw, tf, r, _w) in H_PROFILES {
        if name == profile { return (h, b, tw, tf, r); }
    }
    // 解析字串（strip 前綴 H/h 和可能的 dash/空格）
    let stripped = profile
        .trim_start_matches(|c: char| c == 'H' || c == 'h' || c == '-' || c == ' ');
    let parts: Vec<f32> = stripped
        .split('x')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    match parts.len() {
        4 => { let r = if parts[0] >= 400.0 { 22.0 } else { 13.0 }; (parts[0], parts[1], parts[2], parts[3], r) }
        3 => { let r = if parts[0] >= 400.0 { 22.0 } else { 13.0 }; (parts[0], parts[1], parts[2], parts[2], r) }
        2 => (parts[0], parts[1], 8.0, 12.0, 13.0),
        1 => (parts[0], parts[0] * 0.5, 8.0, 12.0, 13.0),
        _ => (300.0, 150.0, 6.5, 9.0, 13.0),
    }
}

/// Parse C-section profile string like "C200x80x7.5x11" -> (H, B, tw, tf, R) in mm
pub(crate) fn parse_c_profile(profile: &str) -> (f32, f32, f32, f32, f32) {
    // 先從表中查找精確匹配
    for &(name, h, b, tw, tf, r, _w) in C_PROFILES {
        if name == profile { return (h, b, tw, tf, r); }
    }
    let stripped = profile
        .trim_start_matches(|c: char| c == 'C' || c == 'c' || c == '-' || c == ' ');
    let parts: Vec<f32> = stripped
        .split('x')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    match parts.len() {
        4 => (parts[0], parts[1], parts[2], parts[3], (parts[2] * 1.5).min(20.0)),
        3 => (parts[0], parts[1], parts[2], parts[2], (parts[2] * 1.5).min(20.0)),
        2 => (parts[0], parts[1], 6.0, 10.0, 9.0),
        1 => (parts[0], parts[0] * 0.5, 6.0, 10.0, 9.0),
        _ => (200.0, 80.0, 7.5, 11.0, 12.0),
    }
}

/// Parse L-section profile string like "L100x100x10" -> (leg, thickness, R) in mm
pub(crate) fn parse_l_profile(profile: &str) -> (f32, f32, f32) {
    // 先從表中查找精確匹配
    for &(name, leg, t, r, _w) in L_PROFILES {
        if name == profile { return (leg, t, r); }
    }
    let stripped = profile
        .trim_start_matches(|c: char| c == 'L' || c == 'l' || c == '-' || c == ' ');
    let parts: Vec<f32> = stripped
        .split('x')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    match parts.len() {
        3 => { let r = if parts[0] >= 90.0 { 10.0 } else { 6.5 }; (parts[0], parts[2], r) }
        2 => { let r = if parts[0] >= 90.0 { 10.0 } else { 6.5 }; (parts[0], parts[1], r) }
        1 => (parts[0], parts[0] * 0.1, 8.0),
        _ => (100.0, 10.0, 10.0),
    }
}

/// 根據 profile 字串自動判斷截面類型
pub(crate) fn detect_section_type(profile: &str) -> SteelSectionType {
    let first = profile.trim().chars().next().unwrap_or('H');
    match first {
        'C' | 'c' => SteelSectionType::C,
        'L' | 'l' => SteelSectionType::L,
        _ => SteelSectionType::H,
    }
}

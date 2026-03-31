use eframe::egui;

use crate::app::{
    compute_arc, DrawState, KolibriApp, PullFace, RenderMode, RightTab, ScaleHandle, SelectionMode, Tool,
};
use crate::camera;
use crate::scene::{MaterialKind, Shape};

impl KolibriApp {
    pub(crate) fn apply_measure(&mut self) {
        // ── Drafting: Fillet radius / Chamfer distance ──
        #[cfg(feature = "drafting")]
        if self.viewer.layout_mode {
            let input = self.editor.measure_input.trim().to_string();
            if let Ok(val) = input.parse::<f64>() {
                if val > 0.0 {
                    match self.editor.tool {
                        Tool::DraftFillet => {
                            self.editor.draft_fillet_radius = val;
                            self.file_message = Some((format!("圓角半徑: {:.1}mm", val), std::time::Instant::now()));
                            self.editor.measure_input.clear();
                            return;
                        }
                        Tool::DraftChamfer => {
                            self.editor.draft_chamfer_dist = val;
                            self.file_message = Some((format!("倒角距離: {:.1}mm", val), std::time::Instant::now()));
                            self.editor.measure_input.clear();
                            return;
                        }
                        Tool::DraftOffset => {
                            // 用於偏移距離
                            self.file_message = Some((format!("偏移距離: {:.1}mm", val), std::time::Instant::now()));
                            self.editor.measure_input.clear();
                            return;
                        }
                        _ => {}
                    }
                }
            }
        }
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

}

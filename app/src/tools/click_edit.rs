use crate::app::{
    DrawState, KolibriApp, PullFace, RightTab, ScaleHandle, Tool,
};
use crate::scene::{MaterialKind, Shape};

impl KolibriApp {
    /// 編輯工具的 on_click 處理：Move, Rotate, Scale, Offset, PushPull, FollowMe,
    /// Wall, Slab, Steel* 系列工具
    pub(crate) fn on_click_edit(&mut self) {
        match self.editor.tool {
            // Move click = select for moving (only when highlighted)
            Tool::Move => {
                if let Some(ref id) = self.editor.hovered_id.clone() {
                    self.editor.selected_ids = vec![id.clone()];
                    // Expand selection to include all group members
                    self.expand_selection_to_groups();
                    self.right_tab = RightTab::Properties;
                }
            }

            // Rotate: 3-step SU-style protractor (D1)
            Tool::Rotate => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        // Step 1: place rotation center
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

            // Offset: SketchUp-style face inset/outset
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
                    self.file_message = Some(("請點擊方塊的面，拖曳產生內縮/外擴邊框".to_string(), std::time::Instant::now()));
                }
            }

            // FollowMe: path extrusion
            Tool::FollowMe => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if !self.editor.selected_ids.is_empty() {
                            if let Some(p) = self.ground_snapped() {
                                self.editor.draw_state = DrawState::FollowPath {
                                    source_id: self.editor.selected_ids[0].clone(),
                                    path_points: vec![p],
                                };
                                self.file_message = Some(("路徑第一點已設定 — 繼續點擊加入路徑點, ESC 完成".to_string(), std::time::Instant::now()));
                            }
                        } else {
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
                            let dx = p2[0] - p1[0];
                            let dz = p2[2] - p1[2];
                            let len = (dx * dx + dz * dz).sqrt();
                            if len > 10.0 {
                                let t = self.editor.wall_thickness;
                                let h = self.editor.wall_height;
                                let nx = -dz / len * (t / 2.0);
                                let nz = dx / len * (t / 2.0);

                                self.scene.snapshot();
                                let name = self.next_name("Wall");
                                let min_x = p1[0].min(p2[0]) - nx.abs();
                                let min_z = p1[2].min(p2[2]) - nz.abs();
                                let w = (p2[0] - p1[0]).abs() + t;
                                let d = (p2[2] - p1[2]).abs() + t;
                                self.scene.add_box(name.clone(), [min_x, 0.0, min_z], w, h, d, MaterialKind::Concrete);

                                self.file_message = Some((
                                    format!("牆 {} — {:.0}mm × {:.0}mm × {:.0}mm", name, len, t, h),
                                    std::time::Instant::now(),
                                ));
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
                            let y = p1[1];

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
                    let (h_sec, b_sec, tw, tf) = super::geometry_ops::parse_h_profile(&self.editor.steel_profile);
                    let name_base = self.next_name("COL");

                    let cx = p[0];
                    let cz = p[2];

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
                        &self.current_actor, "建立柱",
                        &format!("{} H={:.0}", self.editor.steel_profile, member_h),
                        child_ids,
                    );
                    self.file_message = Some((
                        format!("柱已建立: {} @ [{:.0},{:.0}]", self.editor.steel_profile, cx, cz),
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
                            let (h_sec, b_sec, tw, tf) = super::geometry_ops::parse_h_profile(&self.editor.steel_profile);
                            let beam_y = self.editor.steel_height - h_sec;

                            let dx = p2[0] - p1[0];
                            let dz = p2[2] - p1[2];
                            let length = (dx * dx + dz * dz).sqrt();
                            let name_base = self.next_name("BM");

                            let is_x_dir = dx.abs() > dz.abs();

                            let ids = if is_x_dir {
                                let min_x = p1[0].min(p2[0]);
                                let cz = p1[2];
                                let f1 = self.scene.insert_box_raw(
                                    format!("{}_TF", name_base),
                                    [min_x, beam_y + h_sec - tf, cz - b_sec / 2.0],
                                    length, tf, b_sec, MaterialKind::Steel,
                                );
                                let f2 = self.scene.insert_box_raw(
                                    format!("{}_BF", name_base),
                                    [min_x, beam_y, cz - b_sec / 2.0],
                                    length, tf, b_sec, MaterialKind::Steel,
                                );
                                let w = self.scene.insert_box_raw(
                                    format!("{}_W", name_base),
                                    [min_x, beam_y + tf, cz - tw / 2.0],
                                    length, h_sec - 2.0 * tf, tw, MaterialKind::Steel,
                                );
                                vec![f1, f2, w]
                            } else {
                                let min_z = p1[2].min(p2[2]);
                                let cx = p1[0];
                                let f1 = self.scene.insert_box_raw(
                                    format!("{}_TF", name_base),
                                    [cx - b_sec / 2.0, beam_y + h_sec - tf, min_z],
                                    b_sec, tf, length, MaterialKind::Steel,
                                );
                                let f2 = self.scene.insert_box_raw(
                                    format!("{}_BF", name_base),
                                    [cx - b_sec / 2.0, beam_y, min_z],
                                    b_sec, tf, length, MaterialKind::Steel,
                                );
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
                                &self.current_actor, "建立梁",
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
                                format!("重複推拉 {:.0}mm", dist),
                                std::time::Instant::now(),
                            ));
                            self.editor.last_pull_click_time = std::time::Instant::now();
                        } else {
                            // Check if clicking the SAME face that's already selected → toggle off
                            let same = self.editor.selected_face.as_ref()
                                .map(|(sid, sf)| sid == id && *sf == face)
                                .unwrap_or(false);

                            if same {
                                self.editor.selected_face = None;
                            } else {
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
                        self.editor.selected_face = None;
                    }
                }
            }

            _ => {} // Not an edit tool — handled elsewhere
        }
    }
}

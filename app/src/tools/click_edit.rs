use crate::app::{
    DrawState, KolibriApp, PullFace, RightTab, ScaleHandle, Tool,
};
use crate::scene::{MaterialKind, Shape};

impl KolibriApp {
    /// 編輯工具的 on_click 處理：Move, Rotate, Scale, Offset, PushPull, FollowMe,
    /// Wall, Slab, Steel* 系列工具
    pub(crate) fn on_click_edit(&mut self) {
        match self.editor.tool {
            // Move: SU-style click-click（點擊起點 → 移動 → 點擊終點）
            Tool::Move => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        // 第一次點擊：選取物件 + 設定移動起點
                        if self.editor.selected_ids.is_empty() {
                            // 未選取時先 pick
                            if let Some(ref id) = self.editor.hovered_id.clone() {
                                self.editor.selected_ids = vec![id.clone()];
                                self.expand_selection_to_groups();
                                self.right_tab = RightTab::Properties;
                            }
                        }
                        // 有選取的物件 → 進入 MoveFrom 狀態
                        if !self.editor.selected_ids.is_empty() {
                            if let Some(from) = self.ground_snapped() {
                                let ids = self.editor.selected_ids.clone();
                                let orig_pos: Vec<[f32; 3]> = ids.iter()
                                    .map(|id| self.scene.objects.get(id).map_or([0.0; 3], |o| o.position))
                                    .collect();
                                // Ctrl held = copy mode
                                let ctrl = false; // 在 click 中不易取得 modifier，drag 模式中處理
                                let snap_ids: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
                                self.scene.snapshot_ids(&snap_ids, "移動");
                                self.editor.draw_state = DrawState::MoveFrom {
                                    from,
                                    obj_ids: ids,
                                    original_positions: orig_pos,
                                };
                            }
                        }
                    }
                    DrawState::MoveFrom { from, obj_ids, original_positions } => {
                        // 第二次點擊：確認移動終點
                        if let Some(to) = self.ground_snapped() {
                            let dx = to[0] - from[0];
                            let dy = to[1] - from[1];
                            let dz = to[2] - from[2];
                            // 套用位移（以 original_positions 為基準）
                            for (i, id) in obj_ids.iter().enumerate() {
                                if let Some(obj) = self.scene.objects.get_mut(id) {
                                    let orig = original_positions[i];
                                    obj.position = [orig[0] + dx, orig[1] + dy, orig[2] + dz];
                                }
                            }
                            self.scene.version += 1;
                            self.editor.last_move_delta = Some([dx, dy, dz]);
                            self.editor.draw_state = DrawState::Idle;
                            self.editor.drag_snapshot_taken = false;
                        }
                    }
                    _ => {}
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
                        // Step 3: 確認旋轉 — 套用 rotation_y + 繞中心旋轉位置
                        let delta = *current_angle - *ref_angle;
                        let center = *center;
                        let obj_ids = obj_ids.clone();
                        let original_rotations = original_rotations.clone();

                        for (i, id) in obj_ids.iter().enumerate() {
                            let orig_rot = original_rotations.get(i).copied().unwrap_or(0.0);
                            if let Some(obj) = self.scene.objects.get_mut(id) {
                                // 套用旋轉角
                                obj.rotation_y = orig_rot + delta;

                                // 繞中心點旋轉位置（XZ 平面）
                                let px = obj.position[0] - center[0];
                                let pz = obj.position[2] - center[2];
                                let cos_d = delta.cos();
                                let sin_d = delta.sin();
                                let new_px = px * cos_d - pz * sin_d;
                                let new_pz = px * sin_d + pz * cos_d;
                                obj.position[0] = center[0] + new_px;
                                obj.position[2] = center[2] + new_pz;
                            }
                        }
                        self.scene.version += 1;

                        let deg = delta.to_degrees();
                        self.file_message = Some((
                            format!("旋轉 {:.1}° ({} 個物件)", deg, obj_ids.len()),
                            std::time::Instant::now(),
                        ));
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
                        if let Some(p2_raw) = self.ground_snapped() {
                            let p2 = [p2_raw[0], self.editor.steel_height, p2_raw[2]];
                            let dx = p2[0] - p1[0];
                            let dy = p2[1] - p1[1];
                            let dz = p2[2] - p1[2];
                            let length = (dx * dx + dy * dy + dz * dz).sqrt();
                            if length < 10.0 { return; }

                            self.scene.snapshot();
                            let (h_sec, b_sec, tw, tf) = super::geometry_ops::parse_h_profile(&self.editor.steel_profile);
                            let name_base = self.next_name("BR");

                            // 斜撐沿主水平方向放置 H 型鋼（簡化為水平投影方向）
                            let is_x_dir = dx.abs() > dz.abs();

                            let ids = if is_x_dir {
                                let min_x = p1[0].min(p2[0]);
                                let cz = p1[2];
                                let base_y = p1[1].min(p2[1]);
                                let horiz_len = (dx * dx + dz * dz).sqrt();
                                let f1 = self.scene.insert_box_raw(
                                    format!("{}_TF", name_base),
                                    [min_x, base_y + h_sec - tf, cz - b_sec / 2.0],
                                    horiz_len, tf, b_sec, MaterialKind::Steel,
                                );
                                let f2 = self.scene.insert_box_raw(
                                    format!("{}_BF", name_base),
                                    [min_x, base_y, cz - b_sec / 2.0],
                                    horiz_len, tf, b_sec, MaterialKind::Steel,
                                );
                                let w = self.scene.insert_box_raw(
                                    format!("{}_W", name_base),
                                    [min_x, base_y + tf, cz - tw / 2.0],
                                    horiz_len, h_sec - 2.0 * tf, tw, MaterialKind::Steel,
                                );
                                vec![f1, f2, w]
                            } else {
                                let min_z = p1[2].min(p2[2]);
                                let cx = p1[0];
                                let base_y = p1[1].min(p2[1]);
                                let horiz_len = (dx * dx + dz * dz).sqrt();
                                let f1 = self.scene.insert_box_raw(
                                    format!("{}_TF", name_base),
                                    [cx - b_sec / 2.0, base_y + h_sec - tf, min_z],
                                    b_sec, tf, horiz_len, MaterialKind::Steel,
                                );
                                let f2 = self.scene.insert_box_raw(
                                    format!("{}_BF", name_base),
                                    [cx - b_sec / 2.0, base_y, min_z],
                                    b_sec, tf, horiz_len, MaterialKind::Steel,
                                );
                                let w = self.scene.insert_box_raw(
                                    format!("{}_W", name_base),
                                    [cx - tw / 2.0, base_y + tf, min_z],
                                    tw, h_sec - 2.0 * tf, horiz_len, MaterialKind::Steel,
                                );
                                vec![f1, f2, w]
                            };

                            for id in &ids {
                                if let Some(obj) = self.scene.objects.get_mut(id) {
                                    obj.component_kind = crate::collision::ComponentKind::Brace;
                                }
                            }

                            self.scene.create_group(name_base.clone(), ids.clone());
                            self.scene.version += 1;

                            self.editor.selected_ids = ids.clone();
                            self.editor.draw_state = DrawState::Idle;
                            self.ai_log.log(
                                &self.current_actor, "建立斜撐",
                                &format!("{} L={:.0}", self.editor.steel_profile, length),
                                ids,
                            );
                            self.file_message = Some((
                                format!("斜撐已建立: {} L={:.0}mm", self.editor.steel_profile, length),
                                std::time::Instant::now(),
                            ));
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

            Tool::SteelConnection | Tool::SteelEndPlate | Tool::SteelShearTab => {
                // 已選 ≥2 構件 → 開啟 AISC 對話框
                if self.editor.selected_ids.len() >= 2 {
                    self.open_connection_dialog();
                    self.editor.tool = Tool::Select;
                } else {
                    // pick 構件並追溯群組
                    let picked = self.steel_pick_member();
                    if let Some(id) = picked {
                        if !self.editor.selected_ids.contains(&id) {
                            self.editor.selected_ids.push(id);
                            self.expand_selection_to_groups();
                        }
                        if self.editor.selected_ids.len() >= 2 {
                            self.open_connection_dialog();
                            self.editor.tool = Tool::Select;
                        } else {
                            self.file_message = Some(("✓ 已選第一構件 — 再點第二構件".into(), std::time::Instant::now()));
                        }
                    }
                }
            }

            Tool::SteelBasePlate => {
                if !self.editor.selected_ids.is_empty() {
                    self.open_connection_dialog();
                    self.editor.tool = Tool::Select;
                } else {
                    let picked = self.steel_pick_member();
                    if let Some(id) = picked {
                        self.editor.selected_ids = vec![id];
                        self.expand_selection_to_groups();
                        self.open_connection_dialog();
                        self.editor.tool = Tool::Select;
                    }
                }
            }

            Tool::SteelBolt => {
                // 螺栓放置：點擊面 → 在面上配置螺栓
                let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                if let Some(id) = self.pick(mx, my, vw, vh) {
                    if let Some(p) = self.ground_snapped() {
                        self.scene.snapshot();
                        let bolt_size = self.editor.conn_bolt_size;
                        let bolt_r = bolt_size.diameter() / 2.0;
                        let head_r = bolt_size.head_across_flats() / 2.0;
                        let head_t = bolt_size.head_thickness();
                        let bolt_name = self.next_name("BOLT");

                        let shank_id = self.scene.insert_cylinder_raw(
                            format!("{}_shank", bolt_name), p, bolt_r, 50.0, 8, MaterialKind::Metal,
                        );
                        let head_id = self.scene.insert_cylinder_raw(
                            format!("{}_head", bolt_name), [p[0], p[1] + 50.0, p[2]], head_r, head_t, 8, MaterialKind::Metal,
                        );
                        for bid in [&shank_id, &head_id] {
                            if let Some(obj) = self.scene.objects.get_mut(bid) {
                                obj.component_kind = crate::collision::ComponentKind::Bolt;
                            }
                        }
                        self.scene.create_group(bolt_name, vec![shank_id.clone(), head_id.clone()]);
                        self.scene.version += 1;
                        self.file_message = Some((format!("{} 螺栓已放置", bolt_size.label()), std::time::Instant::now()));
                    }
                }
            }

            Tool::SteelWeld => {
                // 焊接標記：兩點畫焊接線
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::LineFrom { p1: p };
                            self.file_message = Some(("焊接起點 — 點擊終點".into(), std::time::Instant::now()));
                        }
                    }
                    DrawState::LineFrom { p1 } => {
                        let p1 = *p1;
                        if let Some(p2) = self.ground_snapped() {
                            self.scene.snapshot();
                            let weld_name = self.next_name("WELD");
                            let weld_size = self.editor.conn_weld_size;
                            let id = self.scene.insert_weld_line(weld_name, p1, p2, weld_size);
                            if let Some(obj) = self.scene.objects.get_mut(&id) {
                                obj.component_kind = crate::collision::ComponentKind::Weld;
                            }
                            self.scene.version += 1;
                            let length = ((p2[0]-p1[0]).powi(2) + (p2[1]-p1[1]).powi(2) + (p2[2]-p1[2]).powi(2)).sqrt();
                            self.file_message = Some((
                                format!("焊接已標記: {} L={:.0}mm S={:.0}mm",
                                    self.editor.conn_weld_type.label(), length, weld_size),
                                std::time::Instant::now(),
                            ));
                            self.editor.draw_state = DrawState::Idle;
                        }
                    }
                    _ => {}
                }
            }

            Tool::SteelStiffener => {
                // 肋板：選取柱 → 在翼板內側加肋板
                let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                if let Some(id) = self.pick(mx, my, vw, vh) {
                    if let Some(p) = self.ground_snapped() {
                        self.scene.snapshot();
                        let section = self.get_member_section(&id);
                        let (h, b, tw, tf) = section;
                        let stiff_w = (b - tw) / 2.0 - 2.0; // 翼板內淨寬
                        let stiff_h = h - 2.0 * tf; // 翼板間淨高
                        let stiff_t = tf.max(12.0_f32);
                        let name_base = self.next_name("STIFF");

                        let id = self.scene.insert_box_raw(
                            name_base, [p[0] - stiff_w / 2.0, p[1], p[2]],
                            stiff_w, stiff_h, stiff_t, MaterialKind::Metal,
                        );
                        if let Some(obj) = self.scene.objects.get_mut(&id) {
                            obj.component_kind = crate::collision::ComponentKind::Plate;
                        }
                        self.scene.version += 1;
                        self.file_message = Some(("肋板已建立".into(), std::time::Instant::now()));
                    }
                }
            }

            Tool::PushPull => {
                // SU-style PullClick: 如果已在 PullClick 狀態 → 第二次點擊確認
                if let DrawState::PullClick { ref obj_id, face, original_dim } = self.editor.draw_state.clone() {
                    // 確認推拉距離
                    let current_dim = self.scene.objects.get(obj_id.as_str()).map(|obj| {
                        match (&obj.shape, face) {
                            (Shape::Box { height, .. }, PullFace::Top | PullFace::Bottom) => *height,
                            (Shape::Box { depth, .. }, PullFace::Front | PullFace::Back) => *depth,
                            (Shape::Box { width, .. }, PullFace::Left | PullFace::Right) => *width,
                            (Shape::Cylinder { height, .. }, _) => *height,
                            _ => 0.0,
                        }
                    }).unwrap_or(0.0);
                    let dist = current_dim - original_dim;
                    self.editor.last_pull_distance = dist;
                    self.editor.last_pull_face = Some((obj_id.clone(), face));
                    self.editor.last_pull_click_time = std::time::Instant::now();
                    self.editor.draw_state = DrawState::Idle;
                    self.editor.drag_snapshot_taken = false;
                    self.editor.selected_face = None;
                    self.file_message = Some((format!("推拉 {:.0}mm", dist), std::time::Instant::now()));
                } else if matches!(self.editor.draw_state, DrawState::Idle) {
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
                            // SU-style: 點擊面 → 進入 PullClick 狀態（移動滑鼠即時預覽）
                            // 也設定 selected_face 讓拖曳模式繼續可用
                            let same = self.editor.selected_face.as_ref()
                                .map(|(sid, sf)| sid == id && *sf == face)
                                .unwrap_or(false);

                            if same {
                                self.editor.selected_face = None;
                            } else {
                                // 取得 original dim
                                let orig_dim = self.scene.objects.get(id).map(|obj| {
                                    match (&obj.shape, face) {
                                        (Shape::Box { height, .. }, PullFace::Top | PullFace::Bottom) => *height,
                                        (Shape::Box { depth, .. }, PullFace::Front | PullFace::Back) => *depth,
                                        (Shape::Box { width, .. }, PullFace::Left | PullFace::Right) => *width,
                                        (Shape::Cylinder { height, .. }, _) => *height,
                                        _ => 0.0,
                                    }
                                }).unwrap_or(0.0);
                                self.editor.selected_face = Some((id.clone(), face));
                                self.editor.selected_ids = vec![id.clone()];
                                self.right_tab = RightTab::Properties;
                                // 進入 PullClick 狀態
                                self.scene.snapshot_ids(&[id.as_str()], "推拉");
                                self.editor.pull_original_pos = self.scene.objects.get(id).map(|o| o.position);
                                self.editor.pull_original_dims = self.scene.objects.get(id).and_then(|o| {
                                    match &o.shape {
                                        Shape::Box { width, height, depth } => Some([*width, *height, *depth]),
                                        Shape::Cylinder { radius, height, .. } => Some([*radius * 2.0, *height, *radius * 2.0]),
                                        _ => None,
                                    }
                                });
                                self.editor.draw_state = DrawState::PullClick {
                                    obj_id: id.clone(), face, original_dim: orig_dim,
                                };
                                self.editor.last_pull_click_time = std::time::Instant::now();
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

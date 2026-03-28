use crate::app::{
    DrawState, KolibriApp, RightTab, SelectionMode, Tool,
};

impl KolibriApp {
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
                        let picked = self.pick(mx, my, vw, vh);
                        if let Some(ref id) = picked {
                            self.editor.selected_ids = vec![id.clone()];
                            self.clog(format!("選取邊: {}", id));
                        } else {
                            self.editor.selected_ids.clear();
                        }
                    }
                    SelectionMode::Object => {
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

            // Eraser = click to delete (only when highlighted)
            Tool::Eraser => {
                if let Some(ref id) = self.editor.hovered_id.clone() {
                    self.ai_log.log(&self.current_actor.clone(), "刪除物件", id, vec![id.clone()]);
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
                    self.scene.snapshot_ids(&[id], "材質");
                    if let Some(obj) = self.scene.objects.get_mut(id) {
                        obj.material = self.create_mat;
                        self.scene.version += 1;
                    }
                    self.file_message = Some((format!("已套用材質: {}", self.create_mat.label()), std::time::Instant::now()));
                    self.editor.recent_materials.retain(|m| m != &self.create_mat);
                    self.editor.recent_materials.insert(0, self.create_mat);
                    if self.editor.recent_materials.len() > 8 { self.editor.recent_materials.truncate(8); }
                    self.editor.selected_ids.clear();
                } else if !self.editor.selected_ids.is_empty() {
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
                        let p = if let Some(ref snap) = self.editor.snap_result {
                            if snap.snap_type != crate::app::SnapType::None {
                                snap.position
                            } else if let Some(g) = self.ground_snapped() {
                                g
                            } else { return; }
                        } else if let Some(g) = self.ground_snapped() {
                            g
                        } else { return; };

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

            // Dimension = persistent two-point annotation
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

            // Group: tag selected objects as group
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

            // Drawing tools — dispatched to click_draw.rs
            Tool::CreateBox | Tool::CreateCylinder | Tool::CreateSphere
            | Tool::Rectangle | Tool::Circle
            | Tool::Line | Tool::Arc | Tool::Arc3Point | Tool::Pie => {
                self.on_click_draw();
            }

            // Editing tools — dispatched to click_edit.rs
            Tool::Move | Tool::Rotate | Tool::Scale | Tool::Offset
            | Tool::PushPull | Tool::FollowMe
            | Tool::Wall | Tool::Slab
            | Tool::SteelColumn | Tool::SteelBeam | Tool::SteelBrace
            | Tool::SteelPlate | Tool::SteelGrid | Tool::SteelConnection => {
                self.on_click_edit();
            }
        }

        // Fallback: any click on an object selects it (like SketchUp)
        if matches!(self.editor.draw_state, DrawState::Idle) && self.editor.selected_ids.is_empty() {
            let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
            let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
            if let Some(id) = self.pick(mx, my, vw, vh) {
                self.editor.selected_ids = vec![id];
                self.expand_selection_to_groups();
                self.right_tab = RightTab::Properties;
            }
        }

        self.editor.measure_input.clear();
    }
}

use crate::app::{
    DrawState, KolibriApp, RightTab, Tool,
};

impl KolibriApp {
    /// 繪圖工具的 on_click 處理：CreateBox, CreateCylinder, CreateSphere, Rectangle, Circle,
    /// Line, Arc, Arc3Point, Pie
    pub(crate) fn on_click_draw(&mut self) {
        match self.editor.tool {
            Tool::CreateBox => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::BoxBase { p1: p };
                        }
                    }
                    DrawState::BoxBase { p1 } => {
                        if let Some(p2) = self.ground_snapped() {
                            let p1 = *p1;
                            self.editor.draw_state = DrawState::BoxHeight { p1, p2 };
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
                        let id = self.scene.add_box(name.clone(), [x0, 0.0, z0], w, h, d, self.create_mat);
                        self.console_push("ACTION", format!("建立方塊 {} ({:.0}x{:.0}x{:.0}) mat={}", name, w, h, d, self.create_mat.label()));
                        // Collision check on creation (warning only)
                        {
                            let components = crate::scene::scene_to_collision_components(&self.scene);
                            let col_center = [x0 + w / 2.0, h / 2.0, z0 + d / 2.0];
                            let new_comp = crate::collision::Component::new(id.clone(), crate::collision::ComponentKind::Generic, col_center, [w, h, d]);
                            let report = crate::collision::can_place_component(&new_comp, &components, &crate::collision::CollisionConfig::default());
                            if !report.is_allowed || !report.warning_pairs.is_empty() {
                                self.editor.collision_warning = Some("放置位置與現有物件碰撞".to_string());
                            }
                        }
                        self.ai_log.log(&self.current_actor.clone(), "建立方塊", &format!("{:.0}×{:.0}×{:.0}", w, h, d), vec![id.clone()]);
                        self.editor.selected_ids = vec![id.clone()];
                        self.editor.draw_state = DrawState::Idle;
                        self.right_tab = RightTab::Properties;

                        // Check for alignment suggestion
                        self.check_alignment_suggestion(&id);
                    }
                    _ => {}
                }
            }

            Tool::CreateCylinder => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::CylBase { center: p };
                        }
                    }
                    DrawState::CylBase { center } => {
                        let c = *center;
                        if let Some(mouse) = self.ground_snapped() {
                            let r = ((mouse[0]-c[0]).powi(2)+(mouse[2]-c[2]).powi(2)).sqrt().max(10.0);
                            self.editor.draw_state = DrawState::CylHeight { center: c, radius: r };
                        }
                    }
                    DrawState::CylHeight { center, radius } => {
                        let c = *center;
                        let r = *radius;
                        let h = self.current_height(c).max(10.0);
                        let name = self.next_name("Cylinder");
                        let id = self.scene.add_cylinder(name.clone(), c, r, h, 48, self.create_mat);
                        self.console_push("ACTION", format!("建立圓柱 {} (r={:.0} h={:.0}) mat={}", name, r, h, self.create_mat.label()));
                        // Collision check on creation (warning only)
                        {
                            let components = crate::scene::scene_to_collision_components(&self.scene);
                            let col_center = [c[0] + r, c[1] + h / 2.0, c[2] + r];
                            let new_comp = crate::collision::Component::new(id.clone(), crate::collision::ComponentKind::Generic, col_center, [r * 2.0, h, r * 2.0]);
                            let report = crate::collision::can_place_component(&new_comp, &components, &crate::collision::CollisionConfig::default());
                            if !report.is_allowed || !report.warning_pairs.is_empty() {
                                self.editor.collision_warning = Some("放置位置與現有物件碰撞".to_string());
                            }
                        }
                        self.ai_log.log(&self.current_actor.clone(), "建立圓柱", &format!("r={:.0} h={:.0}", r, h), vec![id.clone()]);
                        self.editor.selected_ids = vec![id];
                        self.editor.draw_state = DrawState::Idle;
                        self.right_tab = RightTab::Properties;
                    }
                    _ => {}
                }
            }

            Tool::CreateSphere => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::SphRadius { center: p };
                        }
                    }
                    DrawState::SphRadius { center } => {
                        let c = *center;
                        if let Some(mouse) = self.ground_snapped() {
                            let r = ((mouse[0]-c[0]).powi(2)+(mouse[2]-c[2]).powi(2)).sqrt().max(10.0);
                            let name = self.next_name("Sphere");
                            let id = self.scene.add_sphere(name.clone(), c, r, 32, self.create_mat);
                            self.console_push("ACTION", format!("建立球體 {} (r={:.0}) mat={}", name, r, self.create_mat.label()));
                            // Collision check on creation (warning only)
                            {
                                let components = crate::scene::scene_to_collision_components(&self.scene);
                                let col_center = [c[0] + r, c[1] + r, c[2] + r];
                                let new_comp = crate::collision::Component::new(id.clone(), crate::collision::ComponentKind::Generic, col_center, [r * 2.0, r * 2.0, r * 2.0]);
                                let report = crate::collision::can_place_component(&new_comp, &components, &crate::collision::CollisionConfig::default());
                                if !report.is_allowed || !report.warning_pairs.is_empty() {
                                    self.editor.collision_warning = Some("放置位置與現有物件碰撞".to_string());
                                }
                            }
                            self.ai_log.log(&self.current_actor.clone(), "建立球體", &format!("r={:.0}", r), vec![id.clone()]);
                            self.editor.selected_ids = vec![id];
                            self.editor.draw_state = DrawState::Idle;
                            self.right_tab = RightTab::Properties;
                        }
                    }
                    _ => {}
                }
            }

            // Rectangle = 2D flat rectangle only (use Push/Pull to extrude)
            Tool::Rectangle => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::BoxBase { p1: p };
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
                            self.editor.selected_ids = vec![id];
                            self.editor.draw_state = DrawState::Idle;
                            self.right_tab = RightTab::Properties;
                            self.editor.tool = Tool::PushPull; // auto-switch to Push/Pull
                            self.file_message = Some(("矩形已建立 — 用推拉拉出高度".to_string(), std::time::Instant::now()));
                        }
                    }
                    _ => {}
                }
            }

            // Circle = same as CreateCylinder flow
            Tool::Circle => {
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = self.ground_snapped() {
                            self.editor.draw_state = DrawState::CylBase { center: p };
                        }
                    }
                    DrawState::CylBase { center } => {
                        let c = *center;
                        if let Some(mouse) = self.ground_snapped() {
                            let r = ((mouse[0]-c[0]).powi(2)+(mouse[2]-c[2]).powi(2)).sqrt().max(10.0);
                            self.editor.draw_state = DrawState::CylHeight { center: c, radius: r };
                        }
                    }
                    DrawState::CylHeight { center, radius } => {
                        let (c, r) = (*center, *radius);
                        let h = self.current_height(c).max(10.0);
                        let name = self.next_name("Circle");
                        let id = self.scene.add_cylinder(name, c, r, h, 48, self.create_mat);
                        self.editor.selected_ids = vec![id];
                        self.editor.draw_state = DrawState::Idle;
                        self.right_tab = RightTab::Properties;
                    }
                    _ => {}
                }
            }

            // Line: click-click chain drawing → adds edges to the shared free mesh
            Tool::Line => {
                let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        // Try face snap first, then fall back to ground
                        let pos = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(p) = pos {
                            self.editor.draw_state = DrawState::LineFrom { p1: p };
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
                                    format!("\u{2713} 偵測到 {} 個新面！可用推拉工具拉伸", new_faces),
                                    std::time::Instant::now(),
                                ));
                            }

                            self.scene.version += 1;

                            // Store last drawn edge direction for perpendicular/parallel inference
                            let edge_dir = [p2[0] - p1[0], p2[2] - p1[2]];
                            let edge_len = (edge_dir[0] * edge_dir[0] + edge_dir[1] * edge_dir[1]).sqrt();
                            if edge_len > 1.0 {
                                self.editor.last_line_dir = Some([edge_dir[0] / edge_len, edge_dir[1] / edge_len]);
                            }

                            // Inference 2.0: update context after line drawn
                            crate::inference::update_context_after_line(&mut self.editor.inference_ctx, p1, p2);

                            // Try to split a box face if line lies on it
                            self.try_split_face(p1, p2);

                            // Also try crossing-based split (line crosses box footprint)
                            self.try_split_face_on_line(p1, p2);

                            // Chain: start new line from p2
                            self.editor.draw_state = DrawState::LineFrom { p1: p2 };
                        }
                    }
                    _ => {}
                }
            }

            // Arc: 3-click (start, end, bulge)
            Tool::Arc => {
                let (mx, my) = (self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                let (vw, vh) = (self.viewer.viewport_size[0], self.viewer.viewport_size[1]);
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        let pos = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(p) = pos {
                            self.editor.draw_state = DrawState::ArcP1 { p1: p };
                            self.clog(format!("圓弧: 起點 [{:.0}, {:.0}, {:.0}]", p[0], p[1], p[2]));
                        }
                    }
                    DrawState::ArcP1 { p1 } => {
                        let p1 = *p1;
                        let pos = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(p2) = pos {
                            let chord = ((p2[0]-p1[0]).powi(2) + (p2[2]-p1[2]).powi(2)).sqrt();
                            self.clog(format!("圓弧: 終點 [{:.0}, {:.0}, {:.0}] 弦長={:.0}mm", p2[0], p2[1], p2[2], chord));
                            self.editor.draw_state = DrawState::ArcP2 { p1, p2 };
                        }
                    }
                    DrawState::ArcP2 { p1, p2 } => {
                        let (p1, p2) = (*p1, *p2);
                        let pos = self.snap_to_face(mx, my, vw, vh)
                            .or_else(|| self.ground_snapped());
                        if let Some(mut p3) = pos {
                            // 半圓鎖定：凸度點接近弦長中垂線上的半圓位置時自動吸附
                            if let Some(ref info) = crate::app::compute_arc_info(p1, p2, p3) {
                                if info.is_semicircle() {
                                    // 強制半圓：調整 p3 到精確半圓位置
                                    let mid = [(p1[0]+p2[0])*0.5, (p1[1]+p2[1])*0.5, (p1[2]+p2[2])*0.5];
                                    let chord_dir = [p2[0]-p1[0], p2[1]-p1[1], p2[2]-p1[2]];
                                    let chord_len = (chord_dir[0]*chord_dir[0] + chord_dir[2]*chord_dir[2]).sqrt();
                                    let perp = [-chord_dir[2] / chord_len, 0.0, chord_dir[0] / chord_len];
                                    let r = chord_len * 0.5;
                                    let sign = if (p3[0]-mid[0])*perp[0] + (p3[2]-mid[2])*perp[2] > 0.0 { 1.0 } else { -1.0 };
                                    p3 = [mid[0] + perp[0]*r*sign, mid[1], mid[2] + perp[2]*r*sign];
                                    self.clog("圓弧: 半圓鎖定!".to_string());
                                }
                            }

                            let seg = 32_usize; // TODO: 可調細分數
                            if let Some(info) = crate::app::compute_arc_info(p1, p2, p3) {
                                let arc_pts = info.points(seg);
                                let name = self.next_name("Arc");
                                let id = self.scene.add_line(name.clone(), arc_pts, 20.0, self.create_mat);
                                // 儲存圓弧資訊到 Shape::Line
                                if let Some(obj) = self.scene.objects.get_mut(&id) {
                                    if let crate::scene::Shape::Line { ref mut arc_center, ref mut arc_radius, ref mut arc_angle_deg, .. } = obj.shape {
                                        *arc_center = Some(info.center);
                                        *arc_radius = Some(info.radius);
                                        *arc_angle_deg = Some(info.sweep_degrees());
                                    }
                                }
                                self.console_push("ACTION", format!(
                                    "建立圓弧 {} R={:.0}mm 角度={:.1}° 弧長={:.0}mm",
                                    name, info.radius, info.sweep_degrees(), info.arc_length()
                                ));
                                self.editor.selected_ids = vec![id];
                            } else {
                                // 退回直線
                                let name = self.next_name("Line");
                                let id = self.scene.add_line(name, vec![p1, p2], 20.0, self.create_mat);
                                self.editor.selected_ids = vec![id];
                                self.console_push("WARN", "圓弧: 三點共線，建立直線".to_string());
                            }
                            self.editor.draw_state = DrawState::Idle;
                        }
                    }
                    _ => {}
                }
            }

            // Arc3Point: 任意三點定圓弧（等同 Arc 但語意更清楚）
            Tool::Arc3Point => {
                let pos = self.snap_to_face(self.editor.mouse_screen[0], self.editor.mouse_screen[1],
                    self.viewer.viewport_size[0], self.viewer.viewport_size[1])
                    .or_else(|| self.ground_snapped());
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = pos {
                            self.editor.draw_state = DrawState::ArcP1 { p1: p };
                            self.clog(format!("三點圓弧: P1 [{:.0}, {:.0}, {:.0}]", p[0], p[1], p[2]));
                        }
                    }
                    DrawState::ArcP1 { p1 } => {
                        let p1 = *p1;
                        if let Some(p2) = pos {
                            self.editor.draw_state = DrawState::ArcP2 { p1, p2 };
                            self.clog(format!("三點圓弧: P2 [{:.0}, {:.0}, {:.0}]", p2[0], p2[1], p2[2]));
                        }
                    }
                    DrawState::ArcP2 { p1, p2 } => {
                        let (p1, p2) = (*p1, *p2);
                        if let Some(p3) = pos {
                            if let Some(info) = crate::app::compute_arc_info(p1, p2, p3) {
                                let arc_pts = info.points(32);
                                let name = self.next_name("Arc3P");
                                let id = self.scene.add_line(name.clone(), arc_pts, 20.0, self.create_mat);
                                if let Some(obj) = self.scene.objects.get_mut(&id) {
                                    if let crate::scene::Shape::Line { ref mut arc_center, ref mut arc_radius, ref mut arc_angle_deg, .. } = obj.shape {
                                        *arc_center = Some(info.center);
                                        *arc_radius = Some(info.radius);
                                        *arc_angle_deg = Some(info.sweep_degrees());
                                    }
                                }
                                self.console_push("ACTION", format!("建立三點圓弧 {} R={:.0} {:.1}°", name, info.radius, info.sweep_degrees()));
                                self.editor.selected_ids = vec![id];
                            }
                            self.editor.draw_state = DrawState::Idle;
                        }
                    }
                    _ => {}
                }
            }

            // Pie: 扇形工具 — 中心→邊緣定半徑→第二邊緣定角度
            Tool::Pie => {
                let pos = self.ground_snapped();
                match &self.editor.draw_state {
                    DrawState::Idle => {
                        if let Some(p) = pos {
                            self.editor.draw_state = DrawState::PieCenter { center: p };
                            self.clog(format!("扇形: 中心 [{:.0}, {:.0}, {:.0}]", p[0], p[1], p[2]));
                        }
                    }
                    DrawState::PieCenter { center } => {
                        let c = *center;
                        if let Some(e1) = pos {
                            let r = ((e1[0]-c[0]).powi(2) + (e1[2]-c[2]).powi(2)).sqrt();
                            if r > 10.0 {
                                self.editor.draw_state = DrawState::PieRadius { center: c, edge1: e1 };
                                self.clog(format!("扇形: 半徑={:.0}mm", r));
                            }
                        }
                    }
                    DrawState::PieRadius { center, edge1 } => {
                        let c = *center;
                        let e1 = *edge1;
                        if let Some(e2) = pos {
                            let r = ((e1[0]-c[0]).powi(2) + (e1[2]-c[2]).powi(2)).sqrt();
                            let a1 = (e1[2]-c[2]).atan2(e1[0]-c[0]);
                            let a2 = (e2[2]-c[2]).atan2(e2[0]-c[0]);
                            let mut sweep = a2 - a1;
                            if sweep < 0.0 { sweep += std::f32::consts::TAU; }

                            // 生成扇形邊緣弧線 + 兩條半徑線
                            let seg = 32;
                            let mut pts = vec![c]; // 中心
                            for i in 0..=seg {
                                let t = i as f32 / seg as f32;
                                let a = a1 + sweep * t;
                                pts.push([c[0] + r * a.cos(), c[1], c[2] + r * a.sin()]);
                            }
                            pts.push(c); // 閉合回中心

                            let name = self.next_name("Pie");
                            let id = self.scene.add_line(name.clone(), pts, 20.0, self.create_mat);
                            if let Some(obj) = self.scene.objects.get_mut(&id) {
                                if let crate::scene::Shape::Line { ref mut arc_center, ref mut arc_radius, ref mut arc_angle_deg, .. } = obj.shape {
                                    *arc_center = Some(c);
                                    *arc_radius = Some(r);
                                    *arc_angle_deg = Some(sweep.to_degrees());
                                }
                            }
                            self.console_push("ACTION", format!("建立扇形 {} R={:.0} {:.1}°", name, r, sweep.to_degrees()));
                            self.editor.selected_ids = vec![id];
                            self.editor.draw_state = DrawState::Idle;
                        }
                    }
                    _ => {}
                }
            }

            _ => {} // Not a draw tool — handled elsewhere
        }
    }
}

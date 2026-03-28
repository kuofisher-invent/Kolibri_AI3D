use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use eframe::{egui, wgpu};
use eframe::epaint::mutex::RwLock;
use serde::Serialize;

use crate::camera::{self, OrbitCamera};
use crate::renderer::ViewportRenderer;
use crate::scene::{MaterialKind, Scene, Shape};
use crate::app::{KolibriApp, Tool, WorkMode, DrawState, ScaleHandle, PullFace, SnapType, SnapResult, AiSuggestion, SuggestionAction, RightTab, CursorHint, EditorState, SelectionMode, RenderMode, ViewerState, BackgroundTaskResult, BackgroundSceneBuild, SpatialEntry};

// ─── eframe::App ─────────────────────────────────────────────────────────────

impl eframe::App for KolibriApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Match panel fill so no dark gap shows between panels
        [245.0 / 255.0, 246.0 / 255.0, 250.0 / 255.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Keep a light heartbeat so deferred startup work and file-based automation
        // still progress when the scene is otherwise idle.
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        // Auto-start MCP HTTP bridge（操作 APP 的 GUI 場景）
        if !self.mcp_http_running && !self.editor.recovery_checked {
            let auto_mcp = std::env::args().any(|a| a == "--auto-mcp")
                || std::env::var("KOLIBRI_AUTO_MCP").is_ok();
            if auto_mcp {
                let port = self.mcp_http_port;
                let bridge = crate::mcp_http_bridge::create_bridge_and_start_http(port);
                self.mcp_bridge = Some(bridge);
                self.mcp_http_running = true;
            }
        }
        self.poll_background_task();
        if self.background_task_active() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        // ── Camera smooth transition animation ──
        if self.viewer.tick_camera_anim() {
            ctx.request_repaint(); // 動畫進行中，持續重繪
        }
        if !self.startup_scene_attempted && self.startup_scene_path.is_some() {
            self.try_load_startup_scene();
        }
        if self.startup_scene_attempted
            && self.startup_screenshot_path.is_some()
            && !self.startup_screenshot_completed
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        // ── Crash recovery: 啟動時檢查 autosave ──
        if !self.editor.recovery_checked {
            self.editor.recovery_checked = true;
            let auto_path = "autosave.k3d";
            if self.startup_scene_path.is_none()
                && self.scene.objects.is_empty()
                && std::path::Path::new(auto_path).exists()
            {
                if let Ok(n) = self.scene.load_from_file(auto_path) {
                    if n > 0 {
                        self.toasts.push((format!("已從自動儲存恢復 {} 個物件", n), std::time::Instant::now()));
                        self.file_message = Some((format!("已恢復自動儲存 ({} 物件)", n), std::time::Instant::now()));
                    }
                }
            }
        }

        // 深色模式切換（每幀檢查，因為使用者可能隨時切換）
        if self.viewer.dark_mode != ctx.style().visuals.dark_mode {
            if self.viewer.dark_mode {
                ctx.set_visuals(egui::Visuals::dark());
            } else {
                // 還原淺色 glassmorphism 主題
                let mut v = egui::Visuals::light();
                v.panel_fill = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220);
                v.window_fill = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 240);
                v.extreme_bg_color = egui::Color32::WHITE;
                v.faint_bg_color = egui::Color32::from_rgb(248, 249, 252);
                v.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(76, 139, 245, 36);
                v.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 139, 245));
                ctx.set_visuals(v);
            }
        }

        // ── Command Palette (Ctrl+P) ──
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::P)) {
            self.editor.command_palette_open = !self.editor.command_palette_open;
            self.editor.command_palette_query.clear();
        }
        if self.editor.command_palette_open {
            let commands: &[(&str, &str)] = &[
                ("建立方塊", "B"), ("建立圓柱", ""), ("建立球體", ""),
                ("選取工具", "Space"), ("移動工具", "M"), ("旋轉工具", "Q"),
                ("縮放工具", "S"), ("線段工具", "L"), ("弧線工具", "A"),
                ("矩形工具", "R"), ("圓形工具", "C"), ("推拉工具", "P"),
                ("偏移工具", "F"), ("量尺工具", "T"), ("標註工具", "D"),
                ("橡皮擦", "E"), ("軌道瀏覽", "O"), ("平移瀏覽", "H"),
                ("全部顯示", "Z"), ("群組工具", "G"),
                ("復原", "Ctrl+Z"), ("重做", "Ctrl+Y"),
                ("儲存", "Ctrl+S"), ("開啟", "Ctrl+O"),
                ("複製", "Ctrl+C"), ("貼上", "Ctrl+V"), ("剪下", "Ctrl+X"),
                ("全選", "Ctrl+A"),
                ("切換線框", ""), ("切換X光", ""), ("切換草稿", ""),
                ("深色模式", ""), ("顯示格線", ""),
                ("匯出 OBJ", ""), ("匯出 STL", ""), ("匯出 DXF", ""),
                ("匯入 OBJ", ""), ("匯入 DXF", ""),
                ("清空場景", ""), ("MCP Server", ""),
                ("牆工具", "W"), ("板工具", ""),
                ("就地複製", "Ctrl+D"), ("反轉選取", "Ctrl+I"),
                ("鏡射 X", "Ctrl+M"),
                ("對齊左", ""), ("對齊右", ""), ("對齊上", ""), ("對齊下", ""),
                ("X中心對齊", ""), ("Y中心對齊", ""), ("Z中心對齊", ""),
                ("X等距分佈", ""), ("Y等距分佈", ""), ("Z等距分佈", ""),
                ("CSG 聯集", ""), ("CSG 差集", ""), ("CSG 交集", ""),
                ("隱藏選取", "Alt+H"), ("顯示全部", "Alt+Shift+H"), ("隔離顯示", "Alt+I"),
                ("複製屬性", "Ctrl+Shift+C"), ("貼上屬性", "Ctrl+Shift+V"),
            ];
            let mut close = false;
            egui::Area::new(egui::Id::new("command_palette"))
                .fixed_pos(egui::pos2(ctx.screen_rect().center().x - 200.0, 80.0))
                .show(ctx, |ui| {
                    let frame = egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(30, 32, 48, 240))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 139, 245)))
                        .rounding(12.0)
                        .inner_margin(egui::Margin::same(8.0));
                    frame.show(ui, |ui| {
                        ui.set_min_width(400.0);
                        let resp = ui.add(egui::TextEdit::singleline(&mut self.editor.command_palette_query)
                            .hint_text("搜尋指令...")
                            .desired_width(384.0)
                            .font(egui::FontId::proportional(14.0)));
                        if !resp.has_focus() { resp.request_focus(); }
                        ui.add_space(4.0);

                        let query = self.editor.command_palette_query.to_lowercase();
                        let mut shown = 0;
                        for (name, shortcut) in commands {
                            if !query.is_empty() && !name.to_lowercase().contains(&query) { continue; }
                            if shown >= 12 { break; }
                            shown += 1;
                            ui.horizontal(|ui| {
                                if ui.add(egui::Label::new(
                                    egui::RichText::new(*name).size(13.0).color(egui::Color32::from_rgb(220, 225, 240))
                                ).sense(egui::Sense::click())).clicked() {
                                    // 執行對應指令
                                    self.execute_command_by_name(name);
                                    close = true;
                                }
                                if !shortcut.is_empty() {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new(*shortcut).size(10.0).color(egui::Color32::from_rgb(120, 130, 150)));
                                    });
                                }
                            });
                        }

                        // ESC 關閉
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { close = true; }
                    });
                });
            if close {
                self.editor.command_palette_open = false;
                self.editor.command_palette_query.clear();
            }
        }

        // Dynamic window title
        let title = if let Some(ref path) = self.current_file {
            let filename = path.rsplit(['\\', '/']).next().unwrap_or(path);
            if self.has_unsaved_changes() {
                format!("Kolibri_Ai3D \u{2014} {}*", filename)
            } else {
                format!("Kolibri_Ai3D \u{2014} {}", filename)
            }
        } else {
            if self.scene.objects.is_empty() {
                "Kolibri_Ai3D".to_string()
            } else {
                "Kolibri_Ai3D \u{2014} \u{672a}\u{5132}\u{5b58}\u{5834}\u{666f}*".to_string()
            }
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        // Poll MCP bridge commands
        if let Some(bridge) = self.mcp_bridge.take() {
            while let Ok((cmd, result_tx)) = bridge.cmd_rx.try_recv() {
                let result = self.handle_mcp_command(cmd);
                let _ = result_tx.send(result);
            }
            self.mcp_bridge = Some(bridge);
        }

        // ── UI panels (top bar, left toolbar, right panel, console, status bar) ──
        self.draw_panels(ctx);

        // Handle drag-and-drop files
        self.handle_dropped_files(ctx);

        // Viewport
        egui::CentralPanel::default()
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(245, 246, 250))
                .inner_margin(egui::Margin::same(0.0)))
            .show(ctx, |ui| {
                // ── Layout mode: 2D paper view ──
                if self.viewer.layout_mode {
                    let avail = ui.available_size();
                    let (rect, _response) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
                    crate::layout::draw_layout(ui, &self.viewer.layout, rect);
                    return;
                }

                let avail = ui.available_size();
                let w = (avail.x.ceil() as u32).max(1);
                let h = (avail.y.ceil() as u32).max(1);

                { let mut r = self.egui_renderer.write(); self.viewport.ensure_size(&self.device, &mut r, w, h); }

                // Sync layer visibility from hidden_tags
                for obj in self.scene.objects.values_mut() {
                    obj.visible = !self.viewer.hidden_tags.contains(&obj.tag);
                }

                // 先分配區域取得 response，再處理互動，最後渲染
                // 這確保點擊/材質變更在渲染前生效（同幀即時反映）
                let (rect, response) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
                self.handle_viewport(&response, ui);

                let preview = self.build_preview();
                let aspect = w as f32 / h.max(1) as f32;
                let vp = if self.viewer.use_ortho {
                    self.viewer.camera.proj_ortho(aspect) * self.viewer.camera.view()
                } else {
                    self.viewer.camera.view_proj(aspect)
                };
                let hf = self.editor.hovered_face.as_ref().map(|(id, face)| (id.as_str(), face.as_u8()));
                let sf = self.editor.selected_face.as_ref().map(|(id, face)| (id.as_str(), face.as_u8()));
                // Adaptive grid: subdivide when zoomed in, coarsen when zoomed out
                let cam_dist = self.viewer.camera.distance;
                let base_spacing = self.viewer.grid_spacing;
                let effective_grid_spacing = if cam_dist < base_spacing * 3.0 {
                    base_spacing / 10.0 // fine grid when close
                } else if cam_dist < base_spacing * 10.0 {
                    base_spacing / 2.0  // medium grid
                } else if cam_dist > base_spacing * 50.0 {
                    base_spacing * 5.0  // coarse grid when far
                } else {
                    base_spacing
                };
                let section_plane = if self.viewer.section_plane_enabled {
                    [self.viewer.section_plane_axis as f32, self.viewer.section_plane_offset, if self.viewer.section_plane_flip { 1.0 } else { 0.0 }, 1.0]
                } else {
                    [0.0, 0.0, 0.0, 0.0]
                };
                self.viewport.render(&self.device, &self.queue, vp, &self.scene, &self.editor.selected_ids, self.editor.hovered_id.as_deref(), self.editor.editing_group_id.as_deref(), self.editor.editing_component_def_id.as_deref(), &preview, self.viewer.render_mode.as_u32(), self.viewer.sky_color, self.viewer.ground_color, hf, sf, self.viewer.edge_thickness, self.viewer.show_colors, &self.texture_manager, self.viewer.show_grid, effective_grid_spacing, section_plane);

                if let Some(tex_id) = self.viewport.texture_id {
                    ui.painter().image(tex_id, rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                }

                // Draw rubber band selection rectangle
                if let Some((start, end)) = self.editor.rubber_band {
                    let rb_rect = egui::Rect::from_two_pos(start, end);
                    let is_crossing = start.x > end.x;
                    let painter = ui.painter();
                    // Window = 實線藍色，Crossing = 虛線綠色
                    let (fill_color, stroke_color) = if is_crossing {
                        (egui::Color32::from_rgba_unmultiplied(60, 200, 80, 30),
                         egui::Color32::from_rgb(60, 200, 80))
                    } else {
                        (egui::Color32::from_rgba_unmultiplied(60, 120, 220, 40),
                         egui::Color32::from_rgb(80, 140, 240))
                    };
                    painter.rect_filled(rb_rect, 0.0, fill_color);
                    if is_crossing {
                        // 虛線邊框
                        let corners = [rb_rect.left_top(), rb_rect.right_top(), rb_rect.right_bottom(), rb_rect.left_bottom()];
                        for i in 0..4 {
                            crate::overlay::draw_dashed_line(painter, corners[i], corners[(i+1)%4],
                                egui::Stroke::new(1.5, stroke_color), 6.0, 4.0);
                        }
                    } else {
                        painter.rect_stroke(rb_rect, 0.0, egui::Stroke::new(1.5, stroke_color));
                    }
                }

                // Draw snap indicators on top of viewport
                if let Some(ref snap) = self.editor.snap_result {
                    if snap.snap_type != SnapType::None && snap.snap_type != SnapType::Grid {
                        let painter = ui.painter();

                        // Draw snap point indicator — shape varies by snap type (SketchUp-style)
                        if let Some(screen_pos) = self.world_to_screen(snap.position, &rect) {
                            let color = snap.snap_type.color();
                            let sx = screen_pos.x;
                            let sy = screen_pos.y;

                            match snap.snap_type {
                                SnapType::Endpoint => {
                                    // Green diamond
                                    let diamond = vec![
                                        egui::pos2(sx, sy - 7.0),
                                        egui::pos2(sx + 7.0, sy),
                                        egui::pos2(sx, sy + 7.0),
                                        egui::pos2(sx - 7.0, sy),
                                    ];
                                    painter.add(egui::Shape::convex_polygon(diamond, color, egui::Stroke::new(1.5, egui::Color32::WHITE)));
                                }
                                SnapType::Midpoint => {
                                    // Cyan triangle
                                    let tri = vec![
                                        egui::pos2(sx, sy - 7.0),
                                        egui::pos2(sx + 6.0, sy + 5.0),
                                        egui::pos2(sx - 6.0, sy + 5.0),
                                    ];
                                    painter.add(egui::Shape::convex_polygon(tri, color, egui::Stroke::new(1.5, egui::Color32::WHITE)));
                                }
                                SnapType::OnFace => {
                                    // Blue filled circle
                                    painter.circle_filled(screen_pos, 6.0, color);
                                    painter.circle_stroke(screen_pos, 6.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
                                }
                                SnapType::Intersection => {
                                    // X marker
                                    let sz = 7.0;
                                    painter.line_segment(
                                        [screen_pos + egui::vec2(-sz, -sz), screen_pos + egui::vec2(sz, sz)],
                                        egui::Stroke::new(2.5, color),
                                    );
                                    painter.line_segment(
                                        [screen_pos + egui::vec2(-sz, sz), screen_pos + egui::vec2(sz, -sz)],
                                        egui::Stroke::new(2.5, color),
                                    );
                                }
                                SnapType::FaceCenter => {
                                    // Cross (+) indicator for face center
                                    let sz = 6.0;
                                    painter.line_segment(
                                        [egui::pos2(sx - sz, sy), egui::pos2(sx + sz, sy)],
                                        egui::Stroke::new(2.0, color),
                                    );
                                    painter.line_segment(
                                        [egui::pos2(sx, sy - sz), egui::pos2(sx, sy + sz)],
                                        egui::Stroke::new(2.0, color),
                                    );
                                }
                                SnapType::OnEdge => {
                                    // Red diamond (match SketchUp)
                                    let edge_color = egui::Color32::from_rgb(220, 50, 50);
                                    let diamond = vec![
                                        egui::pos2(sx, sy - 5.0),
                                        egui::pos2(sx + 5.0, sy),
                                        egui::pos2(sx, sy + 5.0),
                                        egui::pos2(sx - 5.0, sy),
                                    ];
                                    painter.add(egui::Shape::convex_polygon(diamond, edge_color, egui::Stroke::NONE));
                                }
                                _ => {
                                    // Default: circle indicator
                                    painter.circle_stroke(screen_pos, 12.0, egui::Stroke::new(2.5, color));
                                    painter.circle_filled(screen_pos, 5.0, color);
                                }
                            }

                            // (Old combined label removed — now displayed in cursor hint card)
                        }

                        // Draw axis / parallel / perpendicular inference line from origin to snap point
                        if let Some(from) = snap.from_point {
                            if matches!(snap.snap_type, SnapType::AxisX | SnapType::AxisZ
                                        | SnapType::Parallel | SnapType::Perpendicular) {
                                if let (Some(from_s), Some(to_s)) = (
                                    self.world_to_screen(from, &rect),
                                    self.world_to_screen(snap.position, &rect),
                                ) {
                                    let color = snap.snap_type.color();
                                    // Draw dashed line (thicker stroke)
                                    let dir = to_s - from_s;
                                    let len = dir.length();
                                    if len > 1.0 {
                                        let step = 8.0;
                                        let norm = dir / len;
                                        let mut d = 0.0;
                                        while d < len {
                                            let a = from_s + norm * d;
                                            let b = from_s + norm * (d + step * 0.6).min(len);
                                            painter.line_segment([a, b], egui::Stroke::new(2.0, color));
                                            d += step;
                                        }
                                    }
                                }
                            }
                        }

                        // Show distance from start point when drawing (B3: distance labels on snap lines)
                        if let Some(from) = snap.from_point {
                            if let (Some(from_s), Some(to_s)) = (
                                self.world_to_screen(from, &rect),
                                self.world_to_screen(snap.position, &rect),
                            ) {
                                let dx = snap.position[0] - from[0];
                                let dz = snap.position[2] - from[2];
                                let dist = (dx * dx + dz * dz).sqrt();
                                if dist > 1.0 {
                                    let mid = egui::pos2(
                                        (from_s.x + to_s.x) * 0.5,
                                        (from_s.y + to_s.y) * 0.5 - 15.0,
                                    );
                                    let dist_text = if dist >= 1000.0 {
                                        format!("{:.2} m", dist / 1000.0)
                                    } else {
                                        format!("{:.0} mm", dist)
                                    };
                                    // Background rectangle for readability
                                    let font = egui::FontId::proportional(13.0);
                                    let galley = painter.layout_no_wrap(dist_text, font, egui::Color32::from_rgb(255, 255, 200));
                                    let bg_rect = egui::Rect::from_center_size(mid, galley.size()).expand(3.0);
                                    painter.rect_filled(bg_rect, 2.0, egui::Color32::from_rgba_unmultiplied(30, 30, 40, 200));
                                    painter.galley(bg_rect.min, galley, egui::Color32::from_rgb(255, 255, 200));

                                    // Also show axis-decomposed distances for non-axis-aligned snaps
                                    if !matches!(snap.snap_type, SnapType::AxisX | SnapType::AxisZ) {
                                        let adx = dx.abs();
                                        let adz = dz.abs();
                                        if adx > 50.0 && adz > 50.0 {
                                            let x_text = if adx >= 1000.0 { format!("X: {:.2} m", adx / 1000.0) }
                                                         else { format!("X: {:.0} mm", adx) };
                                            let z_text = if adz >= 1000.0 { format!("Z: {:.2} m", adz / 1000.0) }
                                                         else { format!("Z: {:.0} mm", adz) };
                                            let detail_pos = mid + egui::vec2(0.0, 16.0);
                                            let detail = format!("{} | {}", x_text, z_text);
                                            let font2 = egui::FontId::proportional(10.0);
                                            let galley2 = painter.layout_no_wrap(detail, font2, egui::Color32::from_gray(180));
                                            let bg2 = egui::Rect::from_center_size(detail_pos, galley2.size()).expand(2.0);
                                            painter.rect_filled(bg2, 2.0, egui::Color32::from_rgba_unmultiplied(30, 30, 40, 180));
                                            painter.galley(bg2.min, galley2, egui::Color32::from_gray(180));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Persistent snap point highlight during measuring/dimension (stays until complete)
                if let DrawState::Measuring { start } = &self.editor.draw_state {
                    if let Some(sp) = self.world_to_screen(*start, &rect) {
                        let painter = ui.painter();
                        // Pulsing highlight ring
                        painter.circle_stroke(sp, 10.0, egui::Stroke::new(2.5, egui::Color32::from_rgb(76, 139, 245)));
                        painter.circle_filled(sp, 4.0, egui::Color32::from_rgb(76, 139, 245));
                        // Label
                        painter.text(
                            egui::pos2(sp.x + 14.0, sp.y - 6.0),
                            egui::Align2::LEFT_CENTER,
                            "起點",
                            egui::FontId::proportional(10.0),
                            egui::Color32::from_rgb(76, 139, 245),
                        );
                    }
                }
                // Also highlight start point during line/arc drawing
                if let DrawState::LineFrom { p1 } | DrawState::ArcP1 { p1 } | DrawState::ArcP2 { p1, .. } = &self.editor.draw_state {
                    if let Some(sp) = self.world_to_screen(*p1, &rect) {
                        let painter = ui.painter();
                        painter.circle_stroke(sp, 8.0, egui::Stroke::new(2.0, egui::Color32::from_rgb(60, 200, 60)));
                        painter.circle_filled(sp, 3.0, egui::Color32::from_rgb(60, 200, 60));
                    }
                }

                // Draw dimension annotations
                if !self.dimensions.is_empty() {
                    crate::dimensions::draw_dimensions_styled(ui.painter(), &self.dimensions, vp, &rect, &self.dim_style);
                }

                // Auto-show dimensions for selected object (CAD style)
                if let Some(ref id) = self.editor.selected_ids.first() {
                    if let Some(obj) = self.scene.objects.get(*id) {
                        let auto_dims = crate::dimensions::auto_dims_for_shape(&obj.shape, obj.position);
                        if !auto_dims.is_empty() {
                            crate::dimensions::draw_dimensions_styled(ui.painter(), &auto_dims, vp, &rect, &self.dim_style);
                        }
                    }
                }

                // ── 標註點擊編輯：雙擊標註文字進入編輯模式 ──
                if response.double_clicked() && !self.dimensions.is_empty() {
                    let positions = crate::dimensions::dim_label_positions(
                        &self.dimensions, vp, &rect, &self.dim_style,
                    );
                    let mx = self.editor.mouse_screen[0] + rect.min.x;
                    let my = self.editor.mouse_screen[1] + rect.min.y;
                    let hit_radius = 20.0;
                    for (idx, pos) in positions.iter().enumerate() {
                        if let Some(p) = pos {
                            let dx = mx - p.x;
                            let dy = my - p.y;
                            if dx * dx + dy * dy < hit_radius * hit_radius {
                                self.editor.editing_dim_idx = Some(idx);
                                self.editor.editing_dim_text = self.dimensions[idx].label_text(&self.dim_style);
                                break;
                            }
                        }
                    }
                }

                // ── 標註編輯浮動輸入框 ──
                if let Some(dim_idx) = self.editor.editing_dim_idx {
                    if dim_idx < self.dimensions.len() {
                        let positions = crate::dimensions::dim_label_positions(
                            &self.dimensions, vp, &rect, &self.dim_style,
                        );
                        if let Some(Some(label_pos)) = positions.get(dim_idx) {
                            let input_rect = egui::Rect::from_center_size(
                                *label_pos,
                                egui::vec2(100.0, 24.0),
                            );
                            let mut text = self.editor.editing_dim_text.clone();
                            let resp = ui.put(input_rect, egui::TextEdit::singleline(&mut text)
                                .font(egui::FontId::proportional(13.0))
                                .desired_width(90.0)
                                .horizontal_align(egui::Align::Center));
                            self.editor.editing_dim_text = text.clone();
                            // 自動 focus
                            if !resp.has_focus() {
                                resp.request_focus();
                            }
                            // Enter 確認
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                // 嘗試解析為數值並更新標註
                                let clean: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
                                if let Ok(val) = clean.parse::<f32>() {
                                    match &mut self.dimensions[dim_idx].kind {
                                        crate::dimensions::DimensionKind::Linear { start, end } => {
                                            // 保持方向，改距離
                                            let dir = glam::Vec3::from(*end) - glam::Vec3::from(*start);
                                            let len = dir.length();
                                            if len > 0.001 {
                                                let new_end = glam::Vec3::from(*start) + dir / len * val;
                                                *end = new_end.to_array();
                                            }
                                        }
                                        crate::dimensions::DimensionKind::Radius { radius, .. } => {
                                            *radius = val;
                                        }
                                        crate::dimensions::DimensionKind::Diameter { radius, .. } => {
                                            *radius = val / 2.0;
                                        }
                                        _ => {
                                            // 角度/弧長：設定 override label
                                            self.dimensions[dim_idx].label = Some(text.clone());
                                        }
                                    }
                                } else {
                                    // 非數值：設為 override label
                                    self.dimensions[dim_idx].label = Some(text.clone());
                                }
                                self.editor.editing_dim_idx = None;
                                self.editor.editing_dim_text.clear();
                            }
                            // ESC 取消
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                self.editor.editing_dim_idx = None;
                                self.editor.editing_dim_text.clear();
                            }
                        }
                    }
                }

                self.draw_viewport_overlays(ui, vp, rect, &response);


                // ── Help overlay ──
                if self.viewer.show_help {
                    // Full viewport overlay
                    let help_bg = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 245);
                    ui.painter().rect_filled(rect, 0.0, help_bg);

                    let help_rect = rect.shrink(40.0);

                    // Title
                    ui.painter().text(egui::pos2(help_rect.center().x, help_rect.top() + 20.0),
                        egui::Align2::CENTER_TOP, "Kolibri Ai3D \u{64cd}\u{4f5c}\u{8aaa}\u{660e}",
                        egui::FontId::proportional(20.0), egui::Color32::from_rgb(31, 36, 48));

                    // Close button
                    let close_rect = egui::Rect::from_min_size(
                        egui::pos2(help_rect.right() - 36.0, help_rect.top() + 8.0),
                        egui::vec2(28.0, 28.0));
                    ui.painter().rect_filled(close_rect, 14.0, egui::Color32::from_rgb(240, 70, 50));
                    ui.painter().text(close_rect.center(), egui::Align2::CENTER_CENTER, "\u{2715}",
                        egui::FontId::proportional(14.0), egui::Color32::WHITE);
                    let close_resp = ui.allocate_rect(close_rect, egui::Sense::click());
                    if close_resp.clicked() { self.viewer.show_help = false; }

                    // Help content columns
                    let col_w = (help_rect.width() - 60.0) / 3.0;
                    let col_y = help_rect.top() + 60.0;
                    let line_h = 22.0;
                    let header_font = egui::FontId::proportional(14.0);
                    let body_font = egui::FontId::proportional(11.0);
                    let header_color = egui::Color32::from_rgb(76, 139, 245);
                    let text_color = egui::Color32::from_rgb(50, 55, 65);
                    let key_color = egui::Color32::from_rgb(110, 118, 135);

                    // Column 1: Mouse & Navigation
                    let x1 = help_rect.left() + 20.0;
                    let mut y = col_y;
                    ui.painter().text(egui::pos2(x1, y), egui::Align2::LEFT_TOP, "\u{1f5b1} \u{6ed1}\u{9f20}\u{64cd}\u{4f5c}", header_font.clone(), header_color);
                    y += line_h + 4.0;
                    let mouse_help: &[(&str, &str)] = &[
                        ("\u{4e2d}\u{9375}\u{62d6}\u{66f3}", "\u{65cb}\u{8f49}\u{8996}\u{89d2}"),
                        ("Shift + \u{4e2d}\u{9375}", "\u{5e73}\u{79fb}\u{8996}\u{89d2}"),
                        ("\u{6efe}\u{8f2a}", "\u{7e2e}\u{653e}\u{ff08}\u{6e38}\u{6a19}\u{4e2d}\u{5fc3}\u{ff09}"),
                        ("\u{4e2d}\u{9375}\u{9ede}\u{64ca}", "\u{5c45}\u{4e2d}\u{5230}\u{9ede}\u{64ca}\u{4f4d}\u{7f6e}"),
                        ("\u{5de6}\u{9375}\u{9ede}\u{64ca}", "\u{9078}\u{53d6} / \u{4f7f}\u{7528}\u{5de5}\u{5177}"),
                        ("\u{5de6}\u{9375}\u{62d6}\u{66f3}", "\u{6846}\u{9078}\u{ff08}Select \u{5de5}\u{5177}\u{ff09}"),
                        ("Shift + \u{5de6}\u{9375}", "\u{591a}\u{9078}\u{5207}\u{63db}"),
                        ("\u{53f3}\u{9375}", "\u{53f3}\u{9375}\u{9078}\u{55ae}"),
                    ];
                    for (key, desc) in mouse_help {
                        ui.painter().text(egui::pos2(x1, y), egui::Align2::LEFT_TOP, *key, body_font.clone(), key_color);
                        ui.painter().text(egui::pos2(x1 + 110.0, y), egui::Align2::LEFT_TOP, *desc, body_font.clone(), text_color);
                        y += line_h;
                    }

                    y += 10.0;
                    ui.painter().text(egui::pos2(x1, y), egui::Align2::LEFT_TOP, "\u{1f4d0} \u{7e6a}\u{5716}\u{64cd}\u{4f5c}", header_font.clone(), header_color);
                    y += line_h + 4.0;
                    let draw_help: &[(&str, &str)] = &[
                        ("\u{756b}\u{5b8c}\u{8f38}\u{5165}\u{6578}\u{5b57}", "\u{7cbe}\u{78ba}\u{5c3a}\u{5bf8}\u{ff08}\u{53f3}\u{4e0b}\u{89d2}\u{ff09}"),
                        ("Shift \u{6309}\u{4f4f}", "\u{9396}\u{5b9a}\u{8ef8}\u{5411}"),
                        ("Ctrl + \u{79fb}\u{52d5}", "\u{8907}\u{88fd}\u{79fb}\u{52d5}"),
                        ("\u{8907}\u{88fd}\u{5f8c}\u{8f38}\u{5165} 3x", "\u{5efa}\u{7acb} 3 \u{500b}\u{7b49}\u{8ddd}\u{526f}\u{672c}"),
                        ("ESC", "取消目前操作"),
                        ("雙擊推拉面", "重複上次推拉距離"),
                        ("TAB", "套用 AI 建議"),
                        ("複製後輸入 3x", "建立 3 個等距副本"),
                        ("複製後輸入 6r", "建立 6 個極座標陣列"),
                        ("雙擊物件名稱", "重新命名"),
                        ("左→右框選", "窗選（藍框）"),
                        ("右→左框選", "交叉選取（綠虛框）"),
                    ];
                    for (key, desc) in draw_help {
                        ui.painter().text(egui::pos2(x1, y), egui::Align2::LEFT_TOP, *key, body_font.clone(), key_color);
                        ui.painter().text(egui::pos2(x1 + 110.0, y), egui::Align2::LEFT_TOP, *desc, body_font.clone(), text_color);
                        y += line_h;
                    }

                    // Column 2: Keyboard Shortcuts
                    let x2 = x1 + col_w + 20.0;
                    y = col_y;
                    ui.painter().text(egui::pos2(x2, y), egui::Align2::LEFT_TOP, "\u{2328} \u{5feb}\u{6377}\u{9375}", header_font.clone(), header_color);
                    y += line_h + 4.0;
                    let shortcuts: &[(&str, &str)] = &[
                        ("Space", "\u{9078}\u{53d6}\u{5de5}\u{5177}"),
                        ("M", "\u{79fb}\u{52d5}"),
                        ("Q", "\u{65cb}\u{8f49}"),
                        ("S", "\u{7e2e}\u{653e}"),
                        ("L", "\u{7dda}\u{6bb5}"),
                        ("A", "\u{5f27}\u{7dda}"),
                        ("R", "\u{77e9}\u{5f62}"),
                        ("C", "\u{5713}\u{5f62}"),
                        ("B", "\u{65b9}\u{584a}"),
                        ("P", "\u{63a8}\u{62c9}"),
                        ("T", "\u{91cf}\u{6e2c}"),
                        ("H", "\u{5e73}\u{79fb}\u{8996}\u{89d2}"),
                        ("O", "\u{74b0}\u{7e5e}\u{8996}\u{89d2}"),
                        ("E", "\u{6a61}\u{76ae}\u{64e6}"),
                        ("G", "\u{7fa4}\u{7d44}"),
                        ("F", "\u{504f}\u{79fb}"),
                        ("Z", "\u{5168}\u{90e8}\u{986f}\u{793a}"),
                        ("5", "\u{900f}\u{8996}/\u{5e73}\u{884c}\u{5207}\u{63db}"),
                        ("1/2/3", "\u{6b63}\u{8996}/\u{4fef}\u{8996}/\u{7b49}\u{89d2}"),
                        ("Ctrl+Z", "\u{5fa9}\u{539f}"),
                        ("Ctrl+Y", "\u{91cd}\u{505a}"),
                        ("Ctrl+S", "\u{5132}\u{5b58}"),
                        ("Ctrl+O", "\u{958b}\u{555f}"),
                        ("Ctrl+C/V/X", "複製/貼上/剪下"),
                        ("Ctrl+D", "就地複製"),
                        ("Ctrl+M", "鏡射 X"),
                        ("Ctrl+I", "反轉選取"),
                        ("Ctrl+P", "指令面板"),
                        ("Ctrl+A", "全選"),
                        ("Ctrl+Shift+C", "複製屬性"),
                        ("Ctrl+Shift+V", "貼上屬性"),
                        ("Alt+H", "隱藏選取"),
                        ("Alt+Shift+H", "顯示全部"),
                        ("Alt+I", "隔離顯示"),
                        ("W", "牆工具"),
                        ("4/6/8", "左/右/後視角"),
                        (".", "Zoom 到選取"),
                        ("F1", "說明（本頁面）"),
                        ("F12", "Console"),
                        ("Delete", "刪除選取"),
                    ];
                    for (key, desc) in shortcuts {
                        ui.painter().text(egui::pos2(x2, y), egui::Align2::LEFT_TOP, *key, body_font.clone(), key_color);
                        ui.painter().text(egui::pos2(x2 + 60.0, y), egui::Align2::LEFT_TOP, *desc, body_font.clone(), text_color);
                        y += line_h;
                    }

                    // Column 3: Features & Tips
                    let x3 = x2 + col_w + 20.0;
                    y = col_y;
                    ui.painter().text(egui::pos2(x3, y), egui::Align2::LEFT_TOP, "\u{1f527} \u{6355}\u{6349}\u{7cfb}\u{7d71}", header_font.clone(), header_color);
                    y += line_h + 4.0;
                    let snap_help: &[(&str, &str)] = &[
                        ("\u{1f7e2} \u{7da0}\u{8272}\u{83f1}\u{5f62}", "\u{7aef}\u{9ede}"),
                        ("\u{1f535} \u{9752}\u{8272}\u{4e09}\u{89d2}", "\u{908a}\u{4e2d}\u{9ede}"),
                        ("\u{ff0b} \u{6dfa}\u{85cd}\u{5341}\u{5b57}", "\u{9762}\u{4e2d}\u{5fc3}"),
                        ("\u{1f534} \u{7d05}\u{8272}\u{83f1}\u{5f62}", "\u{908a}\u{4e0a}"),
                        ("\u{1f7e0} \u{6a59}\u{8272}\u{5713}\u{9ede}", "\u{539f}\u{9ede}"),
                        ("\u{1f534} \u{7d05}\u{8272}\u{865b}\u{7dda}", "X \u{8ef8}"),
                        ("\u{1f7e2} \u{7da0}\u{8272}\u{865b}\u{7dda}", "Y \u{8ef8}"),
                        ("\u{1f535} \u{85cd}\u{8272}\u{865b}\u{7dda}", "Z \u{8ef8}"),
                        ("\u{1f7e3} \u{7d2b}\u{8272}", "\u{5e73}\u{884c}/\u{5782}\u{76f4}"),
                        ("\u{1f4cd}", "\u{4e0a}\u{4e0b}\u{6587}\u{63a8}\u{65b7}"),
                        ("\u{1f916}", "AI \u{610f}\u{5716}\u{63a8}\u{65b7}"),
                    ];
                    for (key, desc) in snap_help {
                        ui.painter().text(egui::pos2(x3, y), egui::Align2::LEFT_TOP, *key, body_font.clone(), key_color);
                        ui.painter().text(egui::pos2(x3 + 85.0, y), egui::Align2::LEFT_TOP, *desc, body_font.clone(), text_color);
                        y += line_h;
                    }

                    y += 10.0;
                    ui.painter().text(egui::pos2(x3, y), egui::Align2::LEFT_TOP, "\u{1f3d7} \u{92fc}\u{69cb}\u{6a21}\u{5f0f}", header_font.clone(), header_color);
                    y += line_h + 4.0;
                    let steel_help: &[(&str, &str)] = &[
                        ("\u{5de6}\u{4e0a}\u{89d2}\u{5207}\u{63db}", "\u{5efa}\u{6a21} \u{2194} \u{92fc}\u{69cb}"),
                        ("\u{8ef8}\u{7dda}", "\u{9ede}\u{64ca}\u{653e}\u{7f6e} XZ \u{8ef8}\u{7dda}"),
                        ("\u{67f1}", "\u{9ede}\u{64ca}\u{653e}\u{7f6e} H \u{578b}\u{92fc}\u{67f1}"),
                        ("\u{6a11}", "\u{5169}\u{9ede}\u{5efa}\u{7acb} H \u{578b}\u{92fc}\u{6a11}"),
                        ("\u{659c}\u{64d0}", "\u{5169}\u{9ede}\u{9023}\u{7dda}"),
                        ("\u{92fc}\u{677f}", "\u{756b}\u{77e9}\u{5f62}\u{2192}\u{63a8}\u{62c9}\u{539a}\u{5ea6}"),
                        ("\u{63a5}\u{982d}", "\u{9078}\u{5169}\u{69cb}\u{4ef6}\u{6a19}\u{8a18}"),
                    ];
                    for (key, desc) in steel_help {
                        ui.painter().text(egui::pos2(x3, y), egui::Align2::LEFT_TOP, *key, body_font.clone(), key_color);
                        ui.painter().text(egui::pos2(x3 + 85.0, y), egui::Align2::LEFT_TOP, *desc, body_font.clone(), text_color);
                        y += line_h;
                    }

                    y += 10.0;
                    ui.painter().text(egui::pos2(x3, y), egui::Align2::LEFT_TOP, "\u{1f4c1} \u{6a94}\u{6848}\u{652f}\u{63f4}", header_font.clone(), header_color);
                    y += line_h + 4.0;
                    let file_help: &[(&str, &str)] = &[
                        ("K3D", "Kolibri \u{539f}\u{751f}\u{683c}\u{5f0f}"),
                        ("DXF", "\u{667a}\u{6167}\u{532f}\u{5165}\u{ff08}\u{8ef8}\u{7dda}/\u{67f1}\u{6a11}\u{ff09}"),
                        ("DWG", "\u{57fa}\u{790e}\u{5e7e}\u{4f55}\u{532f}\u{5165}"),
                        ("OBJ/STL", "3D \u{6a21}\u{578b}\u{532f}\u{51fa}\u{5165}"),
                        ("GLTF", "Web/\u{904a}\u{6232}\u{5f15}\u{64ce}\u{532f}\u{51fa}"),
                        ("PNG/JPG", "\u{622a}\u{5716}\u{532f}\u{51fa}"),
                    ];
                    for (key, desc) in file_help {
                        ui.painter().text(egui::pos2(x3, y), egui::Align2::LEFT_TOP, *key, body_font.clone(), key_color);
                        ui.painter().text(egui::pos2(x3 + 60.0, y), egui::Align2::LEFT_TOP, *desc, body_font.clone(), text_color);
                        y += line_h;
                    }

                    // Bottom note
                    ui.painter().text(egui::pos2(help_rect.center().x, help_rect.bottom() - 20.0),
                        egui::Align2::CENTER_BOTTOM, "\u{6309} F1 \u{6216} ESC \u{95dc}\u{9589}\u{8aaa}\u{660e}",
                        body_font.clone(), key_color);

                    // ESC also closes help
                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.viewer.show_help = false;
                    }
                }

                // AI Suggestion popup
                if let Some(ref suggestion) = self.editor.suggestion.clone() {
                    let popup_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.center().x, rect.max.y - 60.0),
                        egui::vec2(350.0, 40.0),
                    );
                    ui.painter().rect_filled(popup_rect, 6.0, egui::Color32::from_rgba_unmultiplied(40, 40, 50, 230));
                    ui.painter().rect_stroke(popup_rect, 6.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 180, 255)));

                    ui.painter().text(
                        popup_rect.left_center() + egui::vec2(10.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        &format!("\u{1f4a1} {}", suggestion.message),
                        egui::FontId::proportional(13.0),
                        egui::Color32::from_rgb(200, 220, 255),
                    );

                    ui.painter().text(
                        popup_rect.right_center() + egui::vec2(-10.0, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        "Y:接受 N:忽略",
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_gray(140),
                    );
                }
            });

        // ── WASD walk mode when Orbit tool is active ──
        if self.editor.tool == Tool::Orbit && !ctx.wants_keyboard_input() {
            let walk_speed = self.viewer.camera.distance * 0.005;
            ctx.input(|i| {
                if i.key_down(egui::Key::W) { self.viewer.camera.walk_forward(walk_speed); }
                if i.key_down(egui::Key::S) { self.viewer.camera.walk_forward(-walk_speed); }
                if i.key_down(egui::Key::A) { self.viewer.camera.walk_strafe(-walk_speed); }
                if i.key_down(egui::Key::D) { self.viewer.camera.walk_strafe(walk_speed); }
            });
        }

        // ── TAB = apply AI suggestion ──
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
                if let Some(ref suggestion) = self.editor.cursor_hint.ai_suggestion {
                    self.file_message = Some((format!("AI \u{5efa}\u{8b70}\u{5df2}\u{5957}\u{7528}: {}", suggestion), std::time::Instant::now()));
                }
            }
        }

        // ── F1 = toggle help overlay ──
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.viewer.show_help = !self.viewer.show_help;
        }

        // ── F12 = toggle console panel ──
        if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            self.viewer.show_console = !self.viewer.show_console;
        }

        // ── Perspective/Ortho toggle (Num5 key) ──
        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Num5) {
                    self.viewer.use_ortho = !self.viewer.use_ortho;
                }
                // Restore saved cameras with Num6-Num9
                if i.key_pressed(egui::Key::Num6) {
                    if let Some((_, cam)) = self.viewer.saved_cameras.get(0) {
                        self.viewer.camera = cam.clone();
                    }
                }
                if i.key_pressed(egui::Key::Num7) {
                    if let Some((_, cam)) = self.viewer.saved_cameras.get(1) {
                        self.viewer.camera = cam.clone();
                    }
                }
                if i.key_pressed(egui::Key::Num8) {
                    if let Some((_, cam)) = self.viewer.saved_cameras.get(2) {
                        self.viewer.camera = cam.clone();
                    }
                }
                if i.key_pressed(egui::Key::Num9) {
                    if let Some((_, cam)) = self.viewer.saved_cameras.get(3) {
                        self.viewer.camera = cam.clone();
                    }
                }
            });
        }

        // ── Cursor feedback based on active tool ──
        ctx.output_mut(|o| {
            o.cursor_icon = match self.editor.tool {
                Tool::Select => egui::CursorIcon::Default,
                Tool::Move => egui::CursorIcon::Move,
                Tool::Rotate => egui::CursorIcon::Alias,
                Tool::Scale => egui::CursorIcon::ResizeNeSw,
                Tool::Line | Tool::Arc | Tool::Rectangle | Tool::Circle => egui::CursorIcon::Crosshair,
                Tool::CreateBox | Tool::CreateCylinder | Tool::CreateSphere => egui::CursorIcon::Crosshair,
                Tool::PushPull => egui::CursorIcon::ResizeVertical,
                Tool::Eraser => egui::CursorIcon::NotAllowed,
                Tool::PaintBucket => egui::CursorIcon::PointingHand,
                Tool::TapeMeasure | Tool::Dimension => egui::CursorIcon::Crosshair,
                Tool::Text => egui::CursorIcon::Text,
                Tool::Orbit => egui::CursorIcon::Grab,
                Tool::Pan => egui::CursorIcon::AllScroll,
                _ => egui::CursorIcon::Default,
            };
        });

        // ── Auto-save check ──
        self.check_auto_save();

        // ── Test bridge: poll for commands ──
        self.poll_test_bridge();
        self.maybe_capture_startup_screenshot(ctx);

        ctx.request_repaint();
    }
}

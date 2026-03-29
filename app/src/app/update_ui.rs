use eframe::egui;

use crate::app::KolibriApp;

impl KolibriApp {
    /// 繪製所有 UI 面板：頂部列、左側工具列、右側屬性面板、Console、底部狀態列
    pub(super) fn draw_panels(&mut self, ctx: &egui::Context) {
        // ── Top branded bar ──
        egui::TopBottomPanel::top("topbar")
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 217))
                .inner_margin(egui::Margin::symmetric(18.0, 8.0))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239))))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left: Brand logo + name
                {
                    let (logo_rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::hover());
                    ui.painter().rect_filled(logo_rect, 12.0, egui::Color32::from_rgb(76, 139, 245));
                    ui.painter().text(logo_rect.center(), egui::Align2::CENTER_CENTER,
                        "K", egui::FontId::proportional(16.0), egui::Color32::WHITE);

                    ui.vertical(|ui| {
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new("Kolibri Ai3D").strong().size(14.0).color(egui::Color32::from_rgb(31, 36, 48)));
                        ui.label(egui::RichText::new("3D Modeling Workflow").size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    });
                }

                ui.add_space(16.0);

                // Center: Menu bar (functional)
                let has_sel = !self.editor.selected_ids.is_empty();
                let can_undo = self.scene.can_undo();
                let can_redo = self.scene.can_redo();
                let count = self.scene.objects.len();
                let has_file = self.current_file.is_some();
                let action = crate::menu::draw_menu_bar(ui, has_sel, can_undo, can_redo, count, &self.recent_files, has_file, self.viewer.use_ortho, self.viewer.saved_cameras.len());

                // Right side: help + undo/redo + save + project name
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // MCP Server button
                    let mcp_label = if self.mcp_http_running {
                        egui::RichText::new("MCP").size(11.0).strong().color(egui::Color32::WHITE)
                    } else {
                        egui::RichText::new("MCP").size(11.0).color(egui::Color32::from_rgb(110, 118, 135))
                    };
                    let mcp_fill = if self.mcp_http_running {
                        egui::Color32::from_rgb(60, 186, 108)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(110, 118, 135, 30)
                    };
                    let mcp_btn = egui::Button::new(mcp_label)
                        .fill(mcp_fill)
                        .rounding(8.0);
                    let mcp_tip = if self.mcp_http_running {
                        format!("MCP Server 運行中 (port {})\n點擊開啟 Dashboard", self.mcp_http_port)
                    } else {
                        "啟動 MCP Server + Dashboard".to_string()
                    };
                    if ui.add(mcp_btn).on_hover_text(mcp_tip).clicked() {
                        // MCP 已由 auto-start 啟動，按鈕只開 Dashboard
                        let url = format!("http://localhost:{}", self.mcp_http_port);
                        let _ = std::process::Command::new("cmd").args(["/C", "start", &url]).spawn();
                    }

                    ui.add_space(4.0);

                    // Help button
                    let help_btn = egui::Button::new(egui::RichText::new("?").size(14.0).strong())
                        .fill(egui::Color32::from_rgba_unmultiplied(76, 139, 245, 40))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 139, 245)))
                        .rounding(12.0);
                    if ui.add(help_btn).on_hover_text("說明 (F1)").clicked() {
                        self.viewer.show_help = !self.viewer.show_help;
                    }

                    ui.add_space(4.0);

                    // Redo button with count badge
                    let redo_count = self.scene.redo_count();
                    let redo_tip = format!("重做 (Ctrl+Y) — {} 步", redo_count);
                    if ui.add_enabled(self.scene.can_redo(), egui::Button::new(
                        egui::RichText::new("\u{21bb}").size(14.0))).on_hover_text(redo_tip).clicked() {
                        self.scene.redo();
                    }
                    if redo_count > 0 {
                        ui.label(egui::RichText::new(format!("{}", redo_count)).size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    }

                    // Undo button with count badge
                    let undo_count = self.scene.undo_count();
                    let undo_tip = format!("復原 (Ctrl+Z) — {} 步", undo_count);
                    if ui.add_enabled(self.scene.can_undo(), egui::Button::new(
                        egui::RichText::new("\u{21ba}").size(14.0))).on_hover_text(undo_tip).clicked() {
                        self.scene.undo();
                    }
                    if undo_count > 0 {
                        ui.label(egui::RichText::new(format!("{}", undo_count)).size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    }

                    ui.add_space(4.0);

                    // Save button
                    if ui.add(egui::Button::new("\u{1f4be}")).on_hover_text("儲存 (Ctrl+S)").clicked() {
                        self.save_scene();
                    }

                    ui.separator();

                    // Project name (clickable to Save As)
                    let project_display = if let Some(ref path) = self.current_file {
                        let filename = path.rsplit(['\\', '/']).next().unwrap_or(path);
                        if self.has_unsaved_changes() {
                            format!("\u{1f4c4} {}*", filename)
                        } else {
                            format!("\u{1f4c4} {}", filename)
                        }
                    } else if !self.scene.objects.is_empty() {
                        "\u{1f4c4} 未儲存專案 *".to_string()
                    } else {
                        "\u{1f4c4} 新專案".to_string()
                    };

                    let proj_btn = egui::Button::new(egui::RichText::new(&project_display).size(11.0))
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::new(0.5, egui::Color32::from_rgb(229, 231, 239)))
                        .rounding(8.0);
                    if ui.add(proj_btn).on_hover_text("點擊另存新檔").clicked() {
                        self.current_file = None;
                        self.save_scene();
                    }
                });

                // Handle menu action
                match action {
                    crate::menu::MenuAction::ToggleOrtho => {
                        self.viewer.use_ortho = !self.viewer.use_ortho;
                        let mode = if self.viewer.use_ortho { "平行投影" } else { "透視投影" };
                        self.file_message = Some((format!("已切換: {}", mode), std::time::Instant::now()));
                    }
                    crate::menu::MenuAction::SaveCamera => {
                        let name = format!("場景 {}", self.viewer.saved_cameras.len() + 1);
                        self.viewer.saved_cameras.push((name, self.viewer.camera.clone()));
                        self.file_message = Some(("視角已儲存".into(), std::time::Instant::now()));
                    }
                    crate::menu::MenuAction::ToggleConsole => {
                        self.viewer.show_console = !self.viewer.show_console;
                    }
                    other => self.handle_menu_action(other),
                }
            });
        });

        // ── Left panel (toolbar only) ──
        egui::SidePanel::left("left_panel")
            .default_width(116.0).min_width(116.0).max_width(116.0).resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(245, 246, 250))
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(6.0, 0.0)))
            .show(ctx, |ui| {
                ui.add_space(8.0);
                self.toolbar_ui(ui);
            });

        // ── Right panel ──
        egui::SidePanel::right("right_panel")
            .exact_width(240.0).resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220))
                .inner_margin(egui::Margin::symmetric(10.0, 8.0)))
            .show(ctx, |ui| self.right_panel_ui(ui));

        // ── Console/Log panel (above status bar) ──
        if self.viewer.show_console {
            egui::TopBottomPanel::bottom("console")
                .min_height(100.0)
                .max_height(300.0)
                .resizable(true)
                .frame(egui::Frame::none()
                    .fill(egui::Color32::from_rgb(30, 30, 35))
                    .inner_margin(egui::Margin::same(8.0)))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Console").color(egui::Color32::from_gray(180)).size(12.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("\u{2715}").clicked() {
                                self.viewer.show_console = false;
                            }
                            if ui.small_button("清除").clicked() {
                                self.viewer.console_log.clear();
                            }
                        });
                    });
                    ui.separator();

                    egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                        let now = std::time::Instant::now();
                        for (level, msg, time) in &self.viewer.console_log {
                            let color = match level.as_str() {
                                "ERROR" => egui::Color32::from_rgb(255, 80, 80),
                                "WARN" => egui::Color32::from_rgb(255, 200, 60),
                                "ACTION" => egui::Color32::from_rgb(100, 255, 160),
                                "CLICK" => egui::Color32::from_rgb(255, 180, 100),
                                "TOOL" => egui::Color32::from_rgb(180, 140, 255),
                                "INFO" => egui::Color32::from_rgb(150, 200, 255),
                                _ => egui::Color32::from_gray(180),
                            };
                            let elapsed = now.duration_since(*time);
                            let ts = format!("{:.1}s", elapsed.as_secs_f32());
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&ts).color(egui::Color32::from_gray(100)).size(9.0).monospace());
                                ui.label(egui::RichText::new(level).color(color).size(10.0).monospace());
                                ui.label(egui::RichText::new(msg).color(egui::Color32::from_gray(210)).size(11.0));
                            });
                        }
                    });
                });
        }

        // ── Bottom: status + measurement ──
        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210))
                .inner_margin(egui::Margin::symmetric(16.0, 6.0))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239))))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Show file save/load message for 3 seconds
                if let Some((ref msg, when)) = self.file_message {
                    if when.elapsed().as_secs() < 3 {
                        ui.label(egui::RichText::new(msg).size(11.0).color(egui::Color32::from_rgb(20, 174, 92)));
                    } else {
                        self.file_message = None;
                    }
                }
                ui.label(egui::RichText::new(self.status_text()).size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Always-visible measurement input (like SketchUp VCB)
                    ui.label(egui::RichText::new("mm").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    let vcb = ui.add(
                        egui::TextEdit::singleline(&mut self.editor.measure_input)
                            .desired_width(140.0)
                            .hint_text("輸入尺寸...")
                            .font(egui::FontId::proportional(12.0))
                    );
                    if vcb.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.apply_measure();
                    }
                    ui.label(egui::RichText::new("尺寸:").size(11.0).strong().color(egui::Color32::from_rgb(76, 139, 245)));
                    ui.separator();
                    ui.label(egui::RichText::new(format!("物件: {}", self.scene.objects.len())).size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    ui.separator();
                    // ── Performance monitor ──
                    {
                        let fps = if self.perf_frame_times.is_empty() { 0.0 } else {
                            let avg_ms: f32 = self.perf_frame_times.iter().sum::<f32>() / self.perf_frame_times.len() as f32;
                            if avg_ms > 0.01 { 1000.0 / avg_ms } else { 0.0 }
                        };
                        let fps_color = if fps >= 30.0 {
                            egui::Color32::from_rgb(20, 174, 92) // green
                        } else if fps >= 15.0 {
                            egui::Color32::from_rgb(230, 160, 30) // yellow
                        } else {
                            egui::Color32::from_rgb(220, 50, 50) // red
                        };
                        ui.label(egui::RichText::new(format!("{:.0} FPS", fps)).size(11.0).strong().color(fps_color));
                        ui.separator();
                        ui.label(egui::RichText::new(format!("RAM {:.0} MB", self.perf_ram_mb)).size(11.0).color(
                            if self.perf_ram_mb > 2000.0 { egui::Color32::from_rgb(220, 50, 50) }
                            else if self.perf_ram_mb > 1000.0 { egui::Color32::from_rgb(230, 160, 30) }
                            else { egui::Color32::from_rgb(110, 118, 135) }
                        ));
                        ui.separator();
                        let vk = self.perf_gpu_verts as f32 / 1000.0;
                        ui.label(egui::RichText::new(format!("GPU {:.0}K verts", vk)).size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                        if self.perf_mesh_build_ms > 1.0 {
                            ui.separator();
                            ui.label(egui::RichText::new(format!("mesh {:.0}ms", self.perf_mesh_build_ms)).size(11.0).color(
                                if self.perf_mesh_build_ms > 50.0 { egui::Color32::from_rgb(220, 50, 50) }
                                else { egui::Color32::from_rgb(110, 118, 135) }
                            ));
                        }
                    }
                    ui.separator();
                    let console_label = if self.viewer.show_console { "\u{25bc} Console" } else { "\u{25b2} Console" };
                    if ui.small_button(egui::RichText::new(console_label).size(10.0)).on_hover_text("F12").clicked() {
                        self.viewer.show_console = !self.viewer.show_console;
                    }
                });
            });
        });
    }

    /// 處理拖曳放置檔案
    pub(super) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in &dropped {
            if let Some(ref path) = file.path {
                let p = path.to_string_lossy().to_string();
                if p.ends_with(".k3d") {
                    self.console_push("INFO", format!("[File] 載入: {}", p));
                    match self.scene.load_from_file(&p) {
                        Ok(count) => {
                            self.current_file = Some(p.clone());
                            self.add_recent_file(&p);
                            self.editor.selected_ids.clear();
                            self.last_saved_version = self.scene.version;
                            for obj in self.scene.objects.values() {
                                if let Some(ref tex_path) = obj.texture_path {
                                    let _ = self.texture_manager.load(tex_path);
                                }
                            }
                            self.console_push("INFO", format!("[File] 已載入 {} 個物件", count));
                            self.file_message = Some((format!("已載入 {} 個物件", count), std::time::Instant::now()));
                        }
                        Err(e) => {
                            self.console_push("ERROR", format!("[File] 載入失敗: {}", e));
                            self.file_message = Some((format!("載入失敗: {}", e), std::time::Instant::now()));
                        }
                    }
                } else if p.ends_with(".obj") {
                    self.console_push("INFO", format!("[Import] OBJ: {}", p));
                    match crate::obj_io::import_obj(&mut self.scene, &p) {
                        Ok(count) => {
                            self.add_recent_file(&p);
                            self.editor.selected_ids.clear();
                            self.console_push("INFO", format!("[Import] OBJ 已匯入 {} 個物件", count));
                            self.file_message = Some((format!("已匯入 {} 個物件", count), std::time::Instant::now()));
                        }
                        Err(e) => {
                            self.console_push("ERROR", format!("[Import] OBJ 匯入失敗: {}", e));
                            self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now()));
                        }
                    }
                } else if p.ends_with(".stl") {
                    self.console_push("WARN", "[Import] STL 匯入尚未支援".to_string());
                    self.file_message = Some(("STL 匯入尚未支援".to_string(), std::time::Instant::now()));
                }
            }
        }
    }
}

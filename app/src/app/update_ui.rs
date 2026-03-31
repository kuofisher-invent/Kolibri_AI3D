use eframe::egui;

use crate::app::KolibriApp;

impl KolibriApp {
    /// 繪製所有 UI 面板：頂部列、左側工具列、右側屬性面板、Console、底部狀態列
    pub(super) fn draw_panels(&mut self, ctx: &egui::Context) {
        // ── Top branded bar ──
        let topbar_fill = if self.viewer.layout_mode {
            egui::Color32::from_rgb(45, 45, 48)
        } else {
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 217)
        };
        let topbar_border = if self.viewer.layout_mode {
            egui::Color32::from_rgb(60, 60, 64)
        } else {
            egui::Color32::from_rgb(229, 231, 239)
        };
        let topbar_stroke = if self.viewer.layout_mode {
            egui::Stroke::NONE
        } else {
            egui::Stroke::new(1.0, topbar_border)
        };
        egui::TopBottomPanel::top("topbar")
            .show_separator_line(!self.viewer.layout_mode)
            .frame(egui::Frame::none()
                .fill(topbar_fill)
                .inner_margin(egui::Margin::symmetric(18.0, 8.0))
                .stroke(topbar_stroke))
            .show(ctx, |ui| {
            // 出圖模式：臨時切換文字色為淺色
            if self.viewer.layout_mode {
                ui.style_mut().visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 205));
                ui.style_mut().visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 220, 225));
                ui.style_mut().visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
                ui.style_mut().visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
                ui.style_mut().visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
            }
            ui.horizontal(|ui| {
                // Left: Brand logo + name
                let name_color = if self.viewer.layout_mode {
                    egui::Color32::from_rgb(220, 220, 225)
                } else {
                    egui::Color32::from_rgb(31, 36, 48)
                };
                let subtitle_color = if self.viewer.layout_mode {
                    egui::Color32::from_rgb(160, 165, 175)
                } else {
                    egui::Color32::from_rgb(110, 118, 135)
                };
                {
                    let (logo_rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::hover());
                    ui.painter().rect_filled(logo_rect, 12.0, egui::Color32::from_rgb(76, 139, 245));
                    ui.painter().text(logo_rect.center(), egui::Align2::CENTER_CENTER,
                        "K", egui::FontId::proportional(16.0), egui::Color32::WHITE);

                    ui.vertical(|ui| {
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new("Kolibri Ai3D").strong().size(14.0).color(name_color));
                        ui.label(egui::RichText::new("3D Modeling Workflow").size(10.0).color(subtitle_color));
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
                    // 出圖模式：按鈕背景調深
                    let icon_color = if self.viewer.layout_mode {
                        egui::Color32::from_rgb(200, 200, 210)
                    } else {
                        egui::Color32::from_rgb(50, 55, 65)
                    };
                    let badge_color = if self.viewer.layout_mode {
                        egui::Color32::from_rgb(160, 165, 175)
                    } else {
                        egui::Color32::from_rgb(110, 118, 135)
                    };

                    // MCP Server button
                    let mcp_label = if self.mcp_http_running {
                        egui::RichText::new("MCP").size(11.0).strong().color(egui::Color32::WHITE)
                    } else {
                        egui::RichText::new("MCP").size(11.0).color(badge_color)
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
                        egui::RichText::new("\u{21bb}").size(14.0).color(icon_color))).on_hover_text(redo_tip).clicked() {
                        self.scene.redo();
                    }
                    if redo_count > 0 {
                        ui.label(egui::RichText::new(format!("{}", redo_count)).size(10.0).color(badge_color));
                    }

                    // Undo button with count badge
                    let undo_count = self.scene.undo_count();
                    let undo_tip = format!("復原 (Ctrl+Z) — {} 步", undo_count);
                    if ui.add_enabled(self.scene.can_undo(), egui::Button::new(
                        egui::RichText::new("\u{21ba}").size(14.0).color(icon_color))).on_hover_text(undo_tip).clicked() {
                        self.scene.undo();
                    }
                    if undo_count > 0 {
                        ui.label(egui::RichText::new(format!("{}", undo_count)).size(10.0).color(badge_color));
                    }

                    ui.add_space(4.0);

                    // Save button
                    if ui.add(egui::Button::new(egui::RichText::new("\u{1f4be}").color(icon_color))).on_hover_text("儲存 (Ctrl+S)").clicked() {
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

                    let proj_btn = egui::Button::new(egui::RichText::new(&project_display).size(11.0).color(icon_color))
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
                    crate::menu::MenuAction::ToggleGrid => {
                        self.viewer.show_grid = !self.viewer.show_grid;
                    }
                    crate::menu::MenuAction::ToggleAxes => {
                        self.viewer.show_axes = !self.viewer.show_axes;
                    }
                    crate::menu::MenuAction::ToggleToolbar => {
                        self.viewer.show_toolbar = !self.viewer.show_toolbar;
                    }
                    crate::menu::MenuAction::ToggleRightPanel => {
                        self.viewer.show_right_panel = !self.viewer.show_right_panel;
                    }
                    other => self.handle_menu_action(other),
                }
            });
        });

        // ── Ribbon（出圖模式時顯示在 topbar 下方）──
        #[cfg(feature = "drafting")]
        if self.viewer.layout_mode {
            self.draw_ribbon(ctx);
        }

        // ── 圖層管理員對話框 ──
        #[cfg(feature = "drafting")]
        if self.viewer.layout_mode && self.editor.show_layer_manager {
            let mut open = self.editor.show_layer_manager;
            egui::Window::new("圖層管理員")
                .open(&mut open)
                .default_size([320.0, 280.0])
                .resizable(true)
                .show(ctx, |ui| {
                    // 工具列
                    ui.horizontal(|ui| {
                        if ui.button("+ 新增").clicked() {
                            let name = format!("圖層{}", self.editor.draft_layers.layers.len());
                            self.editor.draft_layers.add(kolibri_drafting::DraftLayer::new(&name, [255, 255, 255]));
                        }
                        ui.label(egui::RichText::new(format!("目前: {}", self.editor.draft_layers.current)).size(11.0));
                    });
                    ui.separator();

                    // 圖層列表
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let layer_data: Vec<(String, [u8; 3], bool, bool, bool)> = self.editor.draft_layers.layers.iter()
                            .map(|l| (l.name.clone(), l.color, l.visible, l.locked, l.frozen))
                            .collect();

                        for (name, color, visible, locked, frozen) in &layer_data {
                            ui.horizontal(|ui| {
                                let is_current = *name == self.editor.draft_layers.current;

                                // 可見 toggle
                                let vis_icon = if *visible { "👁" } else { "  " };
                                if ui.small_button(vis_icon).on_hover_text("可見/隱藏").clicked() {
                                    if let Some(l) = self.editor.draft_layers.get_mut(name) {
                                        l.visible = !l.visible;
                                    }
                                }

                                // 鎖定 toggle
                                let lock_icon = if *locked { "🔒" } else { "🔓" };
                                if ui.small_button(lock_icon).on_hover_text("鎖定/解鎖").clicked() {
                                    if let Some(l) = self.editor.draft_layers.get_mut(name) {
                                        l.locked = !l.locked;
                                    }
                                }

                                // 凍結 toggle
                                let freeze_icon = if *frozen { "❄" } else { "☀" };
                                if ui.small_button(freeze_icon).on_hover_text("凍結/解凍").clicked() {
                                    if let Some(l) = self.editor.draft_layers.get_mut(name) {
                                        l.frozen = !l.frozen;
                                    }
                                }

                                // 色塊
                                let (cr, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                                ui.painter().rect_filled(cr, 2.0,
                                    egui::Color32::from_rgb(color[0], color[1], color[2]));

                                // 名稱（可點擊設為目前）
                                let label = if is_current {
                                    egui::RichText::new(name).strong()
                                } else {
                                    egui::RichText::new(name)
                                };
                                if ui.selectable_label(is_current, label).clicked() {
                                    self.editor.draft_layers.current = name.clone();
                                }
                            });
                        }
                    });
                });
            self.editor.show_layer_manager = open;
        }

        // ── 出圖模式：左側屬性/圖層工具列（ZWCAD 風格）──
        #[cfg(feature = "drafting")]
        if self.viewer.layout_mode {
            egui::SidePanel::left("draft_left_tools")
                .exact_width(24.0).resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::none()
                    .fill(egui::Color32::from_rgb(56, 56, 59))
                    .inner_margin(egui::Margin::symmetric(2.0, 4.0)))
                .show(ctx, |ui| {
                    let icon_btn_size = egui::vec2(20.0, 20.0);
                    let dim = egui::Color32::from_rgb(200, 200, 205);
                    let hover_bg = egui::Color32::from_rgb(75, 75, 80);

                    // 屬性/圖層/鎖點等功能按鈕（非繪圖工具）
                    let funcs: &[(&str, &str, &str)] = &[
                        ("⊞", "屬性", "物件屬性面板"),
                        ("◫", "圖層", "圖層管理員"),
                        ("⊕", "鎖點", "物件鎖點設定"),
                        ("⊙", "極座標", "極座標追蹤"),
                        ("▦", "格線", "格線顯示/隱藏"),
                        ("⊡", "正交", "正交模式"),
                        ("▤", "線型", "線型管理"),
                        ("◉", "線寬", "線寬顯示"),
                        ("⊿", "查詢", "查詢距離/面積"),
                        ("⊞", "計算", "快速計算器"),
                    ];

                    for &(icon, label, tooltip) in funcs {
                        let (rect, resp) = ui.allocate_exact_size(icon_btn_size, egui::Sense::click());
                        let p = ui.painter();
                        if resp.hovered() {
                            p.rect_filled(rect, 2.0, hover_bg);
                        }
                        p.text(rect.center(), egui::Align2::CENTER_CENTER, icon,
                            egui::FontId::proportional(11.0), dim);

                        if resp.on_hover_text(tooltip).clicked() {
                            match label {
                                "格線" => self.viewer.show_grid = !self.viewer.show_grid,
                                "圖層" => self.editor.show_layer_manager = !self.editor.show_layer_manager,
                                _ => {}
                            }
                            self.console_push("INFO", format!("{}", label));
                        }
                    }
                });
        }

        // ── Left panel (toolbar only) — 出圖模式時完全不顯示 ──
        if !self.viewer.layout_mode {
            let toolbar_w = if self.viewer.show_toolbar { 124.0 } else { 0.0 };
            egui::SidePanel::left("left_panel")
                .exact_width(toolbar_w).resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::none()
                    .fill(egui::Color32::from_rgb(245, 246, 250))
                    .stroke(egui::Stroke::NONE)
                    .inner_margin(egui::Margin::symmetric(6.0, 0.0)))
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    self.toolbar_ui(ui);
                });
        }

        // ── Right panel（出圖模式時完全不顯示）──
        if !self.viewer.layout_mode {
            let right_w = if self.viewer.show_right_panel { 240.0 } else { 0.0 };
            egui::SidePanel::right("right_panel")
                .exact_width(right_w).resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::none()
                    .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220))
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0)))
                .show(ctx, |ui| self.right_panel_ui(ui));
        }

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

        // ── 出圖模式：模型/配置 Tab 列（ZWCAD 最底部）──
        #[cfg(feature = "drafting")]
        if self.viewer.layout_mode {
            egui::TopBottomPanel::bottom("draft_tabs")
                .exact_height(26.0)
                .frame(egui::Frame::none()
                    .fill(egui::Color32::from_rgb(50, 50, 54))
                    .inner_margin(egui::Margin::symmetric(6.0, 0.0)))
                .show(ctx, |ui| {
                    let tab_active_bg = egui::Color32::from_rgb(64, 64, 68);
                    let tab_active_text = egui::Color32::WHITE;
                    let tab_text = egui::Color32::from_rgb(180, 180, 185);
                    ui.horizontal_centered(|ui| {
                        ui.label(egui::RichText::new("≡").size(14.0).color(tab_text));
                        ui.add_space(4.0);

                        if ui.add(egui::Button::new(egui::RichText::new("模型").size(12.0).color(tab_active_text))
                            .fill(tab_active_bg).rounding(0.0).stroke(egui::Stroke::NONE))
                            .on_hover_text("模型空間").clicked() {
                            self.viewer.layout_mode = false;
                        }
                        ui.add(egui::Button::new(egui::RichText::new("配置1").size(12.0).color(tab_text))
                            .fill(egui::Color32::TRANSPARENT).rounding(0.0).stroke(egui::Stroke::NONE));
                        ui.add(egui::Button::new(egui::RichText::new("配置2").size(12.0).color(tab_text))
                            .fill(egui::Color32::TRANSPARENT).rounding(0.0).stroke(egui::Stroke::NONE));
                        ui.label(egui::RichText::new("+").size(13.0).color(tab_text));
                    });
                });
        }

        // ── 出圖模式：底部狀態列（座標 + 功能 toggles）──
        #[cfg(feature = "drafting")]
        if self.viewer.layout_mode {
            egui::TopBottomPanel::bottom("draft_status")
                .exact_height(24.0)
                .frame(egui::Frame::none()
                    .fill(egui::Color32::from_rgb(40, 40, 44))
                    .inner_margin(egui::Margin::symmetric(8.0, 2.0)))
                .show(ctx, |ui| {
                    let dim = egui::Color32::from_rgb(160, 160, 165);
                    let on_color = egui::Color32::from_rgb(80, 200, 255);
                    ui.horizontal(|ui| {
                        // 左側：座標（較大字）
                        let mouse_mm = self.editor.mouse_screen;
                        ui.label(egui::RichText::new(format!("X:{:.0}", mouse_mm[0])).size(11.0).color(egui::Color32::from_rgb(200, 200, 205)).monospace());
                        ui.label(egui::RichText::new(format!("Y:{:.0}", mouse_mm[1])).size(11.0).color(egui::Color32::from_rgb(200, 200, 205)).monospace());
                        ui.label(egui::RichText::new("Z:0").size(11.0).color(dim).monospace());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // 右側功能 toggles（稍小字）
                            ui.label(egui::RichText::new(format!("{} 圖元", self.editor.draft_doc.objects.len())).size(10.0).color(dim));
                            ui.separator();
                            ui.label(egui::RichText::new("Units:mm").size(10.0).color(dim));
                            ui.separator();
                            ui.label(egui::RichText::new("線寬").size(10.0).color(dim));
                            ui.separator();
                            ui.label(egui::RichText::new("極座標").size(10.0).color(on_color));
                            ui.separator();
                            ui.label(egui::RichText::new("物件鎖點").size(10.0).color(on_color));
                            ui.separator();
                            let grid_color = if self.viewer.show_grid { on_color } else { dim };
                            if ui.add(egui::Label::new(egui::RichText::new("格線").size(10.0).color(grid_color)).sense(egui::Sense::click())).clicked() {
                                self.viewer.show_grid = !self.viewer.show_grid;
                            }
                            ui.separator();
                            ui.label(egui::RichText::new("Snap:ON").size(10.0).color(on_color));
                            ui.separator();
                            ui.label(egui::RichText::new("正交").size(10.0).color(dim));
                        });
                    });
                });
        }

        // ── Bottom: status + measurement ──
        let status_fill = if self.viewer.layout_mode {
            egui::Color32::from_rgb(45, 45, 48)
        } else {
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210)
        };
        let status_border = if self.viewer.layout_mode {
            egui::Color32::from_rgb(60, 60, 64)
        } else {
            egui::Color32::from_rgb(229, 231, 239)
        };
        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::none()
                .fill(status_fill)
                .inner_margin(egui::Margin::symmetric(16.0, 6.0))
                .stroke(egui::Stroke::new(1.0, status_border)))
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
                // Hover entity info（SU 風格：hover 時顯示物件資訊）
                if let Some(ref hid) = self.editor.hovered_id {
                    if let Some(obj) = self.scene.objects.get(hid) {
                        let shape_info = match &obj.shape {
                            crate::scene::Shape::Box { width, height, depth } =>
                                format!("Box {:.0}×{:.0}×{:.0}", width, height, depth),
                            crate::scene::Shape::Cylinder { radius, height, .. } =>
                                format!("Cyl r{:.0} h{:.0}", radius, height),
                            crate::scene::Shape::Sphere { radius, .. } =>
                                format!("Sphere r{:.0}", radius),
                            crate::scene::Shape::Line { points, .. } =>
                                format!("Line {}pts", points.len()),
                            crate::scene::Shape::Mesh(m) =>
                                format!("Mesh {}F", m.faces.len()),
                        };
                        let name = if obj.name.is_empty() { &shape_info } else { &obj.name };
                        ui.separator();
                        ui.label(egui::RichText::new(format!("\u{25b6} {}", name)).size(11.0)
                            .color(egui::Color32::from_rgb(76, 139, 245)));
                    }
                }
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
                        ui.label(egui::RichText::new(format!("GPU {:.0}K verts", vk)).size(11.0).color(egui::Color32::from_rgb(110, 118, 135)))
                            .on_hover_text(&self.gpu_name);
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

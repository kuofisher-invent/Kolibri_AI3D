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

                ui.add_space(12.0);

                // ── 3D / 2D 切換 Toggle（始終可見）──
                #[cfg(feature = "drafting")]
                {
                    let is_2d = self.viewer.layout_mode;
                    let brand = egui::Color32::from_rgb(76, 139, 245);
                    let active_bg = brand;
                    let inactive_bg = if is_2d {
                        egui::Color32::from_rgb(60, 60, 64)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(76, 139, 245, 25)
                    };
                    let active_text = egui::Color32::WHITE;
                    let inactive_text = if is_2d {
                        egui::Color32::from_rgb(160, 160, 165)
                    } else {
                        egui::Color32::from_rgb(100, 110, 130)
                    };

                    // 3D 按鈕（左半圓角）
                    let btn_3d = egui::Button::new(
                        egui::RichText::new("3D").size(13.0).strong()
                            .color(if !is_2d { active_text } else { inactive_text })
                    )
                    .fill(if !is_2d { active_bg } else { inactive_bg })
                    .rounding(egui::Rounding { nw: 6.0, sw: 6.0, ne: 0.0, se: 0.0 })
                    .stroke(egui::Stroke::new(1.0, if is_2d { egui::Color32::from_rgb(80, 80, 85) } else { brand }));
                    if ui.add_sized([42.0, 28.0], btn_3d).on_hover_text("3D 建模 (F6)").clicked() && is_2d {
                        self.exit_layout_mode();
                    }

                    // 2D 按鈕（右半圓角）
                    let btn_2d = egui::Button::new(
                        egui::RichText::new("2D").size(13.0).strong()
                            .color(if is_2d { active_text } else { inactive_text })
                    )
                    .fill(if is_2d { active_bg } else { inactive_bg })
                    .rounding(egui::Rounding { nw: 0.0, sw: 0.0, ne: 6.0, se: 6.0 })
                    .stroke(egui::Stroke::new(1.0, if !is_2d { egui::Color32::from_rgb(200, 205, 215) } else { brand }));
                    if ui.add_sized([42.0, 28.0], btn_2d).on_hover_text("2D 出圖 CAD (F6)").clicked() && !is_2d {
                        self.enter_layout_mode();
                    }

                    ui.add_space(6.0);
                }

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

        // ── 文字編輯器對話框 ──
        #[cfg(feature = "drafting")]
        if self.viewer.layout_mode {
            self.draw_text_editor(ctx);
        }

        // ── 圖層管理員對話框（ZWCAD 風格：表格式、可見/凍結/鎖定/顏色/線型/線寬）──
        #[cfg(feature = "drafting")]
        if self.viewer.layout_mode && self.editor.show_layer_manager {
            let mut open = self.editor.show_layer_manager;
            let dark_frame = egui::Frame::window(&ctx.style())
                .fill(egui::Color32::from_rgb(40, 44, 52))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 70, 75)));
            egui::Window::new("圖層特性管理員")
                .open(&mut open)
                .default_size([680.0, 420.0])
                .min_size([500.0, 200.0])
                .resizable(true)
                .frame(dark_frame)
                .show(ctx, |ui| {
                    // 深色主題文字預設色
                    ui.style_mut().visuals.override_text_color = Some(egui::Color32::from_rgb(220, 220, 225));
                    // ── 工具列 ──
                    ui.horizontal(|ui| {
                        if ui.button("➕ 新增圖層").clicked() {
                            let n = self.editor.draft_layers.layers.len();
                            let name = format!("新圖層{}", n);
                            self.editor.draft_layers.add(kolibri_drafting::DraftLayer::new(&name, [255, 255, 255]));
                        }
                        if ui.button("🗑 刪除").clicked() {
                            // 刪除非目前圖層中最後選的
                        }
                        ui.separator();
                        ui.label(egui::RichText::new(format!("目前圖層: {}", self.editor.draft_layers.current)).size(12.0).strong());
                        ui.separator();
                        ui.label(egui::RichText::new(format!("{} 個圖層", self.editor.draft_layers.layers.len())).size(11.0).color(egui::Color32::GRAY));
                    });
                    ui.separator();

                    // ── 表頭 ──
                    let row_h = 22.0;
                    ui.horizontal(|ui| {
                        ui.set_min_height(row_h);
                        let header_color = egui::Color32::from_rgb(180, 180, 190);
                        let hf = egui::FontId::proportional(11.0);
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("").size(11.0)); // checkbox space
                        ui.add_sized([28.0, row_h], egui::Label::new(egui::RichText::new("狀態").font(hf.clone()).color(header_color)));
                        ui.add_sized([160.0, row_h], egui::Label::new(egui::RichText::new("名稱").font(hf.clone()).color(header_color)));
                        ui.add_sized([28.0, row_h], egui::Label::new(egui::RichText::new("開").font(hf.clone()).color(header_color)));
                        ui.add_sized([28.0, row_h], egui::Label::new(egui::RichText::new("凍").font(hf.clone()).color(header_color)));
                        ui.add_sized([28.0, row_h], egui::Label::new(egui::RichText::new("鎖").font(hf.clone()).color(header_color)));
                        ui.add_sized([40.0, row_h], egui::Label::new(egui::RichText::new("顏色").font(hf.clone()).color(header_color)));
                        ui.add_sized([80.0, row_h], egui::Label::new(egui::RichText::new("線型").font(hf.clone()).color(header_color)));
                        ui.add_sized([50.0, row_h], egui::Label::new(egui::RichText::new("線寬").font(hf.clone()).color(header_color)));
                    });
                    ui.separator();

                    // ── 圖層列表（scroll）──
                    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
                        let layer_data: Vec<(String, [u8; 3], bool, bool, bool, String, f64)> = self.editor.draft_layers.layers.iter()
                            .map(|l| (l.name.clone(), l.color, l.visible, l.frozen, l.locked, format!("{:?}", l.line_type), l.line_weight))
                            .collect();

                        for (idx, (name, color, visible, frozen, locked, line_type, line_weight)) in layer_data.iter().enumerate() {
                            let is_current = *name == self.editor.draft_layers.current;
                            let bg = if is_current {
                                egui::Color32::from_rgba_unmultiplied(0, 122, 204, 40)
                            } else if idx % 2 == 0 {
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 5)
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            let (row_rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), row_h),
                                egui::Sense::hover());
                            ui.painter().rect_filled(row_rect, 0.0, bg);

                            // 用 child_ui 在 row_rect 中排列
                            let mut child = ui.child_ui(row_rect, egui::Layout::left_to_right(egui::Align::Center), None);
                            child.set_min_height(row_h);
                            child.add_space(4.0);

                            // 目前圖層標記
                            let status = if is_current { "✓" } else { " " };
                            child.add_sized([28.0, row_h], egui::Label::new(
                                egui::RichText::new(status).size(12.0).color(egui::Color32::from_rgb(0, 200, 0))));

                            // 名稱（可點擊設為目前）
                            let name_text = if is_current {
                                egui::RichText::new(name).size(12.0).strong().color(egui::Color32::WHITE)
                            } else {
                                egui::RichText::new(name).size(12.0).color(egui::Color32::from_rgb(220, 220, 220))
                            };
                            if child.add_sized([160.0, row_h], egui::Label::new(name_text).sense(egui::Sense::click())).clicked() {
                                self.editor.draft_layers.current = name.clone();
                            }

                            // 可見 toggle (燈泡)
                            let vis_icon = if *visible { "💡" } else { "⚫" };
                            let vis_color = if *visible { egui::Color32::YELLOW } else { egui::Color32::GRAY };
                            if child.add_sized([28.0, row_h], egui::Label::new(
                                egui::RichText::new(vis_icon).size(13.0).color(vis_color))
                                .sense(egui::Sense::click())).clicked() {
                                if let Some(l) = self.editor.draft_layers.layers.get_mut(idx) {
                                    l.visible = !l.visible;
                                }
                            }

                            // 凍結 toggle
                            let frz_icon = if *frozen { "❄" } else { "☀" };
                            let frz_color = if *frozen { egui::Color32::from_rgb(100, 180, 255) } else { egui::Color32::from_rgb(200, 200, 200) };
                            if child.add_sized([28.0, row_h], egui::Label::new(
                                egui::RichText::new(frz_icon).size(13.0).color(frz_color))
                                .sense(egui::Sense::click())).clicked() {
                                if let Some(l) = self.editor.draft_layers.layers.get_mut(idx) {
                                    l.frozen = !l.frozen;
                                }
                            }

                            // 鎖定 toggle
                            let lock_icon = if *locked { "🔒" } else { "🔓" };
                            if child.add_sized([28.0, row_h], egui::Label::new(
                                egui::RichText::new(lock_icon).size(13.0))
                                .sense(egui::Sense::click())).clicked() {
                                if let Some(l) = self.editor.draft_layers.layers.get_mut(idx) {
                                    l.locked = !l.locked;
                                }
                            }

                            // 顏色方塊
                            let color_rect = egui::Rect::from_min_size(
                                egui::pos2(child.cursor().min.x + 8.0, row_rect.center().y - 6.0),
                                egui::vec2(24.0, 12.0));
                            child.painter().rect_filled(color_rect, 2.0,
                                egui::Color32::from_rgb(color[0], color[1], color[2]));
                            child.painter().rect_stroke(color_rect, 2.0,
                                egui::Stroke::new(0.5, egui::Color32::from_rgb(100, 100, 100)));
                            child.add_space(40.0);

                            // 線型
                            child.add_sized([80.0, row_h], egui::Label::new(
                                egui::RichText::new(line_type).size(10.0).color(egui::Color32::from_rgb(180, 180, 180))));

                            // 線寬
                            child.add_sized([50.0, row_h], egui::Label::new(
                                egui::RichText::new(format!("{:.2}", line_weight)).size(10.0).color(egui::Color32::from_rgb(180, 180, 180))));
                        }
                    });
                });
            self.editor.show_layer_manager = open;
        }

        // ── DXF/DWG 匯入模式選擇對話框 ──
        #[cfg(feature = "drafting")]
        if self.show_import_mode_dialog {
            let mut close = false;
            egui::Window::new("匯入模式選擇")
                .collapsible(false)
                .resizable(false)
                .default_size([320.0, 150.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    let file_name = self.pending_import_path.as_ref()
                        .map(|p| p.rsplit(['\\', '/']).next().unwrap_or(p).to_string())
                        .unwrap_or_default();
                    ui.label(egui::RichText::new(format!("匯入: {}", file_name)).size(14.0).strong());
                    ui.add_space(8.0);
                    ui.label("請選擇匯入目標：");
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        // 2D 出圖畫布按鈕
                        let btn_2d = ui.add_sized([130.0, 40.0],
                            egui::Button::new(egui::RichText::new("📐 2D 出圖畫布").size(14.0)));
                        if btn_2d.on_hover_text("匯入到 2D CAD 出圖模式（推薦）\n自動切換到出圖模式，建立新分頁").clicked() {
                            if let Some(path) = self.pending_import_path.take() {
                                match self.import_cad_to_2d_tab(&path) {
                                    Ok(count) => {
                                        self.file_message = Some((format!("已匯入 {} 個 2D 圖元", count), std::time::Instant::now()));
                                    }
                                    Err(e) => {
                                        self.console_push("ERROR", format!("[2D] 匯入失敗: {}", e));
                                        self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now()));
                                    }
                                }
                            }
                            close = true;
                        }

                        ui.add_space(8.0);

                        // 3D 場景按鈕
                        let btn_3d = ui.add_sized([130.0, 40.0],
                            egui::Button::new(egui::RichText::new("🧊 3D 場景").size(14.0)));
                        if btn_3d.on_hover_text("匯入到 3D 建模場景\n線段/弧/圓 轉為 3D 物件").clicked() {
                            if let Some(path) = self.pending_import_path.take() {
                                match crate::dxf_io::import_dxf(&mut self.scene, &path) {
                                    Ok(count) => {
                                        self.editor.selected_ids.clear();
                                        self.file_message = Some((format!("已匯入 {} 個 3D 物件", count), std::time::Instant::now()));
                                    }
                                    Err(e) => self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now())),
                                }
                            }
                            close = true;
                        }
                    });
                    ui.add_space(8.0);
                    if ui.button("取消").clicked() {
                        self.pending_import_path = None;
                        close = true;
                    }
                });
            if close {
                self.show_import_mode_dialog = false;
            }
        }

        // （左側工具列已移除 — Ribbon 已包含所有功能）

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
                .show_separator_line(false)
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
                .exact_height(32.0)
                .show_separator_line(false)
                .frame(egui::Frame::none()
                    .fill(egui::Color32::from_rgb(50, 50, 54))
                    .stroke(egui::Stroke::NONE)
                    .inner_margin(egui::Margin::symmetric(6.0, 0.0)))
                .show(ctx, |ui| {
                    let tab_active_bg = egui::Color32::from_rgb(64, 64, 68);
                    let tab_active_text = egui::Color32::WHITE;
                    let tab_text = egui::Color32::from_rgb(180, 180, 185);
                    ui.horizontal_centered(|ui| {
                        ui.label(egui::RichText::new("≡").size(18.0).color(tab_text));
                        ui.add_space(4.0);

                        if ui.add(egui::Button::new(egui::RichText::new("模型").size(16.0).color(tab_active_text))
                            .fill(tab_active_bg).rounding(0.0).stroke(egui::Stroke::NONE))
                            .on_hover_text("模型空間").clicked() {
                            self.exit_layout_mode();
                        }
                        ui.add(egui::Button::new(egui::RichText::new("配置1").size(16.0).color(tab_text))
                            .fill(egui::Color32::TRANSPARENT).rounding(0.0).stroke(egui::Stroke::NONE));
                        ui.add(egui::Button::new(egui::RichText::new("配置2").size(16.0).color(tab_text))
                            .fill(egui::Color32::TRANSPARENT).rounding(0.0).stroke(egui::Stroke::NONE));
                        ui.label(egui::RichText::new("+").size(17.0).color(tab_text));
                    });
                });
        }

        // ── 出圖模式：底部狀態列（座標 + 功能 toggles）──
        #[cfg(feature = "drafting")]
        if self.viewer.layout_mode {
            egui::TopBottomPanel::bottom("draft_status")
                .exact_height(24.0)
                .show_separator_line(false)
                .frame(egui::Frame::none()
                    .fill(egui::Color32::from_rgb(40, 40, 44))
                    .stroke(egui::Stroke::NONE)
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
                            // 右側功能 toggles（全部可點擊 + F-key 提示）
                            ui.label(egui::RichText::new(format!("{} 圖元", self.editor.draft_doc.objects.len())).size(10.0).color(dim));
                            ui.separator();
                            ui.label(egui::RichText::new("Units:mm").size(10.0).color(dim));
                            ui.separator();
                            // DYN (F12)
                            let dyn_c = if self.editor.draft_dyn_input { on_color } else { dim };
                            if ui.add(egui::Label::new(egui::RichText::new("DYN").size(10.0).color(dyn_c)).sense(egui::Sense::click()))
                                .on_hover_text("動態輸入 (F12)").clicked() {
                                self.editor.draft_dyn_input = !self.editor.draft_dyn_input;
                            }
                            ui.separator();
                            // 線寬
                            ui.label(egui::RichText::new("LWT").size(10.0).color(dim));
                            ui.separator();
                            // POLAR (F10)
                            let polar_c = if self.editor.draft_polar { on_color } else { dim };
                            if ui.add(egui::Label::new(egui::RichText::new("POLAR").size(10.0).color(polar_c)).sense(egui::Sense::click()))
                                .on_hover_text("極座標追蹤 (F10)").clicked() {
                                self.editor.draft_polar = !self.editor.draft_polar;
                                if self.editor.draft_polar { self.editor.draft_ortho = false; }
                            }
                            ui.separator();
                            // OSNAP (F3)
                            let osnap_c = if self.editor.draft_osnap { on_color } else { dim };
                            if ui.add(egui::Label::new(egui::RichText::new("OSNAP").size(10.0).color(osnap_c)).sense(egui::Sense::click()))
                                .on_hover_text("物件鎖點 (F3)").clicked() {
                                self.editor.draft_osnap = !self.editor.draft_osnap;
                            }
                            ui.separator();
                            // GRID (F7)
                            let grid_c = if self.viewer.show_grid { on_color } else { dim };
                            if ui.add(egui::Label::new(egui::RichText::new("GRID").size(10.0).color(grid_c)).sense(egui::Sense::click()))
                                .on_hover_text("格線 (F7)").clicked() {
                                self.viewer.show_grid = !self.viewer.show_grid;
                            }
                            ui.separator();
                            // SNAP (grid snap)
                            ui.label(egui::RichText::new("SNAP").size(10.0).color(dim));
                            ui.separator();
                            // ORTHO (F8)
                            let ortho_c = if self.editor.draft_ortho { on_color } else { dim };
                            if ui.add(egui::Label::new(egui::RichText::new("ORTHO").size(10.0).color(ortho_c)).sense(egui::Sense::click()))
                                .on_hover_text("正交 (F8)").clicked() {
                                self.editor.draft_ortho = !self.editor.draft_ortho;
                                if self.editor.draft_ortho { self.editor.draft_polar = false; }
                            }
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
            .show_separator_line(!self.viewer.layout_mode)
            .frame(egui::Frame::none()
                .fill(status_fill)
                .stroke(if self.viewer.layout_mode { egui::Stroke::NONE } else { egui::Stroke::new(1.0, status_border) })
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
                    self.console_push("INFO", format!("[Import] STL: {}", p));
                    match crate::stl_io::import_stl(&mut self.scene, &p) {
                        Ok(count) => {
                            self.add_recent_file(&p);
                            self.editor.selected_ids.clear();
                            self.file_message = Some((format!("已匯入 {} 個物件", count), std::time::Instant::now()));
                        }
                        Err(e) => {
                            self.console_push("ERROR", format!("[Import] STL 匯入失敗: {}", e));
                            self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now()));
                        }
                    }
                } else if p.to_lowercase().ends_with(".dxf") || p.to_lowercase().ends_with(".dwg") {
                    // 根據目前模式自動路由：2D → DraftDocument, 3D → Scene
                    let route_2d = {
                        #[cfg(feature = "drafting")]
                        { self.viewer.layout_mode }
                        #[cfg(not(feature = "drafting"))]
                        { false }
                    };
                    if route_2d {
                        #[cfg(feature = "drafting")]
                        {
                            match self.import_cad_to_2d_tab(&p) {
                                Ok(count) => {
                                    self.file_message = Some((format!("已匯入 {} 個 2D 圖元", count), std::time::Instant::now()));
                                }
                                Err(e) => {
                                    self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now()));
                                }
                            }
                        }
                    } else {
                        self.console_push("INFO", format!("[Import] DXF/DWG → 3D: {}", p));
                        match crate::dxf_io::import_dxf(&mut self.scene, &p) {
                            Ok(count) => {
                                self.editor.selected_ids.clear();
                                self.file_message = Some((format!("已匯入 {} 個 3D 物件", count), std::time::Instant::now()));
                            }
                            Err(e) => {
                                self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now()));
                            }
                        }
                    }
                }
            }
        }
    }
}

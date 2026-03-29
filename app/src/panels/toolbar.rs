use super::material_swatches::*;
use eframe::egui;
use crate::app::{DrawState, KolibriApp, PullFace, RightTab, ScaleHandle, SelectionMode, Tool, WorkMode};
use crate::scene::{MaterialKind, Shape};


impl KolibriApp {
    pub(crate) fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        // SketchUp-style compact buttons（32x32 vs 原本 48x48）
        let bsz = egui::vec2(36.0, 36.0);

        // ── Mode switch: 建模 / 鋼構 / 出圖 (compact row) ──
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().button_padding = egui::vec2(4.0, 3.0);

            let brand = egui::Color32::from_rgb(76, 139, 245);
            let steel_color = egui::Color32::from_rgb(220, 100, 50);
            let layout_color = egui::Color32::from_rgb(60, 160, 100);
            let muted = egui::Color32::from_rgb(110, 118, 135);

            let modeling_active = !self.viewer.layout_mode && self.editor.work_mode == WorkMode::Modeling;
            let steel_active = !self.viewer.layout_mode && self.editor.work_mode == WorkMode::Steel;
            let layout_active = self.viewer.layout_mode;

            let make_btn = |label: &str, active: bool, color: egui::Color32| {
                egui::Button::new(egui::RichText::new(label).size(10.0)
                    .color(if active { egui::Color32::WHITE } else { muted }))
                    .fill(if active { color } else { egui::Color32::TRANSPARENT })
                    .rounding(6.0)
            };

            if ui.add(make_btn("建模", modeling_active, brand)).clicked() {
                self.viewer.layout_mode = false;
                self.editor.work_mode = WorkMode::Modeling;
            }
            if ui.add(make_btn("鋼構", steel_active, steel_color)).clicked() {
                self.viewer.layout_mode = false;
                self.editor.work_mode = WorkMode::Steel;
            }
            if ui.add(make_btn("出圖", layout_active, layout_color)).clicked() {
                self.viewer.layout_mode = true;
            }
        });

        ui.add_space(2.0);

        // When in layout mode, don't show 3D tools
        if self.viewer.layout_mode {
            ui.separator();
            ui.label(egui::RichText::new("出圖模式").size(11.0).color(egui::Color32::from_gray(130)));
            ui.label(egui::RichText::new("右側面板可編輯\n紙張與圖框設定").size(10.0).color(egui::Color32::from_gray(160)));
            return;
        }

        // Steel mode uses a different variable now (work_mode), skip the old toggle
        let modeling_active = self.editor.work_mode == WorkMode::Modeling;
        let steel_active = self.editor.work_mode == WorkMode::Steel;
        // (The old m_btn/s_btn block below is now handled by the unified row above)
        // Skip the duplicate toggle — just keep the steel_mode sync
        // steel_mode is derived from work_mode (used elsewhere in the app)

        ui.separator();

        match self.editor.work_mode {
            WorkMode::Modeling => {
                // ── SketchUp-style tool layout: compact 2-column ──

                // Select & Transform
                ui.label(egui::RichText::new("選取").size(9.0).color(egui::Color32::from_gray(140)));
                self.tool_row(ui, bsz, &[
                    (Tool::Select,  "選取 (Space)"),
                    (Tool::Move,    "移動 (M)"),
                    (Tool::Rotate,  "旋轉 (Q)"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::Scale,   "縮放 (S)"),
                    (Tool::Eraser,  "刪除 (E)"),
                    (Tool::PaintBucket, "油漆桶"),
                ]);

                ui.add_space(2.0);

                // Draw
                ui.label(egui::RichText::new("繪圖").size(9.0).color(egui::Color32::from_gray(140)));
                let arc_tool = match self.editor.tool {
                    Tool::Arc3Point => Tool::Arc3Point,
                    Tool::Pie => Tool::Pie,
                    _ => Tool::Arc,
                };
                self.tool_row(ui, bsz, &[
                    (Tool::Line,      "線段 (L)"),
                    (arc_tool,        "弧線 (A)"),
                    (Tool::Rectangle, "矩形 (R)"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::Circle,    "圓形 (C)"),
                    (Tool::CreateBox, "方塊 (B)"),
                    (Tool::CreateCylinder, "圓柱"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::CreateSphere, "球體"),
                    (Tool::PushPull,     "推拉 (P)"),
                    (Tool::Offset,       "偏移 (F)"),
                ]);

                ui.add_space(2.0);

                // Modify
                ui.label(egui::RichText::new("修改").size(9.0).color(egui::Color32::from_gray(140)));
                self.tool_row(ui, bsz, &[
                    (Tool::FollowMe,  "跟隨"),
                    (Tool::Group,     "群組 (G)"),
                    (Tool::Component, "元件"),
                ]);

                ui.add_space(2.0);

                // Measure
                ui.label(egui::RichText::new("量測").size(9.0).color(egui::Color32::from_gray(140)));
                self.tool_row(ui, bsz, &[
                    (Tool::TapeMeasure, "捲尺 (T)"),
                    (Tool::Dimension,   "標註 (D)"),
                    (Tool::Text,        "文字"),
                ]);

                ui.add_space(2.0);

                // Camera
                ui.label(egui::RichText::new("相機").size(9.0).color(egui::Color32::from_gray(140)));
                self.tool_row(ui, bsz, &[
                    (Tool::Orbit,       "環繞 (O)"),
                    (Tool::Pan,         "平移 (H)"),
                    (Tool::ZoomExtents, "全部顯示 (Z)"),
                ]);

                ui.add_space(2.0);

                // Architecture
                ui.label(egui::RichText::new("建築").size(9.0).color(egui::Color32::from_gray(140)));
                self.tool_row(ui, bsz, &[
                    (Tool::Wall, "牆 (W)"),
                    (Tool::Slab, "板"),
                ]);
                // 牆/板參數（啟用時顯示）
                if matches!(self.editor.tool, Tool::Wall | Tool::Slab) {
                    ui.add_space(4.0);
                    figma_group(ui, |ui| {
                        if matches!(self.editor.tool, Tool::Wall) {
                            ui.add(egui::DragValue::new(&mut self.editor.wall_thickness)
                                .speed(10.0).prefix("牆厚: ").suffix(" mm").range(50.0..=1000.0));
                            ui.add(egui::DragValue::new(&mut self.editor.wall_height)
                                .speed(50.0).prefix("牆高: ").suffix(" mm").range(500.0..=20000.0));
                        }
                        if matches!(self.editor.tool, Tool::Slab) {
                            ui.add(egui::DragValue::new(&mut self.editor.slab_thickness)
                                .speed(10.0).prefix("板厚: ").suffix(" mm").range(50.0..=1000.0));
                        }
                    });
                }
                // 快速材質面板（PaintBucket 啟用時）
                if self.editor.tool == Tool::PaintBucket {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("快速材質").size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    ui.horizontal_wrapped(|ui| {
                        let mats = self.editor.recent_materials.clone();
                        for mat in &mats {
                            let c = mat.color();
                            let active = self.create_mat == *mat;
                            let btn_color = egui::Color32::from_rgb(
                                (c[0]*255.0) as u8, (c[1]*255.0) as u8, (c[2]*255.0) as u8);
                            let btn = egui::Button::new("")
                                .fill(btn_color)
                                .min_size(egui::vec2(22.0, 22.0))
                                .rounding(4.0)
                                .stroke(if active {
                                    egui::Stroke::new(2.0, egui::Color32::from_rgb(76, 139, 245))
                                } else { egui::Stroke::NONE });
                            if ui.add(btn).on_hover_text(mat.label()).clicked() {
                                self.create_mat = *mat;
                            }
                        }
                    });
                }
            }
            WorkMode::Steel => {
                // Steel tools
                self.tool_row(ui, bsz, &[
                    (Tool::SteelGrid, "軸線\n建立結構軸線系統"),
                    (Tool::SteelColumn, "柱\n點擊放置鋼柱 (Profile)"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::SteelBeam, "梁\n點兩點建立鋼梁"),
                    (Tool::SteelBrace, "斜撐\n點兩點建立斜撐"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::SteelPlate, "鋼板\n畫矩形建立鋼板"),
                    (Tool::SteelConnection, "接頭\n選兩構件建立接頭"),
                ]);

                ui.separator();

                // Steel defaults
                section_header(ui, "預設參數");
                figma_group(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Profile:").size(11.0));
                        ui.text_edit_singleline(&mut self.editor.steel_profile);
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("材質:").size(11.0));
                        ui.text_edit_singleline(&mut self.editor.steel_material);
                    });
                    ui.add(egui::DragValue::new(&mut self.editor.steel_height)
                        .speed(10.0).prefix("柱高: ").suffix(" mm").range(100.0..=50000.0));
                });

                // Common tools (shared between modes)
                ui.separator();
                section_header(ui, "通用");
                self.tool_row(ui, bsz, &[
                    (Tool::Select, "選取 (Space)"),
                    (Tool::Move, "移動 (M)"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::Eraser, "刪除 (E)"),
                    (Tool::TapeMeasure, "量測 (T)"),
                ]);
            }
        }

        // ── Bottom ──
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            ui.small(format!("{}", self.scene.objects.len()));
        });
    }

    pub(crate) fn tool_row(&mut self, ui: &mut egui::Ui, bsz: egui::Vec2, tools: &[(Tool, &str)]) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for &(tool, tip) in tools {
                let active = self.editor.tool == tool;
                let implemented = tool.is_implemented();

                // Allocate button space
                let (rect, resp) = ui.allocate_exact_size(bsz, egui::Sense::click());

                // Light glassmorphism button style
                let bg = if active {
                    egui::Color32::from_rgba_unmultiplied(76, 139, 245, 36) // brand_soft
                } else if resp.hovered() && implemented {
                    egui::Color32::from_rgb(240, 242, 248) // light hover
                } else {
                    egui::Color32::TRANSPARENT
                };
                let border_color = if active {
                    egui::Color32::from_rgb(76, 139, 245)
                } else {
                    egui::Color32::TRANSPARENT
                };
                ui.painter().rect_filled(rect, 12.0, bg);
                if active {
                    ui.painter().rect_stroke(rect, 12.0, egui::Stroke::new(1.0, border_color));
                }

                // Icon color (dark on light)
                let icon_color = if active {
                    egui::Color32::from_rgb(76, 139, 245) // brand blue
                } else if !implemented {
                    egui::Color32::from_gray(200) // very dim on light
                } else if resp.hovered() {
                    egui::Color32::from_rgb(31, 36, 48) // dark text
                } else {
                    egui::Color32::from_rgb(110, 118, 135) // muted
                };
                let icon_rect = rect.shrink(6.0);
                crate::icons::draw_tool_icon(ui.painter(), icon_rect, tool, icon_color);

                // Shortcut key label (bottom-right corner)
                let shortcut = match tool {
                    Tool::Select => Some("Space"),
                    Tool::Move => Some("M"),
                    Tool::Rotate => Some("Q"),
                    Tool::Scale => Some("S"),
                    Tool::Line => Some("L"),
                    Tool::Arc => Some("A"),
                    Tool::Rectangle => Some("R"),
                    Tool::Circle => Some("C"),
                    Tool::CreateBox => Some("B"),
                    Tool::PushPull => Some("P"),
                    Tool::Offset => Some("F"),
                    Tool::TapeMeasure => Some("T"),
                    Tool::Dimension => Some("D"),
                    Tool::Orbit => Some("O"),
                    Tool::Pan => Some("H"),
                    Tool::ZoomExtents => Some("Z"),
                    Tool::Group => Some("G"),
                    Tool::Eraser => Some("E"),
                    _ => None,
                };
                if let Some(key) = shortcut {
                    ui.painter().text(
                        egui::pos2(rect.right() - 3.0, rect.bottom() - 2.0),
                        egui::Align2::RIGHT_BOTTOM,
                        key, egui::FontId::proportional(9.0),
                        egui::Color32::from_rgb(160, 166, 180),
                    );
                }

                // Click handling
                if resp.clicked() && implemented {
                    self.console_push("TOOL", format!("工具列點擊: {:?}", tool));
                    self.editor.tool = tool;
                    self.editor.draw_state = DrawState::Idle;
                    // Inference 2.0: sync tool to inference context
                    self.editor.inference_ctx.current_tool = tool;
                    crate::inference::reset_context(&mut self.editor.inference_ctx);
                    self.editor.inference_ctx.current_tool = tool;
                    match tool {
                        Tool::ZoomExtents => self.zoom_extents(),
                        Tool::Eraser => {
                            for id in std::mem::take(&mut self.editor.selected_ids) {
                                self.scene.delete(&id);
                            }
                        }
                        _ => {}
                    }
                    if matches!(tool, Tool::CreateBox | Tool::CreateCylinder | Tool::CreateSphere
                        | Tool::Rectangle | Tool::Circle | Tool::Line | Tool::Arc | Tool::Arc3Point | Tool::Pie) {
                        self.right_tab = RightTab::Create;
                    }
                }

                let tooltip = if implemented { tip.to_string() }
                    else { format!("{} (尚未實作)", tip) };
                resp.on_hover_text(tooltip);
            }
        });
    }

    // ── Right panel ─────────────────────────────────────────────────────────
}


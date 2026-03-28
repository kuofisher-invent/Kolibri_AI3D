use super::material_swatches::*;
use eframe::egui;
use crate::app::{DrawState, KolibriApp, PullFace, RightTab, ScaleHandle, SelectionMode, Tool, WorkMode};
use crate::scene::{MaterialKind, Shape};


impl KolibriApp {
    pub(crate) fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let bsz = egui::vec2(48.0, 48.0);

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
                // ── Select & Transform ──
                self.tool_row(ui, bsz, &[
                    (Tool::Select,  "選取\n點擊選取物件，拖曳旋轉視角 (Space)"),
                    (Tool::Move,    "移動\n選取物件後拖曳移動位置 (M)"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::Rotate,  "旋轉\n點擊物件旋轉90度 (Q)"),
                    (Tool::Scale,   "縮放\n點擊物件後上下拖曳等比縮放 (S)"),
                ]);

                ui.separator();

                // ── Draw 2D ──
                // 弧線按鈕：顯示當前模式的圖標（Ctrl+A 循環切換）
                let arc_tool = match self.editor.tool {
                    Tool::Arc3Point => Tool::Arc3Point,
                    Tool::Pie => Tool::Pie,
                    _ => Tool::Arc,
                };
                let arc_tip = match arc_tool {
                    Tool::Arc3Point => "三點弧\nCtrl+A 切換模式 (A)",
                    Tool::Pie       => "扇形\nCtrl+A 切換模式 (A)",
                    _               => "兩點弧\nCtrl+A 切換模式 (A)",
                };
                self.tool_row(ui, bsz, &[
                    (Tool::Line,  "線段\n連續點擊繪製線段，ESC結束 (L)"),
                    (arc_tool,    arc_tip),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::Rectangle, "矩形\n點擊兩角定義底面，再拉高度 (R)"),
                    (Tool::Circle,    "圓形\n點擊圓心，拖出半徑，再拉高度 (C)"),
                ]);

                ui.separator();

                // ── Draw 3D ──
                self.tool_row(ui, bsz, &[
                    (Tool::CreateBox,      "方塊\n點擊兩角定義底面，再拉出高度 (B)"),
                    (Tool::CreateCylinder, "圓柱\n點擊圓心→拖出半徑→拉出高度"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::CreateSphere,   "球體\n點擊圓心→拖出半徑"),
                    (Tool::PushPull,       "推拉\n點擊物件面後拖曳拉伸 (P)"),
                ]);

                ui.separator();

                // ── Modify ──
                self.tool_row(ui, bsz, &[
                    (Tool::Offset,   "偏移\n點擊方塊面，拖曳產生內縮邊框 (F)"),
                    (Tool::FollowMe, "跟隨複製\n點擊物件，自動複製並切換移動工具"),
                ]);

                ui.separator();

                // ── Group & Component ──
                self.tool_row(ui, bsz, &[
                    (Tool::Group,     "群組\n將選取的多個物件合併為群組 (G)"),
                    (Tool::Component, "元件\n將選取物件存為可重複使用的元件"),
                ]);

                ui.separator();

                // ── Measure & Paint ──
                self.tool_row(ui, bsz, &[
                    (Tool::TapeMeasure,  "捲尺\n量測兩點之間的距離 (T)"),
                    (Tool::Dimension,    "標註\n兩點標註距離 (D)"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::Text,         "文字\n點擊放置文字標籤"),
                    (Tool::PaintBucket,  "油漆桶\n點擊物件套用目前選擇的材質"),
                ]);

                ui.separator();

                // ── Camera ──
                self.tool_row(ui, bsz, &[
                    (Tool::Orbit, "環繞\n左鍵拖曳旋轉3D視角 (O)"),
                    (Tool::Pan,   "平移\n左鍵拖曳平移視角 (H)"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::ZoomExtents, "全部顯示\n自動縮放至顯示所有物件 (Z)"),
                    (Tool::Eraser,      "橡皮擦\n點擊物件直接刪除 (E)"),
                ]);
                ui.separator();
                ui.label(egui::RichText::new("建築").size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
                self.tool_row(ui, bsz, &[
                    (Tool::Wall, "牆\n兩點畫牆（W）"),
                    (Tool::Slab, "板\n兩角畫板"),
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
                let icon_rect = rect.shrink(8.0);
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


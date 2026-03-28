use super::material_swatches::*;
use eframe::egui;
use crate::app::{DrawState, KolibriApp, PullFace, RightTab, ScaleHandle, SelectionMode, Tool, WorkMode};
use crate::scene::{MaterialKind, Shape};


impl KolibriApp {
    pub(crate) fn tab_create(&mut self, ui: &mut egui::Ui) {
        // ── 弧線模式切換（當 Arc/Arc3Point/Pie 工具啟用時顯示）──
        if matches!(self.editor.tool, Tool::Arc | Tool::Arc3Point | Tool::Pie) {
            section_frame_full(ui, |ui| {
                section_header_text(ui, "ARC MODE");
                ui.horizontal(|ui| {
                    let modes = [
                        (Tool::Arc,       "兩點弧"),
                        (Tool::Arc3Point, "三點弧"),
                        (Tool::Pie,       "扇形"),
                    ];
                    for (tool, label) in modes {
                        if ui.selectable_label(self.editor.tool == tool, label).clicked() {
                            self.console_push("TOOL", format!("弧線模式: {}", label));
                            self.editor.tool = tool;
                            self.editor.draw_state = DrawState::Idle;
                        }
                    }
                });
                let desc = match self.editor.tool {
                    Tool::Arc       => "起點 → 終點 → 凸度拖曳（半圓自動鎖定）",
                    Tool::Arc3Point => "任意三點定義圓弧",
                    Tool::Pie       => "中心 → 邊緣定半徑 → 第二邊緣定角度",
                    _ => "",
                };
                ui.small(desc);
            });
            ui.add_space(8.0);
        }

        ui.label(egui::RichText::new("新物件材質").strong());
        // Material preview sphere
        {
            let preview_size = 80.0;
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(preview_size, preview_size), egui::Sense::hover()
            );
            draw_material_preview(ui.painter(), rect, &self.create_mat);
        }
        // SketchUp-style material browser (shared picker)
        if let Some(new_mat) = material_picker_ui(
            ui,
            self.create_mat,
            &mut self.mat_search,
            &mut self.mat_category_idx,
            &mut self.show_custom_color_picker,
        ) {
            self.create_mat = new_mat;
        }

        ui.add_space(12.0);

        // ── 繪圖設定 ──
        section_frame_full(ui, |ui| {
            section_header_text(ui, "DRAWING SETTINGS");

            // 圓柱/球體細分數
            ui.horizontal(|ui| {
                ui.label("圓弧細分");
                // Store segments as a temporary — can't easily change global default yet
                ui.label(egui::RichText::new("32 段").color(egui::Color32::from_rgb(110, 118, 135)));
            });

            ui.add_space(4.0);

            // 快速建立物件（帶預設尺寸）
            ui.label(egui::RichText::new("快速建立").size(11.0).strong());
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if ui.button("方塊 1m").clicked() {
                    let id = self.scene.add_box("QuickBox".into(), [0.0, 0.0, 0.0], 1000.0, 1000.0, 1000.0, self.create_mat);
                    self.editor.selected_ids = vec![id];
                    self.right_tab = RightTab::Properties;
                }
                if ui.button("方塊 3m").clicked() {
                    let id = self.scene.add_box("QuickBox".into(), [0.0, 0.0, 0.0], 3000.0, 3000.0, 3000.0, self.create_mat);
                    self.editor.selected_ids = vec![id];
                    self.right_tab = RightTab::Properties;
                }
            });
            ui.horizontal(|ui| {
                if ui.button("圓柱 r1m").clicked() {
                    let id = self.scene.add_cylinder("QuickCyl".into(), [0.0, 0.0, 0.0], 1000.0, 2000.0, 32, self.create_mat);
                    self.editor.selected_ids = vec![id];
                    self.right_tab = RightTab::Properties;
                }
                if ui.button("球體 r1m").clicked() {
                    let id = self.scene.add_sphere("QuickSphere".into(), [0.0, 0.0, 0.0], 1000.0, 32, self.create_mat);
                    self.editor.selected_ids = vec![id];
                    self.right_tab = RightTab::Properties;
                }
            });
        });

        ui.add_space(12.0);

        // ── 標註樣式設定（CAD style dimension settings）──
        section_frame_full(ui, |ui| {
            section_header_text(ui, "DIMENSION STYLE");

            let ds = &mut self.dim_style;

            ui.horizontal(|ui| {
                ui.label("線粗");
                ui.add(egui::Slider::new(&mut ds.line_thickness, 0.5..=4.0).step_by(0.5).suffix(" px"));
            });
            ui.horizontal(|ui| {
                ui.label("延伸線粗");
                ui.add(egui::Slider::new(&mut ds.ext_line_thickness, 0.25..=3.0).step_by(0.25).suffix(" px"));
            });
            ui.horizontal(|ui| {
                ui.label("文字大小");
                ui.add(egui::Slider::new(&mut ds.text_size, 8.0..=20.0).step_by(1.0).suffix(" px"));
            });
            ui.horizontal(|ui| {
                ui.label("箭頭大小");
                ui.add(egui::Slider::new(&mut ds.arrow_size, 3.0..=15.0).step_by(1.0).suffix(" px"));
            });
            ui.horizontal(|ui| {
                ui.label("偏移量");
                ui.add(egui::Slider::new(&mut ds.offset, 5.0..=50.0).step_by(1.0).suffix(" px"));
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("箭頭樣式");
                let styles = [
                    (crate::dimensions::ArrowStyle::Tick, "短橫"),
                    (crate::dimensions::ArrowStyle::Arrow, "箭頭"),
                    (crate::dimensions::ArrowStyle::Dot, "圓點"),
                    (crate::dimensions::ArrowStyle::None, "無"),
                ];
                for (style, label) in styles {
                    if ui.selectable_label(ds.arrow_style == style, label).clicked() {
                        ds.arrow_style = style;
                    }
                }
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("單位");
                let units = [
                    (crate::dimensions::UnitDisplay::Auto, "自動"),
                    (crate::dimensions::UnitDisplay::Mm, "mm"),
                    (crate::dimensions::UnitDisplay::Cm, "cm"),
                    (crate::dimensions::UnitDisplay::M, "m"),
                ];
                for (unit, label) in units {
                    if ui.selectable_label(ds.unit_display == unit, label).clicked() {
                        ds.unit_display = unit;
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label("小數位");
                ui.add(egui::Slider::new(&mut ds.precision, 0..=3));
            });

            ui.checkbox(&mut ds.show_bg, "顯示文字背景");

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("線條顏色");
                let mut c = egui::Color32::from_rgba_unmultiplied(ds.line_color[0], ds.line_color[1], ds.line_color[2], ds.line_color[3]);
                if ui.color_edit_button_srgba(&mut c).changed() {
                    ds.line_color = [c.r(), c.g(), c.b(), c.a()];
                }
            });
            ui.horizontal(|ui| {
                ui.label("文字顏色");
                let mut c = egui::Color32::from_rgba_unmultiplied(ds.text_color[0], ds.text_color[1], ds.text_color[2], ds.text_color[3]);
                if ui.color_edit_button_srgba(&mut c).changed() {
                    ds.text_color = [c.r(), c.g(), c.b(), c.a()];
                }
            });
        });
    }

    pub(crate) fn tab_ai_log(&mut self, ui: &mut egui::Ui) {
        ui.heading("AI 修改記錄");
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("匯出記錄").clicked() {
                let _ = self.ai_log.save_to_file("ai_log.json");
                self.file_message = Some(("記錄已匯出".into(), std::time::Instant::now()));
            }
            if ui.button("清除記錄").clicked() {
                self.ai_log.clear();
            }
        });
        ui.separator();

        let entries: Vec<_> = self.ai_log.entries().iter().rev().cloned().collect();
        for entry in &entries {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    let color = match entry.actor.name.as_str() {
                        "Claude" => egui::Color32::from_rgb(100, 180, 255),
                        "使用者" => egui::Color32::from_rgb(180, 220, 180),
                        _ => egui::Color32::from_rgb(255, 180, 100),
                    };
                    ui.colored_label(color, &entry.actor.display_name());
                    ui.label(&entry.timestamp);
                });
                ui.label(egui::RichText::new(&entry.action).strong());
                if !entry.details.is_empty() {
                    ui.label(&entry.details);
                }
                if !entry.objects_affected.is_empty() {
                    ui.small(format!("物件: {}", entry.objects_affected.join(", ")));
                }
            });
        }

        if entries.is_empty() {
            ui.label("尚無記錄");
        }
    }

    pub(crate) fn tab_scene(&mut self, ui: &mut egui::Ui) {
        // ── PAGES / SCENES ──
        section_header(ui, "PAGES / SCENES");
        figma_group(ui, |ui| {
            let scene_name = self.current_file.as_ref()
                .and_then(|p| p.rsplit(['\\', '/']).next())
                .unwrap_or("Scene 1");
            let obj_count = self.scene.objects.len();

            ui.horizontal(|ui| {
                let (thumb_rect, _) = ui.allocate_exact_size(egui::vec2(48.0, 36.0), egui::Sense::hover());
                ui.painter().rect_filled(thumb_rect, 8.0, egui::Color32::from_rgb(230, 233, 240));
                ui.painter().text(thumb_rect.center(), egui::Align2::CENTER_CENTER,
                    "\u{1f3d7}", egui::FontId::proportional(16.0),
                    egui::Color32::from_rgb(110, 118, 135));

                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(scene_name).strong().size(12.0).color(egui::Color32::from_rgb(31, 36, 48)));
                    ui.label(egui::RichText::new(format!("{} objects", obj_count)).size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
                });
            });
        });

        ui.add_space(8.0);

        // ── QUICK ACTIONS ──
        section_header(ui, "QUICK ACTIONS");
        figma_group(ui, |ui| {
            ui.columns(2, |cols| {
                if cols[0].button(egui::RichText::new("+ 新場景").size(11.0)).clicked() {
                    self.handle_menu_action(crate::menu::MenuAction::NewScene);
                }
                if cols[1].button(egui::RichText::new("\u{1f4c2} 開啟").size(11.0)).clicked() {
                    self.handle_menu_action(crate::menu::MenuAction::OpenScene);
                }
            });
            ui.add_space(2.0);
            ui.columns(2, |cols| {
                if cols[0].button(egui::RichText::new("\u{1f4e5} 匯入 OBJ").size(11.0)).clicked() {
                    self.handle_menu_action(crate::menu::MenuAction::ImportObj);
                }
                if cols[1].button(egui::RichText::new("\u{1f4e4} 匯出").size(11.0)).clicked() {
                    self.handle_menu_action(crate::menu::MenuAction::ExportObj);
                }
            });
            ui.add_space(2.0);
            ui.columns(2, |cols| {
                if cols[0].button(egui::RichText::new("\u{1f4e6} 群組").size(11.0)).clicked() {
                    self.editor.tool = Tool::Group;
                }
                if cols[1].button(egui::RichText::new("\u{1f50d} 全部顯示").size(11.0)).clicked() {
                    self.zoom_extents();
                }
            });
        });

        ui.add_space(8.0);

        // ── SNAP ──
        section_header(ui, "SNAP");
        figma_group(ui, |ui| {
            ui.columns(3, |cols| {
                cols[0].label(egui::RichText::new("● 端點").size(10.0));
                cols[1].label(egui::RichText::new("○ 中點").size(10.0));
                cols[2].label(egui::RichText::new("✖ 交點").size(10.0));
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("靈敏度").size(10.0));
                ui.add(egui::Slider::new(&mut self.editor.snap_threshold, 5.0..=40.0).step_by(1.0).suffix("px"));
            });
        });

        ui.add_space(8.0);

        // ── LAYERS / OBJECTS ──
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("場景物件").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.small(format!("共 {}", self.scene.objects.len()));
            });
        });
        ui.separator();

        // Layer/tag filter section
        {
            let tags: Vec<String> = {
                let mut set = std::collections::BTreeSet::new();
                for o in self.scene.objects.values() {
                    set.insert(o.tag.clone());
                }
                set.into_iter().collect()
            };
            if !tags.is_empty() {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("圖層").strong());
                    for tag in &tags {
                        let visible = !self.viewer.hidden_tags.contains(tag);
                        let label = if visible { format!("\u{1f441} {}", tag) } else { format!("   {}", tag) };
                        if ui.selectable_label(visible, &label).clicked() {
                            if visible { self.viewer.hidden_tags.insert(tag.clone()); }
                            else { self.viewer.hidden_tags.remove(tag); }
                        }
                    }
                });
                ui.separator();
            }
        }

        // Groups
        if !self.scene.groups.is_empty() {
            ui.label(egui::RichText::new("群組").strong());
            let groups: Vec<_> = self.scene.groups.values().cloned().collect();
            let mut dissolve_id = None;
            for g in &groups {
                ui.horizontal(|ui| {
                    let label = format!("\u{1f4c1} {} ({} 物件)", g.name, g.children.len());
                    if ui.selectable_label(false, &label).clicked() {
                        // Select all children
                        self.editor.selected_ids = g.children.clone();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("解散").clicked() {
                            dissolve_id = Some(g.id.clone());
                        }
                    });
                });
            }
            if let Some(gid) = dissolve_id {
                self.scene.dissolve_group(&gid);
            }
            ui.separator();
        }

        // Component definitions
        if !self.scene.component_defs.is_empty() {
            ui.label(egui::RichText::new("元件定義").strong());
            let defs: Vec<_> = self.scene.component_defs.values().cloned().collect();
            for def in &defs {
                ui.horizontal(|ui| {
                    let instance_count = self.scene.objects.values()
                        .filter(|o| o.tag == format!("元件:{}", def.id))
                        .count();
                    let label = format!("\u{1f537} {} ({} 個實例)", def.name, instance_count);
                    ui.label(&label);
                });
            }
            ui.separator();
        }

        if self.scene.objects.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                ui.label(egui::RichText::new("場景為空").color(egui::Color32::GRAY));
            });
            return;
        }
        self.render_scene_hierarchy(ui);

        // ── Undo History ──
        ui.separator();
        section_header(ui, "UNDO HISTORY");
        figma_group(ui, |ui| {
            let undo_count = self.scene.undo_count();
            let redo_count = self.scene.redo_count();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("↩ {} 步可復原", undo_count)).size(11.0));
                ui.label(egui::RichText::new(format!("↪ {} 步可重做", redo_count)).size(11.0));
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(self.scene.can_undo(), egui::Button::new("復原").small()).clicked() {
                    self.scene.undo();
                }
                if ui.add_enabled(self.scene.can_redo(), egui::Button::new("重做").small()).clicked() {
                    self.scene.redo();
                }
                if ui.add_enabled(undo_count > 0, egui::Button::new("全部復原").small()).clicked() {
                    while self.scene.can_undo() { self.scene.undo(); }
                }
            });
            // 顯示 undo stack 條目標籤（如果有 Diff 類型）
            for (i, entry) in self.scene.undo_stack_v2.iter().rev().enumerate().take(8) {
                let label = match entry {
                    kolibri_core::command::UndoEntry::Diff(d) => format!("  {} — {}", undo_count - i, d.label),
                    kolibri_core::command::UndoEntry::Full(..) => format!("  {} — 快照", undo_count - i),
                };
                ui.label(egui::RichText::new(label).size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
            }
        });

        ui.separator();
        if ui.button("🧹 清空").clicked() { self.scene.clear(); self.editor.selected_ids.clear(); }
    }

    pub(crate) fn status_text(&self) -> String {
        let snap_info = if let Some(ref snap) = self.editor.snap_result {
            let label = snap.snap_type.label();
            if !label.is_empty() && !matches!(self.editor.draw_state, DrawState::Idle) {
                format!("  [{}]", label)
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        // 滑鼠座標
        let coord = if let Some(p) = self.editor.mouse_ground {
            format!("  X:{:.0} Y:{:.0} Z:{:.0}", p[0], p[1], p[2])
        } else { String::new() };

        let base = match &self.editor.draw_state {
            DrawState::Idle => match self.editor.tool {
                Tool::Select      => "選取 — 點擊選取物件, 左鍵拖曳旋轉, 中鍵平移".into(),
                Tool::Move        => "移動 — 選取物件後拖曳移動".into(),
                Tool::Rotate      => "旋轉 — 點擊物件旋轉90度 (Q)".into(),
                Tool::Scale       => "縮放 — 點擊物件後上下拖曳等比縮放 (S)".into(),
                Tool::Line        => "線段 — 點擊設定起點, 再點擊設定終點".into(),
                Tool::Arc         => "弧線 — 起點→終點→凸度（真圓弧，半圓自動鎖定）(A)".into(),
                Tool::Arc3Point   => "三點圓弧 — 任意三點定義圓弧".into(),
                Tool::Pie         => "扇形 — 中心→邊緣定半徑→第二邊緣定角度".into(),
                Tool::Rectangle   => "矩形 — 點擊地面設定第一角, 等同方塊底面".into(),
                Tool::Circle      => "圓形 — 點擊地面設定圓心, 等同圓柱底面".into(),
                Tool::CreateBox   => "方塊 — 點擊地面設定第一角".into(),
                Tool::CreateCylinder => "圓柱 — 點擊地面設定圓心".into(),
                Tool::CreateSphere   => "球體 — 點擊地面設定圓心".into(),
                Tool::PushPull    => "推拉 — 點擊物件的面，拖曳沿法線方向拉伸 (P)".into(),
                Tool::Offset      => "偏移 — 點擊方塊面，拖曳產生內縮/外擴邊框，放開後自動切換推拉 (F)".into(),
                Tool::FollowMe    => "跟隨複製 — 點擊物件，自動複製並切換移動工具".into(),
                Tool::TapeMeasure => "捲尺 — 點擊兩點量測距離".into(),
                Tool::Dimension   => "標註 — 點擊兩點建立持久標註 (D)".into(),
                Tool::Text        => "文字 — 點擊放置文字標籤".into(),
                Tool::PaintBucket => "油漆桶 — 點擊物件套用目前材質".into(),
                Tool::Orbit       => "環繞 — 左鍵拖曳旋轉視角, WASD走動".into(),
                Tool::Pan         => "平移 — 左鍵拖曳平移視角".into(),
                Tool::ZoomExtents => "全部顯示".into(),
                Tool::Group       => "群組 — 點擊物件標記為群組".into(),
                Tool::Component   => "元件 — 點擊物件標記為可重複使用的元件".into(),
                Tool::Eraser      => "橡皮擦 — 點擊物件刪除".into(),
                Tool::SteelGrid   => "軸線 — 點擊放置軸線".into(),
                Tool::SteelColumn => format!("柱 — 點擊放置 {} 柱", self.editor.steel_profile),
                Tool::SteelBeam   => "梁 — 點擊起點，再點擊終點".into(),
                Tool::SteelBrace  => "斜撐 — 點擊起點，再點擊終點".into(),
                Tool::SteelPlate  => "鋼板 — 畫矩形，再推拉厚度".into(),
                Tool::SteelConnection => "接頭 — 選取兩個構件".into(),
                Tool::Wall => format!("牆 (W) — 點擊兩點畫牆（厚{:.0}mm 高{:.0}mm）", self.editor.wall_thickness, self.editor.wall_height),
                Tool::Slab => format!("板 — 點擊兩角畫板（厚{:.0}mm）", self.editor.slab_thickness),
            },
            DrawState::BoxBase { .. } => "移動滑鼠拖出底面矩形, 點擊確認".into(),
            DrawState::BoxHeight { .. } => "上下移動設定高度, 點擊確認 (或輸入數字+Enter)".into(),
            DrawState::CylBase { .. } => "移動滑鼠拖出半徑, 點擊確認".into(),
            DrawState::CylHeight { .. } => "上下移動設定高度, 點擊確認".into(),
            DrawState::SphRadius { .. } => "移動滑鼠拖出半徑, 點擊確認".into(),
            DrawState::Pulling { face, .. } => {
                let face_name = match face {
                    PullFace::Top => "頂面", PullFace::Bottom => "底面",
                    PullFace::Front => "前面", PullFace::Back => "後面",
                    PullFace::Left => "左面", PullFace::Right => "右面",
                };
                format!("推拉 {} — 拖曳拉伸, 放開確認", face_name)
            }
            DrawState::LineFrom { .. } => "移動到下一點, 點擊確認 (ESC 結束)".into(),
            DrawState::ArcP1 { .. } => "點擊設定弧線終點".into(),
            DrawState::ArcP2 { .. } => "移動設定弧度（半圓自動鎖定），點擊確認".into(),
            DrawState::PieCenter { .. } => "點擊設定扇形半徑終點".into(),
            DrawState::PieRadius { .. } => "移動設定扇形角度，點擊確認".into(),
            DrawState::RotateRef { .. } => {
                "點擊設定參考方向（0° 線）".into()
            }
            DrawState::RotateAngle { ref_angle, current_angle, .. } => {
                let delta_deg = (current_angle - ref_angle).to_degrees();
                format!("旋轉 {:.1}° — 點擊確認, 輸入角度+Enter 精確旋轉", delta_deg)
            }
            DrawState::Scaling { handle, .. } => {
                let axis = match handle {
                    ScaleHandle::Uniform => "等比縮放",
                    ScaleHandle::AxisX => "X軸縮放（寬度）",
                    ScaleHandle::AxisY => "Y軸縮放（高度）",
                    ScaleHandle::AxisZ => "Z軸縮放（深度）",
                };
                format!("縮放 — {} | 輸入比例(x1.5)或尺寸(mm)+Enter", axis)
            }
            DrawState::Offsetting { distance, .. } => {
                let label = if *distance >= 0.0 { "內縮" } else { "外擴" };
                format!("偏移{} {:.0}mm — 右拖內縮/左拖外擴, 放開確認", label, distance.abs())
            }
            DrawState::Measuring { start } => {
                if let Some(p2) = self.editor.mouse_ground {
                    let dx = p2[0] - start[0];
                    let dz = p2[2] - start[2];
                    let dist = (dx*dx + dz*dz).sqrt();
                    let dist_text = if dist >= 1000.0 {
                        format!("{:.2} m", dist / 1000.0)
                    } else {
                        format!("{:.0} mm", dist)
                    };
                    let angle_deg = dz.atan2(dx).to_degrees();
                    format!("捲尺 — 距離: {} | 角度: {:.1}° | 點擊確認 / ESC 取消", dist_text, angle_deg)
                } else {
                    "捲尺 — 點擊第二點完成量測 [捕捉中]".to_string()
                }
            }
            DrawState::PullingFreeMesh { .. } => "推拉自由面 — 拖曳拉伸, 放開確認".into(),
            DrawState::WallFrom { .. } => "牆工具 — 點擊設定終點（連續畫牆，ESC 結束）".into(),
            DrawState::SlabCorner { .. } => "板工具 — 點擊設定第二角".into(),
            DrawState::FollowPath { path_points, .. } => {
                if path_points.is_empty() {
                    "跟隨 — 點擊地面定義路徑".into()
                } else {
                    format!("跟隨 — 已定義 {} 個路徑點 | Enter 完成", path_points.len())
                }
            }
        };

        let base_text = format!("{}{}", base, snap_info);

        // Append cursor world coordinates
        if let Some(p) = self.editor.mouse_ground {
            format!("{}{} | X:{:.0} Y:{:.0} Z:{:.0}", base_text, coord, p[0], p[1], p[2])
        } else {
            base_text
        }
    }
}

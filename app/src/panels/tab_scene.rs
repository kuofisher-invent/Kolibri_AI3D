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
        // ── Scene summary（緊湊的場景摘要列）──
        {
            let scene_name = self.current_file.as_ref()
                .and_then(|p| p.rsplit(['\\', '/']).next())
                .unwrap_or("Scene 1");
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(scene_name).strong().size(11.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("{} 物件", self.scene.objects.len())).size(10.0).color(egui::Color32::from_gray(140)));
                });
            });
            // Quick action row（一列按鈕）
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                ui.spacing_mut().button_padding = egui::vec2(6.0, 2.0);
                if ui.small_button("新建").clicked() { self.handle_menu_action(crate::menu::MenuAction::NewScene); }
                if ui.small_button("開啟").clicked() { self.handle_menu_action(crate::menu::MenuAction::OpenScene); }
                if ui.small_button("匯入").clicked() { self.handle_menu_action(crate::menu::MenuAction::ImportObj); }
                if ui.small_button("匯出").clicked() { self.handle_menu_action(crate::menu::MenuAction::ExportObj); }
                if ui.small_button("全顯").clicked() { self.zoom_extents(); }
            });
        }
        ui.separator();

        // ── LAYERS / OBJECTS ──
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("場景物件").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.small(format!("共 {}", self.scene.objects.len()));
            });
        });
        ui.separator();

        // ── TAGS / LAYERS（可折疊）──
        let tag_count = {
            let mut set = std::collections::BTreeSet::new();
            for o in self.scene.objects.values() { set.insert(o.tag.clone()); }
            set.len()
        };
        egui::CollapsingHeader::new(egui::RichText::new(format!("Tags ({})", tag_count)).size(11.0).strong())
            .default_open(true)
            .show(ui, |ui| {
            let tags: Vec<(String, usize)> = {
                let mut map = std::collections::BTreeMap::new();
                for o in self.scene.objects.values() {
                    *map.entry(o.tag.clone()).or_insert(0usize) += 1;
                }
                map.into_iter().collect()
            };
            if !tags.is_empty() {
                // SU-style tag color palette
                let tag_colors = [
                    egui::Color32::from_rgb(76, 139, 245),   // blue
                    egui::Color32::from_rgb(220, 80, 60),    // red
                    egui::Color32::from_rgb(60, 180, 90),    // green
                    egui::Color32::from_rgb(245, 180, 40),   // yellow
                    egui::Color32::from_rgb(160, 90, 200),   // purple
                    egui::Color32::from_rgb(80, 200, 200),   // cyan
                    egui::Color32::from_rgb(200, 120, 60),   // orange
                    egui::Color32::from_rgb(140, 140, 140),  // grey
                ];
                figma_group(ui, |ui| {
                    for (i, (tag, count)) in tags.iter().enumerate() {
                        let visible = !self.viewer.hidden_tags.contains(tag);
                        let color = tag_colors[i % tag_colors.len()];
                        ui.horizontal(|ui| {
                            // 色彩圓點
                            let (dot_rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(dot_rect.center(), 4.0, if visible { color } else { egui::Color32::from_gray(180) });

                            // 眼睛圖示 + 名稱
                            let eye = if visible { "\u{1f441}" } else { "—" };
                            let label_text = format!("{} {} ({})", eye, tag, count);
                            let resp = ui.selectable_label(visible, egui::RichText::new(&label_text).size(11.0));
                            if resp.clicked() {
                                if visible { self.viewer.hidden_tags.insert(tag.clone()); }
                                else { self.viewer.hidden_tags.remove(tag); }
                            }
                        });
                    }
                });
                ui.add_space(4.0);
            }
        });  // end Tags collapsing header

        // ── STYLES（可折疊）──
        egui::CollapsingHeader::new(egui::RichText::new("Styles").size(11.0).strong())
            .default_open(true)
            .show(ui, |ui| {
            let styles = [
                ("著色", 0u32, "標準著色模式"),
                ("線框", 1, "只顯示邊線"),
                ("X 光", 2, "透明面 + 邊線"),
                ("隱藏線", 3, "白色面 + 邊線"),
                ("單色", 4, "灰色面 + 邊線"),
                ("草稿", 5, "純黑邊線（無面）"),
            ];
            ui.columns(3, |cols| {
                for (i, (name, mode, tooltip)) in styles.iter().enumerate() {
                    let col = &mut cols[i % 3];
                    let is_current = self.viewer.render_mode.as_u32() == *mode;
                    let btn = egui::Button::new(
                        egui::RichText::new(*name).size(10.0).strong()
                            .color(if is_current { egui::Color32::WHITE } else { egui::Color32::from_rgb(60, 65, 80) })
                    )
                    .fill(if is_current { egui::Color32::from_rgb(76, 139, 245) } else { egui::Color32::from_gray(235) })
                    .rounding(6.0);
                    if col.add(btn).on_hover_text(*tooltip).clicked() {
                        self.viewer.render_mode = match mode {
                            0 => crate::viewer::RenderMode::Shaded,
                            1 => crate::viewer::RenderMode::Wireframe,
                            2 => crate::viewer::RenderMode::XRay,
                            3 => crate::viewer::RenderMode::HiddenLine,
                            4 => crate::viewer::RenderMode::Monochrome,
                            _ => crate::viewer::RenderMode::Sketch,
                        };
                    }
                }
            });
            ui.add_space(4.0);
            // Edge thickness
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("邊線").size(10.0));
                ui.add(egui::Slider::new(&mut self.viewer.edge_thickness, 0.5..=8.0).step_by(0.5).suffix("px"));
            });
            // Show/hide toggles
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.viewer.show_colors, egui::RichText::new("色彩").size(10.0));
                ui.checkbox(&mut self.viewer.show_grid, egui::RichText::new("格線").size(10.0));
                ui.checkbox(&mut self.viewer.show_axes, egui::RichText::new("軸向").size(10.0));
            });
        });  // end Styles collapsing header

        // ── SCENES（可折疊）──
        if !self.viewer.saved_cameras.is_empty() {
            egui::CollapsingHeader::new(egui::RichText::new(format!("Scenes ({})", self.viewer.saved_cameras.len())).size(11.0).strong())
                .default_open(false)
                .show(ui, |ui| {
                let mut restore_idx = None;
                let mut delete_idx = None;
                for (i, (name, _cam)) in self.viewer.saved_cameras.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let btn_text = format!("{}  {}", i + 1, name);
                        if ui.button(egui::RichText::new(&btn_text).size(11.0)).clicked() {
                            restore_idx = Some(i);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✕").clicked() {
                                delete_idx = Some(i);
                            }
                        });
                    });
                }
                if let Some(i) = restore_idx {
                    if let Some((_, cam)) = self.viewer.saved_cameras.get(i) {
                        self.viewer.camera = cam.clone();
                    }
                }
                if let Some(i) = delete_idx {
                    self.viewer.saved_cameras.remove(i);
                }
                if ui.button(egui::RichText::new("+ 儲存目前視角").size(10.0)).clicked() {
                    let name = format!("Scene {}", self.viewer.saved_cameras.len() + 1);
                    self.viewer.saved_cameras.push((name, self.viewer.camera.clone()));
                }
            });  // end Scenes collapsing header
        }

        // ── SECTION PLANE（可折疊，預設關閉）──
        egui::CollapsingHeader::new(egui::RichText::new("Section Plane").size(11.0).strong())
            .default_open(false)
            .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.viewer.section_plane_enabled, egui::RichText::new("剖面平面").size(10.0));
            });
            if self.viewer.section_plane_enabled {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("軸向").size(10.0));
                    if ui.selectable_label(self.viewer.section_plane_axis == 0, "X").clicked() { self.viewer.section_plane_axis = 0; }
                    if ui.selectable_label(self.viewer.section_plane_axis == 1, "Y").clicked() { self.viewer.section_plane_axis = 1; }
                    if ui.selectable_label(self.viewer.section_plane_axis == 2, "Z").clicked() { self.viewer.section_plane_axis = 2; }
                    ui.checkbox(&mut self.viewer.section_plane_flip, "翻轉");
                });
                ui.add(egui::Slider::new(&mut self.viewer.section_plane_offset, -20000.0..=20000.0).text("偏移"));
            }
        });  // end Section Plane

        // Groups（可折疊）
        if !self.scene.groups.is_empty() {
            let group_count = self.scene.groups.len();
            egui::CollapsingHeader::new(egui::RichText::new(format!("Groups ({})", group_count)).size(11.0).strong())
                .default_open(false)
                .show(ui, |ui| {
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
            });  // end Groups collapsing header
        }

        // ── COMPONENTS（可折疊）──
        if !self.scene.component_defs.is_empty() {
            let comp_count = self.scene.component_defs.len();
            egui::CollapsingHeader::new(egui::RichText::new(format!("Components ({})", comp_count)).size(11.0).strong())
                .default_open(false)
                .show(ui, |ui| {
                let mut defs: Vec<_> = self.scene.component_defs.values().cloned().collect();
                defs.sort_by(|a, b| a.name.cmp(&b.name));
                for def in &defs {
                    let instance_count = self.scene.objects.values()
                        .filter(|o| o.component_def_id.as_deref() == Some(&def.id))
                        .count();
                    ui.horizontal(|ui| {
                        // 元件圖示
                        let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        ui.painter().rect_filled(icon_rect, 3.0, egui::Color32::from_rgb(76, 139, 245));
                        ui.painter().text(icon_rect.center(), egui::Align2::CENTER_CENTER,
                            "C", egui::FontId::proportional(8.0), egui::Color32::WHITE);
                        // 名稱 + 實例數
                        let name_display = if def.name.len() > 20 { format!("{}…", &def.name[..18]) } else { def.name.clone() };
                        let resp = ui.selectable_label(false,
                            egui::RichText::new(format!("{} ×{}", name_display, instance_count)).size(10.5));
                        if resp.clicked() {
                            // 選取此元件的所有實例
                            self.editor.selected_ids = self.scene.objects.values()
                                .filter(|o| o.component_def_id.as_deref() == Some(&def.id))
                                .map(|o| o.id.clone())
                                .collect();
                        }
                    });
                }
            });  // end Components collapsing header
        }

        if self.scene.objects.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                ui.label(egui::RichText::new("場景為空").color(egui::Color32::GRAY));
            });
            return;
        }
        // Outliner（可折疊）
        egui::CollapsingHeader::new(egui::RichText::new("Outliner").size(11.0).strong())
            .default_open(false)
            .show(ui, |ui| {
                self.render_scene_hierarchy(ui);
            });

        // ── Undo History（可折疊）──
        egui::CollapsingHeader::new(egui::RichText::new(format!("Undo ({})", self.scene.undo_count())).size(11.0).strong())
            .default_open(false)
            .show(ui, |ui| {
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
        });  // end Undo collapsing header

        ui.add_space(4.0);
        if ui.small_button("🧹 清空場景").clicked() { self.scene.clear(); self.editor.selected_ids.clear(); }
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
        // 滑鼠座標已在 viewport 內的 coordinate chips 顯示，不重複
        let coord = String::new();

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
                #[cfg(feature = "steel")]
                Tool::SteelGrid   => "軸線 — 點擊放置軸線".into(),
                #[cfg(feature = "steel")]
                Tool::SteelColumn => format!("柱 — 點擊放置 {} 柱", self.editor.steel_profile),
                #[cfg(feature = "steel")]
                Tool::SteelBeam   => "梁 — 點擊起點，再點擊終點".into(),
                #[cfg(feature = "steel")]
                Tool::SteelBrace  => "斜撐 — 點擊起點，再點擊終點".into(),
                #[cfg(feature = "steel")]
                Tool::SteelPlate  => "鋼板 — 畫矩形，再推拉厚度".into(),
                #[cfg(feature = "steel")]
                Tool::SteelConnection => "接頭 — 選取兩個構件".into(),
                Tool::Walk        => "行走 — WASD 移動, 滑鼠環顧".into(),
                Tool::LookAround  => "環顧 — 滑鼠拖曳環顧視角".into(),
                Tool::SectionPlane => "剖面 — 點擊放置剖面平面".into(),
                Tool::Wall => format!("牆 (W) — 點擊兩點畫牆（厚{:.0}mm 高{:.0}mm）", self.editor.wall_thickness, self.editor.wall_height),
                Tool::Slab => format!("板 — 點擊兩角畫板（厚{:.0}mm）", self.editor.slab_thickness),
                #[cfg(feature = "piping")]
                Tool::PipeDraw | Tool::PipeFitting => self.editor.piping.status_text(),
                #[cfg(feature = "drafting")]
                _ => "出圖工具 — 使用 Ribbon 工具列操作".into(),
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
            DrawState::MoveFrom { .. } => "移動 — 移動滑鼠到目標位置, 點擊確認 (ESC 取消)".into(),
            DrawState::PullClick { face, .. } => {
                let face_name = match face {
                    PullFace::Top => "頂面", PullFace::Bottom => "底面",
                    PullFace::Front => "前面", PullFace::Back => "後面",
                    PullFace::Left => "左面", PullFace::Right => "右面",
                };
                format!("推拉 {} — 移動滑鼠調整距離, 點擊確認 (ESC 取消)", face_name)
            }
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

        // 座標已在 viewport chips 顯示，不重複
        base_text
    }
}

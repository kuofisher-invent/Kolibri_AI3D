use super::material_swatches::*;
use eframe::egui;
use crate::app::{DrawState, KolibriApp, PullFace, RightTab, ScaleHandle, SelectionMode, Tool, WorkMode};
use crate::scene::{MaterialKind, Shape};

/// 簡易 UUID v4 產生器（不依賴外部 crate）
fn uuid_v4_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let seed = t.as_nanos() as u64;
    // xorshift64 pseudo-random
    let mut s = seed ^ 0x6c62272e07bb0142;
    s ^= s << 13; s ^= s >> 7; s ^= s << 17;
    let a = s;
    s ^= s << 13; s ^= s >> 7; s ^= s << 17;
    let b = s;
    format!("{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (a >> 32) as u32,
        (a >> 16) as u16 & 0xFFFF,
        a as u16 & 0x0FFF,
        0x8000 | (b >> 48) as u16 & 0x3FFF,
        b & 0xFFFFFFFFFFFF,
    )
}


impl KolibriApp {
    pub(crate) fn right_panel_ui(&mut self, ui: &mut egui::Ui) {
        let tabs = [
            (RightTab::Create, "設計"),
            (RightTab::Properties, "屬性"),
            (RightTab::Scene, "場景"),
            (RightTab::AiLog, "輸出"),
            (RightTab::Help, "說明"),
        ];

        ui.horizontal(|ui| {
            let tab_frame = egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 204))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)))
                .rounding(egui::Rounding::same(16.0))
                .inner_margin(egui::Margin::same(6.0));

            tab_frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().button_padding = egui::vec2(6.0, 4.0);
                    for (tab, label) in &tabs {
                        let active = self.right_tab == *tab;
                        let btn = if active {
                            egui::Button::new(egui::RichText::new(*label).size(11.0).color(egui::Color32::from_rgb(31, 36, 48)))
                                .fill(egui::Color32::WHITE)
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)))
                                .rounding(10.0)
                        } else {
                            egui::Button::new(egui::RichText::new(*label).size(11.0).color(egui::Color32::from_rgb(110, 118, 135)))
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE)
                                .rounding(10.0)
                        };
                        if ui.add(btn).clicked() {
                            self.right_tab = *tab;
                        }
                    }
                });
            });
        });
        ui.add_space(4.0);

        // Layout mode: show layout properties instead of normal tabs
        if self.viewer.layout_mode {
            egui::ScrollArea::vertical().show(ui, |ui| {
                crate::layout::draw_layout_properties(ui, &mut self.viewer.layout);
            });
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            match self.right_tab {
                RightTab::Properties => {
                    self.tab_properties(ui);
                    self.ai_suggestions_ui(ui);
                }
                RightTab::Create => self.tab_create(ui),
                RightTab::Scene => self.tab_scene(ui),
                RightTab::AiLog => self.tab_ai_log(ui),
                RightTab::Help => self.tab_help(ui),
            }
        });
    }

    pub(crate) fn tab_properties(&mut self, ui: &mut egui::Ui) {
        // ── Selection Summary (always shown) ──
        section_frame_full(ui, |ui| {
            section_header_text(ui, "SELECTION SUMMARY");
            ui.columns(3, |cols| {
                cols[0].vertical(|ui| {
                    ui.label(egui::RichText::new("物件數").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    ui.label(egui::RichText::new(format!("{}", self.scene.objects.len())).size(18.0).strong());
                });
                cols[1].vertical(|ui| {
                    ui.label(egui::RichText::new("群組").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    ui.label(egui::RichText::new(format!("{}", self.scene.groups.len())).size(18.0).strong());
                });
                cols[2].vertical(|ui| {
                    ui.label(egui::RichText::new("選取").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    ui.label(egui::RichText::new(format!("{}", self.editor.selected_ids.len())).size(18.0).strong());
                });
            });
        });

        // ── Selection mode toggle ──
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let modes = [
                (SelectionMode::Object, "物件"),
                (SelectionMode::Face, "面"),
                (SelectionMode::Edge, "邊"),
            ];
            for (mode, label) in &modes {
                let active = self.editor.selection_mode == *mode;
                let btn = egui::Button::new(
                    egui::RichText::new(*label).size(11.0)
                        .color(if active { egui::Color32::WHITE } else { egui::Color32::from_rgb(80, 80, 100) })
                ).fill(if active { egui::Color32::from_rgb(76, 139, 245) } else { egui::Color32::from_rgb(240, 242, 248) })
                 .rounding(8.0)
                 .min_size(egui::vec2(36.0, 22.0));
                if ui.add(btn).clicked() {
                    self.editor.selection_mode = *mode;
                }
            }
        });
        ui.add_space(8.0);

        if self.editor.selected_ids.is_empty() {
            // ── Scene details when nothing selected ──
            section_frame_full(ui, |ui| {
                section_header_text(ui, "SCENE INFO");
                let count = self.scene.objects.len();
                if count > 0 {
                    let mut total_vol = 0.0_f64;
                    let mut total_area = 0.0_f64;
                    let mut box_count = 0u32;
                    let mut cyl_count = 0u32;
                    let mut sph_count = 0u32;
                    let mut line_count = 0u32;
                    for obj in self.scene.objects.values() {
                        total_vol += crate::measure::volume(obj);
                        total_area += crate::measure::surface_area(obj);
                        match &obj.shape {
                            Shape::Box{..} => box_count += 1,
                            Shape::Cylinder{..} => cyl_count += 1,
                            Shape::Sphere{..} => sph_count += 1,
                            Shape::Line{..} => line_count += 1,
                            _ => {}
                        }
                    }
                    if box_count > 0 { ui.small(format!("  ⬜ 方塊: {}", box_count)); }
                    if cyl_count > 0 { ui.small(format!("  ○ 圓柱: {}", cyl_count)); }
                    if sph_count > 0 { ui.small(format!("  ◎ 球體: {}", sph_count)); }
                    if line_count > 0 { ui.small(format!("  ╱ 線段: {}", line_count)); }
                    ui.add_space(4.0);
                    ui.small(format!("總表面積: {}", crate::measure::format_area(total_area)));
                    if total_vol > 0.0 {
                        ui.small(format!("總體積: {}", crate::measure::format_volume(total_vol)));
                        // 重量估算（混凝土密度 2400 kg/m³）
                        let weight_kg = total_vol / 1_000_000_000.0 * 2400.0;
                        if weight_kg >= 1000.0 {
                            ui.small(format!("估重: {:.1} t", weight_kg / 1000.0));
                        } else {
                            ui.small(format!("估重: {:.0} kg", weight_kg));
                        }
                    }
                    let mesh_count = self.scene.objects.values().filter(|o| matches!(o.shape, Shape::Mesh(..))).count();
                    if mesh_count > 0 { ui.small(format!("  ◇ Mesh: {}", mesh_count)); }
                } else {
                    ui.label(egui::RichText::new("場景為空").color(egui::Color32::from_rgb(110, 118, 135)));
                }
            });

            ui.add_space(8.0);

            // ── Quick camera views ──
            section_frame_full(ui, |ui| {
                section_header_text(ui, "CAMERA");
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("前").clicked() { self.viewer.animate_camera_to(|c| c.set_front()); }
                    if ui.small_button("後").clicked() { self.viewer.animate_camera_to(|c| c.set_back()); }
                    if ui.small_button("左").clicked() { self.viewer.animate_camera_to(|c| c.set_left()); }
                    if ui.small_button("右").clicked() { self.viewer.animate_camera_to(|c| c.set_right()); }
                    if ui.small_button("上").clicked() { self.viewer.animate_camera_to(|c| c.set_top()); }
                    if ui.small_button("等角").clicked() { self.viewer.animate_camera_to(|c| c.set_iso()); }
                });
                ui.horizontal(|ui| {
                    if ui.small_button("全部顯示").clicked() { self.zoom_extents(); }
                    let ortho_label = if self.viewer.use_ortho { "透視" } else { "平行" };
                    if ui.small_button(ortho_label).clicked() { self.viewer.use_ortho = !self.viewer.use_ortho; }
                });
            });

            ui.add_space(8.0);

            // ── Render mode ──
            section_frame_full(ui, |ui| {
                section_header_text(ui, "DISPLAY");
                ui.horizontal_wrapped(|ui| {
                    let modes = [
                        (crate::app::RenderMode::Shaded, "著色"),
                        (crate::app::RenderMode::Wireframe, "線框"),
                        (crate::app::RenderMode::XRay, "X光"),
                        (crate::app::RenderMode::HiddenLine, "隱藏線"),
                        (crate::app::RenderMode::Monochrome, "單色"),
                        (crate::app::RenderMode::Sketch, "草稿"),
                    ];
                    for (mode, label) in modes {
                        if ui.selectable_label(self.viewer.render_mode == mode, label).clicked() {
                            self.viewer.render_mode = mode;
                        }
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("線粗");
                    ui.add(egui::Slider::new(&mut self.viewer.edge_thickness, 0.1..=8.0).step_by(0.1));
                });
                ui.checkbox(&mut self.viewer.show_colors, "顯示顏色");
                ui.checkbox(&mut self.viewer.show_grid, "顯示格線");
                ui.checkbox(&mut self.viewer.show_axes, "顯示軸向");
                ui.checkbox(&mut self.viewer.dark_mode, "深色模式");
                ui.checkbox(&mut self.viewer.show_vertex_ids, "頂點編號");
                // ── Section Plane（剖面平面）──
                ui.separator();
                ui.checkbox(&mut self.viewer.section_plane_enabled, "剖面平面");
                if self.viewer.section_plane_enabled {
                    ui.horizontal(|ui| {
                        ui.label("軸");
                        if ui.selectable_label(self.viewer.section_plane_axis == 0, "X").clicked() { self.viewer.section_plane_axis = 0; }
                        if ui.selectable_label(self.viewer.section_plane_axis == 1, "Y").clicked() { self.viewer.section_plane_axis = 1; }
                        if ui.selectable_label(self.viewer.section_plane_axis == 2, "Z").clicked() { self.viewer.section_plane_axis = 2; }
                        if ui.small_button(if self.viewer.section_plane_flip { ">" } else { "<" }).on_hover_text("翻轉剖面方向").clicked() {
                            self.viewer.section_plane_flip = !self.viewer.section_plane_flip;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("偏移");
                        ui.add(egui::DragValue::new(&mut self.viewer.section_plane_offset)
                            .speed(50.0)
                            .suffix(" mm")
                            .range(-50000.0..=50000.0));
                    });
                    ui.add(egui::Slider::new(&mut self.viewer.section_plane_offset, -20000.0..=20000.0).text("mm"));
                }
                ui.horizontal(|ui| {
                    ui.label("語言");
                    if ui.selectable_label(self.viewer.language == 0, "繁中").clicked() { self.viewer.language = 0; }
                    if ui.selectable_label(self.viewer.language == 1, "EN").clicked() { self.viewer.language = 1; }
                });
                // 樓層切換
                ui.horizontal(|ui| {
                    ui.label("樓層");
                    if ui.small_button("▼").clicked() { self.viewer.current_floor -= 1; }
                    let floor_name = match self.viewer.current_floor {
                        f if f < 0 => format!("B{}", -f),
                        0 => "GF".to_string(),
                        f => format!("{}F", f),
                    };
                    ui.label(egui::RichText::new(&floor_name).strong().size(13.0));
                    if ui.small_button("▲").clicked() { self.viewer.current_floor += 1; }
                    ui.add(egui::DragValue::new(&mut self.viewer.floor_height).speed(50.0).prefix("h:").suffix("mm").range(2000.0..=10000.0));
                });
                ui.horizontal(|ui| {
                    ui.label("格線間距");
                    let options = [100.0_f32, 250.0, 500.0, 1000.0, 2000.0, 5000.0];
                    let labels = ["100mm", "250mm", "500mm", "1m", "2m", "5m"];
                    let current = options.iter().position(|v| (*v - self.viewer.grid_spacing).abs() < 1.0).unwrap_or(3);
                    egui::ComboBox::from_id_source("grid_spacing")
                        .selected_text(labels[current])
                        .show_ui(ui, |ui| {
                            for (i, &val) in options.iter().enumerate() {
                                if ui.selectable_label(current == i, labels[i]).clicked() {
                                    self.viewer.grid_spacing = val;
                                }
                            }
                        });
                });
            });
                // 工作平面
                ui.horizontal(|ui| {
                    ui.label("工作平面");
                    let planes = ["地面(XZ)", "正面(XY)", "側面(YZ)"];
                    for (i, label) in planes.iter().enumerate() {
                        let active = self.viewer.work_plane == i as u8;
                        let btn = egui::Button::new(
                            egui::RichText::new(*label).size(10.0)
                                .color(if active { egui::Color32::WHITE } else { egui::Color32::from_rgb(80, 80, 100) })
                        ).fill(if active { egui::Color32::from_rgb(76, 139, 245) } else { egui::Color32::from_rgb(240, 242, 248) })
                         .rounding(6.0);
                        if ui.add(btn).clicked() {
                            self.viewer.work_plane = i as u8;
                        }
                    }
                });
                if self.viewer.work_plane != 0 {
                    ui.horizontal(|ui| {
                        ui.label("偏移");
                        ui.add(egui::DragValue::new(&mut self.viewer.work_plane_offset)
                            .speed(50.0).suffix(" mm"));
                    });
                }

            ui.add_space(8.0);

            // ── Material browser (always accessible, SketchUp-style) ──
            section_frame_full(ui, |ui| {
                section_header_text(ui, "MATERIAL");
                if let Some(new_mat) = material_picker_ui(
                    ui,
                    self.create_mat,
                    &mut self.mat_search,
                    &mut self.mat_category_idx,
                    &mut self.show_custom_color_picker,
                ) {
                    self.create_mat = new_mat;
                }
                if self.show_custom_color_picker {
                    ui.add_space(6.0);
                    figma_group(ui, |ui| {
                        ui.label(egui::RichText::new("自訂材質").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                        let mut color = egui::Color32::from_rgba_unmultiplied(
                            (self.custom_color[0] * 255.0) as u8,
                            (self.custom_color[1] * 255.0) as u8,
                            (self.custom_color[2] * 255.0) as u8,
                            (self.custom_color[3] * 255.0) as u8,
                        );
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            self.custom_color = [
                                color.r() as f32 / 255.0,
                                color.g() as f32 / 255.0,
                                color.b() as f32 / 255.0,
                                color.a() as f32 / 255.0,
                            ];
                        }
                        if ui.button("套用自訂色").clicked() {
                            self.create_mat = crate::scene::MaterialKind::Custom(self.custom_color);
                            self.show_custom_color_picker = false;
                        }
                    });
                }
            });

            ui.add_space(8.0);

            // ── Tips ──
            section_frame_full(ui, |ui| {
                section_header_text(ui, "TIPS");
                ui.small("中鍵拖曳: 旋轉視角");
                ui.small("Shift+中鍵: 平移");
                ui.small("滾輪: 縮放");
                ui.small("B: 建立方塊");
                ui.small("P: 推拉工具");
                ui.small("L: 線段工具");
                ui.small("Ctrl+Z: 復原");
                ui.small("Ctrl+S: 儲存");
            });

            return;
        }
        if self.editor.selected_ids.len() > 1 {
            section_frame_full(ui, |ui| {
                section_header_text(ui, "MULTI-SELECT");
                ui.label(egui::RichText::new(format!("已選取 {} 個物件", self.editor.selected_ids.len())).strong());
                ui.add_space(4.0);
                for sid in &self.editor.selected_ids {
                    if let Some(obj) = self.scene.objects.get(sid) {
                        let icon = match &obj.shape {
                            Shape::Box{..} => "⬜", Shape::Cylinder{..} => "○", Shape::Sphere{..} => "◎", Shape::Line{..} => "╱", Shape::Mesh{..} => "◇",
                        };
                        ui.small(format!("{} {}", icon, obj.name));
                    }
                }
            });
            return;
        }
        let id = self.editor.selected_ids[0].clone();
        let active_component_def_id = self
            .scene
            .objects
            .get(&id)
            .and_then(|obj| {
                obj.component_def_id
                    .clone()
                    .or_else(|| obj.tag.strip_prefix("元件:").map(|s| s.to_string()))
            });
        let active_component_name = active_component_def_id
            .as_ref()
            .and_then(|def_id| self.scene.component_defs.get(def_id))
            .map(|def| def.name.clone());
        let is_editing_component =
            self.editor.editing_component_def_id.as_ref() == active_component_def_id.as_ref();
        let active_component_instance_ids = active_component_def_id
            .as_ref()
            .map(|def_id| self.scene.component_instance_ids(def_id))
            .unwrap_or_default();
        let active_component_visible_count = active_component_def_id
            .as_deref()
            .map(|def_id| self.scene.component_visible_instance_count(def_id))
            .unwrap_or(0);

        if let (Some(def_id), Some(def_name)) = (&active_component_def_id, &active_component_name) {
            section_frame_full(ui, |ui| {
                section_header_text(ui, "COMPONENT EDITING");
                if let Some(summary) = self.current_component_edit_summary() {
                    ui.small(summary);
                }
                ui.label(egui::RichText::new(format!("Definition: {}", def_name)).strong());
                ui.small(format!("Definition ID: {}", def_id));
                ui.small(format!(
                    "Visible instances: {}/{}",
                    active_component_visible_count,
                    active_component_instance_ids.len()
                ));
                ui.horizontal(|ui| {
                    if is_editing_component {
                        if ui.button("Focus").clicked() {
                            self.editor.selected_ids = active_component_instance_ids.clone();
                            self.focus_on_objects(&active_component_instance_ids);
                            self.right_tab = RightTab::Properties;
                        }
                        if ui.button("Select All").clicked() {
                            self.editor.selected_ids = active_component_instance_ids.clone();
                            self.right_tab = RightTab::Properties;
                        }
                        if ui.button("Finish Sync").clicked() {
                            self.finish_component_editing(true);
                        }
                        if ui.button("Exit").clicked() {
                            self.finish_component_editing(false);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Primary").clicked() {
                        if let Some(first_id) = active_component_instance_ids.first() {
                            self.editor.selected_ids = vec![first_id.clone()];
                            if !is_editing_component {
                                self.editor.editing_component_def_id = None;
                            }
                            self.right_tab = RightTab::Properties;
                        }
                    }
                    if ui.button("Show").clicked() {
                        self.scene.set_component_instances_visible(def_id, true);
                    }
                    if ui.button("Hide").clicked() {
                        self.scene.set_component_instances_visible(def_id, false);
                    }
                    if !is_editing_component && ui.button("Edit Component").clicked() {
                        if let Some(first_id) = active_component_instance_ids.first() {
                            self.editor.selected_ids = vec![first_id.clone()];
                        }
                        self.editor.editing_component_def_id = Some(def_id.clone());
                        self.right_tab = RightTab::Properties;
                        self.file_message = Some((
                            format!("Entering component editing: {}", def_name),
                            std::time::Instant::now(),
                        ));
                    }
                });
            });
            ui.add_space(8.0);
        }

        let obj = match self.scene.objects.get_mut(&id) {
            Some(o) => o,
            None => { self.editor.selected_ids.clear(); return; }
        };

        // Object header
        section_frame_full(ui, |ui| {
            ui.horizontal(|ui| {
                let icon = match &obj.shape {
                    Shape::Box{..} => "⬜", Shape::Cylinder{..} => "○", Shape::Sphere{..} => "◎", Shape::Line{..} => "╱", Shape::Mesh{..} => "◇",
                };
                ui.label(egui::RichText::new(icon).size(16.0));
                ui.text_edit_singleline(&mut obj.name);
            });
            ui.small(format!("ID: {}", obj.id));
        });
        ui.add_space(8.0);

        // Dimensions
        section_frame_full(ui, |ui| {
            section_header_text(ui, "DIMENSIONS");
            match &mut obj.shape {
                Shape::Box { width, height, depth } => {
                    ui.add(egui::DragValue::new(width).speed(10.0).prefix("寬 W: ").suffix(" mm").range(1.0..=f32::MAX));
                    ui.add(egui::DragValue::new(height).speed(10.0).prefix("高 H: ").suffix(" mm").range(1.0..=f32::MAX));
                    ui.add(egui::DragValue::new(depth).speed(10.0).prefix("深 D: ").suffix(" mm").range(1.0..=f32::MAX));
                }
                Shape::Cylinder { radius, height, segments } => {
                    ui.add(egui::DragValue::new(radius).speed(10.0).prefix("R: ").suffix(" mm").range(1.0..=f32::MAX));
                    ui.add(egui::DragValue::new(height).speed(10.0).prefix("H: ").suffix(" mm").range(1.0..=f32::MAX));
                    ui.add(egui::DragValue::new(segments).speed(1.0).prefix("細分: ").range(4..=128));
                }
                Shape::Sphere { radius, segments } => {
                    ui.add(egui::DragValue::new(radius).speed(10.0).prefix("R: ").suffix(" mm").range(1.0..=f32::MAX));
                    ui.add(egui::DragValue::new(segments).speed(1.0).prefix("細分: ").range(4..=128));
                }
                Shape::Line { points, thickness, .. } => {
                    ui.label(format!("線段 ({} 點)", points.len()));
                    ui.add(egui::DragValue::new(thickness).speed(1.0).prefix("粗細: ").suffix(" mm").range(1.0..=500.0));
                }
                Shape::Mesh(ref mesh) => {
                    ui.label(format!("網格: {} 頂點, {} 邊, {} 面",
                        mesh.vertices.len(), mesh.edge_count(), mesh.faces.len()));
                }
            }
        });
        ui.add_space(8.0);

        // Transform (Position + Rotation)
        section_frame_full(ui, |ui| {
            section_header_text(ui, "TRANSFORM");

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Position").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("mm").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                });
            });
            ui.add(egui::DragValue::new(&mut obj.position[0]).speed(10.0).prefix("X: ").suffix(" mm"));
            ui.add(egui::DragValue::new(&mut obj.position[1]).speed(10.0).prefix("Y: ").suffix(" mm"));
            ui.add(egui::DragValue::new(&mut obj.position[2]).speed(10.0).prefix("Z: ").suffix(" mm"));

            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Rotation").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("deg").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                });
            });
            let mut deg = obj.rotation_y.to_degrees();
            if ui.add(egui::DragValue::new(&mut deg).speed(1.0).prefix("Y軸: ").suffix("°").range(-360.0..=360.0)).changed() {
                obj.rotation_y = deg.to_radians();
            }
        });
        ui.add_space(8.0);

        // Lock toggle
        ui.horizontal(|ui| {
            let lock_label = if obj.locked { "🔒 已鎖定" } else { "🔓 未鎖定" };
            if ui.toggle_value(&mut obj.locked, lock_label).changed() {
                self.scene.version += 1;
            }
        });
        ui.add_space(4.0);

        // Component Kind (collision)
        section_frame_full(ui, |ui| {
            section_header_text(ui, "COMPONENT");
            ui.horizontal(|ui| {
                ui.label("元件類型:");
                let kind_name = match obj.component_kind {
                    crate::collision::ComponentKind::Column => "柱",
                    crate::collision::ComponentKind::Beam => "梁",
                    crate::collision::ComponentKind::Plate => "板",
                    crate::collision::ComponentKind::Bolt => "螺栓",
                    crate::collision::ComponentKind::Weld => "焊接",
                    crate::collision::ComponentKind::Foundation => "基礎",
                    crate::collision::ComponentKind::Equipment => "設備",
                    crate::collision::ComponentKind::Generic => "一般",
                };
                ui.label(kind_name);
            });
        });
        ui.add_space(8.0);

        // Steel properties (when component is Beam/Column/Plate)
        if !matches!(obj.component_kind, crate::collision::ComponentKind::Generic) {
            section_frame_full(ui, |ui| {
                section_header_text(ui, "STEEL PROPERTIES");
                ui.horizontal(|ui| {
                    ui.label("Profile:");
                    ui.label(egui::RichText::new(&self.editor.steel_profile).strong());
                });
                ui.horizontal(|ui| {
                    ui.label("Material:");
                    ui.label(egui::RichText::new(&self.editor.steel_material).strong());
                });
                let dims = match &obj.shape {
                    Shape::Box { width, height, depth } => format!("{:.0}x{:.0}x{:.0} mm", width, height, depth),
                    _ => String::new(),
                };
                if !dims.is_empty() {
                    ui.label(format!("尺寸: {}", dims));
                }
            });
            ui.add_space(8.0);
        }

        // Layer / Tag
        section_frame_full(ui, |ui| {
            section_header_text(ui, "LAYER");
            ui.horizontal(|ui| {
                ui.label("標籤:");
                ui.text_edit_singleline(&mut obj.tag);
            });
        });
        ui.add_space(8.0);

        // ── IFC / BIM 屬性 ──
        section_frame_full(ui, |ui| {
            section_header_text(ui, "BIM / IFC");

            let ifc_classes = [
                "", "IfcWall", "IfcColumn", "IfcBeam", "IfcSlab",
                "IfcWindow", "IfcDoor", "IfcRoof", "IfcStair",
                "IfcPlate", "IfcMember", "IfcFooting", "IfcPile",
                "IfcCurtainWall", "IfcRailing", "IfcFurnishingElement",
                "IfcBuildingElementProxy",
            ];
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("IFC 類型").size(11.0));
                egui::ComboBox::from_id_source("ifc_class_combo")
                    .width(120.0)
                    .selected_text(if obj.ifc_class.is_empty() { "(未指定)" } else { &obj.ifc_class })
                    .show_ui(ui, |ui| {
                        for cls in &ifc_classes {
                            let label = if cls.is_empty() { "(未指定)" } else { cls };
                            if ui.selectable_label(obj.ifc_class == *cls, label).clicked() {
                                obj.ifc_class = cls.to_string();
                            }
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("樓  層").size(11.0));
                ui.text_edit_singleline(&mut obj.ifc_storey);
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("系  統").size(11.0));
                ui.text_edit_singleline(&mut obj.ifc_system);
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("BIM材料").size(11.0));
                ui.text_edit_singleline(&mut obj.ifc_material_name);
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("防火等級").size(11.0));
                ui.text_edit_singleline(&mut obj.ifc_fire_rating);
            });

            // GlobalId
            if obj.ifc_global_id.is_empty() && !obj.ifc_class.is_empty() {
                obj.ifc_global_id = uuid_v4_simple();
            }
            if !obj.ifc_global_id.is_empty() {
                ui.small(format!("GUID: {}", &obj.ifc_global_id[..obj.ifc_global_id.len().min(22)]));
            }

            // 自訂屬性集
            if !obj.ifc_properties.is_empty() {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("自訂屬性").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                let props: Vec<_> = obj.ifc_properties.iter().map(|(k,v)| (k.clone(), v.clone())).collect();
                for (key, val) in &props {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(key).size(11.0));
                        ui.label(egui::RichText::new(val).size(11.0).strong());
                    });
                }
            }
        });
        ui.add_space(8.0);

        // Material (SketchUp-style browser)
        // We need to work around the borrow of `obj` from `self.scene.objects`
        // by using local copies of the search/category state.
        let mut mat_search_local = std::mem::take(&mut self.mat_search);
        let mut mat_cat_local = self.mat_category_idx;
        let mut show_custom_local = self.show_custom_color_picker;
        section_frame_full(ui, |ui| {
            section_header_text(ui, "MATERIAL");

            if let Some(new_mat) = material_picker_ui(
                ui,
                obj.material,
                &mut mat_search_local,
                &mut mat_cat_local,
                &mut show_custom_local,
            ) {
                obj.material = new_mat;
                self.scene.version += 1;
            }

            ui.add_space(6.0);

            // PBR sliders — 統一寬度對齊
            let slider_w = ui.available_width();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("粗糙度").size(11.0));
                ui.add_sized([slider_w - 50.0, 18.0], egui::Slider::new(&mut obj.roughness, 0.0..=1.0).show_value(true));
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("金屬感").size(11.0));
                ui.add_sized([slider_w - 50.0, 18.0], egui::Slider::new(&mut obj.metallic, 0.0..=1.0).show_value(true));
            });

            // Custom colour picker
            ui.add_space(4.0);
            egui::CollapsingHeader::new("自訂顏色").show(ui, |ui| {
                let rgba = obj.material.color();
                let mut c = egui::Color32::from_rgba_unmultiplied(
                    (rgba[0]*255.0) as u8, (rgba[1]*255.0) as u8,
                    (rgba[2]*255.0) as u8, (rgba[3]*255.0) as u8);
                if ui.color_edit_button_srgba(&mut c).changed() {
                    obj.material = MaterialKind::Custom([
                        c.r() as f32/255.0, c.g() as f32/255.0,
                        c.b() as f32/255.0, c.a() as f32/255.0]);
                    self.scene.version += 1;
                }

                ui.add_space(4.0);
                ui.label("常用色");
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    let paint_colors: &[(u32, &str)] = &[
                        (0xE74C3C, "紅"), (0xE67E22, "橙"), (0xF1C40F, "黃"),
                        (0x2ECC71, "綠"), (0x3498DB, "藍"), (0x9B59B6, "紫"),
                        (0xECF0F1, "白"), (0x95A5A6, "灰"), (0x2C3E50, "深灰"),
                        (0x1ABC9C, "青"), (0xD35400, "棕"), (0x7F8C8D, "石灰"),
                    ];
                    let sw = 32.0;
                    for &(hex, label) in paint_colors {
                        let r = ((hex >> 16) & 0xFF) as u8;
                        let g = ((hex >> 8) & 0xFF) as u8;
                        let b = (hex & 0xFF) as u8;
                        let color = egui::Color32::from_rgb(r, g, b);
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(sw, sw), egui::Sense::click());
                        let is_sel = obj.material == MaterialKind::Paint(hex);
                        ui.painter().rect_filled(rect, 10.0, color);
                        if is_sel {
                            ui.painter().rect_stroke(rect, 10.0,
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(76, 139, 245)));
                        } else {
                            ui.painter().rect_stroke(rect, 10.0,
                                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 20)));
                        }
                        if resp.clicked() {
                            obj.material = MaterialKind::Paint(hex);
                            self.scene.version += 1;
                        }
                        resp.on_hover_text(label);
                    }
                });
            });
        });
        self.mat_search = mat_search_local;
        self.mat_category_idx = mat_cat_local;
        self.show_custom_color_picker = show_custom_local;
        ui.add_space(8.0);

        // Texture mapping
        section_frame_full(ui, |ui| {
            section_header_text(ui, "TEXTURE");

            if let Some(ref path) = obj.texture_path {
                let filename = path.rsplit(['\\', '/']).next().unwrap_or(path);
                ui.label(format!("  {}", filename));
                if let Some((w, h)) = self.texture_manager.info(path) {
                    ui.small(format!("{}x{} px", w, h));
                }
                if ui.button("移除紋理").clicked() {
                    obj.texture_path = None;
                    self.scene.version += 1;
                }
            } else {
                ui.label(egui::RichText::new("無紋理").color(egui::Color32::from_rgb(110, 118, 135)));
            }

            if ui.button("載入紋理圖片...").clicked() {
                let file = rfd::FileDialog::new()
                    .set_title("載入紋理")
                    .add_filter("圖片", &["png", "jpg", "jpeg", "bmp"])
                    .pick_file();
                if let Some(path) = file {
                    let ps = path.to_string_lossy().to_string();
                    match self.texture_manager.load(&ps) {
                        Ok(_) => {
                            obj.texture_path = Some(ps);
                            self.scene.version += 1;
                            self.file_message = Some(("紋理已載入".into(), std::time::Instant::now()));
                        }
                        Err(e) => {
                            self.file_message = Some((e, std::time::Instant::now()));
                        }
                    }
                }
            }
        });
        ui.add_space(8.0);

        // Measurements
        section_frame_full(ui, |ui| {
            section_header_text(ui, "MEASURE");
            let area = crate::measure::surface_area(obj);
            let vol = crate::measure::volume(obj);
            ui.label(format!("表面積: {}", crate::measure::format_area(area)));
            if vol > 0.0 {
                ui.label(format!("體積: {}", crate::measure::format_volume(vol)));
            }
            // Weight estimate based on material density (kg/m³)
            if vol > 0.0 {
                let density = match &obj.material {
                    MaterialKind::Concrete | MaterialKind::ConcreteSmooth => 2400.0,
                    MaterialKind::Stone => 2600.0,
                    MaterialKind::Marble => 2700.0,
                    MaterialKind::Granite => 2750.0,
                    MaterialKind::Wood | MaterialKind::Bamboo | MaterialKind::Plywood => 600.0,
                    MaterialKind::WoodLight => 450.0,
                    MaterialKind::WoodDark => 750.0,
                    MaterialKind::Metal | MaterialKind::Steel => 7800.0,
                    MaterialKind::Aluminum => 2700.0,
                    MaterialKind::Copper => 8960.0,
                    MaterialKind::Gold => 19300.0,
                    MaterialKind::Brick | MaterialKind::BrickWhite => 1800.0,
                    MaterialKind::Tile | MaterialKind::TileDark => 2300.0,
                    MaterialKind::Glass | MaterialKind::GlassTinted | MaterialKind::GlassFrosted => 2500.0,
                    MaterialKind::Asphalt => 2300.0,
                    MaterialKind::Gravel => 1800.0,
                    MaterialKind::Grass => 1200.0,
                    MaterialKind::Soil => 1500.0,
                    MaterialKind::Plaster => 1700.0,
                    _ => 1000.0,
                };
                let weight_kg = vol / 1_000_000_000.0 * density;
                if weight_kg >= 1000.0 {
                    ui.label(format!("估重: {:.2} t", weight_kg / 1000.0));
                } else {
                    ui.label(format!("估重: {:.1} kg", weight_kg));
                }
            }
        });

        // Component instance sync: if this object is a component instance,
        // update the definition and propagate changes to all other instances.
        let comp_tag = obj.tag.clone();
        if comp_tag.starts_with("元件:") {
            let def_id = comp_tag.strip_prefix("元件:").unwrap_or("").to_string();
            let shape_clone = obj.shape.clone();
            let mat_clone = obj.material.clone();
            // Update the definition with the edited values
            if let Some(def) = self.scene.component_defs.get_mut(&def_id) {
                if let Some(def_obj) = def.objects.first_mut() {
                    def_obj.shape = shape_clone;
                    def_obj.material = mat_clone;
                }
            }
            // Sync all instances
            self.scene.sync_component_instances(&def_id);
        }
    }

    /// AI contextual suggestions panel
    fn ai_suggestions_ui(&mut self, ui: &mut egui::Ui) {
        let suggestions = crate::ai_assist::generate_suggestions(
            &self.scene,
            self.editor.tool,
            &self.editor.selected_ids,
            &self.editor.last_action_name,
        );

        if suggestions.is_empty() {
            return;
        }

        ui.add_space(8.0);
        section_frame_full(ui, |ui| {
            section_header_text(ui, "AI 建議");
            for sug in &suggestions {
                ui.horizontal(|ui| {
                    ui.label(sug.icon);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&sug.text).strong().size(12.0));
                        ui.label(
                            egui::RichText::new(&sug.detail)
                                .size(11.0)
                                .color(egui::Color32::from_rgb(110, 118, 135)),
                        );
                    });
                });

                if let Some(action) = &sug.action {
                    match action {
                        crate::ai_assist::SuggestedAction::SwitchTool(tool) => {
                            let t = *tool;
                            if ui.small_button("套用").clicked() {
                                self.editor.tool = t;
                            }
                        }
                        crate::ai_assist::SuggestedAction::SetDimension {
                            obj_id,
                            axis,
                            value,
                        } => {
                            let oid = obj_id.clone();
                            let ax = *axis;
                            let val = *value;
                            if ui.small_button("對齊").clicked() {
                                self.scene.snapshot();
                                if let Some(obj) = self.scene.objects.get_mut(&oid) {
                                    obj.position[ax as usize] = val;
                                }
                            }
                        }
                        crate::ai_assist::SuggestedAction::ApplyMaterial {
                            obj_id,
                            material: _,
                        } => {
                            let oid = obj_id.clone();
                            if ui.small_button("套用").clicked() {
                                if self.scene.objects.contains_key(&oid) {
                                    self.scene.snapshot();
                                    if let Some(obj) = self.scene.objects.get_mut(&oid) {
                                        obj.material = crate::scene::MaterialKind::Brick;
                                    }
                                }
                            }
                        }
                    }
                }

                ui.add_space(4.0);
            }
        });
    }
}


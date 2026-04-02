use super::material_swatches::*;
use eframe::egui;
use crate::app::{DrawState, KolibriApp, PullFace, RightTab, ScaleHandle, SelectionMode, Tool, WorkMode};
use crate::scene::{MaterialKind, Shape};


impl KolibriApp {
    pub(crate) fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        // SketchUp-style compact buttons（32x32 vs 原本 48x48）
        let bsz = egui::vec2(36.0, 36.0);

        // ── Mode switch: 下拉選單 ──
        {
            let current_label = if self.viewer.layout_mode {
                "CAD"
            } else {
                match self.editor.work_mode {
                    WorkMode::Modeling => "建模",
                    #[cfg(feature = "steel")]
                    WorkMode::Steel => "鋼構",
                    #[cfg(feature = "piping")]
                    WorkMode::Piping => "管線",
                }
            };
            let brand = egui::Color32::from_rgb(76, 139, 245);
            egui::ComboBox::from_id_source("work_mode_combo")
                .width(ui.available_width() - 8.0)
                .selected_text(egui::RichText::new(format!("模式: {}", current_label)).size(11.0).strong().color(brand))
                .show_ui(ui, |ui| {
                    if ui.selectable_label(!self.viewer.layout_mode && self.editor.work_mode == WorkMode::Modeling, "建模").clicked() {
                        self.exit_layout_mode();
                        self.editor.work_mode = WorkMode::Modeling;
                    }
                    #[cfg(feature = "steel")]
                    if ui.selectable_label(!self.viewer.layout_mode && self.editor.work_mode == WorkMode::Steel, "鋼構").clicked() {
                        self.exit_layout_mode();
                        self.editor.work_mode = WorkMode::Steel;
                    }
                    #[cfg(feature = "piping")]
                    if ui.selectable_label(!self.viewer.layout_mode && self.editor.work_mode == WorkMode::Piping, "管線").clicked() {
                        self.exit_layout_mode();
                        self.editor.work_mode = WorkMode::Piping;
                    }
                    if ui.selectable_label(self.viewer.layout_mode, "CAD").clicked() {
                        self.enter_layout_mode();
                    }
                });
        }

        ui.add_space(2.0);

        // When in layout mode, don't show 3D tools
        if self.viewer.layout_mode {
            ui.separator();
            ui.label(egui::RichText::new("出圖模式").size(11.0).color(egui::Color32::from_gray(130)));
            ui.label(egui::RichText::new("右側面板可編輯\n紙張與圖框設定").size(10.0).color(egui::Color32::from_gray(160)));
            return;
        }

        // Work mode derived flags
        let _modeling_active = self.editor.work_mode == WorkMode::Modeling;

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
                self.tool_row(ui, bsz, &[
                    (Tool::Walk,        "行走\n第一人稱 WASD"),
                    (Tool::LookAround,  "環顧\n自由環視"),
                    (Tool::SectionPlane,"剖面\n放置剖面平面"),
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
            #[cfg(feature = "steel")]
            WorkMode::Steel => {
                // ── 通用工具（最上方，與鋼構元件一起）──
                section_header(ui, "通用 + 建模");
                self.tool_row(ui, bsz, &[
                    (Tool::Select, "選取 (Space)\n點擊選取構件"),
                    (Tool::Move, "移動 (M)\nSU-style 兩點移動"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::Eraser, "刪除 (E)\n點擊刪除構件"),
                    (Tool::TapeMeasure, "量測 (T)\n量測距離"),
                ]);
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
                    (Tool::Rotate, "旋轉 (R)\n旋轉構件"),
                ]);

                ui.separator();

                // ── 接頭工具（全部可點選）──
                section_header(ui, "接頭（AISC 360-22）");
                self.tool_row(ui, bsz, &[
                    (Tool::SteelEndPlate, "端板 (剛接)\n梁-柱端板+螺栓+肋板\nAISC Part 10 FR"),
                    (Tool::SteelShearTab, "腹板 (鉸接)\n梁-柱剪力板+螺栓\nAISC Part 10 PR"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::SteelBasePlate, "底板\n柱底板+錨栓\nAISC DG1"),
                    (Tool::SteelStiffener, "肋板\n加勁板 J10.1~J10.5"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::SteelBolt, "螺栓\n手動放置螺栓\n含孔位+孔徑"),
                    (Tool::SteelWeld, "焊接\n兩點標記焊接線\n角焊/全滲透"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::SteelConnection, "智慧接頭\n選兩構件→AISC自動建議\n含螺栓配置+板件+焊接"),
                    (Tool::Scale, "縮放 (S)\n縮放構件"),
                ]);

                // ── AISC 建議按鈕 ──
                figma_group(ui, |ui| {
                    if ui.button("AISC 接頭建議").on_hover_text("選取兩構件後，根據 AISC 360-22 自動建議最佳接頭形式").clicked() {
                        self.show_aisc_suggestion();
                    }
                });

                ui.separator();

                // ── 截面參數 ──
                section_header(ui, "截面");
                figma_group(ui, |ui| {
                    ui.label(egui::RichText::new("斷面:").size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    egui::ComboBox::from_id_source("steel_profile_combo")
                        .width(ui.available_width() - 4.0)
                        .selected_text(&self.editor.steel_profile)
                        .show_ui(ui, |ui| {
                            for &(name, _h, _b, _tw, _tf, weight) in crate::tools::geometry_ops::H_PROFILES {
                                let label = format!("{} ({:.0}kg/m)", name, weight);
                                if ui.selectable_label(
                                    self.editor.steel_profile == name,
                                    &label,
                                ).clicked() {
                                    self.editor.steel_profile = name.to_string();
                                }
                            }
                        });
                    let (h, b, tw, tf) = crate::tools::geometry_ops::parse_h_profile(&self.editor.steel_profile);
                    ui.label(egui::RichText::new(format!("H{:.0}×B{:.0} tw{:.1} tf{:.1}", h, b, tw, tf))
                        .size(9.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    ui.label(egui::RichText::new("材質:").size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    egui::ComboBox::from_id_source("steel_material_combo")
                        .width(ui.available_width() - 4.0)
                        .selected_text(&self.editor.steel_material)
                        .show_ui(ui, |ui| {
                            for mat in &["SN400B", "SN490B", "SS400", "A572 Gr.50", "SM490A", "SM520B"] {
                                if ui.selectable_label(self.editor.steel_material == *mat, *mat).clicked() {
                                    self.editor.steel_material = mat.to_string();
                                }
                            }
                        });
                    ui.add(egui::DragValue::new(&mut self.editor.steel_height)
                        .speed(10.0).prefix("柱高: ").suffix(" mm").range(100.0..=50000.0));
                });

                // ── 樓層標高 ──
                section_header(ui, "樓層標高");
                figma_group(ui, |ui| {
                    let sub = egui::Color32::from_rgb(110, 118, 135);
                    let brand = egui::Color32::from_rgb(76, 139, 245);
                    let mut level_changed = false;

                    // 從上到下顯示樓層（RF → 1FL → GL）
                    let levels_copy = self.editor.floor_levels.clone();
                    for i in (0..levels_copy.len()).rev() {
                        let (ref name, elev) = levels_copy[i];
                        let is_active = i == self.editor.active_floor;

                        ui.horizontal(|ui| {
                            // 作業樓層指示
                            let indicator = if is_active { "▶" } else { "  " };
                            if ui.selectable_label(is_active,
                                egui::RichText::new(indicator).size(10.0).color(brand)
                            ).clicked() {
                                self.editor.active_floor = i;
                                if i + 1 < self.editor.floor_levels.len() {
                                    self.editor.steel_height = self.editor.floor_levels[i + 1].1 - self.editor.floor_levels[i].1;
                                }
                            }

                            // 樓層名稱
                            ui.label(egui::RichText::new(name).size(10.0).color(
                                if is_active { brand } else { sub }
                            ));

                            // 標高可編輯
                            let mut elev_val = self.editor.floor_levels[i].1;
                            let resp = ui.add(egui::DragValue::new(&mut elev_val)
                                .speed(50.0).suffix(" mm").range(if i == 0 { 0.0 } else { 0.0 }..=100000.0));
                            if resp.changed() {
                                self.editor.floor_levels[i].1 = elev_val;
                                level_changed = true;
                            }
                        });
                    }

                    // 加/刪樓層
                    ui.horizontal(|ui| {
                        if ui.small_button("+ 加樓層").clicked() {
                            let last_elev = self.editor.floor_levels.last().map_or(0.0, |f| f.1);
                            let new_elev = last_elev + self.editor.steel_height;
                            let n = self.editor.floor_levels.len();
                            let new_name = if n <= 1 { "1FL".into() }
                                else if n == 2 { "RF".into() }
                                else { format!("{}FL", n - 1) };
                            self.editor.floor_levels.push((new_name, new_elev));
                            level_changed = true;
                        }
                        if self.editor.floor_levels.len() > 1 {
                            if ui.small_button("- 刪頂層").clicked() {
                                self.editor.floor_levels.pop();
                                if self.editor.active_floor >= self.editor.floor_levels.len() {
                                    self.editor.active_floor = self.editor.floor_levels.len() - 1;
                                }
                                level_changed = true;
                            }
                        }
                    });

                    // 樓層變更 → 自動更新構件
                    if level_changed {
                        // 更新柱高
                        let af = self.editor.active_floor;
                        if af + 1 < self.editor.floor_levels.len() {
                            self.editor.steel_height = self.editor.floor_levels[af + 1].1 - self.editor.floor_levels[af].1;
                        }
                        self.update_levels();
                    }
                });

                // ── 接頭參數 ──
                section_header(ui, "螺栓/焊接");
                figma_group(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("螺栓:").size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
                        egui::ComboBox::from_id_source("conn_bolt_size")
                            .width(60.0)
                            .selected_text(self.editor.conn_bolt_size.label())
                            .show_ui(ui, |ui| {
                                for &bs in kolibri_core::steel_connection::BoltSize::ALL {
                                    if ui.selectable_label(
                                        self.editor.conn_bolt_size == bs,
                                        format!("{} Ø{:.0} 孔Ø{:.0}", bs.label(), bs.diameter(), bs.hole_diameter()),
                                    ).clicked() {
                                        self.editor.conn_bolt_size = bs;
                                    }
                                }
                            });
                        egui::ComboBox::from_id_source("conn_bolt_grade")
                            .width(55.0)
                            .selected_text(self.editor.conn_bolt_grade.label())
                            .show_ui(ui, |ui| {
                                for &bg in kolibri_core::steel_connection::BoltGrade::ALL {
                                    if ui.selectable_label(self.editor.conn_bolt_grade == bg, bg.label()).clicked() {
                                        self.editor.conn_bolt_grade = bg;
                                    }
                                }
                            });
                    });
                    // 顯示螺栓孔徑和邊距（即時計算）
                    let bs = self.editor.conn_bolt_size;
                    ui.label(egui::RichText::new(format!(
                        "孔Ø{:.0}mm 邊距≥{:.0}mm 間距≥{:.0}mm",
                        bs.hole_diameter(), bs.min_edge(), bs.min_spacing()
                    )).size(8.5).color(egui::Color32::from_rgb(130, 140, 155)));
                    ui.add(egui::DragValue::new(&mut self.editor.conn_weld_size)
                        .speed(0.5).prefix("焊腳: ").suffix(" mm").range(4.0..=20.0));
                    ui.checkbox(&mut self.editor.conn_add_stiffeners, "加勁板 (AISC J10)");
                });

                // ── 輸出功能 ──
                ui.separator();
                section_header(ui, "輸出");
                figma_group(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("料表").on_hover_text("CSV: 材料+螺栓+焊接+組裝件").clicked() {
                            self.export_steel_report();
                        }
                        if ui.button("施工圖").on_hover_text("DXF: GA圖+單件圖").clicked() {
                            self.export_steel_drawings();
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("NC").on_hover_text("DSTV NC1 (CNC)").clicked() {
                            self.export_nc_files();
                        }
                        if ui.button("IFC").on_hover_text("IFC 2x3 (BIM)").clicked() {
                            self.export_ifc_file();
                        }
                        if ui.button("編號").on_hover_text("自動編號 C1/B1").clicked() {
                            self.run_auto_numbering();
                        }
                    });
                    if ui.button("碰撞偵測").on_hover_text("AABB + 螺栓邊距檢查").clicked() {
                        self.run_collision_check();
                    }
                });

                // ── 統計 ──
                {
                    let (cols, beams, braces, plates) = self.count_steel_members();
                    if cols + beams + braces + plates > 0 {
                        ui.separator();
                        ui.label(egui::RichText::new(format!(
                            "柱:{} 梁:{} 撐:{} 板:{}", cols, beams, braces, plates
                        )).size(9.5).color(egui::Color32::from_rgb(110, 118, 135)));
                    }
                }
            }
            #[cfg(feature = "piping")]
            WorkMode::Piping => {
                section_header(ui, "管線工具");
                self.tool_row(ui, bsz, &[
                    (Tool::PipeDraw, "畫管\n連續點擊繪製管線"),
                    (Tool::PipeFitting, "管件\n放置彎頭/三通/閥門"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::Select, "選取\n點擊選取管段"),
                    (Tool::TapeMeasure, "量測\n量測距離/角度"),
                ]);

                ui.separator();
                section_header(ui, "管線參數");
                // 管線系統選擇
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("系統:").size(11.0));
                    egui::ComboBox::from_id_source("pipe_system")
                        .width(ui.available_width() - 4.0)
                        .selected_text(self.editor.piping.current_system.label())
                        .show_ui(ui, |ui| {
                            for &sys in kolibri_piping::PipeSystem::all() {
                                if ui.selectable_label(
                                    self.editor.piping.current_system == sys,
                                    sys.label(),
                                ).clicked() {
                                    self.editor.piping.current_system = sys;
                                    self.editor.piping.current_spec_idx = 2;
                                }
                            }
                        });
                });
                // 管徑選擇
                let specs = kolibri_piping::PipeCatalog::specs_for(self.editor.piping.current_system);
                let cur_spec = specs.get(self.editor.piping.current_spec_idx);
                let cur_name = cur_spec.map(|s| s.spec_name.as_str()).unwrap_or("DN25");
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("管徑:").size(11.0));
                    egui::ComboBox::from_id_source("pipe_dn")
                        .width(ui.available_width() - 4.0)
                        .selected_text(cur_name)
                        .show_ui(ui, |ui| {
                            for (i, spec) in specs.iter().enumerate() {
                                let label = format!("{} (Ø{:.1})", spec.spec_name, spec.outer_diameter);
                                if ui.selectable_label(
                                    self.editor.piping.current_spec_idx == i,
                                    &label,
                                ).clicked() {
                                    self.editor.piping.current_spec_idx = i;
                                }
                            }
                        });
                });
                // 管徑資訊
                if let Some(spec) = cur_spec {
                    ui.label(egui::RichText::new(
                        format!("Ø{:.1} × {:.1}mm", spec.outer_diameter, spec.wall_thickness)
                    ).size(10.0).color(egui::Color32::GRAY));
                }
                // 繪製高度
                ui.add(egui::DragValue::new(&mut self.editor.piping.draw_height)
                    .speed(10.0).prefix("高度: ").suffix(" mm").range(0.0..=20000.0));

                ui.separator();
                // 管件種類（常駐顯示，不只在 PipeFitting 模式）
                section_header(ui, "管件類型");
                let fittings = [
                    (kolibri_piping::FittingKind::Elbow90, "90° 彎頭"),
                    (kolibri_piping::FittingKind::Elbow45, "45° 彎頭"),
                    (kolibri_piping::FittingKind::Tee, "三通"),
                    (kolibri_piping::FittingKind::Cross, "四通"),
                    (kolibri_piping::FittingKind::Reducer, "大小頭"),
                    (kolibri_piping::FittingKind::Valve, "閘閥"),
                    (kolibri_piping::FittingKind::Flange, "法蘭"),
                    (kolibri_piping::FittingKind::Cap, "管帽"),
                    (kolibri_piping::FittingKind::Coupling, "接頭"),
                ];
                for (kind, label) in &fittings {
                    let selected = self.editor.piping.current_fitting == *kind;
                    if ui.selectable_label(selected, egui::RichText::new(*label).size(11.0)).clicked() {
                        self.editor.piping.current_fitting = *kind;
                        self.editor.tool = Tool::PipeFitting;
                    }
                }

                // 管線統計
                let total_len = self.editor.piping.store.total_length(None);
                if total_len > 0.0 {
                    ui.separator();
                    section_header(ui, "統計");
                    figma_group(ui, |ui| {
                        ui.small(format!("管段: {} 段", self.editor.piping.store.segments.len()));
                        ui.small(format!("管件: {} 個", self.editor.piping.store.fittings.len()));
                        if total_len >= 1000.0 {
                            ui.small(format!("總長: {:.1} m", total_len / 1000.0));
                        } else {
                            ui.small(format!("總長: {:.0} mm", total_len));
                        }
                    });
                }

                // 通用工具
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


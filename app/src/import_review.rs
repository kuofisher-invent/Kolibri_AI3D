//! Import Review Panel — user confirmation UI for semantic detection results
//! Shows detected grids, columns, beams with confidence scores
//! Allows editing, adding, removing before 3D building

use eframe::egui;
use crate::cad_import::ir::*;

/// Review state for a pending import
#[derive(Debug, Clone)]
pub struct ImportReview {
    pub active: bool,
    pub source_file: String,
    pub entity_count: usize,

    // Editable detected items
    pub grids_x: Vec<ReviewGridLine>,
    pub grids_y: Vec<ReviewGridLine>,
    pub columns: Vec<ReviewColumn>,
    pub beams: Vec<ReviewBeam>,
    pub plates: Vec<ReviewPlate>,
    pub levels: Vec<ReviewLevel>,

    // Global settings
    pub default_profile: String,
    pub default_material: String,
    pub default_column_height: f32,
    pub auto_normalize_origin: bool,

    // UI state
    pub show_low_confidence: bool,

    // Debug/console lines from parsing
    pub debug_lines: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ReviewGridLine {
    pub name: String,
    pub position: f64,
    pub enabled: bool,
    pub confidence: f32,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct ReviewColumn {
    pub id: String,
    pub grid_x: String,
    pub grid_y: String,
    pub position: [f64; 2],
    pub base_level: f64,
    pub top_level: f64,
    pub profile: String,
    pub material: String,
    pub enabled: bool,
    pub confidence: f32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ReviewBeam {
    pub id: String,
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub elevation: f64,
    pub profile: String,
    pub material: String,
    pub enabled: bool,
    pub confidence: f32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ReviewPlate {
    pub id: String,
    pub position: [f64; 2],
    pub width: f64,
    pub depth: f64,
    pub thickness: f64,
    pub material: String,
    pub enabled: bool,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
pub struct ReviewLevel {
    pub name: String,
    pub elevation: f64,
    pub enabled: bool,
}

impl ImportReview {
    /// Create from parsed DrawingIR
    pub fn from_drawing_ir(
        ir: &DrawingIR,
        source: &str,
        entity_count: usize,
        debug: Vec<String>,
    ) -> Self {
        let default_profile = "H300x150x6x9".to_string();
        let default_material = "SS400".to_string();

        let mut review = Self {
            active: true,
            source_file: source.to_string(),
            entity_count,
            grids_x: Vec::new(),
            grids_y: Vec::new(),
            columns: Vec::new(),
            beams: Vec::new(),
            plates: Vec::new(),
            levels: Vec::new(),
            default_profile: default_profile.clone(),
            default_material: default_material.clone(),
            default_column_height: 4200.0,
            auto_normalize_origin: true,
            show_low_confidence: false,
            debug_lines: debug,
        };

        // Populate from IR
        for g in &ir.grids.x_grids {
            review.grids_x.push(ReviewGridLine {
                name: g.name.clone(),
                position: g.position,
                enabled: true,
                confidence: 70.0,
                source: "geometry".into(),
            });
        }
        for g in &ir.grids.y_grids {
            review.grids_y.push(ReviewGridLine {
                name: g.name.clone(),
                position: g.position,
                enabled: true,
                confidence: 70.0,
                source: "geometry".into(),
            });
        }
        for c in &ir.columns {
            review.columns.push(ReviewColumn {
                id: c.id.clone(),
                grid_x: c.grid_x.clone(),
                grid_y: c.grid_y.clone(),
                position: c.position,
                base_level: c.base_level,
                top_level: c.top_level,
                profile: c.profile.clone().unwrap_or_else(|| default_profile.clone()),
                material: default_material.clone(),
                enabled: true,
                confidence: 60.0,
                reasons: vec!["grid intersection".into()],
            });
        }
        for b in &ir.beams {
            review.beams.push(ReviewBeam {
                id: b.id.clone(),
                start: b.start_pos,
                end: b.end_pos,
                elevation: b.elevation,
                profile: b.profile.clone().unwrap_or_else(|| "H400x200x8x13".into()),
                material: default_material.clone(),
                enabled: true,
                confidence: 55.0,
                reasons: vec!["horizontal member".into()],
            });
        }
        for p in &ir.base_plates {
            review.plates.push(ReviewPlate {
                id: p.id.clone(),
                position: p.position,
                width: p.width,
                depth: p.depth,
                thickness: p.height,
                material: default_material.clone(),
                enabled: true,
                confidence: 50.0,
            });
        }
        for l in &ir.levels {
            review.levels.push(ReviewLevel {
                name: l.name.clone(),
                elevation: l.elevation,
                enabled: true,
            });
        }

        review
    }

    /// Convert back to DrawingIR (only enabled items)
    pub fn to_drawing_ir(&self) -> DrawingIR {
        let mut ir = DrawingIR::default();

        ir.grids.x_grids = self
            .grids_x
            .iter()
            .filter(|g| g.enabled)
            .map(|g| GridLine {
                name: g.name.clone(),
                position: g.position,
            })
            .collect();
        ir.grids.y_grids = self
            .grids_y
            .iter()
            .filter(|g| g.enabled)
            .map(|g| GridLine {
                name: g.name.clone(),
                position: g.position,
            })
            .collect();

        ir.columns = self
            .columns
            .iter()
            .filter(|c| c.enabled)
            .map(|c| ColumnDef {
                id: c.id.clone(),
                grid_x: c.grid_x.clone(),
                grid_y: c.grid_y.clone(),
                position: c.position,
                base_level: c.base_level,
                top_level: c.top_level,
                profile: Some(c.profile.clone()),
            })
            .collect();

        ir.beams = self
            .beams
            .iter()
            .filter(|b| b.enabled)
            .map(|b| BeamDef {
                id: b.id.clone(),
                from_grid: String::new(),
                to_grid: String::new(),
                elevation: b.elevation,
                start_pos: b.start,
                end_pos: b.end,
                profile: Some(b.profile.clone()),
            })
            .collect();

        ir.base_plates = self
            .plates
            .iter()
            .filter(|p| p.enabled)
            .map(|p| BasePlateDef {
                id: p.id.clone(),
                position: p.position,
                width: p.width,
                depth: p.depth,
                height: p.thickness,
            })
            .collect();

        ir.levels = self
            .levels
            .iter()
            .filter(|l| l.enabled)
            .map(|l| LevelDef {
                name: l.name.clone(),
                elevation: l.elevation,
            })
            .collect();

        ir
    }
}

/// Draw the import review panel as a full-screen overlay
pub fn draw_review_panel(
    ui: &mut egui::Ui,
    review: &mut ImportReview,
    rect: egui::Rect,
) -> ReviewAction {
    let mut action = ReviewAction::None;
    let painter = ui.painter();

    // Semi-transparent backdrop
    painter.rect_filled(
        rect,
        0.0,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 240),
    );

    // Content area
    let content = rect.shrink(20.0);
    let mut child_ui = ui.child_ui(content, egui::Layout::top_down(egui::Align::LEFT), None);
    let ui = &mut child_ui;

    // -- Header --
    ui.horizontal(|ui| {
        ui.heading("匯入確認");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(
                    egui::RichText::new("✕ 取消").color(egui::Color32::from_rgb(200, 60, 60)),
                )
                .clicked()
            {
                action = ReviewAction::Cancel;
            }
            if ui
                .button(
                    egui::RichText::new("✓ 確認建模")
                        .color(egui::Color32::from_rgb(60, 160, 60))
                        .strong(),
                )
                .clicked()
            {
                action = ReviewAction::Confirm;
            }
        });
    });

    ui.separator();

    // -- Source info --
    ui.horizontal(|ui| {
        ui.label(format!("檔案: {}", review.source_file));
        ui.label(format!("| 實體: {}", review.entity_count));
    });

    ui.add_space(8.0);

    // -- Summary cards --
    ui.horizontal(|ui| {
        let enabled_cols = review.columns.iter().filter(|c| c.enabled).count();
        let enabled_beams = review.beams.iter().filter(|b| b.enabled).count();
        let enabled_grids = review.grids_x.iter().filter(|g| g.enabled).count()
            + review.grids_y.iter().filter(|g| g.enabled).count();

        summary_card(
            ui,
            "軸線",
            enabled_grids,
            review.grids_x.len() + review.grids_y.len(),
        );
        summary_card(ui, "柱", enabled_cols, review.columns.len());
        summary_card(ui, "梁", enabled_beams, review.beams.len());
        summary_card(
            ui,
            "板",
            review.plates.iter().filter(|p| p.enabled).count(),
            review.plates.len(),
        );
        summary_card(
            ui,
            "標高",
            review.levels.iter().filter(|l| l.enabled).count(),
            review.levels.len(),
        );
    });

    ui.add_space(8.0);

    // -- Scrollable content --
    egui::ScrollArea::vertical()
        .max_height(rect.height() - 200.0)
        .show(ui, |ui| {
            // -- Global settings --
            ui.collapsing("全域設定", |ui| {
                ui.horizontal(|ui| {
                    ui.label("預設斷面:");
                    ui.text_edit_singleline(&mut review.default_profile);
                });
                ui.horizontal(|ui| {
                    ui.label("預設材質:");
                    ui.text_edit_singleline(&mut review.default_material);
                });
                ui.horizontal(|ui| {
                    ui.label("預設柱高:");
                    ui.add(
                        egui::DragValue::new(&mut review.default_column_height)
                            .suffix(" mm")
                            .speed(100.0),
                    );
                });
                ui.checkbox(&mut review.auto_normalize_origin, "自動歸零座標");
                ui.checkbox(&mut review.show_low_confidence, "顯示低信心項目");

                if ui.button("套用預設到所有構件").clicked() {
                    let prof = review.default_profile.clone();
                    let mat = review.default_material.clone();
                    let h = review.default_column_height as f64;
                    for c in &mut review.columns {
                        c.profile = prof.clone();
                        c.material = mat.clone();
                        c.top_level = h;
                    }
                    for b in &mut review.beams {
                        b.material = mat.clone();
                        b.elevation = h;
                    }
                }
            });

            ui.add_space(4.0);

            // -- Grids --
            ui.collapsing(
                format!(
                    "軸線 ({} X + {} Y)",
                    review.grids_x.len(),
                    review.grids_y.len()
                ),
                |ui| {
                    ui.label("X 軸線:");
                    for g in &mut review.grids_x {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut g.enabled, "");
                            ui.text_edit_singleline(&mut g.name);
                            ui.label(format!("@ {:.0} mm", g.position));
                            confidence_badge(ui, g.confidence);
                        });
                    }
                    ui.add_space(4.0);
                    ui.label("Y 軸線:");
                    for g in &mut review.grids_y {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut g.enabled, "");
                            ui.text_edit_singleline(&mut g.name);
                            ui.label(format!("@ {:.0} mm", g.position));
                            confidence_badge(ui, g.confidence);
                        });
                    }
                },
            );

            // -- Columns --
            ui.collapsing(format!("柱 ({})", review.columns.len()), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("全選").clicked() {
                        review.columns.iter_mut().for_each(|c| c.enabled = true);
                    }
                    if ui.button("全不選").clicked() {
                        review.columns.iter_mut().for_each(|c| c.enabled = false);
                    }
                    if ui.button("僅高信心").clicked() {
                        review
                            .columns
                            .iter_mut()
                            .for_each(|c| c.enabled = c.confidence >= 60.0);
                    }
                });

                let show_low = review.show_low_confidence;
                for col in &mut review.columns {
                    if !show_low && col.confidence < 30.0 {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut col.enabled, "");
                        ui.label(format!("{}/{}", col.grid_x, col.grid_y));
                        ui.text_edit_singleline(&mut col.profile);
                        ui.label(format!("H: {:.0}", col.top_level - col.base_level));
                        confidence_badge(ui, col.confidence);
                    });
                }
            });

            // -- Beams --
            ui.collapsing(format!("梁 ({})", review.beams.len()), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("全選").clicked() {
                        review.beams.iter_mut().for_each(|b| b.enabled = true);
                    }
                    if ui.button("全不選").clicked() {
                        review.beams.iter_mut().for_each(|b| b.enabled = false);
                    }
                    if ui.button("僅高信心").clicked() {
                        review
                            .beams
                            .iter_mut()
                            .for_each(|b| b.enabled = b.confidence >= 50.0);
                    }
                });

                let show_low = review.show_low_confidence;
                for beam in &mut review.beams {
                    if !show_low && beam.confidence < 30.0 {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut beam.enabled, "");
                        ui.label(&beam.id);
                        ui.text_edit_singleline(&mut beam.profile);
                        let len = ((beam.end[0] - beam.start[0]).powi(2)
                            + (beam.end[1] - beam.start[1]).powi(2))
                        .sqrt();
                        ui.label(format!("L: {:.0}mm", len));
                        ui.label(format!("EL: {:.0}", beam.elevation));
                        confidence_badge(ui, beam.confidence);
                    });
                }
            });

            // -- Plates --
            if !review.plates.is_empty() {
                ui.collapsing(format!("底板 ({})", review.plates.len()), |ui| {
                    let show_low = review.show_low_confidence;
                    for plate in &mut review.plates {
                        if !show_low && plate.confidence < 30.0 {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut plate.enabled, "");
                            ui.label(&plate.id);
                            ui.label(format!(
                                "{:.0}x{:.0}x{:.0}",
                                plate.width, plate.depth, plate.thickness
                            ));
                            confidence_badge(ui, plate.confidence);
                        });
                    }
                });
            }

            // -- Levels --
            ui.collapsing(format!("標高 ({})", review.levels.len()), |ui| {
                for level in &mut review.levels {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut level.enabled, "");
                        ui.text_edit_singleline(&mut level.name);
                        ui.add(
                            egui::DragValue::new(&mut level.elevation)
                                .suffix(" mm")
                                .speed(100.0),
                        );
                    });
                }
                if ui.button("+ 新增標高").clicked() {
                    review.levels.push(ReviewLevel {
                        name: format!("L{}", review.levels.len() + 1),
                        elevation: 3000.0,
                        enabled: true,
                    });
                }
            });

            // -- Debug Console --
            if !review.debug_lines.is_empty() {
                ui.collapsing("解析記錄 (Console)", |ui| {
                    for line in &review.debug_lines {
                        ui.label(egui::RichText::new(line).monospace().size(10.0));
                    }
                });
            }
        });

    action
}

fn summary_card(ui: &mut egui::Ui, label: &str, enabled: usize, total: usize) {
    let frame = egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 240))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(229, 231, 239),
        ))
        .rounding(12.0)
        .inner_margin(egui::Margin::symmetric(16.0, 8.0));

    frame.show(ui, |ui: &mut egui::Ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(11.0)
                    .color(egui::Color32::from_rgb(110, 118, 135)),
            );
            ui.label(
                egui::RichText::new(format!("{}/{}", enabled, total))
                    .size(18.0)
                    .strong(),
            );
        });
    });
}

fn confidence_badge(ui: &mut egui::Ui, score: f32) {
    let (color, label) = if score >= 70.0 {
        (
            egui::Color32::from_rgb(60, 160, 60),
            format!("{:.0}%", score),
        )
    } else if score >= 40.0 {
        (
            egui::Color32::from_rgb(200, 160, 40),
            format!("{:.0}%", score),
        )
    } else {
        (
            egui::Color32::from_rgb(200, 60, 60),
            format!("{:.0}%", score),
        )
    };

    let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 18.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 9.0, color.linear_multiply(0.15));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &label,
        egui::FontId::proportional(9.0),
        color,
    );
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReviewAction {
    None,
    Confirm,
    Cancel,
}

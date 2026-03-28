//! Navigation overlays: import review panels, floating view buttons, tool info card,
//! navigation pad, zoom buttons, coordinate chips, tool cursor

use eframe::egui;

use crate::app::{KolibriApp, CursorHint, DrawState, PullFace, RenderMode, ScaleHandle, SelectionMode, SnapType, Tool, WorkMode};
use crate::scene::Shape;

impl KolibriApp {
    /// Draw navigation overlays (import review, view buttons, nav pad, zoom, coords, tool cursor)
    pub(crate) fn draw_navigation_overlays(
        &mut self,
        ui: &mut egui::Ui,
        vp: glam::Mat4,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
                // ── DXF Smart Import confirmation panel (legacy, redirects to review) ──
                if let Some(ir) = self.pending_ir.take() {
                    // Convert pending_ir into the new review panel
                    let entity_count = ir.columns.len() + ir.beams.len() + ir.base_plates.len();
                    let debug = ir.debug_report.clone();
                    self.import_review = Some(crate::import_review::ImportReview::from_drawing_ir(
                        &ir, &"DXF", entity_count, debug,
                    ));
                }

                // ── Import Review Panel (full-screen overlay) ──
                if let Some(ref mut review) = self.import_review {
                    if review.active {
                        let action = crate::import_review::draw_review_panel(ui, review, rect);
                        match action {
                            crate::import_review::ReviewAction::Confirm => {
                                let ir = review.to_drawing_ir();
                                self.scene.snapshot();
                                let result = crate::builders::steel_builder::build_from_ir(&mut self.scene, &ir);
                                self.editor.selected_ids.clear();
                                self.editor.selected_ids.extend(result.ids);
                                self.zoom_extents();
                                let msg = format!("建模完成: {} 柱 + {} 梁 + {} 底板",
                                    result.columns_created, result.beams_created, result.plates_created);
                                self.file_message = Some((msg, std::time::Instant::now()));
                                self.import_review = None;
                            }
                            crate::import_review::ReviewAction::Cancel => {
                                self.import_review = None;
                            }
                            _ => {}
                        }
                    }
                }

                // ── Unified Smart Import confirmation panel ──
                if let Some(ref ir) = self.pending_unified_ir.clone() {
                    let panel_w = 420.0;
                    let panel_h = 380.0;
                    let panel_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(panel_w, panel_h));

                    ui.painter().rect_filled(panel_rect, 16.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 245));
                    ui.painter().rect_stroke(panel_rect, 16.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));

                    let mut y_ir = panel_rect.top() + 20.0;
                    let x_ir = panel_rect.left() + 20.0;

                    ui.painter().text(egui::pos2(panel_rect.center().x, y_ir), egui::Align2::CENTER_TOP,
                        format!("智慧匯入結果 ({})", ir.source_format.to_uppercase()),
                        egui::FontId::proportional(16.0), egui::Color32::from_rgb(31, 36, 48));
                    y_ir += 30.0;

                    let heavy_import = crate::app::KolibriApp::is_heavy_import(ir);
                    let info_lines = [
                        format!("來源檔案: {}", std::path::Path::new(&ir.source_file).file_name()
                            .map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
                        format!("網格數: {}", ir.stats.mesh_count),
                        format!("實例數: {}", ir.stats.instance_count),
                        format!("頂點數: {}", ir.stats.vertex_count),
                        format!("面數: {}", ir.stats.face_count),
                        format!("群組數: {}", ir.stats.group_count),
                        format!("元件定義數: {}", ir.stats.component_count),
                        format!("材質數: {}", ir.stats.material_count),
                    ];

                    for line_text in &info_lines {
                        ui.painter().text(egui::pos2(x_ir, y_ir), egui::Align2::LEFT_TOP,
                            line_text, egui::FontId::proportional(12.0), egui::Color32::from_rgb(60, 65, 80));
                        y_ir += 20.0;
                    }

                    if heavy_import {
                        y_ir += 6.0;
                        ui.painter().text(
                            egui::pos2(x_ir, y_ir),
                            egui::Align2::LEFT_TOP,
                            "大型 SKP 將啟用保護模式: 略過自動縮放，並延後 autosave。",
                            egui::FontId::proportional(11.0),
                            egui::Color32::from_rgb(180, 120, 40),
                        );
                        y_ir += 26.0;
                    }

                    y_ir += 15.0;

                    // Confirm button
                    let btn_confirm = egui::Rect::from_min_size(egui::pos2(panel_rect.center().x - 80.0, y_ir), egui::vec2(70.0, 32.0));
                    let btn_cancel = egui::Rect::from_min_size(egui::pos2(panel_rect.center().x + 10.0, y_ir), egui::vec2(70.0, 32.0));

                    ui.painter().rect_filled(btn_confirm, 8.0, egui::Color32::from_rgb(76, 139, 245));
                    ui.painter().text(btn_confirm.center(), egui::Align2::CENTER_CENTER, "確認建模",
                        egui::FontId::proportional(12.0), egui::Color32::WHITE);

                    ui.painter().rect_filled(btn_cancel, 8.0, egui::Color32::from_rgb(200, 200, 200));
                    ui.painter().text(btn_cancel.center(), egui::Align2::CENTER_CENTER, "取消",
                        egui::FontId::proportional(12.0), egui::Color32::from_rgb(60, 60, 60));

                    let confirm_resp = ui.allocate_rect(btn_confirm, egui::Sense::click());
                    let cancel_resp = ui.allocate_rect(btn_cancel, egui::Sense::click());

                    if confirm_resp.clicked() {
                        self.log_import_phase(
                            "import_review_confirmed",
                            format!(
                                "format={} source_file={} heavy_mode={}",
                                ir.source_format.to_uppercase(),
                                ir.source_file,
                                crate::app::KolibriApp::is_heavy_import(ir),
                            ),
                        );
                        let ir_data = self.pending_unified_ir.take().unwrap();
                        self.start_scene_build_task(ir_data);
                    }
                    if cancel_resp.clicked() {
                        self.log_import_phase(
                            "import_review_cancelled",
                            format!(
                                "format={} source_file={} heavy_mode={}",
                                ir.source_format.to_uppercase(),
                                ir.source_file,
                                crate::app::KolibriApp::is_heavy_import(ir),
                            ),
                        );
                        self.pending_unified_ir = None;
                    }
                }

                // Unsaved changes confirmation overlay
                if self.pending_action.is_some() {
                    let popup_rect = egui::Rect::from_center_size(
                        rect.center(),
                        egui::vec2(350.0, 100.0),
                    );
                    ui.painter().rect_filled(popup_rect, 8.0, egui::Color32::from_rgba_unmultiplied(30, 30, 40, 240));
                    ui.painter().rect_stroke(popup_rect, 8.0, egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 150, 220)));
                    ui.painter().text(popup_rect.center_top() + egui::vec2(0.0, 20.0),
                        egui::Align2::CENTER_TOP, "場景有未儲存的修改",
                        egui::FontId::proportional(15.0), egui::Color32::WHITE);
                    ui.painter().text(popup_rect.center() + egui::vec2(0.0, 5.0),
                        egui::Align2::CENTER_CENTER, "按 Y 繼續（放棄修改）/ N 取消",
                        egui::FontId::proportional(13.0), egui::Color32::from_gray(180));
                }

                // ── Floating view buttons (top-center of viewport) ──
                {
                    let view_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.center().x - 120.0, rect.top() + 8.0),
                        egui::vec2(240.0, 44.0),
                    );

                    // Background pill
                    ui.painter().rect_filled(view_rect, 18.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 225));
                    ui.painter().rect_stroke(view_rect, 18.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200)));

                    // View buttons
                    let views = ["\u{900f}\u{8996}", "\u{6b63}\u{8996}", "\u{4fef}\u{8996}", "\u{5de6}\u{8996}"];
                    let btn_w = 50.0;
                    let padding = 8.0;
                    for (i, label) in views.iter().enumerate() {
                        let x = view_rect.left() + padding + i as f32 * (btn_w + 6.0);
                        let btn_rect = egui::Rect::from_min_size(
                            egui::pos2(x, view_rect.top() + 6.0),
                            egui::vec2(btn_w, 32.0),
                        );

                        let is_active = match i {
                            0 => !self.viewer.use_ortho,  // 透視
                            1 => self.viewer.use_ortho && self.viewer.camera.pitch.abs() < 0.1, // 正視
                            2 => self.viewer.use_ortho && self.viewer.camera.pitch < -1.0, // 俯視
                            3 => self.viewer.use_ortho && (self.viewer.camera.yaw + std::f32::consts::FRAC_PI_2).abs() < 0.1, // 左視
                            _ => false,
                        };

                        let response = ui.allocate_rect(btn_rect, egui::Sense::click());
                        let bg = if is_active {
                            egui::Color32::from_rgba_unmultiplied(76, 139, 245, 30)
                        } else if response.hovered() {
                            egui::Color32::from_rgb(240, 242, 248)
                        } else {
                            egui::Color32::WHITE
                        };
                        let text_color = if is_active {
                            egui::Color32::from_rgb(76, 139, 245)
                        } else {
                            egui::Color32::from_rgb(110, 118, 135)
                        };

                        ui.painter().rect_filled(btn_rect, 12.0, bg);
                        ui.painter().rect_stroke(btn_rect, 12.0,
                            egui::Stroke::new(1.0, if is_active {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 90)
                            } else {
                                egui::Color32::from_rgb(229, 231, 239)
                            }));
                        ui.painter().text(btn_rect.center(), egui::Align2::CENTER_CENTER,
                            label, egui::FontId::proportional(12.0), text_color);

                        if response.clicked() {
                            match i {
                                0 => self.viewer.use_ortho = false,
                                1 => { self.viewer.use_ortho = true; self.viewer.animate_camera_to(|c| c.set_front()); }
                                2 => { self.viewer.use_ortho = true; self.viewer.animate_camera_to(|c| c.set_top()); }
                                3 => { self.viewer.use_ortho = true; self.viewer.animate_camera_to(|c| c.set_left()); }
                                _ => {}
                            }
                        }
                    }
                }

                // ── Tool info card (top-left of viewport) ──
                {
                    let card_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + 16.0, rect.top() + 16.0),
                        egui::vec2(280.0, 60.0),
                    );
                    ui.painter().rect_filled(card_rect, 18.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 225));
                    ui.painter().rect_stroke(card_rect, 18.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200)));

                    let tool_name = match self.editor.tool {
                        Tool::Select => "\u{9078}\u{53d6} / Move-Ready",
                        Tool::Move => "\u{79fb}\u{52d5}\u{5de5}\u{5177}",
                        Tool::Rotate => "\u{65cb}\u{8f49}\u{5de5}\u{5177}",
                        Tool::CreateBox => "\u{65b9}\u{584a}\u{5de5}\u{5177}",
                        Tool::PushPull => "\u{63a8}\u{62c9}\u{5de5}\u{5177}",
                        Tool::Line => "\u{7dda}\u{6bb5}\u{5de5}\u{5177}",
                        _ => "\u{5de5}\u{5177}",
                    };
                    ui.painter().text(
                        egui::pos2(card_rect.left() + 14.0, card_rect.top() + 16.0),
                        egui::Align2::LEFT_TOP,
                        format!("\u{76ee}\u{524d}\u{5de5}\u{5177}\u{ff1a}{}", tool_name),
                        egui::FontId::proportional(13.0),
                        egui::Color32::from_rgb(31, 36, 48),
                    );
                    ui.painter().text(
                        egui::pos2(card_rect.left() + 14.0, card_rect.top() + 36.0),
                        egui::Align2::LEFT_TOP,
                        &self.status_text(),
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgb(110, 118, 135),
                    );
                }

                // ── Navigation pad (bottom-left of viewport) ──
                {
                    let pad_size = 130.0;
                    let pad_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + 16.0, rect.bottom() - pad_size - 60.0),
                        egui::vec2(pad_size, pad_size + 24.0),
                    );
                    ui.painter().rect_filled(pad_rect, 22.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 225));
                    ui.painter().rect_stroke(pad_rect, 22.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210)));

                    // Title
                    ui.painter().text(
                        egui::pos2(pad_rect.left() + 12.0, pad_rect.top() + 10.0),
                        egui::Align2::LEFT_TOP, "\u{8996}\u{89d2} / \u{5e73}\u{79fb}",
                        egui::FontId::proportional(11.0), egui::Color32::from_rgb(110, 118, 135));

                    // 3x3 button grid
                    let arrows = ["", "\u{2191}", "", "\u{2190}", "\u{29bf}", "\u{2192}", "", "\u{2193}", ""];
                    let btn_size = 32.0;
                    let gap = 6.0;
                    let grid_start_x = pad_rect.center().x - (btn_size * 1.5 + gap);
                    let grid_start_y = pad_rect.top() + 28.0;

                    for (i, label) in arrows.iter().enumerate() {
                        if label.is_empty() { continue; }
                        let row = i / 3;
                        let col = i % 3;
                        let btn_rect = egui::Rect::from_min_size(
                            egui::pos2(
                                grid_start_x + col as f32 * (btn_size + gap),
                                grid_start_y + row as f32 * (btn_size + gap),
                            ),
                            egui::vec2(btn_size, btn_size),
                        );

                        let response = ui.allocate_rect(btn_rect, egui::Sense::click());
                        let bg = if response.hovered() {
                            egui::Color32::from_rgb(240, 242, 248)
                        } else {
                            egui::Color32::WHITE
                        };
                        ui.painter().rect_filled(btn_rect, 12.0, bg);
                        ui.painter().rect_stroke(btn_rect, 12.0,
                            egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));
                        ui.painter().text(btn_rect.center(), egui::Align2::CENTER_CENTER,
                            label, egui::FontId::proportional(14.0), egui::Color32::from_rgb(110, 118, 135));

                        if response.clicked() {
                            let step = self.viewer.camera.distance * 0.1;
                            match i {
                                1 => self.viewer.camera.walk_forward(step),
                                3 => self.viewer.camera.walk_strafe(-step),
                                4 => self.viewer.animate_camera_to(|c| c.set_iso()),
                                5 => self.viewer.camera.walk_strafe(step),
                                7 => self.viewer.camera.walk_forward(-step),
                                _ => {}
                            }
                        }
                    }
                }

                // ── Zoom In / Zoom Out buttons (below navigation pad) ──
                {
                    let zoom_btn_w = 58.0;
                    let zoom_btn_h = 32.0;
                    let zoom_gap = 6.0;
                    let zoom_total_w = zoom_btn_w * 2.0 + zoom_gap;
                    let zoom_x = rect.left() + 16.0 + (130.0 - zoom_total_w) / 2.0;
                    let zoom_y = rect.bottom() - 52.0; // below nav pad

                    // "+" Zoom In
                    let plus_rect = egui::Rect::from_min_size(
                        egui::pos2(zoom_x, zoom_y),
                        egui::vec2(zoom_btn_w, zoom_btn_h),
                    );
                    let plus_resp = ui.allocate_rect(plus_rect, egui::Sense::click());
                    let plus_bg = if plus_resp.hovered() {
                        egui::Color32::from_rgb(240, 242, 248)
                    } else {
                        egui::Color32::WHITE
                    };
                    ui.painter().rect_filled(plus_rect, 12.0, plus_bg);
                    ui.painter().rect_stroke(plus_rect, 12.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));
                    ui.painter().text(plus_rect.center(), egui::Align2::CENTER_CENTER,
                        "+", egui::FontId::proportional(16.0), egui::Color32::from_rgb(110, 118, 135));
                    if plus_resp.clicked() {
                        // Zoom in: decrease distance by 20%
                        self.viewer.camera.distance = (self.viewer.camera.distance * 0.8).clamp(200.0, 100_000.0);
                    }

                    // "-" Zoom Out
                    let minus_rect = egui::Rect::from_min_size(
                        egui::pos2(zoom_x + zoom_btn_w + zoom_gap, zoom_y),
                        egui::vec2(zoom_btn_w, zoom_btn_h),
                    );
                    let minus_resp = ui.allocate_rect(minus_rect, egui::Sense::click());
                    let minus_bg = if minus_resp.hovered() {
                        egui::Color32::from_rgb(240, 242, 248)
                    } else {
                        egui::Color32::WHITE
                    };
                    ui.painter().rect_filled(minus_rect, 12.0, minus_bg);
                    ui.painter().rect_stroke(minus_rect, 12.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));
                    ui.painter().text(minus_rect.center(), egui::Align2::CENTER_CENTER,
                        "\u{2212}", egui::FontId::proportional(16.0), egui::Color32::from_rgb(110, 118, 135));
                    if minus_resp.clicked() {
                        // Zoom out: increase distance by 20%
                        self.viewer.camera.distance = (self.viewer.camera.distance * 1.2).clamp(200.0, 100_000.0);
                    }
                }

                // ── Coordinate chips (bottom-center of viewport) ──
                {
                    let chips_y = rect.bottom() - 40.0;
                    let chip_data = [
                        format!("X: {:.0}", self.editor.mouse_ground.map(|p| p[0]).unwrap_or(0.0)),
                        format!("Y: {:.0}", self.editor.mouse_ground.map(|p| p[1]).unwrap_or(0.0)),
                        format!("Z: {:.0}", self.editor.mouse_ground.map(|p| p[2]).unwrap_or(0.0)),
                        "Snap: ON".to_string(),
                        "Units: mm".to_string(),
                    ];

                    let chip_w = 70.0;
                    let total_w = chip_data.len() as f32 * (chip_w + 8.0);
                    let start_x = rect.center().x - total_w / 2.0;

                    for (i, text) in chip_data.iter().enumerate() {
                        let chip_rect = egui::Rect::from_min_size(
                            egui::pos2(start_x + i as f32 * (chip_w + 8.0), chips_y),
                            egui::vec2(chip_w, 28.0),
                        );
                        ui.painter().rect_filled(chip_rect, 999.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 215));
                        ui.painter().rect_stroke(chip_rect, 999.0,
                            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200)));
                        ui.painter().text(chip_rect.center(), egui::Align2::CENTER_CENTER,
                            text, egui::FontId::proportional(11.0), egui::Color32::from_rgb(31, 36, 48));
                    }
                }

                // ── Tool cursor icon (small tool icon follows mouse) ──
                if response.hovered() {
                    let mx = rect.min.x + self.editor.mouse_screen[0];
                    let my = rect.min.y + self.editor.mouse_screen[1];

                    // Draw mini tool icon (20x20) at cursor offset
                    let icon_size = 20.0;
                    let icon_rect = egui::Rect::from_min_size(
                        egui::pos2(mx + 14.0, my + 14.0),  // bottom-right of cursor
                        egui::vec2(icon_size, icon_size),
                    );

                    // Semi-transparent background circle per tool category
                    let bg_color = match self.editor.tool {
                        Tool::Select => egui::Color32::from_rgba_unmultiplied(76, 139, 245, 180),
                        Tool::Move => egui::Color32::from_rgba_unmultiplied(245, 166, 35, 180),
                        Tool::Rotate => egui::Color32::from_rgba_unmultiplied(180, 80, 220, 180),
                        Tool::Scale => egui::Color32::from_rgba_unmultiplied(80, 200, 120, 180),
                        Tool::Line | Tool::Arc | Tool::Rectangle | Tool::Circle => egui::Color32::from_rgba_unmultiplied(60, 60, 60, 180),
                        Tool::CreateBox | Tool::CreateCylinder | Tool::CreateSphere => egui::Color32::from_rgba_unmultiplied(76, 139, 245, 180),
                        Tool::PushPull => egui::Color32::from_rgba_unmultiplied(245, 100, 60, 180),
                        Tool::PaintBucket => egui::Color32::from_rgba_unmultiplied(220, 80, 160, 180),
                        Tool::Eraser => egui::Color32::from_rgba_unmultiplied(220, 50, 50, 180),
                        Tool::TapeMeasure | Tool::Dimension => egui::Color32::from_rgba_unmultiplied(100, 180, 100, 180),
                        Tool::Text => egui::Color32::from_rgba_unmultiplied(180, 140, 60, 180),
                        _ => egui::Color32::from_rgba_unmultiplied(100, 100, 100, 160),
                    };

                    // Draw circular background
                    let center = icon_rect.center();
                    ui.painter().circle_filled(center, icon_size * 0.55, bg_color);

                    // Draw the tool icon inside (shrunk)
                    let inner_rect = icon_rect.shrink(3.0);
                    crate::icons::draw_tool_icon(ui.painter(), inner_rect, self.editor.tool, egui::Color32::WHITE);
                }

    }
}

//! AISC 接頭確認對話框 UI
//! 選取兩構件 → 按接頭鍵 → 彈出視窗顯示 AISC 分析 → 使用者確認後繪製

use eframe::egui;
use crate::app::KolibriApp;
use kolibri_core::steel_connection::*;

impl KolibriApp {
    /// 渲染 AISC 接頭確認對話框（在 update_ui 中呼叫）
    pub(crate) fn steel_connection_dialog(&mut self, ctx: &egui::Context) {
        let dialog = match &self.editor.conn_dialog {
            Some(d) => d,
            None => return,
        };

        // 暫存需要的值（避免 borrow 問題）
        let suggestions = dialog.suggestions.clone();
        let beam_sec = dialog.beam_section;
        let col_sec = dialog.col_section;
        let _member_ids = dialog.member_ids.clone();
        let _intent = dialog.intent;

        let mut close = false;
        let mut confirm_idx: Option<usize> = None;

        egui::Window::new("AISC 360-22 接頭設計")
            .collapsible(false)
            .resizable(true)
            .default_size([520.0, 600.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let sub = egui::Color32::from_rgb(110, 118, 135);

                // ── 按鈕先在底部預留（用 bottom_up layout）──
                // egui 沒有 bottom panel，改用 TopBottomPanel 模式：
                // 先算好按鈕高度，用 separator + horizontal 放在最後

                // ── 上方可捲動區域：構件資訊 + 方案 + 參數 ──
                let btn_height = 56.0; // 按鈕區保留高度
                let scroll_h = (ui.available_height() - btn_height).max(200.0);

                egui::ScrollArea::vertical()
                    .max_height(scroll_h)
                    .id_source("conn_dialog_scroll")
                    .show(ui, |ui| {
                        // ── 構件資訊 ──
                        ui.heading("構件分析");
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!(
                                "梁: H{:.0}x{:.0}x{:.0}x{:.0}", beam_sec.0, beam_sec.1, beam_sec.2, beam_sec.3
                            )).strong());
                            ui.separator();
                            ui.label(egui::RichText::new(format!(
                                "柱: H{:.0}x{:.0}x{:.0}x{:.0}", col_sec.0, col_sec.1, col_sec.2, col_sec.3
                            )).strong());
                        });
                        ui.add_space(4.0);

                        // 加勁板判斷
                        let (need_stiff, stiff_reason) = need_stiffeners_check(beam_sec, col_sec);
                        if need_stiff {
                            ui.colored_label(egui::Color32::from_rgb(220, 160, 40),
                                format!("AISC J10: {}", stiff_reason));
                        } else {
                            ui.label(egui::RichText::new(format!("AISC J10: {}", stiff_reason)).color(sub));
                        }
                        ui.separator();

                        // ── AISC 建議方案 ──
                        ui.heading("建議方案");
                        ui.add_space(4.0);

                        let selected = self.editor.conn_dialog.as_ref().map_or(0, |d| d.selected_idx);

                        for (i, s) in suggestions.iter().enumerate() {
                            let is_selected = i == selected;
                            let frame_stroke = if is_selected {
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(76, 139, 245))
                            } else {
                                egui::Stroke::new(1.0, egui::Color32::from_gray(200))
                            };
                            let fill = if is_selected {
                                egui::Color32::from_rgb(235, 242, 255)
                            } else {
                                egui::Color32::from_rgb(250, 250, 252)
                            };

                            egui::Frame::none()
                                .fill(fill)
                                .stroke(frame_stroke)
                                .rounding(8.0)
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    // 方案標題
                                    let title = format!("方案 {} — {}", i + 1, s.conn_type.label());
                                    if ui.selectable_label(is_selected,
                                        egui::RichText::new(&title).strong().size(13.0)
                                    ).clicked() {
                                        if let Some(ref mut d) = self.editor.conn_dialog {
                                            d.selected_idx = i;
                                            d.bolt_size = s.bolt_size;
                                            d.bolt_grade = s.bolt_grade;
                                            d.plate_thickness = s.plate_thickness;
                                            d.add_stiffeners = s.need_stiffeners;
                                            d.weld_size = minimum_fillet_weld_size(s.plate_thickness);
                                        }
                                    }

                                    ui.label(egui::RichText::new(&s.reason).size(11.0).color(sub));

                                    // 螺栓詳情
                                    ui.horizontal(|ui| {
                                        ui.label(format!("螺栓: {} {}", s.bolt_size.label(), s.bolt_grade.label()));
                                        ui.separator();
                                        ui.label(format!("孔徑: Ø{:.0}mm", s.bolt_size.hole_diameter()));
                                        ui.separator();
                                        ui.label(format!("邊距: ≥{:.0}mm", s.bolt_size.min_edge()));
                                    });

                                    // 板件
                                    ui.label(format!("端板/剪力板厚: {:.0}mm | 加勁板: {}",
                                        s.plate_thickness,
                                        if s.need_stiffeners { "需要" } else { "不需" },
                                    ));

                                    // 強度
                                    let cap = &s.estimated_capacity;
                                    let pass_color = if cap.pass {
                                        egui::Color32::from_rgb(40, 160, 60)
                                    } else {
                                        egui::Color32::from_rgb(220, 50, 50)
                                    };
                                    ui.horizontal(|ui| {
                                        ui.label(format!("抗剪: {:.0}kN", cap.total_bolt_shear));
                                        ui.label(format!("抗拉: {:.0}kN", cap.total_bolt_tension));
                                        ui.label(format!("焊接: {:.0}kN", cap.total_weld_capacity));
                                        ui.colored_label(pass_color,
                                            if cap.pass { "PASS" } else { "FAIL" });
                                    });

                                    // AISC 條文
                                    ui.label(egui::RichText::new(&s.aisc_ref).size(9.5).color(sub));
                                });
                            ui.add_space(4.0);
                        }

                        ui.separator();

                        // ── 使用者可調參數 ──
                        ui.heading("參數調整");
                        if let Some(ref mut d) = self.editor.conn_dialog {
                            ui.horizontal(|ui| {
                                ui.label("螺栓:");
                                egui::ComboBox::from_id_source("dlg_bolt_size")
                                    .width(70.0)
                                    .selected_text(d.bolt_size.label())
                                    .show_ui(ui, |ui| {
                                        for &bs in BoltSize::ALL {
                                            if ui.selectable_label(d.bolt_size == bs,
                                                format!("{} Ø{:.0} 孔Ø{:.0}", bs.label(), bs.diameter(), bs.hole_diameter())
                                            ).clicked() {
                                                d.bolt_size = bs;
                                            }
                                        }
                                    });
                                ui.label("等級:");
                                egui::ComboBox::from_id_source("dlg_bolt_grade")
                                    .width(60.0)
                                    .selected_text(d.bolt_grade.label())
                                    .show_ui(ui, |ui| {
                                        for &bg in BoltGrade::ALL {
                                            if ui.selectable_label(d.bolt_grade == bg, bg.label()).clicked() {
                                                d.bolt_grade = bg;
                                            }
                                        }
                                    });
                            });
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut d.plate_thickness)
                                    .speed(0.5).prefix("板厚: ").suffix(" mm").range(10.0..=60.0));
                                ui.add(egui::DragValue::new(&mut d.weld_size)
                                    .speed(0.5).prefix("焊腳: ").suffix(" mm").range(4.0..=20.0));
                            });
                            ui.checkbox(&mut d.add_stiffeners, "加勁板 (AISC J10)");
                        }
                    }); // end ScrollArea

                ui.add_space(4.0);
                ui.separator();

                // ── 按鈕（固定在底部，不在 ScrollArea 內）──
                ui.horizontal(|ui| {
                    let sel = self.editor.conn_dialog.as_ref().map_or(0, |d| d.selected_idx);
                    if ui.add_sized([160.0, 36.0],
                        egui::Button::new(egui::RichText::new("確認建立接頭").size(14.0).strong())
                    ).clicked() {
                        confirm_idx = Some(sel);
                    }
                    if ui.add_sized([100.0, 36.0],
                        egui::Button::new("取消")
                    ).clicked() {
                        close = true;
                    }
                });
            });

        // 處理確認/取消
        if close {
            self.editor.conn_dialog = None;
        }
        if let Some(idx) = confirm_idx {
            self.execute_connection_from_dialog(idx);
            self.editor.conn_dialog = None;
        }
    }
}

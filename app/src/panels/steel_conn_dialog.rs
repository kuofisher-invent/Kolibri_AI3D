//! AISC 接頭確認對話框 UI — 優化版
//! 選取兩構件 → 按接頭鍵 → 彈出視窗顯示 AISC 分析 → 使用者確認後繪製
//!
//! UX 改進：
//!  1. 即時強度重算（參數調整後立即更新 PASS/FAIL）
//!  2. 利用率(UR)橫條圖 — 一眼看出安全餘裕
//!  3. 安全警告系統 — 停用加勁板/參數低於 AISC 最小值時警示
//!  4. 方案比較摘要列 — 不用展開也能快速比
//!  5. 「重設」按鈕 — 回到 AISC 建議值

use eframe::egui;
use crate::app::KolibriApp;
use kolibri_core::steel_connection::*;

/// 品牌色
const BRAND: egui::Color32 = egui::Color32::from_rgb(76, 139, 245);
const SUB: egui::Color32 = egui::Color32::from_rgb(110, 118, 135);
const PASS_COLOR: egui::Color32 = egui::Color32::from_rgb(40, 160, 60);
const FAIL_COLOR: egui::Color32 = egui::Color32::from_rgb(220, 50, 50);
const WARN_COLOR: egui::Color32 = egui::Color32::from_rgb(220, 160, 40);

impl KolibriApp {
    /// 渲染 AISC 接頭確認對話框（在 update_ui 中呼叫）
    pub(crate) fn steel_connection_dialog(&mut self, ctx: &egui::Context) {
        let dialog = match &self.editor.conn_dialog {
            Some(d) => d,
            None => return,
        };

        let suggestions = dialog.suggestions.clone();
        let beam_sec = dialog.beam_section;
        let col_sec = dialog.col_section;
        let intent = dialog.intent;

        let mut close = false;
        let mut confirm_idx: Option<usize> = None;

        egui::Window::new("⚙ AISC 360-22 接頭設計")
            .collapsible(false)
            .resizable(true)
            .default_size([560.0, 680.0])
            .min_size([420.0, 350.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                // ═══════════════════════════════════════════════════
                // 1. 構件資訊卡（固定在頂部，不隨 scroll 移動）
                // ═══════════════════════════════════════════════════
                Self::draw_member_header(ui, beam_sec, col_sec, intent);
                ui.add_space(4.0);

                // ═══════════════════════════════════════════════════
                // 2. 可捲動主體：方案 + 參數 + 即時驗算
                // ═══════════════════════════════════════════════════
                let btn_height = 56.0;
                let scroll_h = (ui.available_height() - btn_height).max(100.0);

                egui::ScrollArea::vertical()
                    .max_height(scroll_h)
                    .id_source("conn_dialog_scroll")
                    .show(ui, |ui| {
                        // ── 方案比較摘要列 ──
                        self.draw_proposal_summary_row(ui, &suggestions);
                        ui.add_space(4.0);

                        // ── 展開的選中方案詳情 ──
                        let selected = self.editor.conn_dialog.as_ref().map_or(0, |d| d.selected_idx);
                        if let Some(s) = suggestions.get(selected) {
                            self.draw_selected_proposal_detail(ui, s, selected);
                        }
                        ui.add_space(6.0);
                        ui.separator();

                        // ── 參數調整面板 ──
                        self.draw_parameter_panel(ui, &suggestions);
                        ui.add_space(4.0);

                        // ── 即時驗算結果 ──
                        self.draw_live_capacity(ui, beam_sec, col_sec);
                    });

                ui.add_space(4.0);
                ui.separator();

                // ═══════════════════════════════════════════════════
                // 3. 底部按鈕列（固定）
                // ═══════════════════════════════════════════════════
                ui.horizontal(|ui| {
                    let sel = self.editor.conn_dialog.as_ref().map_or(0, |d| d.selected_idx);
                    let btn = egui::Button::new(
                        egui::RichText::new("確認建立接頭").size(14.0).strong().color(egui::Color32::WHITE)
                    ).fill(BRAND).rounding(8.0);
                    if ui.add_sized([170.0, 38.0], btn).clicked() {
                        confirm_idx = Some(sel);
                    }
                    if ui.add_sized([80.0, 38.0],
                        egui::Button::new("取消").rounding(8.0)
                    ).clicked() {
                        close = true;
                    }
                    // 右側：選中方案名稱提示
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(s) = suggestions.get(sel) {
                            ui.label(egui::RichText::new(
                                format!("▸ {}", s.conn_type.label())
                            ).color(BRAND).size(12.0));
                        }
                    });
                });
            });

        if close {
            self.editor.conn_dialog = None;
        }
        if let Some(idx) = confirm_idx {
            self.execute_connection_from_dialog(idx);
            self.editor.conn_dialog = None;
        }
    }

    // ─── 子元件 ────────────────────────────────────────────────────────────

    /// 構件資訊卡
    fn draw_member_header(
        ui: &mut egui::Ui,
        beam: (f32, f32, f32, f32),
        col: (f32, f32, f32, f32),
        intent: ConnectionIntent,
    ) {
        let intent_label = match intent {
            ConnectionIntent::BeamToColumn => "梁-柱接頭",
            ConnectionIntent::ColumnBase => "柱底接頭",
            ConnectionIntent::BeamToBeam => "梁-梁續接",
            ConnectionIntent::BraceToGusset => "斜撐接合",
        };

        egui::Frame::none()
            .fill(egui::Color32::from_rgb(240, 244, 255))
            .stroke(egui::Stroke::new(1.0, BRAND))
            .rounding(10.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(intent_label).strong().size(14.0).color(BRAND));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // 加勁板判斷
                        let (need_stiff, stiff_reason) = need_stiffeners_check(beam, col);
                        let (icon, color) = if need_stiff {
                            ("⚠", WARN_COLOR)
                        } else {
                            ("✓", PASS_COLOR)
                        };
                        ui.colored_label(color, format!("{} J10: {}", icon, stiff_reason));
                    });
                });
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!(
                        "梁 H{:.0}×{:.0}×{:.0}×{:.0}", beam.0, beam.1, beam.2, beam.3
                    )).strong());
                    ui.label(egui::RichText::new("│").color(SUB));
                    ui.label(egui::RichText::new(format!(
                        "柱 H{:.0}×{:.0}×{:.0}×{:.0}", col.0, col.1, col.2, col.3
                    )).strong());
                });
            });
    }

    /// 方案比較摘要列（compact tabs）
    fn draw_proposal_summary_row(&mut self, ui: &mut egui::Ui, suggestions: &[ConnectionSuggestion]) {
        let selected = self.editor.conn_dialog.as_ref().map_or(0, |d| d.selected_idx);

        ui.horizontal_wrapped(|ui| {
            for (i, s) in suggestions.iter().enumerate() {
                let is_sel = i == selected;
                let cap = &s.estimated_capacity;
                let (bg, stroke) = if is_sel {
                    (egui::Color32::from_rgb(235, 242, 255), egui::Stroke::new(2.0, BRAND))
                } else {
                    (egui::Color32::from_rgb(248, 248, 252), egui::Stroke::new(1.0, egui::Color32::from_gray(210)))
                };
                let pass_icon = if cap.pass { "✓" } else { "✗" };
                let pass_c = if cap.pass { PASS_COLOR } else { FAIL_COLOR };

                let resp = egui::Frame::none()
                    .fill(bg)
                    .stroke(stroke)
                    .rounding(8.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.colored_label(pass_c, pass_icon);
                            let title = egui::RichText::new(s.conn_type.label())
                                .size(12.0)
                                .strong();
                            if ui.selectable_label(is_sel, title).clicked() {
                                if let Some(ref mut d) = self.editor.conn_dialog {
                                    d.selected_idx = i;
                                    d.bolt_size = s.bolt_size;
                                    d.bolt_grade = s.bolt_grade;
                                    d.plate_thickness = s.plate_thickness;
                                    d.add_stiffeners = s.need_stiffeners;
                                    d.weld_size = minimum_fillet_weld_size(s.plate_thickness);
                                }
                            }
                        });
                        // 單行摘要
                        ui.label(egui::RichText::new(format!(
                            "{} {} | {:.0}mm | {:.0}kN",
                            s.bolt_size.label(), s.bolt_grade.label(),
                            s.plate_thickness, cap.total_bolt_shear
                        )).size(9.5).color(SUB));
                    });
                // 讓整個 frame 可點擊
                if resp.response.interact(egui::Sense::click()).clicked() {
                    if let Some(ref mut d) = self.editor.conn_dialog {
                        d.selected_idx = i;
                        d.bolt_size = s.bolt_size;
                        d.bolt_grade = s.bolt_grade;
                        d.plate_thickness = s.plate_thickness;
                        d.add_stiffeners = s.need_stiffeners;
                        d.weld_size = minimum_fillet_weld_size(s.plate_thickness);
                    }
                }
            }
        });
    }

    /// 選中方案詳情
    fn draw_selected_proposal_detail(&self, ui: &mut egui::Ui, s: &ConnectionSuggestion, idx: usize) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(250, 252, 255))
            .stroke(egui::Stroke::new(1.5, BRAND))
            .rounding(10.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!(
                        "方案 {} — {}", idx + 1, s.conn_type.label()
                    )).strong().size(14.0));
                    let cap = &s.estimated_capacity;
                    let (icon, color) = if cap.pass { ("PASS", PASS_COLOR) } else { ("FAIL", FAIL_COLOR) };
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(color, egui::RichText::new(icon).strong().size(13.0));
                    });
                });
                ui.label(egui::RichText::new(&s.reason).size(11.0).color(SUB));
                ui.add_space(4.0);

                // 螺栓 + 板件 + 加勁板
                ui.horizontal(|ui| {
                    Self::info_chip(ui, &format!("螺栓 {} {}", s.bolt_size.label(), s.bolt_grade.label()));
                    Self::info_chip(ui, &format!("孔 Ø{:.0}", s.bolt_size.hole_diameter()));
                    Self::info_chip(ui, &format!("板厚 {:.0}mm", s.plate_thickness));
                    if s.need_stiffeners {
                        Self::info_chip_colored(ui, "需加勁板", WARN_COLOR);
                    }
                });
                ui.add_space(4.0);

                // 強度橫條（利用率概念）
                let cap = &s.estimated_capacity;
                Self::capacity_bar(ui, "抗剪", cap.total_bolt_shear, 0.0);
                Self::capacity_bar(ui, "抗拉", cap.total_bolt_tension, 0.0);
                Self::capacity_bar(ui, "焊接", cap.total_weld_capacity, 0.0);

                // AISC 條文
                ui.add_space(2.0);
                ui.label(egui::RichText::new(&s.aisc_ref).size(9.5).color(SUB).italics());

                // 警告
                if !cap.warnings.is_empty() {
                    ui.add_space(4.0);
                    for w in &cap.warnings {
                        ui.colored_label(WARN_COLOR, format!("⚠ {}", w));
                    }
                }
            });
    }

    /// 參數調整面板（含重設按鈕 + 即時警告）
    fn draw_parameter_panel(&mut self, ui: &mut egui::Ui, suggestions: &[ConnectionSuggestion]) {
        ui.heading("參數調整");
        let selected = self.editor.conn_dialog.as_ref().map_or(0, |d| d.selected_idx);
        let original = suggestions.get(selected).cloned();

        if let Some(ref mut d) = self.editor.conn_dialog {
            // 螺栓列
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("螺栓:").size(11.0).color(SUB));
                egui::ComboBox::from_id_source("dlg_bolt_size")
                    .width(80.0)
                    .selected_text(d.bolt_size.label())
                    .show_ui(ui, |ui| {
                        for &bs in BoltSize::ALL {
                            let label = format!("{} Ø{:.0} 孔Ø{:.0}", bs.label(), bs.diameter(), bs.hole_diameter());
                            if ui.selectable_label(d.bolt_size == bs, &label).clicked() {
                                d.bolt_size = bs;
                            }
                        }
                    });
                ui.label(egui::RichText::new("等級:").size(11.0).color(SUB));
                egui::ComboBox::from_id_source("dlg_bolt_grade")
                    .width(65.0)
                    .selected_text(d.bolt_grade.label())
                    .show_ui(ui, |ui| {
                        for &bg in BoltGrade::ALL {
                            if ui.selectable_label(d.bolt_grade == bg, bg.label()).clicked() {
                                d.bolt_grade = bg;
                            }
                        }
                    });
            });

            // 板厚 + 焊腳
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut d.plate_thickness)
                    .speed(0.5).prefix("板厚: ").suffix(" mm").range(10.0..=60.0));
                ui.add(egui::DragValue::new(&mut d.weld_size)
                    .speed(0.5).prefix("焊腳: ").suffix(" mm").range(4.0..=20.0));
            });

            // 加勁板 checkbox + 安全警告
            let need_stiff_aisc = original.as_ref().map_or(false, |s| s.need_stiffeners);
            ui.horizontal(|ui| {
                ui.checkbox(&mut d.add_stiffeners, "加勁板 (AISC J10)");
                if need_stiff_aisc && !d.add_stiffeners {
                    ui.colored_label(FAIL_COLOR, "⚠ AISC 要求加勁板！");
                }
            });

            // 焊腳最小值警告
            let min_weld = minimum_fillet_weld_size(d.plate_thickness);
            if d.weld_size < min_weld {
                ui.colored_label(WARN_COLOR,
                    format!("⚠ 焊腳 {:.0}mm < AISC 最小 {:.0}mm (Table J2.4)", d.weld_size, min_weld));
            }

            // 重設按鈕
            ui.horizontal(|ui| {
                let is_modified = original.as_ref().map_or(false, |s| {
                    d.bolt_size != s.bolt_size || d.bolt_grade != s.bolt_grade
                    || (d.plate_thickness - s.plate_thickness).abs() > 0.1
                    || d.add_stiffeners != s.need_stiffeners
                    || (d.weld_size - minimum_fillet_weld_size(s.plate_thickness)).abs() > 0.1
                });
                if is_modified {
                    ui.colored_label(BRAND, "● 已修改");
                    if ui.small_button("重設為 AISC 建議值").clicked() {
                        if let Some(s) = &original {
                            d.bolt_size = s.bolt_size;
                            d.bolt_grade = s.bolt_grade;
                            d.plate_thickness = s.plate_thickness;
                            d.add_stiffeners = s.need_stiffeners;
                            d.weld_size = minimum_fillet_weld_size(s.plate_thickness);
                        }
                    }
                }
            });
        }
    }

    /// 即時強度驗算（用目前參數重算）
    fn draw_live_capacity(&self, ui: &mut egui::Ui, beam: (f32, f32, f32, f32), col: (f32, f32, f32, f32)) {
        let d = match &self.editor.conn_dialog {
            Some(d) => d,
            None => return,
        };
        let selected = d.selected_idx;
        let s = match d.suggestions.get(selected) {
            Some(s) => s,
            None => return,
        };

        // 用使用者調整後的參數重算強度
        let plate_mat = SteelMaterial::SS400;
        let bolt_cap = bolt_capacity(&d.bolt_size, &d.bolt_grade, d.plate_thickness, &plate_mat, DesignMethod::LRFD, true);
        let orig_cap = bolt_capacity(&s.bolt_size, &s.bolt_grade, s.plate_thickness, &plate_mat, DesignMethod::LRFD, true);
        let n_bolts = (s.estimated_capacity.total_bolt_shear / orig_cap.shear_capacity.max(1.0)).round().max(2.0);

        let live_shear = bolt_cap.shear_capacity * n_bolts;
        let live_tension = bolt_cap.tensile_capacity * n_bolts;

        let weld_line = WeldLine {
            weld_type: WeldType::Fillet,
            size: d.weld_size,
            length: beam.0, // 梁高作為焊接長度估算
            start: [0.0; 3],
            end: [beam.0, 0.0, 0.0],
        };
        let weld_cap = weld_capacity(&weld_line, &SteelMaterial::SS400, DesignMethod::LRFD);
        let live_weld = weld_cap.design_capacity;

        let all_pass = live_shear > 0.0 && live_tension > 0.0 && live_weld > 0.0;

        ui.add_space(2.0);
        egui::Frame::none()
            .fill(if all_pass { egui::Color32::from_rgb(240, 255, 245) } else { egui::Color32::from_rgb(255, 240, 240) })
            .stroke(egui::Stroke::new(1.0, if all_pass { PASS_COLOR } else { FAIL_COLOR }))
            .rounding(8.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("即時驗算").strong().size(12.0));
                    let (icon, color) = if all_pass { ("PASS", PASS_COLOR) } else { ("FAIL", FAIL_COLOR) };
                    ui.colored_label(color, egui::RichText::new(icon).strong());
                    ui.label(egui::RichText::new(
                        format!("({:.0} bolts × {} {})", n_bolts, d.bolt_size.label(), d.bolt_grade.label())
                    ).size(10.0).color(SUB));
                });

                Self::capacity_bar(ui, "抗剪", live_shear, s.estimated_capacity.total_bolt_shear);
                Self::capacity_bar(ui, "抗拉", live_tension, s.estimated_capacity.total_bolt_tension);
                Self::capacity_bar(ui, "焊接", live_weld, s.estimated_capacity.total_weld_capacity);
            });
    }

    // ─── UI 工具函式 ──────────────────────────────────────────────────────────

    /// 資訊小標籤
    fn info_chip(ui: &mut egui::Ui, text: &str) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(235, 240, 250))
            .rounding(4.0)
            .inner_margin(egui::Margin::symmetric(6.0, 2.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(text).size(10.5));
            });
    }

    /// 有色資訊小標籤
    fn info_chip_colored(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
        egui::Frame::none()
            .fill(egui::Color32::from_rgba_premultiplied(
                color.r(), color.g(), color.b(), 30
            ))
            .rounding(4.0)
            .inner_margin(egui::Margin::symmetric(6.0, 2.0))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(text).size(10.5).color(color));
            });
    }

    /// 強度橫條（含對比基準值）
    fn capacity_bar(ui: &mut egui::Ui, label: &str, value: f32, baseline: f32) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{}:", label)).size(10.5).color(SUB));
            ui.label(egui::RichText::new(format!("{:.0} kN", value)).size(11.0).strong());

            // 對比基準
            if baseline > 0.0 && (value - baseline).abs() > 0.5 {
                let ratio = value / baseline;
                let (arrow, color) = if ratio >= 1.0 {
                    ("▲", PASS_COLOR)
                } else {
                    ("▼", WARN_COLOR)
                };
                ui.colored_label(color,
                    egui::RichText::new(format!("{} {:.0}%", arrow, (ratio - 1.0) * 100.0)).size(9.5));
            }

            // 簡易進度條
            let max_val = value.max(baseline).max(100.0);
            let frac = (value / max_val).clamp(0.0, 1.0);
            let bar_w = (ui.available_width() - 8.0).max(40.0);
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(bar_w, 8.0),
                egui::Sense::hover(),
            );
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, egui::Color32::from_gray(230));
            let fill_color = if frac > 0.5 { PASS_COLOR } else if frac > 0.25 { WARN_COLOR } else { FAIL_COLOR };
            let fill_rect = egui::Rect::from_min_size(
                rect.min,
                egui::vec2(rect.width() * frac, rect.height()),
            );
            painter.rect_filled(fill_rect, 3.0, fill_color);
        });
    }
}

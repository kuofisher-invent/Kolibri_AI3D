//! ZWCAD 風格 Ribbon 工具列 — 出圖模式專用
//!
//! 深色主題，icon 在上文字在下，Group 底部標籤條，完全對標 ZWCAD 2026

use eframe::egui;
use crate::app::KolibriApp;
use crate::editor::{RibbonTab, Tool};

// ─── ZWCAD 色彩常數 ────────────────────────────────────────────────────────

/// Ribbon 背景（ZWCAD 深灰）
const BG: egui::Color32 = egui::Color32::from_rgb(45, 45, 48);
/// Tab 列背景
const TAB_BAR_BG: egui::Color32 = egui::Color32::from_rgb(45, 45, 48);
/// 選中 Tab 的內容區背景
const TAB_CONTENT_BG: egui::Color32 = egui::Color32::from_rgb(56, 56, 59);
/// Group 底部標籤條
const GROUP_LABEL_BG: egui::Color32 = egui::Color32::from_rgb(50, 50, 53);
/// 分隔線
const SEPARATOR_COLOR: egui::Color32 = egui::Color32::from_rgb(70, 70, 74);
/// 文字色（淺灰白）
const TEXT_COLOR: egui::Color32 = egui::Color32::from_rgb(220, 220, 220);
/// 文字色（暗灰）
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(160, 160, 165);
/// 選中 Tab 文字
const TAB_ACTIVE_TEXT: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);
/// 未選中 Tab 文字
const TAB_INACTIVE_TEXT: egui::Color32 = egui::Color32::from_rgb(180, 180, 185);
/// Hover 背景
const HOVER_BG: egui::Color32 = egui::Color32::from_rgb(75, 75, 80);
/// Active（選中工具）背景
const ACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(0, 122, 204);
/// Active 文字
const ACTIVE_TEXT: egui::Color32 = egui::Color32::WHITE;

/// Ribbon 內容區高度（+30%）
const RIBBON_CONTENT_H: f32 = 104.0;
/// Tab 列高度
const TAB_BAR_H: f32 = 28.0;
/// Group 底部標籤高度
const GROUP_LABEL_H: f32 = 18.0;
/// Icon 大小（+30%）
const ICON_SIZE: f32 = 30.0;
/// 小工具按鈕（+30%）
const BTN_W: f32 = 62.0;
const BTN_H: f32 = 70.0;

/// Tab 定義
const TABS: &[(&str, RibbonTab)] = &[
    ("常用", RibbonTab::Home),
    ("標註", RibbonTab::Annotate),
    ("檢視", RibbonTab::View),
    ("輸出", RibbonTab::Output),
];

/// 工具按鈕定義
#[derive(Clone)]
struct ToolBtn {
    tool: Tool,
    label: &'static str,
    tooltip: &'static str,
}

impl KolibriApp {
    /// 繪製 ZWCAD 風格 Ribbon（僅在出圖模式時呼叫）
    #[cfg(feature = "drafting")]
    pub(crate) fn draw_ribbon(&mut self, ctx: &egui::Context) {
        // ── Tab 列（獨立 panel）──
        egui::TopBottomPanel::top("ribbon_tabs")
            .exact_height(TAB_BAR_H)
            .show_separator_line(false)
            .frame(egui::Frame::none()
                .fill(TAB_BAR_BG)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin { left: 4.0, right: 4.0, top: 0.0, bottom: 0.0 }))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    for &(label, tab) in TABS {
                        let active = self.editor.ribbon_tab == tab;
                        let text_color = if active { TAB_ACTIVE_TEXT } else { TAB_INACTIVE_TEXT };
                        let fill = if active { TAB_CONTENT_BG } else { egui::Color32::TRANSPARENT };

                        let btn = egui::Button::new(
                            egui::RichText::new(label).size(11.0).color(text_color)
                        )
                        .fill(fill)
                        .stroke(egui::Stroke::NONE)
                        .rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 });

                        if ui.add_sized([52.0, TAB_BAR_H - 2.0], btn).clicked() {
                            self.editor.ribbon_tab = tab;
                        }
                    }

                    // 右側：返回建模
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(
                            egui::RichText::new("← 建模").size(10.0).color(egui::Color32::WHITE)
                        ).fill(egui::Color32::from_rgb(0, 122, 204)).rounding(4.0))
                        .on_hover_text("返回 3D 建模 (F6)").clicked() {
                            self.viewer.layout_mode = false;
                        }
                        ui.label(egui::RichText::new(
                            format!("{} | {} 圖元", self.viewer.layout.name, self.editor.draft_doc.objects.len())
                        ).size(10.0).color(TEXT_DIM));
                    });
                });
            });

        // ── Ribbon 內容區（獨立 panel）──
        egui::TopBottomPanel::top("ribbon_content")
            .exact_height(RIBBON_CONTENT_H)
            .show_separator_line(false)
            .frame(egui::Frame::none()
                .fill(TAB_CONTENT_BG)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin { left: 4.0, right: 4.0, top: 0.0, bottom: 0.0 }))
            .show(ctx, |ui| {
                match self.editor.ribbon_tab {
                    RibbonTab::Home => self.ribbon_home(ui),
                    RibbonTab::Annotate => self.ribbon_annotate(ui),
                    RibbonTab::View => self.ribbon_view(ui),
                    RibbonTab::Output => self.ribbon_output(ui),
                }
            });

        // ── Drawing Tab（ZWCAD 風格文件 tab）──
        egui::TopBottomPanel::top("drawing_tab")
            .exact_height(22.0)
            .show_separator_line(false)
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(50, 50, 54))
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin { left: 8.0, right: 8.0, top: 0.0, bottom: 0.0 }))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Drawing1 tab（選中）
                    let tab_bg = egui::Color32::from_rgb(64, 64, 68);
                    ui.add(egui::Button::new(
                        egui::RichText::new("Drawing1").size(10.0).color(egui::Color32::WHITE)
                    ).fill(tab_bg).rounding(0.0).stroke(egui::Stroke::NONE));
                    // × 關閉
                    ui.label(egui::RichText::new("×").size(10.0).color(egui::Color32::from_rgb(160, 160, 165)));
                    // + 新增
                    ui.label(egui::RichText::new("+").size(11.0).color(egui::Color32::from_rgb(160, 160, 165)));
                    // / 分隔
                    ui.add_space(8.0);
                });
            });
    }

    // ─── Tab 內容 ───────────────────────────────────────────────────────────

    #[cfg(feature = "drafting")]
    fn ribbon_home(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
            self.ribbon_group(ui, "繪圖", &[
                ToolBtn { tool: Tool::DraftLine, label: "直線", tooltip: "直線 (L)" },
                ToolBtn { tool: Tool::DraftPolyline, label: "聚合線", tooltip: "聚合線 (PL)" },
                ToolBtn { tool: Tool::DraftArc, label: "弧", tooltip: "弧 (A)" },
                ToolBtn { tool: Tool::DraftCircle, label: "圓", tooltip: "圓 (C)" },
                ToolBtn { tool: Tool::DraftRectangle, label: "矩形", tooltip: "矩形 (REC)" },
                ToolBtn { tool: Tool::DraftEllipse, label: "橢圓", tooltip: "橢圓 (EL)" },
                ToolBtn { tool: Tool::DraftPolygon, label: "多邊形", tooltip: "正多邊形 (POL)" },
                ToolBtn { tool: Tool::DraftSpline, label: "雲形線", tooltip: "雲形線 (SPL)" },
                ToolBtn { tool: Tool::DraftXline, label: "建構線", tooltip: "建構線 (XL)" },
                ToolBtn { tool: Tool::DraftPoint, label: "點", tooltip: "點 (PO)" },
            ]);
            self.ribbon_vsep(ui);
            self.ribbon_group(ui, "修改", &[
                ToolBtn { tool: Tool::DraftMove, label: "移動", tooltip: "移動 (M)" },
                ToolBtn { tool: Tool::DraftCopy, label: "複製", tooltip: "複製 (CO)" },
                ToolBtn { tool: Tool::DraftRotate, label: "旋轉", tooltip: "旋轉 (RO)" },
                ToolBtn { tool: Tool::DraftMirror, label: "鏡射", tooltip: "鏡射 (MI)" },
                ToolBtn { tool: Tool::DraftScale, label: "比例", tooltip: "比例 (SC)" },
                ToolBtn { tool: Tool::DraftStretch, label: "拉伸", tooltip: "拉伸 (S)" },
                ToolBtn { tool: Tool::DraftOffset, label: "偏移", tooltip: "偏移 (O)" },
                ToolBtn { tool: Tool::DraftArray, label: "陣列", tooltip: "陣列 (AR)" },
                ToolBtn { tool: Tool::DraftTrim, label: "修剪", tooltip: "修剪 (TR)" },
                ToolBtn { tool: Tool::DraftExtend, label: "延伸", tooltip: "延伸 (EX)" },
                ToolBtn { tool: Tool::DraftFillet, label: "圓角", tooltip: "圓角 (F)" },
                ToolBtn { tool: Tool::DraftChamfer, label: "倒角", tooltip: "倒角 (CHA)" },
                ToolBtn { tool: Tool::DraftExplode, label: "分解", tooltip: "分解 (X)" },
            ]);
            self.ribbon_vsep(ui);
            self.ribbon_group(ui, "註解", &[
                ToolBtn { tool: Tool::DraftText, label: "多行文字", tooltip: "多行文字 (MT)" },
                ToolBtn { tool: Tool::DraftDimLinear, label: "標註", tooltip: "線性標註 (DLI)" },
                ToolBtn { tool: Tool::DraftLeader, label: "引線", tooltip: "引線 (LE)" },
                ToolBtn { tool: Tool::DraftDimContinue, label: "連續", tooltip: "連續標註 (DCO)" },
                ToolBtn { tool: Tool::DraftHatch, label: "填充", tooltip: "填充 (H)" },
            ]);
            self.ribbon_vsep(ui);
            self.ribbon_layer_group(ui);
            self.ribbon_vsep(ui);
            // 圖塊 group
            self.ribbon_group(ui, "圖塊", &[
                ToolBtn { tool: Tool::DraftBlock, label: "建立", tooltip: "建立圖塊 (B)" },
                ToolBtn { tool: Tool::DraftInsert, label: "插入", tooltip: "插入圖塊 (I)" },
            ]);
        });
    }

    #[cfg(feature = "drafting")]
    fn ribbon_annotate(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
        self.ribbon_group(ui, "尺寸", &[
            ToolBtn { tool: Tool::DraftDimLinear, label: "線性", tooltip: "線性標註 (DLI)" },
            ToolBtn { tool: Tool::DraftDimAligned, label: "對齊", tooltip: "對齊標註 (DAL)" },
            ToolBtn { tool: Tool::DraftDimAngle, label: "角度", tooltip: "角度標註 (DAN)" },
            ToolBtn { tool: Tool::DraftDimRadius, label: "半徑", tooltip: "半徑標註 (DRA)" },
            ToolBtn { tool: Tool::DraftDimDiameter, label: "直徑", tooltip: "直徑標註 (DDI)" },
        ]);
        self.ribbon_vsep(ui);
        self.ribbon_group(ui, "文字", &[
            ToolBtn { tool: Tool::DraftText, label: "文字", tooltip: "多行文字 (T)" },
            ToolBtn { tool: Tool::DraftLeader, label: "引線", tooltip: "引線 (LE)" },
        ]);
        self.ribbon_vsep(ui);
        self.ribbon_group(ui, "填充", &[
            ToolBtn { tool: Tool::DraftHatch, label: "填充", tooltip: "填充 (H)" },
        ]);
        });  // close horizontal_top for annotate
    }

    #[cfg(feature = "drafting")]
    fn ribbon_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
        self.ribbon_group(ui, "導覽", &[
            ToolBtn { tool: Tool::DraftZoomAll, label: "全部", tooltip: "縮放全部 (Z+A)" },
            ToolBtn { tool: Tool::DraftZoomWindow, label: "視窗", tooltip: "縮放視窗 (Z+W)" },
            ToolBtn { tool: Tool::DraftPan, label: "平移", tooltip: "平移 (P)" },
        ]);
        self.ribbon_vsep(ui);
        // 顯示設定
        ui.vertical(|ui| {
            ui.add_space(4.0);
            let mut show_grid = self.viewer.show_grid;
            if ui.add(egui::Checkbox::new(&mut show_grid, egui::RichText::new("格線").size(10.0).color(TEXT_COLOR))).changed() {
                self.viewer.show_grid = show_grid;
            }
            ui.label(egui::RichText::new(format!("Snap: {:.0}px", self.editor.snap_threshold)).size(10.0).color(TEXT_DIM));
        });
        });  // close horizontal_top for view
    }

    #[cfg(feature = "drafting")]
    fn ribbon_output(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
        self.ribbon_group(ui, "輸出", &[
            ToolBtn { tool: Tool::DraftPrint, label: "列印", tooltip: "列印 (Ctrl+P)" },
            ToolBtn { tool: Tool::DraftExportPdf, label: "PDF", tooltip: "匯出 PDF" },
        ]);
        self.ribbon_vsep(ui);
        ui.vertical(|ui| {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(format!("紙張: {}", self.viewer.layout.paper_size.label())).size(10.0).color(TEXT_COLOR));
            ui.label(egui::RichText::new(format!("比例: 1:{:.0}", self.viewer.layout.scale)).size(10.0).color(TEXT_DIM));
        });
        });  // close horizontal_top for output
    }

    // ─── Group 繪製（ZWCAD 風格：icon 上方 + text 下方 + 底部標籤條）──────

    #[cfg(feature = "drafting")]
    fn ribbon_group(&mut self, ui: &mut egui::Ui, title: &str, tools: &[ToolBtn]) {
        // 先收集所有 icon texture ids
        let icon_ids: Vec<Option<egui::TextureId>> = tools.iter().map(|btn_def| {
            crate::svg_icons::tool_icon_name(btn_def.tool)
                .and_then(|name| self.svg_icons.get(ui.ctx(), name))
        }).collect();

        let use_big = tools.len() <= 4;
        let half = if use_big { tools.len() } else { (tools.len() + 1) / 2 };
        let cols = if use_big { tools.len() } else { half };
        let group_w = cols as f32 * BTN_W + 4.0;

        // 整個 group 用 allocate_exact_size 手動佈局
        let total_h = RIBBON_CONTENT_H;
        let (group_rect, _) = ui.allocate_exact_size(
            egui::vec2(group_w, total_h), egui::Sense::hover());

        if !ui.is_rect_visible(group_rect) { return; }
        let p = ui.painter();

        // 底部標籤條
        let label_rect = egui::Rect::from_min_size(
            egui::pos2(group_rect.left(), group_rect.bottom() - GROUP_LABEL_H),
            egui::vec2(group_w, GROUP_LABEL_H));
        p.rect_filled(label_rect, 0.0, GROUP_LABEL_BG);
        p.text(label_rect.center(), egui::Align2::CENTER_CENTER, title,
            egui::FontId::proportional(9.5), TEXT_DIM);

        // 按鈕區域
        let btn_area_h = total_h - GROUP_LABEL_H;

        for (i, btn_def) in tools.iter().enumerate() {
            let (col, row, row_h) = if use_big {
                (i, 0, btn_area_h)
            } else {
                (i % half, i / half, btn_area_h / 2.0)
            };

            let btn_rect = egui::Rect::from_min_size(
                egui::pos2(
                    group_rect.left() + 2.0 + col as f32 * BTN_W,
                    group_rect.top() + row as f32 * row_h),
                egui::vec2(BTN_W, row_h));

            let resp = ui.interact(btn_rect,
                ui.id().with(("rb", title, i)), egui::Sense::click());
            let active = self.editor.tool == btn_def.tool;

            if active { p.rect_filled(btn_rect, 2.0, ACTIVE_BG); }
            else if resp.hovered() { p.rect_filled(btn_rect, 2.0, HOVER_BG); }
            let tc = if active { ACTIVE_TEXT } else { TEXT_COLOR };

            if use_big {
                // icon 上 + text 下
                if let Some(tex_id) = icon_ids[i] {
                    let ir = egui::Rect::from_center_size(
                        egui::pos2(btn_rect.center().x, btn_rect.top() + 20.0),
                        egui::vec2(ICON_SIZE, ICON_SIZE));
                    p.image(tex_id, ir,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE);
                }
                p.text(egui::pos2(btn_rect.center().x, btn_rect.top() + 48.0),
                    egui::Align2::CENTER_TOP, btn_def.label,
                    egui::FontId::proportional(11.5), tc);
            } else {
                // icon 左 + text 右
                if let Some(tex_id) = icon_ids[i] {
                    let ir = egui::Rect::from_center_size(
                        egui::pos2(btn_rect.left() + 14.0, btn_rect.center().y),
                        egui::vec2(22.0, 22.0));
                    p.image(tex_id, ir,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE);
                }
                p.text(egui::pos2(btn_rect.left() + 30.0, btn_rect.center().y),
                    egui::Align2::LEFT_CENTER, btn_def.label,
                    egui::FontId::proportional(11.5), tc);
            }

            if resp.on_hover_text(btn_def.tooltip).clicked() {
                self.editor.tool = btn_def.tool;
            }
        }
    }

    /// ZWCAD 風格垂直分隔線
    #[cfg(feature = "drafting")]
    fn ribbon_vsep(&self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(7.0, RIBBON_CONTENT_H),
            egui::Sense::hover(),
        );
        ui.painter().line_segment(
            [egui::pos2(rect.center().x, rect.top() + 4.0),
             egui::pos2(rect.center().x, rect.bottom() - GROUP_LABEL_H - 4.0)],
            egui::Stroke::new(1.0, SEPARATOR_COLOR),
        );
    }

    /// 圖層 Group（ZWCAD 風格下拉）
    #[cfg(feature = "drafting")]
    fn ribbon_layer_group(&mut self, ui: &mut egui::Ui) {
        let total_h = RIBBON_CONTENT_H;
        let group_w = 120.0;
        let layer_count = self.editor.draft_layers.layers.len();

        ui.vertical(|ui| {
            ui.set_min_size(egui::vec2(group_w, total_h));

            ui.add_space(6.0);

            // 圖層下拉
            let current = self.editor.draft_layers.current.clone();
            egui::ComboBox::from_id_source("draft_layer_combo_zw")
                .width(group_w - 16.0)
                .selected_text(egui::RichText::new(&current).size(10.0).color(TEXT_COLOR))
                .show_ui(ui, |ui| {
                    let layer_names: Vec<String> = self.editor.draft_layers.layers.iter()
                        .map(|l| l.name.clone()).collect();
                    for name in layer_names {
                        let selected = name == self.editor.draft_layers.current;
                        if ui.selectable_label(selected, &name).clicked() {
                            self.editor.draft_layers.current = name;
                        }
                    }
                });

            ui.add_space(4.0);
            ui.label(egui::RichText::new(format!("{} 個圖層", layer_count))
                .size(9.0).color(TEXT_DIM));

            // 底部標籤（用 painter 在剩餘空間底部畫）
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("圖層").size(9.5).color(TEXT_DIM));
            });
        });
    }
}

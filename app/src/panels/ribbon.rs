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

/// Ribbon 內容區高度
const RIBBON_CONTENT_H: f32 = 120.0;
/// Tab 列高度
const TAB_BAR_H: f32 = 30.0;
/// Group 底部標籤高度
const GROUP_LABEL_H: f32 = 20.0;
/// Icon 大小（大按鈕）
const ICON_SIZE: f32 = 36.0;
/// 小 icon 大小
const ICON_SM: f32 = 18.0;
/// 大按鈕寬度
const BIG_BTN_W: f32 = 58.0;
/// 小按鈕寬度
const SMALL_BTN_W: f32 = 72.0;
/// 字體大小
const FONT_TAB: f32 = 15.0;  // Tab 列
const FONT_BIG: f32 = 15.0;  // 大按鈕（跟 Tab 一樣大）
const FONT_SM: f32 = 13.0;   // 小按鈕
const FONT_LABEL: f32 = 13.0; // Group 底部標籤

/// Tab 定義
const TABS: &[(&str, RibbonTab)] = &[
    ("常用", RibbonTab::Home),
    ("插入", RibbonTab::Insert),
    ("標註", RibbonTab::Annotate),
    ("檢視", RibbonTab::View),
    ("管理", RibbonTab::Manage),
    ("輸出", RibbonTab::Output),
];

/// 工具按鈕定義
#[derive(Clone)]
struct ToolBtn {
    tool: Tool,
    label: &'static str,
    tooltip: &'static str,
}

/// 對 egui Context 全域套用深色 visuals（含 popup/dropdown）
fn apply_dark_visuals_to_ctx(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    apply_dark_visuals_inner(&mut style.visuals);
    ctx.set_style(style);
}

/// 恢復淺色 visuals（3D 模式用）
fn restore_light_visuals_to_ctx(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    ctx.set_style(style);
}

/// 深色 visuals 內部設定（共用）
fn apply_dark_visuals_inner(vis: &mut egui::Visuals) {
    vis.override_text_color = Some(egui::Color32::from_rgb(220, 220, 225));
    vis.dark_mode = true;
    // Widget 背景
    let dark_bg = egui::Color32::from_rgb(55, 58, 65);
    let dark_bg_hover = egui::Color32::from_rgb(70, 74, 82);
    let dark_bg_active = egui::Color32::from_rgb(45, 48, 55);
    let border = egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 84, 92));
    // Inactive
    vis.widgets.inactive.bg_fill = dark_bg;
    vis.widgets.inactive.bg_stroke = border;
    vis.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 210));
    vis.widgets.inactive.weak_bg_fill = dark_bg;
    // Hovered
    vis.widgets.hovered.bg_fill = dark_bg_hover;
    vis.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 160, 240));
    vis.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    vis.widgets.hovered.weak_bg_fill = dark_bg_hover;
    // Active
    vis.widgets.active.bg_fill = dark_bg_active;
    vis.widgets.active.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 139, 245));
    vis.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    vis.widgets.active.weak_bg_fill = dark_bg_active;
    // Open
    vis.widgets.open.bg_fill = dark_bg;
    vis.widgets.open.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 139, 245));
    vis.widgets.open.weak_bg_fill = dark_bg;
    // Noninteractive (labels, separators)
    vis.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(40, 43, 50);
    vis.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5, egui::Color32::from_rgb(65, 68, 75));
    vis.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(180, 180, 190));
    // Selection
    vis.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(76, 139, 245, 80);
    vis.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 139, 245));
    // Window/popup
    vis.window_fill = egui::Color32::from_rgb(45, 48, 55);
    vis.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 74, 82));
    vis.panel_fill = egui::Color32::from_rgb(40, 43, 50);
    vis.extreme_bg_color = egui::Color32::from_rgb(35, 38, 44);
    vis.faint_bg_color = egui::Color32::from_rgb(50, 53, 60);
}

/// 深色 widget 樣式覆寫（對 UI 局部）
fn apply_dark_widget_style(ui: &mut egui::Ui) {
    apply_dark_visuals_inner(&mut ui.style_mut().visuals);
}

impl KolibriApp {
    /// 繪製 ZWCAD 風格 Ribbon（僅在出圖模式時呼叫）
    #[cfg(feature = "drafting")]
    pub(crate) fn draw_ribbon(&mut self, ctx: &egui::Context) {
        // ── 全域深色主題（影響 popup/dropdown 等脫離 parent 的元件）──
        apply_dark_visuals_to_ctx(ctx);

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
                            egui::RichText::new(label).size(15.0).color(text_color)
                        )
                        .fill(fill)
                        .stroke(egui::Stroke::NONE)
                        .rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 });

                        if ui.add_sized([52.0, TAB_BAR_H - 2.0], btn).clicked() {
                            self.editor.ribbon_tab = tab;
                        }
                    }

                    // 右側：圖元計數（3D 切換已在 topbar [3D][2D] toggle）
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(
                            format!("{} | {} 圖元", self.viewer.layout.name, self.editor.draft_doc.objects.len())
                        ).size(12.0).color(TEXT_DIM));
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
                // ── 深色主題覆寫：讓 ComboBox/Button/Label 在深色 Ribbon 中可讀 ──
                apply_dark_widget_style(ui);

                match self.editor.ribbon_tab {
                    RibbonTab::Home => self.ribbon_home(ui),
                    RibbonTab::Insert => self.ribbon_insert(ui),
                    RibbonTab::Annotate => self.ribbon_annotate(ui),
                    RibbonTab::View => self.ribbon_view(ui),
                    RibbonTab::Manage => self.ribbon_manage(ui),
                    RibbonTab::Output => self.ribbon_output(ui),
                }
            });

        // ── Drawing Tabs（ZWCAD 風格文件分頁 — 可新增/關閉）──
        let mut switch_to: Option<usize> = None;
        let mut close_idx: Option<usize> = None;
        let mut add_new = false;
        let sheet_count = self.editor.draft_sheets.len();
        let active = self.editor.draft_active_sheet;

        egui::TopBottomPanel::top("drawing_tab")
            .exact_height(24.0)
            .show_separator_line(false)
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(50, 50, 54))
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin { left: 8.0, right: 8.0, top: 0.0, bottom: 0.0 }))
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    for (i, (name, _doc)) in self.editor.draft_sheets.iter().enumerate() {
                        let is_active = i == active;
                        let tab_bg = if is_active {
                            egui::Color32::from_rgb(64, 64, 68)
                        } else {
                            egui::Color32::from_rgb(45, 45, 48)
                        };
                        let text_c = if is_active {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_rgb(160, 160, 165)
                        };

                        // Tab 名稱按鈕
                        if ui.add(egui::Button::new(
                            egui::RichText::new(name).size(11.0).color(text_c)
                        ).fill(tab_bg).rounding(egui::Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 })
                         .stroke(egui::Stroke::NONE))
                            .clicked() {
                            switch_to = Some(i);
                        }

                        // × 關閉按鈕（至少保留 1 個分頁）
                        if sheet_count > 1 {
                            let close_btn = ui.add(egui::Button::new(
                                egui::RichText::new("×").size(10.0).color(egui::Color32::from_rgb(140, 140, 145))
                            ).fill(egui::Color32::TRANSPARENT).stroke(egui::Stroke::NONE)
                             .rounding(0.0));
                            if close_btn.on_hover_text("關閉此圖紙").clicked() {
                                close_idx = Some(i);
                            }
                        }

                        ui.add_space(2.0);
                    }

                    // + 新增分頁按鈕
                    let plus_btn = ui.add(egui::Button::new(
                        egui::RichText::new("+").size(13.0).color(egui::Color32::from_rgb(160, 160, 165))
                    ).fill(egui::Color32::TRANSPARENT).stroke(egui::Stroke::NONE));
                    if plus_btn.on_hover_text("新增圖紙").clicked() {
                        add_new = true;
                    }
                });
            });

        // 處理分頁操作（在 show 外面執行避免 borrow 衝突）
        if let Some(idx) = switch_to {
            // 儲存目前 sheet
            if active < self.editor.draft_sheets.len() {
                self.editor.draft_sheets[active].1 = self.editor.draft_doc.clone();
            }
            self.editor.draft_active_sheet = idx;
            if idx < self.editor.draft_sheets.len() {
                self.editor.draft_doc = self.editor.draft_sheets[idx].1.clone();
            }
            self.editor.draft_selected.clear();
        }
        if let Some(idx) = close_idx {
            // 儲存目前 sheet
            if active < self.editor.draft_sheets.len() {
                self.editor.draft_sheets[active].1 = self.editor.draft_doc.clone();
            }
            self.editor.draft_sheets.remove(idx);
            if self.editor.draft_active_sheet >= self.editor.draft_sheets.len() {
                self.editor.draft_active_sheet = self.editor.draft_sheets.len().saturating_sub(1);
            }
            let new_active = self.editor.draft_active_sheet;
            if new_active < self.editor.draft_sheets.len() {
                self.editor.draft_doc = self.editor.draft_sheets[new_active].1.clone();
            }
            self.editor.draft_selected.clear();
        }
        if add_new {
            // 儲存目前 sheet
            if active < self.editor.draft_sheets.len() {
                self.editor.draft_sheets[active].1 = self.editor.draft_doc.clone();
            }
            let n = self.editor.draft_sheets.len() + 1;
            let name = format!("Drawing{}", n);
            self.editor.draft_sheets.push((name, kolibri_drafting::DraftDocument::new()));
            self.editor.draft_active_sheet = self.editor.draft_sheets.len() - 1;
            self.editor.draft_doc = kolibri_drafting::DraftDocument::new();
            self.editor.draft_selected.clear();
        }
    }

    // ─── Tab 內容 ───────────────────────────────────────────────────────────

    #[cfg(feature = "drafting")]
    fn ribbon_home(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
            // ── 繪圖：直線/聚合線 = 2大, 圓▼/弧▼ = 2大+dropdown, 其餘 = 小 ──
            self.ribbon_group_draw(ui);
            self.ribbon_vsep(ui);
            // ── 修改：移動/旋轉/鏡射/偏移 = 4大, 陣列▼ = dropdown ──
            self.ribbon_group_n(ui, "修改", &[
                ToolBtn { tool: Tool::DraftMove, label: "移動", tooltip: "移動 (M)" },
                ToolBtn { tool: Tool::DraftRotate, label: "旋轉", tooltip: "旋轉 (RO)" },
                ToolBtn { tool: Tool::DraftMirror, label: "鏡射", tooltip: "鏡射 (MI)" },
                ToolBtn { tool: Tool::DraftOffset, label: "偏移", tooltip: "偏移 (O)" },
                ToolBtn { tool: Tool::DraftCopy, label: "複製", tooltip: "複製 (CO)" },
                ToolBtn { tool: Tool::DraftScale, label: "比例", tooltip: "比例 (SC)" },
                ToolBtn { tool: Tool::DraftStretch, label: "拉伸", tooltip: "拉伸 (S)" },
                ToolBtn { tool: Tool::DraftTrim, label: "修剪", tooltip: "修剪 (TR)" },
                ToolBtn { tool: Tool::DraftExtend, label: "延伸", tooltip: "延伸 (EX)" },
                ToolBtn { tool: Tool::DraftFillet, label: "圓角", tooltip: "圓角 (F)" },
                ToolBtn { tool: Tool::DraftChamfer, label: "倒角", tooltip: "倒角 (CHA)" },
                ToolBtn { tool: Tool::DraftExplode, label: "分解", tooltip: "分解 (X)" },
                ToolBtn { tool: Tool::DraftErase, label: "刪除", tooltip: "刪除 (E)" },
                ToolBtn { tool: Tool::DraftBreak, label: "打斷", tooltip: "打斷 (BR)" },
                ToolBtn { tool: Tool::DraftJoin, label: "接合", tooltip: "接合 (J)" },
                ToolBtn { tool: Tool::DraftLengthen, label: "拉長", tooltip: "拉長 (LEN)" },
            ], 4);
            // ── 陣列 dropdown ──
            self.ribbon_dropdown_btn(ui, "陣列", Tool::DraftArrayRect, "array", &[
                ("矩形陣列", Tool::DraftArrayRect, "矩形陣列 (ARRAYRECT)"),
                ("環形陣列", Tool::DraftArrayPolar, "環形陣列 (ARRAYPOLAR)"),
                ("路徑陣列", Tool::DraftArrayPath, "路徑陣列 (ARRAYPATH)"),
            ]);
            self.ribbon_vsep(ui);
            // ── 註解（對標 ZWCAD：多行文字/標註/表格 = 3大 + 線性/引線/欄位 = 小）──
            self.ribbon_group_n(ui, "註解", &[
                ToolBtn { tool: Tool::DraftText, label: "多行文字", tooltip: "多行文字 (MT)" },
                ToolBtn { tool: Tool::DraftDimLinear, label: "標註", tooltip: "線性標註 (DLI)" },
                ToolBtn { tool: Tool::DraftTable, label: "表格", tooltip: "表格 (TABLE)" },
                ToolBtn { tool: Tool::DraftDimAligned, label: "線性", tooltip: "對齊標註 (DAL)" },
                ToolBtn { tool: Tool::DraftLeader, label: "引線", tooltip: "引線 (LE)" },
                ToolBtn { tool: Tool::DraftHatch, label: "填充", tooltip: "填充 (H)" },
                ToolBtn { tool: Tool::DraftDimContinue, label: "連續", tooltip: "連續標註 (DCO)" },
                ToolBtn { tool: Tool::DraftDimAngle, label: "角度", tooltip: "角度標註 (DAN)" },
                ToolBtn { tool: Tool::DraftDimRadius, label: "半徑", tooltip: "半徑標註 (DRA)" },
            ], 3);
            self.ribbon_vsep(ui);
            // ── 圖層（對標 ZWCAD：圖層特性 = 1大 + 圖層匹配/置為目前 = 小 + dropdown）──
            self.ribbon_layer_group_v2(ui);
            self.ribbon_vsep(ui);
            // ── 圖塊：2大 ──
            self.ribbon_group_n(ui, "圖塊", &[
                ToolBtn { tool: Tool::DraftBlock, label: "建立", tooltip: "建立圖塊 (B)" },
                ToolBtn { tool: Tool::DraftInsert, label: "插入", tooltip: "插入圖塊 (I)" },
            ], 2);
            self.ribbon_vsep(ui);
            // ── 公用程式 ──
            self.ribbon_group_n(ui, "公用", &[
                ToolBtn { tool: Tool::DraftMeasureDist, label: "距離", tooltip: "測量距離 (DI)" },
                ToolBtn { tool: Tool::DraftMeasureArea, label: "面積", tooltip: "測量面積 (AA)" },
                ToolBtn { tool: Tool::DraftMatchProp, label: "格式刷", tooltip: "複製格式 (MA)" },
                ToolBtn { tool: Tool::DraftQuickSelect, label: "快選", tooltip: "快速選取 (QSELECT)" },
                ToolBtn { tool: Tool::DraftList, label: "資訊", tooltip: "物件資訊 (LI)" },
                ToolBtn { tool: Tool::DraftIdPoint, label: "座標", tooltip: "ID Point" },
            ], 2);
            self.ribbon_vsep(ui);
            self.ribbon_properties_group(ui);
        });
    }

    #[cfg(feature = "drafting")]
    fn ribbon_annotate(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
        self.ribbon_group_n(ui, "尺寸", &[
            ToolBtn { tool: Tool::DraftDimLinear, label: "線性", tooltip: "線性標註 (DLI)" },
            ToolBtn { tool: Tool::DraftDimAligned, label: "對齊", tooltip: "對齊標註 (DAL)" },
            ToolBtn { tool: Tool::DraftDimAngle, label: "角度", tooltip: "角度標註 (DAN)" },
            ToolBtn { tool: Tool::DraftDimRadius, label: "半徑", tooltip: "半徑標註 (DRA)" },
            ToolBtn { tool: Tool::DraftDimDiameter, label: "直徑", tooltip: "直徑標註 (DDI)" },
            ToolBtn { tool: Tool::DraftDimContinue, label: "連續", tooltip: "連續標註 (DCO)" },
            ToolBtn { tool: Tool::DraftDimBaseline, label: "基線", tooltip: "基線標註 (DBA)" },
        ], 2);
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
    fn ribbon_insert(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
            self.ribbon_group_n(ui, "圖塊", &[
                ToolBtn { tool: Tool::DraftInsert, label: "插入", tooltip: "插入圖塊 (I)" },
                ToolBtn { tool: Tool::DraftBlock, label: "建立", tooltip: "建立圖塊 (B)" },
            ], 2);
            self.ribbon_vsep(ui);
            // 參考 group
            ui.vertical(|ui| {
                ui.set_min_size(egui::vec2(100.0, RIBBON_CONTENT_H));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("外部參考 (Xref)").size(11.0).color(TEXT_COLOR));
                ui.label(egui::RichText::new("PDF 底圖").size(11.0).color(TEXT_DIM));
                ui.label(egui::RichText::new("影像參考").size(11.0).color(TEXT_DIM));
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("參考").size(FONT_LABEL).color(TEXT_DIM));
                });
            });
        });
    }

    #[cfg(feature = "drafting")]
    fn ribbon_manage(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_min_size(egui::vec2(120.0, RIBBON_CONTENT_H));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("自訂介面 (CUI)").size(11.0).color(TEXT_COLOR));
                ui.label(egui::RichText::new("執行腳本 (SCR)").size(11.0).color(TEXT_DIM));
                ui.label(egui::RichText::new("CAD 標準檢查").size(11.0).color(TEXT_DIM));
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("自訂").size(FONT_LABEL).color(TEXT_DIM));
                });
            });
        });
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
        // ── 輸出群組：PDF/DXF/DWG ──
        self.ribbon_group_n(ui, "匯出", &[
            ToolBtn { tool: Tool::DraftExportPdf, label: "PDF", tooltip: "匯出 PDF (Ctrl+P)" },
            ToolBtn { tool: Tool::DraftPrint, label: "DXF", tooltip: "匯出 DXF" },
        ], 2);
        self.ribbon_vsep(ui);

        // ── 紙張設定（可選擇）──
        let total_h = RIBBON_CONTENT_H;
        let btn_area_h = total_h - GROUP_LABEL_H;
        let group_w = 150.0;
        let (group_rect, _) = ui.allocate_exact_size(egui::vec2(group_w, total_h), egui::Sense::hover());
        if ui.is_rect_visible(group_rect) {
            let p = ui.painter();

            // 底部標籤
            let label_rect = egui::Rect::from_min_size(
                egui::pos2(group_rect.left(), group_rect.bottom() - GROUP_LABEL_H),
                egui::vec2(group_w, GROUP_LABEL_H));
            p.rect_filled(label_rect, 0.0, GROUP_LABEL_BG);
            p.text(label_rect.center(), egui::Align2::CENTER_CENTER, "頁面設定",
                egui::FontId::proportional(FONT_LABEL), TEXT_DIM);

            // 紙張大小下拉
            let combo_rect = egui::Rect::from_min_size(
                egui::pos2(group_rect.left() + 4.0, group_rect.top() + 6.0),
                egui::vec2(group_w - 8.0, 24.0));
            let mut child = ui.child_ui(combo_rect, egui::Layout::left_to_right(egui::Align::Center), None);
            apply_dark_widget_style(&mut child);
            child.label(egui::RichText::new("紙張:").size(11.0).color(TEXT_COLOR));
            let current_paper = self.viewer.layout.paper_size.label().to_string();
            egui::ComboBox::from_id_source("output_paper_combo")
                .width(80.0)
                .selected_text(egui::RichText::new(&current_paper).size(11.0).color(TEXT_COLOR))
                .show_ui(&mut child, |ui| {
                    for &size in crate::layout::PaperSize::ALL {
                        if ui.selectable_label(self.viewer.layout.paper_size == size,
                            egui::RichText::new(size.label()).size(11.0)).clicked() {
                            self.viewer.layout.paper_size = size;
                        }
                    }
                });

            // 比例下拉
            let scale_rect = egui::Rect::from_min_size(
                egui::pos2(group_rect.left() + 4.0, group_rect.top() + 34.0),
                egui::vec2(group_w - 8.0, 24.0));
            let mut child2 = ui.child_ui(scale_rect, egui::Layout::left_to_right(egui::Align::Center), None);
            apply_dark_widget_style(&mut child2);
            child2.label(egui::RichText::new("比例:").size(11.0).color(TEXT_COLOR));
            let scales: &[f32] = &[1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0];
            egui::ComboBox::from_id_source("output_scale_combo")
                .width(80.0)
                .selected_text(egui::RichText::new(format!("1:{:.0}", self.viewer.layout.scale)).size(11.0).color(TEXT_COLOR))
                .show_ui(&mut child2, |ui| {
                    for &s in scales {
                        if ui.selectable_label((self.viewer.layout.scale - s).abs() < 0.1,
                            egui::RichText::new(format!("1:{:.0}", s)).size(11.0)).clicked() {
                            self.viewer.layout.scale = s;
                        }
                    }
                });

            // 方向
            let orient_rect = egui::Rect::from_min_size(
                egui::pos2(group_rect.left() + 4.0, group_rect.top() + 62.0),
                egui::vec2(group_w - 8.0, 24.0));
            let mut child3 = ui.child_ui(orient_rect, egui::Layout::left_to_right(egui::Align::Center), None);
            apply_dark_widget_style(&mut child3);
            let is_landscape = matches!(self.viewer.layout.orientation, crate::layout::Orientation::Landscape);
            if child3.add(egui::SelectableLabel::new(is_landscape, egui::RichText::new("橫向").size(10.0))).clicked() {
                self.viewer.layout.orientation = crate::layout::Orientation::Landscape;
            }
            if child3.add(egui::SelectableLabel::new(!is_landscape, egui::RichText::new("直向").size(10.0))).clicked() {
                self.viewer.layout.orientation = crate::layout::Orientation::Portrait;
            }
        }
        });  // close horizontal_top for output
    }

    // ─── Group 繪製（ZWCAD 風格：前 N 個=大按鈕, 其餘=3行小按鈕）──────

    #[cfg(feature = "drafting")]
    fn ribbon_group_n(&mut self, ui: &mut egui::Ui, title: &str, tools: &[ToolBtn], n_large: usize) {
        if tools.is_empty() { return; }
        let icon_ids: Vec<Option<egui::TextureId>> = tools.iter().map(|btn_def| {
            crate::svg_icons::tool_icon_name(btn_def.tool)
                .and_then(|name| self.svg_icons.get(ui.ctx(), name))
        }).collect();

        let total_h = RIBBON_CONTENT_H;
        let btn_area_h = total_h - GROUP_LABEL_H;
        let small_btn_h: f32 = btn_area_h / 3.0;
        let n_big = n_large.min(tools.len());
        let small_count = tools.len().saturating_sub(n_big);
        let small_cols = (small_count + 2) / 3;
        let group_w = n_big as f32 * BIG_BTN_W + small_cols as f32 * SMALL_BTN_W + 4.0;

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
            egui::FontId::proportional(FONT_LABEL), TEXT_DIM);

        // ── 大按鈕（前 n_big 個 — icon 大，text 在下）──
        for bi in 0..n_big {
            let btn_rect = egui::Rect::from_min_size(
                egui::pos2(group_rect.left() + 2.0 + bi as f32 * BIG_BTN_W, group_rect.top()),
                egui::vec2(BIG_BTN_W, btn_area_h));
            let resp = ui.interact(btn_rect, ui.id().with(("rb", title, bi)), egui::Sense::click());
            let active = self.editor.tool == tools[bi].tool;
            if active { p.rect_filled(btn_rect, 2.0, ACTIVE_BG); }
            else if resp.hovered() { p.rect_filled(btn_rect, 2.0, HOVER_BG); }
            let tc = if active { ACTIVE_TEXT } else { TEXT_COLOR };
            if let Some(tex_id) = icon_ids[bi] {
                let ir = egui::Rect::from_center_size(
                    egui::pos2(btn_rect.center().x, btn_rect.top() + btn_area_h * 0.33),
                    egui::vec2(ICON_SIZE, ICON_SIZE));
                p.image(tex_id, ir,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE);
            }
            p.text(egui::pos2(btn_rect.center().x, btn_rect.top() + btn_area_h * 0.75),
                egui::Align2::CENTER_TOP, tools[bi].label,
                egui::FontId::proportional(FONT_BIG), tc);
            if resp.on_hover_text(tools[bi].tooltip).clicked() {
                self.editor.tool = tools[bi].tool;
            }
        }

        // ── 小按鈕（3 行排列）──
        let small_x = group_rect.left() + 2.0 + n_big as f32 * BIG_BTN_W;
        for (si, btn_def) in tools[n_big..].iter().enumerate() {
            let col = si / 3;
            let row = si % 3;
            let btn_rect = egui::Rect::from_min_size(
                egui::pos2(small_x + col as f32 * SMALL_BTN_W,
                           group_rect.top() + row as f32 * small_btn_h),
                egui::vec2(SMALL_BTN_W, small_btn_h));

            let resp = ui.interact(btn_rect,
                ui.id().with(("rb", title, si + n_big)), egui::Sense::click());
            let active = self.editor.tool == btn_def.tool;
            if active { p.rect_filled(btn_rect, 1.0, ACTIVE_BG); }
            else if resp.hovered() { p.rect_filled(btn_rect, 1.0, HOVER_BG); }
            let tc = if active { ACTIVE_TEXT } else { TEXT_COLOR };

            let icon_i = si + n_big;
            if let Some(tex_id) = icon_ids.get(icon_i).and_then(|x| *x) {
                let ir = egui::Rect::from_center_size(
                    egui::pos2(btn_rect.left() + 12.0, btn_rect.center().y),
                    egui::vec2(ICON_SM, ICON_SM));
                p.image(tex_id, ir,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE);
            }
            p.text(egui::pos2(btn_rect.left() + 26.0, btn_rect.center().y),
                egui::Align2::LEFT_CENTER, btn_def.label,
                egui::FontId::proportional(FONT_SM), tc);

            if resp.on_hover_text(btn_def.tooltip).clicked() {
                self.editor.tool = btn_def.tool;
            }
        }
    }

    /// 便捷版：所有工具只有第 1 個大
    #[cfg(feature = "drafting")]
    fn ribbon_group(&mut self, ui: &mut egui::Ui, title: &str, tools: &[ToolBtn]) {
        self.ribbon_group_n(ui, title, tools, 1);
    }

    /// 繪圖群組（ZWCAD 風格：直線/聚合線/圓▼/弧▼ = 4大按鈕，圓和弧有 dropdown）
    #[cfg(feature = "drafting")]
    fn ribbon_group_draw(&mut self, ui: &mut egui::Ui) {
        let total_h = RIBBON_CONTENT_H;
        let btn_area_h = total_h - GROUP_LABEL_H;
        let small_btn_h: f32 = btn_area_h / 3.0;

        // 大按鈕定義（前4個）
        let big_tools: [(Tool, &str, &str); 4] = [
            (Tool::DraftLine, "直線", "直線 (L)"),
            (Tool::DraftPolyline, "聚合線", "聚合線 (PL)"),
            (Tool::DraftCircle, "圓", "圓 (C) ▼"),
            (Tool::DraftArc, "弧", "弧 (A) ▼"),
        ];
        // 小按鈕
        let small_tools: &[ToolBtn] = &[
            ToolBtn { tool: Tool::DraftRectangle, label: "矩形", tooltip: "矩形 (REC)" },
            ToolBtn { tool: Tool::DraftEllipse, label: "橢圓", tooltip: "橢圓 (EL)" },
            ToolBtn { tool: Tool::DraftPolygon, label: "多邊形", tooltip: "正多邊形 (POL)" },
            ToolBtn { tool: Tool::DraftSpline, label: "雲形線", tooltip: "雲形線 (SPL)" },
            ToolBtn { tool: Tool::DraftXline, label: "建構線", tooltip: "建構線 (XL)" },
            ToolBtn { tool: Tool::DraftPoint, label: "點", tooltip: "點 (PO)" },
            ToolBtn { tool: Tool::DraftRevcloud, label: "雲形", tooltip: "修訂雲形 (REVCLOUD)" },
            ToolBtn { tool: Tool::DraftRay, label: "射線", tooltip: "射線 (RAY)" },
        ];

        let small_cols = (small_tools.len() + 2) / 3;
        let group_w = 4.0 * BIG_BTN_W + small_cols as f32 * SMALL_BTN_W + 4.0;

        let (group_rect, _) = ui.allocate_exact_size(egui::vec2(group_w, total_h), egui::Sense::hover());
        if !ui.is_rect_visible(group_rect) { return; }
        let p = ui.painter();

        // 底部標籤
        let label_rect = egui::Rect::from_min_size(
            egui::pos2(group_rect.left(), group_rect.bottom() - GROUP_LABEL_H),
            egui::vec2(group_w, GROUP_LABEL_H));
        p.rect_filled(label_rect, 0.0, GROUP_LABEL_BG);
        p.text(label_rect.center(), egui::Align2::CENTER_CENTER, "繪圖",
            egui::FontId::proportional(FONT_LABEL), TEXT_DIM);

        // ── 大按鈕（4 個）──
        for (bi, (tool, label, tooltip)) in big_tools.iter().enumerate() {
            let btn_rect = egui::Rect::from_min_size(
                egui::pos2(group_rect.left() + 2.0 + bi as f32 * BIG_BTN_W, group_rect.top()),
                egui::vec2(BIG_BTN_W, btn_area_h));

            // 圓和弧：分上下兩區域（上=主工具，下=dropdown 三角形）
            let is_dropdown = bi >= 2; // 圓和弧
            let main_rect = if is_dropdown {
                egui::Rect::from_min_size(btn_rect.min, egui::vec2(BIG_BTN_W, btn_area_h - 16.0))
            } else {
                btn_rect
            };

            let main_resp = ui.interact(main_rect, ui.id().with(("draw_big", bi)), egui::Sense::click());
            let active = self.editor.tool == *tool
                || (bi == 2 && matches!(self.editor.tool, Tool::DraftCircle | Tool::DraftCircle2P | Tool::DraftCircle3P))
                || (bi == 3 && matches!(self.editor.tool, Tool::DraftArc | Tool::DraftArc3P | Tool::DraftArcSCE));

            if active { p.rect_filled(btn_rect, 2.0, ACTIVE_BG); }
            else if main_resp.hovered() { p.rect_filled(btn_rect, 2.0, HOVER_BG); }
            let tc = if active { ACTIVE_TEXT } else { TEXT_COLOR };

            // Icon
            let icon_name = crate::svg_icons::tool_icon_name(*tool);
            if let Some(name) = icon_name {
                if let Some(tex_id) = self.svg_icons.get(ui.ctx(), name) {
                    let ir = egui::Rect::from_center_size(
                        egui::pos2(btn_rect.center().x, btn_rect.top() + btn_area_h * 0.30),
                        egui::vec2(ICON_SIZE, ICON_SIZE));
                    p.image(tex_id, ir,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE);
                }
            }

            // 文字（如果有 dropdown，顯示目前選的子模式名稱）
            let display_label = if bi == 2 {
                match self.editor.tool {
                    Tool::DraftCircle2P => "圓2P",
                    Tool::DraftCircle3P => "圓3P",
                    _ => "圓",
                }
            } else if bi == 3 {
                match self.editor.tool {
                    Tool::DraftArc3P => "弧3P",
                    Tool::DraftArcSCE => "弧SCE",
                    _ => "弧",
                }
            } else {
                label
            };
            p.text(egui::pos2(btn_rect.center().x, btn_rect.top() + btn_area_h * 0.68),
                egui::Align2::CENTER_TOP, display_label,
                egui::FontId::proportional(FONT_BIG), tc);

            // Dropdown 三角形
            if is_dropdown {
                let arrow_y = btn_rect.bottom() - GROUP_LABEL_H - 10.0;
                let arrow_cx = btn_rect.center().x;
                let s = 3.5;
                p.add(egui::Shape::convex_polygon(
                    vec![
                        egui::pos2(arrow_cx - s, arrow_y - 1.0),
                        egui::pos2(arrow_cx + s, arrow_y - 1.0),
                        egui::pos2(arrow_cx, arrow_y + s),
                    ],
                    TEXT_DIM, egui::Stroke::NONE,
                ));

                // Dropdown 區域
                let dd_rect = egui::Rect::from_min_size(
                    egui::pos2(btn_rect.left(), btn_rect.bottom() - GROUP_LABEL_H - 16.0),
                    egui::vec2(BIG_BTN_W, 16.0));
                let dd_resp = ui.interact(dd_rect, ui.id().with(("draw_dd", bi)), egui::Sense::click());
                if dd_resp.hovered() {
                    p.rect_filled(dd_rect, 1.0, egui::Color32::from_rgb(85, 85, 90));
                }

                // 點擊 dropdown 區域 → 顯示子選單
                let popup_id = ui.id().with(("draw_popup", bi));
                if dd_resp.clicked() {
                    ui.memory_mut(|mem| mem.toggle_popup(popup_id));
                }

                // Popup 選單
                egui::popup_below_widget(ui, popup_id, &dd_resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                    ui.set_min_width(120.0);
                    if bi == 2 {
                        // 圓子模式
                        if ui.button("圓 (中心-半徑)").clicked() { self.editor.tool = Tool::DraftCircle; }
                        if ui.button("圓 2P (直徑)").clicked() { self.editor.tool = Tool::DraftCircle2P; }
                        if ui.button("圓 3P (三點)").clicked() { self.editor.tool = Tool::DraftCircle3P; }
                    } else {
                        // 弧子模式
                        if ui.button("弧 (中心-半徑)").clicked() { self.editor.tool = Tool::DraftArc; }
                        if ui.button("弧 3P (三點)").clicked() { self.editor.tool = Tool::DraftArc3P; }
                        if ui.button("弧 SCE (起-中-終)").clicked() { self.editor.tool = Tool::DraftArcSCE; }
                    }
                });
            }

            if main_resp.on_hover_text(*tooltip).clicked() {
                self.editor.tool = *tool;
            }
        }

        // ── 小按鈕（3 行排列）──
        let small_x = group_rect.left() + 2.0 + 4.0 * BIG_BTN_W;
        for (si, btn_def) in small_tools.iter().enumerate() {
            let col = si / 3;
            let row = si % 3;
            let btn_rect = egui::Rect::from_min_size(
                egui::pos2(small_x + col as f32 * SMALL_BTN_W,
                           group_rect.top() + row as f32 * small_btn_h),
                egui::vec2(SMALL_BTN_W, small_btn_h));
            let resp = ui.interact(btn_rect, ui.id().with(("draw_sm", si)), egui::Sense::click());
            let active = self.editor.tool == btn_def.tool;
            if active { p.rect_filled(btn_rect, 1.0, ACTIVE_BG); }
            else if resp.hovered() { p.rect_filled(btn_rect, 1.0, HOVER_BG); }
            let tc = if active { ACTIVE_TEXT } else { TEXT_COLOR };
            if let Some(name) = crate::svg_icons::tool_icon_name(btn_def.tool) {
                if let Some(tex_id) = self.svg_icons.get(ui.ctx(), name) {
                    let ir = egui::Rect::from_center_size(
                        egui::pos2(btn_rect.left() + 12.0, btn_rect.center().y),
                        egui::vec2(ICON_SM, ICON_SM));
                    p.image(tex_id, ir,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE);
                }
            }
            p.text(egui::pos2(btn_rect.left() + 26.0, btn_rect.center().y),
                egui::Align2::LEFT_CENTER, btn_def.label,
                egui::FontId::proportional(FONT_SM), tc);
            if resp.on_hover_text(btn_def.tooltip).clicked() {
                self.editor.tool = btn_def.tool;
            }
        }
    }

    /// ZWCAD 風格 dropdown 按鈕（用於陣列等有子模式的工具）
    #[cfg(feature = "drafting")]
    fn ribbon_dropdown_btn(&mut self, ui: &mut egui::Ui, label: &str, default_tool: Tool, popup_key: &str, items: &[(&str, Tool, &str)]) {
        let total_h = RIBBON_CONTENT_H;
        let btn_area_h = total_h - GROUP_LABEL_H;
        let w = 46.0;

        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, total_h), egui::Sense::hover());
        if !ui.is_rect_visible(rect) { return; }
        let p = ui.painter();

        let active = items.iter().any(|(_, t, _)| self.editor.tool == *t);
        let btn_rect = egui::Rect::from_min_size(rect.min, egui::vec2(w, btn_area_h));
        let resp = ui.interact(btn_rect, ui.id().with(("dd_main", popup_key)), egui::Sense::click());
        if active { p.rect_filled(btn_rect, 2.0, ACTIVE_BG); }
        else if resp.hovered() { p.rect_filled(btn_rect, 2.0, HOVER_BG); }
        let tc = if active { ACTIVE_TEXT } else { TEXT_COLOR };

        // 顯示目前選中的子模式名稱
        let current_label = items.iter().find(|(_, t, _)| self.editor.tool == *t)
            .map(|(l, _, _)| *l).unwrap_or(label);
        p.text(egui::pos2(rect.center().x, rect.top() + btn_area_h * 0.35),
            egui::Align2::CENTER_CENTER, current_label,
            egui::FontId::proportional(11.0), tc);

        // Dropdown 三角
        let arrow_y = rect.top() + btn_area_h * 0.65;
        let s = 3.0;
        p.add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(rect.center().x - s, arrow_y),
                egui::pos2(rect.center().x + s, arrow_y),
                egui::pos2(rect.center().x, arrow_y + s + 1.0),
            ],
            TEXT_DIM, egui::Stroke::NONE,
        ));

        // 點擊 → popup
        let popup_id = ui.id().with(("dd_popup", popup_key));
        if resp.clicked() {
            // 如果沒有展開，先設定預設工具
            if !ui.memory(|mem| mem.is_popup_open(popup_id)) {
                self.editor.tool = default_tool;
            }
            ui.memory_mut(|mem| mem.toggle_popup(popup_id));
        }

        egui::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
            ui.set_min_width(130.0);
            for (item_label, tool, tooltip) in items {
                if ui.button(*item_label).on_hover_text(*tooltip).clicked() {
                    self.editor.tool = *tool;
                }
            }
        });
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

    /// 圖層 Group v2（對標 ZWCAD：圖層特性(大) + 匹配/置為目前(小) + dropdown + 色票）
    #[cfg(feature = "drafting")]
    fn ribbon_layer_group_v2(&mut self, ui: &mut egui::Ui) {
        let total_h = RIBBON_CONTENT_H;
        let btn_area_h = total_h - GROUP_LABEL_H;
        let group_w = 240.0;
        let small_btn_h = btn_area_h / 3.0;

        let (group_rect, _) = ui.allocate_exact_size(
            egui::vec2(group_w, total_h), egui::Sense::hover());
        if !ui.is_rect_visible(group_rect) { return; }
        let p = ui.painter();

        // 底部標籤
        let label_rect = egui::Rect::from_min_size(
            egui::pos2(group_rect.left(), group_rect.bottom() - GROUP_LABEL_H),
            egui::vec2(group_w, GROUP_LABEL_H));
        p.rect_filled(label_rect, 0.0, GROUP_LABEL_BG);
        p.text(label_rect.center(), egui::Align2::CENTER_CENTER, "圖層",
            egui::FontId::proportional(FONT_LABEL), TEXT_DIM);

        // ── 大按鈕：圖層特性 ──
        {
            let btn_rect = egui::Rect::from_min_size(
                egui::pos2(group_rect.left() + 2.0, group_rect.top()),
                egui::vec2(BIG_BTN_W, btn_area_h));
            let resp = ui.interact(btn_rect, ui.id().with("layer_prop_btn"), egui::Sense::click());
            let active = self.editor.tool == Tool::DraftLayerProp;
            if active { p.rect_filled(btn_rect, 2.0, ACTIVE_BG); }
            else if resp.hovered() { p.rect_filled(btn_rect, 2.0, HOVER_BG); }
            let tc = if active { ACTIVE_TEXT } else { TEXT_COLOR };
            if let Some(tex_id) = self.svg_icons.get(ui.ctx(), "layer_properties") {
                let ir = egui::Rect::from_center_size(
                    egui::pos2(btn_rect.center().x, btn_rect.top() + btn_area_h * 0.33),
                    egui::vec2(ICON_SIZE, ICON_SIZE));
                p.image(tex_id, ir,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE);
            }
            p.text(egui::pos2(btn_rect.center().x, btn_rect.top() + btn_area_h * 0.72),
                egui::Align2::CENTER_TOP, "圖層特性",
                egui::FontId::proportional(FONT_BIG), tc);
            if resp.on_hover_text("圖層特性管理員 (LA)").clicked() {
                self.editor.show_layer_manager = !self.editor.show_layer_manager;
            }
        }

        // ── 小按鈕：圖層匹配 + 置為目前 + 圖層凍結 ──
        let small_x = group_rect.left() + 2.0 + BIG_BTN_W;
        let small_w = 78.0;
        let layer_small_tools: &[(&str, &str, &str, &str)] = &[
            ("圖層匹配", "layer_match", "圖層匹配 (LAYMCH)", "layer_match_btn"),
            ("置為目前", "layer_set_current", "置為目前圖層 (LAYMCUR)", "layer_cur_btn"),
            ("圖層凍結", "layer_freeze", "凍結圖層", "layer_freeze_btn"),
        ];
        for (row, &(label, icon_name, tip, id_str)) in layer_small_tools.iter().enumerate() {
            let btn_rect = egui::Rect::from_min_size(
                egui::pos2(small_x, group_rect.top() + row as f32 * small_btn_h),
                egui::vec2(small_w, small_btn_h));
            let resp = ui.interact(btn_rect, ui.id().with(id_str), egui::Sense::click());
            if resp.hovered() { p.rect_filled(btn_rect, 1.0, HOVER_BG); }
            if let Some(tex_id) = self.svg_icons.get(ui.ctx(), icon_name) {
                let ir = egui::Rect::from_center_size(
                    egui::pos2(btn_rect.left() + 12.0, btn_rect.center().y),
                    egui::vec2(ICON_SM, ICON_SM));
                p.image(tex_id, ir,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE);
            }
            p.text(egui::pos2(btn_rect.left() + 26.0, btn_rect.center().y),
                egui::Align2::LEFT_CENTER, label,
                egui::FontId::proportional(FONT_SM), TEXT_COLOR);
            if resp.on_hover_text(tip).clicked() {
                match row {
                    0 => { /* 圖層匹配：選取物件套用圖層 */ }
                    1 => { /* 置為目前：選取物件的圖層設為當前 */
                        if let Some(&id) = self.editor.draft_selected.first() {
                            if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                                self.editor.draft_layers.current = obj.layer.clone();
                                self.console_push("INFO", format!("目前圖層設為: {}", obj.layer));
                            }
                        }
                    }
                    _ => { /* 圖層凍結 */ }
                }
            }
        }

        // ── 色票 + 圖層下拉（需要 child_ui，所以先記錄 paint 資料再畫）──
        let drop_x = small_x + small_w + 2.0;
        let drop_w = group_w - BIG_BTN_W - small_w - 8.0;
        let swatch_y = group_rect.top() + 6.0;
        let sw_size = 10.0;

        // 先用 painter 畫色票
        let swatch_colors: &[[u8; 3]] = &[
            [255, 0, 0], [255, 255, 0], [0, 255, 0],
            [0, 255, 255], [0, 0, 255], [255, 0, 255], [255, 255, 255],
        ];
        for (i, c) in swatch_colors.iter().enumerate() {
            let sr = egui::Rect::from_min_size(
                egui::pos2(drop_x + i as f32 * (sw_size + 2.0), swatch_y),
                egui::vec2(sw_size, sw_size));
            p.rect_filled(sr, 1.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
        }

        // 圖層數
        let layer_count = self.editor.draft_layers.layers.len();
        p.text(egui::pos2(drop_x + drop_w * 0.5, swatch_y + sw_size + 30.0),
            egui::Align2::CENTER_TOP,
            format!("{} 個圖層", layer_count),
            egui::FontId::proportional(9.0), TEXT_DIM);

        // 圖層下拉（使用 child_ui 避免 painter borrow 衝突）
        let current = self.editor.draft_layers.current.clone();
        let cur_color = self.editor.draft_layers.current_layer()
            .map(|l| l.color).unwrap_or([255, 255, 255]);
        let combo_rect = egui::Rect::from_min_size(
            egui::pos2(drop_x, swatch_y + sw_size + 4.0),
            egui::vec2(drop_w, 22.0));
        let mut child = ui.child_ui(combo_rect, egui::Layout::left_to_right(egui::Align::Center), None);
        apply_dark_widget_style(&mut child);
        egui::ComboBox::from_id_source("layer_v2_combo")
            .width(drop_w - 4.0)
            .selected_text(egui::RichText::new(format!("\u{25A0} {}", current))
                .size(10.0)
                .color(egui::Color32::from_rgb(cur_color[0], cur_color[1], cur_color[2])))
            .show_ui(&mut child, |ui| {
                let layer_info: Vec<(String, [u8;3], bool, bool)> = self.editor.draft_layers.layers.iter()
                    .map(|l| (l.name.clone(), l.color, l.visible, l.locked))
                    .collect();
                for (name, color, visible, locked) in &layer_info {
                    let is_current = *name == self.editor.draft_layers.current;
                    ui.horizontal(|ui| {
                        let vis_label = if *visible { "\u{1F441}" } else { "\u{2014}" };
                        if ui.add(egui::Label::new(
                            egui::RichText::new(vis_label).size(11.0).color(TEXT_COLOR))
                            .sense(egui::Sense::click())).clicked() {
                            if let Some(layer) = self.editor.draft_layers.layers.iter_mut().find(|l| l.name == *name) {
                                layer.visible = !layer.visible;
                            }
                        }
                        let lock_label = if *locked { "\u{1F512}" } else { "\u{1F513}" };
                        if ui.add(egui::Label::new(
                            egui::RichText::new(lock_label).size(11.0).color(TEXT_DIM))
                            .sense(egui::Sense::click())).clicked() {
                            if let Some(layer) = self.editor.draft_layers.layers.iter_mut().find(|l| l.name == *name) {
                                layer.locked = !layer.locked;
                            }
                        }
                        let (sr, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                        ui.painter().rect_filled(sr, 2.0,
                            egui::Color32::from_rgb(color[0], color[1], color[2]));
                        if ui.selectable_label(is_current,
                            egui::RichText::new(name).size(10.0).color(TEXT_COLOR)).clicked() {
                            self.editor.draft_layers.current = name.clone();
                        }
                    });
                }
            });
    }

    /// 圖層 Group（舊版，保留供相容）
    #[cfg(feature = "drafting")]
    fn ribbon_layer_group(&mut self, ui: &mut egui::Ui) {
        let total_h = RIBBON_CONTENT_H;
        let group_w = 180.0;
        let layer_count = self.editor.draft_layers.layers.len();

        ui.vertical(|ui| {
            ui.set_min_size(egui::vec2(group_w, total_h));

            ui.add_space(6.0);

            // 圖層下拉（含 visibility / lock / color swatch）
            let current = self.editor.draft_layers.current.clone();
            // 取得當前圖層顏色用於 selected_text 顏色色票
            let cur_color = self.editor.draft_layers.current_layer()
                .map(|l| l.color).unwrap_or([255, 255, 255]);
            let combo_id = egui::Id::new("draft_layer_combo_zw");
            egui::ComboBox::from_id_source(combo_id)
                .width(group_w - 16.0)
                .selected_text(egui::RichText::new(format!(
                    "\u{25A0} {}", current))
                    .size(10.0)
                    .color(egui::Color32::from_rgb(cur_color[0], cur_color[1], cur_color[2])))
                .show_ui(ui, |ui| {
                    // 收集 layer info 以避免多次借用
                    let layer_info: Vec<(String, [u8;3], bool, bool)> = self.editor.draft_layers.layers.iter()
                        .map(|l| (l.name.clone(), l.color, l.visible, l.locked))
                        .collect();
                    for (idx, (name, color, visible, locked)) in layer_info.iter().enumerate() {
                        let is_current = *name == self.editor.draft_layers.current;
                        ui.horizontal(|ui| {
                            ui.set_min_width(group_w - 24.0);
                            // 可見性 toggle
                            let vis_label = if *visible { "\u{1F441}" } else { "\u{2014}" };
                            let vis_resp = ui.add(egui::Label::new(
                                egui::RichText::new(vis_label).size(12.0).color(
                                    if *visible { TEXT_COLOR } else { TEXT_DIM }))
                                .sense(egui::Sense::click()));
                            if vis_resp.clicked() {
                                if let Some(layer) = self.editor.draft_layers.layers.get_mut(idx) {
                                    layer.visible = !layer.visible;
                                }
                            }
                            vis_resp.on_hover_text("切換可見性");

                            // 鎖定 toggle
                            let lock_label = if *locked { "\u{1F512}" } else { "\u{1F513}" };
                            let lock_resp = ui.add(egui::Label::new(
                                egui::RichText::new(lock_label).size(12.0).color(
                                    if *locked { egui::Color32::from_rgb(255, 180, 0) } else { TEXT_DIM }))
                                .sense(egui::Sense::click()));
                            if lock_resp.clicked() {
                                if let Some(layer) = self.editor.draft_layers.layers.get_mut(idx) {
                                    layer.locked = !layer.locked;
                                }
                            }
                            lock_resp.on_hover_text("切換鎖定");

                            // 顏色色票
                            let (swatch_rect, _) = ui.allocate_exact_size(
                                egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().rect_filled(swatch_rect, 2.0,
                                egui::Color32::from_rgb(color[0], color[1], color[2]));

                            // 圖層名稱（可點選切換當前圖層）
                            let name_resp = ui.selectable_label(is_current,
                                egui::RichText::new(name).size(10.0).color(TEXT_COLOR));
                            if name_resp.clicked() {
                                self.editor.draft_layers.current = name.clone();
                            }
                        });
                    }
                });

            ui.add_space(4.0);
            ui.label(egui::RichText::new(format!("{} 個圖層", layer_count))
                .size(9.0).color(TEXT_DIM));

            // 底部標籤
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("圖層").size(14.0).color(TEXT_DIM));
            });
        });
    }

    /// 特性 Group（ZWCAD 風格：顏色/線型/線寬 3 排下拉）
    #[cfg(feature = "drafting")]
    fn ribbon_properties_group(&mut self, ui: &mut egui::Ui) {
        let group_w = 150.0;
        ui.vertical(|ui| {
            ui.set_min_size(egui::vec2(group_w, RIBBON_CONTENT_H));
            ui.add_space(4.0);

            // 色彩預設
            const COLORS: &[(&str, [u8; 3])] = &[
                ("隨圖層", [0, 0, 0]),
                ("紅", [255, 0, 0]), ("黃", [255, 255, 0]),
                ("綠", [0, 255, 0]), ("青", [0, 255, 255]),
                ("藍", [0, 0, 255]), ("洋紅", [255, 0, 255]),
                ("白", [255, 255, 255]),
            ];

            // 顏色下拉
            let current_layer_color = self.editor.draft_layers.current_layer()
                .map(|l| l.color).unwrap_or([255, 255, 255]);
            let active_color = if self.editor.draft_prop_color_idx == 0 {
                current_layer_color
            } else {
                COLORS.get(self.editor.draft_prop_color_idx).map(|c| c.1).unwrap_or(current_layer_color)
            };

            ui.horizontal(|ui| {
                let (cr, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                let display_c = if active_color == [0, 0, 0] { current_layer_color } else { active_color };
                ui.painter().rect_filled(cr, 2.0,
                    egui::Color32::from_rgb(display_c[0], display_c[1], display_c[2]));
                let color_label = COLORS.get(self.editor.draft_prop_color_idx).map(|c| c.0).unwrap_or("隨圖層");
                egui::ComboBox::from_id_source("prop_color")
                    .width(group_w - 40.0)
                    .selected_text(egui::RichText::new(color_label).size(10.0).color(TEXT_COLOR))
                    .show_ui(ui, |ui| {
                        for (i, &(name, _rgb)) in COLORS.iter().enumerate() {
                            if ui.selectable_label(i == self.editor.draft_prop_color_idx, name).clicked() {
                                self.editor.draft_prop_color_idx = i;
                            }
                        }
                    });
            });

            ui.add_space(2.0);

            // 線型下拉
            const LINETYPES: &[&str] = &["Continuous", "Dashed", "DashDot", "Center", "Hidden", "Phantom"];
            ui.horizontal(|ui| {
                let (lr, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                let p = ui.painter();
                let lt_idx = self.editor.draft_prop_linetype_idx;
                // 繪製線型示意
                match lt_idx {
                    1 => { // Dashed
                        for seg in 0..2 {
                            let x0 = lr.left() + 1.0 + seg as f32 * 7.0;
                            p.line_segment([egui::pos2(x0, lr.center().y), egui::pos2(x0 + 4.0, lr.center().y)],
                                egui::Stroke::new(1.5, TEXT_COLOR));
                        }
                    }
                    2 => { // DashDot
                        p.line_segment([egui::pos2(lr.left()+1.0, lr.center().y), egui::pos2(lr.left()+5.0, lr.center().y)],
                            egui::Stroke::new(1.5, TEXT_COLOR));
                        p.circle_filled(egui::pos2(lr.left()+8.0, lr.center().y), 1.0, TEXT_COLOR);
                        p.line_segment([egui::pos2(lr.left()+10.0, lr.center().y), egui::pos2(lr.right()-1.0, lr.center().y)],
                            egui::Stroke::new(1.5, TEXT_COLOR));
                    }
                    _ => {
                        p.line_segment([egui::pos2(lr.left()+1.0, lr.center().y), egui::pos2(lr.right()-1.0, lr.center().y)],
                            egui::Stroke::new(1.5, TEXT_COLOR));
                    }
                }
                egui::ComboBox::from_id_source("prop_linetype")
                    .width(group_w - 40.0)
                    .selected_text(egui::RichText::new(LINETYPES[lt_idx]).size(10.0).color(TEXT_COLOR))
                    .show_ui(ui, |ui| {
                        for (i, &name) in LINETYPES.iter().enumerate() {
                            if ui.selectable_label(i == lt_idx, name).clicked() {
                                self.editor.draft_prop_linetype_idx = i;
                            }
                        }
                    });
            });

            ui.add_space(2.0);

            // 線寬下拉
            const LINEWEIGHTS: &[(&str, f64)] = &[
                ("隨圖層", 0.0), ("0.13", 0.13), ("0.18", 0.18), ("0.25", 0.25),
                ("0.35", 0.35), ("0.50", 0.50), ("0.70", 0.70), ("1.00", 1.00),
            ];
            let lw_idx = self.editor.draft_prop_lineweight_idx;
            ui.horizontal(|ui| {
                let (wr, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                let thickness = if lw_idx == 0 { 1.5 } else { (LINEWEIGHTS[lw_idx].1 as f32 * 3.0).max(0.5) };
                ui.painter().line_segment(
                    [egui::pos2(wr.left()+1.0, wr.center().y), egui::pos2(wr.right()-1.0, wr.center().y)],
                    egui::Stroke::new(thickness, TEXT_COLOR));
                egui::ComboBox::from_id_source("prop_lineweight")
                    .width(group_w - 40.0)
                    .selected_text(egui::RichText::new(LINEWEIGHTS[lw_idx].0).size(10.0).color(TEXT_COLOR))
                    .show_ui(ui, |ui| {
                        for (i, &(name, _)) in LINEWEIGHTS.iter().enumerate() {
                            if ui.selectable_label(i == lw_idx, name).clicked() {
                                self.editor.draft_prop_lineweight_idx = i;
                            }
                        }
                    });
            });

            // 底部標籤
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("特性").size(14.0).color(TEXT_DIM));
            });
        });
    }
}

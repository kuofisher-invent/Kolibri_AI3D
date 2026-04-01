//! K3D 智慧分析模式 — 獨立頁面
//!
//! 未來功能規劃（骨架佔位）：
//! - 匯入後自動偵測構件（H型鋼/牆/門窗/螺栓）
//! - 構件樹列表 + 屬性面板
//! - 料表匯出（Excel/CSV）
//! - 語意高亮（按構件類型上色）
//! - MCP AI 查詢介面

use eframe::egui;
use crate::app::KolibriApp;

impl KolibriApp {
    /// 繪製 AI 智慧分析頁面
    pub(crate) fn draw_ai_analysis_page(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let _response = ui.allocate_rect(rect, egui::Sense::click());
        let painter = ui.painter_at(rect);

        // 深色背景
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(25, 28, 35));

        let cx = rect.center().x;
        let mut y = rect.top() + 60.0;

        // ── 標題 ──
        painter.text(
            egui::pos2(cx, y),
            egui::Align2::CENTER_TOP,
            "K3D 智慧分析",
            egui::FontId::proportional(28.0),
            egui::Color32::from_rgb(220, 160, 40),
        );
        y += 45.0;
        painter.text(
            egui::pos2(cx, y),
            egui::Align2::CENTER_TOP,
            "Geometry Normalization & Semantic Analysis Engine",
            egui::FontId::proportional(14.0),
            egui::Color32::from_rgb(140, 145, 155),
        );
        y += 40.0;

        // ── 分隔線 ──
        painter.line_segment(
            [egui::pos2(cx - 200.0, y), egui::pos2(cx + 200.0, y)],
            egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 65, 75)),
        );
        y += 30.0;

        // ── 功能模組列表 ──
        let modules = [
            ("STEP 1", "頂點量化 (Vertex Quantization)", "合併接近點、降低精度噪音（ε = 0.01mm）", "🔬", egui::Color32::from_rgb(80, 200, 255)),
            ("STEP 2", "頂點去重 (Deduplication)", "HashMap 去重，建立 index mesh，減少記憶體", "📦", egui::Color32::from_rgb(100, 220, 150)),
            ("STEP 3", "拓撲正規化 (Topology Cleanup)", "合併共線邊、移除零長邊、統一面法向", "🧹", egui::Color32::from_rgb(200, 180, 100)),
            ("STEP 4", "構件語意偵測 (Primitive Detection)", "偵測 H型鋼/牆體/門窗/螺栓孔/欄杆/樓梯", "🧠", egui::Color32::from_rgb(255, 140, 80)),
            ("STEP 5", "參數化重建 (Parametric Encoding)", "Beam { profile: H300×150, length: 6000mm }", "🏗", egui::Color32::from_rgb(200, 120, 255)),
        ];

        for (step, title, desc, icon, color) in &modules {
            // 模組卡片背景
            let card_rect = egui::Rect::from_min_size(
                egui::pos2(cx - 280.0, y),
                egui::vec2(560.0, 60.0),
            );
            painter.rect_filled(card_rect, 8.0, egui::Color32::from_rgb(35, 40, 50));
            painter.rect_stroke(card_rect, 8.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(55, 60, 70)));

            // Icon + Step
            painter.text(
                egui::pos2(card_rect.left() + 16.0, card_rect.center().y - 10.0),
                egui::Align2::LEFT_CENTER,
                icon,
                egui::FontId::proportional(20.0),
                *color,
            );
            painter.text(
                egui::pos2(card_rect.left() + 48.0, card_rect.center().y - 10.0),
                egui::Align2::LEFT_CENTER,
                &format!("{}: {}", step, title),
                egui::FontId::proportional(14.0),
                egui::Color32::from_rgb(230, 230, 235),
            );
            painter.text(
                egui::pos2(card_rect.left() + 48.0, card_rect.center().y + 12.0),
                egui::Align2::LEFT_CENTER,
                desc,
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(140, 145, 155),
            );

            // 狀態標籤
            painter.text(
                egui::pos2(card_rect.right() - 16.0, card_rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "待開發",
                egui::FontId::proportional(10.0),
                egui::Color32::from_rgb(100, 105, 115),
            );

            y += 68.0;
        }

        y += 20.0;
        // ── 分隔線 ──
        painter.line_segment(
            [egui::pos2(cx - 200.0, y), egui::pos2(cx + 200.0, y)],
            egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 65, 75)),
        );
        y += 25.0;

        // ── 預計產出 ──
        let outputs = [
            ("📋 構件樹列表", "按類型分組的構件清單 + 點擊高亮"),
            ("📊 料表匯出", "自動產生 Excel/CSV 材料統計表"),
            ("🎨 語意高亮", "按構件類型上色顯示（H鋼=紅, 牆=藍, 門窗=綠）"),
            ("🤖 MCP AI 查詢", "\"這張圖有幾根 H300？\" → AI 自動回答"),
            ("📐 壓縮效果", "1MB~10MB 原始 → 50KB~500KB K3D 語意格式"),
        ];

        for (title, desc) in &outputs {
            painter.text(
                egui::pos2(cx - 250.0, y),
                egui::Align2::LEFT_TOP,
                title,
                egui::FontId::proportional(13.0),
                egui::Color32::from_rgb(200, 200, 210),
            );
            painter.text(
                egui::pos2(cx - 70.0, y),
                egui::Align2::LEFT_TOP,
                desc,
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(140, 145, 155),
            );
            y += 22.0;
        }

        y += 20.0;
        painter.text(
            egui::pos2(cx, y),
            egui::Align2::CENTER_TOP,
            "壓縮 ≠ 減少資料，壓縮 = 提取語意",
            egui::FontId::proportional(16.0),
            egui::Color32::from_rgb(220, 160, 40),
        );
        y += 30.0;
        painter.text(
            egui::pos2(cx, y),
            egui::Align2::CENTER_TOP,
            "K3D Geometry Normalization v1 — 讓 Kolibri 從「畫線軟體」變成「懂建築的 CAD」",
            egui::FontId::proportional(11.0),
            egui::Color32::from_rgb(100, 105, 115),
        );
    }
}

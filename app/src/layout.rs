//! Layout/Sheet mode — 2D drawing sheets for printing
//! Similar to SketchUp Layout / AutoCAD Paper Space

use eframe::egui;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    pub paper_size: PaperSize,
    pub orientation: Orientation,
    pub viewports: Vec<Viewport>,
    pub title_block: TitleBlock,
    pub annotations: Vec<LayoutAnnotation>,
    pub scale: f32,  // e.g., 50.0 = 1:50
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PaperSize {
    A4,  // 210 x 297 mm
    A3,  // 297 x 420 mm
    A2,  // 420 x 594 mm
    A1,  // 594 x 841 mm
    A0,  // 841 x 1189 mm
    Custom { width: f32, height: f32 },
}

impl PaperSize {
    pub fn dimensions_mm(&self) -> (f32, f32) {
        match self {
            Self::A4 => (210.0, 297.0),
            Self::A3 => (297.0, 420.0),
            Self::A2 => (420.0, 594.0),
            Self::A1 => (594.0, 841.0),
            Self::A0 => (841.0, 1189.0),
            Self::Custom { width, height } => (*width, *height),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::A4 => "A4",
            Self::A3 => "A3",
            Self::A2 => "A2",
            Self::A1 => "A1",
            Self::A0 => "A0",
            Self::Custom { .. } => "自訂",
        }
    }

    pub const ALL: &'static [PaperSize] = &[Self::A4, Self::A3, Self::A2, Self::A1, Self::A0];
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Orientation {
    Portrait,   // 直向
    Landscape,  // 橫向
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub id: String,
    pub rect: [f32; 4],  // x, y, width, height on paper (mm)
    pub camera_yaw: f32,
    pub camera_pitch: f32,
    pub camera_target: [f32; 3],
    pub camera_distance: f32,
    pub scale: f32,       // 1:50, 1:100 etc.
    pub render_mode: u32, // 0=shaded, 5=sketch, etc.
    pub show_grid: bool,
    pub label: String,    // "平面圖", "立面圖" etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleBlock {
    pub company: String,
    pub project: String,
    pub drawing_number: String,
    pub drawn_by: String,
    pub date: String,
    pub scale: String,
    pub sheet: String,  // "1/3", "A-01" etc.
}

impl Default for TitleBlock {
    fn default() -> Self {
        Self {
            company: "Kolibri Ai3D".into(),
            project: "專案名稱".into(),
            drawing_number: "A-01".into(),
            drawn_by: "設計者".into(),
            date: "2026-03-25".into(),
            scale: "1:50".into(),
            sheet: "1/1".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutAnnotation {
    Text {
        position: [f32; 2],  // paper coords (mm)
        content: String,
        font_size: f32,
        rotation: f32,
    },
    Line {
        start: [f32; 2],
        end: [f32; 2],
        thickness: f32,
    },
    Dimension {
        start: [f32; 2],
        end: [f32; 2],
        offset: f32,
        text: Option<String>,
    },
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            name: "Sheet 1".into(),
            paper_size: PaperSize::A3,
            orientation: Orientation::Landscape,
            viewports: vec![
                Viewport {
                    id: "vp1".into(),
                    rect: [20.0, 20.0, 250.0, 180.0],  // main viewport
                    camera_yaw: 0.8,
                    camera_pitch: -0.5,
                    camera_target: [0.0, 0.0, 0.0],
                    camera_distance: 10000.0,
                    scale: 50.0,
                    render_mode: 0,
                    show_grid: false,
                    label: "透視圖".into(),
                },
                Viewport {
                    id: "vp2".into(),
                    rect: [290.0, 20.0, 100.0, 80.0],
                    camera_yaw: 0.0,
                    camera_pitch: 0.0,
                    camera_target: [0.0, 0.0, 0.0],
                    camera_distance: 10000.0,
                    scale: 100.0,
                    render_mode: 5, // sketch
                    show_grid: false,
                    label: "正面圖".into(),
                },
            ],
            title_block: TitleBlock::default(),
            annotations: Vec::new(),
            scale: 50.0,
        }
    }
}

/// Draw the layout as a 2D paper view using egui
pub fn draw_layout(
    ui: &mut egui::Ui,
    layout: &Layout,
    rect: egui::Rect,
) {
    let painter = ui.painter();

    // Paper dimensions
    let (paper_w, paper_h) = layout.paper_size.dimensions_mm();
    let (pw, ph) = if layout.orientation == Orientation::Landscape {
        (paper_h, paper_w)
    } else {
        (paper_w, paper_h)
    };

    // Scale paper to fit the viewport
    let scale_x = rect.width() * 0.9 / pw;
    let scale_y = rect.height() * 0.9 / ph;
    let scale = scale_x.min(scale_y);

    let paper_screen_w = pw * scale;
    let paper_screen_h = ph * scale;
    let paper_origin = egui::pos2(
        rect.center().x - paper_screen_w / 2.0,
        rect.center().y - paper_screen_h / 2.0,
    );
    let paper_rect = egui::Rect::from_min_size(paper_origin, egui::vec2(paper_screen_w, paper_screen_h));

    // Paper background (white with shadow)
    painter.rect_filled(
        paper_rect.translate(egui::vec2(4.0, 4.0)),
        0.0,
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 30),
    );
    painter.rect_filled(paper_rect, 0.0, egui::Color32::WHITE);
    painter.rect_stroke(paper_rect, 0.0, egui::Stroke::new(1.0, egui::Color32::from_gray(180)));

    // Convert paper mm to screen coordinates
    let to_screen = |mm_x: f32, mm_y: f32| -> egui::Pos2 {
        egui::pos2(
            paper_origin.x + mm_x * scale,
            paper_origin.y + mm_y * scale,
        )
    };

    // Draw viewports as rectangles
    for vp in &layout.viewports {
        let vp_rect = egui::Rect::from_min_size(
            to_screen(vp.rect[0], vp.rect[1]),
            egui::vec2(vp.rect[2] * scale, vp.rect[3] * scale),
        );
        painter.rect_stroke(vp_rect, 0.0, egui::Stroke::new(0.5, egui::Color32::from_gray(150)));

        // Viewport label
        painter.text(
            egui::pos2(vp_rect.left() + 4.0, vp_rect.top() + 2.0),
            egui::Align2::LEFT_TOP,
            &vp.label,
            egui::FontId::proportional(10.0),
            egui::Color32::from_gray(120),
        );

        // Scale label
        painter.text(
            egui::pos2(vp_rect.right() - 4.0, vp_rect.bottom() - 2.0),
            egui::Align2::RIGHT_BOTTOM,
            &format!("1:{:.0}", vp.scale),
            egui::FontId::proportional(9.0),
            egui::Color32::from_gray(140),
        );

        // 簡易 3D 預覽：繪製十字準心和視圖方向指示
        let cx = vp_rect.center();
        let arm = vp_rect.width().min(vp_rect.height()) * 0.15;
        // 十字準心
        painter.line_segment(
            [egui::pos2(cx.x - arm, cx.y), egui::pos2(cx.x + arm, cx.y)],
            egui::Stroke::new(0.5, egui::Color32::from_gray(200)),
        );
        painter.line_segment(
            [egui::pos2(cx.x, cx.y - arm), egui::pos2(cx.x, cx.y + arm)],
            egui::Stroke::new(0.5, egui::Color32::from_gray(200)),
        );
        // 軸向指示
        let axis_len = arm * 0.5;
        painter.line_segment(
            [cx, egui::pos2(cx.x + axis_len, cx.y)],
            egui::Stroke::new(1.5, egui::Color32::from_rgb(220, 60, 60)),
        );
        painter.text(egui::pos2(cx.x + axis_len + 4.0, cx.y), egui::Align2::LEFT_CENTER, "X", egui::FontId::proportional(9.0), egui::Color32::from_rgb(220, 60, 60));
        painter.line_segment(
            [cx, egui::pos2(cx.x, cx.y - axis_len)],
            egui::Stroke::new(1.5, egui::Color32::from_rgb(60, 180, 60)),
        );
        painter.text(egui::pos2(cx.x, cx.y - axis_len - 8.0), egui::Align2::CENTER_BOTTOM, "Y", egui::FontId::proportional(9.0), egui::Color32::from_rgb(60, 180, 60));
        // 視圖標籤
        painter.text(
            vp_rect.center(),
            egui::Align2::CENTER_CENTER,
            &format!("{} — 出圖模式", vp.label),
            egui::FontId::proportional(12.0),
            egui::Color32::from_gray(160),
        );
    }

    // Draw title block (bottom-right corner)
    let tb = &layout.title_block;
    let tb_w = 180.0 * scale;
    let tb_h = 40.0 * scale;
    let tb_rect = egui::Rect::from_min_size(
        egui::pos2(paper_rect.right() - tb_w - 5.0 * scale, paper_rect.bottom() - tb_h - 5.0 * scale),
        egui::vec2(tb_w, tb_h),
    );
    painter.rect_stroke(tb_rect, 0.0, egui::Stroke::new(1.0, egui::Color32::BLACK));

    // Title block content
    let font_small = egui::FontId::proportional(9.0);
    let font_med = egui::FontId::proportional(11.0);
    let black = egui::Color32::BLACK;
    let grey = egui::Color32::from_gray(100);

    let col1 = tb_rect.left() + 4.0;
    let col2 = tb_rect.left() + tb_w * 0.4;
    let row_h = tb_h / 4.0;

    painter.text(egui::pos2(col1, tb_rect.top() + 2.0), egui::Align2::LEFT_TOP, &tb.company, font_med.clone(), black);
    painter.text(egui::pos2(col2, tb_rect.top() + 2.0), egui::Align2::LEFT_TOP, &format!("圖號: {}", tb.drawing_number), font_small.clone(), grey);
    painter.text(egui::pos2(col1, tb_rect.top() + row_h), egui::Align2::LEFT_TOP, &tb.project, font_small.clone(), black);
    painter.text(egui::pos2(col2, tb_rect.top() + row_h), egui::Align2::LEFT_TOP, &format!("比例: {}", tb.scale), font_small.clone(), grey);
    painter.text(egui::pos2(col1, tb_rect.top() + row_h * 2.0), egui::Align2::LEFT_TOP, &format!("繪製: {}", tb.drawn_by), font_small.clone(), grey);
    painter.text(egui::pos2(col2, tb_rect.top() + row_h * 2.0), egui::Align2::LEFT_TOP, &format!("日期: {}", tb.date), font_small.clone(), grey);
    painter.text(egui::pos2(col1, tb_rect.top() + row_h * 3.0), egui::Align2::LEFT_TOP, &format!("頁次: {}", tb.sheet), font_small.clone(), grey);

    // Draw annotations
    for ann in &layout.annotations {
        match ann {
            LayoutAnnotation::Text { position, content, font_size, .. } => {
                painter.text(
                    to_screen(position[0], position[1]),
                    egui::Align2::LEFT_TOP,
                    content,
                    egui::FontId::proportional(*font_size * scale / 3.0),
                    egui::Color32::BLACK,
                );
            }
            LayoutAnnotation::Line { start, end, thickness } => {
                painter.line_segment(
                    [to_screen(start[0], start[1]), to_screen(end[0], end[1])],
                    egui::Stroke::new(*thickness * scale / 3.0, egui::Color32::BLACK),
                );
            }
            LayoutAnnotation::Dimension { start, end, text, .. } => {
                let s = to_screen(start[0], start[1]);
                let e = to_screen(end[0], end[1]);
                let dist = ((end[0]-start[0]).powi(2) + (end[1]-start[1]).powi(2)).sqrt();
                let label = text.clone().unwrap_or_else(|| format!("{:.0}", dist));
                // Simple dimension line
                painter.line_segment([s, e], egui::Stroke::new(0.5, egui::Color32::from_gray(80)));
                let mid = egui::pos2((s.x+e.x)/2.0, (s.y+e.y)/2.0 - 8.0);
                painter.text(mid, egui::Align2::CENTER_BOTTOM, &label,
                    egui::FontId::proportional(9.0), egui::Color32::from_gray(60));
            }
        }
    }

    // Paper size label
    painter.text(
        egui::pos2(paper_rect.left() + 4.0, paper_rect.bottom() + 4.0),
        egui::Align2::LEFT_TOP,
        &format!("{} {} | {}mm x {}mm",
            layout.paper_size.label(),
            if layout.orientation == Orientation::Landscape { "橫向" } else { "直向" },
            pw as u32, ph as u32),
        egui::FontId::proportional(10.0),
        egui::Color32::from_gray(150),
    );
}

/// Draw layout properties panel (right side) when in layout mode
pub fn draw_layout_properties(
    ui: &mut egui::Ui,
    layout: &mut Layout,
) {
    use eframe::egui;

    // ── Paper settings ──
    ui.add_space(4.0);
    ui.label(egui::RichText::new("紙張設定").size(12.0).strong().color(egui::Color32::from_rgb(31, 36, 48)));
    ui.add_space(2.0);

    let section_frame = egui::Frame::none()
        .fill(egui::Color32::from_rgb(248, 249, 252))
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::same(10.0))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));

    section_frame.show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for size in PaperSize::ALL {
                let active = layout.paper_size == *size;
                let btn = egui::Button::new(
                    egui::RichText::new(size.label()).size(11.0)
                        .color(if active { egui::Color32::WHITE } else { egui::Color32::from_rgb(80, 80, 80) })
                )
                .fill(if active { egui::Color32::from_rgb(76, 139, 245) } else { egui::Color32::from_rgb(240, 241, 245) })
                .rounding(8.0);
                if ui.add(btn).clicked() {
                    layout.paper_size = *size;
                }
            }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let land = layout.orientation == Orientation::Landscape;
            let btn_l = egui::Button::new(
                egui::RichText::new("橫向").size(11.0)
                    .color(if land { egui::Color32::WHITE } else { egui::Color32::from_rgb(80, 80, 80) })
            )
            .fill(if land { egui::Color32::from_rgb(76, 139, 245) } else { egui::Color32::from_rgb(240, 241, 245) })
            .rounding(8.0);
            let btn_p = egui::Button::new(
                egui::RichText::new("直向").size(11.0)
                    .color(if !land { egui::Color32::WHITE } else { egui::Color32::from_rgb(80, 80, 80) })
            )
            .fill(if !land { egui::Color32::from_rgb(76, 139, 245) } else { egui::Color32::from_rgb(240, 241, 245) })
            .rounding(8.0);
            if ui.add(btn_l).clicked() { layout.orientation = Orientation::Landscape; }
            if ui.add(btn_p).clicked() { layout.orientation = Orientation::Portrait; }
        });
    });

    ui.add_space(8.0);

    // ── Title block ──
    ui.label(egui::RichText::new("圖框資訊").size(12.0).strong().color(egui::Color32::from_rgb(31, 36, 48)));
    ui.add_space(2.0);

    section_frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("公司").size(10.0).color(egui::Color32::from_gray(120)));
            ui.add(egui::TextEdit::singleline(&mut layout.title_block.company).desired_width(160.0));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("專案").size(10.0).color(egui::Color32::from_gray(120)));
            ui.add(egui::TextEdit::singleline(&mut layout.title_block.project).desired_width(160.0));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("圖號").size(10.0).color(egui::Color32::from_gray(120)));
            ui.add(egui::TextEdit::singleline(&mut layout.title_block.drawing_number).desired_width(160.0));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("繪製").size(10.0).color(egui::Color32::from_gray(120)));
            ui.add(egui::TextEdit::singleline(&mut layout.title_block.drawn_by).desired_width(160.0));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("日期").size(10.0).color(egui::Color32::from_gray(120)));
            ui.add(egui::TextEdit::singleline(&mut layout.title_block.date).desired_width(160.0));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("比例").size(10.0).color(egui::Color32::from_gray(120)));
            ui.add(egui::TextEdit::singleline(&mut layout.title_block.scale).desired_width(160.0));
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("頁次").size(10.0).color(egui::Color32::from_gray(120)));
            ui.add(egui::TextEdit::singleline(&mut layout.title_block.sheet).desired_width(160.0));
        });
    });

    ui.add_space(8.0);

    // ── Viewports ──
    ui.label(egui::RichText::new("視圖").size(12.0).strong().color(egui::Color32::from_rgb(31, 36, 48)));
    ui.add_space(2.0);

    section_frame.show(ui, |ui| {
        let vp_count = layout.viewports.len();
        for i in 0..vp_count {
            let vp = &layout.viewports[i];
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&vp.label).size(11.0));
                ui.label(egui::RichText::new(format!("1:{:.0}", vp.scale)).size(10.0).color(egui::Color32::from_gray(130)));
            });
        }

        ui.add_space(4.0);
        if ui.button(egui::RichText::new("+ 新增視圖").size(11.0)).clicked() {
            let new_id = format!("vp{}", layout.viewports.len() + 1);
            layout.viewports.push(Viewport {
                id: new_id,
                rect: [20.0, 220.0, 150.0, 100.0],
                camera_yaw: 0.0,
                camera_pitch: -std::f32::consts::FRAC_PI_2,
                camera_target: [0.0, 0.0, 0.0],
                camera_distance: 10000.0,
                scale: 100.0,
                render_mode: 0,
                show_grid: false,
                label: "新視圖".into(),
            });
        }
    });

    ui.add_space(8.0);

    // ── Annotations ──
    ui.label(egui::RichText::new("標註").size(12.0).strong().color(egui::Color32::from_rgb(31, 36, 48)));
    ui.add_space(2.0);

    section_frame.show(ui, |ui| {
        ui.label(egui::RichText::new(format!("{} 項標註", layout.annotations.len())).size(10.0).color(egui::Color32::from_gray(130)));
        ui.add_space(2.0);
        if ui.button(egui::RichText::new("+ 文字").size(11.0)).clicked() {
            layout.annotations.push(LayoutAnnotation::Text {
                position: [50.0, 50.0],
                content: "標註文字".into(),
                font_size: 12.0,
                rotation: 0.0,
            });
        }
    });
}

//! PDF 匯出 — 將 2D DraftDocument 匯出為向量 PDF 檔案
//!
//! 使用 printpdf crate。支援 LINE, CIRCLE, ARC, POLYLINE, TEXT, DIMENSION, RECTANGLE, ELLIPSE, POINT

use kolibri_drafting::{DraftDocument, DraftEntity};
use printpdf::*;

/// 紙張大小
#[derive(Debug, Clone, Copy)]
pub struct PdfPaperSize {
    pub width_mm: f64,
    pub height_mm: f64,
}

impl PdfPaperSize {
    pub fn a4_landscape() -> Self { Self { width_mm: 297.0, height_mm: 210.0 } }
    pub fn a3_landscape() -> Self { Self { width_mm: 420.0, height_mm: 297.0 } }
    pub fn a2_landscape() -> Self { Self { width_mm: 594.0, height_mm: 420.0 } }
    pub fn a1_landscape() -> Self { Self { width_mm: 841.0, height_mm: 594.0 } }
    pub fn a0_landscape() -> Self { Self { width_mm: 1189.0, height_mm: 841.0 } }

    pub fn from_name(name: &str) -> Self {
        match name.to_uppercase().as_str() {
            "A4" => Self::a4_landscape(),
            "A3" => Self::a3_landscape(),
            "A2" => Self::a2_landscape(),
            "A1" => Self::a1_landscape(),
            "A0" => Self::a0_landscape(),
            _ => Self::a3_landscape(),
        }
    }
}

/// 匯出 DraftDocument 到 PDF
pub fn export_draft_to_pdf(
    doc: &DraftDocument,
    path: &str,
    paper: PdfPaperSize,
    scale: f64,
) -> Result<usize, String> {
    if doc.objects.is_empty() {
        return Err("沒有圖元可匯出".into());
    }

    let (min_x, min_y, max_x, max_y) = compute_bounds(doc);
    let center_x = (min_x + max_x) / 2.0;
    let center_y = (min_y + max_y) / 2.0;

    let pdf_w = Mm(paper.width_mm as f32);
    let pdf_h = Mm(paper.height_mm as f32);

    let pdf_doc = PdfDocument::empty("Kolibri CAD Export");
    let (page_idx, layer_idx) = pdf_doc.add_page(pdf_w, pdf_h, "圖面");
    let page = pdf_doc.get_page(page_idx);
    let layer = page.get_layer(layer_idx);

    let margin = 10.0;
    let available_w = paper.width_mm - margin * 2.0;
    let available_h = paper.height_mm - margin * 2.0;

    let actual_scale = if scale <= 0.0 {
        let ew = max_x - min_x;
        let eh = max_y - min_y;
        if ew <= 0.0 || eh <= 0.0 { 1.0 }
        else { (available_w / ew).min(available_h / eh) }
    } else {
        1.0 / scale
    };

    let to_pdf_x = |x: f64| -> f32 { (margin + (x - center_x) * actual_scale + available_w / 2.0) as f32 };
    let to_pdf_y = |y: f64| -> f32 { (margin + (y - center_y) * actual_scale + available_h / 2.0) as f32 };

    let font = pdf_doc.add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| format!("字體載入失敗: {:?}", e))?;

    let mut count = 0;

    for obj in &doc.objects {
        if !obj.visible { continue; }

        let color = Color::Rgb(Rgb::new(
            obj.color[0] as f32 / 255.0,
            obj.color[1] as f32 / 255.0,
            obj.color[2] as f32 / 255.0,
            None,
        ));
        let lw = (obj.line_weight as f32 * actual_scale as f32).max(0.1);

        layer.set_outline_color(color.clone());
        layer.set_outline_thickness(lw);

        match &obj.entity {
            DraftEntity::Line { start, end } => {
                let line = Line {
                    points: vec![
                        (Point::new(Mm(to_pdf_x(start[0])), Mm(to_pdf_y(start[1]))), false),
                        (Point::new(Mm(to_pdf_x(end[0])), Mm(to_pdf_y(end[1]))), false),
                    ],
                    is_closed: false,
                };
                layer.add_line(line);
                count += 1;
            }
            DraftEntity::Circle { center, radius } => {
                let cx = to_pdf_x(center[0]);
                let cy = to_pdf_y(center[1]);
                let r = radius * actual_scale;
                let mut line = Line { points: approx_circle(cx, cy, r, 64), is_closed: true };
                layer.add_line(line);
                count += 1;
            }
            DraftEntity::Arc { center, radius, start_angle, end_angle } => {
                let cx = to_pdf_x(center[0]);
                let cy = to_pdf_y(center[1]);
                let r = radius * actual_scale;
                let line = Line { points: approx_arc(cx, cy, r, *start_angle, *end_angle, 64), is_closed: false };
                layer.add_line(line);
                count += 1;
            }
            DraftEntity::Rectangle { p1, p2 } => {
                let line = Line {
                    points: vec![
                        (Point::new(Mm(to_pdf_x(p1[0])), Mm(to_pdf_y(p1[1]))), false),
                        (Point::new(Mm(to_pdf_x(p2[0])), Mm(to_pdf_y(p1[1]))), false),
                        (Point::new(Mm(to_pdf_x(p2[0])), Mm(to_pdf_y(p2[1]))), false),
                        (Point::new(Mm(to_pdf_x(p1[0])), Mm(to_pdf_y(p2[1]))), false),
                    ],
                    is_closed: true,
                };
                layer.add_line(line);
                count += 1;
            }
            DraftEntity::Polyline { points, closed } => {
                if points.len() >= 2 {
                    let pts: Vec<(Point, bool)> = points.iter()
                        .map(|p| (Point::new(Mm(to_pdf_x(p[0])), Mm(to_pdf_y(p[1]))), false))
                        .collect();
                    let line = Line { points: pts, is_closed: *closed };
                    layer.add_line(line);
                    count += 1;
                }
            }
            DraftEntity::Ellipse { center, semi_major, semi_minor, rotation } => {
                let cx = to_pdf_x(center[0]);
                let cy = to_pdf_y(center[1]);
                let a = semi_major * actual_scale;
                let b = semi_minor * actual_scale;
                let line = Line { points: approx_ellipse(cx, cy, a, b, *rotation, 64), is_closed: true };
                layer.add_line(line);
                count += 1;
            }
            DraftEntity::Text { position, content, height, .. } => {
                let fs = (height * actual_scale).max(1.0) as f32;
                let px = to_pdf_x(position[0]);
                let py = to_pdf_y(position[1]);
                let ascii: String = content.chars().map(|c| if c.is_ascii() { c } else { '?' }).collect();
                if !ascii.trim().is_empty() {
                    layer.use_text(&ascii, fs, Mm(px), Mm(py), &font);
                    count += 1;
                }
            }
            DraftEntity::DimLinear { p1, p2, offset, text_override } => {
                let px1 = to_pdf_x(p1[0]); let py1 = to_pdf_y(p1[1]);
                let px2 = to_pdf_x(p2[0]); let py2 = to_pdf_y(p2[1]);
                let off = (offset * actual_scale) as f32;
                let dim_y = py1 + off;
                // 延伸線
                layer.set_outline_thickness(0.15);
                layer.add_line(Line { points: vec![(Point::new(Mm(px1), Mm(py1)), false), (Point::new(Mm(px1), Mm(dim_y)), false)], is_closed: false });
                layer.add_line(Line { points: vec![(Point::new(Mm(px2), Mm(py2)), false), (Point::new(Mm(px2), Mm(dim_y)), false)], is_closed: false });
                // 尺寸線
                layer.set_outline_thickness(0.2);
                layer.add_line(Line { points: vec![(Point::new(Mm(px1), Mm(dim_y)), false), (Point::new(Mm(px2), Mm(dim_y)), false)], is_closed: false });
                // 文字
                let dist = ((p2[0] - p1[0]).powi(2) + (p2[1] - p1[1]).powi(2)).sqrt();
                let auto_text = format!("{:.0}", dist);
                let dim_text = text_override.as_deref().unwrap_or(&auto_text);
                let ascii_dim: String = dim_text.chars().map(|c| if c.is_ascii() { c } else { '?' }).collect();
                let fs = (2.5 * actual_scale).max(1.0) as f32;
                layer.use_text(&ascii_dim, fs, Mm((px1 + px2) / 2.0), Mm(dim_y + 1.0), &font);
                count += 1;
            }
            DraftEntity::Point { position } => {
                let px = to_pdf_x(position[0]);
                let py = to_pdf_y(position[1]);
                let line = Line { points: approx_circle(px, py, 0.3, 8), is_closed: true };
                layer.set_fill_color(color.clone());
                layer.add_line(line);
                count += 1;
            }
            _ => {}
        }
    }

    pdf_doc.save(&mut std::io::BufWriter::new(
        std::fs::File::create(path).map_err(|e| format!("無法建立 PDF: {}", e))?
    )).map_err(|e| format!("PDF 儲存失敗: {}", e))?;

    tracing::info!("PDF 匯出: {} 個圖元 → {}", count, path);
    Ok(count)
}

fn compute_bounds(doc: &DraftDocument) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX; let mut min_y = f64::MAX;
    let mut max_x = f64::MIN; let mut max_y = f64::MIN;
    for obj in &doc.objects {
        if !obj.visible { continue; }
        let pts: Vec<[f64; 2]> = match &obj.entity {
            DraftEntity::Line { start, end } => vec![*start, *end],
            DraftEntity::Circle { center, radius } => vec![[center[0]-radius, center[1]-radius], [center[0]+radius, center[1]+radius]],
            DraftEntity::Rectangle { p1, p2 } => vec![*p1, *p2],
            DraftEntity::Polyline { points, .. } => points.clone(),
            DraftEntity::Text { position, .. } => vec![*position],
            DraftEntity::DimLinear { p1, p2, .. } => vec![*p1, *p2],
            _ => vec![],
        };
        for p in &pts { min_x = min_x.min(p[0]); min_y = min_y.min(p[1]); max_x = max_x.max(p[0]); max_y = max_y.max(p[1]); }
    }
    (min_x, min_y, max_x, max_y)
}

fn approx_circle(cx: f32, cy: f32, r: f64, n: usize) -> Vec<(Point, bool)> {
    let r = r as f32;
    (0..n).map(|i| {
        let a = std::f32::consts::TAU * i as f32 / n as f32;
        (Point::new(Mm(cx + r * a.cos()), Mm(cy + r * a.sin())), false)
    }).collect()
}

fn approx_arc(cx: f32, cy: f32, r: f64, start: f64, end: f64, n: usize) -> Vec<(Point, bool)> {
    let r = r as f32;
    let sweep = if end > start { end - start } else { end - start + std::f64::consts::TAU };
    (0..=n).map(|i| {
        let a = (start + sweep * i as f64 / n as f64) as f32;
        (Point::new(Mm(cx + r * a.cos()), Mm(cy + r * a.sin())), false)
    }).collect()
}

fn approx_ellipse(cx: f32, cy: f32, a: f64, b: f64, rot: f64, n: usize) -> Vec<(Point, bool)> {
    let (a, b) = (a as f32, b as f32);
    let (cr, sr) = (rot.cos() as f32, rot.sin() as f32);
    (0..n).map(|i| {
        let t = std::f32::consts::TAU * i as f32 / n as f32;
        let ex = a * t.cos(); let ey = b * t.sin();
        (Point::new(Mm(cx + ex * cr - ey * sr), Mm(cy + ex * sr + ey * cr)), false)
    }).collect()
}

use eframe::egui;
use glam::{Vec4, Mat4};

/// A persistent dimension annotation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Dimension {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub offset: f32,  // perpendicular offset for the dimension line (mm)
    pub label: Option<String>,  // override label, None = auto distance
}

impl Dimension {
    pub fn new(start: [f32; 3], end: [f32; 3]) -> Self {
        Self { start, end, offset: 300.0, label: None }
    }

    pub fn distance(&self) -> f32 {
        let dx = self.end[0] - self.start[0];
        let dy = self.end[1] - self.start[1];
        let dz = self.end[2] - self.start[2];
        (dx*dx + dy*dy + dz*dz).sqrt()
    }

    pub fn label_text(&self) -> String {
        if let Some(ref l) = self.label {
            l.clone()
        } else {
            let d = self.distance();
            if d >= 1000.0 {
                format!("{:.2} m", d / 1000.0)
            } else {
                format!("{:.0} mm", d)
            }
        }
    }
}

/// Draw all dimension annotations as 2D overlay
pub fn draw_dimensions(
    painter: &egui::Painter,
    dims: &[Dimension],
    view_proj: Mat4,
    rect: &egui::Rect,
) {
    let project = |world: [f32; 3]| -> Option<egui::Pos2> {
        let clip = view_proj * Vec4::new(world[0], world[1], world[2], 1.0);
        if clip.w <= 0.0 { return None; }
        let ndc = clip.truncate() / clip.w;
        if ndc.x < -1.5 || ndc.x > 1.5 || ndc.y < -1.5 || ndc.y > 1.5 { return None; }
        let x = rect.min.x + (ndc.x * 0.5 + 0.5) * rect.width();
        let y = rect.min.y + (0.5 - ndc.y * 0.5) * rect.height();
        Some(egui::pos2(x, y))
    };

    let dim_color = egui::Color32::from_rgb(220, 60, 60); // red dimension lines
    let text_color = egui::Color32::WHITE;
    let bg_color = egui::Color32::from_rgba_unmultiplied(40, 40, 50, 200);
    let stroke = egui::Stroke::new(1.5, dim_color);
    let thin_stroke = egui::Stroke::new(1.0, dim_color);

    for dim in dims {
        let (s, e) = match (project(dim.start), project(dim.end)) {
            (Some(s), Some(e)) => (s, e),
            _ => continue,
        };

        // Skip if too small on screen
        let screen_dist = ((s.x-e.x).powi(2) + (s.y-e.y).powi(2)).sqrt();
        if screen_dist < 20.0 { continue; }

        // Direction perpendicular to the line (screen space)
        let dx = e.x - s.x;
        let dy = e.y - s.y;
        let len = screen_dist;
        let perp_x = -dy / len;
        let perp_y = dx / len;
        let offset = 15.0; // pixels

        // Offset points
        let s_off = egui::pos2(s.x + perp_x * offset, s.y + perp_y * offset);
        let e_off = egui::pos2(e.x + perp_x * offset, e.y + perp_y * offset);

        // Extension lines (from point to offset line)
        let ext = offset + 5.0;
        painter.line_segment(
            [s, egui::pos2(s.x + perp_x * ext, s.y + perp_y * ext)],
            thin_stroke,
        );
        painter.line_segment(
            [e, egui::pos2(e.x + perp_x * ext, e.y + perp_y * ext)],
            thin_stroke,
        );

        // Dimension line
        painter.line_segment([s_off, e_off], stroke);

        // Arrow heads
        let arrow_size = 6.0;
        let dir_x = dx / len;
        let dir_y = dy / len;
        // Start arrow
        painter.line_segment([
            s_off,
            egui::pos2(s_off.x + dir_x * arrow_size + perp_x * arrow_size * 0.4,
                        s_off.y + dir_y * arrow_size + perp_y * arrow_size * 0.4),
        ], stroke);
        painter.line_segment([
            s_off,
            egui::pos2(s_off.x + dir_x * arrow_size - perp_x * arrow_size * 0.4,
                        s_off.y + dir_y * arrow_size - perp_y * arrow_size * 0.4),
        ], stroke);
        // End arrow
        painter.line_segment([
            e_off,
            egui::pos2(e_off.x - dir_x * arrow_size + perp_x * arrow_size * 0.4,
                        e_off.y - dir_y * arrow_size + perp_y * arrow_size * 0.4),
        ], stroke);
        painter.line_segment([
            e_off,
            egui::pos2(e_off.x - dir_x * arrow_size - perp_x * arrow_size * 0.4,
                        e_off.y - dir_y * arrow_size - perp_y * arrow_size * 0.4),
        ], stroke);

        // Label
        let mid = egui::pos2(
            (s_off.x + e_off.x) * 0.5,
            (s_off.y + e_off.y) * 0.5 - 2.0,
        );
        let label = dim.label_text();
        let font = egui::FontId::proportional(12.0);
        let galley = painter.layout_no_wrap(label, font, text_color);
        let text_rect = egui::Align2::CENTER_BOTTOM.anchor_size(mid, galley.size());
        let bg_rect = text_rect.expand(3.0);
        painter.rect_filled(bg_rect, 2.0, bg_color);
        painter.galley(text_rect.min, galley, text_color);
    }
}

use eframe::egui;

use crate::app::{DrawState, KolibriApp, PullFace, RightTab, ScaleHandle, SelectionMode, Tool, WorkMode};
use crate::scene::{MaterialKind, Shape};

/// Figma-style section header: small, muted, strong
pub(crate) fn section_header(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text).size(11.0).color(egui::Color32::from_rgb(110, 118, 135)).strong());
    ui.add_space(2.0);
}

/// Figma-style group frame (light glassmorphism)
pub(crate) fn figma_group(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 240))
        .rounding(egui::Rounding::same(12.0))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)))
        .inner_margin(egui::Margin::same(10.0))
        .show(ui, |ui| {
            add_contents(ui);
        });
}

/// Glassmorphism section frame matching Figma mockup.
/// Use via: `section_frame_full(ui, |ui| { ... });` to auto-fill width.
pub(crate) fn section_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)))
        .rounding(egui::Rounding::same(16.0))
        .inner_margin(egui::Margin::same(12.0))
}

/// Show a section frame that fills the full available width
pub(crate) fn section_frame_full(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    let w = ui.available_width();
    section_frame().show(ui, |ui| {
        ui.set_min_width(w - 26.0); // subtract frame margins
        add_contents(ui);
    });
}

/// Section header text for glassmorphism panels
pub(crate) fn section_header_text(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text)
        .size(11.0)
        .color(egui::Color32::from_rgb(110, 118, 135))
        .strong());
    ui.add_space(6.0);
}

pub(crate) fn draw_material_preview(painter: &egui::Painter, rect: egui::Rect, material: &MaterialKind) {
    let center = rect.center();
    let r = rect.width().min(rect.height()) * 0.42;
    let bc = material.color();

    // Light background with subtle border
    painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(240, 242, 248));
    painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));

    // Draw sphere using offset concentric circles for directional lighting
    // Light comes from upper-right, giving a 3D appearance
    let light_ox = r * 0.15;
    let light_oy = -r * 0.2;
    let steps = 40;

    for i in (0..steps).rev() {
        let t = i as f32 / steps as f32; // 0 = center, 1 = edge
        let frac = 1.0 - t;

        // Position shifts toward light source for inner rings
        let ox = light_ox * frac;
        let oy = light_oy * frac;

        // Brightness: bright center, darker edges, simulating hemisphere lighting
        let base_bright = 0.35 + 0.65 * (1.0 - t * t);
        let edge_darken = t * t * 0.25;
        let brightness = (base_bright - edge_darken).max(0.12);

        let cr = (bc[0] * brightness * 255.0).min(255.0) as u8;
        let cg = (bc[1] * brightness * 255.0).min(255.0) as u8;
        let cb = (bc[2] * brightness * 255.0).min(255.0) as u8;
        let alpha = if bc[3] < 0.9 { (bc[3] * 255.0).max(100.0) as u8 } else { 255 };

        painter.circle_filled(
            egui::pos2(center.x + ox, center.y + oy),
            r * t,
            egui::Color32::from_rgba_unmultiplied(cr, cg, cb, alpha),
        );
    }

    // Bright specular highlight (upper-right area)
    let spec_pos = egui::pos2(center.x + r * 0.2, center.y - r * 0.25);
    painter.circle_filled(spec_pos, r * 0.12, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 100));
    painter.circle_filled(spec_pos, r * 0.06, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180));

    // Clean edge ring
    painter.circle_stroke(center, r, egui::Stroke::new(0.5, egui::Color32::from_gray(80)));

    // Material label
    painter.text(
        egui::pos2(center.x, rect.max.y - 3.0),
        egui::Align2::CENTER_BOTTOM,
        material.label(),
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgb(31, 36, 48),
    );
}

/// SketchUp-style material browser with category tabs, search, and uniform 32px swatches.
/// Returns `Some(material)` if the user clicked a swatch, or `None` if no change.
/// `show_custom` is toggled when the "+" button is clicked.
pub(crate) fn material_picker_ui(
    ui: &mut egui::Ui,
    current: MaterialKind,
    search: &mut String,
    category: &mut usize,
    show_custom: &mut bool,
) -> Option<MaterialKind> {
    let swatch_size = 32.0_f32;
    let categories: &[&str] = &["全部", "石材混凝土", "木材", "金屬", "磚瓦磁磚", "玻璃", "路面地面", "其他"];
    let mut result: Option<MaterialKind> = None;

    // ── Category tabs ──
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
        for (i, cat) in categories.iter().enumerate() {
            let active = *category == i;
            let btn = if active {
                egui::Button::new(egui::RichText::new(*cat).size(10.0).color(egui::Color32::WHITE))
                    .fill(egui::Color32::from_rgb(76, 139, 245))
                    .rounding(10.0)
            } else {
                egui::Button::new(egui::RichText::new(*cat).size(10.0).color(egui::Color32::from_rgb(110, 118, 135)))
                    .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200))
                    .stroke(egui::Stroke::new(0.5, egui::Color32::from_rgb(229, 231, 239)))
                    .rounding(10.0)
            };
            if ui.add(btn).clicked() {
                *category = i;
            }
        }
    });
    ui.add_space(4.0);

    // ── Search input ──
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("\u{1F50D}").size(12.0));
        ui.add(egui::TextEdit::singleline(search)
            .desired_width(ui.available_width() - 10.0)
            .hint_text("搜尋材質...")
            .font(egui::FontId::proportional(11.0)));
    });
    ui.add_space(4.0);

    // ── Filter materials ──
    let search_lower = search.to_lowercase();
    let filtered: Vec<&MaterialKind> = MaterialKind::ALL.iter()
        .filter(|m| {
            if *category > 0 {
                if m.category() != categories[*category] {
                    return false;
                }
            }
            if !search_lower.is_empty() {
                if !m.label().to_lowercase().contains(&search_lower) {
                    return false;
                }
            }
            true
        })
        .collect();

    // ── Swatch grid (fixed 32px, manual grid to prevent stretching) ──
    let gap = 4.0_f32;
    let avail_w = ui.available_width();
    let cols = ((avail_w + gap) / (swatch_size + gap)).floor().max(1.0) as usize;
    let total_items = filtered.len() + 1; // +1 for "+" button
    let rows = (total_items + cols - 1) / cols;
    let grid_h = rows as f32 * (swatch_size + gap);
    let (grid_rect, _) = ui.allocate_exact_size(egui::vec2(avail_w, grid_h), egui::Sense::hover());
    let grid_start = grid_rect.min;

    for (i, mat) in filtered.iter().enumerate() {
        let row = i / cols;
        let col = i % cols;
        let x = grid_start.x + col as f32 * (swatch_size + gap);
        let y = grid_start.y + row as f32 * (swatch_size + gap);
        let rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(swatch_size, swatch_size));
        let is_selected = current == **mat;
        let response = ui.allocate_rect(rect, egui::Sense::click());
        draw_material_swatch(ui.painter(), rect, mat, is_selected, response.hovered());
        if response.clicked() {
            result = Some((**mat).clone());
        }
        response.on_hover_text(mat.label());
    }

    // "+" custom material button (last cell)
    {
        let i = filtered.len();
        let row = i / cols;
        let col = i % cols;
        let x = grid_start.x + col as f32 * (swatch_size + gap);
        let y = grid_start.y + row as f32 * (swatch_size + gap);
        let plus_rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(swatch_size, swatch_size));
        let plus_resp = ui.allocate_rect(plus_rect, egui::Sense::click());
        ui.painter().rect_filled(plus_rect, 6.0, egui::Color32::from_rgb(45, 45, 50));
        ui.painter().rect_stroke(plus_rect, 6.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 60)));
        ui.painter().text(plus_rect.center(), egui::Align2::CENTER_CENTER, "+",
            egui::FontId::proportional(14.0),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 150));
        if plus_resp.clicked() {
            *show_custom = !*show_custom;
        }
        plus_resp.on_hover_text("新增自訂材質");
    }

    // ── Show current material name ──
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("目前:").size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
        ui.label(egui::RichText::new(current.label()).size(11.0).strong());
    });

    result
}

/// Flat-tile material swatch with procedural pattern overlays.
/// Matches SketchUp / Blender-style material preview tiles.
/// Used for the small swatch grids in panels and the floating picker.
pub(crate) fn draw_material_swatch(
    painter: &egui::Painter,
    rect: egui::Rect,
    mat: &MaterialKind,
    selected: bool,
    hovered: bool,
) {
    let c = mat.color();

    // Slightly larger on hover
    let draw_rect = if hovered {
        rect.expand(2.0)
    } else {
        rect
    };

    // Shadow (bottom-right)
    painter.rect_filled(
        draw_rect.translate(egui::vec2(1.5, 1.5)),
        6.0,
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 25),
    );

    // Base fill with material color
    let base_r = (c[0].min(1.0) * 255.0) as u8;
    let base_g = (c[1].min(1.0) * 255.0) as u8;
    let base_b = (c[2].min(1.0) * 255.0) as u8;
    let base_a = if c[3] < 0.9 { (c[3] * 255.0) as u8 } else { 255 };
    let base_color = egui::Color32::from_rgba_unmultiplied(base_r, base_g, base_b, base_a);
    painter.rect_filled(draw_rect, 6.0, base_color);

    // -- Procedural pattern overlay --
    let cat = mat.category();
    match cat {
        "石材混凝土" => {
            swatch_noise_pattern(painter, draw_rect, c, 8);
            swatch_lighting_gradient(painter, draw_rect, 0.15);
        }
        "木材" => {
            swatch_wood_grain(painter, draw_rect, c);
            swatch_lighting_gradient(painter, draw_rect, 0.1);
        }
        "金屬" => {
            swatch_metal_gradient(painter, draw_rect, c);
        }
        "磚瓦磁磚" => {
            if matches!(mat, MaterialKind::Tile | MaterialKind::TileDark) {
                swatch_tile_grid(painter, draw_rect);
            } else {
                swatch_brick_pattern(painter, draw_rect);
            }
            swatch_lighting_gradient(painter, draw_rect, 0.08);
        }
        "玻璃" => {
            swatch_glass_effect(painter, draw_rect);
        }
        "路面地面" => {
            if matches!(mat, MaterialKind::Grass) {
                swatch_grass_dots(painter, draw_rect, c);
            } else {
                swatch_noise_pattern(painter, draw_rect, c, 6);
            }
            swatch_lighting_gradient(painter, draw_rect, 0.1);
        }
        _ => {
            swatch_lighting_gradient(painter, draw_rect, 0.15);
        }
    }

    // Border
    if selected {
        painter.rect_stroke(
            draw_rect,
            6.0,
            egui::Stroke::new(2.5, egui::Color32::from_rgb(76, 139, 245)),
        );
        // Checkmark badge
        let ck = draw_rect.right_bottom() + egui::vec2(-8.0, -8.0);
        painter.circle_filled(ck, 6.0, egui::Color32::from_rgb(76, 139, 245));
        painter.text(
            ck,
            egui::Align2::CENTER_CENTER,
            "\u{2713}",
            egui::FontId::proportional(8.0),
            egui::Color32::WHITE,
        );
    } else if hovered {
        painter.rect_stroke(
            draw_rect,
            6.0,
            egui::Stroke::new(1.5, egui::Color32::WHITE),
        );
    }
}

// ── Swatch pattern helpers ──────────────────────────────────────────────

pub(crate) fn swatch_lighting_gradient(painter: &egui::Painter, rect: egui::Rect, strength: f32) {
    let s = (strength * 255.0) as u8;
    // Bright top-left region
    let tl = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(rect.width() * 0.6, rect.height() * 0.5),
    );
    painter.rect_filled(
        tl,
        6.0,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, s),
    );
    // Dark bottom-right region
    let br_min = egui::pos2(
        rect.min.x + rect.width() * 0.4,
        rect.min.y + rect.height() * 0.5,
    );
    let br = egui::Rect::from_min_max(br_min, rect.max);
    painter.rect_filled(
        br,
        0.0,
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, s / 2),
    );
}

pub(crate) fn swatch_noise_pattern(
    painter: &egui::Painter,
    rect: egui::Rect,
    base: [f32; 4],
    density: usize,
) {
    let w = rect.width();
    let h = rect.height();
    for i in 0..density {
        for j in 0..density {
            let fx = (i as f32 + 0.3) / density as f32;
            let fy = (j as f32 + 0.7) / density as f32;
            let hash = ((i * 7 + j * 13 + 5) % 11) as f32 / 11.0;
            let px = rect.min.x + (fx + hash * 0.08) * w;
            let py = rect.min.y + (fy + (1.0 - hash) * 0.06) * h;
            let bright = 0.7 + hash * 0.3;
            let r = (base[0] * bright * 255.0).min(255.0) as u8;
            let g = (base[1] * bright * 255.0).min(255.0) as u8;
            let b = (base[2] * bright * 255.0).min(255.0) as u8;
            painter.circle_filled(
                egui::pos2(px, py),
                1.5,
                egui::Color32::from_rgba_unmultiplied(r, g, b, 60),
            );
        }
    }
}

pub(crate) fn swatch_wood_grain(painter: &egui::Painter, rect: egui::Rect, base: [f32; 4]) {
    let lines = 6;
    let line_h = rect.height() / lines as f32;
    for i in 0..lines {
        let y = rect.min.y + i as f32 * line_h + line_h * 0.5;
        let dark = if i % 2 == 0 { 0.85 } else { 1.1 };
        let r = (base[0] * dark * 255.0).min(255.0) as u8;
        let g = (base[1] * dark * 255.0).min(255.0) as u8;
        let b = (base[2] * dark * 255.0).min(255.0) as u8;
        painter.line_segment(
            [
                egui::pos2(rect.min.x + 2.0, y),
                egui::pos2(rect.max.x - 2.0, y),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(r, g, b, 80)),
        );
    }
}

pub(crate) fn swatch_metal_gradient(painter: &egui::Painter, rect: egui::Rect, base: [f32; 4]) {
    let steps = 5;
    let step_h = rect.height() / steps as f32;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let bright = 1.3 - t * 0.6;
        let r = (base[0] * bright * 255.0).min(255.0) as u8;
        let g = (base[1] * bright * 255.0).min(255.0) as u8;
        let b = (base[2] * bright * 255.0).min(255.0) as u8;
        let strip = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, rect.min.y + i as f32 * step_h),
            egui::vec2(rect.width(), step_h + 1.0),
        );
        painter.rect_filled(strip, 0.0, egui::Color32::from_rgb(r, g, b));
    }
    // Specular highlight line
    let hl_y = rect.min.y + rect.height() * 0.2;
    painter.line_segment(
        [
            egui::pos2(rect.min.x + 3.0, hl_y),
            egui::pos2(rect.max.x - 3.0, hl_y),
        ],
        egui::Stroke::new(
            1.5,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 100),
        ),
    );
}

pub(crate) fn swatch_brick_pattern(painter: &egui::Painter, rect: egui::Rect) {
    let mortar = egui::Color32::from_rgba_unmultiplied(200, 195, 185, 120);
    let rows = 4;
    let cols = 3;
    let bh = (rect.height() - 2.0) / rows as f32;
    let bw = (rect.width() - 2.0) / cols as f32;
    for r in 0..rows {
        let offset = if r % 2 == 1 { bw * 0.5 } else { 0.0 };
        let y = rect.min.y + 1.0 + r as f32 * bh;
        painter.line_segment(
            [
                egui::pos2(rect.min.x + 1.0, y),
                egui::pos2(rect.max.x - 1.0, y),
            ],
            egui::Stroke::new(1.0, mortar),
        );
        for c in 0..=cols {
            let x = rect.min.x + 1.0 + c as f32 * bw + offset;
            if x > rect.min.x && x < rect.max.x {
                painter.line_segment(
                    [
                        egui::pos2(x, y),
                        egui::pos2(x, (y + bh).min(rect.max.y - 1.0)),
                    ],
                    egui::Stroke::new(1.0, mortar),
                );
            }
        }
    }
}

pub(crate) fn swatch_tile_grid(painter: &egui::Painter, rect: egui::Rect) {
    let grid = 3;
    let gap = rect.width() / grid as f32;
    let grout = egui::Color32::from_rgba_unmultiplied(180, 180, 175, 100);
    for i in 1..grid {
        let x = rect.min.x + i as f32 * gap;
        let y = rect.min.y + i as f32 * gap;
        painter.line_segment(
            [
                egui::pos2(x, rect.min.y + 1.0),
                egui::pos2(x, rect.max.y - 1.0),
            ],
            egui::Stroke::new(1.0, grout),
        );
        painter.line_segment(
            [
                egui::pos2(rect.min.x + 1.0, y),
                egui::pos2(rect.max.x - 1.0, y),
            ],
            egui::Stroke::new(1.0, grout),
        );
    }
}

pub(crate) fn swatch_glass_effect(painter: &egui::Painter, rect: egui::Rect) {
    let streak = egui::Rect::from_min_size(
        egui::pos2(
            rect.min.x + rect.width() * 0.15,
            rect.min.y + rect.height() * 0.1,
        ),
        egui::vec2(rect.width() * 0.15, rect.height() * 0.6),
    );
    painter.rect_filled(
        streak,
        2.0,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 80),
    );
    painter.circle_filled(
        egui::pos2(
            rect.min.x + rect.width() * 0.3,
            rect.min.y + rect.height() * 0.25,
        ),
        2.5,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 120),
    );
}

pub(crate) fn swatch_grass_dots(painter: &egui::Painter, rect: egui::Rect, base: [f32; 4]) {
    for i in 0..12 {
        let hash = ((i * 17 + 3) % 13) as f32 / 13.0;
        let hash2 = ((i * 11 + 7) % 9) as f32 / 9.0;
        let px = rect.min.x + hash * rect.width();
        let py = rect.min.y + hash2 * rect.height();
        let bright = 0.6 + hash * 0.5;
        let r = (base[0] * bright * 255.0).min(255.0) as u8;
        let g = (base[1] * bright * 255.0).min(255.0) as u8;
        let b = (base[2] * bright * 255.0).min(255.0) as u8;
        painter.line_segment(
            [egui::pos2(px, py), egui::pos2(px + 0.5, py - 3.0)],
            egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(r, g, b, 120),
            ),
        );
    }
}


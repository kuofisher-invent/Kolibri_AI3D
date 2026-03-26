use eframe::egui;

use crate::app::{DrawState, KolibriApp, PullFace, RightTab, ScaleHandle, Tool, WorkMode};
use crate::scene::{MaterialKind, Shape};

/// Figma-style section header: small, muted, strong
fn section_header(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(text).size(11.0).color(egui::Color32::from_rgb(110, 118, 135)).strong());
    ui.add_space(2.0);
}

/// Figma-style group frame (light glassmorphism)
fn figma_group(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
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
fn section_frame() -> egui::Frame {
    egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)))
        .rounding(egui::Rounding::same(16.0))
        .inner_margin(egui::Margin::same(12.0))
}

/// Show a section frame that fills the full available width
fn section_frame_full(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    let w = ui.available_width();
    section_frame().show(ui, |ui| {
        ui.set_min_width(w - 26.0); // subtract frame margins
        add_contents(ui);
    });
}

/// Section header text for glassmorphism panels
fn section_header_text(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text)
        .size(11.0)
        .color(egui::Color32::from_rgb(110, 118, 135))
        .strong());
    ui.add_space(6.0);
}

fn draw_material_preview(painter: &egui::Painter, rect: egui::Rect, material: &MaterialKind) {
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
fn material_picker_ui(
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

fn swatch_lighting_gradient(painter: &egui::Painter, rect: egui::Rect, strength: f32) {
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

fn swatch_noise_pattern(
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

fn swatch_wood_grain(painter: &egui::Painter, rect: egui::Rect, base: [f32; 4]) {
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

fn swatch_metal_gradient(painter: &egui::Painter, rect: egui::Rect, base: [f32; 4]) {
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

fn swatch_brick_pattern(painter: &egui::Painter, rect: egui::Rect) {
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

fn swatch_tile_grid(painter: &egui::Painter, rect: egui::Rect) {
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

fn swatch_glass_effect(painter: &egui::Painter, rect: egui::Rect) {
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

fn swatch_grass_dots(painter: &egui::Painter, rect: egui::Rect, base: [f32; 4]) {
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

impl KolibriApp {

    // ── Left: Two-column toolbar ────────────────────────────────────────────

    pub(crate) fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let bsz = egui::vec2(48.0, 48.0);

        // ── Mode switch: 建模 / 鋼構 / 出圖 (compact row) ──
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().button_padding = egui::vec2(4.0, 3.0);

            let brand = egui::Color32::from_rgb(76, 139, 245);
            let steel_color = egui::Color32::from_rgb(220, 100, 50);
            let layout_color = egui::Color32::from_rgb(60, 160, 100);
            let muted = egui::Color32::from_rgb(110, 118, 135);

            let modeling_active = !self.viewer.layout_mode && self.editor.work_mode == WorkMode::Modeling;
            let steel_active = !self.viewer.layout_mode && self.editor.work_mode == WorkMode::Steel;
            let layout_active = self.viewer.layout_mode;

            let make_btn = |label: &str, active: bool, color: egui::Color32| {
                egui::Button::new(egui::RichText::new(label).size(10.0)
                    .color(if active { egui::Color32::WHITE } else { muted }))
                    .fill(if active { color } else { egui::Color32::TRANSPARENT })
                    .rounding(6.0)
            };

            if ui.add(make_btn("建模", modeling_active, brand)).clicked() {
                self.viewer.layout_mode = false;
                self.editor.work_mode = WorkMode::Modeling;
            }
            if ui.add(make_btn("鋼構", steel_active, steel_color)).clicked() {
                self.viewer.layout_mode = false;
                self.editor.work_mode = WorkMode::Steel;
            }
            if ui.add(make_btn("出圖", layout_active, layout_color)).clicked() {
                self.viewer.layout_mode = true;
            }
        });

        ui.add_space(2.0);

        // When in layout mode, don't show 3D tools
        if self.viewer.layout_mode {
            ui.separator();
            ui.label(egui::RichText::new("出圖模式").size(11.0).color(egui::Color32::from_gray(130)));
            ui.label(egui::RichText::new("右側面板可編輯\n紙張與圖框設定").size(10.0).color(egui::Color32::from_gray(160)));
            return;
        }

        // Steel mode uses a different variable now (work_mode), skip the old toggle
        let modeling_active = self.editor.work_mode == WorkMode::Modeling;
        let steel_active = self.editor.work_mode == WorkMode::Steel;
        // (The old m_btn/s_btn block below is now handled by the unified row above)
        // Skip the duplicate toggle — just keep the steel_mode sync
        // steel_mode is derived from work_mode (used elsewhere in the app)

        ui.separator();

        match self.editor.work_mode {
            WorkMode::Modeling => {
                // ── Select & Transform ──
                self.tool_row(ui, bsz, &[
                    (Tool::Select,  "選取\n點擊選取物件，拖曳旋轉視角 (Space)"),
                    (Tool::Move,    "移動\n選取物件後拖曳移動位置 (M)"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::Rotate,  "旋轉\n點擊物件旋轉90度 (Q)"),
                    (Tool::Scale,   "縮放\n點擊物件後上下拖曳等比縮放 (S)"),
                ]);

                ui.separator();

                // ── Draw 2D ──
                // 弧線按鈕：顯示當前模式的圖標（Ctrl+A 循環切換）
                let arc_tool = match self.editor.tool {
                    Tool::Arc3Point => Tool::Arc3Point,
                    Tool::Pie => Tool::Pie,
                    _ => Tool::Arc,
                };
                let arc_tip = match arc_tool {
                    Tool::Arc3Point => "三點弧\nCtrl+A 切換模式 (A)",
                    Tool::Pie       => "扇形\nCtrl+A 切換模式 (A)",
                    _               => "兩點弧\nCtrl+A 切換模式 (A)",
                };
                self.tool_row(ui, bsz, &[
                    (Tool::Line,  "線段\n連續點擊繪製線段，ESC結束 (L)"),
                    (arc_tool,    arc_tip),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::Rectangle, "矩形\n點擊兩角定義底面，再拉高度 (R)"),
                    (Tool::Circle,    "圓形\n點擊圓心，拖出半徑，再拉高度 (C)"),
                ]);

                ui.separator();

                // ── Draw 3D ──
                self.tool_row(ui, bsz, &[
                    (Tool::CreateBox,      "方塊\n點擊兩角定義底面，再拉出高度 (B)"),
                    (Tool::CreateCylinder, "圓柱\n點擊圓心→拖出半徑→拉出高度"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::CreateSphere,   "球體\n點擊圓心→拖出半徑"),
                    (Tool::PushPull,       "推拉\n點擊物件面後拖曳拉伸 (P)"),
                ]);

                ui.separator();

                // ── Modify ──
                self.tool_row(ui, bsz, &[
                    (Tool::Offset,   "偏移複製\n點擊物件，再點擊地面放置複製品 (F)"),
                    (Tool::FollowMe, "跟隨複製\n點擊物件，自動複製並切換移動工具"),
                ]);

                ui.separator();

                // ── Group & Component ──
                self.tool_row(ui, bsz, &[
                    (Tool::Group,     "群組\n將選取的多個物件合併為群組 (G)"),
                    (Tool::Component, "元件\n將選取物件存為可重複使用的元件"),
                ]);

                ui.separator();

                // ── Measure & Paint ──
                self.tool_row(ui, bsz, &[
                    (Tool::TapeMeasure,  "捲尺\n量測兩點之間的距離 (T)"),
                    (Tool::Dimension,    "標註\n兩點標註距離 (D)"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::Text,         "文字\n點擊放置文字標籤"),
                    (Tool::PaintBucket,  "油漆桶\n點擊物件套用目前選擇的材質"),
                ]);

                ui.separator();

                // ── Camera ──
                self.tool_row(ui, bsz, &[
                    (Tool::Orbit, "環繞\n左鍵拖曳旋轉3D視角 (O)"),
                    (Tool::Pan,   "平移\n左鍵拖曳平移視角 (H)"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::ZoomExtents, "全部顯示\n自動縮放至顯示所有物件 (Z)"),
                    (Tool::Eraser,      "橡皮擦\n點擊物件直接刪除 (E)"),
                ]);
            }
            WorkMode::Steel => {
                // Steel tools
                self.tool_row(ui, bsz, &[
                    (Tool::SteelGrid, "軸線\n建立結構軸線系統"),
                    (Tool::SteelColumn, "柱\n點擊放置鋼柱 (Profile)"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::SteelBeam, "梁\n點兩點建立鋼梁"),
                    (Tool::SteelBrace, "斜撐\n點兩點建立斜撐"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::SteelPlate, "鋼板\n畫矩形建立鋼板"),
                    (Tool::SteelConnection, "接頭\n選兩構件建立接頭"),
                ]);

                ui.separator();

                // Steel defaults
                section_header(ui, "預設參數");
                figma_group(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Profile:").size(11.0));
                        ui.text_edit_singleline(&mut self.editor.steel_profile);
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("材質:").size(11.0));
                        ui.text_edit_singleline(&mut self.editor.steel_material);
                    });
                    ui.add(egui::DragValue::new(&mut self.editor.steel_height)
                        .speed(10.0).prefix("柱高: ").suffix(" mm").range(100.0..=50000.0));
                });

                // Common tools (shared between modes)
                ui.separator();
                section_header(ui, "通用");
                self.tool_row(ui, bsz, &[
                    (Tool::Select, "選取 (Space)"),
                    (Tool::Move, "移動 (M)"),
                ]);
                self.tool_row(ui, bsz, &[
                    (Tool::Eraser, "刪除 (E)"),
                    (Tool::TapeMeasure, "量測 (T)"),
                ]);
            }
        }

        // ── Bottom ──
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            ui.small(format!("{}", self.scene.objects.len()));
        });
    }

    pub(crate) fn tool_row(&mut self, ui: &mut egui::Ui, bsz: egui::Vec2, tools: &[(Tool, &str)]) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for &(tool, tip) in tools {
                let active = self.editor.tool == tool;
                let implemented = tool.is_implemented();

                // Allocate button space
                let (rect, resp) = ui.allocate_exact_size(bsz, egui::Sense::click());

                // Light glassmorphism button style
                let bg = if active {
                    egui::Color32::from_rgba_unmultiplied(76, 139, 245, 36) // brand_soft
                } else if resp.hovered() && implemented {
                    egui::Color32::from_rgb(240, 242, 248) // light hover
                } else {
                    egui::Color32::TRANSPARENT
                };
                let border_color = if active {
                    egui::Color32::from_rgb(76, 139, 245)
                } else {
                    egui::Color32::TRANSPARENT
                };
                ui.painter().rect_filled(rect, 12.0, bg);
                if active {
                    ui.painter().rect_stroke(rect, 12.0, egui::Stroke::new(1.0, border_color));
                }

                // Icon color (dark on light)
                let icon_color = if active {
                    egui::Color32::from_rgb(76, 139, 245) // brand blue
                } else if !implemented {
                    egui::Color32::from_gray(200) // very dim on light
                } else if resp.hovered() {
                    egui::Color32::from_rgb(31, 36, 48) // dark text
                } else {
                    egui::Color32::from_rgb(110, 118, 135) // muted
                };
                let icon_rect = rect.shrink(8.0);
                crate::icons::draw_tool_icon(ui.painter(), icon_rect, tool, icon_color);

                // Shortcut key label (bottom-right corner)
                let shortcut = match tool {
                    Tool::Select => Some("Space"),
                    Tool::Move => Some("M"),
                    Tool::Rotate => Some("Q"),
                    Tool::Scale => Some("S"),
                    Tool::Line => Some("L"),
                    Tool::Arc => Some("A"),
                    Tool::Rectangle => Some("R"),
                    Tool::Circle => Some("C"),
                    Tool::CreateBox => Some("B"),
                    Tool::PushPull => Some("P"),
                    Tool::Offset => Some("F"),
                    Tool::TapeMeasure => Some("T"),
                    Tool::Dimension => Some("D"),
                    Tool::Orbit => Some("O"),
                    Tool::Pan => Some("H"),
                    Tool::ZoomExtents => Some("Z"),
                    Tool::Group => Some("G"),
                    Tool::Eraser => Some("E"),
                    _ => None,
                };
                if let Some(key) = shortcut {
                    ui.painter().text(
                        egui::pos2(rect.right() - 3.0, rect.bottom() - 2.0),
                        egui::Align2::RIGHT_BOTTOM,
                        key, egui::FontId::proportional(9.0),
                        egui::Color32::from_rgb(160, 166, 180),
                    );
                }

                // Click handling
                if resp.clicked() && implemented {
                    self.console_push("TOOL", format!("工具列點擊: {:?}", tool));
                    self.editor.tool = tool;
                    self.editor.draw_state = DrawState::Idle;
                    // Inference 2.0: sync tool to inference context
                    self.editor.inference_ctx.current_tool = tool;
                    crate::inference::reset_context(&mut self.editor.inference_ctx);
                    self.editor.inference_ctx.current_tool = tool;
                    match tool {
                        Tool::ZoomExtents => self.zoom_extents(),
                        Tool::Eraser => {
                            for id in std::mem::take(&mut self.editor.selected_ids) {
                                self.scene.delete(&id);
                            }
                        }
                        _ => {}
                    }
                    if matches!(tool, Tool::CreateBox | Tool::CreateCylinder | Tool::CreateSphere
                        | Tool::Rectangle | Tool::Circle | Tool::Line | Tool::Arc | Tool::Arc3Point | Tool::Pie) {
                        self.right_tab = RightTab::Create;
                    }
                }

                let tooltip = if implemented { tip.to_string() }
                    else { format!("{} (尚未實作)", tip) };
                resp.on_hover_text(tooltip);
            }
        });
    }

    // ── Right panel ─────────────────────────────────────────────────────────

    pub(crate) fn right_panel_ui(&mut self, ui: &mut egui::Ui) {
        let tabs = [
            (RightTab::Create, "\u{8a2d}\u{8a08}"),       // 設計
            (RightTab::Properties, "\u{5c6c}\u{6027}"),   // 屬性
            (RightTab::Scene, "\u{5834}\u{666f}"),         // 場景
            (RightTab::AiLog, "\u{8f38}\u{51fa}"),         // 輸出
        ];

        ui.horizontal(|ui| {
            let tab_frame = egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 204))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)))
                .rounding(egui::Rounding::same(16.0))
                .inner_margin(egui::Margin::same(6.0));

            tab_frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (tab, label) in &tabs {
                        let active = self.right_tab == *tab;
                        let btn = if active {
                            egui::Button::new(egui::RichText::new(*label).size(12.0).color(egui::Color32::from_rgb(31, 36, 48)))
                                .fill(egui::Color32::WHITE)
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)))
                                .rounding(12.0)
                        } else {
                            egui::Button::new(egui::RichText::new(*label).size(12.0).color(egui::Color32::from_rgb(110, 118, 135)))
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE)
                                .rounding(12.0)
                        };
                        if ui.add(btn).clicked() {
                            self.right_tab = *tab;
                        }
                    }
                });
            });
        });
        ui.add_space(4.0);

        // Layout mode: show layout properties instead of normal tabs
        if self.viewer.layout_mode {
            egui::ScrollArea::vertical().show(ui, |ui| {
                crate::layout::draw_layout_properties(ui, &mut self.viewer.layout);
            });
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            match self.right_tab {
                RightTab::Properties => {
                    self.tab_properties(ui);
                    self.ai_suggestions_ui(ui);
                }
                RightTab::Create => self.tab_create(ui),
                RightTab::Scene => self.tab_scene(ui),
                RightTab::AiLog => self.tab_ai_log(ui),
            }
        });
    }

    pub(crate) fn tab_properties(&mut self, ui: &mut egui::Ui) {
        // ── Selection Summary (always shown) ──
        section_frame_full(ui, |ui| {
            section_header_text(ui, "SELECTION SUMMARY");
            ui.columns(3, |cols| {
                cols[0].vertical(|ui| {
                    ui.label(egui::RichText::new("物件數").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    ui.label(egui::RichText::new(format!("{}", self.scene.objects.len())).size(18.0).strong());
                });
                cols[1].vertical(|ui| {
                    ui.label(egui::RichText::new("群組").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    ui.label(egui::RichText::new(format!("{}", self.scene.groups.len())).size(18.0).strong());
                });
                cols[2].vertical(|ui| {
                    ui.label(egui::RichText::new("選取").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    ui.label(egui::RichText::new(format!("{}", self.editor.selected_ids.len())).size(18.0).strong());
                });
            });
        });
        ui.add_space(8.0);

        if self.editor.selected_ids.is_empty() {
            // ── Scene details when nothing selected ──
            section_frame_full(ui, |ui| {
                section_header_text(ui, "SCENE INFO");
                let count = self.scene.objects.len();
                if count > 0 {
                    let mut total_vol = 0.0_f64;
                    let mut total_area = 0.0_f64;
                    let mut box_count = 0u32;
                    let mut cyl_count = 0u32;
                    let mut sph_count = 0u32;
                    let mut line_count = 0u32;
                    for obj in self.scene.objects.values() {
                        total_vol += crate::measure::volume(obj);
                        total_area += crate::measure::surface_area(obj);
                        match &obj.shape {
                            Shape::Box{..} => box_count += 1,
                            Shape::Cylinder{..} => cyl_count += 1,
                            Shape::Sphere{..} => sph_count += 1,
                            Shape::Line{..} => line_count += 1,
                            _ => {}
                        }
                    }
                    if box_count > 0 { ui.small(format!("  ⬜ 方塊: {}", box_count)); }
                    if cyl_count > 0 { ui.small(format!("  ○ 圓柱: {}", cyl_count)); }
                    if sph_count > 0 { ui.small(format!("  ◎ 球體: {}", sph_count)); }
                    if line_count > 0 { ui.small(format!("  ╱ 線段: {}", line_count)); }
                    ui.add_space(4.0);
                    ui.small(format!("總表面積: {}", crate::measure::format_area(total_area)));
                    if total_vol > 0.0 {
                        ui.small(format!("總體積: {}", crate::measure::format_volume(total_vol)));
                    }
                } else {
                    ui.label(egui::RichText::new("場景為空").color(egui::Color32::from_rgb(110, 118, 135)));
                }
            });

            ui.add_space(8.0);

            // ── Quick camera views ──
            section_frame_full(ui, |ui| {
                section_header_text(ui, "CAMERA");
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("前").clicked() { self.viewer.camera.set_front(); }
                    if ui.small_button("後").clicked() { self.viewer.camera.set_back(); }
                    if ui.small_button("左").clicked() { self.viewer.camera.set_left(); }
                    if ui.small_button("右").clicked() { self.viewer.camera.set_right(); }
                    if ui.small_button("上").clicked() { self.viewer.camera.set_top(); }
                    if ui.small_button("等角").clicked() { self.viewer.camera.set_iso(); }
                });
                ui.horizontal(|ui| {
                    if ui.small_button("全部顯示").clicked() { self.zoom_extents(); }
                    let ortho_label = if self.viewer.use_ortho { "透視" } else { "平行" };
                    if ui.small_button(ortho_label).clicked() { self.viewer.use_ortho = !self.viewer.use_ortho; }
                });
            });

            ui.add_space(8.0);

            // ── Render mode ──
            section_frame_full(ui, |ui| {
                section_header_text(ui, "DISPLAY");
                ui.horizontal_wrapped(|ui| {
                    let modes = [
                        (crate::app::RenderMode::Shaded, "著色"),
                        (crate::app::RenderMode::Wireframe, "線框"),
                        (crate::app::RenderMode::XRay, "X光"),
                        (crate::app::RenderMode::HiddenLine, "隱藏線"),
                        (crate::app::RenderMode::Monochrome, "單色"),
                        (crate::app::RenderMode::Sketch, "草稿"),
                    ];
                    for (mode, label) in modes {
                        if ui.selectable_label(self.viewer.render_mode == mode, label).clicked() {
                            self.viewer.render_mode = mode;
                        }
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("線粗");
                    ui.add(egui::Slider::new(&mut self.viewer.edge_thickness, 0.5..=8.0).step_by(0.5));
                });
                ui.checkbox(&mut self.viewer.show_colors, "顯示顏色");
            });

            ui.add_space(8.0);

            // ── Material browser (always accessible, SketchUp-style) ──
            section_frame_full(ui, |ui| {
                section_header_text(ui, "MATERIAL");
                if let Some(new_mat) = material_picker_ui(
                    ui,
                    self.create_mat,
                    &mut self.mat_search,
                    &mut self.mat_category_idx,
                    &mut self.show_custom_color_picker,
                ) {
                    self.create_mat = new_mat;
                }
                if self.show_custom_color_picker {
                    ui.add_space(6.0);
                    figma_group(ui, |ui| {
                        ui.label(egui::RichText::new("自訂材質").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                        let mut color = egui::Color32::from_rgba_unmultiplied(
                            (self.custom_color[0] * 255.0) as u8,
                            (self.custom_color[1] * 255.0) as u8,
                            (self.custom_color[2] * 255.0) as u8,
                            (self.custom_color[3] * 255.0) as u8,
                        );
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            self.custom_color = [
                                color.r() as f32 / 255.0,
                                color.g() as f32 / 255.0,
                                color.b() as f32 / 255.0,
                                color.a() as f32 / 255.0,
                            ];
                        }
                        if ui.button("套用自訂色").clicked() {
                            self.create_mat = crate::scene::MaterialKind::Custom(self.custom_color);
                            self.show_custom_color_picker = false;
                        }
                    });
                }
            });

            ui.add_space(8.0);

            // ── Tips ──
            section_frame_full(ui, |ui| {
                section_header_text(ui, "TIPS");
                ui.small("中鍵拖曳: 旋轉視角");
                ui.small("Shift+中鍵: 平移");
                ui.small("滾輪: 縮放");
                ui.small("B: 建立方塊");
                ui.small("P: 推拉工具");
                ui.small("L: 線段工具");
                ui.small("Ctrl+Z: 復原");
                ui.small("Ctrl+S: 儲存");
            });

            return;
        }
        if self.editor.selected_ids.len() > 1 {
            section_frame_full(ui, |ui| {
                section_header_text(ui, "MULTI-SELECT");
                ui.label(egui::RichText::new(format!("已選取 {} 個物件", self.editor.selected_ids.len())).strong());
                ui.add_space(4.0);
                for sid in &self.editor.selected_ids {
                    if let Some(obj) = self.scene.objects.get(sid) {
                        let icon = match &obj.shape {
                            Shape::Box{..} => "⬜", Shape::Cylinder{..} => "○", Shape::Sphere{..} => "◎", Shape::Line{..} => "╱", Shape::Mesh{..} => "◇",
                        };
                        ui.small(format!("{} {}", icon, obj.name));
                    }
                }
            });
            return;
        }
        let id = self.editor.selected_ids[0].clone();
        let obj = match self.scene.objects.get_mut(&id) {
            Some(o) => o,
            None => { self.editor.selected_ids.clear(); return; }
        };

        // Object header
        section_frame_full(ui, |ui| {
            ui.horizontal(|ui| {
                let icon = match &obj.shape {
                    Shape::Box{..} => "⬜", Shape::Cylinder{..} => "○", Shape::Sphere{..} => "◎", Shape::Line{..} => "╱", Shape::Mesh{..} => "◇",
                };
                ui.label(egui::RichText::new(icon).size(16.0));
                ui.text_edit_singleline(&mut obj.name);
            });
            ui.small(format!("ID: {}", obj.id));
        });
        ui.add_space(8.0);

        // Dimensions
        section_frame_full(ui, |ui| {
            section_header_text(ui, "DIMENSIONS");
            match &mut obj.shape {
                Shape::Box { width, height, depth } => {
                    ui.add(egui::DragValue::new(width).speed(10.0).prefix("寬 W: ").suffix(" mm").range(1.0..=f32::MAX));
                    ui.add(egui::DragValue::new(height).speed(10.0).prefix("高 H: ").suffix(" mm").range(1.0..=f32::MAX));
                    ui.add(egui::DragValue::new(depth).speed(10.0).prefix("深 D: ").suffix(" mm").range(1.0..=f32::MAX));
                }
                Shape::Cylinder { radius, height, segments } => {
                    ui.add(egui::DragValue::new(radius).speed(10.0).prefix("R: ").suffix(" mm").range(1.0..=f32::MAX));
                    ui.add(egui::DragValue::new(height).speed(10.0).prefix("H: ").suffix(" mm").range(1.0..=f32::MAX));
                    ui.add(egui::DragValue::new(segments).speed(1.0).prefix("細分: ").range(4..=128));
                }
                Shape::Sphere { radius, segments } => {
                    ui.add(egui::DragValue::new(radius).speed(10.0).prefix("R: ").suffix(" mm").range(1.0..=f32::MAX));
                    ui.add(egui::DragValue::new(segments).speed(1.0).prefix("細分: ").range(4..=128));
                }
                Shape::Line { points, thickness, .. } => {
                    ui.label(format!("線段 ({} 點)", points.len()));
                    ui.add(egui::DragValue::new(thickness).speed(1.0).prefix("粗細: ").suffix(" mm").range(1.0..=500.0));
                }
                Shape::Mesh(ref mesh) => {
                    ui.label(format!("網格: {} 頂點, {} 邊, {} 面",
                        mesh.vertices.len(), mesh.edge_count(), mesh.faces.len()));
                }
            }
        });
        ui.add_space(8.0);

        // Transform (Position + Rotation)
        section_frame_full(ui, |ui| {
            section_header_text(ui, "TRANSFORM");

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Position").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("mm").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                });
            });
            ui.add(egui::DragValue::new(&mut obj.position[0]).speed(10.0).prefix("X: ").suffix(" mm"));
            ui.add(egui::DragValue::new(&mut obj.position[1]).speed(10.0).prefix("Y: ").suffix(" mm"));
            ui.add(egui::DragValue::new(&mut obj.position[2]).speed(10.0).prefix("Z: ").suffix(" mm"));

            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Rotation").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("deg").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                });
            });
            let mut deg = obj.rotation_y.to_degrees();
            if ui.add(egui::DragValue::new(&mut deg).speed(1.0).prefix("Y軸: ").suffix("°").range(-360.0..=360.0)).changed() {
                obj.rotation_y = deg.to_radians();
            }
        });
        ui.add_space(8.0);

        // Component Kind (collision)
        section_frame_full(ui, |ui| {
            section_header_text(ui, "COMPONENT");
            ui.horizontal(|ui| {
                ui.label("元件類型:");
                let kind_name = match obj.component_kind {
                    crate::collision::ComponentKind::Column => "柱",
                    crate::collision::ComponentKind::Beam => "梁",
                    crate::collision::ComponentKind::Plate => "板",
                    crate::collision::ComponentKind::Bolt => "螺栓",
                    crate::collision::ComponentKind::Weld => "焊接",
                    crate::collision::ComponentKind::Foundation => "基礎",
                    crate::collision::ComponentKind::Equipment => "設備",
                    crate::collision::ComponentKind::Generic => "一般",
                };
                ui.label(kind_name);
            });
        });
        ui.add_space(8.0);

        // Steel properties (when component is Beam/Column/Plate)
        if !matches!(obj.component_kind, crate::collision::ComponentKind::Generic) {
            section_frame_full(ui, |ui| {
                section_header_text(ui, "STEEL PROPERTIES");
                ui.horizontal(|ui| {
                    ui.label("Profile:");
                    ui.label(egui::RichText::new(&self.editor.steel_profile).strong());
                });
                ui.horizontal(|ui| {
                    ui.label("Material:");
                    ui.label(egui::RichText::new(&self.editor.steel_material).strong());
                });
                let dims = match &obj.shape {
                    Shape::Box { width, height, depth } => format!("{:.0}x{:.0}x{:.0} mm", width, height, depth),
                    _ => String::new(),
                };
                if !dims.is_empty() {
                    ui.label(format!("尺寸: {}", dims));
                }
            });
            ui.add_space(8.0);
        }

        // Layer / Tag
        section_frame_full(ui, |ui| {
            section_header_text(ui, "LAYER");
            ui.horizontal(|ui| {
                ui.label("標籤:");
                ui.text_edit_singleline(&mut obj.tag);
            });
        });
        ui.add_space(8.0);

        // Material (SketchUp-style browser)
        // We need to work around the borrow of `obj` from `self.scene.objects`
        // by using local copies of the search/category state.
        let mut mat_search_local = std::mem::take(&mut self.mat_search);
        let mut mat_cat_local = self.mat_category_idx;
        let mut show_custom_local = self.show_custom_color_picker;
        section_frame_full(ui, |ui| {
            section_header_text(ui, "MATERIAL");

            if let Some(new_mat) = material_picker_ui(
                ui,
                obj.material,
                &mut mat_search_local,
                &mut mat_cat_local,
                &mut show_custom_local,
            ) {
                obj.material = new_mat;
                self.scene.version += 1;
            }

            ui.add_space(6.0);

            // PBR sliders
            ui.add(egui::Slider::new(&mut obj.roughness, 0.0..=1.0).text("粗糙度"));
            ui.add(egui::Slider::new(&mut obj.metallic, 0.0..=1.0).text("金屬感"));

            // Custom colour picker
            ui.add_space(4.0);
            egui::CollapsingHeader::new("自訂顏色").show(ui, |ui| {
                let rgba = obj.material.color();
                let mut c = egui::Color32::from_rgba_unmultiplied(
                    (rgba[0]*255.0) as u8, (rgba[1]*255.0) as u8,
                    (rgba[2]*255.0) as u8, (rgba[3]*255.0) as u8);
                if ui.color_edit_button_srgba(&mut c).changed() {
                    obj.material = MaterialKind::Custom([
                        c.r() as f32/255.0, c.g() as f32/255.0,
                        c.b() as f32/255.0, c.a() as f32/255.0]);
                    self.scene.version += 1;
                }

                ui.add_space(4.0);
                ui.label("常用色");
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                    let paint_colors: &[(u32, &str)] = &[
                        (0xE74C3C, "紅"), (0xE67E22, "橙"), (0xF1C40F, "黃"),
                        (0x2ECC71, "綠"), (0x3498DB, "藍"), (0x9B59B6, "紫"),
                        (0xECF0F1, "白"), (0x95A5A6, "灰"), (0x2C3E50, "深灰"),
                        (0x1ABC9C, "青"), (0xD35400, "棕"), (0x7F8C8D, "石灰"),
                    ];
                    let sw = 32.0;
                    for &(hex, label) in paint_colors {
                        let r = ((hex >> 16) & 0xFF) as u8;
                        let g = ((hex >> 8) & 0xFF) as u8;
                        let b = (hex & 0xFF) as u8;
                        let color = egui::Color32::from_rgb(r, g, b);
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(sw, sw), egui::Sense::click());
                        let is_sel = obj.material == MaterialKind::Paint(hex);
                        ui.painter().rect_filled(rect, 10.0, color);
                        if is_sel {
                            ui.painter().rect_stroke(rect, 10.0,
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(76, 139, 245)));
                        } else {
                            ui.painter().rect_stroke(rect, 10.0,
                                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 20)));
                        }
                        if resp.clicked() {
                            obj.material = MaterialKind::Paint(hex);
                            self.scene.version += 1;
                        }
                        resp.on_hover_text(label);
                    }
                });
            });
        });
        self.mat_search = mat_search_local;
        self.mat_category_idx = mat_cat_local;
        self.show_custom_color_picker = show_custom_local;
        ui.add_space(8.0);

        // Texture mapping
        section_frame_full(ui, |ui| {
            section_header_text(ui, "TEXTURE");

            if let Some(ref path) = obj.texture_path {
                let filename = path.rsplit(['\\', '/']).next().unwrap_or(path);
                ui.label(format!("  {}", filename));
                if let Some((w, h)) = self.texture_manager.info(path) {
                    ui.small(format!("{}x{} px", w, h));
                }
                if ui.button("移除紋理").clicked() {
                    obj.texture_path = None;
                    self.scene.version += 1;
                }
            } else {
                ui.label(egui::RichText::new("無紋理").color(egui::Color32::from_rgb(110, 118, 135)));
            }

            if ui.button("載入紋理圖片...").clicked() {
                let file = rfd::FileDialog::new()
                    .set_title("載入紋理")
                    .add_filter("圖片", &["png", "jpg", "jpeg", "bmp"])
                    .pick_file();
                if let Some(path) = file {
                    let ps = path.to_string_lossy().to_string();
                    match self.texture_manager.load(&ps) {
                        Ok(_) => {
                            obj.texture_path = Some(ps);
                            self.scene.version += 1;
                            self.file_message = Some(("紋理已載入".into(), std::time::Instant::now()));
                        }
                        Err(e) => {
                            self.file_message = Some((e, std::time::Instant::now()));
                        }
                    }
                }
            }
        });
        ui.add_space(8.0);

        // Measurements
        section_frame_full(ui, |ui| {
            section_header_text(ui, "MEASURE");
            let area = crate::measure::surface_area(obj);
            let vol = crate::measure::volume(obj);
            ui.label(format!("表面積: {}", crate::measure::format_area(area)));
            if vol > 0.0 {
                ui.label(format!("體積: {}", crate::measure::format_volume(vol)));
            }
            // Weight estimate based on material density (kg/m³)
            if vol > 0.0 {
                let density = match &obj.material {
                    MaterialKind::Concrete | MaterialKind::ConcreteSmooth => 2400.0,
                    MaterialKind::Stone => 2600.0,
                    MaterialKind::Marble => 2700.0,
                    MaterialKind::Granite => 2750.0,
                    MaterialKind::Wood | MaterialKind::Bamboo | MaterialKind::Plywood => 600.0,
                    MaterialKind::WoodLight => 450.0,
                    MaterialKind::WoodDark => 750.0,
                    MaterialKind::Metal | MaterialKind::Steel => 7800.0,
                    MaterialKind::Aluminum => 2700.0,
                    MaterialKind::Copper => 8960.0,
                    MaterialKind::Gold => 19300.0,
                    MaterialKind::Brick | MaterialKind::BrickWhite => 1800.0,
                    MaterialKind::Tile | MaterialKind::TileDark => 2300.0,
                    MaterialKind::Glass | MaterialKind::GlassTinted | MaterialKind::GlassFrosted => 2500.0,
                    MaterialKind::Asphalt => 2300.0,
                    MaterialKind::Gravel => 1800.0,
                    MaterialKind::Grass => 1200.0,
                    MaterialKind::Soil => 1500.0,
                    MaterialKind::Plaster => 1700.0,
                    _ => 1000.0,
                };
                let weight_kg = vol / 1_000_000_000.0 * density;
                if weight_kg >= 1000.0 {
                    ui.label(format!("估重: {:.2} t", weight_kg / 1000.0));
                } else {
                    ui.label(format!("估重: {:.1} kg", weight_kg));
                }
            }
        });

        // Component instance sync: if this object is a component instance,
        // update the definition and propagate changes to all other instances.
        let comp_tag = obj.tag.clone();
        if comp_tag.starts_with("元件:") {
            let def_id = comp_tag.strip_prefix("元件:").unwrap_or("").to_string();
            let shape_clone = obj.shape.clone();
            let mat_clone = obj.material.clone();
            // Update the definition with the edited values
            if let Some(def) = self.scene.component_defs.get_mut(&def_id) {
                if let Some(def_obj) = def.objects.first_mut() {
                    def_obj.shape = shape_clone;
                    def_obj.material = mat_clone;
                }
            }
            // Sync all instances
            self.scene.sync_component_instances(&def_id);
        }
    }

    /// AI contextual suggestions panel
    fn ai_suggestions_ui(&mut self, ui: &mut egui::Ui) {
        let suggestions = crate::ai_assist::generate_suggestions(
            &self.scene,
            self.editor.tool,
            &self.editor.selected_ids,
            &self.editor.last_action_name,
        );

        if suggestions.is_empty() {
            return;
        }

        ui.add_space(8.0);
        section_frame_full(ui, |ui| {
            section_header_text(ui, "AI \u{5efa}\u{8b70}");
            for sug in &suggestions {
                ui.horizontal(|ui| {
                    ui.label(sug.icon);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&sug.text).strong().size(12.0));
                        ui.label(
                            egui::RichText::new(&sug.detail)
                                .size(11.0)
                                .color(egui::Color32::from_rgb(110, 118, 135)),
                        );
                    });
                });

                if let Some(action) = &sug.action {
                    match action {
                        crate::ai_assist::SuggestedAction::SwitchTool(tool) => {
                            let t = *tool;
                            if ui.small_button("\u{5957}\u{7528}").clicked() {
                                self.editor.tool = t;
                            }
                        }
                        crate::ai_assist::SuggestedAction::SetDimension {
                            obj_id,
                            axis,
                            value,
                        } => {
                            let oid = obj_id.clone();
                            let ax = *axis;
                            let val = *value;
                            if ui.small_button("\u{5c0d}\u{9f4a}").clicked() {
                                self.scene.snapshot();
                                if let Some(obj) = self.scene.objects.get_mut(&oid) {
                                    obj.position[ax as usize] = val;
                                }
                            }
                        }
                        crate::ai_assist::SuggestedAction::ApplyMaterial {
                            obj_id,
                            material: _,
                        } => {
                            let oid = obj_id.clone();
                            if ui.small_button("\u{5957}\u{7528}").clicked() {
                                if self.scene.objects.contains_key(&oid) {
                                    self.scene.snapshot();
                                    if let Some(obj) = self.scene.objects.get_mut(&oid) {
                                        obj.material = crate::scene::MaterialKind::Brick;
                                    }
                                }
                            }
                        }
                    }
                }

                ui.add_space(4.0);
            }
        });
    }

    pub(crate) fn tab_create(&mut self, ui: &mut egui::Ui) {
        // ── 弧線模式切換（當 Arc/Arc3Point/Pie 工具啟用時顯示）──
        if matches!(self.editor.tool, Tool::Arc | Tool::Arc3Point | Tool::Pie) {
            section_frame_full(ui, |ui| {
                section_header_text(ui, "ARC MODE");
                ui.horizontal(|ui| {
                    let modes = [
                        (Tool::Arc,       "兩點弧"),
                        (Tool::Arc3Point, "三點弧"),
                        (Tool::Pie,       "扇形"),
                    ];
                    for (tool, label) in modes {
                        if ui.selectable_label(self.editor.tool == tool, label).clicked() {
                            self.console_push("TOOL", format!("弧線模式: {}", label));
                            self.editor.tool = tool;
                            self.editor.draw_state = DrawState::Idle;
                        }
                    }
                });
                let desc = match self.editor.tool {
                    Tool::Arc       => "起點 → 終點 → 凸度拖曳（半圓自動鎖定）",
                    Tool::Arc3Point => "任意三點定義圓弧",
                    Tool::Pie       => "中心 → 邊緣定半徑 → 第二邊緣定角度",
                    _ => "",
                };
                ui.small(desc);
            });
            ui.add_space(8.0);
        }

        ui.label(egui::RichText::new("新物件材質").strong());
        // Material preview sphere
        {
            let preview_size = 80.0;
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(preview_size, preview_size), egui::Sense::hover()
            );
            draw_material_preview(ui.painter(), rect, &self.create_mat);
        }
        // SketchUp-style material browser (shared picker)
        if let Some(new_mat) = material_picker_ui(
            ui,
            self.create_mat,
            &mut self.mat_search,
            &mut self.mat_category_idx,
            &mut self.show_custom_color_picker,
        ) {
            self.create_mat = new_mat;
        }

        ui.add_space(12.0);

        // ── 繪圖設定 ──
        section_frame_full(ui, |ui| {
            section_header_text(ui, "DRAWING SETTINGS");

            // 圓柱/球體細分數
            ui.horizontal(|ui| {
                ui.label("圓弧細分");
                // Store segments as a temporary — can't easily change global default yet
                ui.label(egui::RichText::new("32 段").color(egui::Color32::from_rgb(110, 118, 135)));
            });

            ui.add_space(4.0);

            // 快速建立物件（帶預設尺寸）
            ui.label(egui::RichText::new("快速建立").size(11.0).strong());
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if ui.button("方塊 1m").clicked() {
                    let id = self.scene.add_box("QuickBox".into(), [0.0, 0.0, 0.0], 1000.0, 1000.0, 1000.0, self.create_mat);
                    self.editor.selected_ids = vec![id];
                    self.right_tab = RightTab::Properties;
                }
                if ui.button("方塊 3m").clicked() {
                    let id = self.scene.add_box("QuickBox".into(), [0.0, 0.0, 0.0], 3000.0, 3000.0, 3000.0, self.create_mat);
                    self.editor.selected_ids = vec![id];
                    self.right_tab = RightTab::Properties;
                }
            });
            ui.horizontal(|ui| {
                if ui.button("圓柱 r1m").clicked() {
                    let id = self.scene.add_cylinder("QuickCyl".into(), [0.0, 0.0, 0.0], 1000.0, 2000.0, 32, self.create_mat);
                    self.editor.selected_ids = vec![id];
                    self.right_tab = RightTab::Properties;
                }
                if ui.button("球體 r1m").clicked() {
                    let id = self.scene.add_sphere("QuickSphere".into(), [0.0, 0.0, 0.0], 1000.0, 32, self.create_mat);
                    self.editor.selected_ids = vec![id];
                    self.right_tab = RightTab::Properties;
                }
            });
        });

        ui.add_space(12.0);

        // ── 標註樣式設定（CAD style dimension settings）──
        section_frame_full(ui, |ui| {
            section_header_text(ui, "DIMENSION STYLE");

            let ds = &mut self.dim_style;

            ui.horizontal(|ui| {
                ui.label("線粗");
                ui.add(egui::Slider::new(&mut ds.line_thickness, 0.5..=4.0).step_by(0.5).suffix(" px"));
            });
            ui.horizontal(|ui| {
                ui.label("延伸線粗");
                ui.add(egui::Slider::new(&mut ds.ext_line_thickness, 0.25..=3.0).step_by(0.25).suffix(" px"));
            });
            ui.horizontal(|ui| {
                ui.label("文字大小");
                ui.add(egui::Slider::new(&mut ds.text_size, 8.0..=20.0).step_by(1.0).suffix(" px"));
            });
            ui.horizontal(|ui| {
                ui.label("箭頭大小");
                ui.add(egui::Slider::new(&mut ds.arrow_size, 3.0..=15.0).step_by(1.0).suffix(" px"));
            });
            ui.horizontal(|ui| {
                ui.label("偏移量");
                ui.add(egui::Slider::new(&mut ds.offset, 5.0..=50.0).step_by(1.0).suffix(" px"));
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("箭頭樣式");
                let styles = [
                    (crate::dimensions::ArrowStyle::Tick, "短橫"),
                    (crate::dimensions::ArrowStyle::Arrow, "箭頭"),
                    (crate::dimensions::ArrowStyle::Dot, "圓點"),
                    (crate::dimensions::ArrowStyle::None, "無"),
                ];
                for (style, label) in styles {
                    if ui.selectable_label(ds.arrow_style == style, label).clicked() {
                        ds.arrow_style = style;
                    }
                }
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("單位");
                let units = [
                    (crate::dimensions::UnitDisplay::Auto, "自動"),
                    (crate::dimensions::UnitDisplay::Mm, "mm"),
                    (crate::dimensions::UnitDisplay::Cm, "cm"),
                    (crate::dimensions::UnitDisplay::M, "m"),
                ];
                for (unit, label) in units {
                    if ui.selectable_label(ds.unit_display == unit, label).clicked() {
                        ds.unit_display = unit;
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label("小數位");
                ui.add(egui::Slider::new(&mut ds.precision, 0..=3));
            });

            ui.checkbox(&mut ds.show_bg, "顯示文字背景");

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("線條顏色");
                let mut c = egui::Color32::from_rgba_unmultiplied(ds.line_color[0], ds.line_color[1], ds.line_color[2], ds.line_color[3]);
                if ui.color_edit_button_srgba(&mut c).changed() {
                    ds.line_color = [c.r(), c.g(), c.b(), c.a()];
                }
            });
            ui.horizontal(|ui| {
                ui.label("文字顏色");
                let mut c = egui::Color32::from_rgba_unmultiplied(ds.text_color[0], ds.text_color[1], ds.text_color[2], ds.text_color[3]);
                if ui.color_edit_button_srgba(&mut c).changed() {
                    ds.text_color = [c.r(), c.g(), c.b(), c.a()];
                }
            });
        });
    }

    pub(crate) fn tab_ai_log(&mut self, ui: &mut egui::Ui) {
        ui.heading("AI \u{4fee}\u{6539}\u{8a18}\u{9304}");
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("\u{532f}\u{51fa}\u{8a18}\u{9304}").clicked() {
                let _ = self.ai_log.save_to_file("ai_log.json");
                self.file_message = Some(("\u{8a18}\u{9304}\u{5df2}\u{532f}\u{51fa}".into(), std::time::Instant::now()));
            }
            if ui.button("\u{6e05}\u{9664}\u{8a18}\u{9304}").clicked() {
                self.ai_log.clear();
            }
        });
        ui.separator();

        let entries: Vec<_> = self.ai_log.entries().iter().rev().cloned().collect();
        for entry in &entries {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    let color = match entry.actor.name.as_str() {
                        "Claude" => egui::Color32::from_rgb(100, 180, 255),
                        "\u{4f7f}\u{7528}\u{8005}" => egui::Color32::from_rgb(180, 220, 180),
                        _ => egui::Color32::from_rgb(255, 180, 100),
                    };
                    ui.colored_label(color, &entry.actor.display_name());
                    ui.label(&entry.timestamp);
                });
                ui.label(egui::RichText::new(&entry.action).strong());
                if !entry.details.is_empty() {
                    ui.label(&entry.details);
                }
                if !entry.objects_affected.is_empty() {
                    ui.small(format!("\u{7269}\u{4ef6}: {}", entry.objects_affected.join(", ")));
                }
            });
        }

        if entries.is_empty() {
            ui.label("\u{5c1a}\u{7121}\u{8a18}\u{9304}");
        }
    }

    pub(crate) fn tab_scene(&mut self, ui: &mut egui::Ui) {
        // ── PAGES / SCENES ──
        section_header(ui, "PAGES / SCENES");
        figma_group(ui, |ui| {
            let scene_name = self.current_file.as_ref()
                .and_then(|p| p.rsplit(['\\', '/']).next())
                .unwrap_or("Scene 1");
            let obj_count = self.scene.objects.len();

            ui.horizontal(|ui| {
                let (thumb_rect, _) = ui.allocate_exact_size(egui::vec2(48.0, 36.0), egui::Sense::hover());
                ui.painter().rect_filled(thumb_rect, 8.0, egui::Color32::from_rgb(230, 233, 240));
                ui.painter().text(thumb_rect.center(), egui::Align2::CENTER_CENTER,
                    "\u{1f3d7}", egui::FontId::proportional(16.0),
                    egui::Color32::from_rgb(110, 118, 135));

                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(scene_name).strong().size(12.0).color(egui::Color32::from_rgb(31, 36, 48)));
                    ui.label(egui::RichText::new(format!("{} objects", obj_count)).size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
                });
            });
        });

        ui.add_space(8.0);

        // ── QUICK ACTIONS ──
        section_header(ui, "QUICK ACTIONS");
        figma_group(ui, |ui| {
            ui.columns(2, |cols| {
                if cols[0].button(egui::RichText::new("+ \u{65b0}\u{5834}\u{666f}").size(11.0)).clicked() {
                    self.handle_menu_action(crate::menu::MenuAction::NewScene);
                }
                if cols[1].button(egui::RichText::new("\u{1f4c2} \u{958b}\u{555f}").size(11.0)).clicked() {
                    self.handle_menu_action(crate::menu::MenuAction::OpenScene);
                }
            });
            ui.add_space(2.0);
            ui.columns(2, |cols| {
                if cols[0].button(egui::RichText::new("\u{1f4e5} \u{532f}\u{5165} OBJ").size(11.0)).clicked() {
                    self.handle_menu_action(crate::menu::MenuAction::ImportObj);
                }
                if cols[1].button(egui::RichText::new("\u{1f4e4} \u{532f}\u{51fa}").size(11.0)).clicked() {
                    self.handle_menu_action(crate::menu::MenuAction::ExportObj);
                }
            });
            ui.add_space(2.0);
            ui.columns(2, |cols| {
                if cols[0].button(egui::RichText::new("\u{1f4e6} \u{7fa4}\u{7d44}").size(11.0)).clicked() {
                    self.editor.tool = Tool::Group;
                }
                if cols[1].button(egui::RichText::new("\u{1f50d} \u{5168}\u{90e8}\u{986f}\u{793a}").size(11.0)).clicked() {
                    self.zoom_extents();
                }
            });
        });

        ui.add_space(8.0);

        // ── SNAP ──
        section_header(ui, "SNAP");
        figma_group(ui, |ui| {
            ui.columns(3, |cols| {
                cols[0].label(egui::RichText::new("\u{25cf} \u{7aef}\u{9ede}").size(10.0).color(egui::Color32::from_rgb(31, 36, 48)));
                cols[1].label(egui::RichText::new("\u{25cb} \u{4e2d}\u{9ede}").size(10.0).color(egui::Color32::from_rgb(31, 36, 48)));
                cols[2].label(egui::RichText::new("\u{2716} \u{4ea4}\u{9ede}").size(10.0).color(egui::Color32::from_rgb(31, 36, 48)));
            });
        });

        ui.add_space(8.0);

        // ── LAYERS / OBJECTS ──
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("場景物件").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.small(format!("共 {}", self.scene.objects.len()));
            });
        });
        ui.separator();

        // Layer/tag filter section
        {
            let tags: Vec<String> = {
                let mut set = std::collections::BTreeSet::new();
                for o in self.scene.objects.values() {
                    set.insert(o.tag.clone());
                }
                set.into_iter().collect()
            };
            if !tags.is_empty() {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("圖層").strong());
                    for tag in &tags {
                        let visible = !self.viewer.hidden_tags.contains(tag);
                        let label = if visible { format!("\u{1f441} {}", tag) } else { format!("   {}", tag) };
                        if ui.selectable_label(visible, &label).clicked() {
                            if visible { self.viewer.hidden_tags.insert(tag.clone()); }
                            else { self.viewer.hidden_tags.remove(tag); }
                        }
                    }
                });
                ui.separator();
            }
        }

        // Groups
        if !self.scene.groups.is_empty() {
            ui.label(egui::RichText::new("群組").strong());
            let groups: Vec<_> = self.scene.groups.values().cloned().collect();
            let mut dissolve_id = None;
            for g in &groups {
                ui.horizontal(|ui| {
                    let label = format!("\u{1f4c1} {} ({} 物件)", g.name, g.children.len());
                    if ui.selectable_label(false, &label).clicked() {
                        // Select all children
                        self.editor.selected_ids = g.children.clone();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("解散").clicked() {
                            dissolve_id = Some(g.id.clone());
                        }
                    });
                });
            }
            if let Some(gid) = dissolve_id {
                self.scene.dissolve_group(&gid);
            }
            ui.separator();
        }

        // Component definitions
        if !self.scene.component_defs.is_empty() {
            ui.label(egui::RichText::new("元件定義").strong());
            let defs: Vec<_> = self.scene.component_defs.values().cloned().collect();
            for def in &defs {
                ui.horizontal(|ui| {
                    let instance_count = self.scene.objects.values()
                        .filter(|o| o.tag == format!("元件:{}", def.id))
                        .count();
                    let label = format!("\u{1f537} {} ({} 個實例)", def.name, instance_count);
                    ui.label(&label);
                });
            }
            ui.separator();
        }

        if self.scene.objects.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                ui.label(egui::RichText::new("場景為空").color(egui::Color32::GRAY));
            });
            return;
        }

        let items: Vec<(String, String, String)> = self.scene.objects.values()
            .map(|o| {
                let icon = match &o.shape { Shape::Box{..}=>"⬜", Shape::Cylinder{..}=>"○", Shape::Sphere{..}=>"◎", Shape::Line{..}=>"╱", Shape::Mesh{..}=>"◇" };
                (o.id.clone(), o.name.clone(), icon.to_string())
            }).collect();

        let mut to_delete = None;
        for (oid, name, icon) in &items {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.editor.selected_ids.iter().any(|s| s == oid), format!("{} {}", icon, name)).clicked() {
                    self.editor.selected_ids = vec![oid.clone()];
                    self.right_tab = RightTab::Properties;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("✕").clicked() { to_delete = Some(oid.clone()); }
                });
            });
        }
        if let Some(id) = to_delete {
            self.editor.selected_ids.retain(|s| s != &id);
            self.scene.delete(&id);
        }

        ui.separator();
        if ui.button("🧹 清空").clicked() { self.scene.clear(); self.editor.selected_ids.clear(); }
    }

    pub(crate) fn status_text(&self) -> String {
        let snap_info = if let Some(ref snap) = self.editor.snap_result {
            let label = snap.snap_type.label();
            if !label.is_empty() && !matches!(self.editor.draw_state, DrawState::Idle) {
                format!("  [{}]", label)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let base = match &self.editor.draw_state {
            DrawState::Idle => match self.editor.tool {
                Tool::Select      => "選取 — 點擊選取物件, 左鍵拖曳旋轉, 中鍵平移".into(),
                Tool::Move        => "移動 — 選取物件後拖曳移動".into(),
                Tool::Rotate      => "旋轉 — 點擊物件旋轉90度 (Q)".into(),
                Tool::Scale       => "縮放 — 點擊物件後上下拖曳等比縮放 (S)".into(),
                Tool::Line        => "線段 — 點擊設定起點, 再點擊設定終點".into(),
                Tool::Arc         => "弧線 — 起點→終點→凸度（真圓弧，半圓自動鎖定）(A)".into(),
                Tool::Arc3Point   => "三點圓弧 — 任意三點定義圓弧".into(),
                Tool::Pie         => "扇形 — 中心→邊緣定半徑→第二邊緣定角度".into(),
                Tool::Rectangle   => "矩形 — 點擊地面設定第一角, 等同方塊底面".into(),
                Tool::Circle      => "圓形 — 點擊地面設定圓心, 等同圓柱底面".into(),
                Tool::CreateBox   => "方塊 — 點擊地面設定第一角".into(),
                Tool::CreateCylinder => "圓柱 — 點擊地面設定圓心".into(),
                Tool::CreateSphere   => "球體 — 點擊地面設定圓心".into(),
                Tool::PushPull    => "推拉 — 點擊物件的面，拖曳沿法線方向拉伸 (P)".into(),
                Tool::Offset      => "偏移複製 — 點擊物件，再點擊地面放置複製品 (F)".into(),
                Tool::FollowMe    => "跟隨複製 — 點擊物件，自動複製並切換移動工具".into(),
                Tool::TapeMeasure => "捲尺 — 點擊兩點量測距離".into(),
                Tool::Dimension   => "標註 — 點擊兩點建立持久標註 (D)".into(),
                Tool::Text        => "文字 — 點擊放置文字標籤".into(),
                Tool::PaintBucket => "油漆桶 — 點擊物件套用目前材質".into(),
                Tool::Orbit       => "環繞 — 左鍵拖曳旋轉視角, WASD走動".into(),
                Tool::Pan         => "平移 — 左鍵拖曳平移視角".into(),
                Tool::ZoomExtents => "全部顯示".into(),
                Tool::Group       => "群組 — 點擊物件標記為群組".into(),
                Tool::Component   => "元件 — 點擊物件標記為可重複使用的元件".into(),
                Tool::Eraser      => "橡皮擦 — 點擊物件刪除".into(),
                Tool::SteelGrid   => "軸線 — 點擊放置軸線".into(),
                Tool::SteelColumn => format!("柱 — 點擊放置 {} 柱", self.editor.steel_profile),
                Tool::SteelBeam   => "梁 — 點擊起點，再點擊終點".into(),
                Tool::SteelBrace  => "斜撐 — 點擊起點，再點擊終點".into(),
                Tool::SteelPlate  => "鋼板 — 畫矩形，再推拉厚度".into(),
                Tool::SteelConnection => "接頭 — 選取兩個構件".into(),
            },
            DrawState::BoxBase { .. } => "移動滑鼠拖出底面矩形, 點擊確認".into(),
            DrawState::BoxHeight { .. } => "上下移動設定高度, 點擊確認 (或輸入數字+Enter)".into(),
            DrawState::CylBase { .. } => "移動滑鼠拖出半徑, 點擊確認".into(),
            DrawState::CylHeight { .. } => "上下移動設定高度, 點擊確認".into(),
            DrawState::SphRadius { .. } => "移動滑鼠拖出半徑, 點擊確認".into(),
            DrawState::Pulling { face, .. } => {
                let face_name = match face {
                    PullFace::Top => "頂面", PullFace::Bottom => "底面",
                    PullFace::Front => "前面", PullFace::Back => "後面",
                    PullFace::Left => "左面", PullFace::Right => "右面",
                };
                format!("推拉 {} — 拖曳拉伸, 放開確認", face_name)
            }
            DrawState::LineFrom { .. } => "移動到下一點, 點擊確認 (ESC 結束)".into(),
            DrawState::ArcP1 { .. } => "點擊設定弧線終點".into(),
            DrawState::ArcP2 { .. } => "移動設定弧度（半圓自動鎖定），點擊確認".into(),
            DrawState::PieCenter { .. } => "點擊設定扇形半徑終點".into(),
            DrawState::PieRadius { .. } => "移動設定扇形角度，點擊確認".into(),
            DrawState::RotateRef { .. } => {
                "點擊設定參考方向（0° 線）".into()
            }
            DrawState::RotateAngle { ref_angle, current_angle, .. } => {
                let delta_deg = (current_angle - ref_angle).to_degrees();
                format!("旋轉 {:.1}° — 點擊確認, 輸入角度+Enter 精確旋轉", delta_deg)
            }
            DrawState::Scaling { handle, .. } => {
                let axis = match handle {
                    ScaleHandle::Uniform => "\u{7b49}\u{6bd4}\u{7e2e}\u{653e}",
                    ScaleHandle::AxisX => "X\u{8ef8}\u{7e2e}\u{653e}\u{ff08}\u{5bec}\u{5ea6}\u{ff09}",
                    ScaleHandle::AxisY => "Y\u{8ef8}\u{7e2e}\u{653e}\u{ff08}\u{9ad8}\u{5ea6}\u{ff09}",
                    ScaleHandle::AxisZ => "Z\u{8ef8}\u{7e2e}\u{653e}\u{ff08}\u{6df1}\u{5ea6}\u{ff09}",
                };
                format!("\u{7e2e}\u{653e} \u{2014} {} | \u{8f38}\u{5165}\u{6bd4}\u{4f8b}(x1.5)\u{6216}\u{5c3a}\u{5bf8}(mm)+Enter", axis)
            }
            DrawState::Offsetting { distance, .. } => {
                format!("偏移 {:.0}mm — 拖曳調整距離, 放開確認", distance)
            }
            DrawState::Measuring { start } => {
                if let Some(p2) = self.editor.mouse_ground {
                    let dx = p2[0] - start[0];
                    let dz = p2[2] - start[2];
                    let dist = (dx*dx + dz*dz).sqrt();
                    let dist_text = if dist >= 1000.0 {
                        format!("{:.2} m", dist / 1000.0)
                    } else {
                        format!("{:.0} mm", dist)
                    };
                    let angle_deg = dz.atan2(dx).to_degrees();
                    format!("捲尺 — 距離: {} | 角度: {:.1}° | 點擊確認 / ESC 取消", dist_text, angle_deg)
                } else {
                    "捲尺 — 點擊第二點完成量測 [捕捉中]".to_string()
                }
            }
            DrawState::PullingFreeMesh { .. } => "推拉自由面 — 拖曳拉伸, 放開確認".into(),
            DrawState::FollowPath { path_points, .. } => {
                if path_points.is_empty() {
                    "跟隨 — 點擊地面定義路徑".into()
                } else {
                    format!("跟隨 — 已定義 {} 個路徑點 | Enter 完成", path_points.len())
                }
            }
        };

        let base_text = format!("{}{}", base, snap_info);

        // Append cursor world coordinates
        if let Some(p) = self.editor.mouse_ground {
            format!("{} | X:{:.0} Y:{:.0} Z:{:.0}", base_text, p[0], 0.0, p[2])
        } else {
            base_text
        }
    }
}

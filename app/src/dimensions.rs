use eframe::egui;
use glam::{Vec4, Mat4};

// ─── Dimension Style（可調整的標註樣式）─────────────────────────────────────

/// CAD-style dimension configuration — user can adjust all visual parameters
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DimensionStyle {
    pub line_thickness: f32,      // 線粗 (px), default 1.0
    pub ext_line_thickness: f32,  // 延伸線粗 (px), default 0.5
    pub text_size: f32,           // 文字大小 (px), default 11.0
    pub arrow_size: f32,          // 箭頭大小 (px), default 6.0
    pub offset: f32,              // 偏移量 (px), default 20.0
    pub ext_gap: f32,             // 延伸線起始間距 (px), default 4.0
    pub ext_beyond: f32,          // 延伸線超出量 (px), default 4.0
    pub arrow_style: ArrowStyle,  // 箭頭樣式
    pub line_color: [u8; 4],      // 線條顏色 RGBA
    pub text_color: [u8; 4],      // 文字顏色 RGBA
    pub show_bg: bool,            // 是否顯示文字背景
    pub precision: u8,            // 小數位數 (0-3)
    pub unit_display: UnitDisplay,// 單位顯示方式
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ArrowStyle {
    Tick,       // 短橫線（SU 風格）
    Arrow,      // 實心箭頭（CAD 風格）
    Dot,        // 圓點
    None,       // 無
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UnitDisplay {
    Auto,       // 自動（>=1000mm 顯示 m）
    Mm,         // 永遠 mm
    M,          // 永遠 m
    Cm,         // 永遠 cm
}

impl Default for DimensionStyle {
    fn default() -> Self {
        Self {
            line_thickness: 1.0,
            ext_line_thickness: 0.5,
            text_size: 11.0,
            arrow_size: 6.0,
            offset: 20.0,
            ext_gap: 4.0,
            ext_beyond: 4.0,
            arrow_style: ArrowStyle::Tick,
            line_color: [80, 80, 100, 200],
            text_color: [50, 55, 70, 255],
            show_bg: true,
            precision: 2,
            unit_display: UnitDisplay::Auto,
        }
    }
}

impl DimensionStyle {
    fn format_distance(&self, d: f32) -> String {
        match self.unit_display {
            UnitDisplay::Auto => {
                if d >= 1000.0 {
                    format!("{:.prec$} m", d / 1000.0, prec = self.precision as usize)
                } else {
                    format!("{:.0} mm", d)
                }
            }
            UnitDisplay::Mm => format!("{:.0} mm", d),
            UnitDisplay::M => format!("{:.prec$} m", d / 1000.0, prec = self.precision as usize),
            UnitDisplay::Cm => format!("{:.prec$} cm", d / 10.0, prec = self.precision as usize),
        }
    }

    fn format_angle(&self, deg: f32) -> String {
        format!("{:.1}°", deg)
    }

    fn format_radius(&self, r: f32) -> String {
        let d = match self.unit_display {
            UnitDisplay::Auto => {
                if r >= 1000.0 { format!("{:.prec$} m", r / 1000.0, prec = self.precision as usize) }
                else { format!("{:.0} mm", r) }
            }
            UnitDisplay::Mm => format!("{:.0} mm", r),
            UnitDisplay::M => format!("{:.prec$} m", r / 1000.0, prec = self.precision as usize),
            UnitDisplay::Cm => format!("{:.prec$} cm", r / 10.0, prec = self.precision as usize),
        };
        d
    }

    fn line_stroke(&self) -> egui::Stroke {
        let [r, g, b, a] = self.line_color;
        egui::Stroke::new(self.line_thickness, egui::Color32::from_rgba_unmultiplied(r, g, b, a))
    }

    fn ext_stroke(&self) -> egui::Stroke {
        let [r, g, b, a] = self.line_color;
        egui::Stroke::new(self.ext_line_thickness, egui::Color32::from_rgba_unmultiplied(r, g, b, (a as f32 * 0.7) as u8))
    }

    fn text_egui_color(&self) -> egui::Color32 {
        let [r, g, b, a] = self.text_color;
        egui::Color32::from_rgba_unmultiplied(r, g, b, a)
    }
}

// ─── Dimension Types（標註類型）──────────────────────────────────────────────

/// A dimension annotation — supports linear, radius, diameter, angle, arc
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Dimension {
    pub kind: DimensionKind,
    pub label: Option<String>,  // override label, None = auto
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DimensionKind {
    /// 線性標註：兩點間距離
    Linear {
        start: [f32; 3],
        end: [f32; 3],
    },
    /// 半徑標註：圓心 + 半徑 + 標記方向
    Radius {
        center: [f32; 3],
        radius: f32,
        direction: [f32; 3],  // 標記指向方向（單位向量）
    },
    /// 直徑標註：圓心 + 直徑方向
    Diameter {
        center: [f32; 3],
        radius: f32,
        direction: [f32; 3],
    },
    /// 角度標註：頂點 + 兩條邊方向
    Angle {
        vertex: [f32; 3],
        dir_a: [f32; 3],
        dir_b: [f32; 3],
        radius_px: f32,  // 角度弧線的螢幕半徑
    },
    /// 弧長標註
    ArcLength {
        center: [f32; 3],
        radius: f32,
        start_angle: f32,  // radians
        end_angle: f32,
    },
}

impl Dimension {
    /// 建立線性標註（向下相容）
    pub fn new(start: [f32; 3], end: [f32; 3]) -> Self {
        Self {
            kind: DimensionKind::Linear { start, end },
            label: None,
        }
    }

    pub fn radius(center: [f32; 3], radius: f32, dir: [f32; 3]) -> Self {
        Self {
            kind: DimensionKind::Radius { center, radius, direction: dir },
            label: None,
        }
    }

    pub fn diameter(center: [f32; 3], radius: f32, dir: [f32; 3]) -> Self {
        Self {
            kind: DimensionKind::Diameter { center, radius, direction: dir },
            label: None,
        }
    }

    pub fn angle(vertex: [f32; 3], dir_a: [f32; 3], dir_b: [f32; 3]) -> Self {
        Self {
            kind: DimensionKind::Angle { vertex, dir_a, dir_b, radius_px: 40.0 },
            label: None,
        }
    }

    pub fn distance(&self) -> f32 {
        match &self.kind {
            DimensionKind::Linear { start, end } => {
                let dx = end[0] - start[0];
                let dy = end[1] - start[1];
                let dz = end[2] - start[2];
                (dx*dx + dy*dy + dz*dz).sqrt()
            }
            DimensionKind::Radius { radius, .. } => *radius,
            DimensionKind::Diameter { radius, .. } => radius * 2.0,
            _ => 0.0,
        }
    }

    pub fn label_text(&self, style: &DimensionStyle) -> String {
        if let Some(ref l) = self.label {
            return l.clone();
        }
        match &self.kind {
            DimensionKind::Linear { .. } => style.format_distance(self.distance()),
            DimensionKind::Radius { radius, .. } => format!("R{}", style.format_radius(*radius)),
            DimensionKind::Diameter { radius, .. } => format!("⌀{}", style.format_radius(radius * 2.0)),
            DimensionKind::Angle { dir_a, dir_b, .. } => {
                let a = glam::Vec3::from(*dir_a).normalize();
                let b = glam::Vec3::from(*dir_b).normalize();
                let deg = a.dot(b).clamp(-1.0, 1.0).acos().to_degrees();
                style.format_angle(deg)
            }
            DimensionKind::ArcLength { radius, start_angle, end_angle, .. } => {
                let arc = radius * (end_angle - start_angle).abs();
                style.format_distance(arc)
            }
        }
    }
}

// ─── Projection helper ──────────────────────────────────────────────────────

fn project(world: [f32; 3], vp: Mat4, rect: &egui::Rect) -> Option<egui::Pos2> {
    let clip = vp * Vec4::new(world[0], world[1], world[2], 1.0);
    if clip.w <= 0.0 { return None; }
    let ndc = clip.truncate() / clip.w;
    if ndc.x < -1.5 || ndc.x > 1.5 || ndc.y < -1.5 || ndc.y > 1.5 { return None; }
    let x = rect.min.x + (ndc.x * 0.5 + 0.5) * rect.width();
    let y = rect.min.y + (0.5 - ndc.y * 0.5) * rect.height();
    Some(egui::pos2(x, y))
}

// ─── Drawing ─────────────────────────────────────────────────────────────────

/// Draw all dimension annotations with configurable style
pub fn draw_dimensions(
    painter: &egui::Painter,
    dims: &[Dimension],
    view_proj: Mat4,
    rect: &egui::Rect,
) {
    draw_dimensions_styled(painter, dims, view_proj, rect, &DimensionStyle::default());
}

pub fn draw_dimensions_styled(
    painter: &egui::Painter,
    dims: &[Dimension],
    view_proj: Mat4,
    rect: &egui::Rect,
    style: &DimensionStyle,
) {
    for dim in dims {
        match &dim.kind {
            DimensionKind::Linear { start, end } => {
                draw_linear(painter, *start, *end, dim, view_proj, rect, style);
            }
            DimensionKind::Radius { center, radius, direction } => {
                draw_radius(painter, *center, *radius, *direction, dim, view_proj, rect, style, false);
            }
            DimensionKind::Diameter { center, radius, direction } => {
                draw_radius(painter, *center, *radius, *direction, dim, view_proj, rect, style, true);
            }
            DimensionKind::Angle { vertex, dir_a, dir_b, radius_px } => {
                draw_angle(painter, *vertex, *dir_a, *dir_b, *radius_px, dim, view_proj, rect, style);
            }
            DimensionKind::ArcLength { center, radius, start_angle, end_angle } => {
                draw_arc_length(painter, *center, *radius, *start_angle, *end_angle, dim, view_proj, rect, style);
            }
        }
    }
}

fn draw_linear(
    painter: &egui::Painter,
    start: [f32; 3], end: [f32; 3],
    dim: &Dimension, vp: Mat4, rect: &egui::Rect, style: &DimensionStyle,
) {
    let (s, e) = match (project(start, vp, rect), project(end, vp, rect)) {
        (Some(s), Some(e)) => (s, e),
        _ => return,
    };

    let dx = e.x - s.x;
    let dy = e.y - s.y;
    let screen_dist = (dx * dx + dy * dy).sqrt();
    if screen_dist < 15.0 { return; }

    let dir_x = dx / screen_dist;
    let dir_y = dy / screen_dist;
    let perp_x = -dir_y;
    let perp_y = dir_x;

    let offset = style.offset;
    let s_off = egui::pos2(s.x + perp_x * offset, s.y + perp_y * offset);
    let e_off = egui::pos2(e.x + perp_x * offset, e.y + perp_y * offset);

    // Extension lines
    painter.line_segment(
        [egui::pos2(s.x + perp_x * style.ext_gap, s.y + perp_y * style.ext_gap),
         egui::pos2(s.x + perp_x * (offset + style.ext_beyond), s.y + perp_y * (offset + style.ext_beyond))],
        style.ext_stroke(),
    );
    painter.line_segment(
        [egui::pos2(e.x + perp_x * style.ext_gap, e.y + perp_y * style.ext_gap),
         egui::pos2(e.x + perp_x * (offset + style.ext_beyond), e.y + perp_y * (offset + style.ext_beyond))],
        style.ext_stroke(),
    );

    // Dimension line
    painter.line_segment([s_off, e_off], style.line_stroke());

    // Arrow/tick at endpoints
    draw_endpoint(painter, s_off, perp_x, perp_y, dir_x, dir_y, style, true);
    draw_endpoint(painter, e_off, perp_x, perp_y, dir_x, dir_y, style, false);

    // Label
    let mid = egui::pos2((s_off.x + e_off.x) * 0.5, (s_off.y + e_off.y) * 0.5);
    draw_label(painter, mid, &dim.label_text(style), style);
}

fn draw_radius(
    painter: &egui::Painter,
    center: [f32; 3], radius: f32, direction: [f32; 3],
    dim: &Dimension, vp: Mat4, rect: &egui::Rect, style: &DimensionStyle,
    is_diameter: bool,
) {
    let c = match project(center, vp, rect) { Some(p) => p, None => return };
    let edge_pt = [
        center[0] + direction[0] * radius,
        center[1] + direction[1] * radius,
        center[2] + direction[2] * radius,
    ];
    let e = match project(edge_pt, vp, rect) { Some(p) => p, None => return };

    if is_diameter {
        // 直徑：從一邊畫到另一邊穿過中心
        let opp = [
            center[0] - direction[0] * radius,
            center[1] - direction[1] * radius,
            center[2] - direction[2] * radius,
        ];
        let o = match project(opp, vp, rect) { Some(p) => p, None => return };
        painter.line_segment([o, e], style.line_stroke());

        let dx = e.x - o.x;
        let dy = e.y - o.y;
        let len = (dx*dx + dy*dy).sqrt();
        if len < 15.0 { return; }
        let dir_x = dx / len;
        let dir_y = dy / len;
        let perp_x = -dir_y;
        let perp_y = dir_x;
        draw_endpoint(painter, o, perp_x, perp_y, dir_x, dir_y, style, true);
        draw_endpoint(painter, e, perp_x, perp_y, dir_x, dir_y, style, false);

        let mid = egui::pos2((o.x + e.x) * 0.5, (o.y + e.y) * 0.5 - 12.0);
        draw_label(painter, mid, &dim.label_text(style), style);
    } else {
        // 半徑：從中心到邊緣
        painter.line_segment([c, e], style.line_stroke());

        let dx = e.x - c.x;
        let dy = e.y - c.y;
        let len = (dx*dx + dy*dy).sqrt();
        if len < 15.0 { return; }
        let dir_x = dx / len;
        let dir_y = dy / len;
        let perp_x = -dir_y;
        let perp_y = dir_x;
        draw_endpoint(painter, e, perp_x, perp_y, dir_x, dir_y, style, false);

        // 中心十字
        let cross = 4.0;
        painter.line_segment([egui::pos2(c.x - cross, c.y), egui::pos2(c.x + cross, c.y)], style.line_stroke());
        painter.line_segment([egui::pos2(c.x, c.y - cross), egui::pos2(c.x, c.y + cross)], style.line_stroke());

        let mid = egui::pos2((c.x + e.x) * 0.5, (c.y + e.y) * 0.5 - 12.0);
        draw_label(painter, mid, &dim.label_text(style), style);
    }
}

fn draw_angle(
    painter: &egui::Painter,
    vertex: [f32; 3], dir_a: [f32; 3], dir_b: [f32; 3], radius_px: f32,
    dim: &Dimension, vp: Mat4, rect: &egui::Rect, style: &DimensionStyle,
) {
    let v = match project(vertex, vp, rect) { Some(p) => p, None => return };
    let a_world = [vertex[0] + dir_a[0] * 1000.0, vertex[1] + dir_a[1] * 1000.0, vertex[2] + dir_a[2] * 1000.0];
    let b_world = [vertex[0] + dir_b[0] * 1000.0, vertex[1] + dir_b[1] * 1000.0, vertex[2] + dir_b[2] * 1000.0];
    let a = match project(a_world, vp, rect) { Some(p) => p, None => return };
    let b = match project(b_world, vp, rect) { Some(p) => p, None => return };

    // 計算螢幕空間角度
    let angle_a = (a.y - v.y).atan2(a.x - v.x);
    let angle_b = (b.y - v.y).atan2(b.x - v.x);

    // 畫弧線
    let steps = 24;
    let mut sweep = angle_b - angle_a;
    if sweep > std::f32::consts::PI { sweep -= std::f32::consts::TAU; }
    if sweep < -std::f32::consts::PI { sweep += std::f32::consts::TAU; }

    let mut prev = egui::pos2(v.x + radius_px * angle_a.cos(), v.y + radius_px * angle_a.sin());
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let ang = angle_a + sweep * t;
        let pt = egui::pos2(v.x + radius_px * ang.cos(), v.y + radius_px * ang.sin());
        painter.line_segment([prev, pt], style.line_stroke());
        prev = pt;
    }

    // 角度標籤
    let mid_angle = angle_a + sweep * 0.5;
    let label_pos = egui::pos2(
        v.x + (radius_px + 12.0) * mid_angle.cos(),
        v.y + (radius_px + 12.0) * mid_angle.sin(),
    );
    draw_label(painter, label_pos, &dim.label_text(style), style);
}

fn draw_arc_length(
    painter: &egui::Painter,
    center: [f32; 3], radius: f32, start_angle: f32, end_angle: f32,
    dim: &Dimension, vp: Mat4, rect: &egui::Rect, style: &DimensionStyle,
) {
    let c = match project(center, vp, rect) { Some(p) => p, None => return };
    // 在 XZ 平面上畫弧
    let steps = 24;
    let mut prev: Option<egui::Pos2> = None;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let ang = start_angle + (end_angle - start_angle) * t;
        let world = [
            center[0] + radius * ang.cos(),
            center[1],
            center[2] + radius * ang.sin(),
        ];
        if let Some(pt) = project(world, vp, rect) {
            if let Some(p) = prev {
                painter.line_segment([p, pt], style.line_stroke());
            }
            prev = Some(pt);
        }
    }
    // 弧長標籤
    let mid_angle = (start_angle + end_angle) * 0.5;
    let mid_world = [
        center[0] + (radius + 500.0) * mid_angle.cos(),
        center[1],
        center[2] + (radius + 500.0) * mid_angle.sin(),
    ];
    if let Some(mid) = project(mid_world, vp, rect) {
        draw_label(painter, mid, &dim.label_text(style), style);
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn draw_endpoint(
    painter: &egui::Painter, pos: egui::Pos2,
    perp_x: f32, perp_y: f32, dir_x: f32, dir_y: f32,
    style: &DimensionStyle, is_start: bool,
) {
    let s = style.arrow_size;
    match style.arrow_style {
        ArrowStyle::Tick => {
            painter.line_segment(
                [egui::pos2(pos.x - perp_x * s, pos.y - perp_y * s),
                 egui::pos2(pos.x + perp_x * s, pos.y + perp_y * s)],
                style.line_stroke(),
            );
        }
        ArrowStyle::Arrow => {
            let sign = if is_start { 1.0 } else { -1.0 };
            painter.line_segment([
                pos,
                egui::pos2(pos.x + sign * dir_x * s + perp_x * s * 0.35,
                            pos.y + sign * dir_y * s + perp_y * s * 0.35),
            ], style.line_stroke());
            painter.line_segment([
                pos,
                egui::pos2(pos.x + sign * dir_x * s - perp_x * s * 0.35,
                            pos.y + sign * dir_y * s - perp_y * s * 0.35),
            ], style.line_stroke());
        }
        ArrowStyle::Dot => {
            painter.circle_filled(pos, s * 0.5, style.text_egui_color());
        }
        ArrowStyle::None => {}
    }
}

fn draw_label(painter: &egui::Painter, pos: egui::Pos2, text: &str, style: &DimensionStyle) {
    let font = egui::FontId::proportional(style.text_size);
    let galley = painter.layout_no_wrap(text.to_string(), font, style.text_egui_color());
    let text_rect = egui::Align2::CENTER_CENTER.anchor_size(pos, galley.size());

    if style.show_bg {
        let bg_rect = text_rect.expand2(egui::vec2(5.0, 2.0));
        painter.rect_filled(bg_rect, 6.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210));
        let [r, g, b, a] = style.line_color;
        painter.rect_stroke(bg_rect, 6.0, egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(r, g, b, a)));
    }
    painter.galley(text_rect.min, galley, style.text_egui_color());
}

// ─── Auto-dimension generation for shapes ────────────────────────────────────

/// Generate automatic dimensions for a selected shape (SketchUp/CAD style)
pub fn auto_dims_for_shape(
    shape: &crate::scene::Shape,
    position: [f32; 3],
) -> Vec<Dimension> {
    let p = position;
    match shape {
        crate::scene::Shape::Box { width, height, depth } => {
            vec![
                // 寬度：底邊前方
                Dimension::new([p[0], p[1], p[2]], [p[0] + width, p[1], p[2]]),
                // 高度：右側邊
                Dimension::new([p[0] + width, p[1], p[2]], [p[0] + width, p[1] + height, p[2]]),
                // 深度：底邊右側
                Dimension::new([p[0] + width, p[1], p[2]], [p[0] + width, p[1], p[2] + depth]),
            ]
        }
        crate::scene::Shape::Cylinder { radius, height, .. } => {
            // push_cylinder 用 p 作為底面圓心
            let cx = p[0];
            let cz = p[2];
            let top_y = p[1] + height;
            vec![
                // 直徑標註：頂部圓面，起止點在圓邊上
                Dimension::diameter(
                    [cx, top_y, cz],  // 頂部圓心
                    *radius,
                    [1.0, 0.0, 0.0],  // X 方向穿過圓心
                ),
                // 高度：沿圓柱右側邊緣
                Dimension::new(
                    [cx + radius, p[1], cz],
                    [cx + radius, top_y, cz],
                ),
            ]
        }
        crate::scene::Shape::Sphere { radius, .. } => {
            // 球心在 [p[0], p[1]+r, p[2]]
            let cx = p[0];
            let cy = p[1] + radius;
            let cz = p[2];
            vec![
                // 水平直徑（X 方向，穿過球心）
                Dimension::diameter(
                    [cx, cy, cz],
                    *radius,
                    [1.0, 0.0, 0.0],
                ),
                // 垂直直徑（Y 方向，穿過球心）
                Dimension::diameter(
                    [cx, cy, cz],
                    *radius,
                    [0.0, 1.0, 0.0],
                ),
            ]
        }
        crate::scene::Shape::Line { arc_center: Some(center), arc_radius: Some(radius), arc_angle_deg, .. } => {
            let mut dims = vec![
                // 半徑標註（從圓心到弧上）
                Dimension::radius(*center, *radius, [1.0, 0.0, 0.0]),
            ];
            // 角度標註（如果有角度資訊）
            if let Some(deg) = arc_angle_deg {
                let mut d = Dimension::angle(
                    *center,
                    [1.0, 0.0, 0.0],
                    [deg.to_radians().cos(), 0.0, deg.to_radians().sin()],
                );
                d.label = Some(format!("{:.1}°", deg));
                dims.push(d);
            }
            dims
        }
        _ => vec![],
    }
}

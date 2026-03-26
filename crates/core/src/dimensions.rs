//! Dimension data types（純資料，無 GUI 依賴）
//! 繪圖函式留在 app/src/dimensions.rs

use crate::scene::Shape;

// ─── Dimension Style ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DimensionStyle {
    pub line_thickness: f32,
    pub ext_line_thickness: f32,
    pub text_size: f32,
    pub arrow_size: f32,
    pub offset: f32,
    pub ext_gap: f32,
    pub ext_beyond: f32,
    pub arrow_style: ArrowStyle,
    pub line_color: [u8; 4],
    pub text_color: [u8; 4],
    pub show_bg: bool,
    pub precision: u8,
    pub unit_display: UnitDisplay,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ArrowStyle { Tick, Arrow, Dot, None }

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum UnitDisplay { Auto, Mm, M, Cm }

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
    pub fn format_distance(&self, d: f32) -> String {
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

    pub fn format_angle(&self, deg: f32) -> String {
        format!("{:.1}°", deg)
    }

    pub fn format_radius(&self, r: f32) -> String {
        match self.unit_display {
            UnitDisplay::Auto => {
                if r >= 1000.0 { format!("{:.prec$} m", r / 1000.0, prec = self.precision as usize) }
                else { format!("{:.0} mm", r) }
            }
            UnitDisplay::Mm => format!("{:.0} mm", r),
            UnitDisplay::M => format!("{:.prec$} m", r / 1000.0, prec = self.precision as usize),
            UnitDisplay::Cm => format!("{:.prec$} cm", r / 10.0, prec = self.precision as usize),
        }
    }
}

// ─── Dimension Types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Dimension {
    pub kind: DimensionKind,
    pub label: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DimensionKind {
    Linear { start: [f32; 3], end: [f32; 3] },
    Radius { center: [f32; 3], radius: f32, direction: [f32; 3] },
    Diameter { center: [f32; 3], radius: f32, direction: [f32; 3] },
    Angle { vertex: [f32; 3], dir_a: [f32; 3], dir_b: [f32; 3], radius_px: f32 },
    ArcLength { center: [f32; 3], radius: f32, start_angle: f32, end_angle: f32 },
}

impl Dimension {
    pub fn new(start: [f32; 3], end: [f32; 3]) -> Self {
        Self { kind: DimensionKind::Linear { start, end }, label: None }
    }
    pub fn radius(center: [f32; 3], radius: f32, dir: [f32; 3]) -> Self {
        Self { kind: DimensionKind::Radius { center, radius, direction: dir }, label: None }
    }
    pub fn diameter(center: [f32; 3], radius: f32, dir: [f32; 3]) -> Self {
        Self { kind: DimensionKind::Diameter { center, radius, direction: dir }, label: None }
    }
    pub fn angle(vertex: [f32; 3], dir_a: [f32; 3], dir_b: [f32; 3]) -> Self {
        Self { kind: DimensionKind::Angle { vertex, dir_a, dir_b, radius_px: 40.0 }, label: None }
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

// ─── Auto-dimension generation ───────────────────────────────────────────────

pub fn auto_dims_for_shape(shape: &Shape, position: [f32; 3]) -> Vec<Dimension> {
    let p = position;
    match shape {
        Shape::Box { width, height, depth } => {
            vec![
                Dimension::new([p[0], p[1], p[2]], [p[0] + width, p[1], p[2]]),
                Dimension::new([p[0] + width, p[1], p[2]], [p[0] + width, p[1] + height, p[2]]),
                Dimension::new([p[0] + width, p[1], p[2]], [p[0] + width, p[1], p[2] + depth]),
            ]
        }
        Shape::Cylinder { radius, height, .. } => {
            let cx = p[0]; let cz = p[2]; let top_y = p[1] + height;
            vec![
                Dimension::diameter([cx, top_y, cz], *radius, [1.0, 0.0, 0.0]),
                Dimension::new([cx + radius, p[1], cz], [cx + radius, top_y, cz]),
            ]
        }
        Shape::Sphere { radius, .. } => {
            let cx = p[0]; let cy = p[1] + radius; let cz = p[2];
            vec![
                Dimension::diameter([cx, cy, cz], *radius, [1.0, 0.0, 0.0]),
                Dimension::diameter([cx, cy, cz], *radius, [0.0, 1.0, 0.0]),
            ]
        }
        Shape::Line { arc_center: Some(center), arc_radius: Some(radius), arc_angle_deg, .. } => {
            let mut dims = vec![Dimension::radius(*center, *radius, [1.0, 0.0, 0.0])];
            if let Some(deg) = arc_angle_deg {
                let mut d = Dimension::angle(
                    *center, [1.0, 0.0, 0.0],
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

//! Measurement utilities: area, volume, protractor

use crate::scene::{SceneObject, Shape};

/// Calculate surface area of a shape (in mm²)
pub fn surface_area(obj: &SceneObject) -> f64 {
    match &obj.shape {
        Shape::Box { width, height, depth } => {
            let (w, h, d) = (*width as f64, *height as f64, *depth as f64);
            2.0 * (w * h + w * d + h * d)
        }
        Shape::Cylinder { radius, height, segments: _ } => {
            let (r, h) = (*radius as f64, *height as f64);
            let pi = std::f64::consts::PI;
            2.0 * pi * r * r + 2.0 * pi * r * h
        }
        Shape::Sphere { radius, .. } => {
            let r = *radius as f64;
            4.0 * std::f64::consts::PI * r * r
        }
        Shape::Line { points, thickness, .. } => {
            let t = *thickness as f64;
            let mut len = 0.0f64;
            for pair in points.windows(2) {
                let dx = (pair[1][0] - pair[0][0]) as f64;
                let dy = (pair[1][1] - pair[0][1]) as f64;
                let dz = (pair[1][2] - pair[0][2]) as f64;
                len += (dx * dx + dy * dy + dz * dz).sqrt();
            }
            len * t * 4.0 // approximate surface of line-as-tube
        }
        Shape::Mesh(_) => 0.0,
        Shape::SteelProfile { params, length, .. } => {
            // 截面周長 × 長度 + 2 × 截面積（近似）
            let a = params.area() as f64;
            let l = *length as f64;
            let perim = (2.0 * (params.h + params.b)) as f64; // 近似周長
            perim * l + 2.0 * a
        }
    }
}

/// Calculate volume of a shape (in mm³)
pub fn volume(obj: &SceneObject) -> f64 {
    match &obj.shape {
        Shape::Box { width, height, depth } => {
            (*width as f64) * (*height as f64) * (*depth as f64)
        }
        Shape::Cylinder { radius, height, .. } => {
            std::f64::consts::PI * (*radius as f64).powi(2) * (*height as f64)
        }
        Shape::Sphere { radius, .. } => {
            (4.0 / 3.0) * std::f64::consts::PI * (*radius as f64).powi(3)
        }
        Shape::Line { .. } => 0.0,
        Shape::Mesh(_) => 0.0,
        Shape::SteelProfile { params, length, .. } => {
            params.area() as f64 * *length as f64
        }
    }
}

/// Format area for display
pub fn format_area(area_mm2: f64) -> String {
    if area_mm2 >= 1_000_000.0 {
        format!("{:.2} m\u{00B2}", area_mm2 / 1_000_000.0)
    } else {
        format!("{:.0} mm\u{00B2}", area_mm2)
    }
}

/// Format volume for display
pub fn format_volume(vol_mm3: f64) -> String {
    if vol_mm3 >= 1_000_000_000.0 {
        format!("{:.3} m\u{00B3}", vol_mm3 / 1_000_000_000.0)
    } else if vol_mm3 >= 1_000_000.0 {
        format!("{:.2} L", vol_mm3 / 1_000_000.0)
    } else {
        format!("{:.0} mm\u{00B3}", vol_mm3)
    }
}

/// Calculate angle between two 3D vectors (in degrees)
/// Angle at point b, between vectors ba and bc
pub fn angle_between(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    let ba = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let bc = [c[0] - b[0], c[1] - b[1], c[2] - b[2]];
    let dot = ba[0] * bc[0] + ba[1] * bc[1] + ba[2] * bc[2];
    let len_ba = (ba[0] * ba[0] + ba[1] * ba[1] + ba[2] * ba[2]).sqrt();
    let len_bc = (bc[0] * bc[0] + bc[1] * bc[1] + bc[2] * bc[2]).sqrt();
    if len_ba < 0.001 || len_bc < 0.001 {
        return 0.0;
    }
    let cos_angle = (dot / (len_ba * len_bc)).clamp(-1.0, 1.0);
    cos_angle.acos().to_degrees()
}

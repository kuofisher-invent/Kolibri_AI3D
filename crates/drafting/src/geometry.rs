//! 2D 幾何運算 — offset, trim, fillet, mirror, array

use crate::entities::{Point2, DraftEntity};

/// 線段 offset（平移）
pub fn offset_line(start: &Point2, end: &Point2, distance: f64) -> (Point2, Point2) {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-10 {
        return (*start, *end);
    }
    // 法向量（左側）
    let nx = -dy / len * distance;
    let ny = dx / len * distance;
    (
        [start[0] + nx, start[1] + ny],
        [end[0] + nx, end[1] + ny],
    )
}

/// 圓弧 offset（調整半徑）
pub fn offset_arc(_center: &Point2, radius: f64, distance: f64, outward: bool) -> f64 {
    if outward {
        (radius + distance).max(0.0)
    } else {
        (radius - distance).max(0.0)
    }
}

/// 圓 offset
pub fn offset_circle(radius: f64, distance: f64, outward: bool) -> f64 {
    offset_arc(&[0.0, 0.0], radius, distance, outward)
}

/// 兩線段交點
pub fn line_intersection(
    a1: &Point2, a2: &Point2,
    b1: &Point2, b2: &Point2,
) -> Option<Point2> {
    let d1x = a2[0] - a1[0];
    let d1y = a2[1] - a1[1];
    let d2x = b2[0] - b1[0];
    let d2y = b2[1] - b1[1];

    let denom = d1x * d2y - d1y * d2x;
    if denom.abs() < 1e-10 {
        return None; // 平行
    }

    let t = ((b1[0] - a1[0]) * d2y - (b1[1] - a1[1]) * d2x) / denom;
    Some([a1[0] + t * d1x, a1[1] + t * d1y])
}

/// 點到線段的最近點
pub fn point_to_line_nearest(p: &Point2, a: &Point2, b: &Point2) -> Point2 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-10 {
        return *a;
    }
    let t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    [a[0] + t * dx, a[1] + t * dy]
}

/// 鏡射點
pub fn mirror_point(p: &Point2, axis_a: &Point2, axis_b: &Point2) -> Point2 {
    let dx = axis_b[0] - axis_a[0];
    let dy = axis_b[1] - axis_a[1];
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-10 {
        return *p;
    }
    let t = ((p[0] - axis_a[0]) * dx + (p[1] - axis_a[1]) * dy) / len_sq;
    let foot = [axis_a[0] + t * dx, axis_a[1] + t * dy];
    [2.0 * foot[0] - p[0], 2.0 * foot[1] - p[1]]
}

/// 鏡射實體
pub fn mirror_entity(entity: &DraftEntity, axis_a: &Point2, axis_b: &Point2) -> DraftEntity {
    match entity {
        DraftEntity::Line { start, end } => DraftEntity::Line {
            start: mirror_point(start, axis_a, axis_b),
            end: mirror_point(end, axis_a, axis_b),
        },
        DraftEntity::Circle { center, radius } => DraftEntity::Circle {
            center: mirror_point(center, axis_a, axis_b),
            radius: *radius,
        },
        DraftEntity::Arc { center, radius, start_angle, end_angle } => {
            let mc = mirror_point(center, axis_a, axis_b);
            // 鏡射後角度需翻轉
            let axis_angle = (axis_b[1] - axis_a[1]).atan2(axis_b[0] - axis_a[0]);
            let new_start = 2.0 * axis_angle - end_angle;
            let new_end = 2.0 * axis_angle - start_angle;
            DraftEntity::Arc {
                center: mc,
                radius: *radius,
                start_angle: new_start,
                end_angle: new_end,
            }
        },
        DraftEntity::Rectangle { p1, p2 } => DraftEntity::Rectangle {
            p1: mirror_point(p1, axis_a, axis_b),
            p2: mirror_point(p2, axis_a, axis_b),
        },
        other => other.clone(),
    }
}

/// 旋轉點
pub fn rotate_point(p: &Point2, center: &Point2, angle: f64) -> Point2 {
    let cos = angle.cos();
    let sin = angle.sin();
    let dx = p[0] - center[0];
    let dy = p[1] - center[1];
    [
        center[0] + dx * cos - dy * sin,
        center[1] + dx * sin + dy * cos,
    ]
}

/// 線性陣列（copy count 次，每次位移 dx, dy）
pub fn linear_array(entity: &DraftEntity, dx: f64, dy: f64, count: usize) -> Vec<DraftEntity> {
    (1..=count).map(|i| {
        translate_entity(entity, dx * i as f64, dy * i as f64)
    }).collect()
}

/// 平移實體
pub fn translate_entity(entity: &DraftEntity, dx: f64, dy: f64) -> DraftEntity {
    match entity {
        DraftEntity::Line { start, end } => DraftEntity::Line {
            start: [start[0] + dx, start[1] + dy],
            end: [end[0] + dx, end[1] + dy],
        },
        DraftEntity::Circle { center, radius } => DraftEntity::Circle {
            center: [center[0] + dx, center[1] + dy],
            radius: *radius,
        },
        DraftEntity::Arc { center, radius, start_angle, end_angle } => DraftEntity::Arc {
            center: [center[0] + dx, center[1] + dy],
            radius: *radius,
            start_angle: *start_angle,
            end_angle: *end_angle,
        },
        DraftEntity::Rectangle { p1, p2 } => DraftEntity::Rectangle {
            p1: [p1[0] + dx, p1[1] + dy],
            p2: [p2[0] + dx, p2[1] + dy],
        },
        DraftEntity::Text { position, content, height, rotation } => DraftEntity::Text {
            position: [position[0] + dx, position[1] + dy],
            content: content.clone(),
            height: *height,
            rotation: *rotation,
        },
        other => other.clone(),
    }
}

/// 圓角（Fillet）：兩線段交角處插入圓弧
/// 回傳修剪後的兩線段 + 圓弧 entity
pub fn fillet_lines(
    a1: &Point2, a2: &Point2,
    b1: &Point2, b2: &Point2,
    radius: f64,
) -> Option<(DraftEntity, DraftEntity, DraftEntity)> {
    let ix = line_intersection(a1, a2, b1, b2)?;
    if radius < 1e-6 {
        return None;
    }
    // 方向向量
    let da = [(a1[0] - ix[0]), (a1[1] - ix[1])];
    let db = [(b2[0] - ix[0]), (b2[1] - ix[1])];
    let la = (da[0] * da[0] + da[1] * da[1]).sqrt();
    let lb = (db[0] * db[0] + db[1] * db[1]).sqrt();
    if la < 1e-6 || lb < 1e-6 { return None; }
    let ua = [da[0] / la, da[1] / la];
    let ub = [db[0] / lb, db[1] / lb];

    // 角平分線方向
    let bisect = [ua[0] + ub[0], ua[1] + ub[1]];
    let bl = (bisect[0] * bisect[0] + bisect[1] * bisect[1]).sqrt();
    if bl < 1e-6 { return None; }

    // 半角
    let half_angle = ((ua[0] * ub[0] + ua[1] * ub[1]).max(-1.0).min(1.0)).acos() / 2.0;
    let tan_half = half_angle.tan();
    if tan_half.abs() < 1e-6 { return None; }
    let trim_dist = radius / tan_half;

    // 修剪點
    let ta = [ix[0] + ua[0] * trim_dist, ix[1] + ua[1] * trim_dist];
    let tb = [ix[0] + ub[0] * trim_dist, ix[1] + ub[1] * trim_dist];

    // 圓弧中心
    let center_dist = radius / half_angle.sin();
    let center = [ix[0] + bisect[0] / bl * center_dist, ix[1] + bisect[1] / bl * center_dist];

    let sa = (ta[1] - center[1]).atan2(ta[0] - center[0]);
    let ea = (tb[1] - center[1]).atan2(tb[0] - center[0]);

    Some((
        DraftEntity::Line { start: *a1, end: ta },
        DraftEntity::Line { start: tb, end: *b2 },
        DraftEntity::Arc { center, radius, start_angle: sa, end_angle: ea },
    ))
}

/// 倒角（Chamfer）：兩線段交角處插入斜線
pub fn chamfer_lines(
    a1: &Point2, a2: &Point2,
    b1: &Point2, b2: &Point2,
    dist_a: f64, dist_b: f64,
) -> Option<(DraftEntity, DraftEntity, DraftEntity)> {
    let ix = line_intersection(a1, a2, b1, b2)?;

    let da = [(a1[0] - ix[0]), (a1[1] - ix[1])];
    let db = [(b2[0] - ix[0]), (b2[1] - ix[1])];
    let la = (da[0] * da[0] + da[1] * da[1]).sqrt();
    let lb = (db[0] * db[0] + db[1] * db[1]).sqrt();
    if la < 1e-6 || lb < 1e-6 { return None; }

    let ta = [ix[0] + da[0] / la * dist_a, ix[1] + da[1] / la * dist_a];
    let tb = [ix[0] + db[0] / lb * dist_b, ix[1] + db[1] / lb * dist_b];

    Some((
        DraftEntity::Line { start: *a1, end: ta },
        DraftEntity::Line { start: tb, end: *b2 },
        DraftEntity::Line { start: ta, end: tb }, // chamfer line
    ))
}

/// 生成正多邊形頂點
pub fn polygon_points(center: &Point2, radius: f64, sides: u32, inscribed: bool) -> Vec<Point2> {
    let n = sides.max(3) as usize;
    let r = if inscribed { radius } else { radius / (std::f64::consts::PI / n as f64).cos() };
    (0..n).map(|i| {
        let angle = std::f64::consts::TAU * i as f64 / n as f64 - std::f64::consts::FRAC_PI_2;
        [center[0] + r * angle.cos(), center[1] + r * angle.sin()]
    }).collect()
}

/// 雲形線（Catmull-Rom 近似）：回傳平滑曲線上的取樣點
pub fn spline_interpolate(control_points: &[Point2], segments_per_span: usize) -> Vec<Point2> {
    if control_points.len() < 2 { return control_points.to_vec(); }
    let n = control_points.len();
    let segs = segments_per_span.max(4);
    let mut result = Vec::new();

    for i in 0..n - 1 {
        let p0 = if i > 0 { control_points[i - 1] } else { control_points[i] };
        let p1 = control_points[i];
        let p2 = control_points[i + 1];
        let p3 = if i + 2 < n { control_points[i + 2] } else { control_points[i + 1] };

        for j in 0..segs {
            let t = j as f64 / segs as f64;
            let t2 = t * t;
            let t3 = t2 * t;
            let x = 0.5 * ((2.0 * p1[0])
                + (-p0[0] + p2[0]) * t
                + (2.0 * p0[0] - 5.0 * p1[0] + 4.0 * p2[0] - p3[0]) * t2
                + (-p0[0] + 3.0 * p1[0] - 3.0 * p2[0] + p3[0]) * t3);
            let y = 0.5 * ((2.0 * p1[1])
                + (-p0[1] + p2[1]) * t
                + (2.0 * p0[1] - 5.0 * p1[1] + 4.0 * p2[1] - p3[1]) * t2
                + (-p0[1] + 3.0 * p1[1] - 3.0 * p2[1] + p3[1]) * t3);
            result.push([x, y]);
        }
    }
    result.push(*control_points.last().unwrap());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_line() {
        let (a, b) = offset_line(&[0.0, 0.0], &[10.0, 0.0], 5.0);
        assert!((a[1] - 5.0).abs() < 1e-6);
        assert!((b[1] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_line_intersection() {
        let p = line_intersection(
            &[0.0, 0.0], &[10.0, 10.0],
            &[10.0, 0.0], &[0.0, 10.0],
        ).unwrap();
        assert!((p[0] - 5.0).abs() < 1e-6);
        assert!((p[1] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_mirror_point() {
        let p = mirror_point(&[1.0, 2.0], &[0.0, 0.0], &[1.0, 0.0]);
        assert!((p[0] - 1.0).abs() < 1e-6);
        assert!((p[1] - (-2.0)).abs() < 1e-6);
    }
}

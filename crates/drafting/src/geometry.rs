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

/// Trim：裁剪線段到最近的交點（從 click_point 側裁掉）
/// 回傳裁剪後的線段，如果無交點則回傳 None
pub fn trim_line_at_boundary(
    line_start: &Point2, line_end: &Point2,
    cutting_entities: &[DraftEntity],
    click_point: &Point2,
) -> Option<DraftEntity> {
    // 找所有交點
    let mut intersections: Vec<(f64, Point2)> = Vec::new();
    let dx = line_end[0] - line_start[0];
    let dy = line_end[1] - line_start[1];
    let line_len = (dx * dx + dy * dy).sqrt();
    if line_len < 1e-10 { return None; }

    for entity in cutting_entities {
        match entity {
            DraftEntity::Line { start: cs, end: ce } => {
                if let Some(ix) = line_intersection(line_start, line_end, cs, ce) {
                    // 檢查交點是否在兩條線段上
                    let t_line = ((ix[0] - line_start[0]) * dx + (ix[1] - line_start[1]) * dy) / (line_len * line_len);
                    let t_cut_dx = ce[0] - cs[0];
                    let t_cut_dy = ce[1] - cs[1];
                    let t_cut_len = (t_cut_dx * t_cut_dx + t_cut_dy * t_cut_dy).sqrt();
                    let t_cut = if t_cut_len > 1e-10 {
                        ((ix[0] - cs[0]) * t_cut_dx + (ix[1] - cs[1]) * t_cut_dy) / (t_cut_len * t_cut_len)
                    } else { -1.0 };

                    if t_line > 0.01 && t_line < 0.99 && t_cut > -0.01 && t_cut < 1.01 {
                        intersections.push((t_line, ix));
                    }
                }
            }
            DraftEntity::Circle { center, radius } => {
                // 線段與圓的交點
                let ex = line_start[0] - center[0];
                let ey = line_start[1] - center[1];
                let ux = dx / line_len;
                let uy = dy / line_len;
                let a = 1.0;
                let b = 2.0 * (ex * ux + ey * uy);
                let c = ex * ex + ey * ey - radius * radius;
                let disc = b * b - 4.0 * a * c;
                if disc >= 0.0 {
                    for sign in &[1.0_f64, -1.0] {
                        let t = (-b + sign * disc.sqrt()) / (2.0 * a);
                        let t_norm = t / line_len;
                        if t_norm > 0.01 && t_norm < 0.99 {
                            intersections.push((t_norm, [line_start[0] + ux * t, line_start[1] + uy * t]));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if intersections.is_empty() { return None; }

    // click_point 距離 line_start 的 t 值
    let click_t = ((click_point[0] - line_start[0]) * dx + (click_point[1] - line_start[1]) * dy) / (line_len * line_len);

    // 找最近的交點（在 click 側）
    intersections.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    if click_t < 0.5 {
        // 點擊在 start 側 → 找第一個交點，保留交點到 end
        let (_, ix) = intersections[0];
        Some(DraftEntity::Line { start: ix, end: *line_end })
    } else {
        // 點擊在 end 側 → 找最後一個交點，保留 start 到交點
        let (_, ix) = intersections.last().unwrap();
        Some(DraftEntity::Line { start: *line_start, end: *ix })
    }
}

/// Extend：延伸線段到最近的邊界實體
pub fn extend_line_to_boundary(
    line_start: &Point2, line_end: &Point2,
    boundary_entities: &[DraftEntity],
) -> Option<DraftEntity> {
    let dx = line_end[0] - line_start[0];
    let dy = line_end[1] - line_start[1];
    let line_len = (dx * dx + dy * dy).sqrt();
    if line_len < 1e-10 { return None; }
    let ux = dx / line_len;
    let uy = dy / line_len;

    // 延伸 end 方向，找最近的邊界交點
    let ext_end = [line_end[0] + ux * 100000.0, line_end[1] + uy * 100000.0];
    let mut best_t = f64::MAX;
    let mut best_ix = None;

    for entity in boundary_entities {
        if let DraftEntity::Line { start: bs, end: be } = entity {
            if let Some(ix) = line_intersection(line_end, &ext_end, bs, be) {
                let t = ((ix[0] - line_end[0]) * ux + (ix[1] - line_end[1]) * uy);
                // 只取正方向（延伸方向）
                if t > 0.1 && t < best_t {
                    // 檢查交點在邊界線段上
                    let bdx = be[0] - bs[0];
                    let bdy = be[1] - bs[1];
                    let bl = (bdx * bdx + bdy * bdy).sqrt();
                    if bl > 1e-10 {
                        let bt = ((ix[0] - bs[0]) * bdx + (ix[1] - bs[1]) * bdy) / (bl * bl);
                        if bt > -0.01 && bt < 1.01 {
                            best_t = t;
                            best_ix = Some(ix);
                        }
                    }
                }
            }
        }
    }

    best_ix.map(|ix| DraftEntity::Line { start: *line_start, end: ix })
}

/// Offset entity：產生偏移複製
pub fn offset_entity(entity: &DraftEntity, distance: f64) -> Option<DraftEntity> {
    match entity {
        DraftEntity::Line { start, end } => {
            let (new_start, new_end) = offset_line(start, end, distance);
            Some(DraftEntity::Line { start: new_start, end: new_end })
        }
        DraftEntity::Circle { center, radius } => {
            let new_r = (radius + distance).max(0.1);
            Some(DraftEntity::Circle { center: *center, radius: new_r })
        }
        DraftEntity::Arc { center, radius, start_angle, end_angle } => {
            let new_r = (radius + distance).max(0.1);
            Some(DraftEntity::Arc { center: *center, radius: new_r, start_angle: *start_angle, end_angle: *end_angle })
        }
        DraftEntity::Rectangle { p1, p2 } => {
            // 矩形偏移：內縮或外擴
            let cx = (p1[0] + p2[0]) / 2.0;
            let cy = (p1[1] + p2[1]) / 2.0;
            let hw = ((p2[0] - p1[0]).abs() / 2.0 + distance).max(0.1);
            let hh = ((p2[1] - p1[1]).abs() / 2.0 + distance).max(0.1);
            Some(DraftEntity::Rectangle {
                p1: [cx - hw, cy - hh],
                p2: [cx + hw, cy + hh],
            })
        }
        DraftEntity::Polyline { points, closed } => {
            // 每段偏移
            if points.len() < 2 { return None; }
            let mut new_pts = Vec::new();
            for w in points.windows(2) {
                let (a, b) = offset_line(&w[0], &w[1], distance);
                if new_pts.is_empty() { new_pts.push(a); }
                new_pts.push(b);
            }
            Some(DraftEntity::Polyline { points: new_pts, closed: *closed })
        }
        _ => None,
    }
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

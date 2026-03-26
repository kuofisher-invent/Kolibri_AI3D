//! Geometry Preprocessor — clean, deduplicate, classify raw entities
//! Reduces 17,902 entities to ~500 meaningful candidates

use super::geometry_parser::*;

#[derive(Debug, Default)]
pub struct PreprocessResult {
    pub long_h_lines: Vec<ClassifiedLine>,    // horizontal, >500mm
    pub long_v_lines: Vec<ClassifiedLine>,    // vertical, >500mm
    pub short_lines: usize,                    // removed count
    pub closed_contours: Vec<ClosedContour>,   // closed polylines/rectangles
    pub texts_structural: Vec<RawText>,        // likely structural labels
    pub texts_dimension: Vec<RawText>,         // numeric dimension values
    pub texts_noise: usize,                    // removed count
    pub dimensions: Vec<RawDimension>,
    pub frame_rect: Option<[f64; 4]>,          // detected drawing frame [x,y,w,h]
    pub drawing_regions: Vec<DrawingRegion>,    // separated drawing areas
    pub dedup_count: usize,                    // merged duplicate lines
    pub debug: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ClassifiedLine {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub length: f64,
    pub angle: f64,        // radians from horizontal
    pub layer: String,
    pub midpoint: [f64; 2],
}

#[derive(Debug, Clone)]
pub struct ClosedContour {
    pub points: Vec<[f64; 2]>,
    pub area: f64,
    pub center: [f64; 2],
    pub bbox: [f64; 4],   // min_x, min_y, max_x, max_y
    pub is_rectangular: bool,
    pub aspect_ratio: f64,
}

#[derive(Debug, Clone)]
pub struct DrawingRegion {
    pub bbox: [f64; 4],
    pub entity_count: usize,
    pub region_type: RegionType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionType {
    MainDrawing,
    TitleBlock,
    DetailView,
    DimensionArea,
    Unknown,
}

pub fn preprocess(geom: &RawGeometry) -> PreprocessResult {
    let mut result = PreprocessResult::default();
    result.debug.push("=== PREPROCESSOR ===".into());

    // -- Step 1: Classify all lines by direction and length --
    let mut all_lines: Vec<ClassifiedLine> = Vec::new();

    for line in &geom.lines {
        let dx = line.end[0] - line.start[0];
        let dy = line.end[1] - line.start[1];
        let length = (dx * dx + dy * dy).sqrt();
        let angle = dy.atan2(dx);

        if length < 10.0 {
            result.short_lines += 1;
            continue;
        }

        all_lines.push(ClassifiedLine {
            start: [line.start[0], line.start[1]],
            end: [line.end[0], line.end[1]],
            length,
            angle,
            layer: line.layer.clone(),
            midpoint: [
                (line.start[0] + line.end[0]) / 2.0,
                (line.start[1] + line.end[1]) / 2.0,
            ],
        });
    }

    // Add polyline segments
    for poly in &geom.polylines {
        for pair in poly.points.windows(2) {
            let dx = pair[1][0] - pair[0][0];
            let dy = pair[1][1] - pair[0][1];
            let length = (dx * dx + dy * dy).sqrt();
            if length < 10.0 {
                result.short_lines += 1;
                continue;
            }
            all_lines.push(ClassifiedLine {
                start: [pair[0][0], pair[0][1]],
                end: [pair[1][0], pair[1][1]],
                length,
                angle: dy.atan2(dx),
                layer: poly.layer.clone(),
                midpoint: [
                    (pair[0][0] + pair[1][0]) / 2.0,
                    (pair[0][1] + pair[1][1]) / 2.0,
                ],
            });
        }
    }

    result.debug.push(format!(
        "  Total lines: {} (removed {} short)",
        all_lines.len(),
        result.short_lines
    ));

    // -- Step 2: Deduplicate overlapping lines --
    all_lines.sort_by(|a, b| {
        a.midpoint[0]
            .partial_cmp(&b.midpoint[0])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let before = all_lines.len();
    all_lines.dedup_by(|a, b| {
        (a.midpoint[0] - b.midpoint[0]).abs() < 1.0
            && (a.midpoint[1] - b.midpoint[1]).abs() < 1.0
            && (a.length - b.length).abs() < 5.0
    });
    result.dedup_count = before - all_lines.len();
    result.debug.push(format!(
        "  After dedup: {} (merged {})",
        all_lines.len(),
        result.dedup_count
    ));

    // -- Step 3: Detect drawing frame (largest rectangle) --
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    for l in &all_lines {
        min_x = min_x.min(l.start[0]).min(l.end[0]);
        min_y = min_y.min(l.start[1]).min(l.end[1]);
        max_x = max_x.max(l.start[0]).max(l.end[0]);
        max_y = max_y.max(l.start[1]).max(l.end[1]);
    }
    result.frame_rect = Some([min_x, min_y, max_x - min_x, max_y - min_y]);

    // Lines that form the outer border (within 2% of total extent) are frame lines
    let frame_margin_x = (max_x - min_x) * 0.02;
    let frame_margin_y = (max_y - min_y) * 0.02;

    // -- Step 4: Split into horizontal and vertical long lines --
    let h_threshold = 0.15; // radians (~8.6 degrees)
    let min_long = 500.0; // minimum "long line" length

    for line in &all_lines {
        // Skip frame lines
        let is_frame = (line.start[0] - min_x).abs() < frame_margin_x
            || (line.end[0] - min_x).abs() < frame_margin_x
            || (line.start[0] - max_x).abs() < frame_margin_x
            || (line.end[0] - max_x).abs() < frame_margin_x
            || (line.start[1] - min_y).abs() < frame_margin_y
            || (line.end[1] - min_y).abs() < frame_margin_y
            || (line.start[1] - max_y).abs() < frame_margin_y
            || (line.end[1] - max_y).abs() < frame_margin_y;

        if line.length < min_long {
            continue;
        }

        let abs_angle = line.angle.abs();
        if abs_angle < h_threshold || (std::f64::consts::PI - abs_angle) < h_threshold {
            // Horizontal
            if !is_frame {
                result.long_h_lines.push(line.clone());
            }
        } else if (abs_angle - std::f64::consts::FRAC_PI_2).abs() < h_threshold {
            // Vertical
            if !is_frame {
                result.long_v_lines.push(line.clone());
            }
        }
    }

    result.debug.push(format!(
        "  Long H lines: {} | Long V lines: {}",
        result.long_h_lines.len(),
        result.long_v_lines.len()
    ));

    // -- Step 5: Detect closed contours from polylines --
    for poly in &geom.polylines {
        if !poly.closed || poly.points.len() < 3 {
            continue;
        }

        let mut area = 0.0f64;
        let n = poly.points.len();
        let mut cx = 0.0f64;
        let mut cy = 0.0f64;
        let mut bmin = [f64::MAX; 2];
        let mut bmax = [f64::MIN; 2];

        for i in 0..n {
            let j = (i + 1) % n;
            let (x1, y1) = (poly.points[i][0], poly.points[i][1]);
            let (x2, y2) = (poly.points[j][0], poly.points[j][1]);
            area += x1 * y2 - x2 * y1;
            cx += x1;
            cy += y1;
            bmin[0] = bmin[0].min(x1);
            bmin[1] = bmin[1].min(y1);
            bmax[0] = bmax[0].max(x1);
            bmax[1] = bmax[1].max(y1);
        }
        area = area.abs() / 2.0;
        cx /= n as f64;
        cy /= n as f64;

        if area < 100.0 {
            continue;
        } // too small

        let w = bmax[0] - bmin[0];
        let h = bmax[1] - bmin[1];
        let is_rect = n == 4 || (n <= 6 && w * h > 0.001 && (area / (w * h) > 0.85));
        let aspect = if h > 0.001 { w / h } else { 999.0 };

        result.closed_contours.push(ClosedContour {
            points: poly.points.iter().map(|p| [p[0], p[1]]).collect(),
            area,
            center: [cx, cy],
            bbox: [bmin[0], bmin[1], bmax[0], bmax[1]],
            is_rectangular: is_rect,
            aspect_ratio: aspect,
        });
    }

    result.debug.push(format!(
        "  Closed contours: {}",
        result.closed_contours.len()
    ));

    // -- Step 6: Classify texts --
    for text in &geom.texts {
        let s = text.content.trim();
        let is_short = s.len() <= 4;
        let is_alpha = !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric());
        let is_numeric = s.parse::<f64>().is_ok();
        let is_dim = s.starts_with('+') || s.starts_with('-') || (is_numeric && s.len() >= 2);

        if is_dim || (is_numeric && s.parse::<f64>().unwrap_or(0.0).abs() > 10.0) {
            result.texts_dimension.push(text.clone());
        } else if is_short && is_alpha {
            result.texts_structural.push(text.clone());
        } else {
            result.texts_noise += 1;
        }
    }

    result.debug.push(format!(
        "  Texts: {} structural, {} dimension, {} noise",
        result.texts_structural.len(),
        result.texts_dimension.len(),
        result.texts_noise
    ));

    // Copy dimensions through
    result.dimensions = geom.dimensions.clone();

    result.debug.push("=== PREPROCESSOR DONE ===".into());
    result
}

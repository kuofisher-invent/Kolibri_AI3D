//! Drawing type classifier — determines what kind of drawing this is

use super::geometry_parser::RawGeometry;
use super::ir::DrawingType;

pub fn classify_drawing(geom: &RawGeometry) -> DrawingType {
    let mut scores = [
        (DrawingType::ColumnLayoutPlan, 0i32),
        (DrawingType::SteelElevation, 0i32),
        (DrawingType::FloorPlan, 0i32),
    ];

    // Check for grid-like patterns (long lines, grid labels)
    let long_lines = geom.lines.iter().filter(|l| {
        let dx = l.end[0] - l.start[0];
        let dy = l.end[1] - l.start[1];
        (dx * dx + dy * dy).sqrt() > 5000.0
    }).count();

    // Check for grid label texts (single letters A-Z or numbers 1-9)
    let grid_labels = geom.texts.iter().filter(|t| {
        let s = t.content.trim();
        (s.len() == 1 && s.chars().next().map_or(false, |c| c.is_ascii_uppercase())) ||
        (s.len() <= 2 && s.parse::<u32>().is_ok())
    }).count();

    // Check for elevation markers (+0, +4200, etc.)
    let elevation_texts = geom.texts.iter().filter(|t| {
        t.content.starts_with('+') || t.content.starts_with('-') ||
        t.content.contains("EL.") || t.content.contains("FL.")
    }).count();

    // Check for repeated blocks at regular intervals (columns)
    let block_count = geom.blocks.len();

    // Check for dimension chains
    let dim_count = geom.dimensions.len();

    // Scoring
    if grid_labels > 4 { scores[0].1 += 30; }
    if block_count > 4 { scores[0].1 += 25; }
    if long_lines > 8 { scores[0].1 += 10; }

    if elevation_texts > 2 { scores[1].1 += 40; }
    if dim_count > 5 { scores[1].1 += 15; }

    // Check for mostly vertical lines (elevation view)
    let vertical_lines = geom.lines.iter().filter(|l| {
        let dx = (l.end[0] - l.start[0]).abs();
        let dy = (l.end[1] - l.start[1]).abs();
        dy > dx * 3.0 && dy > 1000.0
    }).count();
    if vertical_lines > 5 { scores[1].1 += 20; }

    // Floor plan: lots of closed polylines (rooms)
    let closed_polys = geom.polylines.iter().filter(|p| p.closed).count();
    if closed_polys > 3 { scores[2].1 += 30; }

    // Suppress unused variable warnings
    let _ = long_lines;
    let _ = grid_labels;
    let _ = elevation_texts;
    let _ = block_count;
    let _ = dim_count;
    let _ = vertical_lines;
    let _ = closed_polys;

    // Pick highest score
    scores.sort_by(|a, b| b.1.cmp(&a.1));
    if scores[0].1 > 10 {
        scores[0].0.clone()
    } else {
        DrawingType::Unknown
    }
}

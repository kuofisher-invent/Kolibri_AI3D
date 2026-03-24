//! Grid Parser — detects axis lines and grid labels from DXF geometry

use super::geometry_parser::RawGeometry;
use super::ir::{GridSystem, GridLine};

pub fn parse_grids(geom: &RawGeometry) -> GridSystem {
    let mut system = GridSystem::default();

    // Strategy 1: Find text labels that look like grid names (A, B, C... or 1, 2, 3...)
    // and associate them with nearby long lines

    let mut x_candidates: Vec<(String, f64)> = Vec::new();
    let mut y_candidates: Vec<(String, f64)> = Vec::new();

    for text in &geom.texts {
        let s = text.content.trim().to_uppercase();
        let is_grid_label = (s.len() <= 3 && s.chars().all(|c| c.is_ascii_uppercase()))
            || (s.len() <= 2 && s.parse::<u32>().is_ok());
        if !is_grid_label { continue; }

        // Find the nearest long line to this text
        let tx = text.position[0];
        let ty = text.position[1];

        // First pass: find best matching line within proximity
        let mut best_x: Option<(f64, f64)> = None; // (distance, line_x)
        let mut best_y: Option<(f64, f64)> = None;

        for line in &geom.lines {
            let dx = (line.end[0] - line.start[0]).abs();
            let dy = (line.end[1] - line.start[1]).abs();
            let len = (dx * dx + dy * dy).sqrt();
            if len < 2000.0 { continue; }

            if dy > dx * 2.0 {
                // Vertical line -> X grid
                let line_x = (line.start[0] + line.end[0]) / 2.0;
                let dist = (tx - line_x).abs();
                if dist < 2000.0 {
                    if best_x.is_none() || dist < best_x.unwrap().0 {
                        best_x = Some((dist, line_x));
                    }
                }
            } else if dx > dy * 2.0 {
                // Horizontal line -> Y grid
                let line_y = (line.start[1] + line.end[1]) / 2.0;
                let dist = (ty - line_y).abs();
                if dist < 2000.0 {
                    if best_y.is_none() || dist < best_y.unwrap().0 {
                        best_y = Some((dist, line_y));
                    }
                }
            }
        }

        if let Some((_, line_x)) = best_x {
            x_candidates.push((s.clone(), line_x));
        }
        if let Some((_, line_y)) = best_y {
            // Only add as Y if we didn't already add as X (prefer X for letter labels)
            if best_x.is_none() || s.parse::<u32>().is_ok() {
                y_candidates.push((s.clone(), line_y));
            }
        }
    }

    // Strategy 2: If no text-based grids found, use dimension chains
    if x_candidates.is_empty() && !geom.dimensions.is_empty() {
        let mut x_positions: Vec<f64> = Vec::new();
        x_positions.push(0.0);
        let mut accumulated = 0.0;

        let mut h_dims: Vec<_> = geom.dimensions.iter()
            .filter(|d| (d.start[1] - d.end[1]).abs() < 100.0)
            .collect();

        h_dims.sort_by(|a, b| a.start[0].partial_cmp(&b.start[0]).unwrap_or(std::cmp::Ordering::Equal));

        for dim in &h_dims {
            accumulated += dim.value;
            x_positions.push(accumulated);
        }

        for (i, &pos) in x_positions.iter().enumerate() {
            let name = if i < 26 {
                ((b'A' + i as u8) as char).to_string()
            } else {
                format!("{}", i + 1)
            };
            x_candidates.push((name, pos));
        }
    }

    // Deduplicate and sort
    x_candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    x_candidates.dedup_by(|a, b| (a.1 - b.1).abs() < 50.0);

    y_candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    y_candidates.dedup_by(|a, b| (a.1 - b.1).abs() < 50.0);

    // Normalize to origin
    let x_offset = x_candidates.first().map(|c| c.1).unwrap_or(0.0);
    let y_offset = y_candidates.first().map(|c| c.1).unwrap_or(0.0);

    system.x_grids = x_candidates.into_iter()
        .map(|(name, pos)| GridLine { name, position: pos - x_offset })
        .collect();
    system.y_grids = y_candidates.into_iter()
        .map(|(name, pos)| GridLine { name, position: pos - y_offset })
        .collect();

    system
}

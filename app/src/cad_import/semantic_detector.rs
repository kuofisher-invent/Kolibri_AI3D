//! Semantic detector: identifies structural members from raw 2D geometry
//! Works without grid labels -- uses geometric patterns only
//!
//! Pipeline: DXF/PDF raw geometry -> classify lines -> find grid patterns -> detect members

use super::ir::*;
use super::geometry_parser::RawGeometry;

/// Result of semantic detection from raw geometry
pub struct SemanticResult {
    pub columns: Vec<ColumnDef>,
    pub beams: Vec<BeamDef>,
    pub plates: Vec<BasePlateDef>,
    pub grids: GridSystem,
    pub levels: Vec<LevelDef>,
    pub debug_lines: Vec<String>,
}

/// Main entry point: detect structural members from raw 2D geometry
pub fn detect_from_geometry(geom: &RawGeometry) -> SemanticResult {
    let mut result = SemanticResult {
        columns: Vec::new(),
        beams: Vec::new(),
        plates: Vec::new(),
        grids: GridSystem::default(),
        levels: Vec::new(),
        debug_lines: Vec::new(),
    };

    result.debug_lines.push("--- Semantic Detection (geometry-based) ---".into());
    result.debug_lines.push(format!(
        "  Input: {} lines, {} polylines, {} texts",
        geom.lines.len(),
        geom.polylines.len(),
        geom.texts.len()
    ));

    // -- Step 0: Detect and exclude drawing frame lines --
    // Frame lines are typically the longest lines forming the border rectangle.
    // Exclude the top ~8 longest lines (likely frame borders) from structural detection.
    let frame_indices: std::collections::HashSet<usize> = {
        let mut line_lengths: Vec<(usize, f64)> = geom.lines.iter().enumerate()
            .map(|(i, l)| {
                let dx = l.end[0] - l.start[0];
                let dy = l.end[1] - l.start[1];
                (i, (dx * dx + dy * dy).sqrt())
            })
            .collect();
        line_lengths.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // The top 8 longest lines are likely frame borders — use 90% of 9th longest as threshold
        let frame_threshold = line_lengths.get(8).map(|l| l.1).unwrap_or(0.0);
        let indices: std::collections::HashSet<usize> = line_lengths.iter()
            .filter(|l| l.1 > frame_threshold * 0.9 && l.1 > 10000.0) // must be >10m to be a frame line
            .map(|l| l.0)
            .collect();

        result.debug_lines.push(format!(
            "  Frame exclusion: {} lines excluded (threshold={:.0}mm)",
            indices.len(), frame_threshold * 0.9
        ));
        indices
    };

    // -- Step 1: Classify all lines by orientation --
    let mut h_lines: Vec<&super::geometry_parser::RawLine> = Vec::new();
    let mut v_lines: Vec<&super::geometry_parser::RawLine> = Vec::new();
    let mut d_lines: Vec<&super::geometry_parser::RawLine> = Vec::new();

    for (i, line) in geom.lines.iter().enumerate() {
        // Skip drawing frame lines
        if frame_indices.contains(&i) {
            continue;
        }

        let dx = (line.end[0] - line.start[0]).abs();
        let dy = (line.end[1] - line.start[1]).abs();
        let length = (dx * dx + dy * dy).sqrt();

        if length < 100.0 {
            continue; // skip tiny lines
        }

        if dy < dx * 0.1 {
            h_lines.push(line);
        } else if dx < dy * 0.1 {
            v_lines.push(line);
        } else {
            d_lines.push(line);
        }
    }

    // Also extract lines from polyline segments
    let mut poly_h_lines: Vec<super::geometry_parser::RawLine> = Vec::new();
    let mut poly_v_lines: Vec<super::geometry_parser::RawLine> = Vec::new();

    for poly in &geom.polylines {
        if poly.points.len() < 2 {
            continue;
        }
        for w in poly.points.windows(2) {
            let dx = (w[1][0] - w[0][0]).abs();
            let dy = (w[1][1] - w[0][1]).abs();
            let length = (dx * dx + dy * dy).sqrt();
            if length < 100.0 {
                continue;
            }
            let seg = super::geometry_parser::RawLine {
                start: w[0],
                end: w[1],
                layer: poly.layer.clone(),
                linetype: "CONTINUOUS".into(),
            };
            if dy < dx * 0.1 {
                poly_h_lines.push(seg);
            } else if dx < dy * 0.1 {
                poly_v_lines.push(seg);
            }
        }
    }

    // Borrow polyline-derived segments into the main arrays
    for l in &poly_h_lines {
        h_lines.push(l);
    }
    for l in &poly_v_lines {
        v_lines.push(l);
    }

    result.debug_lines.push(format!(
        "  Classified: {} horizontal, {} vertical, {} diagonal",
        h_lines.len(),
        v_lines.len(),
        d_lines.len()
    ));

    // -- Step 2: Find grid-like patterns from line positions --
    // Group vertical lines by X position (within tolerance)
    let mut x_positions: Vec<f64> = Vec::new();
    for line in &v_lines {
        let x = (line.start[0] + line.end[0]) / 2.0;
        if !x_positions.iter().any(|&ex| (ex - x).abs() < 200.0) {
            x_positions.push(x);
        }
    }
    x_positions.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Group horizontal lines by Y position
    let mut y_positions: Vec<f64> = Vec::new();
    for line in &h_lines {
        let y = (line.start[1] + line.end[1]) / 2.0;
        if !y_positions.iter().any(|&ey| (ey - y).abs() < 200.0) {
            y_positions.push(y);
        }
    }
    y_positions.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    result.debug_lines.push(format!(
        "  Unique positions: {} X, {} Y",
        x_positions.len(),
        y_positions.len()
    ));

    // -- Step 3: Build grid system from detected positions --
    let x_offset = x_positions.first().copied().unwrap_or(0.0);
    let y_offset = y_positions.first().copied().unwrap_or(0.0);

    for (i, &x) in x_positions.iter().enumerate() {
        let name = if i < 26 {
            ((b'A' + i as u8) as char).to_string()
        } else {
            format!("X{}", i + 1)
        };
        result.grids.x_grids.push(GridLine {
            name,
            position: x - x_offset,
        });
    }
    for (i, &y) in y_positions.iter().enumerate() {
        result.grids.y_grids.push(GridLine {
            name: format!("{}", i + 1),
            position: y - y_offset,
        });
    }

    result.debug_lines.push(format!(
        "  Grid: {} X-grids, {} Y-grids",
        result.grids.x_grids.len(),
        result.grids.y_grids.len()
    ));

    // -- Step 4: Detect beams from horizontal lines --
    for line in &h_lines {
        let length = (line.end[0] - line.start[0]).abs();
        if length < 500.0 {
            continue; // too short for a beam
        }

        let y = (line.start[1] + line.end[1]) / 2.0 - y_offset;
        let x1 = line.start[0].min(line.end[0]) - x_offset;
        let x2 = line.start[0].max(line.end[0]) - x_offset;

        // Check if this line connects two grid positions
        let near_grid = result
            .grids
            .x_grids
            .iter()
            .any(|g| (g.position - x1).abs() < 200.0)
            && result
                .grids
                .x_grids
                .iter()
                .any(|g| (g.position - x2).abs() < 200.0);

        if near_grid || length > 1000.0 {
            // Avoid duplicates: check if we already have a very similar beam
            let is_dup = result.beams.iter().any(|b| {
                (b.start_pos[0] - x1).abs() < 100.0
                    && (b.start_pos[1] - y).abs() < 100.0
                    && (b.end_pos[0] - x2).abs() < 100.0
            });
            if is_dup {
                continue;
            }

            let beam_id = format!("BM_{}", result.beams.len() + 1);
            result.beams.push(BeamDef {
                id: beam_id,
                from_grid: String::new(),
                to_grid: String::new(),
                elevation: 3000.0, // default, refined by text parsing
                start_pos: [x1, y],
                end_pos: [x2, y],
                profile: None,
            });
        }
    }

    result.debug_lines.push(format!(
        "  Beams detected: {}",
        result.beams.len()
    ));

    // -- Step 5: Detect columns at grid intersections --
    if result.grids.x_grids.len() >= 2 && result.grids.y_grids.len() >= 2 {
        for xg in &result.grids.x_grids {
            for yg in &result.grids.y_grids {
                // Verify there's actually geometry near this intersection
                let has_v_line = v_lines.iter().any(|l| {
                    let lx = (l.start[0] + l.end[0]) / 2.0 - x_offset;
                    (lx - xg.position).abs() < 200.0
                });
                let has_h_line = h_lines.iter().any(|l| {
                    let ly = (l.start[1] + l.end[1]) / 2.0 - y_offset;
                    (ly - yg.position).abs() < 200.0
                });

                if has_v_line && has_h_line {
                    let col_id = format!("COL_{}_{}", xg.name, yg.name);
                    result.columns.push(ColumnDef {
                        id: col_id,
                        grid_x: xg.name.clone(),
                        grid_y: yg.name.clone(),
                        position: [xg.position, yg.position],
                        base_level: 0.0,
                        top_level: 3000.0,
                        profile: None,
                    });
                }
            }
        }
    }

    result.debug_lines.push(format!(
        "  Columns detected: {}",
        result.columns.len()
    ));

    // -- Step 6: Detect plates from closed polylines --
    for poly in &geom.polylines {
        if !poly.closed || poly.points.len() < 3 {
            continue;
        }

        // Compute area using shoelace formula
        let mut area = 0.0f64;
        let n = poly.points.len();
        for i in 0..n {
            let j = (i + 1) % n;
            area += poly.points[i][0] * poly.points[j][1];
            area -= poly.points[j][0] * poly.points[i][1];
        }
        area = area.abs() / 2.0;

        if area > 10000.0 {
            // > 100cm^2 = meaningful plate
            let mut min_x = f64::MAX;
            let mut min_y = f64::MAX;
            let mut max_x = f64::MIN;
            let mut max_y = f64::MIN;
            for p in &poly.points {
                min_x = min_x.min(p[0]);
                min_y = min_y.min(p[1]);
                max_x = max_x.max(p[0]);
                max_y = max_y.max(p[1]);
            }
            result.plates.push(BasePlateDef {
                id: format!("PL_{}", result.plates.len() + 1),
                position: [min_x - x_offset, min_y - y_offset],
                width: max_x - min_x,
                depth: max_y - min_y,
                height: 12.0, // default plate thickness
            });
        }
    }

    result.debug_lines.push(format!(
        "  Plates detected: {}",
        result.plates.len()
    ));

    // -- Step 7: Extract elevations from text entities --
    for text in &geom.texts {
        let s = text.content.trim();
        // Look for elevation markers: +4200, -500, EL.+3000, FL.+2800
        let clean = s
            .replace("EL.", "")
            .replace("FL.", "")
            .replace("el.", "")
            .replace("fl.", "");
        if let Ok(val) = clean.trim().parse::<f64>() {
            if val.abs() < 100000.0 {
                let name = if val.abs() < 1.0 {
                    "GL".to_string()
                } else {
                    format!("EL{:+.0}", val)
                };
                if !result.levels.iter().any(|l| (l.elevation - val).abs() < 10.0) {
                    result.levels.push(LevelDef {
                        name,
                        elevation: val,
                    });
                }
            }
        }
        // Log dimension-like text
        if let Ok(val) = s.parse::<f64>() {
            if val > 100.0 && val < 50000.0 {
                result.debug_lines.push(format!(
                    "  Dimension text: {} @ [{:.0},{:.0}]",
                    s, text.position[0], text.position[1]
                ));
            }
        }
    }

    result
        .levels
        .sort_by(|a, b| a.elevation.partial_cmp(&b.elevation).unwrap_or(std::cmp::Ordering::Equal));
    if result.levels.is_empty() {
        result.levels.push(LevelDef {
            name: "GL".into(),
            elevation: 0.0,
        });
        result.levels.push(LevelDef {
            name: "TOP".into(),
            elevation: 3000.0,
        });
    }

    // Apply detected levels to columns and beams
    if let (Some(base), Some(top)) = (result.levels.first(), result.levels.last()) {
        let base_el = base.elevation;
        let top_el = top.elevation;
        for col in &mut result.columns {
            col.base_level = base_el;
            col.top_level = top_el;
        }
        for beam in &mut result.beams {
            beam.elevation = top_el;
        }
    }

    result.debug_lines.push(format!(
        "  Levels: {}",
        result
            .levels
            .iter()
            .map(|l| format!("{}({:.0})", l.name, l.elevation))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    result
        .debug_lines
        .push("--- Semantic Detection Complete ---".into());

    result
}

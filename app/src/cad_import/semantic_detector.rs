//! Semantic Detector v2 — Geometry-first with confidence scoring
//!
//! Pipeline: Preprocessed geometry -> Candidate generation -> Scoring -> Ranking

use super::geometry_parser::RawText;
use super::ir::*;
use super::preprocessor::*;

#[allow(dead_code)]
pub struct SemanticCandidate {
    pub kind: CandidateKind,
    pub score: f32,           // 0-100
    pub reasons: Vec<String>,
    pub geometry: CandidateGeometry,
}

#[allow(dead_code)]
pub enum CandidateKind {
    Grid,
    Column,
    Beam,
    Plate,
    #[allow(dead_code)]
    Brace,
}

#[allow(dead_code)]
pub enum CandidateGeometry {
    Line { start: [f64; 2], end: [f64; 2] },
    Point { position: [f64; 2] },
    #[allow(dead_code)]
    Rect { bbox: [f64; 4] },
}

pub struct SemanticResult {
    pub candidates: Vec<SemanticCandidate>,
    pub grids: GridSystem,
    pub columns: Vec<ColumnDef>,
    pub beams: Vec<BeamDef>,
    pub plates: Vec<BasePlateDef>,
    pub levels: Vec<LevelDef>,
    pub debug_lines: Vec<String>,
}

/// Legacy entry point — kept for backward compatibility.
/// Runs the full v2 pipeline internally.
pub fn detect_from_geometry(geom: &super::geometry_parser::RawGeometry) -> SemanticResult {
    let prep = super::preprocessor::preprocess(geom);
    detect_v2(&prep, &geom.texts)
}

/// New entry point: detect structural members from preprocessed geometry
pub fn detect_v2(prep: &PreprocessResult, texts: &[RawText]) -> SemanticResult {
    let mut result = SemanticResult {
        candidates: Vec::new(),
        grids: GridSystem::default(),
        columns: Vec::new(),
        beams: Vec::new(),
        plates: Vec::new(),
        levels: Vec::new(),
        debug_lines: Vec::new(),
    };

    result.debug_lines.push("=== SEMANTIC DETECTOR v2 ===".into());
    result.debug_lines.push("  Mode: Geometry-first + Text Verification".into());

    // Relay preprocessor debug
    for line in &prep.debug {
        result.debug_lines.push(line.clone());
    }

    // -- Phase 1: Grid detection from parallel line clusters --
    detect_grids_from_geometry(prep, texts, &mut result);

    // -- Phase 2: Column detection at grid intersections --
    detect_columns(prep, &mut result);

    // -- Phase 3: Beam detection from horizontal members --
    detect_beams(prep, &mut result);

    // -- Phase 4: Plate detection from closed contours --
    detect_plates(prep, &mut result);

    // -- Phase 5: Level extraction from dimension texts --
    detect_levels(prep, &mut result);

    // Summary
    result.debug_lines.push("-------------------------------".into());
    result.debug_lines.push("  RESULTS:".into());
    result.debug_lines.push(format!(
        "    Grid X: {} | Grid Y: {}",
        result.grids.x_grids.len(),
        result.grids.y_grids.len()
    ));
    result.debug_lines.push(format!(
        "    Columns: {} | Beams: {} | Plates: {}",
        result.columns.len(),
        result.beams.len(),
        result.plates.len()
    ));
    result.debug_lines.push(format!("    Levels: {}", result.levels.len()));

    let high = result.candidates.iter().filter(|c| c.score >= 70.0).count();
    let mid = result
        .candidates
        .iter()
        .filter(|c| c.score >= 40.0 && c.score < 70.0)
        .count();
    let low = result.candidates.iter().filter(|c| c.score < 40.0).count();
    result.debug_lines.push(format!(
        "    Confidence: {} high, {} medium, {} low",
        high, mid, low
    ));
    result.debug_lines.push("=== DETECTION COMPLETE ===".into());

    result
}

fn detect_grids_from_geometry(
    prep: &PreprocessResult,
    texts: &[RawText],
    result: &mut SemanticResult,
) {
    result
        .debug_lines
        .push("  [Grid Detection: Parallel Line Clustering]".into());

    // -- Find vertical line clusters (X-axis grids) --
    // Cluster vertical lines by their X position
    let mut v_x_positions: Vec<(f64, f64)> = Vec::new(); // (x_position, total_length)
    for line in &prep.long_v_lines {
        let x = (line.start[0] + line.end[0]) / 2.0;
        if let Some(existing) = v_x_positions.iter_mut().find(|p| (p.0 - x).abs() < 200.0) {
            existing.1 += line.length; // accumulate length
        } else {
            v_x_positions.push((x, line.length));
        }
    }
    v_x_positions.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Only keep clusters with significant total length
    let avg_length = v_x_positions.iter().map(|p| p.1).sum::<f64>()
        / v_x_positions.len().max(1) as f64;
    let threshold = (avg_length * 0.3).max(1000.0);
    v_x_positions.retain(|p| p.1 >= threshold);

    result.debug_lines.push(format!(
        "    Vertical clusters: {} (threshold: {:.0}mm)",
        v_x_positions.len(),
        threshold
    ));

    // -- Find horizontal line clusters (Y-axis grids) --
    let mut h_y_positions: Vec<(f64, f64)> = Vec::new();
    for line in &prep.long_h_lines {
        let y = (line.start[1] + line.end[1]) / 2.0;
        if let Some(existing) = h_y_positions.iter_mut().find(|p| (p.0 - y).abs() < 200.0) {
            existing.1 += line.length;
        } else {
            h_y_positions.push((y, line.length));
        }
    }
    h_y_positions.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    h_y_positions.retain(|p| p.1 >= threshold);

    result.debug_lines.push(format!(
        "    Horizontal clusters: {}",
        h_y_positions.len()
    ));

    // -- Check for regular spacing (strong grid indicator) --
    let x_spacings = compute_spacings(&v_x_positions);
    let y_spacings = compute_spacings(&h_y_positions);

    let x_regularity = spacing_regularity(&x_spacings);
    let y_regularity = spacing_regularity(&y_spacings);

    result.debug_lines.push(format!(
        "    X spacing regularity: {:.0}% | Y: {:.0}%",
        x_regularity * 100.0,
        y_regularity * 100.0
    ));

    // -- Build grid from geometry clusters --
    let x_offset = v_x_positions.first().map(|p| p.0).unwrap_or(0.0);
    let y_offset = h_y_positions.first().map(|p| p.0).unwrap_or(0.0);

    // -- Text verification: find nearby labels --
    for (i, &(x_pos, total_len)) in v_x_positions.iter().enumerate() {
        let mut score: f32 = 30.0; // base score for being a parallel line cluster
        let mut reasons = vec![format!(
            "vertical cluster at x={:.0}, length={:.0}",
            x_pos, total_len
        )];

        if total_len > avg_length * 0.8 {
            score += 20.0;
            reasons.push("long line cluster".into());
        }
        if x_regularity > 0.5 {
            score += 15.0;
            reasons.push(format!("regular spacing ({:.0}%)", x_regularity * 100.0));
        }

        // Find nearest text label
        let mut label = grid_name_default(i, true);
        for text in texts {
            let tx = text.position[0];
            let clean = super::grid_parser::clean_mtext(&text.content);
            if super::grid_parser::is_x_grid_label(&clean) && (tx - x_pos).abs() < 2000.0 {
                label = clean.clone();
                score += 25.0;
                reasons.push(format!("text label '{}' nearby", clean));
                break;
            }
        }

        result.grids.x_grids.push(GridLine {
            name: label,
            position: x_pos - x_offset,
        });
        result.candidates.push(SemanticCandidate {
            kind: CandidateKind::Grid,
            score,
            reasons,
            geometry: CandidateGeometry::Line {
                start: [
                    x_pos,
                    h_y_positions.first().map(|p| p.0).unwrap_or(0.0),
                ],
                end: [
                    x_pos,
                    h_y_positions.last().map(|p| p.0).unwrap_or(1000.0),
                ],
            },
        });
    }

    for (i, &(y_pos, total_len)) in h_y_positions.iter().enumerate() {
        let mut score: f32 = 30.0;
        let reasons = vec![format!("horizontal cluster at y={:.0}", y_pos)];
        if total_len > avg_length * 0.8 {
            score += 20.0;
        }
        if y_regularity > 0.5 {
            score += 15.0;
        }

        let mut label = grid_name_default(i, false);
        for text in texts {
            let ty = text.position[1];
            let clean = super::grid_parser::clean_mtext(&text.content);
            if super::grid_parser::is_y_grid_label(&clean) && (ty - y_pos).abs() < 2000.0 {
                label = clean;
                score += 25.0;
                break;
            }
        }

        result.grids.y_grids.push(GridLine {
            name: label,
            position: y_pos - y_offset,
        });
        result.candidates.push(SemanticCandidate {
            kind: CandidateKind::Grid,
            score,
            reasons,
            geometry: CandidateGeometry::Line {
                start: [
                    v_x_positions.first().map(|p| p.0).unwrap_or(0.0),
                    y_pos,
                ],
                end: [
                    v_x_positions.last().map(|p| p.0).unwrap_or(1000.0),
                    y_pos,
                ],
            },
        });
    }

    if !x_spacings.is_empty() {
        result.debug_lines.push(format!(
            "    X spacings: {:?}",
            x_spacings
                .iter()
                .map(|s| format!("{:.0}", s))
                .collect::<Vec<_>>()
        ));
    }
}

fn detect_columns(prep: &PreprocessResult, result: &mut SemanticResult) {
    if result.grids.x_grids.len() < 2 || result.grids.y_grids.len() < 2 {
        result
            .debug_lines
            .push("  [Columns: skipped -- insufficient grid]".into());
        return;
    }

    result
        .debug_lines
        .push("  [Column Detection: Grid Intersections]".into());

    let x_offset = result.grids.x_grids[0].position;

    for xg in &result.grids.x_grids.clone() {
        for yg in &result.grids.y_grids.clone() {
            let world_x = xg.position;
            let world_y = yg.position;

            let mut score: f32 = 40.0; // base score for being at grid intersection
            let mut reasons = vec![format!("grid intersection {}/{}", xg.name, yg.name)];

            // Check for closed contour (column symbol) nearby
            let has_contour = prep.closed_contours.iter().any(|c| {
                (c.center[0] - (world_x + x_offset)).abs() < 500.0
                    && (c.center[1] - (world_y + result.grids.y_grids[0].position)).abs() < 500.0
            });
            if has_contour {
                score += 30.0;
                reasons.push("closed contour at intersection".into());
            }

            // Check for line intersection (cross pattern)
            let has_cross = prep
                .long_v_lines
                .iter()
                .any(|l| (l.midpoint[0] - (world_x + x_offset)).abs() < 300.0)
                && prep.long_h_lines.iter().any(|l| {
                    (l.midpoint[1] - (world_y + result.grids.y_grids[0].position)).abs() < 300.0
                });
            if has_cross {
                score += 20.0;
                reasons.push("line cross at intersection".into());
            }

            if score >= 40.0 {
                result.columns.push(ColumnDef {
                    id: format!("COL_{}_{}", xg.name, yg.name),
                    grid_x: xg.name.clone(),
                    grid_y: yg.name.clone(),
                    position: [world_x, world_y],
                    base_level: 0.0,
                    top_level: 4200.0,
                    profile: None,
                });
                result.candidates.push(SemanticCandidate {
                    kind: CandidateKind::Column,
                    score,
                    reasons,
                    geometry: CandidateGeometry::Point {
                        position: [world_x, world_y],
                    },
                });
            }
        }
    }

    result.debug_lines.push(format!(
        "    Columns detected: {}",
        result.columns.len()
    ));
}

fn detect_beams(prep: &PreprocessResult, result: &mut SemanticResult) {
    if result.grids.x_grids.len() < 2 {
        return;
    }

    result
        .debug_lines
        .push("  [Beam Detection: Topology + Direction + Scoring]".into());

    let x_offset = result.grids.x_grids[0].position;
    let y_offset = result
        .grids
        .y_grids
        .first()
        .map(|g| g.position)
        .unwrap_or(0.0);

    // Find horizontal members that connect grid points
    for line in &prep.long_h_lines {
        if line.length < 1000.0 {
            continue;
        }

        let mut score = 0.0f32;
        let mut reasons = Vec::new();

        // Score: horizontal direction
        let abs_angle = line.angle.abs();
        if abs_angle < 0.05 || (std::f64::consts::PI - abs_angle) < 0.05 {
            score += 30.0;
            reasons.push("horizontal member".into());
        }

        // Score: connects two grid X positions
        let x1 = line.start[0].min(line.end[0]);
        let x2 = line.start[0].max(line.end[0]);
        let connects_grids = result
            .grids
            .x_grids
            .iter()
            .any(|g| ((g.position + x_offset) - x1).abs() < 500.0)
            && result
                .grids
                .x_grids
                .iter()
                .any(|g| ((g.position + x_offset) - x2).abs() < 500.0);
        if connects_grids {
            score += 30.0;
            reasons.push("connects two grid points".into());
        }

        // Score: high aspect ratio (beam-like)
        if line.length > 2000.0 {
            score += 10.0;
            reasons.push("long member".into());
        }

        // Score: consistent Y position with other beams
        let beam_y = (line.start[1] + line.end[1]) / 2.0;
        let similar_y_count = prep
            .long_h_lines
            .iter()
            .filter(|l| {
                let ly = (l.start[1] + l.end[1]) / 2.0;
                (ly - beam_y).abs() < 200.0 && l.length > 1000.0
            })
            .count();
        if similar_y_count >= 2 {
            score += 20.0;
            reasons.push(format!("{} members at similar Y", similar_y_count));
        }

        if score >= 50.0 {
            let y = (line.start[1] + line.end[1]) / 2.0;

            // Deduplicate: don't add if already have a beam at same position
            let already_exists = result.beams.iter().any(|b| {
                (b.start_pos[0] - (x1 - x_offset)).abs() < 100.0
                    && (b.end_pos[0] - (x2 - x_offset)).abs() < 100.0
                    && (b.elevation - 4200.0).abs() < 100.0
            });

            if !already_exists {
                result.beams.push(BeamDef {
                    id: format!("BM_{}", result.beams.len() + 1),
                    from_grid: String::new(),
                    to_grid: String::new(),
                    elevation: 4200.0,
                    start_pos: [x1 - x_offset, y - y_offset],
                    end_pos: [x2 - x_offset, y - y_offset],
                    profile: None,
                });
                result.candidates.push(SemanticCandidate {
                    kind: CandidateKind::Beam,
                    score,
                    reasons,
                    geometry: CandidateGeometry::Line {
                        start: [x1, y],
                        end: [x2, y],
                    },
                });
            }
        }
    }

    result.debug_lines.push(format!(
        "    Beams detected: {}",
        result.beams.len()
    ));
}

fn detect_plates(prep: &PreprocessResult, result: &mut SemanticResult) {
    for contour in &prep.closed_contours {
        if contour.area < 10000.0 {
            continue;
        } // < 100cm^2
        if !contour.is_rectangular {
            continue;
        }

        let w = contour.bbox[2] - contour.bbox[0];
        let h = contour.bbox[3] - contour.bbox[1];
        let x_offset = result
            .grids
            .x_grids
            .first()
            .map(|g| g.position)
            .unwrap_or(0.0);
        let y_offset = result
            .grids
            .y_grids
            .first()
            .map(|g| g.position)
            .unwrap_or(0.0);

        result.plates.push(BasePlateDef {
            id: format!("PL_{}", result.plates.len() + 1),
            position: [contour.bbox[0] - x_offset, contour.bbox[1] - y_offset],
            width: w,
            depth: h,
            height: 12.0,
        });
    }
    result.debug_lines.push(format!(
        "    Plates detected: {}",
        result.plates.len()
    ));
}

fn detect_levels(prep: &PreprocessResult, result: &mut SemanticResult) {
    for text in &prep.texts_dimension {
        let s = text
            .content
            .replace('+', "")
            .replace("EL.", "")
            .replace("FL.", "")
            .replace(',', "");
        if let Ok(v) = s.trim().parse::<f64>() {
            if v > 100.0 && v < 50000.0 {
                if !result.levels.iter().any(|l| (l.elevation - v).abs() < 10.0) {
                    result.levels.push(LevelDef {
                        name: format!("{:.0}", v),
                        elevation: v,
                    });
                }
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
            elevation: 4200.0,
        });
    }

    // Apply levels to columns
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
        "    Levels: {:?}",
        result
            .levels
            .iter()
            .map(|l| format!("{}({:.0})", l.name, l.elevation))
            .collect::<Vec<_>>()
    ));
}

// -- Helpers --

fn compute_spacings(positions: &[(f64, f64)]) -> Vec<f64> {
    if positions.len() < 2 {
        return Vec::new();
    }
    positions
        .windows(2)
        .map(|w| (w[1].0 - w[0].0).abs())
        .collect()
}

fn spacing_regularity(spacings: &[f64]) -> f64 {
    if spacings.len() < 2 {
        return 0.0;
    }
    let avg = spacings.iter().sum::<f64>() / spacings.len() as f64;
    if avg < 1.0 {
        return 0.0;
    }
    let variance = spacings.iter().map(|s| (s - avg).powi(2)).sum::<f64>() / spacings.len() as f64;
    let std_dev = variance.sqrt();
    let cv = std_dev / avg; // coefficient of variation
    (1.0 - cv).max(0.0) // 1.0 = perfectly regular, 0.0 = random
}

fn grid_name_default(idx: usize, is_x: bool) -> String {
    if is_x {
        if idx < 26 {
            ((b'A' + idx as u8) as char).to_string()
        } else {
            format!("X{}", idx)
        }
    } else {
        format!("{}", idx + 1)
    }
}

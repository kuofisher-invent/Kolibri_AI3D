//! Grid Parser — TEXT position clustering + DIMENSION chain analysis
//!
//! Strategy:
//! 1. Find ALL text that looks like grid labels (A-Z, AB, 1-9)
//! 2. Cluster them by position (X-axis labels cluster at similar Y, Y-axis labels at similar X)
//! 3. Use DIMENSION values to verify spacing
//! 4. Build grid from text positions + dimension verification

use super::geometry_parser::RawGeometry;
use super::ir::{GridSystem, GridLine};

pub fn parse_grids(geom: &RawGeometry) -> GridSystem {
    let mut system = GridSystem::default();

    // ═══ Strategy 1: TEXT-based grid detection ═══

    // Step 1: Collect all potential grid label texts
    let mut x_labels: Vec<(String, f64, f64)> = Vec::new(); // (label, x_pos, y_pos)
    let mut y_labels: Vec<(String, f64, f64)> = Vec::new();

    for text in &geom.texts {
        let clean = clean_mtext(&text.content);

        // X-axis labels: single or double uppercase letters
        if is_x_grid_label(&clean) {
            x_labels.push((clean.clone(), text.position[0], text.position[1]));
        }
        // Y-axis labels: single or double digits
        if is_y_grid_label(&clean) {
            y_labels.push((clean.clone(), text.position[0], text.position[1]));
        }
    }

    tracing::info!("Grid detection: found {} X label candidates, {} Y label candidates",
        x_labels.len(), y_labels.len());
    for (name, x, y) in &x_labels {
        tracing::info!("  X label '{}' at ({:.0}, {:.0})", name, x, y);
    }
    for (name, x, y) in &y_labels {
        tracing::info!("  Y label '{}' at ({:.0}, {:.0})", name, x, y);
    }

    // ═══ Multi-page detection: if same labels appear at very different positions,
    // the DXF has multiple pages (e.g. plan + elevation). Keep only one page. ═══
    {
        let mut label_groups: std::collections::HashMap<String, Vec<(f64, f64)>> = std::collections::HashMap::new();
        for (name, x, y) in &x_labels {
            label_groups.entry(name.clone()).or_default().push((*x, *y));
        }

        // Check if any label appears more than once at significantly different positions
        let has_multi_page = label_groups.values().any(|positions| {
            if positions.len() < 2 { return false; }
            let y_min = positions.iter().map(|p| p.1).fold(f64::MAX, f64::min);
            let y_max = positions.iter().map(|p| p.1).fold(f64::MIN, f64::max);
            (y_max - y_min).abs() > 5000.0 // pages are typically >5m apart
        });

        if has_multi_page {
            tracing::info!("Multi-page detected! Filtering to primary page...");

            // Find the Y median of all x_labels to split into two clusters
            let all_ys: Vec<f64> = x_labels.iter().map(|(_, _, y)| *y).collect();
            let y_median = {
                let mut sorted = all_ys.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                sorted[sorted.len() / 2]
            };

            // Split into two clusters: below and above median
            let cluster_low: Vec<_> = x_labels.iter().filter(|(_, _, y)| *y < y_median + 2000.0).cloned().collect();
            let cluster_high: Vec<_> = x_labels.iter().filter(|(_, _, y)| *y >= y_median + 2000.0).cloned().collect();

            // Use the cluster with more unique labels (likely the plan view)
            let unique_low: std::collections::HashSet<_> = cluster_low.iter().map(|(n, _, _)| n.clone()).collect();
            let unique_high: std::collections::HashSet<_> = cluster_high.iter().map(|(n, _, _)| n.clone()).collect();

            tracing::info!("  Cluster low ({} labels, {} unique): Y < {:.0}", cluster_low.len(), unique_low.len(), y_median + 2000.0);
            tracing::info!("  Cluster high ({} labels, {} unique): Y >= {:.0}", cluster_high.len(), unique_high.len(), y_median + 2000.0);

            x_labels = if unique_low.len() >= unique_high.len() { cluster_low } else { cluster_high };

            // Same for y_labels — use X position to split pages (plan vs elevation are side by side)
            let y_all_x: Vec<f64> = y_labels.iter().map(|(_, x, _)| *x).collect();
            if !y_all_x.is_empty() {
                let x_median = {
                    let mut sorted = y_all_x.clone();
                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    sorted[sorted.len() / 2]
                };

                // Check if y_labels also span multiple pages
                let y_label_groups: std::collections::HashMap<String, Vec<f64>> = {
                    let mut groups: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
                    for (name, x, _) in &y_labels {
                        groups.entry(name.clone()).or_default().push(*x);
                    }
                    groups
                };
                let y_has_multi = y_label_groups.values().any(|xs| {
                    if xs.len() < 2 { return false; }
                    let xmin = xs.iter().cloned().fold(f64::MAX, f64::min);
                    let xmax = xs.iter().cloned().fold(f64::MIN, f64::max);
                    (xmax - xmin).abs() > 5000.0
                });

                if y_has_multi {
                    let y_cluster_low: Vec<_> = y_labels.iter().filter(|(_, x, _)| *x < x_median + 2000.0).cloned().collect();
                    let y_cluster_high: Vec<_> = y_labels.iter().filter(|(_, x, _)| *x >= x_median + 2000.0).cloned().collect();
                    y_labels = if y_cluster_low.len() >= y_cluster_high.len() { y_cluster_low } else { y_cluster_high };
                }
            }

            tracing::info!("  After page filter: {} X labels, {} Y labels", x_labels.len(), y_labels.len());
        }
    }

    // Step 2: Remove duplicates — keep the one closest to the drawing's bottom/left edge
    // Sort X labels by their X position (the relevant axis)
    x_labels.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    dedup_labels_by_name(&mut x_labels, 500.0, true);

    // Sort Y labels by their Y position (the relevant axis)
    y_labels.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    dedup_labels_by_name(&mut y_labels, 500.0, false);

    tracing::info!("After dedup: {} X labels, {} Y labels", x_labels.len(), y_labels.len());

    // Step 3: X-axis labels -> X grid positions
    // The X position of each label IS the grid position
    if x_labels.len() >= 2 {
        let x_offset = x_labels[0].1;
        for (name, x, _y) in &x_labels {
            system.x_grids.push(GridLine {
                name: name.clone(),
                position: *x - x_offset,
            });
        }
    }

    // Step 4: Y-axis labels -> Y grid positions
    if y_labels.len() >= 2 {
        let y_offset = y_labels[0].2;
        for (name, _x, y) in &y_labels {
            system.y_grids.push(GridLine {
                name: name.clone(),
                position: *y - y_offset,
            });
        }
    }

    // ═══ Strategy 2: TEXT + LINE proximity (original approach, as fallback) ═══
    if system.x_grids.len() < 2 || system.y_grids.len() < 2 {
        tracing::info!("Grid: TEXT-only detection incomplete, trying TEXT+LINE proximity");
        let (x_from_lines, y_from_lines) = detect_grids_from_lines(geom);
        if system.x_grids.len() < 2 && x_from_lines.len() >= 2 {
            system.x_grids = x_from_lines;
        }
        if system.y_grids.len() < 2 && y_from_lines.len() >= 2 {
            system.y_grids = y_from_lines;
        }
    }

    // ═══ Strategy 3: DIMENSION chain analysis ═══
    // Collect dimension values from text field or computed from endpoints
    let dim_values = collect_dimension_values(geom);
    tracing::info!("Grid: collected {} dimension values: {:?}", dim_values.len(),
        dim_values.iter().map(|v| format!("{:.0}", v)).collect::<Vec<_>>());

    if system.x_grids.len() < 2 && !dim_values.is_empty() {
        tracing::info!("Grid: using DIMENSION chain to build X grids");
        // Build grid from dimension chains — consecutive dimensions that form a chain
        let mut x_pos = 0.0;
        let mut grid_idx = 0;
        let mut chain_grids = vec![GridLine { name: grid_name(grid_idx, true), position: 0.0 }];

        // Use horizontal dimensions sorted by start X
        let mut h_dims: Vec<(f64, f64)> = Vec::new(); // (start_x, value)
        for dim in &geom.dimensions {
            let dx = (dim.start[0] - dim.end[0]).abs();
            let dy = (dim.start[1] - dim.end[1]).abs();
            // Horizontal dimension
            if dx > dy * 0.5 {
                let val = dimension_numeric_value(dim);
                if val > 500.0 && val < 50000.0 {
                    let start_x = dim.start[0].min(dim.end[0]);
                    h_dims.push((start_x, val));
                }
            }
        }
        h_dims.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        // Deduplicate very close dimensions
        h_dims.dedup_by(|a, b| (a.0 - b.0).abs() < 100.0);

        if !h_dims.is_empty() {
            for &(_, val) in &h_dims {
                x_pos += val;
                grid_idx += 1;
                chain_grids.push(GridLine { name: grid_name(grid_idx, true), position: x_pos });
            }
            if chain_grids.len() >= 2 {
                system.x_grids = chain_grids;
            }
        } else {
            // No horizontal dims with start/end info — just chain the values
            for &val in &dim_values {
                if val > 500.0 && val < 50000.0 {
                    x_pos += val;
                    grid_idx += 1;
                    chain_grids.push(GridLine { name: grid_name(grid_idx, true), position: x_pos });
                }
            }
            if chain_grids.len() >= 2 {
                system.x_grids = chain_grids;
            }
        }
    }

    // If we still have no Y grids but have at least 2 X grids, create a default pair
    if system.y_grids.is_empty() && system.x_grids.len() >= 2 {
        // Look for vertical dimensions
        let mut v_dims: Vec<f64> = Vec::new();
        for dim in &geom.dimensions {
            let dx = (dim.start[0] - dim.end[0]).abs();
            let dy = (dim.start[1] - dim.end[1]).abs();
            if dy > dx * 2.0 {
                let val = dimension_numeric_value(dim);
                if val > 500.0 && val < 50000.0 {
                    v_dims.push(val);
                }
            }
        }
        v_dims.dedup_by(|a, b| (*a - *b).abs() < 100.0);

        if !v_dims.is_empty() {
            let mut y_pos = 0.0;
            system.y_grids.push(GridLine { name: "1".to_string(), position: 0.0 });
            for (i, &val) in v_dims.iter().enumerate() {
                y_pos += val;
                system.y_grids.push(GridLine { name: format!("{}", i + 2), position: y_pos });
            }
        }
    }

    // ═══ Final: ensure we have at least a minimal grid ═══
    // If we only have 1 axis in one direction and >=2 in the other, add a default second axis
    if system.x_grids.len() >= 2 && system.y_grids.is_empty() {
        // Default: place Y grids at 0 and max_span
        let total_span = system.x_grids.last().map(|g| g.position).unwrap_or(6000.0);
        let y_span = (total_span * 0.5).max(3000.0).min(10000.0);
        system.y_grids.push(GridLine { name: "1".to_string(), position: 0.0 });
        system.y_grids.push(GridLine { name: "2".to_string(), position: y_span });
        tracing::info!("Grid: added default Y grids at 0 and {:.0}", y_span);
    }
    if system.y_grids.len() >= 2 && system.x_grids.is_empty() {
        let total_span = system.y_grids.last().map(|g| g.position).unwrap_or(6000.0);
        let x_span = (total_span * 0.5).max(3000.0).min(10000.0);
        system.x_grids.push(GridLine { name: "A".to_string(), position: 0.0 });
        system.x_grids.push(GridLine { name: "B".to_string(), position: x_span });
        tracing::info!("Grid: added default X grids at 0 and {:.0}", x_span);
    }

    tracing::info!("Grid final result: {} X grids, {} Y grids", system.x_grids.len(), system.y_grids.len());
    for g in &system.x_grids {
        tracing::info!("  X grid '{}' at {:.0}", g.name, g.position);
    }
    for g in &system.y_grids {
        tracing::info!("  Y grid '{}' at {:.0}", g.name, g.position);
    }

    system
}

/// Fallback: original text-near-line proximity approach
fn detect_grids_from_lines(geom: &RawGeometry) -> (Vec<GridLine>, Vec<GridLine>) {
    let mut x_candidates: Vec<(String, f64)> = Vec::new();
    let mut y_candidates: Vec<(String, f64)> = Vec::new();

    for text in &geom.texts {
        let s = clean_mtext(&text.content);
        let is_grid_label = is_x_grid_label(&s) || is_y_grid_label(&s);
        if !is_grid_label { continue; }

        let tx = text.position[0];
        let ty = text.position[1];

        let mut best_x: Option<(f64, f64)> = None;
        let mut best_y: Option<(f64, f64)> = None;

        for line in &geom.lines {
            let dx = (line.end[0] - line.start[0]).abs();
            let dy = (line.end[1] - line.start[1]).abs();
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1000.0 { continue; } // relaxed from 2000

            if dy > dx * 2.0 {
                // Vertical line -> X grid
                let line_x = (line.start[0] + line.end[0]) / 2.0;
                let dist = (tx - line_x).abs();
                if dist < 3000.0 { // relaxed from 2000
                    if best_x.is_none() || dist < best_x.unwrap().0 {
                        best_x = Some((dist, line_x));
                    }
                }
            } else if dx > dy * 2.0 {
                // Horizontal line -> Y grid
                let line_y = (line.start[1] + line.end[1]) / 2.0;
                let dist = (ty - line_y).abs();
                if dist < 3000.0 {
                    if best_y.is_none() || dist < best_y.unwrap().0 {
                        best_y = Some((dist, line_y));
                    }
                }
            }
        }

        if let Some((_, line_x)) = best_x {
            if is_x_grid_label(&s) {
                x_candidates.push((s.clone(), line_x));
            }
        }
        if let Some((_, line_y)) = best_y {
            if is_y_grid_label(&s) {
                y_candidates.push((s.clone(), line_y));
            } else if best_x.is_none() {
                // Letter label near horizontal line only → treat as Y if no X match
                y_candidates.push((s.clone(), line_y));
            }
        }
    }

    // Deduplicate and sort
    x_candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    x_candidates.dedup_by(|a, b| (a.1 - b.1).abs() < 50.0);
    y_candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    y_candidates.dedup_by(|a, b| (a.1 - b.1).abs() < 50.0);

    let x_offset = x_candidates.first().map(|c| c.1).unwrap_or(0.0);
    let y_offset = y_candidates.first().map(|c| c.1).unwrap_or(0.0);

    let x_grids: Vec<GridLine> = x_candidates.into_iter()
        .map(|(name, pos)| GridLine { name, position: pos - x_offset })
        .collect();
    let y_grids: Vec<GridLine> = y_candidates.into_iter()
        .map(|(name, pos)| GridLine { name, position: pos - y_offset })
        .collect();

    (x_grids, y_grids)
}

/// Clean MTEXT formatting codes like {\fArial|b1|i0|...; content}
pub(crate) fn clean_mtext(s: &str) -> String {
    let mut clean = s.to_string();

    // Remove MTEXT formatting: {\fArial|b1|i0|...; content}
    // Pattern: { \f...; text } or { \H...; text } etc.
    let mut safety = 0;
    while let Some(start) = clean.find('{') {
        safety += 1;
        if safety > 50 { break; }
        if let Some(end) = clean[start..].find('}') {
            let inner = &clean[start + 1..start + end];
            // Find the semicolon that ends the format code
            if let Some(semi) = inner.find(';') {
                let text_part = inner[semi + 1..].to_string();
                clean = format!("{}{}{}", &clean[..start], text_part, &clean[start + end + 1..]);
            } else {
                let inner_owned = inner.to_string();
                clean = format!("{}{}{}", &clean[..start], inner_owned, &clean[start + end + 1..]);
            }
        } else {
            break;
        }
    }

    // Remove remaining escape sequences
    clean = clean.replace("\\P", "").replace("\\p", "");
    clean = clean.replace("%%u", "").replace("%%U", "");
    // Remove \A1; alignment codes
    while let Some(pos) = clean.find("\\A") {
        if pos + 4 <= clean.len() {
            if let Some(semi) = clean[pos..].find(';') {
                clean = format!("{}{}", &clean[..pos], &clean[pos + semi + 1..]);
            } else {
                break;
            }
        } else {
            break;
        }
    }
    // Remove \C color codes like \C1;
    while let Some(pos) = clean.find("\\C") {
        if let Some(semi) = clean[pos..].find(';') {
            clean = format!("{}{}", &clean[..pos], &clean[pos + semi + 1..]);
        } else {
            break;
        }
    }

    clean.trim().to_string()
}

pub(crate) fn is_x_grid_label(s: &str) -> bool {
    // A, B, C, ..., Z, AB, AC, etc.
    // Exclude known non-grid labels (floor levels, directions, abbreviations)
    const EXCLUDED: &[&str] = &[
        "GL", "FL", "RFL", "EL", "DN", "BFL", "TFL", "SFL",
        "UP", "DW", "FH", "CH", "WH", "TH", "OK", "NO", "NA",
        "NE", "NW", "SE", "SW", "G", "X", "Y", "Z", "R", "L",
    ];
    if EXCLUDED.contains(&s) { return false; }
    !s.is_empty() && s.len() <= 3 && s.chars().all(|c| c.is_ascii_uppercase())
}

pub(crate) fn is_y_grid_label(s: &str) -> bool {
    // 1, 2, 3, ..., 99
    s.parse::<u32>().map(|n| n >= 1 && n <= 99).unwrap_or(false)
}

fn grid_name(idx: usize, is_x: bool) -> String {
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

/// Deduplicate labels by name, keeping the first occurrence
/// `by_x` = true means compare X positions for closeness, false means compare Y
fn dedup_labels_by_name(labels: &mut Vec<(String, f64, f64)>, tolerance: f64, by_x: bool) {
    let mut seen: Vec<(String, f64, f64)> = Vec::new();
    for item in labels.iter() {
        let pos = if by_x { item.1 } else { item.2 };
        let already = seen.iter().any(|s| {
            let s_pos = if by_x { s.1 } else { s.2 };
            s.0 == item.0 && (s_pos - pos).abs() < tolerance
        });
        if !already {
            seen.push(item.clone());
        }
    }
    *labels = seen;
}

/// Extract numeric value from a dimension (from text field, value field, or endpoint distance)
fn dimension_numeric_value(dim: &super::geometry_parser::RawDimension) -> f64 {
    // First try the stored value
    if dim.value > 1.0 {
        return dim.value;
    }
    // Then try parsing the text field
    let cleaned = dim.text.replace(',', "").replace(' ', "");
    if let Ok(v) = cleaned.parse::<f64>() {
        if v > 1.0 {
            return v;
        }
    }
    // Finally compute from endpoints
    let dx = dim.end[0] - dim.start[0];
    let dy = dim.end[1] - dim.start[1];
    (dx * dx + dy * dy).sqrt()
}

/// Collect all meaningful dimension values from the geometry
fn collect_dimension_values(geom: &RawGeometry) -> Vec<f64> {
    let mut values: Vec<f64> = Vec::new();

    for dim in &geom.dimensions {
        let val = dimension_numeric_value(dim);
        if val > 100.0 && val < 100_000.0 {
            values.push(val);
        }
    }

    // Deduplicate very similar values
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    values.dedup_by(|a, b| (*a - *b).abs() < 10.0);

    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_mtext_formatting() {
        assert_eq!(clean_mtext("{\\fArial|b1|i0|c134|p34;A}"), "A");
        assert_eq!(clean_mtext("{\\fArial;B}"), "B");
        assert_eq!(clean_mtext("{\\H3.5;AB}"), "AB");
        assert_eq!(clean_mtext("C"), "C");
        assert_eq!(clean_mtext("{\\fArial|b1;1}"), "1");
        assert_eq!(clean_mtext("\\A1;D"), "D");
        assert_eq!(clean_mtext("plain text"), "plain text");
    }

    #[test]
    fn test_is_x_grid_label() {
        assert!(is_x_grid_label("A"));
        assert!(is_x_grid_label("AB"));
        assert!(is_x_grid_label("Z"));
        assert!(!is_x_grid_label("1"));
        assert!(!is_x_grid_label(""));
        assert!(!is_x_grid_label("ABCD"));
        assert!(!is_x_grid_label("a"));
    }

    #[test]
    fn test_is_y_grid_label() {
        assert!(is_y_grid_label("1"));
        assert!(is_y_grid_label("2"));
        assert!(is_y_grid_label("99"));
        assert!(!is_y_grid_label("0"));
        assert!(!is_y_grid_label("100"));
        assert!(!is_y_grid_label("A"));
    }

    #[test]
    fn test_text_based_grid_detection() {
        let geom = RawGeometry {
            lines: vec![],
            polylines: vec![],
            texts: vec![
                super::super::geometry_parser::RawText { content: "A".into(), position: [0.0, -500.0], height: 3.5, layer: "GRID".into() },
                super::super::geometry_parser::RawText { content: "AB".into(), position: [3040.0, -500.0], height: 3.5, layer: "GRID".into() },
                super::super::geometry_parser::RawText { content: "B".into(), position: [6840.0, -500.0], height: 3.5, layer: "GRID".into() },
                super::super::geometry_parser::RawText { content: "C".into(), position: [9790.0, -500.0], height: 3.5, layer: "GRID".into() },
                super::super::geometry_parser::RawText { content: "D".into(), position: [12830.0, -500.0], height: 3.5, layer: "GRID".into() },
                super::super::geometry_parser::RawText { content: "1".into(), position: [-500.0, 0.0], height: 3.5, layer: "GRID".into() },
                super::super::geometry_parser::RawText { content: "2".into(), position: [-500.0, 6000.0], height: 3.5, layer: "GRID".into() },
            ],
            dimensions: vec![],
            blocks: vec![],
            circles: vec![],
        };

        let grids = parse_grids(&geom);
        assert_eq!(grids.x_grids.len(), 5, "Expected 5 X grids (A, AB, B, C, D)");
        assert_eq!(grids.y_grids.len(), 2, "Expected 2 Y grids (1, 2)");
        assert_eq!(grids.x_grids[0].name, "A");
        assert_eq!(grids.x_grids[1].name, "AB");
        assert_eq!(grids.x_grids[4].name, "D");
    }
}

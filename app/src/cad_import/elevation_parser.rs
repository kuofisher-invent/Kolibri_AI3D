//! Elevation Parser — extracts level/height information from DXF

use super::geometry_parser::RawGeometry;
use super::ir::LevelDef;

pub fn parse_elevations(geom: &RawGeometry) -> Vec<LevelDef> {
    let mut levels = Vec::new();

    // Look for elevation texts: +0, +4200, -500, EL.+4200, FL.+3000, etc.
    for text in &geom.texts {
        let s = text.content.trim();

        if let Some(val) = parse_elevation_text(s) {
            let name = if val.abs() < 1.0 {
                "GL".to_string()
            } else {
                format!("EL{:+.0}", val)
            };

            // Avoid duplicates
            if !levels.iter().any(|l: &LevelDef| (l.elevation - val).abs() < 10.0) {
                levels.push(LevelDef { name, elevation: val });
            }
        } else {
            // Also check plain numbers that could be heights (e.g., "3495", "455")
            let cleaned = s.replace(',', "");
            if let Ok(val) = cleaned.parse::<f64>() {
                if val > 100.0 && val < 20000.0 {
                    if !levels.iter().any(|l: &LevelDef| (l.elevation - val).abs() < 10.0) {
                        levels.push(LevelDef { name: format!("H{:.0}", val), elevation: val });
                    }
                }
            }
        }
    }

    // Also check dimension values for vertical dimensions
    for dim in &geom.dimensions {
        let dx = (dim.start[0] - dim.end[0]).abs();
        let dy = (dim.start[1] - dim.end[1]).abs();
        // Vertical dimension (from value field)
        if dy > dx * 2.0 && dim.value > 100.0 {
            let _top_y = dim.start[1].max(dim.end[1]);

            if !levels.iter().any(|l: &LevelDef| (l.elevation - dim.value).abs() < 50.0) {
                levels.push(LevelDef {
                    name: format!("H{:.0}", dim.value),
                    elevation: dim.value,
                });
            }
        }
        // Also try parsing the dimension text field for elevation values
        if !dim.text.is_empty() {
            let clean = dim.text.replace("+", "").replace("EL.", "").replace("FL.", "")
                .replace("el.", "").replace("fl.", "").replace(",", "");
            if let Ok(v) = clean.trim().parse::<f64>() {
                if v > 100.0 && v < 20000.0 {
                    if !levels.iter().any(|l: &LevelDef| (l.elevation - v).abs() < 10.0) {
                        levels.push(LevelDef { name: format!("H{:.0}", v), elevation: v });
                    }
                }
            }
        }
    }

    // Sort by elevation
    levels.sort_by(|a, b| a.elevation.partial_cmp(&b.elevation).unwrap_or(std::cmp::Ordering::Equal));

    // If no levels found, add defaults
    if levels.is_empty() {
        levels.push(LevelDef { name: "GL".into(), elevation: 0.0 });
        levels.push(LevelDef { name: "TOP".into(), elevation: 3000.0 });
    }

    levels
}

fn parse_elevation_text(s: &str) -> Option<f64> {
    let s = s.replace("EL.", "").replace("FL.", "").replace("el.", "").replace("fl.", "");
    let s = s.trim();

    if let Ok(val) = s.parse::<f64>() {
        if val.abs() < 100_000.0 {
            return Some(val);
        }
    }

    if s.starts_with('+') {
        if let Ok(val) = s[1..].parse::<f64>() {
            return Some(val);
        }
    }

    None
}

//! DXF Geometry Parser — extracts raw geometric entities from DXF files

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawGeometry {
    pub lines: Vec<RawLine>,
    pub polylines: Vec<RawPolyline>,
    pub texts: Vec<RawText>,
    pub dimensions: Vec<RawDimension>,
    pub blocks: Vec<RawBlock>,
    pub circles: Vec<RawCircle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawLine {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub layer: String,
    pub linetype: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPolyline {
    pub points: Vec<[f64; 2]>,
    pub closed: bool,
    pub layer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawText {
    pub content: String,
    pub position: [f64; 2],
    pub height: f64,
    pub layer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawDimension {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub value: f64,
    pub text: String,
    pub layer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawBlock {
    pub name: String,
    pub insert_point: [f64; 2],
    pub layer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCircle {
    pub center: [f64; 2],
    pub radius: f64,
    pub layer: String,
}

/// Parse a DXF file into raw geometry
pub fn parse_dxf(path: &str) -> Result<RawGeometry, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("讀取失敗: {}", e))?;
    let lines: Vec<&str> = content.lines().collect();

    let mut geom = RawGeometry {
        lines: Vec::new(), polylines: Vec::new(), texts: Vec::new(),
        dimensions: Vec::new(), blocks: Vec::new(), circles: Vec::new(),
    };

    let mut i = 0;
    while i < lines.len().saturating_sub(1) {
        let code = lines[i].trim();
        let value = lines[i + 1].trim();

        if code == "0" {
            match value {
                "LINE" => {
                    if let Some(line) = parse_line_entity(&lines, &mut i) {
                        geom.lines.push(line);
                    }
                }
                "TEXT" | "MTEXT" => {
                    if let Some(text) = parse_text_entity(&lines, &mut i) {
                        geom.texts.push(text);
                    }
                }
                "DIMENSION" => {
                    if let Some(dim) = parse_dimension_entity(&lines, &mut i) {
                        geom.dimensions.push(dim);
                    }
                }
                "CIRCLE" => {
                    if let Some(circle) = parse_circle_entity(&lines, &mut i) {
                        geom.circles.push(circle);
                    }
                }
                "INSERT" => {
                    if let Some(block) = parse_insert_entity(&lines, &mut i) {
                        geom.blocks.push(block);
                    }
                }
                "LWPOLYLINE" => {
                    if let Some(poly) = parse_lwpolyline_entity(&lines, &mut i) {
                        geom.polylines.push(poly);
                    }
                }
                _ => { i += 2; }
            }
        } else {
            i += 2;
        }
    }

    Ok(geom)
}

fn parse_line_entity(lines: &[&str], i: &mut usize) -> Option<RawLine> {
    let mut line = RawLine { start: [0.0; 2], end: [0.0; 2], layer: String::new(), linetype: "CONTINUOUS".into() };
    *i += 2; // skip "0\nLINE"
    while *i < lines.len().saturating_sub(1) {
        let code: i32 = lines[*i].trim().parse().unwrap_or(-1);
        let val = lines[*i + 1].trim();
        match code {
            0 => break,
            8 => line.layer = val.to_string(),
            6 => line.linetype = val.to_string(),
            10 => line.start[0] = val.parse().unwrap_or(0.0),
            20 => line.start[1] = val.parse().unwrap_or(0.0),
            11 => line.end[0] = val.parse().unwrap_or(0.0),
            21 => line.end[1] = val.parse().unwrap_or(0.0),
            _ => {}
        }
        *i += 2;
    }
    Some(line)
}

fn parse_text_entity(lines: &[&str], i: &mut usize) -> Option<RawText> {
    let mut text = RawText { content: String::new(), position: [0.0; 2], height: 2.5, layer: String::new() };
    *i += 2;
    while *i < lines.len().saturating_sub(1) {
        let code: i32 = lines[*i].trim().parse().unwrap_or(-1);
        let val = lines[*i + 1].trim();
        match code {
            0 => break,
            8 => text.layer = val.to_string(),
            1 => text.content = val.to_string(),
            10 => text.position[0] = val.parse().unwrap_or(0.0),
            20 => text.position[1] = val.parse().unwrap_or(0.0),
            40 => text.height = val.parse().unwrap_or(2.5),
            _ => {}
        }
        *i += 2;
    }
    if text.content.is_empty() { return None; }
    Some(text)
}

fn parse_dimension_entity(lines: &[&str], i: &mut usize) -> Option<RawDimension> {
    let mut dim = RawDimension { start: [0.0; 2], end: [0.0; 2], value: 0.0, text: String::new(), layer: String::new() };
    *i += 2;
    while *i < lines.len().saturating_sub(1) {
        let code: i32 = lines[*i].trim().parse().unwrap_or(-1);
        let val = lines[*i + 1].trim();
        match code {
            0 => break,
            8 => dim.layer = val.to_string(),
            1 => dim.text = val.to_string(),
            13 => dim.start[0] = val.parse().unwrap_or(0.0),
            23 => dim.start[1] = val.parse().unwrap_or(0.0),
            14 => dim.end[0] = val.parse().unwrap_or(0.0),
            24 => dim.end[1] = val.parse().unwrap_or(0.0),
            42 => dim.value = val.parse().unwrap_or(0.0),
            _ => {}
        }
        *i += 2;
    }
    Some(dim)
}

fn parse_circle_entity(lines: &[&str], i: &mut usize) -> Option<RawCircle> {
    let mut circle = RawCircle { center: [0.0; 2], radius: 0.0, layer: String::new() };
    *i += 2;
    while *i < lines.len().saturating_sub(1) {
        let code: i32 = lines[*i].trim().parse().unwrap_or(-1);
        let val = lines[*i + 1].trim();
        match code {
            0 => break,
            8 => circle.layer = val.to_string(),
            10 => circle.center[0] = val.parse().unwrap_or(0.0),
            20 => circle.center[1] = val.parse().unwrap_or(0.0),
            40 => circle.radius = val.parse().unwrap_or(0.0),
            _ => {}
        }
        *i += 2;
    }
    Some(circle)
}

fn parse_insert_entity(lines: &[&str], i: &mut usize) -> Option<RawBlock> {
    let mut block = RawBlock { name: String::new(), insert_point: [0.0; 2], layer: String::new() };
    *i += 2;
    while *i < lines.len().saturating_sub(1) {
        let code: i32 = lines[*i].trim().parse().unwrap_or(-1);
        let val = lines[*i + 1].trim();
        match code {
            0 => break,
            8 => block.layer = val.to_string(),
            2 => block.name = val.to_string(),
            10 => block.insert_point[0] = val.parse().unwrap_or(0.0),
            20 => block.insert_point[1] = val.parse().unwrap_or(0.0),
            _ => {}
        }
        *i += 2;
    }
    Some(block)
}

fn parse_lwpolyline_entity(lines: &[&str], i: &mut usize) -> Option<RawPolyline> {
    let mut poly = RawPolyline { points: Vec::new(), closed: false, layer: String::new() };
    let mut current_x: Option<f64> = None;
    *i += 2;
    while *i < lines.len().saturating_sub(1) {
        let code: i32 = lines[*i].trim().parse().unwrap_or(-1);
        let val = lines[*i + 1].trim();
        match code {
            0 => break,
            8 => poly.layer = val.to_string(),
            70 => poly.closed = val.parse::<i32>().unwrap_or(0) & 1 == 1,
            10 => {
                if let Some(x) = current_x {
                    poly.points.push([x, 0.0]);
                }
                current_x = Some(val.parse().unwrap_or(0.0));
            }
            20 => {
                if let Some(x) = current_x.take() {
                    poly.points.push([x, val.parse().unwrap_or(0.0)]);
                }
            }
            _ => {}
        }
        *i += 2;
    }
    if let Some(x) = current_x {
        poly.points.push([x, 0.0]);
    }
    if poly.points.len() >= 2 { Some(poly) } else { None }
}

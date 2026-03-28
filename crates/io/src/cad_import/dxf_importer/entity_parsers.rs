use super::types::*;

pub(super) fn parse_single_entity(entity_type: &str, payload: &[(i32, String)]) -> ImportResult<ParsedEntity> {
    match entity_type {
        "LINE" => Ok(ParsedEntity::Line(parse_line(payload))),
        "LWPOLYLINE" => Ok(ParsedEntity::Polyline(parse_lwpolyline(payload))),
        "SPLINE" => Ok(ParsedEntity::Polyline(parse_spline(payload))),
        "ELLIPSE" => Ok(ParsedEntity::Polyline(parse_ellipse(payload))),
        "SOLID" | "3DFACE" => Ok(ParsedEntity::Polyline(parse_solid_or_3dface(payload))),
        "HATCH" => Ok(ParsedEntity::Unsupported(to_raw(entity_type, payload))),
        "CIRCLE" => Ok(ParsedEntity::Circle(parse_circle(payload))),
        "ARC" => Ok(ParsedEntity::Arc(parse_arc(payload))),
        "TEXT" | "MTEXT" => Ok(ParsedEntity::Text(parse_text(payload))),
        "DIMENSION" => Ok(ParsedEntity::Dimension(parse_dimension(payload))),
        "INSERT" => Ok(ParsedEntity::Insert(parse_insert(payload))),
        _ => Ok(ParsedEntity::Unsupported(to_raw(entity_type, payload))),
    }
}

/// CHANGELOG: v0.1.0 - Parse LINE entity.
fn parse_line(payload: &[(i32, String)]) -> LineIr {
    let mut layer = String::from("0");
    let mut start = [0.0, 0.0, 0.0];
    let mut end = [0.0, 0.0, 0.0];
    let mut color = None;

    for (code, value) in payload {
        match *code {
            8 => layer = value.clone(),
            10 => start[0] = value.parse::<f32>().unwrap_or(0.0),
            20 => start[1] = value.parse::<f32>().unwrap_or(0.0),
            30 => start[2] = value.parse::<f32>().unwrap_or(0.0),
            11 => end[0] = value.parse::<f32>().unwrap_or(0.0),
            21 => end[1] = value.parse::<f32>().unwrap_or(0.0),
            31 => end[2] = value.parse::<f32>().unwrap_or(0.0),
            62 => color = value.parse::<i16>().ok().map(CadColor),
            _ => {}
        }
    }

    LineIr {
        layer,
        start,
        end,
        color,
    }
}

/// CHANGELOG: v0.1.0 - Parse LWPOLYLINE entity.
fn parse_lwpolyline(payload: &[(i32, String)]) -> PolylineIr {
    let mut layer = String::from("0");
    let mut points = Vec::new();
    let mut is_closed = false;
    let mut color = None;

    let mut current_x = None;

    for (code, value) in payload {
        match *code {
            8 => layer = value.clone(),
            62 => color = value.parse::<i16>().ok().map(CadColor),
            70 => {
                let flags = value.parse::<i32>().unwrap_or(0);
                is_closed = (flags & 1) != 0;
            }
            10 => current_x = value.parse::<f32>().ok(),
            20 => {
                if let Some(x) = current_x.take() {
                    let y = value.parse::<f32>().unwrap_or(0.0);
                    points.push([x, y, 0.0]);
                }
            }
            _ => {}
        }
    }

    PolylineIr {
        layer,
        points,
        is_closed,
        color,
    }
}

/// CHANGELOG: v0.1.0 - Parse CIRCLE entity.
fn parse_circle(payload: &[(i32, String)]) -> CircleIr {
    let mut layer = String::from("0");
    let mut center = [0.0, 0.0, 0.0];
    let mut radius = 0.0;
    let mut color = None;

    for (code, value) in payload {
        match *code {
            8 => layer = value.clone(),
            10 => center[0] = value.parse::<f32>().unwrap_or(0.0),
            20 => center[1] = value.parse::<f32>().unwrap_or(0.0),
            30 => center[2] = value.parse::<f32>().unwrap_or(0.0),
            40 => radius = value.parse::<f32>().unwrap_or(0.0),
            62 => color = value.parse::<i16>().ok().map(CadColor),
            _ => {}
        }
    }

    CircleIr {
        layer,
        center,
        radius,
        color,
    }
}

/// CHANGELOG: v0.1.0 - Parse ARC entity.
fn parse_arc(payload: &[(i32, String)]) -> ArcIr {
    let mut layer = String::from("0");
    let mut center = [0.0, 0.0, 0.0];
    let mut radius = 0.0;
    let mut start_angle_deg = 0.0;
    let mut end_angle_deg = 0.0;
    let mut color = None;

    for (code, value) in payload {
        match *code {
            8 => layer = value.clone(),
            10 => center[0] = value.parse::<f32>().unwrap_or(0.0),
            20 => center[1] = value.parse::<f32>().unwrap_or(0.0),
            30 => center[2] = value.parse::<f32>().unwrap_or(0.0),
            40 => radius = value.parse::<f32>().unwrap_or(0.0),
            50 => start_angle_deg = value.parse::<f32>().unwrap_or(0.0),
            51 => end_angle_deg = value.parse::<f32>().unwrap_or(0.0),
            62 => color = value.parse::<i16>().ok().map(CadColor),
            _ => {}
        }
    }

    ArcIr {
        layer,
        center,
        radius,
        start_angle_deg,
        end_angle_deg,
        color,
    }
}

/// CHANGELOG: v0.2.0 - Parse TEXT / MTEXT entity.
/// MTEXT uses code 3 for continuation chunks (before code 1 which is the final chunk).
/// They are concatenated directly without separators.
fn parse_text(payload: &[(i32, String)]) -> TextIr {
    let mut layer = String::from("0");
    let mut continuation = String::new(); // code 3 chunks
    let mut final_text = String::new(); // code 1
    let mut position = [0.0f32, 0.0, 0.0];
    let mut height = 0.0f32;
    let mut rotation_deg = 0.0f32;

    for (code, value) in payload {
        match *code {
            8 => layer = value.clone(),
            3 => continuation.push_str(value), // MTEXT continuation — concatenate directly
            1 => final_text = value.clone(),
            10 => position[0] = value.parse().unwrap_or(0.0),
            20 => position[1] = value.parse().unwrap_or(0.0),
            30 => position[2] = value.parse().unwrap_or(0.0),
            40 => height = value.parse().unwrap_or(0.0),
            50 => rotation_deg = value.parse().unwrap_or(0.0),
            _ => {}
        }
    }

    // MTEXT: continuation (code 3) comes before final chunk (code 1)
    let value = if continuation.is_empty() {
        final_text
    } else {
        continuation.push_str(&final_text);
        continuation
    };

    TextIr {
        layer,
        value,
        position,
        height,
        rotation_deg,
    }
}

/// CHANGELOG: v0.2.0 - Parse DIMENSION entity with measured value (code 42) and Z coords.
fn parse_dimension(payload: &[(i32, String)]) -> DimensionIr {
    let mut layer = String::from("0");
    let mut value_text = None;
    let mut definition_points = Vec::new();
    let mut current_x: Option<f32> = None;
    let mut current_y: Option<f32> = None;

    for (code, value) in payload {
        match *code {
            8 => layer = value.clone(),
            1 => value_text = Some(value.clone()),
            10 | 13 | 14 => {
                // If we had a pending x without a matching y, flush it
                if let (Some(x), Some(y)) = (current_x.take(), current_y.take()) {
                    definition_points.push([x, y, 0.0]);
                }
                current_x = value.parse::<f32>().ok();
                current_y = None;
            }
            20 | 23 | 24 => {
                current_y = value.parse::<f32>().ok();
                if let (Some(x), Some(y)) = (current_x.take(), current_y.take()) {
                    definition_points.push([x, y, 0.0]);
                }
            }
            42 => {
                // Actual measured distance — use as fallback text if none provided
                if value_text.is_none() {
                    if let Ok(v) = value.parse::<f32>() {
                        value_text = Some(format!("{:.0}", v));
                    }
                }
            }
            _ => {}
        }
    }

    // Flush any remaining point
    if let (Some(x), Some(y)) = (current_x, current_y) {
        definition_points.push([x, y, 0.0]);
    }

    DimensionIr {
        layer,
        value_text,
        definition_points,
    }
}

/// CHANGELOG: v0.1.0 - Parse INSERT entity.
fn parse_insert(payload: &[(i32, String)]) -> InsertIr {
    let mut layer = String::from("0");
    let mut block_name = String::new();
    let mut position = [0.0, 0.0, 0.0];
    let mut rotation_deg = 0.0;
    let mut scale = [1.0, 1.0, 1.0];

    for (code, value) in payload {
        match *code {
            8 => layer = value.clone(),
            2 => block_name = value.clone(),
            10 => position[0] = value.parse::<f32>().unwrap_or(0.0),
            20 => position[1] = value.parse::<f32>().unwrap_or(0.0),
            30 => position[2] = value.parse::<f32>().unwrap_or(0.0),
            41 => scale[0] = value.parse::<f32>().unwrap_or(1.0),
            42 => scale[1] = value.parse::<f32>().unwrap_or(1.0),
            43 => scale[2] = value.parse::<f32>().unwrap_or(1.0),
            50 => rotation_deg = value.parse::<f32>().unwrap_or(0.0),
            _ => {}
        }
    }

    InsertIr {
        layer,
        block_name,
        position,
        rotation_deg,
        scale,
    }
}

/// CHANGELOG: v0.2.0 - Parse SPLINE entity (approximated as polyline from control points).
fn parse_spline(payload: &[(i32, String)]) -> PolylineIr {
    let mut layer = String::from("0");
    let mut points = Vec::new();
    let mut color = None;
    let mut is_closed = false;
    let mut current_x: Option<f32> = None;
    let mut current_y: Option<f32> = None;

    for (code, value) in payload {
        match *code {
            8 => layer = value.clone(),
            62 => color = value.parse::<i16>().ok().map(CadColor),
            70 => is_closed = (value.parse::<i32>().unwrap_or(0) & 1) != 0,
            10 => {
                // Flush previous point if we had x+y
                if let (Some(x), Some(y)) = (current_x.take(), current_y.take()) {
                    points.push([x, y, 0.0]);
                }
                current_x = value.parse().ok();
                current_y = None;
            }
            20 => {
                current_y = value.parse().ok();
                if let (Some(x), Some(y)) = (current_x.take(), current_y.take()) {
                    points.push([x, y, 0.0]);
                }
            }
            30 => {
                // Update Z on the last pushed point
                if let Some(last) = points.last_mut() {
                    last[2] = value.parse().unwrap_or(0.0);
                }
            }
            _ => {}
        }
    }
    // Flush remaining point
    if let (Some(x), Some(y)) = (current_x, current_y) {
        points.push([x, y, 0.0]);
    }

    PolylineIr {
        layer,
        points,
        is_closed,
        color,
    }
}

/// CHANGELOG: v0.2.0 - Parse ELLIPSE entity (approximated as polyline with 32 segments).
fn parse_ellipse(payload: &[(i32, String)]) -> PolylineIr {
    let mut layer = String::from("0");
    let mut center = [0.0f32; 3];
    let mut major_endpoint = [0.0f32; 3];
    let mut ratio = 1.0f32;
    let mut start_angle = 0.0f32;
    let mut end_angle = std::f32::consts::TAU;
    let mut color = None;

    for (code, value) in payload {
        match *code {
            8 => layer = value.clone(),
            10 => center[0] = value.parse().unwrap_or(0.0),
            20 => center[1] = value.parse().unwrap_or(0.0),
            30 => center[2] = value.parse().unwrap_or(0.0),
            11 => major_endpoint[0] = value.parse().unwrap_or(0.0),
            21 => major_endpoint[1] = value.parse().unwrap_or(0.0),
            31 => major_endpoint[2] = value.parse().unwrap_or(0.0),
            40 => ratio = value.parse().unwrap_or(1.0),
            41 => start_angle = value.parse().unwrap_or(0.0),
            42 => end_angle = value.parse().unwrap_or(std::f32::consts::TAU),
            62 => color = value.parse::<i16>().ok().map(CadColor),
            _ => {}
        }
    }

    let major_len = (major_endpoint[0] * major_endpoint[0]
        + major_endpoint[1] * major_endpoint[1])
    .sqrt();
    let segments = 32;
    let mut points = Vec::with_capacity(segments + 1);
    let angle = major_endpoint[1].atan2(major_endpoint[0]);
    let (sa, ca) = angle.sin_cos();

    for seg in 0..=segments {
        let t = start_angle + (end_angle - start_angle) * (seg as f32 / segments as f32);
        let cos_t = t.cos();
        let sin_t = t.sin();
        let px = major_len * cos_t;
        let py = major_len * ratio * sin_t;
        points.push([
            center[0] + px * ca - py * sa,
            center[1] + px * sa + py * ca,
            center[2],
        ]);
    }

    let is_closed = (end_angle - start_angle - std::f32::consts::TAU).abs() < 0.01;

    PolylineIr {
        layer,
        points,
        is_closed,
        color,
    }
}

/// CHANGELOG: v0.2.0 - Parse SOLID / 3DFACE entity (3-4 corner points as closed polyline).
fn parse_solid_or_3dface(payload: &[(i32, String)]) -> PolylineIr {
    let mut layer = String::from("0");
    let mut pts = [[0.0f32; 3]; 4];
    let mut color = None;

    for (code, value) in payload {
        match *code {
            8 => layer = value.clone(),
            62 => color = value.parse::<i16>().ok().map(CadColor),
            10 => pts[0][0] = value.parse().unwrap_or(0.0),
            20 => pts[0][1] = value.parse().unwrap_or(0.0),
            30 => pts[0][2] = value.parse().unwrap_or(0.0),
            11 => pts[1][0] = value.parse().unwrap_or(0.0),
            21 => pts[1][1] = value.parse().unwrap_or(0.0),
            31 => pts[1][2] = value.parse().unwrap_or(0.0),
            12 => pts[2][0] = value.parse().unwrap_or(0.0),
            22 => pts[2][1] = value.parse().unwrap_or(0.0),
            32 => pts[2][2] = value.parse().unwrap_or(0.0),
            13 => pts[3][0] = value.parse().unwrap_or(0.0),
            23 => pts[3][1] = value.parse().unwrap_or(0.0),
            33 => pts[3][2] = value.parse().unwrap_or(0.0),
            _ => {}
        }
    }

    PolylineIr {
        layer,
        points: pts.to_vec(),
        is_closed: true,
        color,
    }
}

/// CHANGELOG: v0.1.0 - Convert unknown entity payload into raw debug record.
fn to_raw(entity_type: &str, payload: &[(i32, String)]) -> RawEntityIr {
    let layer = payload
        .iter()
        .find_map(|(code, value)| if *code == 8 { Some(value.clone()) } else { None })
        .unwrap_or_else(|| "0".to_string());

    RawEntityIr {
        entity_type: entity_type.to_string(),
        layer,
        group_codes: payload.to_vec(),
    }
}

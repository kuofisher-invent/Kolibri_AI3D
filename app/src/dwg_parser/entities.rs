//! DWG Entity Decoder
//!
//! Decodes individual DWG objects/entities from the binary data.
//! Each entity has: common header + type-specific data + common tail.

use super::bitreader::{BitReader, DwgHandle, DwgReadError};
use super::objects::ObjectMap;
use super::sections::{DwgSection, SectionType};
use super::version::DwgVersionInfo;
use crate::cad_import::dxf_importer::*;

/// DWG entity types (from OpenDesign Specification)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DwgEntityType {
    Line,
    Point,
    Circle,
    Arc,
    LwPolyline,
    Text,
    MText,
    Dimension,
    Insert,
    Polyline2d,
    Polyline3d,
    Vertex2d,
    Vertex3d,
    Seqend,
    Spline,
    Ellipse,
    Solid,
    Face3d,
    Block,
    EndBlock,
    Layer,
    Unknown(u16),
}

impl DwgEntityType {
    pub fn from_raw(v: u16) -> Self {
        match v {
            19 => Self::Line,
            27 => Self::Point,
            17 => Self::Circle,
            16 => Self::Arc,
            1 => Self::Text,
            44 => Self::MText,
            21 => Self::Dimension,
            7 => Self::Insert,
            15 => Self::Polyline2d,
            28 => Self::Polyline3d,
            20 => Self::Vertex2d,
            29 => Self::Vertex3d,
            6 => Self::Seqend,
            36 => Self::Spline,
            35 => Self::Ellipse,
            31 => Self::Solid,
            30 => Self::Face3d,
            4 => Self::Block,
            5 => Self::EndBlock,
            51 => Self::Layer,
            other => Self::Unknown(other),
        }
    }
}

/// A decoded DWG entity
#[derive(Debug, Clone)]
pub struct DwgEntity {
    pub entity_type: DwgEntityType,
    pub handle: u32,
    pub layer_handle: u32,
    pub data: EntityData,
}

/// Entity-specific data
#[derive(Debug, Clone)]
pub enum EntityData {
    Line { start: [f64; 3], end: [f64; 3] },
    Circle { center: [f64; 3], radius: f64 },
    Arc { center: [f64; 3], radius: f64, start_angle: f64, end_angle: f64 },
    Text { position: [f64; 3], height: f64, text: String, rotation: f64 },
    MText { position: [f64; 3], text: String },
    LwPolyline { points: Vec<[f64; 2]>, closed: bool },
    Insert { block_handle: u32, position: [f64; 3], scale: [f64; 3], rotation: f64 },
    Dimension { def_points: Vec<[f64; 3]>, text: String },
    Spline { control_points: Vec<[f64; 3]> },
    Point { position: [f64; 3] },
    Unknown,
}

/// Parse all entities from the DWG data
pub fn parse_entities(
    full_data: &[u8],
    object_map: &ObjectMap,
    sections: &[DwgSection],
    ver: &DwgVersionInfo,
) -> Result<Vec<DwgEntity>, ImportError> {
    let mut entities = Vec::new();
    let mut type_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    // Method 1: Use object map offsets
    for (&handle, &offset) in object_map {
        let offset = offset as usize;
        if offset + 4 >= full_data.len() { continue; }

        match decode_entity(full_data, offset, handle, ver) {
            Ok(entity) => {
                let type_name = format!("{:?}", entity.entity_type);
                *type_counts.entry(type_name).or_insert(0) += 1;
                entities.push(entity);
            }
            Err(_) => {
                // Skip unreadable objects — common with heuristic scanning
            }
        }
    }

    // Method 2: If object map gave us nothing, scan section data directly
    if entities.is_empty() {
        for section in sections {
            if section.section_type == SectionType::Objects {
                scan_entities_in_data(&section.data, ver, &mut entities, &mut type_counts);
            }
        }
    }

    // Log entity counts
    let mut counts_sorted: Vec<_> = type_counts.into_iter().collect();
    counts_sorted.sort_by(|a, b| b.1.cmp(&a.1));
    for (t, c) in &counts_sorted {
        log::info!("  DWG entity {}: {}", t, c);
    }

    Ok(entities)
}

/// Decode a single entity at the given offset
fn decode_entity(
    data: &[u8],
    offset: usize,
    handle: u32,
    ver: &DwgVersionInfo,
) -> Result<DwgEntity, DwgReadError> {
    if offset + 10 > data.len() {
        return Err(DwgReadError::Eof);
    }

    let mut reader = BitReader::from_offset(data, offset);

    // Read object size (MS for R2000, or different for R2004+)
    let obj_size = reader.read_ms().unwrap_or(0) as usize;
    if obj_size < 4 || obj_size > 100_000 {
        return Err(DwgReadError::InvalidData("Invalid object size".into()));
    }

    // Read entity type (BS)
    let entity_type_raw = reader.read_bs().unwrap_or(0) as u16;
    let entity_type = DwgEntityType::from_raw(entity_type_raw);

    // Read common entity data size (RL for R2000)
    let _data_size = reader.read_rl().unwrap_or(0);

    // Read entity handle
    let entity_handle = reader.read_handle().unwrap_or(super::bitreader::DwgHandle { code: 0, value: handle });

    // Read layer handle (simplified — actual format has extended entity data first)
    let layer_handle = reader.read_handle().unwrap_or(super::bitreader::DwgHandle { code: 0, value: 0 });

    // Decode entity-specific data
    let entity_data = match entity_type {
        DwgEntityType::Line => decode_line(&mut reader)?,
        DwgEntityType::Circle => decode_circle(&mut reader)?,
        DwgEntityType::Arc => decode_arc(&mut reader)?,
        DwgEntityType::Text => decode_text(&mut reader, ver)?,
        DwgEntityType::Point => decode_point(&mut reader)?,
        _ => EntityData::Unknown,
    };

    Ok(DwgEntity {
        entity_type,
        handle: entity_handle.value,
        layer_handle: layer_handle.value,
        data: entity_data,
    })
}

fn decode_line(reader: &mut BitReader) -> Result<EntityData, DwgReadError> {
    let start = reader.read_3bd()?;
    let end = reader.read_3bd()?;
    Ok(EntityData::Line { start, end })
}

fn decode_circle(reader: &mut BitReader) -> Result<EntityData, DwgReadError> {
    let center = reader.read_3bd()?;
    let radius = reader.read_bd()?;
    Ok(EntityData::Circle { center, radius })
}

fn decode_arc(reader: &mut BitReader) -> Result<EntityData, DwgReadError> {
    let center = reader.read_3bd()?;
    let radius = reader.read_bd()?;
    let start_angle = reader.read_bd()?;
    let end_angle = reader.read_bd()?;
    Ok(EntityData::Arc { center, radius, start_angle, end_angle })
}

fn decode_text(reader: &mut BitReader, ver: &DwgVersionInfo) -> Result<EntityData, DwgReadError> {
    let position = reader.read_3bd()?;
    let height = reader.read_bd()?;
    let text = reader.read_text(ver.is_r2007_plus)?;
    let rotation = reader.read_bd().unwrap_or(0.0);
    Ok(EntityData::Text { position, height, text, rotation })
}

fn decode_point(reader: &mut BitReader) -> Result<EntityData, DwgReadError> {
    let position = reader.read_3bd()?;
    Ok(EntityData::Point { position })
}

/// Scan raw data for entity patterns (fallback when object map fails)
fn scan_entities_in_data(
    data: &[u8],
    ver: &DwgVersionInfo,
    entities: &mut Vec<DwgEntity>,
    type_counts: &mut std::collections::HashMap<String, usize>,
) {
    // Scan for f64 coordinate pairs that form valid LINE/CIRCLE/ARC patterns
    let mut i = 0;
    let mut handle = 1u32;

    while i + 48 <= data.len() {
        // Try to read 6 consecutive f64s (potential LINE: start xyz + end xyz)
        let vals: Vec<f64> = (0..6).map(|j| {
            f64::from_le_bytes(data[i + j * 8..i + j * 8 + 8].try_into().unwrap_or([0; 8]))
        }).collect();

        let all_valid = vals.iter().all(|v| v.is_finite() && v.abs() < 1e7);

        if all_valid {
            let has_meaningful = vals.iter().any(|v| v.abs() > 0.1);
            if has_meaningful {
                // Check if this could be a LINE (two distinct 3D points)
                let start = [vals[0], vals[1], vals[2]];
                let end = [vals[3], vals[4], vals[5]];
                let dist = ((end[0]-start[0]).powi(2) + (end[1]-start[1]).powi(2) + (end[2]-start[2]).powi(2)).sqrt();

                if dist > 0.1 && dist < 1e6 {
                    entities.push(DwgEntity {
                        entity_type: DwgEntityType::Line,
                        handle,
                        layer_handle: 0,
                        data: EntityData::Line { start, end },
                    });
                    *type_counts.entry("Line(scan)".into()).or_insert(0) += 1;
                    handle += 1;
                    i += 48;
                    continue;
                }
            }
        }

        i += 8;
    }
}

/// Convert decoded entities to GeometryIR
pub fn fill_geometry_ir(entities: &[DwgEntity], ir: &mut GeometryIr) {
    for entity in entities {
        match &entity.data {
            EntityData::Line { start, end } => {
                ir.curves.push(CurveIr::Line(LineIr {
                    layer: "0".into(),
                    start: [start[0] as f32, start[1] as f32, start[2] as f32],
                    end: [end[0] as f32, end[1] as f32, end[2] as f32],
                    color: None,
                }));
            }
            EntityData::Circle { center, radius } => {
                ir.curves.push(CurveIr::Circle(CircleIr {
                    layer: "0".into(),
                    center: [center[0] as f32, center[1] as f32, center[2] as f32],
                    radius: *radius as f32,
                    color: None,
                }));
            }
            EntityData::Arc { center, radius, start_angle, end_angle } => {
                ir.curves.push(CurveIr::Arc(ArcIr {
                    layer: "0".into(),
                    center: [center[0] as f32, center[1] as f32, center[2] as f32],
                    radius: *radius as f32,
                    start_angle_deg: start_angle.to_degrees() as f32,
                    end_angle_deg: end_angle.to_degrees() as f32,
                    color: None,
                }));
            }
            EntityData::Text { position, height, text, rotation } => {
                ir.texts.push(TextIr {
                    layer: "0".into(),
                    value: text.clone(),
                    position: [position[0] as f32, position[1] as f32, position[2] as f32],
                    height: *height as f32,
                    rotation_deg: rotation.to_degrees() as f32,
                });
            }
            EntityData::Dimension { def_points, text } => {
                ir.dimensions.push(DimensionIr {
                    layer: "0".into(),
                    value_text: if text.is_empty() { None } else { Some(text.clone()) },
                    definition_points: def_points.iter()
                        .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
                        .collect(),
                });
            }
            EntityData::LwPolyline { points, closed } => {
                ir.curves.push(CurveIr::Polyline(PolylineIr {
                    layer: "0".into(),
                    points: points.iter().map(|p| [p[0] as f32, p[1] as f32, 0.0]).collect(),
                    is_closed: *closed,
                    color: None,
                }));
            }
            EntityData::Insert { block_handle, position, scale, rotation } => {
                ir.inserts.push(InsertIr {
                    layer: "0".into(),
                    block_name: format!("BLOCK_{}", block_handle),
                    position: [position[0] as f32, position[1] as f32, position[2] as f32],
                    rotation_deg: rotation.to_degrees() as f32,
                    scale: [scale[0] as f32, scale[1] as f32, scale[2] as f32],
                });
            }
            _ => {}
        }
    }
}

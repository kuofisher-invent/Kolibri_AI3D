// version: v0.1.0
// changelog: Initial skeleton for DXF importer pipeline with IR conversion hooks.
// DEPENDENCY: std
// REQUIRED_METHODS: import_dxf, parse_text_dxf, parse_entities, to_import_report
// SIDE_EFFECTS: Reads DXF file from disk and returns parsed IR data only.

#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// CHANGELOG: v0.1.0 - Added importer result alias.
pub type ImportResult<T> = Result<T, ImportError>;

/// CHANGELOG: v0.1.0 - Added importer error model.
#[derive(Debug, Clone)]
pub enum ImportError {
    Io(String),
    InvalidFormat(String),
    UnsupportedEntity(String),
    Parse(String),
}

impl From<std::io::Error> for ImportError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

/// CHANGELOG: v0.1.0 - Added source format enum for future import manager integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    Dxf,
    DwgConvertedDxf,
}

/// CHANGELOG: v0.1.0 - Added import configuration.
#[derive(Debug, Clone)]
pub struct DxfImportConfig {
    pub assume_units: Unit,
    pub explode_inserts: bool,
    pub preserve_raw_entities: bool,
    pub merge_collinear_lines: bool,
}

impl Default for DxfImportConfig {
    fn default() -> Self {
        Self {
            assume_units: Unit::Millimeter,
            explode_inserts: false,
            preserve_raw_entities: true,
            merge_collinear_lines: false,
        }
    }
}

/// CHANGELOG: v0.1.0 - Added basic unit enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Millimeter,
    Centimeter,
    Meter,
    Inch,
    Foot,
    Unknown,
}

/// CHANGELOG: v0.1.0 - Added simple 3D point alias.
pub type Vec3 = [f32; 3];

/// CHANGELOG: v0.1.0 - Added generic color index wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CadColor(pub i16);

/// CHANGELOG: v0.1.0 - Added normalized intermediate representation root.
#[derive(Debug, Clone)]
pub struct GeometryIr {
    pub source_path: PathBuf,
    pub source_format: SourceFormat,
    pub units: Unit,
    pub layers: Vec<LayerIr>,
    pub curves: Vec<CurveIr>,
    pub texts: Vec<TextIr>,
    pub dimensions: Vec<DimensionIr>,
    pub blocks: Vec<BlockDefinitionIr>,
    pub inserts: Vec<InsertIr>,
    pub raw_entities: Vec<RawEntityIr>,
    pub metadata: HashMap<String, String>,
}

impl GeometryIr {
    /// CHANGELOG: v0.1.0 - Added root constructor.
    pub fn new(source_path: PathBuf, source_format: SourceFormat, units: Unit) -> Self {
        Self {
            source_path,
            source_format,
            units,
            layers: Vec::new(),
            curves: Vec::new(),
            texts: Vec::new(),
            dimensions: Vec::new(),
            blocks: Vec::new(),
            inserts: Vec::new(),
            raw_entities: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// CHANGELOG: v0.1.0 - Added summary helper.
    pub fn to_import_report(&self) -> ImportReport {
        ImportReport {
            layer_count: self.layers.len(),
            curve_count: self.curves.len(),
            text_count: self.texts.len(),
            dimension_count: self.dimensions.len(),
            block_count: self.blocks.len(),
            insert_count: self.inserts.len(),
            raw_entity_count: self.raw_entities.len(),
            units: self.units,
        }
    }
}

/// CHANGELOG: v0.1.0 - Added import summary struct.
#[derive(Debug, Clone)]
pub struct ImportReport {
    pub layer_count: usize,
    pub curve_count: usize,
    pub text_count: usize,
    pub dimension_count: usize,
    pub block_count: usize,
    pub insert_count: usize,
    pub raw_entity_count: usize,
    pub units: Unit,
}

/// CHANGELOG: v0.1.0 - Added layer IR.
#[derive(Debug, Clone)]
pub struct LayerIr {
    pub name: String,
    pub color: Option<CadColor>,
    pub is_visible: bool,
}

/// CHANGELOG: v0.1.0 - Added basic curve IR.
#[derive(Debug, Clone)]
pub enum CurveIr {
    Line(LineIr),
    Polyline(PolylineIr),
    Circle(CircleIr),
    Arc(ArcIr),
}

/// CHANGELOG: v0.1.0 - Added line IR.
#[derive(Debug, Clone)]
pub struct LineIr {
    pub layer: String,
    pub start: Vec3,
    pub end: Vec3,
    pub color: Option<CadColor>,
}

/// CHANGELOG: v0.1.0 - Added polyline IR.
#[derive(Debug, Clone)]
pub struct PolylineIr {
    pub layer: String,
    pub points: Vec<Vec3>,
    pub is_closed: bool,
    pub color: Option<CadColor>,
}

/// CHANGELOG: v0.1.0 - Added circle IR.
#[derive(Debug, Clone)]
pub struct CircleIr {
    pub layer: String,
    pub center: Vec3,
    pub radius: f32,
    pub color: Option<CadColor>,
}

/// CHANGELOG: v0.1.0 - Added arc IR.
#[derive(Debug, Clone)]
pub struct ArcIr {
    pub layer: String,
    pub center: Vec3,
    pub radius: f32,
    pub start_angle_deg: f32,
    pub end_angle_deg: f32,
    pub color: Option<CadColor>,
}

/// CHANGELOG: v0.1.0 - Added text IR.
#[derive(Debug, Clone)]
pub struct TextIr {
    pub layer: String,
    pub value: String,
    pub position: Vec3,
    pub height: f32,
    pub rotation_deg: f32,
}

/// CHANGELOG: v0.1.0 - Added dimension IR placeholder.
#[derive(Debug, Clone)]
pub struct DimensionIr {
    pub layer: String,
    pub value_text: Option<String>,
    pub definition_points: Vec<Vec3>,
}

/// CHANGELOG: v0.1.0 - Added block definition IR.
#[derive(Debug, Clone)]
pub struct BlockDefinitionIr {
    pub name: String,
    pub base_point: Vec3,
    pub entities: Vec<RawEntityIr>,
}

/// CHANGELOG: v0.1.0 - Added insert IR.
#[derive(Debug, Clone)]
pub struct InsertIr {
    pub layer: String,
    pub block_name: String,
    pub position: Vec3,
    pub rotation_deg: f32,
    pub scale: Vec3,
}

/// CHANGELOG: v0.1.0 - Added raw entity storage for debugging / fallback.
#[derive(Debug, Clone)]
pub struct RawEntityIr {
    pub entity_type: String,
    pub layer: String,
    pub group_codes: Vec<(i32, String)>,
}

/// CHANGELOG: v0.1.0 - Added parsed entity model used during low-level parsing.
#[derive(Debug, Clone)]
enum ParsedEntity {
    Line(LineIr),
    Polyline(PolylineIr),
    Circle(CircleIr),
    Arc(ArcIr),
    Text(TextIr),
    Dimension(DimensionIr),
    Insert(InsertIr),
    Unsupported(RawEntityIr),
}

/// CHANGELOG: v0.1.0 - Added importer entry point by file path.
pub fn import_dxf(path: impl AsRef<Path>, config: &DxfImportConfig) -> ImportResult<GeometryIr> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)?;
    parse_text_dxf(path.to_path_buf(), &content, config)
}

/// CHANGELOG: v0.1.0 - Added importer entry point by in-memory text.
pub fn parse_text_dxf(
    source_path: PathBuf,
    content: &str,
    config: &DxfImportConfig,
) -> ImportResult<GeometryIr> {
    let unit = detect_units(content).unwrap_or(config.assume_units);
    let mut ir = GeometryIr::new(source_path, SourceFormat::Dxf, unit);

    let sections = split_sections(content);
    let tables = sections
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("TABLES"));
    let entities = sections
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("ENTITIES"));
    let blocks = sections
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("BLOCKS"));

    if let Some(tables_section) = tables {
        ir.layers = parse_layers_from_tables(tables_section)?;
    }

    if let Some(blocks_section) = blocks {
        ir.blocks = parse_block_definitions(blocks_section)?;
    }

    if let Some(entity_section) = entities {
        parse_entities(entity_section, config, &mut ir)?;
    } else {
        return Err(ImportError::InvalidFormat(
            "DXF ENTITIES section not found".to_string(),
        ));
    }

    Ok(ir)
}

/// CHANGELOG: v0.1.0 - Added section model.
#[derive(Debug, Clone)]
struct DxfSection {
    name: String,
    pairs: Vec<(i32, String)>,
}

/// CHANGELOG: v0.1.0 - Split DXF text into named sections.
fn split_sections(content: &str) -> Vec<DxfSection> {
    let pairs = parse_group_code_pairs(content);
    let mut sections = Vec::new();
    let mut i = 0usize;

    while i + 1 < pairs.len() {
        if pairs[i].0 == 0 && pairs[i].1 == "SECTION" {
            if i + 3 < pairs.len() && pairs[i + 1].0 == 2 {
                let name = pairs[i + 1].1.clone();
                i += 2;

                let mut section_pairs = Vec::new();
                while i + 1 < pairs.len() {
                    if pairs[i].0 == 0 && pairs[i].1 == "ENDSEC" {
                        break;
                    }
                    section_pairs.push(pairs[i].clone());
                    i += 1;
                }

                sections.push(DxfSection {
                    name,
                    pairs: section_pairs,
                });
            }
        }
        i += 1;
    }

    sections
}

/// CHANGELOG: v0.1.0 - Parse raw group code pairs from DXF text.
fn parse_group_code_pairs(content: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = content.lines().collect();
    let mut out = Vec::new();
    let mut i = 0usize;

    while i + 1 < lines.len() {
        let code_str = lines[i].trim();
        let value_str = lines[i + 1].trim().to_string();

        if let Ok(code) = code_str.parse::<i32>() {
            out.push((code, value_str));
        }

        i += 2;
    }

    out
}

/// CHANGELOG: v0.1.0 - Parse LAYER table entries.
fn parse_layers_from_tables(section: &DxfSection) -> ImportResult<Vec<LayerIr>> {
    let mut layers = Vec::new();
    let mut i = 0usize;

    while i < section.pairs.len() {
        if section.pairs[i].0 == 0 && section.pairs[i].1 == "LAYER" {
            let mut layer_name = String::new();
            let mut color = None;
            let mut is_visible = true;
            i += 1;

            while i < section.pairs.len() {
                let (code, value) = &section.pairs[i];
                if *code == 0 {
                    break;
                }

                match *code {
                    2 => layer_name = value.clone(),
                    62 => {
                        if let Ok(c) = value.parse::<i16>() {
                            is_visible = c >= 0;
                            color = Some(CadColor(c.abs()));
                        }
                    }
                    _ => {}
                }

                i += 1;
            }

            if !layer_name.is_empty() {
                layers.push(LayerIr {
                    name: layer_name,
                    color,
                    is_visible,
                });
            }
        } else {
            i += 1;
        }
    }

    Ok(layers)
}

/// CHANGELOG: v0.1.0 - Parse block definition section.
/// NOTE: Skeleton only; entity extraction inside blocks is intentionally shallow for v0.1.
fn parse_block_definitions(section: &DxfSection) -> ImportResult<Vec<BlockDefinitionIr>> {
    let mut blocks = Vec::new();
    let mut i = 0usize;

    while i < section.pairs.len() {
        if section.pairs[i].0 == 0 && section.pairs[i].1 == "BLOCK" {
            let mut name = String::new();
            let mut base_point = [0.0, 0.0, 0.0];
            let mut raw_entities = Vec::new();
            i += 1;

            while i < section.pairs.len() {
                if section.pairs[i].0 == 0 && section.pairs[i].1 == "ENDBLK" {
                    break;
                }

                match section.pairs[i].0 {
                    2 => name = section.pairs[i].1.clone(),
                    10 => base_point[0] = section.pairs[i].1.parse::<f32>().unwrap_or(0.0),
                    20 => base_point[1] = section.pairs[i].1.parse::<f32>().unwrap_or(0.0),
                    30 => base_point[2] = section.pairs[i].1.parse::<f32>().unwrap_or(0.0),
                    0 => {
                        let start = i;
                        let entity_type = section.pairs[i].1.clone();
                        i += 1;
                        let mut codes = Vec::new();
                        let mut layer = String::new();

                        while i < section.pairs.len() {
                            if section.pairs[i].0 == 0 {
                                break;
                            }
                            if section.pairs[i].0 == 8 {
                                layer = section.pairs[i].1.clone();
                            }
                            codes.push(section.pairs[i].clone());
                            i += 1;
                        }

                        if start < i {
                            raw_entities.push(RawEntityIr {
                                entity_type,
                                layer,
                                group_codes: codes,
                            });
                            continue;
                        }
                    }
                    _ => {}
                }

                i += 1;
            }

            if !name.is_empty() {
                blocks.push(BlockDefinitionIr {
                    name,
                    base_point,
                    entities: raw_entities,
                });
            }
        } else {
            i += 1;
        }
    }

    Ok(blocks)
}

/// CHANGELOG: v0.1.0 - Parse entities section into normalized IR.
fn parse_entities(
    section: &DxfSection,
    config: &DxfImportConfig,
    ir: &mut GeometryIr,
) -> ImportResult<()> {
    let mut i = 0usize;

    while i < section.pairs.len() {
        if section.pairs[i].0 != 0 {
            i += 1;
            continue;
        }

        let entity_type = section.pairs[i].1.clone();
        i += 1;

        let mut payload = Vec::new();
        while i < section.pairs.len() {
            if section.pairs[i].0 == 0 {
                break;
            }
            payload.push(section.pairs[i].clone());
            i += 1;
        }

        let parsed = parse_single_entity(&entity_type, &payload)?;
        match parsed {
            ParsedEntity::Line(v) => ir.curves.push(CurveIr::Line(v)),
            ParsedEntity::Polyline(v) => ir.curves.push(CurveIr::Polyline(v)),
            ParsedEntity::Circle(v) => ir.curves.push(CurveIr::Circle(v)),
            ParsedEntity::Arc(v) => ir.curves.push(CurveIr::Arc(v)),
            ParsedEntity::Text(v) => ir.texts.push(v),
            ParsedEntity::Dimension(v) => ir.dimensions.push(v),
            ParsedEntity::Insert(v) => {
                if config.explode_inserts {
                    // CHANGELOG: v0.1.0 - Placeholder for future block explosion.
                    ir.inserts.push(v);
                } else {
                    ir.inserts.push(v);
                }
            }
            ParsedEntity::Unsupported(v) => {
                if config.preserve_raw_entities {
                    ir.raw_entities.push(v);
                }
            }
        }
    }

    Ok(())
}

/// CHANGELOG: v0.1.0 - Parse one low-level entity payload.
fn parse_single_entity(entity_type: &str, payload: &[(i32, String)]) -> ImportResult<ParsedEntity> {
    match entity_type {
        "LINE" => Ok(ParsedEntity::Line(parse_line(payload))),
        "LWPOLYLINE" => Ok(ParsedEntity::Polyline(parse_lwpolyline(payload))),
        "POLYLINE" => Ok(ParsedEntity::Unsupported(to_raw(entity_type, payload))),
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

/// CHANGELOG: v0.1.0 - Parse TEXT / MTEXT entity.
fn parse_text(payload: &[(i32, String)]) -> TextIr {
    let mut layer = String::from("0");
    let mut value_text = String::new();
    let mut position = [0.0, 0.0, 0.0];
    let mut height = 0.0;
    let mut rotation_deg = 0.0;

    for (code, value) in payload {
        match *code {
            8 => layer = value.clone(),
            1 | 3 => {
                if !value_text.is_empty() {
                    value_text.push(' ');
                }
                value_text.push_str(value);
            }
            10 => position[0] = value.parse::<f32>().unwrap_or(0.0),
            20 => position[1] = value.parse::<f32>().unwrap_or(0.0),
            30 => position[2] = value.parse::<f32>().unwrap_or(0.0),
            40 => height = value.parse::<f32>().unwrap_or(0.0),
            50 => rotation_deg = value.parse::<f32>().unwrap_or(0.0),
            _ => {}
        }
    }

    TextIr {
        layer,
        value: value_text,
        position,
        height,
        rotation_deg,
    }
}

/// CHANGELOG: v0.1.0 - Parse DIMENSION entity placeholder.
fn parse_dimension(payload: &[(i32, String)]) -> DimensionIr {
    let mut layer = String::from("0");
    let mut value_text = None;
    let mut definition_points = Vec::new();
    let mut current_x = None;

    for (code, value) in payload {
        match *code {
            8 => layer = value.clone(),
            1 => value_text = Some(value.clone()),
            10 | 13 | 14 => current_x = value.parse::<f32>().ok(),
            20 | 23 | 24 => {
                if let Some(x) = current_x.take() {
                    let y = value.parse::<f32>().unwrap_or(0.0);
                    definition_points.push([x, y, 0.0]);
                }
            }
            _ => {}
        }
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

/// CHANGELOG: v0.1.0 - Minimal unit detection placeholder.
/// NOTE: DXF INSUNITS handling is intentionally basic in v0.1.
fn detect_units(content: &str) -> Option<Unit> {
    let pairs = parse_group_code_pairs(content);

    for win in pairs.windows(2) {
        if win[0].0 == 9 && win[0].1 == "$INSUNITS" && win[1].0 == 70 {
            return match win[1].1.parse::<i32>().ok() {
                Some(1) => Some(Unit::Inch),
                Some(2) => Some(Unit::Foot),
                Some(4) => Some(Unit::Millimeter),
                Some(5) => Some(Unit::Centimeter),
                Some(6) => Some(Unit::Meter),
                _ => Some(Unit::Unknown),
            };
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_line_dxf() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
LINE
8
GRID
10
0.0
20
0.0
11
1000.0
21
0.0
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.curves.len(), 1);
        match &ir.curves[0] {
            CurveIr::Line(line) => {
                assert_eq!(line.layer, "GRID");
                assert_eq!(line.start, [0.0, 0.0, 0.0]);
                assert_eq!(line.end, [1000.0, 0.0, 0.0]);
            }
            _ => panic!("Expected line"),
        }
    }
}

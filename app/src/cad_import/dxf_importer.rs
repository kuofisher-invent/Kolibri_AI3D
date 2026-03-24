// version: v0.2.0
// changelog: v0.2.0 - Production-quality DXF importer with POLYLINE, SPLINE, ELLIPSE,
//   SOLID/3DFACE support, improved DIMENSION parsing (code 42), MTEXT continuation,
//   block INSERT explosion, entity counting, and debug reporting.
// changelog: v0.1.0 - Initial skeleton for DXF importer pipeline with IR conversion hooks.
// DEPENDENCY: std
// REQUIRED_METHODS: import_dxf, parse_text_dxf, parse_entities, to_import_report
// SIDE_EFFECTS: Reads DXF file from disk and returns parsed IR data only.

// (dead_code allowed at module level via mod.rs)

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

/// CHANGELOG: v0.2.0 - Parse entities section into normalized IR.
/// Handles old-style POLYLINE+VERTEX+SEQEND sequences, entity counting,
/// and block INSERT explosion.
fn parse_entities(
    section: &DxfSection,
    config: &DxfImportConfig,
    ir: &mut GeometryIr,
) -> ImportResult<()> {
    let mut i = 0usize;
    let mut entity_counts: HashMap<String, usize> = HashMap::new();

    while i < section.pairs.len() {
        if section.pairs[i].0 != 0 {
            i += 1;
            continue;
        }

        let entity_type = section.pairs[i].1.clone();
        *entity_counts.entry(entity_type.clone()).or_insert(0) += 1;
        i += 1;

        // Special handling for old-style POLYLINE (VERTEX/SEQEND sequence)
        if entity_type == "POLYLINE" {
            let mut payload = Vec::new();
            while i < section.pairs.len() {
                if section.pairs[i].0 == 0 {
                    break;
                }
                payload.push(section.pairs[i].clone());
                i += 1;
            }

            let mut poly_layer = String::from("0");
            let mut is_closed = false;
            let mut color = None;

            for (code, value) in &payload {
                match *code {
                    8 => poly_layer = value.clone(),
                    62 => color = value.parse::<i16>().ok().map(CadColor),
                    70 => is_closed = (value.parse::<i32>().unwrap_or(0) & 1) != 0,
                    _ => {}
                }
            }

            // Collect subsequent VERTEX entities until SEQEND
            let mut points = Vec::new();
            while i < section.pairs.len() {
                if section.pairs[i].0 == 0 {
                    let next_type = &section.pairs[i].1;
                    if next_type == "SEQEND" {
                        // Skip past SEQEND and its payload
                        i += 1;
                        while i < section.pairs.len() && section.pairs[i].0 != 0 {
                            i += 1;
                        }
                        break;
                    }
                    if next_type == "VERTEX" {
                        i += 1;
                        let mut vx = 0.0f32;
                        let mut vy = 0.0f32;
                        let mut vz = 0.0f32;
                        while i < section.pairs.len() && section.pairs[i].0 != 0 {
                            match section.pairs[i].0 {
                                10 => vx = section.pairs[i].1.parse().unwrap_or(0.0),
                                20 => vy = section.pairs[i].1.parse().unwrap_or(0.0),
                                30 => vz = section.pairs[i].1.parse().unwrap_or(0.0),
                                _ => {}
                            }
                            i += 1;
                        }
                        points.push([vx, vy, vz]);
                        continue;
                    }
                    // Unknown entity inside POLYLINE sequence — break out
                    break;
                }
                i += 1;
            }

            ir.curves.push(CurveIr::Polyline(PolylineIr {
                layer: poly_layer,
                points,
                is_closed,
                color,
            }));
            continue;
        }

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
                    // Explode: find block definition and add its entities as raw
                    if let Some(block) = ir.blocks.iter().find(|b| b.name == v.block_name) {
                        let block_entities = block.entities.clone();
                        for raw_entity in block_entities {
                            ir.raw_entities.push(raw_entity);
                        }
                    }
                }
                ir.inserts.push(v);
            }
            ParsedEntity::Unsupported(v) => {
                if config.preserve_raw_entities {
                    ir.raw_entities.push(v);
                }
            }
        }
    }

    // Store entity counts in metadata
    let mut counts_report = Vec::new();
    let mut sorted_counts: Vec<_> = entity_counts.iter().collect();
    sorted_counts.sort_by(|a, b| b.1.cmp(a.1));
    for (etype, count) in &sorted_counts {
        counts_report.push(format!("{}:{}", etype, count));
    }
    ir.metadata.insert("entity_counts".to_string(), counts_report.join(","));

    // Log entity counts
    eprintln!("[DXF] Entity counts:");
    for (etype, count) in &sorted_counts {
        eprintln!("[DXF]   {}: {}", etype, count);
    }
    eprintln!("[DXF] Total parsed: curves={}, texts={}, dims={}, inserts={}, raw={}",
        ir.curves.len(), ir.texts.len(), ir.dimensions.len(),
        ir.inserts.len(), ir.raw_entities.len());

    Ok(())
}

/// CHANGELOG: v0.2.0 - Parse one low-level entity payload.
/// Handles LINE, LWPOLYLINE, SPLINE, ELLIPSE, SOLID, 3DFACE, CIRCLE, ARC,
/// TEXT, MTEXT, DIMENSION, INSERT. Old-style POLYLINE is handled in parse_entities.
fn parse_single_entity(entity_type: &str, payload: &[(i32, String)]) -> ImportResult<ParsedEntity> {
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

    #[test]
    fn parse_old_style_polyline_with_vertices() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
POLYLINE
8
WALLS
70
1
0
VERTEX
10
0.0
20
0.0
30
0.0
0
VERTEX
10
100.0
20
0.0
30
0.0
0
VERTEX
10
100.0
20
50.0
30
0.0
0
SEQEND
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.curves.len(), 1);
        match &ir.curves[0] {
            CurveIr::Polyline(p) => {
                assert_eq!(p.layer, "WALLS");
                assert!(p.is_closed);
                assert_eq!(p.points.len(), 3);
                assert_eq!(p.points[0], [0.0, 0.0, 0.0]);
                assert_eq!(p.points[1], [100.0, 0.0, 0.0]);
                assert_eq!(p.points[2], [100.0, 50.0, 0.0]);
            }
            _ => panic!("Expected polyline"),
        }
    }

    #[test]
    fn parse_spline_as_polyline() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
SPLINE
8
CURVES
70
0
10
0.0
20
0.0
10
50.0
20
100.0
10
100.0
20
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
            CurveIr::Polyline(p) => {
                assert_eq!(p.layer, "CURVES");
                assert_eq!(p.points.len(), 3);
            }
            _ => panic!("Expected polyline from spline"),
        }
    }

    #[test]
    fn parse_ellipse_as_polyline() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
ELLIPSE
8
SHAPES
10
500.0
20
500.0
30
0.0
11
200.0
21
0.0
31
0.0
40
0.5
41
0.0
42
6.283185
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.curves.len(), 1);
        match &ir.curves[0] {
            CurveIr::Polyline(p) => {
                assert_eq!(p.layer, "SHAPES");
                assert!(p.is_closed);
                assert_eq!(p.points.len(), 33); // 32 segments + 1
            }
            _ => panic!("Expected polyline from ellipse"),
        }
    }

    #[test]
    fn parse_solid_as_closed_polyline() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
SOLID
8
FILL
10
0.0
20
0.0
11
100.0
21
0.0
12
0.0
22
100.0
13
100.0
23
100.0
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.curves.len(), 1);
        match &ir.curves[0] {
            CurveIr::Polyline(p) => {
                assert_eq!(p.layer, "FILL");
                assert!(p.is_closed);
                assert_eq!(p.points.len(), 4);
            }
            _ => panic!("Expected closed polyline from SOLID"),
        }
    }

    #[test]
    fn parse_dimension_with_measured_value() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
DIMENSION
8
DIM
13
0.0
23
0.0
14
5000.0
24
0.0
42
5000.0
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.dimensions.len(), 1);
        assert_eq!(ir.dimensions[0].value_text, Some("5000".to_string()));
        assert!(ir.dimensions[0].definition_points.len() >= 2);
    }

    #[test]
    fn parse_mtext_continuation() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
MTEXT
8
NOTES
10
100.0
20
200.0
40
10.0
3
Hello
3
World
1
Final
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        assert_eq!(ir.texts.len(), 1);
        assert_eq!(ir.texts[0].value, "HelloWorldFinal");
        assert_eq!(ir.texts[0].layer, "NOTES");
    }

    #[test]
    fn parse_multi_entity_dxf() {
        let dxf = "\
0
SECTION
2
TABLES
0
TABLE
2
LAYER
0
LAYER
2
WALLS
62
1
0
LAYER
2
GRID
62
3
0
ENDTAB
0
ENDSEC
0
SECTION
2
ENTITIES
0
LINE
8
WALLS
10
0.0
20
0.0
30
0.0
11
5000.0
21
0.0
31
0.0
0
LINE
8
WALLS
10
5000.0
20
0.0
11
5000.0
21
3000.0
0
ARC
8
GRID
10
2500.0
20
1500.0
40
500.0
50
0.0
51
180.0
0
CIRCLE
8
GRID
10
2500.0
20
1500.0
40
300.0
0
TEXT
8
GRID
10
100.0
20
100.0
40
200.0
1
A
0
DIMENSION
8
DIM
13
0.0
23
0.0
14
5000.0
24
0.0
1
5000
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        // 2 lines + 1 arc + 1 circle = 4 curves
        assert_eq!(ir.curves.len(), 4);
        assert_eq!(ir.texts.len(), 1);
        assert_eq!(ir.dimensions.len(), 1);
        assert_eq!(ir.layers.len(), 2);

        // Entity counts should be in metadata
        assert!(ir.metadata.contains_key("entity_counts"));
    }

    #[test]
    fn entity_counts_in_metadata() {
        let dxf = "\
0
SECTION
2
ENTITIES
0
LINE
8
L
10
0.0
20
0.0
11
1.0
21
1.0
0
LINE
8
L
10
1.0
20
1.0
11
2.0
21
2.0
0
CIRCLE
8
L
10
0.0
20
0.0
40
5.0
0
ENDSEC
0
EOF
";
        let ir = parse_text_dxf(PathBuf::from("test.dxf"), dxf, &DxfImportConfig::default())
            .expect("DXF should parse");

        let counts = ir.metadata.get("entity_counts").expect("should have counts");
        assert!(counts.contains("LINE:2"));
        assert!(counts.contains("CIRCLE:1"));
    }
}

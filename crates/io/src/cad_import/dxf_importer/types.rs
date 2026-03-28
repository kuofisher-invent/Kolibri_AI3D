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
use std::path::PathBuf;

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

/// DXF section (HEADER, TABLES, ENTITIES, BLOCKS...)
#[derive(Debug, Clone)]
pub(super) struct DxfSection {
    pub name: String,
    pub pairs: Vec<(i32, String)>,
}

#[derive(Debug, Clone)]
pub(super) enum ParsedEntity {
    Line(LineIr),
    Polyline(PolylineIr),
    Circle(CircleIr),
    Arc(ArcIr),
    Text(TextIr),
    Dimension(DimensionIr),
    Insert(InsertIr),
    Unsupported(RawEntityIr),
}

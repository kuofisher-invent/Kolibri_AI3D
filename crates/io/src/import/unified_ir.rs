//! Unified Intermediate Representation for all import formats
//! Both DWG and SKP convert to this format before building the scene

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnifiedIR {
    pub source_format: String,      // "dwg", "skp", "obj"
    pub source_file: String,
    pub units: String,              // "mm"

    // Geometry
    pub meshes: Vec<IrMesh>,
    pub curves: Vec<IrCurve>,

    // Scene graph
    pub instances: Vec<IrInstance>,
    pub groups: Vec<IrGroup>,
    pub component_defs: Vec<IrComponentDef>,

    // Materials
    pub materials: Vec<IrMaterial>,

    // Semantic (from parsers)
    pub grids: Option<crate::cad_import::ir::GridSystem>,
    pub members: Vec<IrMember>,
    pub levels: Vec<crate::cad_import::ir::LevelDef>,

    // Metadata
    pub stats: ImportStats,
    /// Structured debug report lines for Console display
    pub debug_report: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrMesh {
    pub id: String,
    pub name: String,
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,           // triangle indices
    pub material_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrCurve {
    pub id: String,
    pub points: Vec<[f64; 2]>,
    pub layer: String,
    pub is_closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrInstance {
    pub id: String,
    pub mesh_id: String,            // references IrMesh.id
    pub component_def_id: Option<String>,
    pub transform: [f32; 16],       // 4x4 matrix, column-major
    pub name: String,
    pub layer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrGroup {
    pub id: String,
    pub name: String,
    pub children: Vec<String>,      // instance IDs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrComponentDef {
    pub id: String,
    pub name: String,
    pub mesh_ids: Vec<String>,
    pub instance_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrMaterial {
    pub id: String,
    pub name: String,
    pub color: [f32; 4],            // RGBA
    pub texture_path: Option<String>,
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemberType {
    Beam, Column, Plate, Brace, Foundation, Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrMember {
    pub id: String,
    pub member_type: MemberType,
    pub start: [f64; 3],
    pub end: [f64; 3],
    pub profile: Option<String>,
    pub material: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImportStats {
    pub mesh_count: usize,
    pub face_count: usize,
    pub vertex_count: usize,
    pub instance_count: usize,
    pub group_count: usize,
    pub component_count: usize,
    pub material_count: usize,
    pub member_count: usize,
}

//! CAD Import Intermediate Representation
//! Clean structured data between raw DXF geometry and 3D modeling

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawingIR {
    pub units: String,
    pub drawing_type: DrawingType,
    pub pages: Vec<PageInfo>,
    pub grids: GridSystem,
    pub columns: Vec<ColumnDef>,
    pub beams: Vec<BeamDef>,
    pub levels: Vec<LevelDef>,
    pub base_plates: Vec<BasePlateDef>,
    pub debug_report: Vec<String>,
}

impl Default for DrawingIR {
    fn default() -> Self {
        Self {
            units: "mm".into(),
            drawing_type: DrawingType::Unknown,
            pages: Vec::new(),
            grids: GridSystem::default(),
            columns: Vec::new(),
            beams: Vec::new(),
            levels: Vec::new(),
            base_plates: Vec::new(),
            debug_report: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DrawingType {
    ColumnLayoutPlan,
    SteelElevation,
    FloorPlan,
    SteelDetail,
    SectionView,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    pub page_number: usize,
    pub drawing_type: DrawingType,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GridSystem {
    pub x_grids: Vec<GridLine>,
    pub y_grids: Vec<GridLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridLine {
    pub name: String,
    pub position: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnDef {
    pub id: String,
    pub grid_x: String,
    pub grid_y: String,
    pub position: [f64; 2],
    pub base_level: f64,
    pub top_level: f64,
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeamDef {
    pub id: String,
    pub from_grid: String,
    pub to_grid: String,
    pub elevation: f64,
    pub start_pos: [f64; 2],
    pub end_pos: [f64; 2],
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelDef {
    pub name: String,
    pub elevation: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasePlateDef {
    pub id: String,
    pub position: [f64; 2],
    pub width: f64,
    pub depth: f64,
    pub height: f64,
}

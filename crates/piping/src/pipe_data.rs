//! 管線資料型別

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 管線系統分類
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PipeSystem {
    /// PVC 給水管
    PvcWater,
    /// PVC 排水管
    PvcDrain,
    /// EMT 電線導管
    ElectricalConduit,
    /// 鍍鋅鐵管（消防灑水）
    IronFireSprinkler,
    /// 碳鋼管（製程）
    SteelProcess,
    /// 不鏽鋼管
    StainlessSteel,
    /// 銅管（冷媒/瓦斯）
    Copper,
}

impl PipeSystem {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PvcWater => "PVC 給水",
            Self::PvcDrain => "PVC 排水",
            Self::ElectricalConduit => "EMT 電管",
            Self::IronFireSprinkler => "鐵管 消防",
            Self::SteelProcess => "碳鋼管",
            Self::StainlessSteel => "不鏽鋼管",
            Self::Copper => "銅管",
        }
    }

    pub fn color(&self) -> [f32; 3] {
        match self {
            Self::PvcWater => [0.3, 0.5, 0.9],       // 藍
            Self::PvcDrain => [0.5, 0.5, 0.5],       // 灰
            Self::ElectricalConduit => [0.9, 0.6, 0.2], // 橙
            Self::IronFireSprinkler => [0.9, 0.2, 0.2], // 紅
            Self::SteelProcess => [0.6, 0.6, 0.6],   // 銀灰
            Self::StainlessSteel => [0.75, 0.75, 0.78], // 亮銀
            Self::Copper => [0.8, 0.5, 0.2],         // 銅色
        }
    }

    pub fn all() -> &'static [PipeSystem] {
        &[
            Self::PvcWater, Self::PvcDrain, Self::ElectricalConduit,
            Self::IronFireSprinkler, Self::SteelProcess,
            Self::StainlessSteel, Self::Copper,
        ]
    }
}

/// 管件種類
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FittingKind {
    Elbow90,
    Elbow45,
    Tee,
    Cross,
    Reducer,
    Cap,
    Valve,
    Coupling,
    Flange,
}

impl FittingKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Elbow90 => "90° 彎頭",
            Self::Elbow45 => "45° 彎頭",
            Self::Tee => "三通",
            Self::Cross => "四通",
            Self::Reducer => "大小頭",
            Self::Cap => "管帽",
            Self::Valve => "閥門",
            Self::Coupling => "接頭",
            Self::Flange => "法蘭",
        }
    }
}

/// 管材規格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipeSpec {
    /// 管線系統
    pub system: PipeSystem,
    /// 公稱口徑 (mm)
    pub nominal_dn: f32,
    /// 外徑 (mm)
    pub outer_diameter: f32,
    /// 壁厚 (mm)
    pub wall_thickness: f32,
    /// 規格名稱（如 "DN50 SCH40"）
    pub spec_name: String,
}

impl PipeSpec {
    pub fn inner_diameter(&self) -> f32 {
        (self.outer_diameter - 2.0 * self.wall_thickness).max(1.0)
    }
}

/// 一段管線
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipeSegment {
    pub id: String,
    pub spec: PipeSpec,
    pub start: [f32; 3],
    pub end: [f32; 3],
    /// 對應的 SceneObject ID
    pub scene_object_id: String,
}

impl PipeSegment {
    pub fn length(&self) -> f32 {
        let dx = self.end[0] - self.start[0];
        let dy = self.end[1] - self.start[1];
        let dz = self.end[2] - self.start[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// 管件（彎頭、三通等）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipeFitting {
    pub id: String,
    pub kind: FittingKind,
    pub spec: PipeSpec,
    pub position: [f32; 3],
    pub rotation_y: f32,
    pub scene_object_id: String,
}

/// 管路（一組連續的管段+管件）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipeRun {
    pub id: String,
    pub name: String,
    pub system: PipeSystem,
    pub segments: Vec<String>,
    pub fittings: Vec<String>,
}

/// 管線模組的資料儲存
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PipingStore {
    pub segments: HashMap<String, PipeSegment>,
    pub fittings: HashMap<String, PipeFitting>,
    pub runs: Vec<PipeRun>,
    next_id: u32,
}

impl PipingStore {
    pub fn next_id(&mut self) -> String {
        self.next_id += 1;
        format!("pipe_{}", self.next_id)
    }

    /// 計算管路總長 (mm)
    pub fn total_length(&self, system: Option<PipeSystem>) -> f32 {
        self.segments.values()
            .filter(|s| system.map_or(true, |sys| s.spec.system == sys))
            .map(|s| s.length())
            .sum()
    }
}

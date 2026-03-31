//! 圖層管理

use serde::{Serialize, Deserialize};
use crate::entities::LineType;

/// 圖層
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftLayer {
    pub name: String,
    pub color: [u8; 3],
    pub line_type: LineType,
    pub line_weight: f64,
    pub visible: bool,
    pub locked: bool,
    pub frozen: bool,
}

impl DraftLayer {
    pub fn new(name: &str, color: [u8; 3]) -> Self {
        Self {
            name: name.into(),
            color,
            line_type: LineType::Continuous,
            line_weight: 0.25,
            visible: true,
            locked: false,
            frozen: false,
        }
    }
}

/// 圖層管理器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerManager {
    pub layers: Vec<DraftLayer>,
    pub current: String,
}

impl Default for LayerManager {
    fn default() -> Self {
        Self {
            layers: vec![
                DraftLayer::new("0", [0, 0, 0]),
                DraftLayer::new("標註", [255, 0, 0]),
                DraftLayer::new("中心線", [0, 128, 0]),
                DraftLayer::new("隱藏線", [128, 128, 128]),
                DraftLayer::new("文字", [0, 0, 255]),
            ],
            current: "0".into(),
        }
    }
}

impl LayerManager {
    pub fn get(&self, name: &str) -> Option<&DraftLayer> {
        self.layers.iter().find(|l| l.name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut DraftLayer> {
        self.layers.iter_mut().find(|l| l.name == name)
    }

    pub fn add(&mut self, layer: DraftLayer) {
        if !self.layers.iter().any(|l| l.name == layer.name) {
            self.layers.push(layer);
        }
    }

    pub fn current_layer(&self) -> Option<&DraftLayer> {
        self.get(&self.current)
    }
}

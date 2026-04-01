//! 2D 出圖實體 — DraftEntity、DraftDocument

use serde::{Serialize, Deserialize};

/// 唯一 ID
pub type DraftId = u64;

/// 2D 點（mm 座標）
pub type Point2 = [f64; 2];

/// 線型
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LineType {
    Continuous,
    Dashed,
    DashDot,
    Center,
    Hidden,
    Phantom,
}

impl LineType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Continuous => "實線",
            Self::Dashed => "虛線",
            Self::DashDot => "點虛線",
            Self::Center => "中心線",
            Self::Hidden => "隱藏線",
            Self::Phantom => "假想線",
        }
    }

    /// 回傳 dash pattern（mm 單位）
    pub fn pattern(&self) -> &'static [f64] {
        match self {
            Self::Continuous => &[],
            Self::Dashed => &[6.0, 3.0],
            Self::DashDot => &[12.0, 3.0, 2.0, 3.0],
            Self::Center => &[18.0, 3.0, 6.0, 3.0],
            Self::Hidden => &[4.0, 2.0],
            Self::Phantom => &[18.0, 3.0, 2.0, 3.0, 2.0, 3.0],
        }
    }
}

/// 2D 繪圖實體
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DraftEntity {
    /// 直線
    Line {
        start: Point2,
        end: Point2,
    },
    /// 圓弧
    Arc {
        center: Point2,
        radius: f64,
        start_angle: f64,  // radians
        end_angle: f64,
    },
    /// 完整圓
    Circle {
        center: Point2,
        radius: f64,
    },
    /// 矩形（對角兩點）
    Rectangle {
        p1: Point2,
        p2: Point2,
    },
    /// 多段線
    Polyline {
        points: Vec<Point2>,
        closed: bool,
    },
    /// 橢圓
    Ellipse {
        center: Point2,
        semi_major: f64,
        semi_minor: f64,
        rotation: f64,
    },
    /// 線性標註
    DimLinear {
        p1: Point2,
        p2: Point2,
        offset: f64,
        text_override: Option<String>,
    },
    /// 對齊標註
    DimAligned {
        p1: Point2,
        p2: Point2,
        offset: f64,
        text_override: Option<String>,
    },
    /// 角度標註
    DimAngle {
        center: Point2,
        p1: Point2,
        p2: Point2,
        radius: f64,
    },
    /// 半徑標註
    DimRadius {
        center: Point2,
        radius: f64,
        angle: f64,
    },
    /// 直徑標註
    DimDiameter {
        center: Point2,
        radius: f64,
        angle: f64,
    },
    /// 文字
    Text {
        position: Point2,
        content: String,
        height: f64,   // mm
        rotation: f64,
    },
    /// 引線
    Leader {
        points: Vec<Point2>,
        text: String,
    },
    /// 填充
    Hatch {
        boundary: Vec<Point2>,
        pattern: HatchPattern,
        scale: f64,
        angle: f64,
    },
    /// 正多邊形
    Polygon {
        center: Point2,
        radius: f64,
        sides: u32,
        inscribed: bool, // true=內接, false=外接
    },
    /// 雲形線（Spline，以控制點逼近）
    Spline {
        points: Vec<Point2>,
        closed: bool,
    },
    /// 點
    Point {
        position: Point2,
    },
    /// 建構線（無限長）
    Xline {
        base: Point2,
        direction: Point2,  // 方向向量
    },
    /// 圖塊參考
    BlockRef {
        name: String,
        insert_point: Point2,
        scale: [f64; 2],
        rotation: f64,
    },
    /// 修訂雲形（Revcloud — 波浪邊界）
    Revcloud {
        points: Vec<Point2>,
        arc_radius: f64,
    },
    /// 表格
    Table {
        position: Point2,
        rows: u32,
        cols: u32,
        row_height: f64,
        col_width: f64,
        cells: Vec<String>,  // row-major, len = rows * cols
    },
}

/// 填充花樣
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum HatchPattern {
    Solid,
    Lines,
    Cross,
    Dots,
    Brick,
    Concrete,
}

impl HatchPattern {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Solid => "實心",
            Self::Lines => "平行線",
            Self::Cross => "交叉線",
            Self::Dots => "點",
            Self::Brick => "磚",
            Self::Concrete => "混凝土",
        }
    }
}

/// 單一圖元（包含實體 + 屬性）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftObject {
    pub id: DraftId,
    pub entity: DraftEntity,
    pub layer: String,
    pub color: [u8; 3],
    pub line_type: LineType,
    pub line_weight: f64,  // mm
    pub visible: bool,
}

impl DraftObject {
    pub fn new(id: DraftId, entity: DraftEntity) -> Self {
        Self {
            id,
            entity,
            layer: "0".into(),
            color: [0, 0, 0],
            line_type: LineType::Continuous,
            line_weight: 0.25,
            visible: true,
        }
    }
}

/// 2D 出圖文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftDocument {
    pub objects: Vec<DraftObject>,
    next_id: DraftId,
}

impl Default for DraftDocument {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            next_id: 1,
        }
    }
}

impl DraftDocument {
    pub fn new() -> Self {
        Self::default()
    }

    /// 新增實體，回傳 ID
    pub fn add(&mut self, entity: DraftEntity) -> DraftId {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.push(DraftObject::new(id, entity));
        id
    }

    /// 新增實體並指定顏色
    pub fn add_with_color(&mut self, entity: DraftEntity, color: [u8; 3]) -> DraftId {
        let id = self.next_id;
        self.next_id += 1;
        let mut obj = DraftObject::new(id, entity);
        obj.color = color;
        self.objects.push(obj);
        id
    }

    /// 刪除實體
    pub fn remove(&mut self, id: DraftId) -> bool {
        if let Some(pos) = self.objects.iter().position(|o| o.id == id) {
            self.objects.remove(pos);
            true
        } else {
            false
        }
    }

    /// 取得實體（可變）
    pub fn get_mut(&mut self, id: DraftId) -> Option<&mut DraftObject> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    /// 計算線性距離
    pub fn distance(p1: &Point2, p2: &Point2) -> f64 {
        ((p2[0] - p1[0]).powi(2) + (p2[1] - p1[1]).powi(2)).sqrt()
    }

    /// 計算角度（radians）
    pub fn angle(p1: &Point2, p2: &Point2) -> f64 {
        (p2[1] - p1[1]).atan2(p2[0] - p1[0])
    }
}

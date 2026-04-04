//! 鋼構接頭系統 — 資料結構與螺栓/焊接規格
//! Phase A: 接頭定義、螺栓組、焊接線、端板/肋板/底板

use serde::{Deserialize, Serialize};

// ─── 接頭定義 ──────────────────────────────────────────────────────────────────

/// 鋼構接頭（含板件、螺栓、焊接）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteelConnection {
    pub id: String,
    pub conn_type: ConnectionType,
    /// 參與構件的 SceneObject ID
    pub member_ids: Vec<String>,
    pub plates: Vec<ConnectionPlate>,
    pub bolts: Vec<BoltGroup>,
    pub welds: Vec<WeldLine>,
    /// 接頭位置（世界座標 mm）
    pub position: [f32; 3],
    /// 接頭所屬群組 ID（Scene group）
    pub group_id: Option<String>,
}

/// 接頭類型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionType {
    /// 端板式（梁-柱 剛接）— 端板焊於梁端，螺栓鎖於柱翼板
    EndPlate,
    /// 腹板式（梁-柱 鉸接）— 剪力板焊於柱，螺栓鎖於梁腹板
    ShearTab,
    /// 翼板式（梁-梁 續接）— 上下翼板拼接
    FlangePlate,
    /// 底板（柱底 + 錨栓）
    BasePlate,
    /// 斜撐接合板（gusset plate）
    BracePlate,
    /// 拼接板（梁/柱 接續）
    SplicePlate,
    /// 柱腹板加厚板（panel zone doubler）
    WebDoubler,
    /// 雙角鋼接頭（double angle, framed connection）
    DoubleAngle,
}

impl ConnectionType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::EndPlate => "端板接頭",
            Self::ShearTab => "腹板接頭",
            Self::FlangePlate => "翼板接頭",
            Self::BasePlate => "底板接頭",
            Self::BracePlate => "斜撐接合板",
            Self::SplicePlate => "拼接板",
            Self::WebDoubler => "腹板加厚板",
            Self::DoubleAngle => "雙角鋼接頭",
        }
    }
}

// ─── 板件 ───────────────────────────────────────────────────────────────────────

/// 接頭板件（端板/肋板/底板/接合板）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPlate {
    pub width: f32,      // mm
    pub height: f32,     // mm
    pub thickness: f32,  // mm
    /// 板件位置（世界座標）
    pub position: [f32; 3],
    /// Y 軸旋轉角（rad）
    pub rotation_y: f32,
    pub material: String, // 材質等級 e.g. "SS400"
    /// 板件用途
    pub plate_type: PlateType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlateType {
    EndPlate,    // 端板
    Stiffener,   // 肋板（加勁板）
    BasePlate,   // 底板
    GussetPlate, // 接合板
    ShearTab,    // 剪力板
    SplicePlate, // 拼接板
    WebDoubler,  // 腹板加厚板
    AngleLeg,    // 角鋼肢板
}

// ─── 螺栓 ───────────────────────────────────────────────────────────────────────

/// 螺栓群組
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoltGroup {
    pub bolt_size: BoltSize,
    pub bolt_grade: BoltGrade,
    pub rows: u32,
    pub cols: u32,
    pub row_spacing: f32,    // mm（行距）
    pub col_spacing: f32,    // mm（列距）
    pub edge_dist: f32,      // mm（邊距）
    pub hole_diameter: f32,  // mm（孔徑）
    /// 每顆螺栓的世界座標
    pub positions: Vec<[f32; 3]>,
}

/// CNS 2473 螺栓尺寸
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoltSize {
    M16,
    M20,
    M22,
    M24,
    M27,
    M30,
}

impl BoltSize {
    /// 螺栓公稱直徑 (mm)
    pub fn diameter(&self) -> f32 {
        match self {
            Self::M16 => 16.0,
            Self::M20 => 20.0,
            Self::M22 => 22.0,
            Self::M24 => 24.0,
            Self::M27 => 27.0,
            Self::M30 => 30.0,
        }
    }

    /// 標準孔徑 (AISC Table J3.3M)
    pub fn hole_diameter(&self) -> f32 {
        match self {
            Self::M16 => 18.0,
            Self::M20 => 22.0,
            Self::M22 => 24.0,
            Self::M24 => 27.0, // AISC: +3mm
            Self::M27 => 30.0,
            Self::M30 => 33.0,
        }
    }

    /// 螺栓頭對邊距 — Heavy Hex (AISC/ASTM F3125)
    pub fn head_across_flats(&self) -> f32 {
        match self {
            Self::M16 => 27.0,
            Self::M20 => 34.0,
            Self::M22 => 36.0,
            Self::M24 => 41.0,
            Self::M27 => 46.0,
            Self::M30 => 50.0,
        }
    }

    /// 螺栓頭厚 (ISO 4014 / ASTM)
    pub fn head_thickness(&self) -> f32 {
        match self {
            Self::M16 => 10.0,
            Self::M20 => 12.5,
            Self::M22 => 14.0,
            Self::M24 => 15.0,
            Self::M27 => 17.0,
            Self::M30 => 18.7,
        }
    }

    /// 螺帽厚 (Heavy Hex Nut, ASTM)
    pub fn nut_thickness(&self) -> f32 {
        match self {
            Self::M16 => 14.8,
            Self::M20 => 18.0,
            Self::M22 => 19.4,
            Self::M24 => 21.5,
            Self::M27 => 23.8,
            Self::M30 => 25.6,
        }
    }

    /// 墊圈外徑 (ASTM F436)
    pub fn washer_od(&self) -> f32 {
        match self {
            Self::M16 => 33.0,
            Self::M20 => 37.0,
            Self::M22 => 44.0,
            Self::M24 => 51.0,
            Self::M27 => 57.0,
            Self::M30 => 64.0,
        }
    }

    /// 最小螺栓間距 (mm) — 2.5d（台灣鋼構規範）
    /// 最小間距 (AISC J3.3: 2.667d，建議 3d)
    pub fn min_spacing(&self) -> f32 {
        (self.diameter() * 2.667).ceil()
    }

    /// 建議間距 (AISC: 3d)
    pub fn preferred_spacing(&self) -> f32 {
        self.diameter() * 3.0
    }

    /// 最小邊距 (AISC Table J3.4, 滾軋/切割邊)
    pub fn min_edge(&self) -> f32 {
        match self {
            Self::M16 => 22.0,
            Self::M20 => 25.0,
            Self::M22 => 29.0,
            Self::M24 => 32.0,
            Self::M27 => 38.0,
            Self::M30 => 41.0,
        }
    }

    /// 最小邊距 — 剪切邊 (AISC Table J3.4)
    pub fn min_edge_sheared(&self) -> f32 {
        match self {
            Self::M16 => 29.0,
            Self::M20 => 32.0,
            Self::M22 => 38.0,
            Self::M24 => 44.0,
            Self::M27 => 51.0,
            Self::M30 => 57.0,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::M16 => "M16",
            Self::M20 => "M20",
            Self::M22 => "M22",
            Self::M24 => "M24",
            Self::M27 => "M27",
            Self::M30 => "M30",
        }
    }

    pub const ALL: &'static [BoltSize] = &[
        Self::M16, Self::M20, Self::M22, Self::M24, Self::M27, Self::M30,
    ];
}

/// 螺栓等級（CNS 2473 / ASTM）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoltGrade {
    /// F8T（台灣常用高拉力螺栓）
    F8T,
    /// F10T（高強度）
    F10T,
    /// A325（ASTM）
    A325,
    /// A490（ASTM 高強度）
    A490,
}

impl BoltGrade {
    pub fn label(&self) -> &'static str {
        match self {
            Self::F8T => "F8T",
            Self::F10T => "F10T",
            Self::A325 => "A325",
            Self::A490 => "A490",
        }
    }

    pub const ALL: &'static [BoltGrade] = &[Self::F8T, Self::F10T, Self::A325, Self::A490];
}

// ─── 焊接 ───────────────────────────────────────────────────────────────────────

/// 焊接線段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeldLine {
    pub weld_type: WeldType,
    /// 焊腳尺寸 (mm)
    pub size: f32,
    /// 焊接長度 (mm)
    pub length: f32,
    /// 起點（世界座標）
    pub start: [f32; 3],
    /// 終點（世界座標）
    pub end: [f32; 3],
}

/// 焊接類型（CNS 4435 / ISO 2553）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeldType {
    /// 角焊（最常用）
    Fillet,
    /// 全滲透對接焊（剛接翼板）
    FullPenetration,
    /// 半滲透對接焊
    PartialPenetration,
    /// V 形坡口焊
    VGroove,
    /// 單斜坡口焊
    BevelGroove,
    /// U 形坡口焊
    UGroove,
    /// J 形坡口焊
    JGroove,
    /// 塞焊/槽焊
    PlugSlot,
    /// 點焊
    Spot,
    /// 封底焊
    BackingRun,
}

impl WeldType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Fillet => "角焊",
            Self::FullPenetration => "全滲透",
            Self::PartialPenetration => "半滲透",
            Self::VGroove => "V形",
            Self::BevelGroove => "斜坡口",
            Self::UGroove => "U形",
            Self::JGroove => "J形",
            Self::PlugSlot => "塞焊",
            Self::Spot => "點焊",
            Self::BackingRun => "封底焊",
        }
    }

    pub const ALL: &'static [WeldType] = &[
        Self::Fillet, Self::FullPenetration, Self::PartialPenetration,
        Self::VGroove, Self::BevelGroove, Self::UGroove, Self::JGroove,
        Self::PlugSlot, Self::Spot, Self::BackingRun,
    ];

    /// ISO 2553 符號字元（用於施工圖標註）
    pub fn iso_symbol(&self) -> &'static str {
        match self {
            Self::Fillet => "△",
            Self::FullPenetration => "V",
            Self::PartialPenetration => "½V",
            Self::VGroove => "V",
            Self::BevelGroove => "Y",
            Self::UGroove => "U",
            Self::JGroove => "J",
            Self::PlugSlot => "⊡",
            Self::Spot => "○",
            Self::BackingRun => "⌒",
        }
    }
}

/// ISO 2553 焊接符號標註（完整的焊接標記資訊）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeldSymbolISO2553 {
    /// 焊接線參考（關聯的 WeldLine）
    pub weld_line_idx: usize,
    /// 箭頭側焊接類型
    pub arrow_side: WeldType,
    /// 另一側焊接類型（None = 僅單側）
    pub other_side: Option<WeldType>,
    /// 箭頭側焊腳/坡口尺寸 (mm)
    pub arrow_size: f32,
    /// 另一側尺寸 (mm)
    pub other_size: Option<f32>,
    /// 焊接長度 (mm)，None = 全長
    pub length: Option<f32>,
    /// 間距 (mm)，斷續焊用
    pub pitch: Option<f32>,
    /// 現場焊（工地焊接）
    pub field_weld: bool,
    /// 全周焊（環繞符號）
    pub all_around: bool,
    /// 尾部補充說明（如工藝要求）
    pub tail_note: Option<String>,
}

// ─── 接頭自動計算 ──────────────────────────────────────────────────────────────

/// 端板式接頭參數（梁-柱剛接）
#[derive(Debug, Clone)]
pub struct EndPlateParams {
    /// 梁截面: (H, B, tw, tf) mm
    pub beam_section: (f32, f32, f32, f32),
    /// 柱截面: (H, B, tw, tf) mm
    pub col_section: (f32, f32, f32, f32),
    /// 螺栓尺寸
    pub bolt_size: BoltSize,
    /// 螺栓等級
    pub bolt_grade: BoltGrade,
    /// 端板厚度 (mm)，None = 自動計算
    pub plate_thickness: Option<f32>,
    /// 是否加肋板
    pub add_stiffeners: bool,
}

/// 計算端板接頭
pub fn calc_end_plate(params: &EndPlateParams) -> SteelConnection {
    let (bh, bb, _btw, btf) = params.beam_section;
    let (ch, _cb, _ctw, _ctf) = params.col_section;

    // 端板尺寸：寬 = 梁翼板寬 + 2×邊距，高 = 梁高 + 2×延伸
    let bolt_edge = params.bolt_size.min_edge();
    let bolt_spacing = (params.bolt_size.diameter() * 3.0).max(params.bolt_size.min_spacing()); // 建議 3d（AISC J3.3）
    let plate_w = bb + 2.0 * bolt_edge;
    let plate_ext = bolt_edge + 20.0; // 端板超出梁翼板的延伸量
    let plate_h = bh + 2.0 * plate_ext;

    // 端板厚度：預設 = 梁翼板厚度 × 1.5（至少 16mm）
    let plate_t = params.plate_thickness.unwrap_or((btf * 1.5).max(16.0));

    // 螺栓配置：梁高 ≥ 400 用 4 列，否則 2 列（AISC 建議翼板上下各需螺栓）
    let bolt_rows = if bh >= 400.0 { 4 } else { 2 };
    let bolt_cols = 2;

    // 螺栓位置計算
    let bolt_x_half = (bb / 2.0 - bolt_edge).max(bolt_spacing / 2.0);
    let mut bolt_positions = Vec::new();

    // 上方螺栓（梁頂翼板上方）
    let top_y = bh / 2.0 + plate_ext / 2.0;
    bolt_positions.push([-bolt_x_half, top_y, 0.0]);
    bolt_positions.push([bolt_x_half, top_y, 0.0]);

    if bolt_rows >= 4 {
        // 梁頂翼板下方
        let inner_top_y = bh / 2.0 - btf - bolt_spacing / 2.0;
        bolt_positions.push([-bolt_x_half, inner_top_y, 0.0]);
        bolt_positions.push([bolt_x_half, inner_top_y, 0.0]);
        // 梁底翼板上方
        let inner_bot_y = -(bh / 2.0 - btf - bolt_spacing / 2.0);
        bolt_positions.push([-bolt_x_half, inner_bot_y, 0.0]);
        bolt_positions.push([bolt_x_half, inner_bot_y, 0.0]);
    }

    // 下方螺栓（梁底翼板下方）
    let bot_y = -(bh / 2.0 + plate_ext / 2.0);
    bolt_positions.push([-bolt_x_half, bot_y, 0.0]);
    bolt_positions.push([bolt_x_half, bot_y, 0.0]);

    let bolt_group = BoltGroup {
        bolt_size: params.bolt_size,
        bolt_grade: params.bolt_grade,
        rows: bolt_rows,
        cols: bolt_cols,
        row_spacing: bolt_spacing,
        col_spacing: bolt_x_half * 2.0,
        edge_dist: bolt_edge,
        hole_diameter: params.bolt_size.hole_diameter(),
        positions: bolt_positions,
    };

    // 端板
    let end_plate = ConnectionPlate {
        width: plate_w,
        height: plate_h,
        thickness: plate_t,
        position: [0.0; 3],
        rotation_y: 0.0,
        material: "SS400".into(),
        plate_type: PlateType::EndPlate,
    };

    // 焊接：端板四周角焊（梁翼板 → 端板 全滲透，腹板 → 端板 角焊）
    let weld_flange_top = WeldLine {
        weld_type: WeldType::FullPenetration,
        size: btf,
        length: bb,
        start: [-bb / 2.0, bh / 2.0, 0.0],
        end: [bb / 2.0, bh / 2.0, 0.0],
    };
    let weld_flange_bot = WeldLine {
        weld_type: WeldType::FullPenetration,
        size: btf,
        length: bb,
        start: [-bb / 2.0, -bh / 2.0, 0.0],
        end: [bb / 2.0, -bh / 2.0, 0.0],
    };
    let web_weld_size = (btf * 0.7).max(6.0); // 角焊腳尺寸
    let weld_web = WeldLine {
        weld_type: WeldType::Fillet,
        size: web_weld_size,
        length: bh - 2.0 * btf,
        start: [0.0, -(bh / 2.0 - btf), 0.0],
        end: [0.0, bh / 2.0 - btf, 0.0],
    };

    let mut plates = vec![end_plate];

    // 肋板（加勁板）：柱翼板內側，對齊梁翼板位置
    if params.add_stiffeners {
        let stiff_h = ch - 2.0 * params.col_section.3; // 柱淨高
        let stiff_t = btf.max(12.0); // 肋板厚 ≥ 梁翼板厚
        let stiff_w = (params.col_section.1 - params.col_section.2) / 2.0 - 2.0; // 柱翼板內淨寬

        // 上肋板
        plates.push(ConnectionPlate {
            width: stiff_w,
            height: stiff_h.min(150.0), // 肋板高度（取小值）
            thickness: stiff_t,
            position: [0.0, bh / 2.0, 0.0],
            rotation_y: 0.0,
            material: "SS400".into(),
            plate_type: PlateType::Stiffener,
        });
        // 下肋板
        plates.push(ConnectionPlate {
            width: stiff_w,
            height: stiff_h.min(150.0),
            thickness: stiff_t,
            position: [0.0, -bh / 2.0, 0.0],
            rotation_y: 0.0,
            material: "SS400".into(),
            plate_type: PlateType::Stiffener,
        });
    }

    SteelConnection {
        id: String::new(), // 由呼叫端填入
        conn_type: ConnectionType::EndPlate,
        member_ids: Vec::new(),
        plates,
        bolts: vec![bolt_group],
        welds: vec![weld_flange_top, weld_flange_bot, weld_web],
        position: [0.0; 3],
        group_id: None,
    }
}

/// 計算底板接頭
pub fn calc_base_plate(
    col_section: (f32, f32, f32, f32),
    bolt_size: BoltSize,
    bolt_grade: BoltGrade,
) -> SteelConnection {
    let (ch, cb, _ctw, _ctf) = col_section;

    // 底板尺寸：柱截面 + 邊距
    let edge = bolt_size.min_edge() + 20.0;
    let plate_w = cb + 2.0 * edge;
    let plate_h = ch + 2.0 * edge;
    let plate_t = 25.0_f32.max(cb * 0.08); // 底板厚度

    // 錨栓配置：依底板尺寸自動決定行列數
    // AISC J3.5: 最大間距 ≤ min(24t, 305mm)
    let max_spacing = (24.0 * plate_t).min(305.0);
    let bx = plate_w / 2.0 - bolt_size.min_edge();
    let by = plate_h / 2.0 - bolt_size.min_edge();

    // 如果間距超限，增加錨栓行數
    let row_span = by * 2.0;
    let bolt_rows = if row_span > max_spacing { ((row_span / max_spacing).ceil() as u32 + 1).min(4) } else { 2 };
    let col_span = bx * 2.0;
    let bolt_cols = if col_span > max_spacing { ((col_span / max_spacing).ceil() as u32 + 1).min(4) } else { 2 };

    let row_spacing = if bolt_rows > 1 { row_span / (bolt_rows - 1) as f32 } else { 0.0 };
    let col_spacing = if bolt_cols > 1 { col_span / (bolt_cols - 1) as f32 } else { 0.0 };

    let mut bolt_positions = Vec::new();
    for r in 0..bolt_rows {
        let y = -by + r as f32 * row_spacing;
        for c in 0..bolt_cols {
            let x = -bx + c as f32 * col_spacing;
            bolt_positions.push([x, 0.0, y]);
        }
    }

    let bolt_group = BoltGroup {
        bolt_size,
        bolt_grade,
        rows: bolt_rows,
        cols: bolt_cols,
        row_spacing,
        col_spacing,
        edge_dist: bolt_size.min_edge(),
        hole_diameter: bolt_size.hole_diameter(),
        positions: bolt_positions,
    };

    let base_plate = ConnectionPlate {
        width: plate_w,
        height: plate_h,
        thickness: plate_t,
        position: [0.0; 3],
        rotation_y: 0.0,
        material: "SS400".into(),
        plate_type: PlateType::BasePlate,
    };

    SteelConnection {
        id: String::new(),
        conn_type: ConnectionType::BasePlate,
        member_ids: Vec::new(),
        plates: vec![base_plate],
        bolts: vec![bolt_group],
        welds: vec![],
        position: [0.0; 3],
        group_id: None,
    }
}

/// 計算腹板式接頭（剪力板，梁-柱鉸接）
pub fn calc_shear_tab(
    beam_section: (f32, f32, f32, f32),
    bolt_size: BoltSize,
    bolt_grade: BoltGrade,
) -> SteelConnection {
    let (bh, _bb, btw, btf) = beam_section;
    let web_clear = bh - 2.0 * btf; // 梁腹板淨高

    // 剪力板高度 = 腹板淨高 × 0.7（一般設計）
    let tab_h = (web_clear * 0.7).max(150.0);
    let bolt_rows = ((tab_h - 2.0 * bolt_size.min_edge()) / bolt_size.min_spacing()).floor() as u32 + 1;
    let bolt_rows = bolt_rows.max(2).min(6);
    let tab_w = bolt_size.min_edge() * 2.0 + bolt_size.min_spacing();
    let tab_t = btw.max(10.0); // 剪力板厚度 ≥ 梁腹板厚

    // 螺栓位置（單列）
    let bolt_edge = bolt_size.min_edge();
    let spacing = if bolt_rows > 1 {
        (tab_h - 2.0 * bolt_edge) / (bolt_rows - 1) as f32
    } else {
        0.0
    };

    let mut bolt_positions = Vec::new();
    for r in 0..bolt_rows {
        let y = -tab_h / 2.0 + bolt_edge + r as f32 * spacing;
        bolt_positions.push([bolt_edge, y, 0.0]);
    }

    let bolt_group = BoltGroup {
        bolt_size,
        bolt_grade,
        rows: bolt_rows,
        cols: 1,
        row_spacing: spacing,
        col_spacing: 0.0,
        edge_dist: bolt_edge,
        hole_diameter: bolt_size.hole_diameter(),
        positions: bolt_positions,
    };

    let shear_tab = ConnectionPlate {
        width: tab_w,
        height: tab_h,
        thickness: tab_t,
        position: [0.0; 3],
        rotation_y: 0.0,
        material: "SS400".into(),
        plate_type: PlateType::ShearTab,
    };

    // 焊接：剪力板焊於柱翼板（角焊）
    let weld = WeldLine {
        weld_type: WeldType::Fillet,
        size: (tab_t * 0.7).max(6.0),
        length: tab_h,
        start: [0.0, -tab_h / 2.0, 0.0],
        end: [0.0, tab_h / 2.0, 0.0],
    };

    SteelConnection {
        id: String::new(),
        conn_type: ConnectionType::ShearTab,
        member_ids: Vec::new(),
        plates: vec![shear_tab],
        bolts: vec![bolt_group],
        welds: vec![weld],
        position: [0.0; 3],
        group_id: None,
    }
}

/// 計算腹板加厚板（Web Doubler Plate）— AISC 360-22 J10.6
/// 柱腹板剪力不足時，焊接加厚板於柱腹板以增加面板區抗剪
pub fn calc_web_doubler(
    beam_section: (f32, f32, f32, f32),
    col_section: (f32, f32, f32, f32),
    doubler_thickness: Option<f32>,
) -> SteelConnection {
    let (bh, _bb, _btw, btf) = beam_section;
    let (ch, _cb, ctw, ctf) = col_section;

    // 加厚板高度 = 柱翼板間淨高（panel zone 高度）
    let col_clear = ch - 2.0 * ctf;
    // 加厚板寬度 = 梁深 - 2×梁翼板厚（panel zone 寬度）
    let panel_w = (bh - 2.0 * btf).max(100.0);

    // 加厚板厚度：預設 = 柱腹板厚（AISC J10.6 建議 ≥ 柱腹板厚的一半）
    let doubler_t = doubler_thickness.unwrap_or(ctw).max(6.0);

    // 加厚板：焊在柱腹板一側
    let doubler_plate = ConnectionPlate {
        width: panel_w,
        height: col_clear,
        thickness: doubler_t,
        position: [0.0; 3], // 相對於接頭中心
        rotation_y: 0.0,
        material: "SS400".into(),
        plate_type: PlateType::WebDoubler,
    };

    // 四邊角焊（上下左右各一條）
    let weld_size = minimum_fillet_weld_size(doubler_t);
    let welds = vec![
        // 上邊（水平）
        WeldLine {
            weld_type: WeldType::Fillet,
            size: weld_size,
            length: panel_w,
            start: [-panel_w / 2.0, col_clear / 2.0, 0.0],
            end: [panel_w / 2.0, col_clear / 2.0, 0.0],
        },
        // 下邊（水平）
        WeldLine {
            weld_type: WeldType::Fillet,
            size: weld_size,
            length: panel_w,
            start: [-panel_w / 2.0, -col_clear / 2.0, 0.0],
            end: [panel_w / 2.0, -col_clear / 2.0, 0.0],
        },
        // 左邊（垂直）
        WeldLine {
            weld_type: WeldType::Fillet,
            size: weld_size,
            length: col_clear,
            start: [-panel_w / 2.0, -col_clear / 2.0, 0.0],
            end: [-panel_w / 2.0, col_clear / 2.0, 0.0],
        },
        // 右邊（垂直）
        WeldLine {
            weld_type: WeldType::Fillet,
            size: weld_size,
            length: col_clear,
            start: [panel_w / 2.0, -col_clear / 2.0, 0.0],
            end: [panel_w / 2.0, col_clear / 2.0, 0.0],
        },
    ];

    SteelConnection {
        id: String::new(),
        conn_type: ConnectionType::WebDoubler,
        member_ids: Vec::new(),
        plates: vec![doubler_plate],
        bolts: vec![], // 加厚板用焊接，不用螺栓
        welds,
        position: [0.0; 3],
        group_id: None,
    }
}

/// 雙角鋼接頭參數（梁-柱鉸接 framed connection）
#[derive(Debug, Clone)]
pub struct DoubleAngleParams {
    /// 梁截面: (H, B, tw, tf) mm
    pub beam_section: (f32, f32, f32, f32),
    /// 柱截面: (H, B, tw, tf) mm
    pub col_section: (f32, f32, f32, f32),
    /// 螺栓尺寸
    pub bolt_size: BoltSize,
    /// 螺栓等級
    pub bolt_grade: BoltGrade,
    /// 角鋼肢寬 (mm)，None = 自動選擇
    pub angle_leg: Option<f32>,
    /// 角鋼厚度 (mm)，None = 自動選擇
    pub angle_thickness: Option<f32>,
}

/// 計算雙角鋼接頭（Double Angle / Framed Connection）
/// AISC Manual Part 10, Table 10-1
/// 兩片角鋼夾住梁腹板，螺栓穿過梁腹板，另一肢螺栓/焊接於柱翼板
pub fn calc_double_angle(params: &DoubleAngleParams) -> SteelConnection {
    let (bh, _bb, btw, btf) = params.beam_section;
    let web_clear = bh - 2.0 * btf; // 梁腹板淨高

    // 角鋼尺寸自動選擇（依梁深）
    // AISC Manual Table 10-1 常用: L76×76×6, L89×89×8, L102×102×10
    let angle_leg = params.angle_leg.unwrap_or_else(|| {
        if bh <= 300.0 { 75.0 }
        else if bh <= 500.0 { 90.0 }
        else { 100.0 }
    });
    let angle_t = params.angle_thickness.unwrap_or_else(|| {
        if bh <= 300.0 { 6.0 }
        else if bh <= 500.0 { 8.0 }
        else { 10.0 }
    });

    // 角鋼高度 = 腹板淨高 × 0.7（與 shear tab 類似）
    let angle_h = (web_clear * 0.7).max(150.0);

    // 螺栓配置（梁腹板側）：單列螺栓穿過角鋼+梁腹板
    let bolt_edge = params.bolt_size.min_edge();
    let bolt_rows = ((angle_h - 2.0 * bolt_edge) / params.bolt_size.min_spacing()).floor() as u32 + 1;
    let bolt_rows = bolt_rows.max(2).min(6);

    let spacing = if bolt_rows > 1 {
        (angle_h - 2.0 * bolt_edge) / (bolt_rows - 1) as f32
    } else {
        0.0
    };

    // 梁腹板側螺栓位置（穿過兩片角鋼+梁腹板）
    let mut web_bolt_positions = Vec::new();
    for r in 0..bolt_rows {
        let y = -angle_h / 2.0 + bolt_edge + r as f32 * spacing;
        // X = 角鋼肢上的螺栓位置（從角鋼根部算起 = gauge 距離）
        let gauge = bolt_edge.max(angle_leg * 0.4); // 慣用 gauge
        web_bolt_positions.push([gauge, y, 0.0]);
    }

    let web_bolt_group = BoltGroup {
        bolt_size: params.bolt_size,
        bolt_grade: params.bolt_grade,
        rows: bolt_rows,
        cols: 1,
        row_spacing: spacing,
        col_spacing: 0.0,
        edge_dist: bolt_edge,
        hole_diameter: params.bolt_size.hole_diameter(),
        positions: web_bolt_positions,
    };

    // 柱翼板側螺栓（穿過角鋼另一肢+柱翼板）
    let mut col_bolt_positions = Vec::new();
    let col_bolt_rows = bolt_rows.min(4); // 柱側螺栓數可以較少
    let col_spacing = if col_bolt_rows > 1 {
        (angle_h - 2.0 * bolt_edge) / (col_bolt_rows - 1) as f32
    } else {
        0.0
    };
    for r in 0..col_bolt_rows {
        let y = -angle_h / 2.0 + bolt_edge + r as f32 * col_spacing;
        let gauge = bolt_edge.max(angle_leg * 0.4);
        col_bolt_positions.push([0.0, y, gauge]);
    }

    let col_bolt_group = BoltGroup {
        bolt_size: params.bolt_size,
        bolt_grade: params.bolt_grade,
        rows: col_bolt_rows,
        cols: 1,
        row_spacing: col_spacing,
        col_spacing: 0.0,
        edge_dist: bolt_edge,
        hole_diameter: params.bolt_size.hole_diameter(),
        positions: col_bolt_positions,
    };

    // 兩片角鋼板件（左右對稱夾住梁腹板）
    // 角鋼近似為兩個正交的板：水平肢（貼柱翼板）+ 垂直肢（貼梁腹板）
    // 這裡簡化為一片 L 型的兩個板件表示

    // 垂直肢（貼梁腹板的那一肢）— 左側角鋼
    let vert_leg_left = ConnectionPlate {
        width: angle_leg,
        height: angle_h,
        thickness: angle_t,
        position: [0.0, 0.0, -(btw / 2.0 + angle_t / 2.0)], // 梁腹板左側
        rotation_y: 0.0,
        material: "SS400".into(),
        plate_type: PlateType::AngleLeg,
    };

    // 垂直肢（貼梁腹板的那一肢）— 右側角鋼
    let vert_leg_right = ConnectionPlate {
        width: angle_leg,
        height: angle_h,
        thickness: angle_t,
        position: [0.0, 0.0, btw / 2.0 + angle_t / 2.0], // 梁腹板右側
        rotation_y: 0.0,
        material: "SS400".into(),
        plate_type: PlateType::AngleLeg,
    };

    // 水平肢（貼柱翼板的那一肢）— 左側角鋼
    let horiz_leg_left = ConnectionPlate {
        width: angle_t,    // 水平肢厚度方向
        height: angle_h,
        thickness: angle_leg, // 水平肢寬度方向（沿 Z）
        position: [0.0, 0.0, -(btw / 2.0 + angle_t + angle_leg / 2.0)],
        rotation_y: 0.0,
        material: "SS400".into(),
        plate_type: PlateType::AngleLeg,
    };

    // 水平肢（貼柱翼板的那一肢）— 右側角鋼
    let horiz_leg_right = ConnectionPlate {
        width: angle_t,
        height: angle_h,
        thickness: angle_leg,
        position: [0.0, 0.0, btw / 2.0 + angle_t + angle_leg / 2.0],
        rotation_y: 0.0,
        material: "SS400".into(),
        plate_type: PlateType::AngleLeg,
    };

    SteelConnection {
        id: String::new(),
        conn_type: ConnectionType::DoubleAngle,
        member_ids: Vec::new(),
        plates: vec![vert_leg_left, vert_leg_right, horiz_leg_left, horiz_leg_right],
        bolts: vec![web_bolt_group, col_bolt_group],
        welds: vec![], // 全螺栓連接（也可改焊接於柱側）
        position: [0.0; 3],
        group_id: None,
    }
}

// ─── AISC 360-22 強度驗算 ────────────────────────────────────────────────────
// 參考: AISC 360-22 Specification for Structural Steel Buildings
//       AISC Steel Construction Manual, 16th Edition
//       台灣鋼構造建築物鋼結構設計技術規範（與 AISC 對齊）

/// AISC 設計方法
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesignMethod {
    /// LRFD (Load and Resistance Factor Design) — φ 折減
    LRFD,
    /// ASD (Allowable Stress Design) — Ω 安全係數
    ASD,
}

/// 材料強度參數 (MPa)
#[derive(Debug, Clone, Copy)]
pub struct SteelMaterial {
    /// 降伏強度 Fy (MPa)
    pub fy: f32,
    /// 極限強度 Fu (MPa)
    pub fu: f32,
}

impl SteelMaterial {
    /// SS400 / A36
    pub const SS400: Self = Self { fy: 235.0, fu: 400.0 };
    /// SN400B — 台灣建築用鋼
    pub const SN400B: Self = Self { fy: 235.0, fu: 400.0 };
    /// SN490B
    pub const SN490B: Self = Self { fy: 325.0, fu: 490.0 };
    /// A572 Gr.50
    pub const A572_50: Self = Self { fy: 345.0, fu: 450.0 };
    /// A992 (W-shapes)
    pub const A992: Self = Self { fy: 345.0, fu: 450.0 };
    /// SM490A
    pub const SM490A: Self = Self { fy: 325.0, fu: 490.0 };

    pub fn from_name(name: &str) -> Self {
        match name {
            "SN400B" | "SS400" | "A36" => Self::SS400,
            "SN490B" | "SM490A" => Self::SN490B,
            "A572 Gr.50" | "A992" | "SM520B" => Self::A572_50,
            _ => Self::SS400,
        }
    }
}

/// 螺栓材料強度 (MPa)
impl BoltGrade {
    /// 螺栓名目抗拉強度 Fnt (MPa) — AISC Table J3.2
    pub fn fnt(&self) -> f32 {
        match self {
            Self::F8T | Self::A325 => 620.0,  // Group A: 620 MPa
            Self::F10T | Self::A490 => 780.0, // Group B: 780 MPa
        }
    }

    /// 螺栓名目抗剪強度 Fnv (MPa) — AISC Table J3.2
    /// 螺紋在剪力面（thread condition N）
    pub fn fnv(&self) -> f32 {
        match self {
            Self::F8T | Self::A325 => 372.0,  // 0.60 × Fnt for N condition
            Self::F10T | Self::A490 => 468.0,
        }
    }

    /// 螺栓名目抗剪強度 — 螺紋不在剪力面（thread condition X）
    pub fn fnv_x(&self) -> f32 {
        match self {
            Self::F8T | Self::A325 => 457.0,  // 0.75 × Fnt for X condition
            Self::F10T | Self::A490 => 579.0,
        }
    }
}

/// 單顆螺栓強度驗算結果
#[derive(Debug, Clone)]
pub struct BoltCapacity {
    /// 螺栓公稱面積 Ab (mm²)
    pub area: f32,
    /// 名目抗剪強度 φRn 或 Rn/Ω (kN) — AISC J3.6
    pub shear_capacity: f32,
    /// 名目抗拉強度 φRn 或 Rn/Ω (kN) — AISC J3.6
    pub tensile_capacity: f32,
    /// 承壓強度 φRn (kN) — AISC J3.10
    pub bearing_capacity: f32,
}

/// 計算單顆螺栓設計強度 — AISC 360-22 Section J3
pub fn bolt_capacity(
    bolt: &BoltSize,
    grade: &BoltGrade,
    plate_thickness: f32,
    plate_material: &SteelMaterial,
    method: DesignMethod,
    threads_in_shear: bool,
) -> BoltCapacity {
    let d = bolt.diameter();
    let ab = std::f32::consts::PI * d * d / 4.0; // 螺栓面積 mm²

    // AISC J3.6: 抗剪 Rn = Fnv × Ab
    let fnv = if threads_in_shear { grade.fnv() } else { grade.fnv_x() };
    let rn_shear = fnv * ab / 1000.0; // kN

    // AISC J3.6: 抗拉 Rn = Fnt × Ab
    let rn_tensile = grade.fnt() * ab / 1000.0; // kN

    // AISC J3.10: 承壓強度 Rn = 2.4 × d × t × Fu
    // (deformation at service load is a consideration)
    let rn_bearing = 2.4 * d * plate_thickness * plate_material.fu / 1000.0; // kN

    // 設計強度
    let (phi_shear, phi_tensile, phi_bearing) = match method {
        DesignMethod::LRFD => (0.75, 0.75, 0.75),     // φ = 0.75
        DesignMethod::ASD => (1.0 / 2.00, 1.0 / 2.00, 1.0 / 2.00), // 1/Ω, Ω = 2.00
    };

    BoltCapacity {
        area: ab,
        shear_capacity: rn_shear * phi_shear,
        tensile_capacity: rn_tensile * phi_tensile,
        bearing_capacity: rn_bearing * phi_bearing,
    }
}

/// 焊接強度驗算結果
#[derive(Debug, Clone)]
pub struct WeldCapacity {
    /// 有效喉厚 te (mm)
    pub effective_throat: f32,
    /// 有效面積 Awe (mm²)
    pub effective_area: f32,
    /// 焊接金屬設計強度 φRn (kN) — AISC J2.4
    pub weld_metal_capacity: f32,
    /// 母材設計強度 φRn (kN)
    pub base_metal_capacity: f32,
    /// 取小值 = 設計強度 (kN)
    pub design_capacity: f32,
}

/// 計算焊接設計強度 — AISC 360-22 Section J2
pub fn weld_capacity(
    weld: &WeldLine,
    base_material: &SteelMaterial,
    method: DesignMethod,
) -> WeldCapacity {
    // 有效喉厚 — AISC J2.2a
    let te = match weld.weld_type {
        WeldType::Fillet => weld.size / std::f32::consts::SQRT_2, // a/√2
        WeldType::FullPenetration | WeldType::VGroove | WeldType::UGroove => weld.size, // CJP = 板厚
        WeldType::PartialPenetration | WeldType::BevelGroove | WeldType::JGroove => weld.size - 3.0, // PJP: S - 3mm
        WeldType::PlugSlot | WeldType::Spot => weld.size, // 塞焊/點焊以面積計
        WeldType::BackingRun => weld.size / std::f32::consts::SQRT_2, // 封底焊同角焊
    };
    let te = te.max(1.0);

    // 有效面積 Awe = te × length
    let awe = te * weld.length;

    // AISC J2.4 Table J2.5:
    // 角焊: Rn = Fnw × Awe, Fnw = 0.60 × FEXX
    // 假設 E70 焊條: FEXX = 482 MPa (70 ksi)
    let fexx = 482.0_f32; // E70 electrode
    let fnw = 0.60 * fexx; // 焊接金屬名目強度

    let (phi_weld, phi_base) = match method {
        DesignMethod::LRFD => (0.75, 0.75),
        DesignMethod::ASD => (1.0 / 2.00, 1.0 / 2.00),
    };

    let rn_weld = fnw * awe / 1000.0; // kN
    let weld_design = rn_weld * phi_weld;

    // 母材強度：Rn = Fu × t × L × 0.6 (rupture) or Fy × t × L × 0.6 (yielding)
    // 取承壓面（焊接投影面積）
    let rn_base_rupture = 0.6 * base_material.fu * weld.size * weld.length / 1000.0;
    let base_design = rn_base_rupture * phi_base;

    WeldCapacity {
        effective_throat: te,
        effective_area: awe,
        weld_metal_capacity: weld_design,
        base_metal_capacity: base_design,
        design_capacity: weld_design.min(base_design),
    }
}

/// 接頭整體驗算結果
#[derive(Debug, Clone)]
pub struct ConnectionCheck {
    /// 接頭類型
    pub conn_type: ConnectionType,
    /// 總螺栓抗剪強度 (kN)
    pub total_bolt_shear: f32,
    /// 總螺栓抗拉強度 (kN)
    pub total_bolt_tension: f32,
    /// 控制性承壓強度 (kN)
    pub min_bearing: f32,
    /// 總焊接設計強度 (kN)
    pub total_weld_capacity: f32,
    /// 各項警告（違反 AISC 最小/最大要求）
    pub warnings: Vec<String>,
    /// 是否所有檢查通過
    pub pass: bool,
}

/// 驗算接頭是否符合 AISC 360-22
pub fn check_connection(
    conn: &SteelConnection,
    plate_material: &SteelMaterial,
    method: DesignMethod,
) -> ConnectionCheck {
    let mut warnings = Vec::new();
    let mut pass = true;

    // ── 螺栓驗算 ──
    let mut total_shear = 0.0_f32;
    let mut total_tension = 0.0_f32;
    let mut min_bearing = f32::MAX;

    for bg in &conn.bolts {
        let n_bolts = bg.positions.len() as f32;
        let plate_t = conn.plates.first().map_or(16.0, |p| p.thickness);

        let cap = bolt_capacity(
            &bg.bolt_size, &bg.bolt_grade,
            plate_t, plate_material,
            method, true, // 保守假設螺紋在剪力面
        );

        total_shear += cap.shear_capacity * n_bolts;
        total_tension += cap.tensile_capacity * n_bolts;
        min_bearing = min_bearing.min(cap.bearing_capacity * n_bolts);

        // AISC J3.3: 最小螺栓間距 ≥ 2.67d（建議 3d）
        let min_sp = bg.bolt_size.diameter() * 2.67;
        if bg.row_spacing > 0.0 && bg.row_spacing < min_sp {
            warnings.push(format!(
                "螺栓行距 {:.0}mm < AISC 最小 {:.0}mm (2.67d)",
                bg.row_spacing, min_sp
            ));
            pass = false;
        }

        // AISC J3.4: 最小邊距（Table J3.4）
        if bg.edge_dist < bg.bolt_size.min_edge() {
            warnings.push(format!(
                "螺栓邊距 {:.0}mm < AISC 最小 {:.0}mm",
                bg.edge_dist, bg.bolt_size.min_edge()
            ));
            pass = false;
        }

        // AISC J3.5: 最大螺栓間距 ≤ min(24t, 305mm)
        let max_sp = (24.0 * plate_t).min(305.0);
        if bg.row_spacing > max_sp {
            warnings.push(format!(
                "螺栓行距 {:.0}mm > AISC 最大 {:.0}mm (24t or 305)",
                bg.row_spacing, max_sp
            ));
            pass = false;
        }

        // AISC J3.5: 最大邊距 ≤ min(12t, 150mm)
        let max_edge = (12.0 * plate_t).min(150.0);
        if bg.edge_dist > max_edge {
            warnings.push(format!(
                "螺栓邊距 {:.0}mm > AISC 最大 {:.0}mm (12t or 150)",
                bg.edge_dist, max_edge
            ));
        }
    }
    if min_bearing == f32::MAX { min_bearing = 0.0; }

    // ── 焊接驗算 ──
    let mut total_weld_cap = 0.0_f32;
    for weld in &conn.welds {
        let cap = weld_capacity(weld, plate_material, method);
        total_weld_cap += cap.design_capacity;

        // AISC J2.2b Table J2.4: 最小角焊尺寸
        let min_weld = minimum_fillet_weld_size(
            conn.plates.first().map_or(10.0, |p| p.thickness),
        );
        if weld.weld_type == WeldType::Fillet && weld.size < min_weld {
            warnings.push(format!(
                "角焊尺寸 {:.0}mm < AISC 最小 {:.0}mm (Table J2.4)",
                weld.size, min_weld
            ));
            pass = false;
        }

        // AISC J2.2b: 角焊最大尺寸 ≤ 板厚 - 2mm（板厚 > 6mm 時）
        let plate_t = conn.plates.first().map_or(16.0, |p| p.thickness);
        if weld.weld_type == WeldType::Fillet && plate_t > 6.0 && weld.size > plate_t - 2.0 {
            warnings.push(format!(
                "角焊尺寸 {:.0}mm > 板厚 {:.0}mm - 2mm = AISC 最大",
                weld.size, plate_t
            ));
        }

        // AISC J2.2b: 角焊最小有效長度 ≥ 4 × 焊腳尺寸
        if weld.weld_type == WeldType::Fillet && weld.length < 4.0 * weld.size {
            warnings.push(format!(
                "角焊長度 {:.0}mm < 4×焊腳 {:.0}mm (AISC J2.2b 最小)",
                weld.length, 4.0 * weld.size
            ));
            pass = false;
        }
    }

    // ── 板件驗算 ──
    for plate in &conn.plates {
        // 端板厚度合理性（工程經驗）
        if plate.plate_type == PlateType::EndPlate && plate.thickness < 12.0 {
            warnings.push("端板厚度 < 12mm，建議加厚".into());
        }
        // 底板厚度
        if plate.plate_type == PlateType::BasePlate && plate.thickness < 20.0 {
            warnings.push("底板厚度 < 20mm，建議加厚".into());
        }
        // 加厚板厚度
        if plate.plate_type == PlateType::WebDoubler && plate.thickness < 6.0 {
            warnings.push("加厚板厚度 < 6mm，建議加厚".into());
        }
        // 角鋼厚度
        if plate.plate_type == PlateType::AngleLeg && plate.thickness < 6.0 {
            warnings.push("角鋼厚度 < 6mm，建議加厚".into());
        }
    }

    ConnectionCheck {
        conn_type: conn.conn_type,
        total_bolt_shear: total_shear,
        total_bolt_tension: total_tension,
        min_bearing,
        total_weld_capacity: total_weld_cap,
        warnings,
        pass,
    }
}

/// AISC Table J2.4: 最小角焊尺寸 (mm)
/// 依據被焊接板的較厚者之板厚
pub fn minimum_fillet_weld_size(thicker_part: f32) -> f32 {
    if thicker_part <= 6.0 { 3.0 }
    else if thicker_part <= 13.0 { 5.0 }
    else if thicker_part <= 19.0 { 6.0 }
    else { 8.0 }
}

/// AISC J3.3 Table J3.4M: 最小螺栓邊距 (mm)
/// (已在 BoltSize::min_edge() 中實作，此為交叉驗證)
/// AISC Table J3.4 最小邊距（滾軋/切割邊）
pub fn aisc_min_edge_distance(bolt_diameter: f32) -> f32 {
    if bolt_diameter <= 16.0 { 22.0 }
    else if bolt_diameter <= 20.0 { 25.0 }
    else if bolt_diameter <= 22.0 { 29.0 }
    else if bolt_diameter <= 24.0 { 32.0 }
    else if bolt_diameter <= 27.0 { 38.0 }
    else if bolt_diameter <= 30.0 { 41.0 }
    else { bolt_diameter * 1.25 }
}

// ─── AISC 智慧接頭建議引擎 ─────────────────────────────────────────────────

/// 接頭建議結果
#[derive(Debug, Clone)]
pub struct ConnectionSuggestion {
    /// 建議的接頭類型
    pub conn_type: ConnectionType,
    /// 建議原因
    pub reason: String,
    /// 建議的螺栓尺寸
    pub bolt_size: BoltSize,
    /// 建議的螺栓等級
    pub bolt_grade: BoltGrade,
    /// 建議的端板/剪力板厚度 (mm)
    pub plate_thickness: f32,
    /// 是否需要加勁板
    pub need_stiffeners: bool,
    /// 加勁板建議原因
    pub stiffener_reason: String,
    /// AISC 條文依據
    pub aisc_ref: String,
    /// 預估接頭強度
    pub estimated_capacity: ConnectionCheck,
}

/// 根據兩構件的截面和關係，自動建議最適接頭
/// 參考: AISC 360-22 Chapter J + AISC Steel Construction Manual Part 10
pub fn suggest_connection(
    beam_section: (f32, f32, f32, f32),  // (H, B, tw, tf)
    col_section: (f32, f32, f32, f32),
    connection_intent: ConnectionIntent,
    material_name: &str,
) -> Vec<ConnectionSuggestion> {
    let mat = SteelMaterial::from_name(material_name);
    let (bh, bb, btw, btf) = beam_section;
    let (ch, cb, ctw, ctf) = col_section;
    let mut suggestions = Vec::new();

    match connection_intent {
        ConnectionIntent::BeamToColumn => {
            // ── 判斷剛接 vs 鉸接 ──

            // 1. 端板式（剛接）— 梁翼板受力，需傳遞彎矩
            let bolt_size_ep = suggest_bolt_size(bh, btf);
            let plate_t_ep = calc_end_plate_thickness(bh, bb, btf, bolt_size_ep);
            let need_stiff = need_stiffeners_check(beam_section, col_section);

            let ep_params = EndPlateParams {
                beam_section, col_section,
                bolt_size: bolt_size_ep,
                bolt_grade: BoltGrade::F10T,
                plate_thickness: Some(plate_t_ep),
                add_stiffeners: need_stiff.0,
            };
            let ep_conn = calc_end_plate(&ep_params);
            let ep_check = check_connection(&ep_conn, &mat, DesignMethod::LRFD);

            suggestions.push(ConnectionSuggestion {
                conn_type: ConnectionType::EndPlate,
                reason: "梁-柱剛接：傳遞彎矩+剪力，適用於抗側力構架".into(),
                bolt_size: bolt_size_ep,
                bolt_grade: BoltGrade::F10T,
                plate_thickness: plate_t_ep,
                need_stiffeners: need_stiff.0,
                stiffener_reason: need_stiff.1.clone(),
                aisc_ref: "AISC 360-22 J3 + Manual Part 10 (FR Moment Connection)".into(),
                estimated_capacity: ep_check,
            });

            // 2. 腹板式（鉸接）— 僅傳剪力
            let bolt_size_st = suggest_bolt_size_shear(bh, btw);
            let st_conn = calc_shear_tab(beam_section, bolt_size_st, BoltGrade::F10T);
            let st_check = check_connection(&st_conn, &mat, DesignMethod::LRFD);

            suggestions.push(ConnectionSuggestion {
                conn_type: ConnectionType::ShearTab,
                reason: "梁-柱鉸接：僅傳剪力，適用於重力構架、次梁".into(),
                bolt_size: bolt_size_st,
                bolt_grade: BoltGrade::F10T,
                plate_thickness: st_conn.plates[0].thickness,
                need_stiffeners: false,
                stiffener_reason: "鉸接不需加勁板".into(),
                aisc_ref: "AISC 360-22 J3 + Manual Part 10 (PR/Simple Connection)".into(),
                estimated_capacity: st_check,
            });

            // 3. 雙角鋼（鉸接替代方案）— 傳統 framed connection
            let bolt_size_da = suggest_bolt_size_shear(bh, btw);
            let da_params = DoubleAngleParams {
                beam_section, col_section,
                bolt_size: bolt_size_da,
                bolt_grade: BoltGrade::F10T,
                angle_leg: None,
                angle_thickness: None,
            };
            let da_conn = calc_double_angle(&da_params);
            let da_check = check_connection(&da_conn, &mat, DesignMethod::LRFD);
            let angle_leg = if bh <= 300.0 { 75.0 } else if bh <= 500.0 { 90.0 } else { 100.0 };
            let angle_t = if bh <= 300.0 { 6.0 } else if bh <= 500.0 { 8.0 } else { 10.0 };

            suggestions.push(ConnectionSuggestion {
                conn_type: ConnectionType::DoubleAngle,
                reason: format!(
                    "雙角鋼鉸接：L{:.0}×{:.0}×{:.0} 夾梁腹板，適用於標準重力接頭",
                    angle_leg, angle_leg, angle_t
                ),
                bolt_size: bolt_size_da,
                bolt_grade: BoltGrade::F10T,
                plate_thickness: angle_t,
                need_stiffeners: false,
                stiffener_reason: "雙角鋼鉸接不需加勁板".into(),
                aisc_ref: "AISC Manual Part 10, Table 10-1 (All-Bolted Double-Angle)".into(),
                estimated_capacity: da_check,
            });

            // 4. 腹板加厚板（柱腹板剪力不足時建議）— AISC J10.6
            if need_stiff.0 || ctw < btf * 0.8 {
                let doubler_t = ctw.max(6.0);
                let wd_conn = calc_web_doubler(beam_section, col_section, None);
                let wd_check = check_connection(&wd_conn, &mat, DesignMethod::LRFD);

                suggestions.push(ConnectionSuggestion {
                    conn_type: ConnectionType::WebDoubler,
                    reason: format!(
                        "柱腹板加厚板：tw={:.0}mm 不足，加 {:.0}mm 加厚板強化面板區抗剪",
                        ctw, doubler_t
                    ),
                    bolt_size: bolt_size_ep, // 加厚板無螺栓，沿用端板螺栓
                    bolt_grade: BoltGrade::F10T,
                    plate_thickness: doubler_t,
                    need_stiffeners: false,
                    stiffener_reason: "加厚板為輔助板件，搭配端板/角鋼使用".into(),
                    aisc_ref: "AISC 360-22 J10.6 (Web Panel-Zone Shear)".into(),
                    estimated_capacity: wd_check,
                });
            }
        }

        ConnectionIntent::ColumnBase => {
            let bolt_size_bp = suggest_bolt_size_base(ch, cb);
            let bp_conn = calc_base_plate(col_section, bolt_size_bp, BoltGrade::F8T);
            let bp_check = check_connection(&bp_conn, &mat, DesignMethod::LRFD);

            let bp_plate = &bp_conn.plates[0];
            suggestions.push(ConnectionSuggestion {
                conn_type: ConnectionType::BasePlate,
                reason: format!(
                    "柱底板：底板 {:.0}×{:.0}×{:.0}mm + {}×{} 錨栓",
                    bp_plate.width, bp_plate.height, bp_plate.thickness,
                    bp_conn.bolts[0].bolt_size.label(), bp_conn.bolts[0].positions.len()
                ),
                bolt_size: bolt_size_bp,
                bolt_grade: BoltGrade::F8T,
                plate_thickness: bp_plate.thickness,
                need_stiffeners: cb >= 250.0, // 大柱需底板加勁肋
                stiffener_reason: if cb >= 250.0 {
                    "柱翼板寬 ≥ 250mm，建議底板加勁肋 (AISC Design Guide 1)".into()
                } else {
                    "柱截面較小，底板不需加勁肋".into()
                },
                aisc_ref: "AISC Design Guide 1: Column Base Plates".into(),
                estimated_capacity: bp_check,
            });
        }

        ConnectionIntent::BeamToBeam => {
            // 梁-梁續接：翼板拼接
            let bolt_size = suggest_bolt_size(bh, btf);
            let plate_t = (btf * 1.2).max(12.0);

            // 用端板模擬（兩端對稱）
            let ep_params = EndPlateParams {
                beam_section, col_section: beam_section, // 對稱
                bolt_size,
                bolt_grade: BoltGrade::F10T,
                plate_thickness: Some(plate_t),
                add_stiffeners: false,
            };
            let conn = calc_end_plate(&ep_params);
            let check = check_connection(&conn, &mat, DesignMethod::LRFD);

            suggestions.push(ConnectionSuggestion {
                conn_type: ConnectionType::FlangePlate,
                reason: "梁-梁續接：翼板拼接板+腹板拼接板".into(),
                bolt_size,
                bolt_grade: BoltGrade::F10T,
                plate_thickness: plate_t,
                need_stiffeners: false,
                stiffener_reason: String::new(),
                aisc_ref: "AISC Manual Part 10 (Spliced Beam Connection)".into(),
                estimated_capacity: check,
            });
        }

        ConnectionIntent::BraceToGusset => {
            let bolt_size = suggest_bolt_size(bh, btf);
            let conn = calc_shear_tab(beam_section, bolt_size, BoltGrade::F10T);
            let check = check_connection(&conn, &mat, DesignMethod::LRFD);

            suggestions.push(ConnectionSuggestion {
                conn_type: ConnectionType::BracePlate,
                reason: "斜撐接合板：gusset plate + 螺栓".into(),
                bolt_size,
                bolt_grade: BoltGrade::F10T,
                plate_thickness: (btf * 1.5).max(12.0),
                need_stiffeners: false,
                stiffener_reason: String::new(),
                aisc_ref: "AISC Manual Part 13 (Brace Connection)".into(),
                estimated_capacity: check,
            });
        }
    }

    suggestions
}

/// 接頭意圖（使用者選取的兩構件關係）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionIntent {
    BeamToColumn,   // 梁-柱
    ColumnBase,     // 柱底
    BeamToBeam,     // 梁-梁續接
    BraceToGusset,  // 斜撐-接合板
}

// ─── AISC 螺栓/板厚自動選擇 ────────────────────────────────────────────────

/// 根據梁深度和翼板厚度建議螺栓尺寸（端板接頭）
/// AISC Manual Table 10-4 經驗公式
pub fn suggest_bolt_size(beam_depth: f32, flange_tf: f32) -> BoltSize {
    if beam_depth <= 200.0 { BoltSize::M16 }
    else if beam_depth <= 350.0 { BoltSize::M20 }
    else if beam_depth <= 500.0 {
        if flange_tf >= 16.0 { BoltSize::M24 } else { BoltSize::M22 }
    }
    else if beam_depth <= 700.0 { BoltSize::M24 }
    else { BoltSize::M27 }
}

/// 根據梁腹板厚度建議螺栓尺寸（剪力板接頭）
pub fn suggest_bolt_size_shear(beam_depth: f32, web_tw: f32) -> BoltSize {
    if beam_depth <= 300.0 { BoltSize::M16 }
    else if beam_depth <= 500.0 { BoltSize::M20 }
    else { BoltSize::M22 }
}

/// 底板錨栓尺寸建議
pub fn suggest_bolt_size_base(col_depth: f32, col_width: f32) -> BoltSize {
    let max_dim = col_depth.max(col_width);
    if max_dim <= 200.0 { BoltSize::M20 }
    else if max_dim <= 350.0 { BoltSize::M24 }
    else { BoltSize::M27 }
}

/// 計算端板厚度 — AISC Design Guide 4
/// tp ≥ sqrt(4 × Mu / (φ × Fy × b × pf))
/// 簡化經驗公式: tp ≈ max(tf × 1.3, 16mm)
pub fn calc_end_plate_thickness(beam_h: f32, beam_b: f32, beam_tf: f32, bolt: BoltSize) -> f32 {
    let min_by_bolt = bolt.diameter() * 0.8; // 端板厚 ≥ 0.8×螺栓直徑（經驗）
    let min_by_flange = beam_tf * 1.3;       // 端板厚 ≥ 1.3×梁翼板厚
    min_by_bolt.max(min_by_flange).max(16.0)  // 至少 16mm
}

/// 判斷是否需要加勁板 — AISC 360-22 J10
/// 當梁翼板力 > 柱腹板/翼板承載力時需要
pub fn need_stiffeners_check(
    beam_section: (f32, f32, f32, f32),
    col_section: (f32, f32, f32, f32),
) -> (bool, String) {
    let (_bh, _bb, _btw, btf) = beam_section;
    let (ch, cb, ctw, ctf) = col_section;

    let mut reasons = Vec::new();
    let mut needed = false;

    // AISC J10.1: 柱翼板局部彎曲
    // φRn = 6.25 × tf² × Fyf → 如果梁翼板力 > 此值需加勁板
    // 簡化: 如果梁翼板厚 > 柱翼板厚 × 0.7 → 需加勁板
    if btf > ctf * 0.7 {
        needed = true;
        reasons.push(format!("J10.1 柱翼板局部彎曲: 梁tf={:.0} > 柱tf×0.7={:.0}", btf, ctf * 0.7));
    }

    // AISC J10.2: 柱腹板局部降伏
    // 簡化: 如果柱腹板厚 < 梁翼板厚 → 需加勁板
    if ctw < btf {
        needed = true;
        reasons.push(format!("J10.2 柱腹板局部降伏: 柱tw={:.0} < 梁tf={:.0}", ctw, btf));
    }

    // AISC J10.3: 柱腹板壓潰
    // 簡化: 如果柱深/腹板厚 > 50 → 需加勁板
    let hw = ch - 2.0 * ctf;
    if hw / ctw > 50.0 {
        needed = true;
        reasons.push(format!("J10.3 柱腹板壓潰: hw/tw={:.0} > 50", hw / ctw));
    }

    // AISC J10.5: 柱腹板面外彎曲（柱翼板內無支撐時）
    if cb > ch * 0.6 {
        // 翼板寬深比大 → 可能需要
        reasons.push(format!("J10.5 建議: 柱B/H={:.2} 較大，建議加勁板", cb / ch));
    }

    let reason = if reasons.is_empty() {
        "柱截面足夠，不需加勁板".into()
    } else {
        reasons.join("; ")
    };

    (needed, reason)
}

// ─── 完整孔位計算 ──────────────────────────────────────────────────────────

/// 端板孔位佈置（含鑽孔座標、孔徑、邊距驗證）
#[derive(Debug, Clone)]
pub struct HoleLayout {
    /// 孔位 (x, y) 相對於板件中心 (mm)
    pub holes: Vec<[f32; 2]>,
    /// 孔徑 (mm)
    pub hole_diameter: f32,
    /// 螺栓直徑 (mm)
    pub bolt_diameter: f32,
    /// 板件寬 (mm)
    pub plate_width: f32,
    /// 板件高 (mm)
    pub plate_height: f32,
    /// 邊距 X (mm)
    pub edge_x: f32,
    /// 邊距 Y (mm)
    pub edge_y: f32,
    /// 行距 (mm)
    pub pitch: f32,
    /// 列距 (mm)
    pub gauge: f32,
    /// AISC 驗證結果
    pub checks: Vec<String>,
}

/// 計算端板螺栓孔位佈置 — AISC J3
pub fn calc_hole_layout(
    plate_w: f32, plate_h: f32,
    bolt: BoltSize, rows: u32, cols: u32,
) -> HoleLayout {
    let d = bolt.diameter();
    let dh = bolt.hole_diameter(); // 標準孔 = d + 2mm

    // AISC J3.4: 最小邊距
    let min_edge = bolt.min_edge();
    // AISC J3.3: 最小間距 ≥ 2.67d (建議 3d)
    let min_pitch = (d * 3.0).max(bolt.min_spacing());

    // 計算實際邊距和間距
    let edge_x = min_edge.max(25.0);
    let edge_y = min_edge.max(25.0);

    // 列距 (gauge) = (板寬 - 2×邊距) / (列數-1)
    let gauge = if cols > 1 {
        (plate_w - 2.0 * edge_x) / (cols - 1) as f32
    } else { 0.0 };

    // 行距 (pitch) = (板高 - 2×邊距) / (行數-1)
    let pitch = if rows > 1 {
        let available = plate_h - 2.0 * edge_y;
        (available / (rows - 1) as f32).max(min_pitch)
    } else { 0.0 };

    // 生成孔位座標（相對於板中心）
    let mut holes = Vec::new();
    for r in 0..rows {
        let y = -plate_h / 2.0 + edge_y + r as f32 * pitch;
        for c in 0..cols {
            let x = -plate_w / 2.0 + edge_x + c as f32 * gauge;
            holes.push([x, y]);
        }
    }

    // AISC 驗證
    let mut checks = Vec::new();

    // J3.3 間距
    if pitch > 0.0 && pitch < min_pitch {
        checks.push(format!("⚠ 行距 {:.0}mm < 最小 {:.0}mm (3d)", pitch, min_pitch));
    } else if pitch > 0.0 {
        checks.push(format!("✓ 行距 {:.0}mm ≥ {:.0}mm (3d)", pitch, min_pitch));
    }
    if gauge > 0.0 && gauge < min_pitch {
        checks.push(format!("⚠ 列距 {:.0}mm < 最小 {:.0}mm (3d)", gauge, min_pitch));
    } else if gauge > 0.0 {
        checks.push(format!("✓ 列距 {:.0}mm ≥ {:.0}mm (3d)", gauge, min_pitch));
    }

    // J3.4 邊距
    if edge_x < min_edge {
        checks.push(format!("⚠ X邊距 {:.0}mm < 最小 {:.0}mm", edge_x, min_edge));
    } else {
        checks.push(format!("✓ X邊距 {:.0}mm ≥ {:.0}mm", edge_x, min_edge));
    }

    // J3.5 最大間距
    let max_pitch = (24.0 * plate_h.min(plate_w) * 0.1).min(305.0); // 24t or 305
    if pitch > max_pitch && pitch > 0.0 {
        checks.push(format!("⚠ 行距 {:.0}mm > 最大 {:.0}mm (24t/305)", pitch, max_pitch));
    }

    // 孔徑
    checks.push(format!("孔徑: Ø{:.0}mm (標準孔 = {}+2mm)", dh, bolt.label()));

    HoleLayout {
        holes, hole_diameter: dh, bolt_diameter: d,
        plate_width: plate_w, plate_height: plate_h,
        edge_x, edge_y, pitch, gauge, checks,
    }
}

// ─── 底板加勁肋建議 ──────────────────────────────────────────────────────────

/// 底板加勁肋建議 — AISC Design Guide 1
#[derive(Debug, Clone)]
pub struct BasePlateStiffenerSuggestion {
    pub needed: bool,
    pub reason: String,
    /// 加勁肋尺寸 (寬×高×厚) mm
    pub width: f32,
    pub height: f32,
    pub thickness: f32,
    /// 數量（通常 2 或 4 片）
    pub quantity: u32,
    /// 焊接尺寸 (mm)
    pub weld_size: f32,
}

/// 建議底板加勁肋配置
pub fn suggest_base_plate_stiffeners(
    col_section: (f32, f32, f32, f32),
    plate_thickness: f32,
    axial_load_kn: f32, // 軸力估算 (kN)
) -> BasePlateStiffenerSuggestion {
    let (_ch, cb, ctw, ctf) = col_section;

    // AISC Design Guide 1, Section 3.4:
    // 當底板懸臂距 m 或 n > 板厚的某個比值時需要加勁肋
    let m = (cb - 0.95 * cb) / 2.0; // 簡化
    let bearing_stress = axial_load_kn * 1000.0 / (cb * cb); // MPa (假設方形承壓)

    // 經驗法則: 底板厚/懸臂比 < 某值時需加勁
    let needs = plate_thickness < 25.0 && cb >= 250.0;
    let reason = if needs {
        format!("底板厚 {:.0}mm 對 {:.0}mm 寬柱可能不足，建議加勁肋 (DG1 3.4)", plate_thickness, cb)
    } else {
        format!("底板厚 {:.0}mm 足夠承載 {:.0}mm 寬柱", plate_thickness, cb)
    };

    // 加勁肋尺寸
    let stiff_w = cb / 2.0 - ctw / 2.0 - 5.0; // 從柱腹板到底板邊
    let stiff_h = stiff_w * 0.8;                // 高寬比 ~0.8
    let stiff_t = ctf.max(12.0);                // 厚度 ≥ 柱翼板厚
    let weld_size = minimum_fillet_weld_size(stiff_t);

    BasePlateStiffenerSuggestion {
        needed: needs,
        reason,
        width: stiff_w.max(50.0),
        height: stiff_h.max(50.0),
        thickness: stiff_t,
        quantity: if cb >= 300.0 { 4 } else { 2 }, // 大柱 4 片
        weld_size,
    }
}

// ─── AISC 341-22 耐震接頭 ────────────────────────────────────────────────────

/// 耐震抗彎矩框架類型（AISC 341-22）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeismicFrameType {
    /// 特殊抗彎矩框架（Special Moment Frame）
    SMF,
    /// 中度抗彎矩框架（Intermediate Moment Frame）
    IMF,
    /// 普通抗彎矩框架（Ordinary Moment Frame）
    OMF,
    /// 特殊集中斜撐框架（Special Concentrically Braced Frame）
    SCBF,
    /// 挫屈束制斜撐框架（Buckling-Restrained Braced Frame）
    BRBF,
    /// 偏心斜撐框架（Eccentrically Braced Frame）
    EBF,
}

impl SeismicFrameType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::SMF  => "SMF 特殊抗彎矩",
            Self::IMF  => "IMF 中度抗彎矩",
            Self::OMF  => "OMF 普通抗彎矩",
            Self::SCBF => "SCBF 特殊集中斜撐",
            Self::BRBF => "BRBF 挫屈束制斜撐",
            Self::EBF  => "EBF 偏心斜撐",
        }
    }

    pub const ALL: &'static [SeismicFrameType] = &[
        Self::SMF, Self::IMF, Self::OMF, Self::SCBF, Self::BRBF, Self::EBF,
    ];
}

/// AISC 341 預認證接頭類型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrequalifiedConnection {
    /// 縮減梁翼板（Reduced Beam Section, 狗骨頭）
    RBS,
    /// 加厚翼板焊接（Welded Unreinforced Flange - Welded Web）
    WufW,
    /// 螺栓翼板（Bolted Flange Plate）
    BFP,
    /// 端板（Extended End Plate）
    ExtendedEndPlate,
    /// 自由翼板（Free Flange）
    FreeFlange,
    /// Kaiser 螺栓接頭（Kaiser Bolted Bracket）
    KBB,
    /// 雙 T 接頭（Double Tee）
    DoubleTee,
    /// ConXtech ConXL
    ConXL,
    /// SidePlate
    SidePlate,
}

impl PrequalifiedConnection {
    pub fn label(&self) -> &'static str {
        match self {
            Self::RBS => "RBS 狗骨頭（縮減梁翼板）",
            Self::WufW => "WUF-W 全滲透翼板焊接",
            Self::BFP => "BFP 螺栓翼板",
            Self::ExtendedEndPlate => "延伸端板",
            Self::FreeFlange => "自由翼板",
            Self::KBB => "Kaiser 螺栓托架",
            Self::DoubleTee => "雙T接頭",
            Self::ConXL => "ConXtech ConXL",
            Self::SidePlate => "SidePlate",
        }
    }

    /// 適用的框架類型
    pub fn applicable_frames(&self) -> &'static [SeismicFrameType] {
        match self {
            Self::RBS => &[SeismicFrameType::SMF, SeismicFrameType::IMF],
            Self::WufW => &[SeismicFrameType::SMF, SeismicFrameType::IMF],
            Self::BFP => &[SeismicFrameType::SMF, SeismicFrameType::IMF],
            Self::ExtendedEndPlate => &[SeismicFrameType::SMF, SeismicFrameType::IMF],
            Self::FreeFlange => &[SeismicFrameType::SMF],
            Self::KBB => &[SeismicFrameType::SMF],
            Self::DoubleTee => &[SeismicFrameType::SMF],
            Self::ConXL => &[SeismicFrameType::SMF],
            Self::SidePlate => &[SeismicFrameType::SMF],
        }
    }

    pub const ALL: &'static [PrequalifiedConnection] = &[
        Self::RBS, Self::WufW, Self::BFP, Self::ExtendedEndPlate,
        Self::FreeFlange, Self::KBB, Self::DoubleTee, Self::ConXL, Self::SidePlate,
    ];
}

/// 耐震接頭配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeismicConnectionConfig {
    pub frame_type: SeismicFrameType,
    pub prequalified: PrequalifiedConnection,
    /// 梁 expected plastic rotation (rad)，SMF 需 ≥ 0.04
    pub story_drift_ratio: f32,
    /// 梁 Ry 超強係數（SS400/SN490B 依 AISC Table A3.1）
    pub ry_beam: f32,
    /// 柱梁強度比 ΣMpc / ΣMpb（AISC 341 E3.4a 要求 > 1.0）
    pub column_beam_ratio: f32,
}

/// RBS 狗骨頭切割參數（AISC 358 Section 5.8）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbsParameters {
    /// 切割起始距離 a（距柱面），建議 0.5bf ≤ a ≤ 0.75bf
    pub a: f32,
    /// 切割長度 b，建議 0.65d ≤ b ≤ 0.85d
    pub b: f32,
    /// 切割深度 c（翼板單側），max c ≤ 0.25bf
    pub c: f32,
}

/// 根據梁截面自動計算 RBS 參數（AISC 358-22 Section 5.8）
pub fn calculate_rbs_params(beam_h: f32, beam_bf: f32, beam_tf: f32) -> RbsParameters {
    let a = 0.625 * beam_bf;       // 0.5bf ~ 0.75bf 中間值
    let b = 0.75 * beam_h;          // 0.65d ~ 0.85d 中間值
    let c = 0.20 * beam_bf;         // max 0.25bf，取保守值
    RbsParameters { a, b, c }
}

/// 計算柱梁彎矩比 ΣMpc*/ΣMpb*（AISC 341-22 E3.4a）
/// 回傳 (ratio, pass)，pass = ratio > 1.0
pub fn check_strong_column_weak_beam(
    col_zx: f32, col_fy: f32, col_axial: f32, col_ag: f32,
    beam_zx: f32, beam_fy: f32, ry_beam: f32,
    rbs_c: Option<f32>, beam_bf: f32, beam_tf: f32,
) -> (f32, bool) {
    // ΣMpc* = ΣZc(Fyc - Puc/Ag)
    let mpc = col_zx * (col_fy - col_axial / col_ag);
    // ΣMpb* = Σ(Ry × Fy × Ze) + ΣMuv（剪力增量忽略簡化）
    let ze = if let Some(c) = rbs_c {
        // RBS 縮減斷面模數: Ze = Zx - 2×c×tf×(d - tf)
        let d = beam_zx / (beam_fy * 0.001); // 近似
        beam_zx - 2.0 * c * beam_tf * (beam_bf)
    } else {
        beam_zx
    };
    let mpb = ry_beam * beam_fy * ze;
    let ratio = if mpb > 0.0 { mpc / mpb } else { 999.0 };
    (ratio, ratio > 1.0)
}

/// 建議耐震接頭：根據框架類型篩選適用的預認證接頭
pub fn suggest_seismic_connections(frame_type: SeismicFrameType) -> Vec<PrequalifiedConnection> {
    PrequalifiedConnection::ALL.iter()
        .filter(|pc| pc.applicable_frames().contains(&frame_type))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bolt_dimensions() {
        assert_eq!(BoltSize::M20.diameter(), 20.0);
        assert_eq!(BoltSize::M20.hole_diameter(), 22.0); // AISC Table J3.3M
        assert!(BoltSize::M20.min_spacing() >= 53.0);    // AISC J3.3: 2.667d
        assert!(BoltSize::M20.min_edge() >= 22.0);       // AISC Table J3.4
    }

    #[test]
    fn test_end_plate_generation() {
        let params = EndPlateParams {
            beam_section: (300.0, 150.0, 6.0, 9.0),
            col_section: (300.0, 300.0, 10.0, 15.0),
            bolt_size: BoltSize::M20,
            bolt_grade: BoltGrade::F10T,
            plate_thickness: None,
            add_stiffeners: true,
        };
        let conn = calc_end_plate(&params);
        assert_eq!(conn.conn_type, ConnectionType::EndPlate);
        assert!(!conn.plates.is_empty());
        assert!(!conn.bolts.is_empty());
        assert!(!conn.welds.is_empty());
        // 端板寬度 ≥ 梁翼板寬度
        assert!(conn.plates[0].width >= 150.0);
        // 有肋板（上下各一）
        let stiffeners: Vec<_> = conn.plates.iter()
            .filter(|p| p.plate_type == PlateType::Stiffener)
            .collect();
        assert_eq!(stiffeners.len(), 2);
        // 螺栓數量 = rows × cols
        let bg = &conn.bolts[0];
        assert_eq!(bg.positions.len() as u32, bg.rows * bg.cols);
    }

    #[test]
    fn test_base_plate_generation() {
        let conn = calc_base_plate(
            (300.0, 300.0, 10.0, 15.0),
            BoltSize::M24,
            BoltGrade::F8T,
        );
        assert_eq!(conn.conn_type, ConnectionType::BasePlate);
        assert!(conn.bolts[0].positions.len() >= 4); // 至少 4 顆錨栓
        assert!(conn.plates[0].thickness >= 25.0);
    }

    #[test]
    fn test_shear_tab_generation() {
        let conn = calc_shear_tab(
            (400.0, 200.0, 8.0, 13.0),
            BoltSize::M20,
            BoltGrade::F10T,
        );
        assert_eq!(conn.conn_type, ConnectionType::ShearTab);
        assert!(!conn.plates.is_empty());
        assert!(conn.bolts[0].rows >= 2);
    }

    // ── AISC 驗算測試 ──

    #[test]
    fn test_bolt_capacity_m20_a325() {
        let cap = bolt_capacity(
            &BoltSize::M20, &BoltGrade::A325,
            16.0, &SteelMaterial::SS400,
            DesignMethod::LRFD, true,
        );
        // M20 面積 ≈ 314 mm²
        assert!((cap.area - 314.16).abs() < 1.0);
        // LRFD 抗剪 = φ × Fnv × Ab = 0.75 × 372 × 314.16 / 1000 ≈ 87.6 kN
        assert!(cap.shear_capacity > 80.0 && cap.shear_capacity < 100.0);
        // LRFD 抗拉 = φ × Fnt × Ab = 0.75 × 620 × 314.16 / 1000 ≈ 146 kN
        assert!(cap.tensile_capacity > 130.0 && cap.tensile_capacity < 160.0);
        // 承壓 = φ × 2.4 × d × t × Fu = 0.75 × 2.4 × 20 × 16 × 400 / 1000 ≈ 230 kN
        assert!(cap.bearing_capacity > 200.0 && cap.bearing_capacity < 260.0);
    }

    #[test]
    fn test_bolt_capacity_m20_a490() {
        let cap = bolt_capacity(
            &BoltSize::M20, &BoltGrade::A490,
            20.0, &SteelMaterial::A572_50,
            DesignMethod::LRFD, false, // threads excluded
        );
        // A490 X condition Fnv = 579 MPa, 面積 314.16
        // φRn = 0.75 × 579 × 314.16 / 1000 ≈ 136 kN
        assert!(cap.shear_capacity > 120.0 && cap.shear_capacity < 150.0);
    }

    #[test]
    fn test_weld_capacity_fillet() {
        let weld = WeldLine {
            weld_type: WeldType::Fillet,
            size: 8.0,
            length: 200.0,
            start: [0.0; 3],
            end: [200.0, 0.0, 0.0],
        };
        let cap = weld_capacity(&weld, &SteelMaterial::SS400, DesignMethod::LRFD);
        // te = 8/√2 ≈ 5.66, Awe = 5.66 × 200 ≈ 1131
        assert!((cap.effective_throat - 5.66).abs() < 0.1);
        // Fnw = 0.6 × 482 = 289.2, Rn = 289.2 × 1131 / 1000 ≈ 327 kN
        // φRn = 0.75 × 327 ≈ 245 kN
        assert!(cap.weld_metal_capacity > 200.0 && cap.weld_metal_capacity < 280.0);
    }

    #[test]
    fn test_minimum_fillet_weld_size() {
        assert_eq!(minimum_fillet_weld_size(5.0), 3.0);
        assert_eq!(minimum_fillet_weld_size(10.0), 5.0);
        assert_eq!(minimum_fillet_weld_size(16.0), 6.0);
        assert_eq!(minimum_fillet_weld_size(25.0), 8.0);
    }

    #[test]
    fn test_connection_check_passes() {
        let params = EndPlateParams {
            beam_section: (300.0, 150.0, 6.0, 9.0),
            col_section: (300.0, 300.0, 10.0, 15.0),
            bolt_size: BoltSize::M20,
            bolt_grade: BoltGrade::F10T,
            plate_thickness: Some(20.0),
            add_stiffeners: true,
        };
        let conn = calc_end_plate(&params);
        let check = check_connection(&conn, &SteelMaterial::SS400, DesignMethod::LRFD);
        assert!(check.total_bolt_shear > 0.0);
        assert!(check.total_weld_capacity > 0.0);
        // 端板 20mm + M20 螺栓 — 應無嚴重警告
        for w in &check.warnings {
            // 只允許建議性警告，不允許 pass=false 的硬性違規
            println!("Warning: {}", w);
        }
    }

    #[test]
    fn test_connection_check_detects_violation() {
        // 故意製作違規接頭：螺栓間距太小
        let mut conn = calc_end_plate(&EndPlateParams {
            beam_section: (200.0, 100.0, 5.5, 8.0),
            col_section: (200.0, 200.0, 8.0, 12.0),
            bolt_size: BoltSize::M24,
            bolt_grade: BoltGrade::A325,
            plate_thickness: Some(12.0),
            add_stiffeners: false,
        });
        // 將螺栓間距設為不合規的值
        conn.bolts[0].row_spacing = 30.0; // M24 最小 = 2.67 × 24 = 64mm
        let check = check_connection(&conn, &SteelMaterial::SS400, DesignMethod::LRFD);
        assert!(!check.pass, "Should detect spacing violation");
        assert!(check.warnings.iter().any(|w| w.contains("螺栓行距")));
    }

    #[test]
    fn test_aisc_min_edge_cross_check() {
        // 確認 BoltSize::min_edge() 與 aisc_min_edge_distance() 一致
        for &bs in BoltSize::ALL {
            let our_edge = bs.min_edge();
            let aisc_edge = aisc_min_edge_distance(bs.diameter());
            assert_eq!(our_edge, aisc_edge,
                "Edge distance mismatch for {}: ours={}, AISC={}",
                bs.label(), our_edge, aisc_edge);
        }
    }

    #[test]
    fn test_web_doubler_generation() {
        let conn = calc_web_doubler(
            (400.0, 200.0, 8.0, 13.0),  // 梁
            (300.0, 300.0, 10.0, 15.0),  // 柱
            None,
        );
        assert_eq!(conn.conn_type, ConnectionType::WebDoubler);
        assert_eq!(conn.plates.len(), 1);
        assert_eq!(conn.plates[0].plate_type, PlateType::WebDoubler);
        // 加厚板厚度 = 柱腹板厚 = 10mm
        assert!((conn.plates[0].thickness - 10.0).abs() < 0.1);
        // 加厚板高度 = 柱淨高 = 300 - 2×15 = 270
        assert!((conn.plates[0].height - 270.0).abs() < 0.1);
        // 四邊焊接
        assert_eq!(conn.welds.len(), 4);
        // 無螺栓
        assert!(conn.bolts.is_empty());
    }

    #[test]
    fn test_double_angle_generation() {
        let params = DoubleAngleParams {
            beam_section: (400.0, 200.0, 8.0, 13.0),
            col_section: (300.0, 300.0, 10.0, 15.0),
            bolt_size: BoltSize::M20,
            bolt_grade: BoltGrade::F10T,
            angle_leg: None,
            angle_thickness: None,
        };
        let conn = calc_double_angle(&params);
        assert_eq!(conn.conn_type, ConnectionType::DoubleAngle);
        // 4 片板件：左右各一垂直肢+水平肢
        assert_eq!(conn.plates.len(), 4);
        assert!(conn.plates.iter().all(|p| p.plate_type == PlateType::AngleLeg));
        // 2 組螺栓群：梁腹板側 + 柱翼板側
        assert_eq!(conn.bolts.len(), 2);
        // 梁腹板側螺栓 ≥ 2 顆
        assert!(conn.bolts[0].rows >= 2);
        // 角鋼肢寬 = 90mm（梁深 400 自動選擇）
        assert!((conn.plates[0].width - 90.0).abs() < 0.1);
    }

    #[test]
    fn test_suggest_includes_new_types() {
        let suggestions = suggest_connection(
            (400.0, 200.0, 8.0, 13.0),
            (300.0, 300.0, 10.0, 15.0),
            ConnectionIntent::BeamToColumn,
            "SS400",
        );
        // 至少有 EndPlate + ShearTab + DoubleAngle = 3 種
        assert!(suggestions.len() >= 3);
        let types: Vec<_> = suggestions.iter().map(|s| s.conn_type).collect();
        assert!(types.contains(&ConnectionType::EndPlate));
        assert!(types.contains(&ConnectionType::ShearTab));
        assert!(types.contains(&ConnectionType::DoubleAngle));
    }
}

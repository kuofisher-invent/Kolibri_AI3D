//! EditorState — 工具/編輯狀態層（當前工具、選取、snap、inference）
//! 從 app.rs 拆分出來，減少 god module 耦合

use eframe::egui;
use serde::Serialize;

// ── Debug Trace Record ──────────────────────────────────────────────────────

/// 單次採樣記錄：物件位置 + 旋轉 + 工具狀態 + 滑鼠座標
#[derive(Debug, Clone, Serialize)]
pub(crate) struct DebugTraceRecord {
    /// 自 debug 啟動後的毫秒數
    pub(crate) t_ms: u64,
    /// 事件類型：None=定時採樣, Some=事件觸發
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) event: Option<String>,
    /// 當前工具名稱
    pub(crate) tool: String,
    /// DrawState 名稱（Idle/MoveFrom/RotateRef 等）
    pub(crate) draw_state: String,
    /// 滑鼠螢幕座標 [x, y]
    pub(crate) mouse_screen: [f32; 2],
    /// 滑鼠 ground 座標 [x, y, z]（若有）
    pub(crate) mouse_ground: Option<[f32; 3]>,
    /// 選取的面（PushPull 時）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) selected_face: Option<String>,
    /// 選取的物件 ID 列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) selected_ids: Option<Vec<String>>,
    /// hover 的物件 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) hovered_id: Option<String>,
    /// 旋轉盤中心（世界座標）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rotate_center: Option<[f32; 3]>,
    /// 旋轉軸 (0=X, 1=Y, 2=Z)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rotate_axis: Option<u8>,
    /// 被操作物件的快照
    pub(crate) objects: Vec<DebugTraceObject>,
}

/// 物件位置/旋轉快照
#[derive(Debug, Clone, Serialize)]
pub(crate) struct DebugTraceObject {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) position: [f32; 3],
    pub(crate) rotation_xyz: [f32; 3],
    /// 形狀尺寸 [width, height, depth]（Box 才有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dimensions: Option<[f32; 3]>,
    /// 世界空間 8 角點（position + self-rotation 後的實際頂點）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) world_corners: Option<Vec<[f32; 3]>>,
}

// ─── Tool ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tool {
    // ── Select & Transform ──
    Select,
    Move,
    Rotate,
    Scale,
    // ── Draw 2D ──
    Line,
    Arc,
    Arc3Point,  // 三點圓弧
    Pie,        // 扇形/餅圖
    Rectangle,
    Circle,
    // ── Draw 3D ──
    CreateBox,
    CreateCylinder,
    CreateSphere,
    // ── Modify ──
    PushPull,
    Offset,
    FollowMe,
    // ── Measure & Annotate ──
    TapeMeasure,
    Dimension,    // 標註工具（兩點標註距離）
    Text,         // 文字標註
    PaintBucket,
    // ── Camera ──
    Orbit,
    Pan,
    ZoomExtents,
    // ── Organize ──
    Group,
    Component,
    // ── Edit ──
    Eraser,
    // ── Camera ──
    Walk,         // 第一人稱行走
    LookAround,   // 環顧
    // ── Section ──
    SectionPlane,  // 互動式剖面平面
    // ── Architecture ──
    Wall,       // 參數化牆（兩點 + 厚度 + 高度）
    Slab,       // 參數化板（矩形 + 厚度）
    // ── Steel Mode Tools（feature gate）──
    #[cfg(feature = "steel")]
    SteelGrid,
    #[cfg(feature = "steel")]
    SteelColumn,
    #[cfg(feature = "steel")]
    SteelBeam,
    #[cfg(feature = "steel")]
    SteelBrace,
    #[cfg(feature = "steel")]
    SteelPlate,
    #[cfg(feature = "steel")]
    SteelConnection,
    #[cfg(feature = "steel")]
    SteelEndPlate,     // 端板接頭（梁-柱剛接）
    #[cfg(feature = "steel")]
    SteelShearTab,     // 腹板接頭（梁-柱鉸接）
    #[cfg(feature = "steel")]
    SteelBasePlate,    // 底板接頭（柱底+錨栓）
    #[cfg(feature = "steel")]
    SteelBolt,         // 螺栓放置
    #[cfg(feature = "steel")]
    SteelWeld,         // 焊接標記
    #[cfg(feature = "steel")]
    SteelStiffener,    // 肋板
    #[cfg(feature = "steel")]
    SteelDoubler,      // 腹板加厚板（Web Doubler）
    #[cfg(feature = "steel")]
    SteelDoubleAngle,  // 雙角鋼接頭（Double Angle）
    // ── Piping（管線外掛，feature gate）──
    #[cfg(feature = "piping")]
    PipeDraw,
    #[cfg(feature = "piping")]
    PipeFitting,
    // ── Drafting（2D 出圖工具，feature gate）──
    #[cfg(feature = "drafting")]
    DraftSelect,
    #[cfg(feature = "drafting")]
    DraftLine,
    #[cfg(feature = "drafting")]
    DraftArc,
    #[cfg(feature = "drafting")]
    DraftCircle,
    #[cfg(feature = "drafting")]
    DraftRectangle,
    #[cfg(feature = "drafting")]
    DraftPolyline,
    #[cfg(feature = "drafting")]
    DraftEllipse,
    #[cfg(feature = "drafting")]
    DraftOffset,
    #[cfg(feature = "drafting")]
    DraftTrim,
    #[cfg(feature = "drafting")]
    DraftMirror,
    #[cfg(feature = "drafting")]
    DraftArray,
    #[cfg(feature = "drafting")]
    DraftMove,
    #[cfg(feature = "drafting")]
    DraftRotate,
    #[cfg(feature = "drafting")]
    DraftScale,
    #[cfg(feature = "drafting")]
    DraftDimLinear,
    #[cfg(feature = "drafting")]
    DraftDimAligned,
    #[cfg(feature = "drafting")]
    DraftDimAngle,
    #[cfg(feature = "drafting")]
    DraftDimRadius,
    #[cfg(feature = "drafting")]
    DraftDimDiameter,
    #[cfg(feature = "drafting")]
    DraftText,
    #[cfg(feature = "drafting")]
    DraftLeader,
    #[cfg(feature = "drafting")]
    DraftHatch,
    #[cfg(feature = "drafting")]
    DraftZoomAll,
    #[cfg(feature = "drafting")]
    DraftZoomWindow,
    #[cfg(feature = "drafting")]
    DraftPan,
    #[cfg(feature = "drafting")]
    DraftPrint,
    #[cfg(feature = "drafting")]
    DraftExportPdf,
    // ── Top 10 新增工具 ──
    #[cfg(feature = "drafting")]
    DraftCopy,        // 複製
    #[cfg(feature = "drafting")]
    DraftFillet,      // 圓角
    #[cfg(feature = "drafting")]
    DraftChamfer,     // 倒角
    #[cfg(feature = "drafting")]
    DraftExplode,     // 分解
    #[cfg(feature = "drafting")]
    DraftStretch,     // 拉伸
    #[cfg(feature = "drafting")]
    DraftExtend,      // 延伸
    #[cfg(feature = "drafting")]
    DraftDimContinue, // 連續標註
    #[cfg(feature = "drafting")]
    DraftDimBaseline, // 基線標註
    #[cfg(feature = "drafting")]
    DraftPolygon,     // 多邊形
    #[cfg(feature = "drafting")]
    DraftSpline,      // 雲形線
    #[cfg(feature = "drafting")]
    DraftBlock,       // 建立圖塊
    #[cfg(feature = "drafting")]
    DraftInsert,      // 插入圖塊
    #[cfg(feature = "drafting")]
    DraftPoint,       // 點
    #[cfg(feature = "drafting")]
    DraftXline,       // 建構線
    #[cfg(feature = "drafting")]
    DraftErase,       // 刪除（Erase）
    #[cfg(feature = "drafting")]
    DraftBreak,       // 打斷（Break）
    #[cfg(feature = "drafting")]
    DraftJoin,        // 接合（Join）
    #[cfg(feature = "drafting")]
    DraftRevcloud,    // 修訂雲形（Revcloud）
    #[cfg(feature = "drafting")]
    DraftTable,       // 表格（Table）
    // ── Circle/Arc sub-modes ──
    #[cfg(feature = "drafting")]
    DraftCircle2P,    // 兩點圓
    #[cfg(feature = "drafting")]
    DraftCircle3P,    // 三點圓
    #[cfg(feature = "drafting")]
    DraftArc3P,       // 三點弧
    #[cfg(feature = "drafting")]
    DraftArcSCE,      // 起點-圓心-終點弧
    #[cfg(feature = "drafting")]
    DraftLengthen,    // 加長
    #[cfg(feature = "drafting")]
    DraftAlign,       // 對齊
    #[cfg(feature = "drafting")]
    DraftRay,         // 射線
    #[cfg(feature = "drafting")]
    DraftMatchProp,   // 格式刷
    #[cfg(feature = "drafting")]
    DraftMeasureDist, // 測量距離
    #[cfg(feature = "drafting")]
    DraftMeasureArea, // 測量面積
    #[cfg(feature = "drafting")]
    DraftQuickSelect, // 快速選取
    #[cfg(feature = "drafting")]
    DraftList,        // 物件資訊
    #[cfg(feature = "drafting")]
    DraftIdPoint,     // 座標辨識
    #[cfg(feature = "drafting")]
    DraftDimOrdinate, // 座標標註
    #[cfg(feature = "drafting")]
    DraftDimArcLen,   // 弧長標註
    #[cfg(feature = "drafting")]
    DraftCenterMark,  // 中心標記
    #[cfg(feature = "drafting")]
    DraftArrayRect,   // 矩形陣列
    #[cfg(feature = "drafting")]
    DraftArrayPolar,  // 環形陣列
    #[cfg(feature = "drafting")]
    DraftArrayPath,   // 路徑陣列
    #[cfg(feature = "drafting")]
    DraftLayerProp,   // 圖層特性管理員
    #[cfg(feature = "drafting")]
    DraftLayerMatch,  // 圖層匹配
    #[cfg(feature = "drafting")]
    DraftMakeCurrent, // 置為目前圖層
}

impl Tool {
    pub(crate) fn is_implemented(self) -> bool {
        true
    }
}

// ─── Work Mode ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum WorkMode {
    Modeling,  // SketchUp-style general modeling
    #[cfg(feature = "steel")]
    Steel,     // Tekla-style structural steel
    #[cfg(feature = "piping")]
    Piping,    // 管線繪製模式
}

// ─── Interactive draw state (SketchUp style) ────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) enum DrawState {
    Idle,
    BoxBase { p1: [f32; 3] },
    BoxHeight { p1: [f32; 3], p2: [f32; 3] },
    CylBase { center: [f32; 3] },
    CylHeight { center: [f32; 3], radius: f32 },
    SphRadius { center: [f32; 3] },
    Pulling { obj_id: String, face: PullFace, original_dim: f32, skip_frames: u8 },
    LineFrom { p1: [f32; 3] },
    ArcP1 { p1: [f32; 3] },
    ArcP2 { p1: [f32; 3], p2: [f32; 3] },
    PieCenter { center: [f32; 3] },
    PieRadius { center: [f32; 3], edge1: [f32; 3] },
    RotateRef { obj_ids: Vec<String>, center: [f32; 3], rotate_axis: u8 },
    RotateAngle { obj_ids: Vec<String>, center: [f32; 3], ref_angle: f32, current_angle: f32, original_rotations: Vec<[f32; 4]>, original_positions: Vec<[f32; 3]>, rotate_axis: u8 },
    Scaling { obj_id: String, handle: ScaleHandle, original_dims: [f32; 3] },
    Offsetting { obj_id: String, face: PullFace, distance: f32 },
    FollowPath { source_id: String, path_points: Vec<[f32; 3]> },
    Measuring { start: [f32; 3] },
    PullingFreeMesh { face_id: u32 },
    /// SU-style Move click-click: 第一次點擊設定起點，第二次點擊設定終點
    MoveFrom { from: [f32; 3], obj_ids: Vec<String>, original_positions: Vec<[f32; 3]> },
    /// SU-style PushPull click-click: 點擊面後移動滑鼠，再點擊確認距離
    PullClick { obj_id: String, face: PullFace, original_dim: f32 },
    WallFrom { p1: [f32; 3] },
    SlabCorner { p1: [f32; 3] },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ScaleHandle {
    Uniform,
    AxisX,
    AxisY,
    AxisZ,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PullFace { Top, Bottom, Front, Back, Left, Right }

impl PullFace {
    pub(crate) fn as_u8(self) -> u8 {
        match self {
            PullFace::Front  => 0,
            PullFace::Back   => 1,
            PullFace::Top    => 2,
            PullFace::Bottom => 3,
            PullFace::Left   => 4,
            PullFace::Right  => 5,
        }
    }
}

// ─── Snap inference system ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SnapType {
    None,
    Grid,
    Endpoint,
    Midpoint,
    OnEdge,
    Origin,
    AxisX,
    AxisY,
    AxisZ,
    Parallel,
    Perpendicular,
    Intersection,
    OnFace,
    FaceCenter,
    Tangent,      // 切線點（弧/圓）
}

impl SnapType {
    pub(crate) fn label(&self) -> &str {
        match self {
            Self::None => "",
            Self::Grid => "格點",
            Self::Endpoint => "端點",
            Self::Midpoint => "中點",
            Self::OnEdge => "邊上",
            Self::Origin => "原點",
            Self::AxisX => "X 軸上",
            Self::AxisY => "Y 軸上",
            Self::AxisZ => "Z 軸上",
            Self::Parallel => "平行",
            Self::Perpendicular => "垂直",
            Self::Intersection => "交點",
            Self::OnFace => "面上",
            Self::FaceCenter => "面中心",
            Self::Tangent => "切線",
        }
    }
    pub(crate) fn color(&self) -> egui::Color32 {
        match self {
            Self::AxisX => egui::Color32::from_rgb(220, 60, 60),
            Self::AxisY => egui::Color32::from_rgb(60, 180, 60),
            Self::AxisZ => egui::Color32::from_rgb(60, 60, 220),
            Self::Endpoint => egui::Color32::from_rgb(60, 200, 60),
            Self::Midpoint => egui::Color32::from_rgb(60, 200, 200),
            Self::Origin => egui::Color32::from_rgb(200, 60, 60),
            Self::OnEdge => egui::Color32::from_rgb(220, 50, 50),
            Self::Parallel => egui::Color32::from_rgb(200, 60, 200),
            Self::Perpendicular => egui::Color32::from_rgb(200, 60, 200),
            Self::Intersection => egui::Color32::from_rgb(220, 220, 60),
            Self::OnFace => egui::Color32::from_rgb(60, 180, 220),
            Self::FaceCenter => egui::Color32::from_rgb(60, 180, 220),
            Self::Tangent => egui::Color32::from_rgb(200, 120, 60),  // orange
            _ => egui::Color32::from_rgb(200, 200, 200),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SnapResult {
    pub(crate) position: [f32; 3],
    pub(crate) snap_type: SnapType,
    pub(crate) from_point: Option<[f32; 3]>,
}

// ─── AI Suggestion system ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct AiSuggestion {
    pub(crate) message: String,
    pub(crate) action: SuggestionAction,
}

#[derive(Debug, Clone)]
pub(crate) enum SuggestionAction {
    AlignToEdge { obj_id: String, edge_pos: f32, axis: u8 },
    CompleteRectangle { points: Vec<[f32; 3]> },
    SnapToGrid { obj_id: String, snapped_pos: [f32; 3] },
}

// ─── Selection mode ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SelectionMode {
    Object,  // 預設：選取整個物件
    Face,    // 選取個別面
    Edge,    // 選取個別邊
}

// ─── Right panel tabs ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RightTab { Properties, Create, Scene, AiLog, Help }

// ─── Cursor Hint UI ──────────────────────────────────────────────────────────

#[derive(Default, Clone)]
pub(crate) struct CursorHint {
    pub active: bool,
    pub inference_label: String,
    pub inference_color: egui::Color32,
    pub distance_text: String,
    pub chips: Vec<(String, bool)>,
    pub ai_suggestion: Option<String>,
    pub ghost_dir: Option<([f32; 3], [f32; 3])>,
}

// ─── EditorState ─────────────────────────────────────────────────────────────

/// AISC 接頭確認對話框狀態
#[cfg(feature = "steel")]
#[derive(Clone)]
pub(crate) struct ConnectionDialogState {
    /// 選取的構件 ID
    pub member_ids: Vec<String>,
    /// 偵測到的截面
    pub beam_section: (f32, f32, f32, f32),
    pub col_section: (f32, f32, f32, f32),
    /// 接頭意圖
    pub intent: kolibri_core::steel_connection::ConnectionIntent,
    /// AISC 建議結果
    pub suggestions: Vec<kolibri_core::steel_connection::ConnectionSuggestion>,
    /// 使用者選擇的方案 index
    pub selected_idx: usize,
    /// 使用者可調整的參數
    pub bolt_size: kolibri_core::steel_connection::BoltSize,
    pub bolt_grade: kolibri_core::steel_connection::BoltGrade,
    pub plate_thickness: f32,
    pub add_stiffeners: bool,
    pub weld_size: f32,
}

pub(crate) struct EditorState {
    pub(crate) tool: Tool,
    pub(crate) draw_state: DrawState,
    pub(crate) selected_ids: Vec<String>,
    pub(crate) hovered_id: Option<String>,
    pub(crate) hovered_face: Option<(String, PullFace)>,
    pub(crate) selected_face: Option<(String, PullFace)>,
    pub(crate) rubber_band: Option<(egui::Pos2, egui::Pos2)>,
    pub(crate) shift_held: bool,
    pub(crate) ctrl_was_down: bool,
    /// Ctrl 循環設定的軸鎖定（MoveFrom 期間不被 Shift-lock cleanup 清除）
    pub(crate) axis_locked_by_ctrl: bool,
    pub(crate) mouse_ground: Option<[f32; 3]>,
    pub(crate) mouse_screen: [f32; 2],
    pub(crate) measure_input: String,
    pub(crate) drag_snapshot_taken: bool,
    pub(crate) snap_result: Option<SnapResult>,
    pub(crate) locked_axis: Option<u8>,
    pub(crate) sticky_axis: Option<u8>,
    pub(crate) last_line_dir: Option<[f32; 2]>,
    pub(crate) editing_group_id: Option<String>,
    pub(crate) editing_component_def_id: Option<String>,
    pub(crate) suggestion: Option<AiSuggestion>,
    pub(crate) cursor_dimension: Option<(f32, f32, String)>,
    pub(crate) move_origin: Option<[f32; 3]>,
    pub(crate) move_is_copy: bool,
    pub(crate) last_move_delta: Option<[f32; 3]>,
    pub(crate) last_move_was_copy: bool,
    pub(crate) pull_original_pos: Option<[f32; 3]>,
    pub(crate) pull_original_dims: Option<[f32; 3]>,
    pub(crate) last_pull_distance: f32,
    pub(crate) last_pull_click_time: std::time::Instant,
    pub(crate) last_pull_face: Option<(String, PullFace)>,
    pub(crate) last_action_name: String,
    pub(crate) inference_ctx: crate::inference::InferenceContext,
    pub(crate) inference_label: Option<(String, crate::inference::InferenceSource)>,
    pub(crate) inference_engine: crate::inference_engine::InferenceEngine,
    pub(crate) cursor_hint: CursorHint,
    pub(crate) cursor_hint_fade: Option<std::time::Instant>,
    pub(crate) prev_tool_for_hint: Tool,
    pub(crate) work_mode: WorkMode,
    pub(crate) steel_section_type: crate::tools::geometry_ops::SteelSectionType,
    pub(crate) steel_profile: String,
    pub(crate) steel_material: String,
    pub(crate) steel_height: f32,
    /// 地面標高 GL (Ground Level, mm) — 所有結構的基準面
    pub(crate) ground_level: f32,
    /// 樓層標高列表 [GL, 1FL, 2FL, RF...] (mm, 相對於 GL)
    pub(crate) floor_levels: Vec<(String, f32)>,
    /// 目前作業樓層 index
    pub(crate) active_floor: usize,
    pub(crate) collision_warning: Option<String>,
    pub(crate) editing_dim_idx: Option<usize>,
    pub(crate) editing_dim_text: String,
    /// 內部剪貼簿（Ctrl+C/V）
    pub(crate) clipboard: Vec<crate::scene::SceneObject>,
    /// 選取模式（Object/Face/Edge）
    pub(crate) selection_mode: SelectionMode,
    /// Snap 靈敏度（像素，預設 18）
    pub(crate) snap_threshold: f32,
    /// Crash recovery: 啟動時檢查是否有 autosave
    pub(crate) recovery_checked: bool,
    /// 最近使用的材質（快速切換）
    pub(crate) recent_materials: Vec<crate::scene::MaterialKind>,
    /// Property clipboard（材質/roughness/metallic）
    pub(crate) property_clipboard: Option<(crate::scene::MaterialKind, f32, f32)>,
    /// Outliner rename 狀態
    pub(crate) renaming_id: Option<String>,
    pub(crate) rename_buf: String,
    /// 建築參數
    pub(crate) wall_thickness: f32,  // mm, 預設 200
    pub(crate) wall_height: f32,     // mm, 預設 3000
    pub(crate) slab_thickness: f32,  // mm, 預設 200
    /// Command Palette 狀態
    pub(crate) command_palette_open: bool,
    pub(crate) command_palette_query: String,
    /// Gizmo hover 狀態：None / Some(0=X, 1=Y, 2=Z)
    pub(crate) gizmo_hovered_axis: Option<u8>,
    /// Gizmo drag 狀態：拖曳中鎖定的軸
    pub(crate) gizmo_drag_axis: Option<u8>,
    /// 附近的 snap 候選點（SU-style：顯示附近所有端點/中點小圓點）
    pub(crate) nearby_snaps: Vec<([f32; 3], SnapType)>,

    // ── Steel Connection（接頭）──
    #[cfg(feature = "steel")]
    pub(crate) conn_bolt_size: kolibri_core::steel_connection::BoltSize,
    #[cfg(feature = "steel")]
    pub(crate) conn_bolt_grade: kolibri_core::steel_connection::BoltGrade,
    #[cfg(feature = "steel")]
    pub(crate) conn_add_stiffeners: bool,
    #[cfg(feature = "steel")]
    pub(crate) conn_weld_type: kolibri_core::steel_connection::WeldType,
    #[cfg(feature = "steel")]
    pub(crate) conn_weld_size: f32,
    /// AISC 接頭確認對話框狀態
    #[cfg(feature = "steel")]
    pub(crate) conn_dialog: Option<ConnectionDialogState>,

    // ── Piping（管線外掛）──
    #[cfg(feature = "piping")]
    pub(crate) piping: kolibri_piping::PipingState,

    // ── Drafting（2D 出圖）──
    #[cfg(feature = "drafting")]
    pub(crate) draft_state: DraftDrawState,
    #[cfg(feature = "drafting")]
    pub(crate) draft_doc: kolibri_drafting::DraftDocument,
    #[cfg(feature = "drafting")]
    pub(crate) draft_layers: kolibri_drafting::LayerManager,
    #[cfg(feature = "drafting")]
    pub(crate) draft_selected: Vec<kolibri_drafting::DraftId>,
    #[cfg(feature = "drafting")]
    pub(crate) ribbon_tab: RibbonTab,
    #[cfg(feature = "drafting")]
    pub(crate) show_layer_manager: bool,
    // ── 特性面板狀態 ──
    #[cfg(feature = "drafting")]
    pub(crate) draft_prop_color_idx: usize,      // 0=隨圖層
    #[cfg(feature = "drafting")]
    pub(crate) draft_prop_linetype_idx: usize,    // 0=Continuous
    #[cfg(feature = "drafting")]
    pub(crate) draft_prop_lineweight_idx: usize,  // 0=隨圖層
    // ── Fillet/Chamfer 參數 ──
    #[cfg(feature = "drafting")]
    pub(crate) draft_fillet_radius: f64,   // mm, 預設 5.0
    #[cfg(feature = "drafting")]
    pub(crate) draft_chamfer_dist: f64,    // mm, 預設 5.0
    // ── Text 輸入 ──
    #[cfg(feature = "drafting")]
    pub(crate) draft_text_input: String,
    #[cfg(feature = "drafting")]
    pub(crate) draft_text_height: f64,     // mm, 預設 3.5
    #[cfg(feature = "drafting")]
    pub(crate) show_text_editor: bool,
    #[cfg(feature = "drafting")]
    pub(crate) draft_text_place: Option<[f64; 2]>,
    // ── Block ──
    #[cfg(feature = "drafting")]
    pub(crate) draft_blocks: std::collections::HashMap<String, Vec<kolibri_drafting::DraftObject>>,
    #[cfg(feature = "drafting")]
    pub(crate) draft_block_name: String,
    // ── DraftMove/Rotate/Scale base point ──
    #[cfg(feature = "drafting")]
    pub(crate) draft_transform_base: Option<[f64; 2]>,
    // ── Fillet/Chamfer 第一條線記憶 ──
    #[cfg(feature = "drafting")]
    pub(crate) draft_fillet_first: Option<kolibri_drafting::DraftId>,
    // ── ORTHO / POLAR / DYN ──
    #[cfg(feature = "drafting")]
    pub(crate) draft_ortho: bool,     // F8 正交模式
    #[cfg(feature = "drafting")]
    pub(crate) draft_polar: bool,     // F10 極座標追蹤
    #[cfg(feature = "drafting")]
    pub(crate) draft_dyn_input: bool, // F12 動態輸入
    #[cfg(feature = "drafting")]
    pub(crate) draft_osnap: bool,     // F3 物件鎖點
    // ── 圖紙分頁 ──
    #[cfg(feature = "drafting")]
    pub(crate) draft_sheets: Vec<(String, kolibri_drafting::DraftDocument)>, // (name, doc)
    #[cfg(feature = "drafting")]
    pub(crate) draft_active_sheet: usize,  // 目前選中的 sheet index
    // ── 指令別名輸入緩衝 ──
    #[cfg(feature = "drafting")]
    pub(crate) draft_cmd_buf: String,
    #[cfg(feature = "drafting")]
    pub(crate) draft_cmd_time: std::time::Instant,
    /// AutoCAD/ZWCAD 風格數值輸入緩衝（畫線時輸入長度/座標）
    #[cfg(feature = "drafting")]
    pub(crate) draft_num_input: String,
    // ── 2D 畫布 Pan/Zoom ──
    #[cfg(feature = "drafting")]
    pub(crate) draft_zoom: f32,           // 像素/mm（預設 2.0）
    #[cfg(feature = "drafting")]
    pub(crate) draft_offset: egui::Vec2,  // 畫布偏移（像素）
    #[cfg(feature = "drafting")]
    pub(crate) draft_pan_drag: Option<egui::Pos2>,  // 中鍵拖曳起始點
    // ── 匯入後自動 Zoom All ──
    #[cfg(feature = "drafting")]
    pub(crate) draft_needs_zoom_all: bool,
    #[cfg(feature = "drafting")]
    pub(crate) draft_zoom_all_delay: u8, // 延遲幀數
    // ── Grip editing ──
    #[cfg(feature = "drafting")]
    pub(crate) grip_edit_mode: GripEditMode,  // Space 循環模式
    #[cfg(feature = "drafting")]
    pub(crate) grip_hot_idx: Option<usize>,   // 被拖曳的 grip 點 index
    #[cfg(feature = "drafting")]
    pub(crate) grip_base_point: Option<[f64; 2]>,  // grip editing 基準點

    // ── Debug Trace（運動軌跡記錄）──
    /// Debug 模式開關（Console 面板按鈕控制）
    pub(crate) debug_trace_active: bool,
    /// 採樣間隔（ms），10~1500，步進 10
    pub(crate) debug_trace_interval_ms: u32,
    /// 上次採樣時間
    pub(crate) debug_trace_last_sample: std::time::Instant,
    /// 當前 session 的 trace 記錄（記憶體中暫存）
    pub(crate) debug_trace_records: Vec<DebugTraceRecord>,
    /// 當前 session 的輸出檔路徑
    pub(crate) debug_trace_path: Option<String>,
    /// 上次記錄的 fingerprint（差異偵測用）
    /// (tool, draw_state, scene_version, selected_count, mouse_ground_quantized)
    pub(crate) debug_trace_last_fingerprint: (String, String, u64, usize, [i32; 3]),
    /// Trace 啟動時間（用於計算 t_ms）
    pub(crate) debug_trace_start: std::time::Instant,
}

// ─── Drafting draw state ────────────────────────────────────────────────────

#[cfg(feature = "drafting")]
#[derive(Debug, Clone)]
pub(crate) enum DraftDrawState {
    Idle,
    LineFrom { p1: [f64; 2] },
    ArcCenter { center: [f64; 2] },
    ArcRadius { center: [f64; 2], radius: f64 },
    CircleCenter { center: [f64; 2] },
    RectFrom { p1: [f64; 2] },
    PolylinePoints { points: Vec<[f64; 2]> },
    DimP1 { p1: [f64; 2] },
    /// 標註 3-click：p1, p2 已確定，等使用者拖曳決定 offset 位置
    DimP2 { p1: [f64; 2], p2: [f64; 2] },
    /// 角度標註：中心 + 第一端點已確定
    AngleP1 { center: [f64; 2], p1: [f64; 2] },
    TextPlace,
    LeaderPoints { points: Vec<[f64; 2]> },
}

#[cfg(feature = "drafting")]
impl Default for DraftDrawState {
    fn default() -> Self { Self::Idle }
}

// ─── Grip edit mode（Space 循環）──────────────────────────────────────────────

#[cfg(feature = "drafting")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum GripEditMode {
    Stretch,    // 預設：拉伸 grip 點
    Move,       // 移動選取物件
    Rotate,     // 旋轉
    Scale,      // 比例
    Mirror,     // 鏡射
}

#[cfg(feature = "drafting")]
impl GripEditMode {
    /// Space 循環到下一個模式
    pub fn next(self) -> Self {
        match self {
            Self::Stretch => Self::Move,
            Self::Move => Self::Rotate,
            Self::Rotate => Self::Scale,
            Self::Scale => Self::Mirror,
            Self::Mirror => Self::Stretch,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Stretch => "** 拉伸 **",
            Self::Move => "** 移動 **",
            Self::Rotate => "** 旋轉 **",
            Self::Scale => "** 比例 **",
            Self::Mirror => "** 鏡射 **",
        }
    }
}

// ─── Ribbon tab ─────────────────────────────────────────────────────────────

#[cfg(feature = "drafting")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RibbonTab {
    Home,       // 常用
    Insert,     // 插入
    Annotate,   // 標註
    View,       // 檢視
    Manage,     // 管理
    Output,     // 輸出
}

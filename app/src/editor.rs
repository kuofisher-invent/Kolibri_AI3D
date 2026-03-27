//! EditorState — 工具/編輯狀態層（當前工具、選取、snap、inference）
//! 從 app.rs 拆分出來，減少 god module 耦合

use eframe::egui;

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
    // ── Architecture ──
    Wall,       // 參數化牆（兩點 + 厚度 + 高度）
    Slab,       // 參數化板（矩形 + 厚度）
    // ── Steel Mode Tools ──
    SteelGrid,
    SteelColumn,
    SteelBeam,
    SteelBrace,
    SteelPlate,
    SteelConnection,
}

impl Tool {
    pub(crate) fn is_implemented(self) -> bool {
        match self {
            Tool::SteelGrid | Tool::SteelColumn | Tool::SteelBeam | Tool::SteelBrace |
            Tool::SteelPlate | Tool::SteelConnection => true,
            _ => true,
        }
    }
}

// ─── Work Mode ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum WorkMode {
    Modeling,  // SketchUp-style general modeling
    Steel,     // Tekla-style structural steel
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
    Pulling { obj_id: String, face: PullFace, original_dim: f32 },
    LineFrom { p1: [f32; 3] },
    ArcP1 { p1: [f32; 3] },
    ArcP2 { p1: [f32; 3], p2: [f32; 3] },
    PieCenter { center: [f32; 3] },
    PieRadius { center: [f32; 3], edge1: [f32; 3] },
    RotateRef { obj_ids: Vec<String>, center: [f32; 3] },
    RotateAngle { obj_ids: Vec<String>, center: [f32; 3], ref_angle: f32, current_angle: f32, original_rotations: Vec<f32> },
    Scaling { obj_id: String, handle: ScaleHandle, original_dims: [f32; 3] },
    Offsetting { obj_id: String, face: PullFace, distance: f32 },
    FollowPath { source_id: String, path_points: Vec<[f32; 3]> },
    Measuring { start: [f32; 3] },
    PullingFreeMesh { face_id: u32 },
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
pub(crate) enum RightTab { Properties, Create, Scene, AiLog }

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
    pub(crate) mouse_ground: Option<[f32; 3]>,
    pub(crate) mouse_screen: [f32; 2],
    pub(crate) measure_input: String,
    pub(crate) drag_snapshot_taken: bool,
    pub(crate) snap_result: Option<SnapResult>,
    pub(crate) locked_axis: Option<u8>,
    pub(crate) sticky_axis: Option<u8>,
    pub(crate) last_line_dir: Option<[f32; 2]>,
    pub(crate) editing_group_id: Option<String>,
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
    pub(crate) steel_profile: String,
    pub(crate) steel_material: String,
    pub(crate) steel_height: f32,
    pub(crate) collision_warning: Option<String>,
    pub(crate) editing_dim_idx: Option<usize>,
    pub(crate) editing_dim_text: String,
    /// 內部剪貼簿（Ctrl+C/V）
    pub(crate) clipboard: Vec<crate::scene::SceneObject>,
    /// 選取模式（Object/Face/Edge）
    pub(crate) selection_mode: SelectionMode,
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
}

//! ViewerState — 視圖狀態層（相機、渲染模式、顯示設定）
//! 從 app.rs 拆分出來，減少 god module 耦合

// ─── Render mode ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum RenderMode {
    Shaded,
    Wireframe,
    XRay,
    HiddenLine,
    Monochrome,
    Sketch,
}

impl RenderMode {
    pub(crate) fn as_u32(self) -> u32 {
        match self {
            Self::Shaded => 0,
            Self::Wireframe => 1,
            Self::XRay => 2,
            Self::HiddenLine => 3,
            Self::Monochrome => 4,
            Self::Sketch => 5,
        }
    }
}

// ─── ViewerState ─────────────────────────────────────────────────────────────

pub(crate) struct ViewerState {
    pub(crate) camera: crate::camera::OrbitCamera,
    pub(crate) render_mode: RenderMode,
    pub(crate) edge_thickness: f32,
    pub(crate) show_colors: bool,
    pub(crate) sky_color: [f32; 3],
    pub(crate) ground_color: [f32; 3],
    pub(crate) use_ortho: bool,
    pub(crate) saved_cameras: Vec<(String, crate::camera::OrbitCamera)>,
    pub(crate) viewport_size: [f32; 2],
    pub(crate) hidden_tags: std::collections::HashSet<String>,
    pub(crate) show_help: bool,
    pub(crate) show_console: bool,
    pub(crate) console_log: Vec<(String, String, std::time::Instant)>,
    pub(crate) layout_mode: bool,
    pub(crate) layout: crate::layout::Layout,
    pub(crate) show_grid: bool,
    pub(crate) grid_spacing: f32,  // mm, default 1000.0 (1m)
}

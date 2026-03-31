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
    pub(crate) show_axes: bool,
    pub(crate) show_toolbar: bool,
    pub(crate) show_right_panel: bool,
    pub(crate) grid_spacing: f32,  // mm, default 1000.0 (1m)
    pub(crate) dark_mode: bool,
    /// 語言：0=繁體中文, 1=English
    pub(crate) language: u8,
    /// 樓層管理
    pub(crate) current_floor: i32,   // 0=GF, 1=1F, 2=2F, -1=B1
    pub(crate) floor_height: f32,    // mm, 預設 3000
    /// 工作平面：0=Ground(Y=0), 1=XY(front), 2=YZ(side)
    pub(crate) work_plane: u8,
    /// 工作平面高度偏移 (mm)
    pub(crate) work_plane_offset: f32,
    /// 除錯：顯示匯入 mesh 的頂點編號
    pub(crate) show_vertex_ids: bool,

    // ── Section Plane ──
    /// 剖面平面：啟用時裁切渲染
    pub(crate) section_plane_enabled: bool,
    /// 剖面軸：0=X, 1=Y, 2=Z
    pub(crate) section_plane_axis: u8,
    /// 剖面偏移 (mm)
    pub(crate) section_plane_offset: f32,
    /// 剖面方向翻轉
    pub(crate) section_plane_flip: bool,

    // ── Camera animation ──
    /// 動畫起始相機快照
    pub(crate) camera_anim_from: Option<crate::camera::OrbitCamera>,
    /// 動畫目標相機
    pub(crate) camera_anim_to: Option<crate::camera::OrbitCamera>,
    /// 動畫開始時刻
    pub(crate) camera_anim_start: Option<std::time::Instant>,
    /// 動畫時長（秒），預設 0.3
    pub(crate) camera_anim_duration: f32,
}

impl ViewerState {
    /// 啟動平滑相機動畫：拍攝目前相機快照，建立目標，開始計時
    pub(crate) fn animate_camera_to(&mut self, mut target_fn: impl FnMut(&mut crate::camera::OrbitCamera)) {
        let from = self.camera.clone();
        let mut to = from.clone();
        target_fn(&mut to);
        // 如果目標和目前一樣就不動畫
        if (from.yaw - to.yaw).abs() < 1e-5
            && (from.pitch - to.pitch).abs() < 1e-5
            && (from.distance - to.distance).abs() < 1e-2
            && (from.target - to.target).length() < 1e-2
        {
            return;
        }
        self.camera_anim_from = Some(from);
        self.camera_anim_to = Some(to);
        self.camera_anim_start = Some(std::time::Instant::now());
    }

    /// 每幀呼叫：推進相機動畫，回傳 true 表示動畫仍在進行中（需 repaint）
    pub(crate) fn tick_camera_anim(&mut self) -> bool {
        let (Some(from), Some(to), Some(start)) =
            (&self.camera_anim_from, &self.camera_anim_to, self.camera_anim_start)
        else {
            return false;
        };
        let elapsed = start.elapsed().as_secs_f32();
        let dur = self.camera_anim_duration;
        if elapsed >= dur {
            // 動畫結束，確保精確到達目標
            self.camera = to.clone();
            self.camera_anim_from = None;
            self.camera_anim_to = None;
            self.camera_anim_start = None;
            return false;
        }
        // ease-out cubic: 1 - (1-t)^3
        let t = elapsed / dur;
        let t_ease = 1.0 - (1.0 - t).powi(3);
        self.camera = crate::camera::OrbitCamera::lerp(from, to, t_ease);
        true
    }
}

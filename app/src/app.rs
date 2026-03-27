use std::sync::Arc;
use eframe::{egui, wgpu};
use eframe::epaint::mutex::RwLock;

use crate::camera::{self, OrbitCamera};
use crate::renderer::ViewportRenderer;
use crate::scene::{MaterialKind, Scene, Shape};

// ── Re-export types from editor.rs / viewer.rs / overlay.rs ──────────────────
// 保持 `use crate::app::Tool` 等既有 import 路徑不變
pub(crate) use crate::editor::{
    Tool, WorkMode, DrawState, ScaleHandle, PullFace, SnapType, SnapResult,
    AiSuggestion, SuggestionAction, RightTab, CursorHint, EditorState, SelectionMode,
};
pub(crate) use crate::viewer::{RenderMode, ViewerState};
pub(crate) use crate::overlay::{
    ArcInfo, compute_arc, compute_arc_info, draw_dashed_line,
};

// ─── App ─────────────────────────────────────────────────────────────────────

pub struct KolibriApp {
    // GPU resources
    pub(crate) device: Arc<wgpu::Device>,
    pub(crate) queue: Arc<wgpu::Queue>,
    pub(crate) egui_renderer: Arc<RwLock<eframe::egui_wgpu::Renderer>>,
    pub(crate) viewport: ViewportRenderer,

    // ── Three-layer stores (Pascal Editor style) ──
    pub(crate) scene: Scene,           // SceneStore: 場景資料層
    pub(crate) viewer: ViewerState,    // ViewerStore: 視圖狀態層
    pub(crate) editor: EditorState,    // EditorStore: 工具/編輯狀態層

    // ── App-level state (不屬於任何 store) ──
    pub(crate) right_tab: RightTab,
    pub(crate) create_mat: MaterialKind,
    pub(crate) obj_counter: usize,

    // File management
    pub(crate) current_file: Option<String>,
    pub(crate) file_message: Option<(String, std::time::Instant)>,
    pub(crate) toasts: Vec<(String, std::time::Instant)>,
    pub(crate) recent_files: Vec<String>,

    // Auto-save
    pub(crate) last_auto_save: std::time::Instant,
    pub(crate) auto_save_version: u64,
    pub(crate) last_saved_version: u64,
    pub(crate) pending_action: Option<crate::menu::MenuAction>,

    // AI Audit Log
    pub(crate) ai_log: crate::ai_log::AiLog,
    pub(crate) current_actor: crate::ai_log::ActorId,
    pub(crate) mcp_bridge: Option<crate::mcp_server::McpBridge>,
    pub(crate) mcp_http_running: bool,
    pub(crate) mcp_http_port: u16,

    // Dimension annotations (tape measure)
    pub(crate) dimensions: Vec<crate::dimensions::Dimension>,
    pub(crate) dim_style: crate::dimensions::DimensionStyle,

    // Custom material picker UI
    pub(crate) show_custom_color_picker: bool,
    pub(crate) custom_color: [f32; 4],
    pub(crate) mat_search: String,
    pub(crate) mat_category_idx: usize,

    // CAD import
    pub(crate) pending_ir: Option<crate::cad_import::ir::DrawingIR>,
    pub(crate) import_review: Option<crate::import_review::ImportReview>,
    pub(crate) pending_unified_ir: Option<crate::import::unified_ir::UnifiedIR>,

    // Texture manager
    pub(crate) texture_manager: crate::texture_manager::TextureManager,

    // Spatial index for fast pick()
    pub(crate) spatial_index: Option<rstar::RTree<SpatialEntry>>,
    pub(crate) spatial_index_version: u64,
}

/// rstar entry: AABB + object ID
#[derive(Debug, Clone)]
pub(crate) struct SpatialEntry {
    pub id: String,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl rstar::RTreeObject for SpatialEntry {
    type Envelope = rstar::AABB<[f32; 3]>;
    fn envelope(&self) -> Self::Envelope {
        rstar::AABB::from_corners(self.min, self.max)
    }
}

impl rstar::PointDistance for SpatialEntry {
    fn distance_2(&self, point: &[f32; 3]) -> f32 {
        let dx = (point[0] - self.min[0].max(point[0].min(self.max[0]))).powi(2);
        let dy = (point[1] - self.min[1].max(point[1].min(self.max[1]))).powi(2);
        let dz = (point[2] - self.min[2].max(point[2].min(self.max[2]))).powi(2);
        dx + dy + dz
    }
}

impl KolibriApp {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        setup_cjk_fonts(&cc.egui_ctx);

        // ── Figma-style light glassmorphism theme ──
        {
            let ctx = &cc.egui_ctx;
            let mut style = (*ctx.style()).clone();

            // Colors
            let _bg = egui::Color32::from_rgb(245, 246, 250);        // #f5f6fa
            let panel = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220); // rgba(255,255,255,0.86)
            let panel_strong = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 240);
            let border = egui::Color32::from_rgb(229, 231, 239);     // #e5e7ef
            let text = egui::Color32::from_rgb(31, 36, 48);          // #1f2430
            let muted = egui::Color32::from_rgb(110, 118, 135);      // #6e7687
            let brand = egui::Color32::from_rgb(76, 139, 245);       // #4c8bf5
            let brand_soft = egui::Color32::from_rgba_unmultiplied(76, 139, 245, 36);

            style.visuals.dark_mode = false;
            style.visuals.panel_fill = panel;
            style.visuals.window_fill = panel_strong;
            style.visuals.extreme_bg_color = egui::Color32::WHITE;
            style.visuals.faint_bg_color = egui::Color32::from_rgb(248, 249, 252);

            // Rounding (18px panels, 12px buttons)
            style.visuals.window_rounding = egui::Rounding::same(18.0);
            style.visuals.menu_rounding = egui::Rounding::same(12.0);
            style.visuals.widgets.noninteractive.rounding = egui::Rounding::same(8.0);
            style.visuals.widgets.inactive.rounding = egui::Rounding::same(12.0);
            style.visuals.widgets.hovered.rounding = egui::Rounding::same(12.0);
            style.visuals.widgets.active.rounding = egui::Rounding::same(12.0);

            // Widget colors
            style.visuals.widgets.noninteractive.bg_fill = panel;
            style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, muted);
            style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5, border);

            style.visuals.widgets.inactive.bg_fill = panel_strong;
            style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, text);
            style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, border);

            style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(240, 242, 248);
            style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, text);
            style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, brand);

            style.visuals.widgets.active.bg_fill = brand_soft;
            style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, brand);
            style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, brand);

            // Open (dropdown open etc)
            style.visuals.widgets.open.bg_fill = egui::Color32::from_rgb(240, 242, 248);
            style.visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, text);
            style.visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, brand);

            style.visuals.selection.bg_fill = brand_soft;
            style.visuals.selection.stroke = egui::Stroke::new(1.0, brand);

            style.visuals.window_shadow = egui::epaint::Shadow {
                offset: egui::vec2(0.0, 4.0),
                blur: 15.0,
                spread: 0.0,
                color: egui::Color32::from_rgba_unmultiplied(21, 28, 45, 20),
            };
            style.visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180));

            // Hyperlinks
            style.visuals.hyperlink_color = brand;

            // Spacing
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.button_padding = egui::vec2(10.0, 6.0);
            style.spacing.window_margin = egui::Margin::same(8.0);
            style.spacing.indent = 16.0;

            // Scrollbar
            style.spacing.scroll = egui::style::ScrollStyle {
                bar_width: 6.0,
                ..Default::default()
            };

            // Remove dark separator/resize lines between panels
            style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
            style.visuals.window_stroke = egui::Stroke::new(0.5, border);

            ctx.set_style(style);
        }

        let rs = cc.wgpu_render_state.as_ref().expect("需要 wgpu 後端");

        Self {
            device: rs.device.clone(),
            queue: rs.queue.clone(),
            egui_renderer: rs.renderer.clone(),
            viewport: ViewportRenderer::new(&rs.device),

            scene: Scene::default(),

            viewer: ViewerState {
                camera: OrbitCamera::default(),
                render_mode: RenderMode::Shaded,
                edge_thickness: 2.0,
                show_colors: true,
                sky_color: [0.53, 0.72, 0.9],
                ground_color: [0.65, 0.63, 0.60],
                use_ortho: false,
                saved_cameras: Vec::new(),
                viewport_size: [800.0, 600.0],
                hidden_tags: std::collections::HashSet::new(),
                show_help: false,
                show_console: true,  // 預設開啟 Console 即時監控
                console_log: Vec::new(),
                layout_mode: false,
                layout: Default::default(),
                show_grid: true,
                grid_spacing: 1000.0,
                dark_mode: false,
                current_floor: 0,
                floor_height: 3000.0,
                work_plane: 0,
                work_plane_offset: 0.0,
            },

            editor: EditorState {
                tool: Tool::Select,
                draw_state: DrawState::Idle,
                selected_ids: Vec::new(),
                hovered_id: None,
                hovered_face: None,
                selected_face: None,
                rubber_band: None,
                shift_held: false,
                ctrl_was_down: false,
                mouse_ground: None,
                mouse_screen: [0.0; 2],
                measure_input: String::new(),
                drag_snapshot_taken: false,
                snap_result: None,
                locked_axis: None,
                sticky_axis: None,
                last_line_dir: None,
                editing_group_id: None,
                suggestion: None,
                cursor_dimension: None,
                move_origin: None,
                move_is_copy: false,
                last_move_delta: None,
                last_move_was_copy: false,
                pull_original_pos: None,
                pull_original_dims: None,
                last_pull_distance: 0.0,
                last_pull_click_time: std::time::Instant::now(),
                last_pull_face: None,
                last_action_name: String::new(),
                inference_ctx: crate::inference::InferenceContext::default(),
                inference_label: None,
                inference_engine: crate::inference_engine::InferenceEngine::new(),
                cursor_hint: CursorHint::default(),
                cursor_hint_fade: None,
                prev_tool_for_hint: Tool::Select,
                work_mode: WorkMode::Modeling,
                steel_profile: "H300x150x6x9".into(),
                steel_material: "SS400".into(),
                steel_height: 4200.0,
                collision_warning: None,
                editing_dim_idx: None,
                editing_dim_text: String::new(),
                clipboard: Vec::new(),
                selection_mode: SelectionMode::Object,
                snap_threshold: 18.0,
                property_clipboard: None,
                recovery_checked: false,
                renaming_id: None,
                rename_buf: String::new(),
                wall_thickness: 200.0,
                wall_height: 3000.0,
                slab_thickness: 200.0,
                command_palette_open: false,
                command_palette_query: String::new(),
                gizmo_hovered_axis: None,
                gizmo_drag_axis: None,
            },

            right_tab: RightTab::Properties,
            create_mat: MaterialKind::Concrete,
            obj_counter: 0,
            current_file: None,
            file_message: None,
            toasts: Vec::new(),
            recent_files: Vec::new(),
            last_auto_save: std::time::Instant::now(),
            auto_save_version: 0,
            last_saved_version: 0,
            pending_action: None,
            ai_log: crate::ai_log::AiLog::new(),
            current_actor: crate::ai_log::ActorId::user(),
            mcp_bridge: None,
            mcp_http_running: false,
            mcp_http_port: 3001,
            dimensions: Vec::new(),
            dim_style: crate::dimensions::DimensionStyle::default(),
            show_custom_color_picker: false,
            custom_color: [0.8, 0.8, 0.8, 1.0],
            mat_search: String::new(),
            mat_category_idx: 0,
            pending_ir: None,
            import_review: None,
            pending_unified_ir: None,
            texture_manager: crate::texture_manager::TextureManager::new(),
            spatial_index: None,
            spatial_index_version: u64::MAX,
        }
    }

    pub(crate) fn console_push(&mut self, level: &str, msg: String) {
        self.viewer.console_log.push((level.to_string(), msg, std::time::Instant::now()));
        if self.viewer.console_log.len() > 500 {
            self.viewer.console_log.remove(0);
        }
    }

    /// Log with timestamp prefix for debug console
    pub(crate) fn clog(&mut self, msg: impl Into<String>) {
        self.console_push("INFO", msg.into());
    }

    pub(crate) fn toast(&mut self, msg: impl Into<String>) {
        self.toasts.push((msg.into(), std::time::Instant::now()));
        if self.toasts.len() > 5 { self.toasts.remove(0); }
    }

    pub(crate) fn next_name(&mut self, prefix: &str) -> String {
        self.obj_counter += 1;
        format!("{}_{}", prefix, self.obj_counter)
    }

    pub(crate) fn execute_command_by_name(&mut self, name: &str) {
        match name {
            "建立方塊" => self.editor.tool = Tool::CreateBox,
            "建立圓柱" => self.editor.tool = Tool::CreateCylinder,
            "建立球體" => self.editor.tool = Tool::CreateSphere,
            "選取工具" => self.editor.tool = Tool::Select,
            "移動工具" => self.editor.tool = Tool::Move,
            "旋轉工具" => self.editor.tool = Tool::Rotate,
            "縮放工具" => self.editor.tool = Tool::Scale,
            "線段工具" => self.editor.tool = Tool::Line,
            "弧線工具" => self.editor.tool = Tool::Arc,
            "矩形工具" => self.editor.tool = Tool::Rectangle,
            "圓形工具" => self.editor.tool = Tool::Circle,
            "推拉工具" => self.editor.tool = Tool::PushPull,
            "偏移工具" => self.editor.tool = Tool::Offset,
            "量尺工具" => self.editor.tool = Tool::TapeMeasure,
            "標註工具" => self.editor.tool = Tool::Dimension,
            "橡皮擦" => self.editor.tool = Tool::Eraser,
            "軌道瀏覽" => self.editor.tool = Tool::Orbit,
            "平移瀏覽" => self.editor.tool = Tool::Pan,
            "全部顯示" => self.zoom_extents(),
            "群組工具" => self.editor.tool = Tool::Group,
            "復原" => { self.scene.undo(); },
            "重做" => { self.scene.redo(); },
            "儲存" => self.save_scene(),
            "開啟" => self.open_scene(),
            "全選" => { self.editor.selected_ids = self.scene.objects.keys().cloned().collect(); },
            "切換線框" => self.viewer.render_mode = RenderMode::Wireframe,
            "切換X光" => self.viewer.render_mode = RenderMode::XRay,
            "切換草稿" => self.viewer.render_mode = RenderMode::Sketch,
            "深色模式" => self.viewer.dark_mode = !self.viewer.dark_mode,
            "顯示格線" => self.viewer.show_grid = !self.viewer.show_grid,
            "清空場景" => { self.scene.snapshot(); self.scene.objects.clear(); self.scene.version += 1; },
            "MCP Server" => {
                if !self.mcp_http_running {
                    let port = self.mcp_http_port;
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
                        rt.block_on(kolibri_mcp::transport_http::run_http(port));
                    });
                    self.mcp_http_running = true;
                }
                let url = format!("http://localhost:{}", self.mcp_http_port);
                let _ = std::process::Command::new("cmd").args(["/C", "start", &url]).spawn();
            },
            "匯出 OBJ" => self.handle_menu_action(crate::menu::MenuAction::ExportObj),
            "匯出 STL" => self.handle_menu_action(crate::menu::MenuAction::ExportStl),
            "匯出 DXF" => self.handle_menu_action(crate::menu::MenuAction::ExportDxf),
            "匯入 OBJ" => self.handle_menu_action(crate::menu::MenuAction::ImportObj),
            "匯入 DXF" => self.handle_menu_action(crate::menu::MenuAction::ImportDxf),
            "CSG 聯集" => self.handle_menu_action(crate::menu::MenuAction::CsgUnion),
            "CSG 差集" => self.handle_menu_action(crate::menu::MenuAction::CsgSubtract),
            "CSG 交集" => self.handle_menu_action(crate::menu::MenuAction::CsgIntersect),
            "牆工具" => self.editor.tool = Tool::Wall,
            "板工具" => self.editor.tool = Tool::Slab,
            "對齊左" => self.align_selected(0),
            "對齊右" => self.align_selected(1),
            "對齊上" => self.align_selected(3),
            "對齊下" => self.align_selected(2),
            "X中心對齊" => self.align_selected(6),
            "Y中心對齊" => self.align_selected(7),
            "Z中心對齊" => self.align_selected(8),
            "X等距分佈" => self.distribute_selected(0),
            "Y等距分佈" => self.distribute_selected(1),
            "Z等距分佈" => self.distribute_selected(2),
            _ => {},
        }
    }

    /// 對齊選取物件：axis=0(X左), 1(X右), 2(Y下), 3(Y上), 4(Z前), 5(Z後), 6(X中), 7(Y中), 8(Z中)
    pub(crate) fn align_selected(&mut self, mode: u8) {
        if self.editor.selected_ids.len() < 2 { return; }
        let objs: Vec<_> = self.editor.selected_ids.iter()
            .filter_map(|id| self.scene.objects.get(id).cloned())
            .collect();
        if objs.len() < 2 { return; }

        let bbox = |o: &crate::scene::SceneObject| -> ([f32;3], [f32;3]) {
            let p = o.position;
            match &o.shape {
                Shape::Box { width, height, depth } => (p, [p[0]+width, p[1]+height, p[2]+depth]),
                Shape::Cylinder { radius, height, .. } => (p, [p[0]+radius*2.0, p[1]+height, p[2]+radius*2.0]),
                Shape::Sphere { radius, .. } => (p, [p[0]+radius*2.0, p[1]+radius*2.0, p[2]+radius*2.0]),
                _ => (p, [p[0]+100.0, p[1]+100.0, p[2]+100.0]),
            }
        };

        let target = match mode {
            0 => objs.iter().map(|o| bbox(o).0[0]).fold(f32::MAX, f32::min), // align left
            1 => objs.iter().map(|o| bbox(o).1[0]).fold(f32::MIN, f32::max), // align right
            2 => objs.iter().map(|o| bbox(o).0[1]).fold(f32::MAX, f32::min), // align bottom
            3 => objs.iter().map(|o| bbox(o).1[1]).fold(f32::MIN, f32::max), // align top
            4 => objs.iter().map(|o| bbox(o).0[2]).fold(f32::MAX, f32::min), // align front
            5 => objs.iter().map(|o| bbox(o).1[2]).fold(f32::MIN, f32::max), // align back
            6 => { let s: f32 = objs.iter().map(|o| (bbox(o).0[0]+bbox(o).1[0])*0.5).sum(); s / objs.len() as f32 } // center X
            7 => { let s: f32 = objs.iter().map(|o| (bbox(o).0[1]+bbox(o).1[1])*0.5).sum(); s / objs.len() as f32 } // center Y
            _ => { let s: f32 = objs.iter().map(|o| (bbox(o).0[2]+bbox(o).1[2])*0.5).sum(); s / objs.len() as f32 } // center Z
        };

        self.scene.snapshot();
        for id in &self.editor.selected_ids {
            if let Some(obj) = self.scene.objects.get_mut(id) {
                let (mn, mx) = bbox(obj);
                match mode {
                    0 => obj.position[0] += target - mn[0],
                    1 => obj.position[0] += target - mx[0],
                    2 => obj.position[1] += target - mn[1],
                    3 => obj.position[1] += target - mx[1],
                    4 => obj.position[2] += target - mn[2],
                    5 => obj.position[2] += target - mx[2],
                    6 => obj.position[0] += target - (mn[0]+mx[0])*0.5,
                    7 => obj.position[1] += target - (mn[1]+mx[1])*0.5,
                    _ => obj.position[2] += target - (mn[2]+mx[2])*0.5,
                }
            }
        }
        self.scene.version += 1;
    }

    /// 等距分佈選取物件：axis 0=X, 1=Y, 2=Z
    pub(crate) fn distribute_selected(&mut self, axis: u8) {
        if self.editor.selected_ids.len() < 3 { return; }
        let mut items: Vec<(String, f32)> = self.editor.selected_ids.iter()
            .filter_map(|id| self.scene.objects.get(id).map(|o| (id.clone(), o.position[axis as usize])))
            .collect();
        items.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        if items.len() < 3 { return; }

        let first = items.first().unwrap().1;
        let last = items.last().unwrap().1;
        let step = (last - first) / (items.len() - 1) as f32;

        self.scene.snapshot();
        for (i, (id, _)) in items.iter().enumerate() {
            if let Some(obj) = self.scene.objects.get_mut(id) {
                obj.position[axis as usize] = first + step * i as f32;
            }
        }
        self.scene.version += 1;
    }

    pub(crate) fn has_unsaved_changes(&self) -> bool {
        self.scene.version != self.last_saved_version
    }

    pub(crate) fn snap(v: f32, grid: f32) -> f32 {
        (v / grid).round() * grid
    }

    pub(crate) fn current_height(&self, base: [f32; 3]) -> f32 {
        let (origin, dir) = self.viewer.camera.screen_ray(
            self.editor.mouse_screen[0], self.editor.mouse_screen[1],
            self.viewer.viewport_size[0], self.viewer.viewport_size[1],
        );
        camera::ray_vertical_height(origin, dir, glam::Vec3::from(base))
    }

    pub(crate) fn zoom_extents(&mut self) {
        if self.scene.objects.is_empty() {
            self.viewer.camera = OrbitCamera::default();
            return;
        }
        let mut min = glam::Vec3::splat(f32::MAX);
        let mut max = glam::Vec3::splat(f32::MIN);
        for obj in self.scene.objects.values() {
            let p = glam::Vec3::from(obj.position);
            let s = match &obj.shape {
                Shape::Box { width, height, depth } => glam::Vec3::new(*width, *height, *depth),
                Shape::Cylinder { radius, height, .. } => glam::Vec3::new(*radius*2.0, *height, *radius*2.0),
                Shape::Sphere { radius, .. } => glam::Vec3::splat(*radius * 2.0),
                Shape::Line { points, .. } => {
                    let mut mx = glam::Vec3::ZERO;
                    for pt in points { mx = mx.max(glam::Vec3::from(*pt) - p); }
                    mx
                }
                Shape::Mesh(ref mesh) => {
                    let (mmin, mmax) = mesh.aabb();
                    glam::Vec3::from(mmax) - glam::Vec3::from(mmin)
                }
            };
            min = min.min(p);
            max = max.max(p + s);
        }
        let center = (min + max) * 0.5;
        let extent = (max - min).length();
        self.viewer.camera.target = center;
        self.viewer.camera.distance = extent * 1.5;
        self.editor.tool = Tool::Select;
    }
}

// ─── eframe::App ─────────────────────────────────────────────────────────────

impl eframe::App for KolibriApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Match panel fill so no dark gap shows between panels
        [245.0 / 255.0, 246.0 / 255.0, 250.0 / 255.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Crash recovery: 啟動時檢查 autosave ──
        if !self.editor.recovery_checked {
            self.editor.recovery_checked = true;
            let auto_path = "autosave.k3d";
            if self.scene.objects.is_empty() && std::path::Path::new(auto_path).exists() {
                if let Ok(n) = self.scene.load_from_file(auto_path) {
                    if n > 0 {
                        self.toasts.push((format!("已從自動儲存恢復 {} 個物件", n), std::time::Instant::now()));
                        self.file_message = Some((format!("已恢復自動儲存 ({} 物件)", n), std::time::Instant::now()));
                    }
                }
            }
        }

        // 深色模式切換（每幀檢查，因為使用者可能隨時切換）
        if self.viewer.dark_mode != ctx.style().visuals.dark_mode {
            if self.viewer.dark_mode {
                ctx.set_visuals(egui::Visuals::dark());
            } else {
                // 還原淺色 glassmorphism 主題
                let mut v = egui::Visuals::light();
                v.panel_fill = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220);
                v.window_fill = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 240);
                v.extreme_bg_color = egui::Color32::WHITE;
                v.faint_bg_color = egui::Color32::from_rgb(248, 249, 252);
                v.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(76, 139, 245, 36);
                v.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 139, 245));
                ctx.set_visuals(v);
            }
        }

        // ── Command Palette (Ctrl+P) ──
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::P)) {
            self.editor.command_palette_open = !self.editor.command_palette_open;
            self.editor.command_palette_query.clear();
        }
        if self.editor.command_palette_open {
            let commands: &[(&str, &str)] = &[
                ("建立方塊", "B"), ("建立圓柱", ""), ("建立球體", ""),
                ("選取工具", "Space"), ("移動工具", "M"), ("旋轉工具", "Q"),
                ("縮放工具", "S"), ("線段工具", "L"), ("弧線工具", "A"),
                ("矩形工具", "R"), ("圓形工具", "C"), ("推拉工具", "P"),
                ("偏移工具", "F"), ("量尺工具", "T"), ("標註工具", "D"),
                ("橡皮擦", "E"), ("軌道瀏覽", "O"), ("平移瀏覽", "H"),
                ("全部顯示", "Z"), ("群組工具", "G"),
                ("復原", "Ctrl+Z"), ("重做", "Ctrl+Y"),
                ("儲存", "Ctrl+S"), ("開啟", "Ctrl+O"),
                ("複製", "Ctrl+C"), ("貼上", "Ctrl+V"), ("剪下", "Ctrl+X"),
                ("全選", "Ctrl+A"),
                ("切換線框", ""), ("切換X光", ""), ("切換草稿", ""),
                ("深色模式", ""), ("顯示格線", ""),
                ("匯出 OBJ", ""), ("匯出 STL", ""), ("匯出 DXF", ""),
                ("匯入 OBJ", ""), ("匯入 DXF", ""),
                ("清空場景", ""), ("MCP Server", ""),
                ("牆工具", "W"), ("板工具", ""),
                ("就地複製", "Ctrl+D"), ("反轉選取", "Ctrl+I"),
                ("鏡射 X", "Ctrl+M"),
                ("對齊左", ""), ("對齊右", ""), ("對齊上", ""), ("對齊下", ""),
                ("X中心對齊", ""), ("Y中心對齊", ""), ("Z中心對齊", ""),
                ("X等距分佈", ""), ("Y等距分佈", ""), ("Z等距分佈", ""),
                ("CSG 聯集", ""), ("CSG 差集", ""), ("CSG 交集", ""),
                ("複製屬性", "Ctrl+Shift+C"), ("貼上屬性", "Ctrl+Shift+V"),
            ];
            let mut close = false;
            egui::Area::new(egui::Id::new("command_palette"))
                .fixed_pos(egui::pos2(ctx.screen_rect().center().x - 200.0, 80.0))
                .show(ctx, |ui| {
                    let frame = egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(30, 32, 48, 240))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 139, 245)))
                        .rounding(12.0)
                        .inner_margin(egui::Margin::same(8.0));
                    frame.show(ui, |ui| {
                        ui.set_min_width(400.0);
                        let resp = ui.add(egui::TextEdit::singleline(&mut self.editor.command_palette_query)
                            .hint_text("搜尋指令...")
                            .desired_width(384.0)
                            .font(egui::FontId::proportional(14.0)));
                        if !resp.has_focus() { resp.request_focus(); }
                        ui.add_space(4.0);

                        let query = self.editor.command_palette_query.to_lowercase();
                        let mut shown = 0;
                        for (name, shortcut) in commands {
                            if !query.is_empty() && !name.to_lowercase().contains(&query) { continue; }
                            if shown >= 12 { break; }
                            shown += 1;
                            ui.horizontal(|ui| {
                                if ui.add(egui::Label::new(
                                    egui::RichText::new(*name).size(13.0).color(egui::Color32::from_rgb(220, 225, 240))
                                ).sense(egui::Sense::click())).clicked() {
                                    // 執行對應指令
                                    self.execute_command_by_name(name);
                                    close = true;
                                }
                                if !shortcut.is_empty() {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new(*shortcut).size(10.0).color(egui::Color32::from_rgb(120, 130, 150)));
                                    });
                                }
                            });
                        }

                        // ESC 關閉
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { close = true; }
                    });
                });
            if close {
                self.editor.command_palette_open = false;
                self.editor.command_palette_query.clear();
            }
        }

        // Dynamic window title
        let title = if let Some(ref path) = self.current_file {
            let filename = path.rsplit(['\\', '/']).next().unwrap_or(path);
            if self.has_unsaved_changes() {
                format!("Kolibri_Ai3D \u{2014} {}*", filename)
            } else {
                format!("Kolibri_Ai3D \u{2014} {}", filename)
            }
        } else {
            if self.scene.objects.is_empty() {
                "Kolibri_Ai3D".to_string()
            } else {
                "Kolibri_Ai3D \u{2014} \u{672a}\u{5132}\u{5b58}\u{5834}\u{666f}*".to_string()
            }
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        // Poll MCP bridge commands
        if let Some(bridge) = self.mcp_bridge.take() {
            while let Ok((cmd, result_tx)) = bridge.cmd_rx.try_recv() {
                let result = self.handle_mcp_command(cmd);
                let _ = result_tx.send(result);
            }
            self.mcp_bridge = Some(bridge);
        }

        // ── Top branded bar ──
        egui::TopBottomPanel::top("topbar")
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 217))
                .inner_margin(egui::Margin::symmetric(18.0, 8.0))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239))))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left: Brand logo + name
                {
                    let (logo_rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::hover());
                    ui.painter().rect_filled(logo_rect, 12.0, egui::Color32::from_rgb(76, 139, 245));
                    ui.painter().text(logo_rect.center(), egui::Align2::CENTER_CENTER,
                        "K", egui::FontId::proportional(16.0), egui::Color32::WHITE);

                    ui.vertical(|ui| {
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new("Kolibri Ai3D").strong().size(14.0).color(egui::Color32::from_rgb(31, 36, 48)));
                        ui.label(egui::RichText::new("3D Modeling Workflow").size(10.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    });
                }

                ui.add_space(16.0);

                // Center: Menu bar (functional)
                let has_sel = !self.editor.selected_ids.is_empty();
                let can_undo = self.scene.can_undo();
                let can_redo = self.scene.can_redo();
                let count = self.scene.objects.len();
                let has_file = self.current_file.is_some();
                let action = crate::menu::draw_menu_bar(ui, has_sel, can_undo, can_redo, count, &self.recent_files, has_file, self.viewer.use_ortho, self.viewer.saved_cameras.len());

                // Right side: help + undo/redo + save + project name
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // MCP Server button
                    let mcp_label = if self.mcp_http_running {
                        egui::RichText::new("MCP").size(11.0).strong().color(egui::Color32::WHITE)
                    } else {
                        egui::RichText::new("MCP").size(11.0).color(egui::Color32::from_rgb(110, 118, 135))
                    };
                    let mcp_fill = if self.mcp_http_running {
                        egui::Color32::from_rgb(60, 186, 108)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(110, 118, 135, 30)
                    };
                    let mcp_btn = egui::Button::new(mcp_label)
                        .fill(mcp_fill)
                        .rounding(8.0);
                    let mcp_tip = if self.mcp_http_running {
                        format!("MCP Server 運行中 (port {})\n點擊開啟 Dashboard", self.mcp_http_port)
                    } else {
                        "啟動 MCP Server + Dashboard".to_string()
                    };
                    if ui.add(mcp_btn).on_hover_text(mcp_tip).clicked() {
                        if !self.mcp_http_running {
                            let port = self.mcp_http_port;
                            std::thread::spawn(move || {
                                let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
                                rt.block_on(kolibri_mcp::transport_http::run_http(port));
                            });
                            self.mcp_http_running = true;
                            self.file_message = Some((
                                format!("MCP Server 已啟動 http://localhost:{}", port),
                                std::time::Instant::now(),
                            ));
                        }
                        // 開啟瀏覽器
                        let url = format!("http://localhost:{}", self.mcp_http_port);
                        let _ = std::process::Command::new("cmd").args(["/C", "start", &url]).spawn();
                    }

                    ui.add_space(4.0);

                    // Help button
                    let help_btn = egui::Button::new(egui::RichText::new("?").size(14.0).strong())
                        .fill(egui::Color32::from_rgba_unmultiplied(76, 139, 245, 40))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 139, 245)))
                        .rounding(12.0);
                    if ui.add(help_btn).on_hover_text("\u{8aaa}\u{660e} (F1)").clicked() {
                        self.viewer.show_help = !self.viewer.show_help;
                    }

                    ui.add_space(4.0);

                    // Redo button with count
                    let redo_tip = format!("\u{91cd}\u{505a} (Ctrl+Y) \u{2014} {} \u{6b65}", self.scene.redo_count());
                    if ui.add_enabled(self.scene.can_redo(), egui::Button::new(
                        egui::RichText::new("\u{21bb}").size(14.0))).on_hover_text(redo_tip).clicked() {
                        self.scene.redo();
                    }

                    // Undo button with count
                    let undo_tip = format!("\u{5fa9}\u{539f} (Ctrl+Z) \u{2014} {} \u{6b65}", self.scene.undo_count());
                    if ui.add_enabled(self.scene.can_undo(), egui::Button::new(
                        egui::RichText::new("\u{21ba}").size(14.0))).on_hover_text(undo_tip).clicked() {
                        self.scene.undo();
                    }

                    ui.add_space(4.0);

                    // Save button
                    if ui.add(egui::Button::new("\u{1f4be}")).on_hover_text("\u{5132}\u{5b58} (Ctrl+S)").clicked() {
                        self.save_scene();
                    }

                    ui.separator();

                    // Project name (clickable to Save As)
                    let project_display = if let Some(ref path) = self.current_file {
                        let filename = path.rsplit(['\\', '/']).next().unwrap_or(path);
                        if self.has_unsaved_changes() {
                            format!("\u{1f4c4} {}*", filename)
                        } else {
                            format!("\u{1f4c4} {}", filename)
                        }
                    } else if !self.scene.objects.is_empty() {
                        "\u{1f4c4} \u{672a}\u{5132}\u{5b58}\u{5c08}\u{6848} *".to_string()
                    } else {
                        "\u{1f4c4} \u{65b0}\u{5c08}\u{6848}".to_string()
                    };

                    let proj_btn = egui::Button::new(egui::RichText::new(&project_display).size(11.0))
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::new(0.5, egui::Color32::from_rgb(229, 231, 239)))
                        .rounding(8.0);
                    if ui.add(proj_btn).on_hover_text("\u{9ede}\u{64ca}\u{53e6}\u{5b58}\u{65b0}\u{6a94}").clicked() {
                        self.current_file = None;
                        self.save_scene();
                    }
                });

                // Handle menu action
                match action {
                    crate::menu::MenuAction::ToggleOrtho => {
                        self.viewer.use_ortho = !self.viewer.use_ortho;
                        let mode = if self.viewer.use_ortho { "\u{5e73}\u{884c}\u{6295}\u{5f71}" } else { "\u{900f}\u{8996}\u{6295}\u{5f71}" };
                        self.file_message = Some((format!("\u{5df2}\u{5207}\u{63db}: {}", mode), std::time::Instant::now()));
                    }
                    crate::menu::MenuAction::SaveCamera => {
                        let name = format!("\u{5834}\u{666f} {}", self.viewer.saved_cameras.len() + 1);
                        self.viewer.saved_cameras.push((name, self.viewer.camera.clone()));
                        self.file_message = Some(("\u{8996}\u{89d2}\u{5df2}\u{5132}\u{5b58}".into(), std::time::Instant::now()));
                    }
                    crate::menu::MenuAction::ToggleConsole => {
                        self.viewer.show_console = !self.viewer.show_console;
                    }
                    other => self.handle_menu_action(other),
                }
            });
        });

        // ── Left panel (toolbar only) ──
        egui::SidePanel::left("left_panel")
            .default_width(116.0).min_width(116.0).max_width(116.0).resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(245, 246, 250))
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(6.0, 0.0)))
            .show(ctx, |ui| {
                ui.add_space(8.0);
                self.toolbar_ui(ui);
            });

        // ── Right panel ──
        egui::SidePanel::right("right_panel")
            .default_width(260.0).min_width(200.0).resizable(true)
            .show_separator_line(false)
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220))
                .inner_margin(egui::Margin::symmetric(10.0, 8.0)))
            .show(ctx, |ui| self.right_panel_ui(ui));

        // ── Console/Log panel (above status bar) ──
        if self.viewer.show_console {
            egui::TopBottomPanel::bottom("console")
                .min_height(100.0)
                .max_height(300.0)
                .resizable(true)
                .frame(egui::Frame::none()
                    .fill(egui::Color32::from_rgb(30, 30, 35))
                    .inner_margin(egui::Margin::same(8.0)))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Console").color(egui::Color32::from_gray(180)).size(12.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✕").clicked() {
                                self.viewer.show_console = false;
                            }
                            if ui.small_button("清除").clicked() {
                                self.viewer.console_log.clear();
                            }
                        });
                    });
                    ui.separator();

                    egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                        let now = std::time::Instant::now();
                        for (level, msg, time) in &self.viewer.console_log {
                            let color = match level.as_str() {
                                "ERROR" => egui::Color32::from_rgb(255, 80, 80),
                                "WARN" => egui::Color32::from_rgb(255, 200, 60),
                                "ACTION" => egui::Color32::from_rgb(100, 255, 160),
                                "CLICK" => egui::Color32::from_rgb(255, 180, 100),
                                "TOOL" => egui::Color32::from_rgb(180, 140, 255),
                                "INFO" => egui::Color32::from_rgb(150, 200, 255),
                                _ => egui::Color32::from_gray(180),
                            };
                            let elapsed = now.duration_since(*time);
                            let ts = format!("{:.1}s", elapsed.as_secs_f32());
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&ts).color(egui::Color32::from_gray(100)).size(9.0).monospace());
                                ui.label(egui::RichText::new(level).color(color).size(10.0).monospace());
                                ui.label(egui::RichText::new(msg).color(egui::Color32::from_gray(210)).size(11.0));
                            });
                        }
                    });
                });
        }

        // ── Bottom: status + measurement ──
        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210))
                .inner_margin(egui::Margin::symmetric(16.0, 6.0))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239))))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Show file save/load message for 3 seconds
                if let Some((ref msg, when)) = self.file_message {
                    if when.elapsed().as_secs() < 3 {
                        ui.label(egui::RichText::new(msg).size(11.0).color(egui::Color32::from_rgb(20, 174, 92)));
                    } else {
                        self.file_message = None;
                    }
                }
                ui.label(egui::RichText::new(self.status_text()).size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Always-visible measurement input (like SketchUp VCB)
                    ui.label(egui::RichText::new("mm").size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    let vcb = ui.add(
                        egui::TextEdit::singleline(&mut self.editor.measure_input)
                            .desired_width(140.0)
                            .hint_text("輸入尺寸...")
                            .font(egui::FontId::proportional(12.0))
                    );
                    if vcb.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.apply_measure();
                    }
                    ui.label(egui::RichText::new("尺寸:").size(11.0).strong().color(egui::Color32::from_rgb(76, 139, 245)));
                    ui.separator();
                    ui.label(egui::RichText::new(format!("物件: {}", self.scene.objects.len())).size(11.0).color(egui::Color32::from_rgb(110, 118, 135)));
                    ui.separator();
                    let console_label = if self.viewer.show_console { "▼ Console" } else { "▲ Console" };
                    if ui.small_button(egui::RichText::new(console_label).size(10.0)).on_hover_text("F12").clicked() {
                        self.viewer.show_console = !self.viewer.show_console;
                    }
                });
            });
        });

        // Handle drag-and-drop files
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in &dropped {
            if let Some(ref path) = file.path {
                let p = path.to_string_lossy().to_string();
                if p.ends_with(".k3d") {
                    self.console_push("INFO", format!("[File] 載入: {}", p));
                    match self.scene.load_from_file(&p) {
                        Ok(count) => {
                            self.current_file = Some(p.clone());
                            self.add_recent_file(&p);
                            self.editor.selected_ids.clear();
                            self.last_saved_version = self.scene.version;
                            // Auto-load textures referenced by scene objects
                            for obj in self.scene.objects.values() {
                                if let Some(ref tex_path) = obj.texture_path {
                                    let _ = self.texture_manager.load(tex_path);
                                }
                            }
                            self.console_push("INFO", format!("[File] 已載入 {} 個物件", count));
                            self.file_message = Some((format!("已載入 {} 個物件", count), std::time::Instant::now()));
                        }
                        Err(e) => {
                            self.console_push("ERROR", format!("[File] 載入失敗: {}", e));
                            self.file_message = Some((format!("載入失敗: {}", e), std::time::Instant::now()));
                        }
                    }
                } else if p.ends_with(".obj") {
                    self.console_push("INFO", format!("[Import] OBJ: {}", p));
                    match crate::obj_io::import_obj(&mut self.scene, &p) {
                        Ok(count) => {
                            self.add_recent_file(&p);
                            self.editor.selected_ids.clear();
                            self.console_push("INFO", format!("[Import] OBJ 已匯入 {} 個物件", count));
                            self.file_message = Some((format!("已匯入 {} 個物件", count), std::time::Instant::now()));
                        }
                        Err(e) => {
                            self.console_push("ERROR", format!("[Import] OBJ 匯入失敗: {}", e));
                            self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now()));
                        }
                    }
                } else if p.ends_with(".stl") {
                    self.console_push("WARN", "[Import] STL 匯入尚未支援".to_string());
                    self.file_message = Some(("STL 匯入尚未支援".to_string(), std::time::Instant::now()));
                }
            }
        }

        // Viewport
        egui::CentralPanel::default()
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(245, 246, 250))
                .inner_margin(egui::Margin::same(0.0)))
            .show(ctx, |ui| {
                // ── Layout mode: 2D paper view ──
                if self.viewer.layout_mode {
                    let avail = ui.available_size();
                    let (rect, _response) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
                    crate::layout::draw_layout(ui, &self.viewer.layout, rect);
                    return;
                }

                let avail = ui.available_size();
                let w = (avail.x.ceil() as u32).max(1);
                let h = (avail.y.ceil() as u32).max(1);

                { let mut r = self.egui_renderer.write(); self.viewport.ensure_size(&self.device, &mut r, w, h); }

                // Sync layer visibility from hidden_tags
                for obj in self.scene.objects.values_mut() {
                    obj.visible = !self.viewer.hidden_tags.contains(&obj.tag);
                }

                // 先分配區域取得 response，再處理互動，最後渲染
                // 這確保點擊/材質變更在渲染前生效（同幀即時反映）
                let (rect, response) = ui.allocate_exact_size(avail, egui::Sense::click_and_drag());
                self.handle_viewport(&response, ui);

                let preview = self.build_preview();
                let aspect = w as f32 / h.max(1) as f32;
                let vp = if self.viewer.use_ortho {
                    self.viewer.camera.proj_ortho(aspect) * self.viewer.camera.view()
                } else {
                    self.viewer.camera.view_proj(aspect)
                };
                let hf = self.editor.hovered_face.as_ref().map(|(id, face)| (id.as_str(), face.as_u8()));
                let sf = self.editor.selected_face.as_ref().map(|(id, face)| (id.as_str(), face.as_u8()));
                self.viewport.render(&self.device, &self.queue, vp, &self.scene, &self.editor.selected_ids, self.editor.hovered_id.as_deref(), self.editor.editing_group_id.as_deref(), &preview, self.viewer.render_mode.as_u32(), self.viewer.sky_color, self.viewer.ground_color, hf, sf, self.viewer.edge_thickness, self.viewer.show_colors, &self.texture_manager, self.viewer.show_grid, self.viewer.grid_spacing);

                if let Some(tex_id) = self.viewport.texture_id {
                    ui.painter().image(tex_id, rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                }

                // Draw rubber band selection rectangle
                if let Some((start, end)) = self.editor.rubber_band {
                    let rb_rect = egui::Rect::from_two_pos(start, end);
                    let is_crossing = start.x > end.x;
                    let painter = ui.painter();
                    // Window = 實線藍色，Crossing = 虛線綠色
                    let (fill_color, stroke_color) = if is_crossing {
                        (egui::Color32::from_rgba_unmultiplied(60, 200, 80, 30),
                         egui::Color32::from_rgb(60, 200, 80))
                    } else {
                        (egui::Color32::from_rgba_unmultiplied(60, 120, 220, 40),
                         egui::Color32::from_rgb(80, 140, 240))
                    };
                    painter.rect_filled(rb_rect, 0.0, fill_color);
                    if is_crossing {
                        // 虛線邊框
                        let corners = [rb_rect.left_top(), rb_rect.right_top(), rb_rect.right_bottom(), rb_rect.left_bottom()];
                        for i in 0..4 {
                            draw_dashed_line(painter, corners[i], corners[(i+1)%4],
                                egui::Stroke::new(1.5, stroke_color), 6.0, 4.0);
                        }
                    } else {
                        painter.rect_stroke(rb_rect, 0.0, egui::Stroke::new(1.5, stroke_color));
                    }
                }

                // Draw snap indicators on top of viewport
                if let Some(ref snap) = self.editor.snap_result {
                    if snap.snap_type != SnapType::None && snap.snap_type != SnapType::Grid {
                        let painter = ui.painter();

                        // Draw snap point indicator — shape varies by snap type (SketchUp-style)
                        if let Some(screen_pos) = self.world_to_screen(snap.position, &rect) {
                            let color = snap.snap_type.color();
                            let sx = screen_pos.x;
                            let sy = screen_pos.y;

                            match snap.snap_type {
                                SnapType::Endpoint => {
                                    // Green diamond
                                    let diamond = vec![
                                        egui::pos2(sx, sy - 7.0),
                                        egui::pos2(sx + 7.0, sy),
                                        egui::pos2(sx, sy + 7.0),
                                        egui::pos2(sx - 7.0, sy),
                                    ];
                                    painter.add(egui::Shape::convex_polygon(diamond, color, egui::Stroke::new(1.5, egui::Color32::WHITE)));
                                }
                                SnapType::Midpoint => {
                                    // Cyan triangle
                                    let tri = vec![
                                        egui::pos2(sx, sy - 7.0),
                                        egui::pos2(sx + 6.0, sy + 5.0),
                                        egui::pos2(sx - 6.0, sy + 5.0),
                                    ];
                                    painter.add(egui::Shape::convex_polygon(tri, color, egui::Stroke::new(1.5, egui::Color32::WHITE)));
                                }
                                SnapType::OnFace => {
                                    // Blue filled circle
                                    painter.circle_filled(screen_pos, 6.0, color);
                                    painter.circle_stroke(screen_pos, 6.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
                                }
                                SnapType::Intersection => {
                                    // X marker
                                    let sz = 7.0;
                                    painter.line_segment(
                                        [screen_pos + egui::vec2(-sz, -sz), screen_pos + egui::vec2(sz, sz)],
                                        egui::Stroke::new(2.5, color),
                                    );
                                    painter.line_segment(
                                        [screen_pos + egui::vec2(-sz, sz), screen_pos + egui::vec2(sz, -sz)],
                                        egui::Stroke::new(2.5, color),
                                    );
                                }
                                SnapType::FaceCenter => {
                                    // Cross (+) indicator for face center
                                    let sz = 6.0;
                                    painter.line_segment(
                                        [egui::pos2(sx - sz, sy), egui::pos2(sx + sz, sy)],
                                        egui::Stroke::new(2.0, color),
                                    );
                                    painter.line_segment(
                                        [egui::pos2(sx, sy - sz), egui::pos2(sx, sy + sz)],
                                        egui::Stroke::new(2.0, color),
                                    );
                                }
                                SnapType::OnEdge => {
                                    // Red diamond (match SketchUp)
                                    let edge_color = egui::Color32::from_rgb(220, 50, 50);
                                    let diamond = vec![
                                        egui::pos2(sx, sy - 5.0),
                                        egui::pos2(sx + 5.0, sy),
                                        egui::pos2(sx, sy + 5.0),
                                        egui::pos2(sx - 5.0, sy),
                                    ];
                                    painter.add(egui::Shape::convex_polygon(diamond, edge_color, egui::Stroke::NONE));
                                }
                                _ => {
                                    // Default: circle indicator
                                    painter.circle_stroke(screen_pos, 12.0, egui::Stroke::new(2.5, color));
                                    painter.circle_filled(screen_pos, 5.0, color);
                                }
                            }

                            // (Old combined label removed — now displayed in cursor hint card)
                        }

                        // Draw axis / parallel / perpendicular inference line from origin to snap point
                        if let Some(from) = snap.from_point {
                            if matches!(snap.snap_type, SnapType::AxisX | SnapType::AxisZ
                                        | SnapType::Parallel | SnapType::Perpendicular) {
                                if let (Some(from_s), Some(to_s)) = (
                                    self.world_to_screen(from, &rect),
                                    self.world_to_screen(snap.position, &rect),
                                ) {
                                    let color = snap.snap_type.color();
                                    // Draw dashed line (thicker stroke)
                                    let dir = to_s - from_s;
                                    let len = dir.length();
                                    if len > 1.0 {
                                        let step = 8.0;
                                        let norm = dir / len;
                                        let mut d = 0.0;
                                        while d < len {
                                            let a = from_s + norm * d;
                                            let b = from_s + norm * (d + step * 0.6).min(len);
                                            painter.line_segment([a, b], egui::Stroke::new(2.0, color));
                                            d += step;
                                        }
                                    }
                                }
                            }
                        }

                        // Show distance from start point when drawing (B3: distance labels on snap lines)
                        if let Some(from) = snap.from_point {
                            if let (Some(from_s), Some(to_s)) = (
                                self.world_to_screen(from, &rect),
                                self.world_to_screen(snap.position, &rect),
                            ) {
                                let dx = snap.position[0] - from[0];
                                let dz = snap.position[2] - from[2];
                                let dist = (dx * dx + dz * dz).sqrt();
                                if dist > 1.0 {
                                    let mid = egui::pos2(
                                        (from_s.x + to_s.x) * 0.5,
                                        (from_s.y + to_s.y) * 0.5 - 15.0,
                                    );
                                    let dist_text = if dist >= 1000.0 {
                                        format!("{:.2} m", dist / 1000.0)
                                    } else {
                                        format!("{:.0} mm", dist)
                                    };
                                    // Background rectangle for readability
                                    let font = egui::FontId::proportional(13.0);
                                    let galley = painter.layout_no_wrap(dist_text, font, egui::Color32::from_rgb(255, 255, 200));
                                    let bg_rect = egui::Rect::from_center_size(mid, galley.size()).expand(3.0);
                                    painter.rect_filled(bg_rect, 2.0, egui::Color32::from_rgba_unmultiplied(30, 30, 40, 200));
                                    painter.galley(bg_rect.min, galley, egui::Color32::from_rgb(255, 255, 200));

                                    // Also show axis-decomposed distances for non-axis-aligned snaps
                                    if !matches!(snap.snap_type, SnapType::AxisX | SnapType::AxisZ) {
                                        let adx = dx.abs();
                                        let adz = dz.abs();
                                        if adx > 50.0 && adz > 50.0 {
                                            let x_text = if adx >= 1000.0 { format!("X: {:.2} m", adx / 1000.0) }
                                                         else { format!("X: {:.0} mm", adx) };
                                            let z_text = if adz >= 1000.0 { format!("Z: {:.2} m", adz / 1000.0) }
                                                         else { format!("Z: {:.0} mm", adz) };
                                            let detail_pos = mid + egui::vec2(0.0, 16.0);
                                            let detail = format!("{} | {}", x_text, z_text);
                                            let font2 = egui::FontId::proportional(10.0);
                                            let galley2 = painter.layout_no_wrap(detail, font2, egui::Color32::from_gray(180));
                                            let bg2 = egui::Rect::from_center_size(detail_pos, galley2.size()).expand(2.0);
                                            painter.rect_filled(bg2, 2.0, egui::Color32::from_rgba_unmultiplied(30, 30, 40, 180));
                                            painter.galley(bg2.min, galley2, egui::Color32::from_gray(180));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Persistent snap point highlight during measuring/dimension (stays until complete)
                if let DrawState::Measuring { start } = &self.editor.draw_state {
                    if let Some(sp) = self.world_to_screen(*start, &rect) {
                        let painter = ui.painter();
                        // Pulsing highlight ring
                        painter.circle_stroke(sp, 10.0, egui::Stroke::new(2.5, egui::Color32::from_rgb(76, 139, 245)));
                        painter.circle_filled(sp, 4.0, egui::Color32::from_rgb(76, 139, 245));
                        // Label
                        painter.text(
                            egui::pos2(sp.x + 14.0, sp.y - 6.0),
                            egui::Align2::LEFT_CENTER,
                            "起點",
                            egui::FontId::proportional(10.0),
                            egui::Color32::from_rgb(76, 139, 245),
                        );
                    }
                }
                // Also highlight start point during line/arc drawing
                if let DrawState::LineFrom { p1 } | DrawState::ArcP1 { p1 } | DrawState::ArcP2 { p1, .. } = &self.editor.draw_state {
                    if let Some(sp) = self.world_to_screen(*p1, &rect) {
                        let painter = ui.painter();
                        painter.circle_stroke(sp, 8.0, egui::Stroke::new(2.0, egui::Color32::from_rgb(60, 200, 60)));
                        painter.circle_filled(sp, 3.0, egui::Color32::from_rgb(60, 200, 60));
                    }
                }

                // Draw dimension annotations
                if !self.dimensions.is_empty() {
                    crate::dimensions::draw_dimensions_styled(ui.painter(), &self.dimensions, vp, &rect, &self.dim_style);
                }

                // Auto-show dimensions for selected object (CAD style)
                if let Some(ref id) = self.editor.selected_ids.first() {
                    if let Some(obj) = self.scene.objects.get(*id) {
                        let auto_dims = crate::dimensions::auto_dims_for_shape(&obj.shape, obj.position);
                        if !auto_dims.is_empty() {
                            crate::dimensions::draw_dimensions_styled(ui.painter(), &auto_dims, vp, &rect, &self.dim_style);
                        }
                    }
                }

                // ── 標註點擊編輯：雙擊標註文字進入編輯模式 ──
                if response.double_clicked() && !self.dimensions.is_empty() {
                    let positions = crate::dimensions::dim_label_positions(
                        &self.dimensions, vp, &rect, &self.dim_style,
                    );
                    let mx = self.editor.mouse_screen[0] + rect.min.x;
                    let my = self.editor.mouse_screen[1] + rect.min.y;
                    let hit_radius = 20.0;
                    for (idx, pos) in positions.iter().enumerate() {
                        if let Some(p) = pos {
                            let dx = mx - p.x;
                            let dy = my - p.y;
                            if dx * dx + dy * dy < hit_radius * hit_radius {
                                self.editor.editing_dim_idx = Some(idx);
                                self.editor.editing_dim_text = self.dimensions[idx].label_text(&self.dim_style);
                                break;
                            }
                        }
                    }
                }

                // ── 標註編輯浮動輸入框 ──
                if let Some(dim_idx) = self.editor.editing_dim_idx {
                    if dim_idx < self.dimensions.len() {
                        let positions = crate::dimensions::dim_label_positions(
                            &self.dimensions, vp, &rect, &self.dim_style,
                        );
                        if let Some(Some(label_pos)) = positions.get(dim_idx) {
                            let input_rect = egui::Rect::from_center_size(
                                *label_pos,
                                egui::vec2(100.0, 24.0),
                            );
                            let mut text = self.editor.editing_dim_text.clone();
                            let resp = ui.put(input_rect, egui::TextEdit::singleline(&mut text)
                                .font(egui::FontId::proportional(13.0))
                                .desired_width(90.0)
                                .horizontal_align(egui::Align::Center));
                            self.editor.editing_dim_text = text.clone();
                            // 自動 focus
                            if !resp.has_focus() {
                                resp.request_focus();
                            }
                            // Enter 確認
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                // 嘗試解析為數值並更新標註
                                let clean: String = text.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
                                if let Ok(val) = clean.parse::<f32>() {
                                    match &mut self.dimensions[dim_idx].kind {
                                        crate::dimensions::DimensionKind::Linear { start, end } => {
                                            // 保持方向，改距離
                                            let dir = glam::Vec3::from(*end) - glam::Vec3::from(*start);
                                            let len = dir.length();
                                            if len > 0.001 {
                                                let new_end = glam::Vec3::from(*start) + dir / len * val;
                                                *end = new_end.to_array();
                                            }
                                        }
                                        crate::dimensions::DimensionKind::Radius { radius, .. } => {
                                            *radius = val;
                                        }
                                        crate::dimensions::DimensionKind::Diameter { radius, .. } => {
                                            *radius = val / 2.0;
                                        }
                                        _ => {
                                            // 角度/弧長：設定 override label
                                            self.dimensions[dim_idx].label = Some(text.clone());
                                        }
                                    }
                                } else {
                                    // 非數值：設為 override label
                                    self.dimensions[dim_idx].label = Some(text.clone());
                                }
                                self.editor.editing_dim_idx = None;
                                self.editor.editing_dim_text.clear();
                            }
                            // ESC 取消
                            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                self.editor.editing_dim_idx = None;
                                self.editor.editing_dim_text.clear();
                            }
                        }
                    }
                }

                self.draw_viewport_overlays(ui, vp, rect, &response);


                // ── Help overlay ──
                if self.viewer.show_help {
                    // Full viewport overlay
                    let help_bg = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 245);
                    ui.painter().rect_filled(rect, 0.0, help_bg);

                    let help_rect = rect.shrink(40.0);

                    // Title
                    ui.painter().text(egui::pos2(help_rect.center().x, help_rect.top() + 20.0),
                        egui::Align2::CENTER_TOP, "Kolibri Ai3D \u{64cd}\u{4f5c}\u{8aaa}\u{660e}",
                        egui::FontId::proportional(20.0), egui::Color32::from_rgb(31, 36, 48));

                    // Close button
                    let close_rect = egui::Rect::from_min_size(
                        egui::pos2(help_rect.right() - 36.0, help_rect.top() + 8.0),
                        egui::vec2(28.0, 28.0));
                    ui.painter().rect_filled(close_rect, 14.0, egui::Color32::from_rgb(240, 70, 50));
                    ui.painter().text(close_rect.center(), egui::Align2::CENTER_CENTER, "\u{2715}",
                        egui::FontId::proportional(14.0), egui::Color32::WHITE);
                    let close_resp = ui.allocate_rect(close_rect, egui::Sense::click());
                    if close_resp.clicked() { self.viewer.show_help = false; }

                    // Help content columns
                    let col_w = (help_rect.width() - 60.0) / 3.0;
                    let col_y = help_rect.top() + 60.0;
                    let line_h = 22.0;
                    let header_font = egui::FontId::proportional(14.0);
                    let body_font = egui::FontId::proportional(11.0);
                    let header_color = egui::Color32::from_rgb(76, 139, 245);
                    let text_color = egui::Color32::from_rgb(50, 55, 65);
                    let key_color = egui::Color32::from_rgb(110, 118, 135);

                    // Column 1: Mouse & Navigation
                    let x1 = help_rect.left() + 20.0;
                    let mut y = col_y;
                    ui.painter().text(egui::pos2(x1, y), egui::Align2::LEFT_TOP, "\u{1f5b1} \u{6ed1}\u{9f20}\u{64cd}\u{4f5c}", header_font.clone(), header_color);
                    y += line_h + 4.0;
                    let mouse_help: &[(&str, &str)] = &[
                        ("\u{4e2d}\u{9375}\u{62d6}\u{66f3}", "\u{65cb}\u{8f49}\u{8996}\u{89d2}"),
                        ("Shift + \u{4e2d}\u{9375}", "\u{5e73}\u{79fb}\u{8996}\u{89d2}"),
                        ("\u{6efe}\u{8f2a}", "\u{7e2e}\u{653e}\u{ff08}\u{6e38}\u{6a19}\u{4e2d}\u{5fc3}\u{ff09}"),
                        ("\u{4e2d}\u{9375}\u{9ede}\u{64ca}", "\u{5c45}\u{4e2d}\u{5230}\u{9ede}\u{64ca}\u{4f4d}\u{7f6e}"),
                        ("\u{5de6}\u{9375}\u{9ede}\u{64ca}", "\u{9078}\u{53d6} / \u{4f7f}\u{7528}\u{5de5}\u{5177}"),
                        ("\u{5de6}\u{9375}\u{62d6}\u{66f3}", "\u{6846}\u{9078}\u{ff08}Select \u{5de5}\u{5177}\u{ff09}"),
                        ("Shift + \u{5de6}\u{9375}", "\u{591a}\u{9078}\u{5207}\u{63db}"),
                        ("\u{53f3}\u{9375}", "\u{53f3}\u{9375}\u{9078}\u{55ae}"),
                    ];
                    for (key, desc) in mouse_help {
                        ui.painter().text(egui::pos2(x1, y), egui::Align2::LEFT_TOP, *key, body_font.clone(), key_color);
                        ui.painter().text(egui::pos2(x1 + 110.0, y), egui::Align2::LEFT_TOP, *desc, body_font.clone(), text_color);
                        y += line_h;
                    }

                    y += 10.0;
                    ui.painter().text(egui::pos2(x1, y), egui::Align2::LEFT_TOP, "\u{1f4d0} \u{7e6a}\u{5716}\u{64cd}\u{4f5c}", header_font.clone(), header_color);
                    y += line_h + 4.0;
                    let draw_help: &[(&str, &str)] = &[
                        ("\u{756b}\u{5b8c}\u{8f38}\u{5165}\u{6578}\u{5b57}", "\u{7cbe}\u{78ba}\u{5c3a}\u{5bf8}\u{ff08}\u{53f3}\u{4e0b}\u{89d2}\u{ff09}"),
                        ("Shift \u{6309}\u{4f4f}", "\u{9396}\u{5b9a}\u{8ef8}\u{5411}"),
                        ("Ctrl + \u{79fb}\u{52d5}", "\u{8907}\u{88fd}\u{79fb}\u{52d5}"),
                        ("\u{8907}\u{88fd}\u{5f8c}\u{8f38}\u{5165} 3x", "\u{5efa}\u{7acb} 3 \u{500b}\u{7b49}\u{8ddd}\u{526f}\u{672c}"),
                        ("ESC", "\u{53d6}\u{6d88}\u{76ee}\u{524d}\u{64cd}\u{4f5c}"),
                        ("\u{96d9}\u{64ca}\u{63a8}\u{62c9}\u{9762}", "\u{91cd}\u{8907}\u{4e0a}\u{6b21}\u{63a8}\u{62c9}\u{8ddd}\u{96e2}"),
                        ("TAB", "\u{5957}\u{7528} AI \u{5efa}\u{8b70}"),
                    ];
                    for (key, desc) in draw_help {
                        ui.painter().text(egui::pos2(x1, y), egui::Align2::LEFT_TOP, *key, body_font.clone(), key_color);
                        ui.painter().text(egui::pos2(x1 + 110.0, y), egui::Align2::LEFT_TOP, *desc, body_font.clone(), text_color);
                        y += line_h;
                    }

                    // Column 2: Keyboard Shortcuts
                    let x2 = x1 + col_w + 20.0;
                    y = col_y;
                    ui.painter().text(egui::pos2(x2, y), egui::Align2::LEFT_TOP, "\u{2328} \u{5feb}\u{6377}\u{9375}", header_font.clone(), header_color);
                    y += line_h + 4.0;
                    let shortcuts: &[(&str, &str)] = &[
                        ("Space", "\u{9078}\u{53d6}\u{5de5}\u{5177}"),
                        ("M", "\u{79fb}\u{52d5}"),
                        ("Q", "\u{65cb}\u{8f49}"),
                        ("S", "\u{7e2e}\u{653e}"),
                        ("L", "\u{7dda}\u{6bb5}"),
                        ("A", "\u{5f27}\u{7dda}"),
                        ("R", "\u{77e9}\u{5f62}"),
                        ("C", "\u{5713}\u{5f62}"),
                        ("B", "\u{65b9}\u{584a}"),
                        ("P", "\u{63a8}\u{62c9}"),
                        ("T", "\u{91cf}\u{6e2c}"),
                        ("H", "\u{5e73}\u{79fb}\u{8996}\u{89d2}"),
                        ("O", "\u{74b0}\u{7e5e}\u{8996}\u{89d2}"),
                        ("E", "\u{6a61}\u{76ae}\u{64e6}"),
                        ("G", "\u{7fa4}\u{7d44}"),
                        ("F", "\u{504f}\u{79fb}"),
                        ("Z", "\u{5168}\u{90e8}\u{986f}\u{793a}"),
                        ("5", "\u{900f}\u{8996}/\u{5e73}\u{884c}\u{5207}\u{63db}"),
                        ("1/2/3", "\u{6b63}\u{8996}/\u{4fef}\u{8996}/\u{7b49}\u{89d2}"),
                        ("Ctrl+Z", "\u{5fa9}\u{539f}"),
                        ("Ctrl+Y", "\u{91cd}\u{505a}"),
                        ("Ctrl+S", "\u{5132}\u{5b58}"),
                        ("Ctrl+O", "\u{958b}\u{555f}"),
                        ("Ctrl+C/V/X", "複製/貼上/剪下"),
                        ("Ctrl+D", "就地複製"),
                        ("Ctrl+M", "鏡射 X"),
                        ("Ctrl+I", "反轉選取"),
                        ("Ctrl+P", "指令面板"),
                        ("Ctrl+A", "全選"),
                        ("W", "牆工具"),
                        ("F1", "說明（本頁面）"),
                        ("F12", "Console"),
                        ("Delete", "刪除選取"),
                    ];
                    for (key, desc) in shortcuts {
                        ui.painter().text(egui::pos2(x2, y), egui::Align2::LEFT_TOP, *key, body_font.clone(), key_color);
                        ui.painter().text(egui::pos2(x2 + 60.0, y), egui::Align2::LEFT_TOP, *desc, body_font.clone(), text_color);
                        y += line_h;
                    }

                    // Column 3: Features & Tips
                    let x3 = x2 + col_w + 20.0;
                    y = col_y;
                    ui.painter().text(egui::pos2(x3, y), egui::Align2::LEFT_TOP, "\u{1f527} \u{6355}\u{6349}\u{7cfb}\u{7d71}", header_font.clone(), header_color);
                    y += line_h + 4.0;
                    let snap_help: &[(&str, &str)] = &[
                        ("\u{1f7e2} \u{7da0}\u{8272}\u{83f1}\u{5f62}", "\u{7aef}\u{9ede}"),
                        ("\u{1f535} \u{9752}\u{8272}\u{4e09}\u{89d2}", "\u{908a}\u{4e2d}\u{9ede}"),
                        ("\u{ff0b} \u{6dfa}\u{85cd}\u{5341}\u{5b57}", "\u{9762}\u{4e2d}\u{5fc3}"),
                        ("\u{1f534} \u{7d05}\u{8272}\u{83f1}\u{5f62}", "\u{908a}\u{4e0a}"),
                        ("\u{1f7e0} \u{6a59}\u{8272}\u{5713}\u{9ede}", "\u{539f}\u{9ede}"),
                        ("\u{1f534} \u{7d05}\u{8272}\u{865b}\u{7dda}", "X \u{8ef8}"),
                        ("\u{1f7e2} \u{7da0}\u{8272}\u{865b}\u{7dda}", "Y \u{8ef8}"),
                        ("\u{1f535} \u{85cd}\u{8272}\u{865b}\u{7dda}", "Z \u{8ef8}"),
                        ("\u{1f7e3} \u{7d2b}\u{8272}", "\u{5e73}\u{884c}/\u{5782}\u{76f4}"),
                        ("\u{1f4cd}", "\u{4e0a}\u{4e0b}\u{6587}\u{63a8}\u{65b7}"),
                        ("\u{1f916}", "AI \u{610f}\u{5716}\u{63a8}\u{65b7}"),
                    ];
                    for (key, desc) in snap_help {
                        ui.painter().text(egui::pos2(x3, y), egui::Align2::LEFT_TOP, *key, body_font.clone(), key_color);
                        ui.painter().text(egui::pos2(x3 + 85.0, y), egui::Align2::LEFT_TOP, *desc, body_font.clone(), text_color);
                        y += line_h;
                    }

                    y += 10.0;
                    ui.painter().text(egui::pos2(x3, y), egui::Align2::LEFT_TOP, "\u{1f3d7} \u{92fc}\u{69cb}\u{6a21}\u{5f0f}", header_font.clone(), header_color);
                    y += line_h + 4.0;
                    let steel_help: &[(&str, &str)] = &[
                        ("\u{5de6}\u{4e0a}\u{89d2}\u{5207}\u{63db}", "\u{5efa}\u{6a21} \u{2194} \u{92fc}\u{69cb}"),
                        ("\u{8ef8}\u{7dda}", "\u{9ede}\u{64ca}\u{653e}\u{7f6e} XZ \u{8ef8}\u{7dda}"),
                        ("\u{67f1}", "\u{9ede}\u{64ca}\u{653e}\u{7f6e} H \u{578b}\u{92fc}\u{67f1}"),
                        ("\u{6a11}", "\u{5169}\u{9ede}\u{5efa}\u{7acb} H \u{578b}\u{92fc}\u{6a11}"),
                        ("\u{659c}\u{64d0}", "\u{5169}\u{9ede}\u{9023}\u{7dda}"),
                        ("\u{92fc}\u{677f}", "\u{756b}\u{77e9}\u{5f62}\u{2192}\u{63a8}\u{62c9}\u{539a}\u{5ea6}"),
                        ("\u{63a5}\u{982d}", "\u{9078}\u{5169}\u{69cb}\u{4ef6}\u{6a19}\u{8a18}"),
                    ];
                    for (key, desc) in steel_help {
                        ui.painter().text(egui::pos2(x3, y), egui::Align2::LEFT_TOP, *key, body_font.clone(), key_color);
                        ui.painter().text(egui::pos2(x3 + 85.0, y), egui::Align2::LEFT_TOP, *desc, body_font.clone(), text_color);
                        y += line_h;
                    }

                    y += 10.0;
                    ui.painter().text(egui::pos2(x3, y), egui::Align2::LEFT_TOP, "\u{1f4c1} \u{6a94}\u{6848}\u{652f}\u{63f4}", header_font.clone(), header_color);
                    y += line_h + 4.0;
                    let file_help: &[(&str, &str)] = &[
                        ("K3D", "Kolibri \u{539f}\u{751f}\u{683c}\u{5f0f}"),
                        ("DXF", "\u{667a}\u{6167}\u{532f}\u{5165}\u{ff08}\u{8ef8}\u{7dda}/\u{67f1}\u{6a11}\u{ff09}"),
                        ("DWG", "\u{57fa}\u{790e}\u{5e7e}\u{4f55}\u{532f}\u{5165}"),
                        ("OBJ/STL", "3D \u{6a21}\u{578b}\u{532f}\u{51fa}\u{5165}"),
                        ("GLTF", "Web/\u{904a}\u{6232}\u{5f15}\u{64ce}\u{532f}\u{51fa}"),
                        ("PNG/JPG", "\u{622a}\u{5716}\u{532f}\u{51fa}"),
                    ];
                    for (key, desc) in file_help {
                        ui.painter().text(egui::pos2(x3, y), egui::Align2::LEFT_TOP, *key, body_font.clone(), key_color);
                        ui.painter().text(egui::pos2(x3 + 60.0, y), egui::Align2::LEFT_TOP, *desc, body_font.clone(), text_color);
                        y += line_h;
                    }

                    // Bottom note
                    ui.painter().text(egui::pos2(help_rect.center().x, help_rect.bottom() - 20.0),
                        egui::Align2::CENTER_BOTTOM, "\u{6309} F1 \u{6216} ESC \u{95dc}\u{9589}\u{8aaa}\u{660e}",
                        body_font.clone(), key_color);

                    // ESC also closes help
                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.viewer.show_help = false;
                    }
                }

                // AI Suggestion popup
                if let Some(ref suggestion) = self.editor.suggestion.clone() {
                    let popup_rect = egui::Rect::from_center_size(
                        egui::pos2(rect.center().x, rect.max.y - 60.0),
                        egui::vec2(350.0, 40.0),
                    );
                    ui.painter().rect_filled(popup_rect, 6.0, egui::Color32::from_rgba_unmultiplied(40, 40, 50, 230));
                    ui.painter().rect_stroke(popup_rect, 6.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 180, 255)));

                    ui.painter().text(
                        popup_rect.left_center() + egui::vec2(10.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        &format!("\u{1f4a1} {}", suggestion.message),
                        egui::FontId::proportional(13.0),
                        egui::Color32::from_rgb(200, 220, 255),
                    );

                    ui.painter().text(
                        popup_rect.right_center() + egui::vec2(-10.0, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        "Y:接受 N:忽略",
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_gray(140),
                    );
                }
            });

        // ── WASD walk mode when Orbit tool is active ──
        if self.editor.tool == Tool::Orbit && !ctx.wants_keyboard_input() {
            let walk_speed = self.viewer.camera.distance * 0.005;
            ctx.input(|i| {
                if i.key_down(egui::Key::W) { self.viewer.camera.walk_forward(walk_speed); }
                if i.key_down(egui::Key::S) { self.viewer.camera.walk_forward(-walk_speed); }
                if i.key_down(egui::Key::A) { self.viewer.camera.walk_strafe(-walk_speed); }
                if i.key_down(egui::Key::D) { self.viewer.camera.walk_strafe(walk_speed); }
            });
        }

        // ── TAB = apply AI suggestion ──
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
                if let Some(ref suggestion) = self.editor.cursor_hint.ai_suggestion {
                    self.file_message = Some((format!("AI \u{5efa}\u{8b70}\u{5df2}\u{5957}\u{7528}: {}", suggestion), std::time::Instant::now()));
                }
            }
        }

        // ── F1 = toggle help overlay ──
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.viewer.show_help = !self.viewer.show_help;
        }

        // ── F12 = toggle console panel ──
        if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            self.viewer.show_console = !self.viewer.show_console;
        }

        // ── Perspective/Ortho toggle (Num5 key) ──
        if !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Num5) {
                    self.viewer.use_ortho = !self.viewer.use_ortho;
                }
                // Restore saved cameras with Num6-Num9
                if i.key_pressed(egui::Key::Num6) {
                    if let Some((_, cam)) = self.viewer.saved_cameras.get(0) {
                        self.viewer.camera = cam.clone();
                    }
                }
                if i.key_pressed(egui::Key::Num7) {
                    if let Some((_, cam)) = self.viewer.saved_cameras.get(1) {
                        self.viewer.camera = cam.clone();
                    }
                }
                if i.key_pressed(egui::Key::Num8) {
                    if let Some((_, cam)) = self.viewer.saved_cameras.get(2) {
                        self.viewer.camera = cam.clone();
                    }
                }
                if i.key_pressed(egui::Key::Num9) {
                    if let Some((_, cam)) = self.viewer.saved_cameras.get(3) {
                        self.viewer.camera = cam.clone();
                    }
                }
            });
        }

        // ── Cursor feedback based on active tool ──
        ctx.output_mut(|o| {
            o.cursor_icon = match self.editor.tool {
                Tool::Select => egui::CursorIcon::Default,
                Tool::Move => egui::CursorIcon::Move,
                Tool::Rotate => egui::CursorIcon::Alias,
                Tool::Scale => egui::CursorIcon::ResizeNeSw,
                Tool::Line | Tool::Arc | Tool::Rectangle | Tool::Circle => egui::CursorIcon::Crosshair,
                Tool::CreateBox | Tool::CreateCylinder | Tool::CreateSphere => egui::CursorIcon::Crosshair,
                Tool::PushPull => egui::CursorIcon::ResizeVertical,
                Tool::Eraser => egui::CursorIcon::NotAllowed,
                Tool::PaintBucket => egui::CursorIcon::PointingHand,
                Tool::TapeMeasure | Tool::Dimension => egui::CursorIcon::Crosshair,
                Tool::Text => egui::CursorIcon::Text,
                Tool::Orbit => egui::CursorIcon::Grab,
                Tool::Pan => egui::CursorIcon::AllScroll,
                _ => egui::CursorIcon::Default,
            };
        });

        // ── Auto-save check ──
        self.check_auto_save();

        // ── Test bridge: poll for commands ──
        self.poll_test_bridge();

        ctx.request_repaint();
    }
}

// ─── Arc geometry ────────────────────────────────────────────────────────────

// ─── CJK Fonts ───────────────────────────────────────────────────────────────

// ── MCP command handler ──────────────────────────────────────────────────────

impl KolibriApp {
    pub(crate) fn handle_mcp_command(&mut self, cmd: crate::mcp_server::McpCommand) -> crate::mcp_server::McpResult {
        use crate::mcp_server::{McpCommand, McpResult};
        use serde_json::json;

        let actor = crate::ai_log::ActorId::claude();

        match cmd {
            McpCommand::GetSceneState => {
                let objects: Vec<serde_json::Value> = self.scene.objects.values().map(|obj| {
                    json!({
                        "id": obj.id,
                        "name": obj.name,
                        "position": obj.position,
                        "shape": format!("{:?}", obj.shape),
                        "material": format!("{:?}", obj.material),
                    })
                }).collect();
                McpResult { success: true, data: json!({ "objects": objects, "count": objects.len() }) }
            }
            McpCommand::CreateBox { name, position, width, height, depth, material } => {
                self.scene.snapshot();
                let mat = parse_material_name(&material);
                let n = if name.is_empty() { self.next_name("Box") } else { name };
                let id = self.scene.add_box(n, position, width, height, depth, mat);
                self.ai_log.log(&actor, "\u{5efa}\u{7acb}\u{65b9}\u{584a}", &format!("{:.0}\u{00d7}{:.0}\u{00d7}{:.0}", width, height, depth), vec![id.clone()]);
                McpResult { success: true, data: json!({ "id": id }) }
            }
            McpCommand::CreateCylinder { name, position, radius, height, material } => {
                self.scene.snapshot();
                let mat = parse_material_name(&material);
                let n = if name.is_empty() { self.next_name("Cylinder") } else { name };
                let id = self.scene.add_cylinder(n, position, radius, height, 48, mat);
                self.ai_log.log(&actor, "\u{5efa}\u{7acb}\u{5713}\u{67f1}", &format!("r={:.0} h={:.0}", radius, height), vec![id.clone()]);
                McpResult { success: true, data: json!({ "id": id }) }
            }
            McpCommand::CreateSphere { name, position, radius, material } => {
                self.scene.snapshot();
                let mat = parse_material_name(&material);
                let n = if name.is_empty() { self.next_name("Sphere") } else { name };
                let id = self.scene.add_sphere(n, position, radius, 32, mat);
                self.ai_log.log(&actor, "\u{5efa}\u{7acb}\u{7403}\u{9ad4}", &format!("r={:.0}", radius), vec![id.clone()]);
                McpResult { success: true, data: json!({ "id": id }) }
            }
            McpCommand::DeleteObject { id } => {
                self.scene.snapshot();
                self.scene.delete(&id);
                self.ai_log.log(&actor, "\u{522a}\u{9664}\u{7269}\u{4ef6}", &id, vec![id.clone()]);
                McpResult { success: true, data: json!({ "deleted": id }) }
            }
            McpCommand::MoveObject { id, position } => {
                self.scene.snapshot();
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    obj.position = position;
                    self.scene.version += 1;
                    self.ai_log.log(&actor, "\u{79fb}\u{52d5}\u{7269}\u{4ef6}", &format!("{:?}", position), vec![id.clone()]);
                    McpResult { success: true, data: json!({ "moved": id }) }
                } else {
                    McpResult { success: false, data: json!({ "error": "Object not found" }) }
                }
            }
            McpCommand::SetMaterial { id, material } => {
                self.scene.snapshot();
                let mat = parse_material_name(&material);
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    obj.material = mat;
                    self.scene.version += 1;
                    self.ai_log.log(&actor, "\u{8a2d}\u{5b9a}\u{6750}\u{8cea}", &material, vec![id.clone()]);
                    McpResult { success: true, data: json!({ "updated": id }) }
                } else {
                    McpResult { success: false, data: json!({ "error": "Object not found" }) }
                }
            }
            McpCommand::ClearScene => {
                self.scene.snapshot();
                let count = self.scene.objects.len();
                self.scene.objects.clear();
                self.scene.version += 1;
                self.editor.selected_ids.clear();
                self.ai_log.log(&actor, "清空場景", &format!("{} objects removed", count), vec![]);
                McpResult { success: true, data: json!({ "cleared": count }) }
            }
            McpCommand::RotateObject { id, angle_deg } => {
                self.scene.snapshot_ids(&[&id], "MCP旋轉");
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    obj.rotation_y += angle_deg.to_radians();
                    self.scene.version += 1;
                    self.ai_log.log(&actor, "旋轉物件", &format!("{} {:.0}°", id, angle_deg), vec![id.clone()]);
                    McpResult { success: true, data: json!({ "rotated": id, "angle_deg": angle_deg }) }
                } else {
                    McpResult { success: false, data: json!({ "error": "Object not found" }) }
                }
            }
            McpCommand::ScaleObject { id, factor } => {
                self.scene.snapshot_ids(&[&id], "MCP縮放");
                if let Some(obj) = self.scene.objects.get_mut(&id) {
                    match &mut obj.shape {
                        Shape::Box { width, height, depth } => {
                            *width *= factor[0]; *height *= factor[1]; *depth *= factor[2];
                        }
                        Shape::Cylinder { radius, height, .. } => {
                            *radius *= factor[0]; *height *= factor[1];
                        }
                        Shape::Sphere { radius, .. } => {
                            *radius *= factor[0];
                        }
                        _ => {}
                    }
                    self.scene.version += 1;
                    self.ai_log.log(&actor, "縮放物件", &format!("{} x[{:.2},{:.2},{:.2}]", id, factor[0], factor[1], factor[2]), vec![id.clone()]);
                    McpResult { success: true, data: json!({ "scaled": id }) }
                } else {
                    McpResult { success: false, data: json!({ "error": "Object not found" }) }
                }
            }
            McpCommand::DuplicateObject { id, offset } => {
                if let Some(obj) = self.scene.objects.get(&id).cloned() {
                    self.scene.snapshot();
                    let mut clone = obj;
                    clone.id = self.scene.next_id_pub();
                    clone.name = format!("{}_copy", clone.name);
                    clone.position[0] += offset[0];
                    clone.position[1] += offset[1];
                    clone.position[2] += offset[2];
                    let new_id = clone.id.clone();
                    self.scene.objects.insert(new_id.clone(), clone);
                    self.scene.version += 1;
                    self.ai_log.log(&actor, "複製物件", &format!("{} → {}", id, new_id), vec![new_id.clone()]);
                    McpResult { success: true, data: json!({ "original": id, "copy_id": new_id }) }
                } else {
                    McpResult { success: false, data: json!({ "error": "Object not found" }) }
                }
            }
            McpCommand::GetObjectInfo { id } => {
                if let Some(obj) = self.scene.objects.get(&id) {
                    let shape_info = match &obj.shape {
                        Shape::Box { width, height, depth } => json!({"type":"box","width":width,"height":height,"depth":depth}),
                        Shape::Cylinder { radius, height, segments } => json!({"type":"cylinder","radius":radius,"height":height,"segments":segments}),
                        Shape::Sphere { radius, segments } => json!({"type":"sphere","radius":radius,"segments":segments}),
                        Shape::Line { points, thickness, .. } => json!({"type":"line","point_count":points.len(),"thickness":thickness}),
                        Shape::Mesh(mesh) => json!({"type":"mesh","vertices":mesh.vertices.len(),"faces":mesh.faces.len(),"edges":mesh.edges.len()}),
                    };
                    McpResult { success: true, data: json!({
                        "id": obj.id, "name": obj.name, "position": obj.position,
                        "rotation_y_deg": obj.rotation_y.to_degrees(),
                        "material": obj.material.label(),
                        "tag": obj.tag, "visible": obj.visible,
                        "roughness": obj.roughness, "metallic": obj.metallic,
                        "shape": shape_info,
                    }) }
                } else {
                    McpResult { success: false, data: json!({ "error": "Object not found" }) }
                }
            }
            McpCommand::Undo => {
                let ok = self.scene.undo();
                McpResult { success: ok, data: json!({ "undo": ok, "undo_count": self.scene.undo_count() }) }
            }
            McpCommand::Redo => {
                let ok = self.scene.redo();
                McpResult { success: ok, data: json!({ "redo": ok, "redo_count": self.scene.redo_count() }) }
            }
            McpCommand::Shutdown => {
                self.ai_log.log(&actor, "關閉應用", "MCP shutdown", vec![]);
                // 延遲一小段時間讓回應送出後再結束
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    std::process::exit(0);
                });
                McpResult { success: true, data: json!({ "message": "Shutting down..." }) }
            }
        }
    }
}

fn parse_material_name(name: &str) -> MaterialKind {
    match name.to_lowercase().as_str() {
        "concrete" => MaterialKind::Concrete,
        "wood" => MaterialKind::Wood,
        "glass" => MaterialKind::Glass,
        "metal" => MaterialKind::Metal,
        "brick" => MaterialKind::Brick,
        "white" => MaterialKind::White,
        "black" => MaterialKind::Black,
        "stone" => MaterialKind::Stone,
        "marble" => MaterialKind::Marble,
        "steel" => MaterialKind::Steel,
        "aluminum" => MaterialKind::Aluminum,
        "copper" => MaterialKind::Copper,
        "gold" => MaterialKind::Gold,
        "grass" => MaterialKind::Grass,
        "tile" => MaterialKind::Tile,
        "plaster" => MaterialKind::Plaster,
        _ => MaterialKind::Concrete,
    }
}

fn setup_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for path in &["C:/Windows/Fonts/msjh.ttc","C:/Windows/Fonts/msyh.ttc","C:/Windows/Fonts/mingliu.ttc"] {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert("cjk".into(), egui::FontData::from_owned(data));
            if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) { f.insert(0, "cjk".into()); }
            if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Monospace) { f.push("cjk".into()); }
            break;
        }
    }
    ctx.set_fonts(fonts);
}

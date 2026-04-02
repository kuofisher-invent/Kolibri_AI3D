use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use eframe::{egui, wgpu};
use eframe::epaint::mutex::RwLock;
use serde::Serialize;
mod update;
mod update_ui;
mod import_tasks;
mod commands;
mod mcp_handler;

use crate::camera::{self, OrbitCamera};
use crate::renderer::ViewportRenderer;
use crate::scene::{MaterialKind, Scene, Shape};

// ── Re-export types from editor.rs / viewer.rs / overlay.rs ──────────────────
// 保持 `use crate::app::Tool` 等既有 import 路徑不變
pub(crate) use crate::editor::{
    Tool, WorkMode, DrawState, ScaleHandle, PullFace, SnapType, SnapResult,
    AiSuggestion, SuggestionAction, RightTab, CursorHint, EditorState, SelectionMode,
};
#[cfg(feature = "drafting")]
#[allow(unused_imports)]
pub(crate) use crate::editor::{DraftDrawState, RibbonTab};
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

    // DXF/DWG 匯入模式選擇對話框
    #[cfg(feature = "drafting")]
    pub(crate) pending_import_path: Option<String>,
    #[cfg(feature = "drafting")]
    pub(crate) show_import_mode_dialog: bool,

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
    pub(crate) import_object_debug: std::collections::HashMap<String, crate::import::import_manager::ImportedObjectDebug>,
    pub(crate) background_task_rx: Option<Receiver<BackgroundTaskResult>>,
    pub(crate) background_task_label: Option<String>,
    pub(crate) background_task_started_at: Option<std::time::Instant>,
    pub(crate) defer_auto_save_until: Option<std::time::Instant>,
    pub(crate) startup_scene_path: Option<String>,
    pub(crate) startup_scene_attempted: bool,
    pub(crate) startup_screenshot_path: Option<String>,
    pub(crate) startup_screenshot_delay_frames: u32,
    pub(crate) startup_screenshot_attempts: u32,
    pub(crate) startup_screenshot_requested: bool,
    pub(crate) startup_screenshot_wait_logged: bool,
    pub(crate) startup_screenshot_missing_logged: bool,
    pub(crate) startup_screenshot_completed: bool,

    // Texture manager
    pub(crate) texture_manager: crate::texture_manager::TextureManager,

    // Spatial index for fast pick()
    pub(crate) spatial_index: Option<rstar::RTree<SpatialEntry>>,
    pub(crate) spatial_index_version: u64,

    // ── Damage-based redraw ──
    pub(crate) cached_repaint_version: u64,

    // ── Performance monitor ──
    pub(crate) perf_frame_times: std::collections::VecDeque<f32>, // 最近 120 幀的時間（ms）
    pub(crate) perf_last_frame: std::time::Instant,
    pub(crate) perf_ram_mb: f32,
    pub(crate) perf_ram_update: std::time::Instant,
    pub(crate) perf_gpu_verts: usize,
    pub(crate) perf_gpu_idx: usize,
    pub(crate) perf_mesh_build_ms: f32,

    // GPU info
    pub(crate) gpu_name: String,

    // Help tab category
    pub(crate) help_category: u8,

    // SVG icon cache（出圖模式 Ribbon 用）
    #[cfg(feature = "drafting")]
    pub(crate) svg_icons: crate::svg_icons::SvgIconCache,
}

/// rstar entry: AABB + object ID
#[derive(Debug, Clone)]
pub(crate) struct SpatialEntry {
    pub id: String,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

pub(crate) struct BackgroundSceneBuild {
    pub scene: Scene,
    pub result: crate::import::import_manager::BuildResult,
    pub duration: std::time::Duration,
    pub replace_scene: bool,
    pub skip_zoom_extents: bool,
    pub defer_auto_save: bool,
    pub source_format: String,
}

pub(crate) enum BackgroundTaskResult {
    Import(Result<crate::import::unified_ir::UnifiedIR, String>),
    Build(Result<BackgroundSceneBuild, String>),
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
        // GPU adapter info — HighPerformance 已在 NativeOptions 設定，自動選最強 GPU
        let gpu_info = rs.adapter.get_info();
        let gpu_name = format!("{} ({})", gpu_info.name, match gpu_info.device_type {
            wgpu::DeviceType::DiscreteGpu => "獨顯",
            wgpu::DeviceType::IntegratedGpu => "內顯",
            wgpu::DeviceType::VirtualGpu => "虛擬",
            wgpu::DeviceType::Cpu => "CPU",
            _ => "其他",
        });
        eprintln!("[GPU] 使用: {} | Backend: {:?}", gpu_name, gpu_info.backend);

        let mut app = Self {
            device: rs.device.clone(),
            queue: rs.queue.clone(),
            egui_renderer: rs.renderer.clone(),
            viewport: ViewportRenderer::new(&rs.device),

            scene: Scene::default(),

            viewer: ViewerState {
                camera: OrbitCamera::default(),
                render_mode: RenderMode::Shaded,
                edge_thickness: 1.0,
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
                ai_mode: false,
                layout: Default::default(),
                show_grid: true,
                show_axes: true,
                show_toolbar: true,
                show_right_panel: true,
                grid_spacing: 1000.0,
                dark_mode: false,
                language: 0, // 0=繁中, 1=English
                current_floor: 0,
                floor_height: 3000.0,
                work_plane: 0,
                work_plane_offset: 0.0,
                show_vertex_ids: false,
                section_plane_enabled: false,
                section_plane_axis: 1, // Y-axis default
                section_plane_offset: 3000.0, // 3m default
                section_plane_flip: false,
                camera_anim_from: None,
                camera_anim_to: None,
                camera_anim_start: None,
                camera_anim_duration: 0.3,
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
                editing_component_def_id: None,
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
                steel_profile: "H300×150×6.5×9".into(),
                steel_material: "SS400".into(),
                steel_height: 4200.0,
                ground_level: 0.0,
                floor_levels: vec![
                    ("GL".into(), 0.0),
                    ("1FL".into(), 4200.0),
                ],
                active_floor: 0,
                collision_warning: None,
                editing_dim_idx: None,
                editing_dim_text: String::new(),
                clipboard: Vec::new(),
                selection_mode: SelectionMode::Object,
                snap_threshold: 18.0,
                recent_materials: vec![
                    crate::scene::MaterialKind::Concrete,
                    crate::scene::MaterialKind::Wood,
                    crate::scene::MaterialKind::Steel,
                    crate::scene::MaterialKind::Glass,
                    crate::scene::MaterialKind::White,
                ],
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
                nearby_snaps: Vec::new(),
                #[cfg(feature = "steel")]
                conn_bolt_size: kolibri_core::steel_connection::BoltSize::M20,
                #[cfg(feature = "steel")]
                conn_bolt_grade: kolibri_core::steel_connection::BoltGrade::F10T,
                #[cfg(feature = "steel")]
                conn_add_stiffeners: true,
                #[cfg(feature = "steel")]
                conn_weld_type: kolibri_core::steel_connection::WeldType::Fillet,
                #[cfg(feature = "steel")]
                conn_weld_size: 6.0,
                #[cfg(feature = "steel")]
                conn_dialog: None,
                #[cfg(feature = "piping")]
                piping: kolibri_piping::PipingState::default(),
                #[cfg(feature = "drafting")]
                draft_state: crate::editor::DraftDrawState::Idle,
                #[cfg(feature = "drafting")]
                draft_doc: kolibri_drafting::DraftDocument::new(),
                #[cfg(feature = "drafting")]
                draft_layers: kolibri_drafting::LayerManager::default(),
                #[cfg(feature = "drafting")]
                draft_selected: Vec::new(),
                #[cfg(feature = "drafting")]
                ribbon_tab: crate::editor::RibbonTab::Home,
                #[cfg(feature = "drafting")]
                show_layer_manager: false,
                #[cfg(feature = "drafting")]
                draft_prop_color_idx: 0,
                #[cfg(feature = "drafting")]
                draft_prop_linetype_idx: 0,
                #[cfg(feature = "drafting")]
                draft_prop_lineweight_idx: 0,
                #[cfg(feature = "drafting")]
                draft_fillet_radius: 5.0,
                #[cfg(feature = "drafting")]
                draft_chamfer_dist: 5.0,
                #[cfg(feature = "drafting")]
                draft_text_input: String::new(),
                #[cfg(feature = "drafting")]
                draft_text_height: 3.5,
                #[cfg(feature = "drafting")]
                show_text_editor: false,
                #[cfg(feature = "drafting")]
                draft_text_place: None,
                #[cfg(feature = "drafting")]
                draft_blocks: std::collections::HashMap::new(),
                #[cfg(feature = "drafting")]
                draft_block_name: String::new(),
                #[cfg(feature = "drafting")]
                draft_transform_base: None,
                #[cfg(feature = "drafting")]
                draft_fillet_first: None,
                #[cfg(feature = "drafting")]
                draft_ortho: false,
                #[cfg(feature = "drafting")]
                draft_polar: true,
                #[cfg(feature = "drafting")]
                draft_dyn_input: true,
                #[cfg(feature = "drafting")]
                draft_osnap: true,
                #[cfg(feature = "drafting")]
                draft_sheets: vec![("Drawing1".to_string(), kolibri_drafting::DraftDocument::new())],
                #[cfg(feature = "drafting")]
                draft_active_sheet: 0,
                #[cfg(feature = "drafting")]
                draft_cmd_buf: String::new(),
                #[cfg(feature = "drafting")]
                draft_cmd_time: std::time::Instant::now(),
                #[cfg(feature = "drafting")]
                draft_num_input: String::new(),
                #[cfg(feature = "drafting")]
                draft_zoom: 2.0,
                #[cfg(feature = "drafting")]
                draft_offset: egui::Vec2::ZERO,
                #[cfg(feature = "drafting")]
                draft_pan_drag: None,
                #[cfg(feature = "drafting")]
                draft_needs_zoom_all: false,
                #[cfg(feature = "drafting")]
                draft_zoom_all_delay: 0,
                #[cfg(feature = "drafting")]
                grip_edit_mode: crate::editor::GripEditMode::Stretch,
                #[cfg(feature = "drafting")]
                grip_hot_idx: None,
                #[cfg(feature = "drafting")]
                grip_base_point: None,
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
            #[cfg(feature = "drafting")]
            pending_import_path: None,
            #[cfg(feature = "drafting")]
            show_import_mode_dialog: false,
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
            import_object_debug: std::collections::HashMap::new(),
            background_task_rx: None,
            background_task_label: None,
            background_task_started_at: None,
            defer_auto_save_until: None,
            startup_scene_path: std::env::var("KOLIBRI_STARTUP_SCENE").ok(),
            startup_scene_attempted: false,
            startup_screenshot_path: std::env::var("KOLIBRI_STARTUP_SCREENSHOT_OUT").ok(),
            startup_screenshot_delay_frames: 0,
            startup_screenshot_attempts: 0,
            startup_screenshot_requested: false,
            startup_screenshot_wait_logged: false,
            startup_screenshot_missing_logged: false,
            startup_screenshot_completed: false,
            texture_manager: crate::texture_manager::TextureManager::new(),
            spatial_index: None,
            spatial_index_version: u64::MAX,
            cached_repaint_version: u64::MAX,
            // Performance monitor
            perf_frame_times: std::collections::VecDeque::with_capacity(120),
            perf_last_frame: std::time::Instant::now(),
            perf_ram_mb: 0.0,
            perf_ram_update: std::time::Instant::now(),
            perf_gpu_verts: 0,
            perf_gpu_idx: 0,
            perf_mesh_build_ms: 0.0,
            gpu_name,
            help_category: 0,
            #[cfg(feature = "drafting")]
            svg_icons: crate::svg_icons::SvgIconCache::new(),
        };
        // 預載 SVG icons
        #[cfg(feature = "drafting")]
        app.svg_icons.preload_ribbon_icons(&cc.egui_ctx);

        app
    }

    /// 切換到 2D CAD 出圖模式
    pub(crate) fn enter_layout_mode(&mut self) {
        if self.viewer.layout_mode { return; }
        self.viewer.layout_mode = true;
        #[cfg(feature = "drafting")]
        {
            self.editor.tool = Tool::DraftSelect;
            // 清空 2D 畫布（新的 Drawing 分頁）
            self.editor.draft_doc = kolibri_drafting::DraftDocument::new();
            self.editor.draft_selected.clear();
            self.editor.draft_state = crate::editor::DraftDrawState::Idle;
            self.editor.draft_zoom = 2.0;
            self.editor.draft_offset = egui::Vec2::ZERO;
            self.editor.grip_edit_mode = crate::editor::GripEditMode::Stretch;
            // 重設分頁：只保留一個空白 Drawing
            self.editor.draft_sheets = vec![("Drawing1".to_string(), kolibri_drafting::DraftDocument::new())];
            self.editor.draft_active_sheet = 0;
        }
    }

    /// 切回 3D 建模模式
    pub(crate) fn exit_layout_mode(&mut self) {
        if !self.viewer.layout_mode { return; }
        self.viewer.layout_mode = false;
        // 將工具重設為 3D Select，避免殘留 Draft* 工具
        self.editor.tool = Tool::Select;
    }

    /// 匯入 DXF/DWG 到 2D 畫布，自動建立以檔名命名的新分頁（ZWCAD 風格）
    #[cfg(feature = "drafting")]
    pub(crate) fn import_cad_to_2d_tab(&mut self, file_path: &str) -> Result<usize, String> {
        // 取得檔名（不含路徑和副檔名）
        let file_name = file_path
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(file_path)
            .rsplit_once('.')
            .map(|(name, _)| name)
            .unwrap_or(file_path)
            .to_string();

        // 建立新的 DraftDocument 並匯入
        let mut new_doc = kolibri_drafting::DraftDocument::new();
        let count = crate::dxf_io::import_cad_to_draft(&mut new_doc, file_path)?;

        // 儲存目前 active sheet
        let active = self.editor.draft_active_sheet;
        if active < self.editor.draft_sheets.len() {
            self.editor.draft_sheets[active].1 = self.editor.draft_doc.clone();
        }

        // 新增分頁，命名為檔名
        self.editor.draft_sheets.push((file_name.clone(), new_doc.clone()));
        let new_idx = self.editor.draft_sheets.len() - 1;

        // 切換到新分頁
        self.editor.draft_active_sheet = new_idx;
        self.editor.draft_doc = new_doc;
        self.editor.draft_selected.clear();

        // 同步 DXF 中的圖層定義到 LayerManager
        // DWG 檔案先被轉為 .tmp.dxf，需要找到實際的 DXF 路徑
        let dxf_path = if file_path.to_lowercase().ends_with(".dwg") {
            let tmp = format!("{}.tmp.dxf", file_path);
            let sibling = file_path.rsplit_once('.').map(|(b, _)| format!("{}.dxf", b)).unwrap_or_default();
            if std::path::Path::new(&tmp).exists() { tmp }
            else if std::path::Path::new(&sibling).exists() { sibling }
            else { file_path.to_string() }
        } else {
            file_path.to_string()
        };
        let dxf_layers = crate::dxf_io::parse_dxf_layers(&dxf_path);
        for (name, color) in &dxf_layers {
            self.editor.draft_layers.add(kolibri_drafting::DraftLayer::new(name, *color));
        }
        if !dxf_layers.is_empty() {
            self.console_push("INFO", format!("已同步 {} 個圖層", dxf_layers.len()));
        }

        // 確保在 2D 模式
        if !self.viewer.layout_mode {
            self.viewer.layout_mode = true;
            self.editor.tool = Tool::DraftSelect;
        }

        // 自動 Zoom All — 延遲 3 幀，等 layout 穩定
        self.editor.draft_needs_zoom_all = true;
        self.editor.draft_zoom_all_delay = 3;

        self.console_push("ACTION", format!("[2D] 已匯入 {} 個圖元到分頁「{}」", count, file_name));
        Ok(count)
    }

    /// F6 切換 2D/3D
    pub(crate) fn toggle_layout_mode(&mut self) {
        if self.viewer.layout_mode {
            self.exit_layout_mode();
        } else {
            self.enter_layout_mode();
        }
    }
}

pub(crate) fn parse_material_name(name: &str) -> MaterialKind {
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

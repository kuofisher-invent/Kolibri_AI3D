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
            },

            right_tab: RightTab::Properties,
            create_mat: MaterialKind::Concrete,
            obj_counter: 0,
            current_file: None,
            file_message: None,
            recent_files: Vec::new(),
            last_auto_save: std::time::Instant::now(),
            auto_save_version: 0,
            last_saved_version: 0,
            pending_action: None,
            ai_log: crate::ai_log::AiLog::new(),
            current_actor: crate::ai_log::ActorId::user(),
            mcp_bridge: None,
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

    pub(crate) fn next_name(&mut self, prefix: &str) -> String {
        self.obj_counter += 1;
        format!("{}_{}", prefix, self.obj_counter)
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

                // ── Draw guide/construction lines ──
                if !self.scene.guide_lines.is_empty() {
                    let guide_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(150, 50, 50, 180));
                    for (start, end) in &self.scene.guide_lines {
                        let p1 = Self::world_to_screen_vp(*start, &vp, &rect);
                        let p2 = Self::world_to_screen_vp(*end, &vp, &rect);
                        if let (Some(s1), Some(s2)) = (p1, p2) {
                            // Dashed guide line
                            let dir: egui::Vec2 = s2 - s1;
                            let total = dir.length();
                            if total > 1.0 {
                                let norm = dir / total;
                                let step: f32 = 10.0;
                                let mut d_val: f32 = 0.0;
                                while d_val < total {
                                    let a_pt = s1 + norm * d_val;
                                    let b_pt = s1 + norm * (d_val + step * 0.6).min(total);
                                    ui.painter().line_segment([a_pt, b_pt], guide_stroke);
                                    d_val += step;
                                }
                            }
                        }
                    }
                }

                // ── A6: Track move origin for rubber band ──
                if self.editor.tool == Tool::Move && self.editor.drag_snapshot_taken && !self.editor.selected_ids.is_empty() {
                    // Move drag is in progress; capture origin if not yet set
                    if self.editor.move_origin.is_none() {
                        // Use the undo stack's last snapshot to get the original position
                        if let Some((prev_objects, _)) = self.scene.undo_stack.last() {
                            if let Some(obj) = prev_objects.get(&self.editor.selected_ids[0]) {
                                self.editor.move_origin = Some(obj.position);
                            }
                        }
                    }
                } else {
                    // Not in a move drag; clear origin
                    self.editor.move_origin = None;
                }

                // ── A2/A3: Cursor-following dimension during drag/push-pull ──
                self.editor.cursor_dimension = match &self.editor.draw_state {
                    DrawState::Pulling { obj_id, face, original_dim } => {
                        if let Some(obj) = self.scene.objects.get(obj_id) {
                            let current_dim = match (&obj.shape, face) {
                                (Shape::Box { height, .. }, PullFace::Top | PullFace::Bottom) => *height,
                                (Shape::Box { depth, .. }, PullFace::Front | PullFace::Back) => *depth,
                                (Shape::Box { width, .. }, PullFace::Left | PullFace::Right) => *width,
                                (Shape::Cylinder { height, .. }, _) => *height,
                                _ => 0.0,
                            };
                            let delta = current_dim - original_dim;
                            if delta.abs() > 0.5 {
                                Some((self.editor.mouse_screen[0] + 20.0, self.editor.mouse_screen[1] - 20.0,
                                    format!("拉伸 {:.0} mm", delta)))
                            } else { None }
                        } else { None }
                    }
                    DrawState::Scaling { ref obj_id, handle, original_dims } => {
                        if let Some(obj) = self.scene.objects.get(obj_id) {
                            let current = match &obj.shape {
                                Shape::Box { width, height, depth } => [*width, *height, *depth],
                                Shape::Cylinder { radius, height, .. } => [*radius * 2.0, *height, *radius * 2.0],
                                Shape::Sphere { radius, .. } => [*radius * 2.0, *radius * 2.0, *radius * 2.0],
                                _ => [0.0; 3],
                            };
                            let text = match handle {
                                ScaleHandle::Uniform => {
                                    let ratio = if original_dims[0] > 0.1 { current[0] / original_dims[0] } else { 1.0 };
                                    format!("\u{00d7}{:.2}", ratio)
                                }
                                ScaleHandle::AxisX => format!("W: {:.0} mm (\u{00d7}{:.2})", current[0], current[0] / original_dims[0].max(1.0)),
                                ScaleHandle::AxisY => format!("H: {:.0} mm (\u{00d7}{:.2})", current[1], current[1] / original_dims[1].max(1.0)),
                                ScaleHandle::AxisZ => format!("D: {:.0} mm (\u{00d7}{:.2})", current[2], current[2] / original_dims[2].max(1.0)),
                            };
                            Some((self.editor.mouse_screen[0] + 20.0, self.editor.mouse_screen[1] - 20.0, text))
                        } else { None }
                    }
                    _ => None,
                };

                // ── Build cursor hint ──
                {
                    // Detect tool change => trigger fade
                    if self.editor.tool != self.editor.prev_tool_for_hint {
                        self.editor.cursor_hint_fade = Some(std::time::Instant::now());
                        self.editor.prev_tool_for_hint = self.editor.tool;
                    }
                    // Fade-out after tool change
                    if let Some(fade_time) = self.editor.cursor_hint_fade {
                        if fade_time.elapsed().as_millis() > 300 {
                            self.editor.cursor_hint.active = false;
                            self.editor.cursor_hint_fade = None;
                        }
                    }

                    self.editor.cursor_hint = CursorHint::default();

                    let is_drawing = !matches!(self.editor.draw_state, DrawState::Idle)
                        || matches!(self.editor.tool, Tool::Line | Tool::Arc | Tool::Rectangle | Tool::Circle
                            | Tool::CreateBox | Tool::CreateCylinder | Tool::CreateSphere
                            | Tool::PushPull | Tool::Move | Tool::Rotate | Tool::Scale | Tool::TapeMeasure
                            | Tool::Dimension | Tool::Text);

                    if is_drawing {
                        self.editor.cursor_hint.active = true;

                        // Layer 1: Inference source
                        if let Some(ref snap) = self.editor.snap_result {
                            if snap.snap_type != SnapType::None && snap.snap_type != SnapType::Grid {
                                let (dot, label) = match snap.snap_type {
                                    SnapType::Endpoint => ("\u{1f7e2}", "Endpoint"),
                                    SnapType::Midpoint => ("\u{1f535}", "Midpoint"),
                                    SnapType::Origin => ("\u{1f7e0}", "Origin"),
                                    SnapType::AxisX => ("\u{1f534}", "On Red Axis"),
                                    SnapType::AxisY => ("\u{1f7e2}", "On Green Axis"),
                                    SnapType::AxisZ => ("\u{1f535}", "On Blue Axis"),
                                    SnapType::OnFace => ("\u{1f7e1}", "On Face"),
                                    SnapType::FaceCenter => ("\u{2795}", "Face Center"),
                                    SnapType::OnEdge => ("\u{1f534}", "On Edge"),
                                    SnapType::Perpendicular => ("\u{1f7e3}", "Perpendicular"),
                                    SnapType::Parallel => ("\u{1f7e3}", "Parallel to Edge"),
                                    SnapType::Intersection => ("\u{26aa}", "Intersection"),
                                    _ => ("", ""),
                                };
                                self.editor.cursor_hint.inference_label = format!("{} {}", dot, label);
                                self.editor.cursor_hint.inference_color = snap.snap_type.color();
                            }
                        }

                        // Layer 2: Distance
                        if let Some((_, _, ref text)) = self.editor.cursor_dimension {
                            self.editor.cursor_hint.distance_text = text.clone();
                        } else if let Some(ref snap) = self.editor.snap_result {
                            if let Some(from) = snap.from_point {
                                let p = snap.position;
                                let dx = p[0] - from[0];
                                let dy = p[1] - from[1];
                                let dz = p[2] - from[2];
                                let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                                if dist > 1.0 {
                                    self.editor.cursor_hint.distance_text = if dist >= 1000.0 {
                                        format!("\u{2194} {:.2} m", dist / 1000.0)
                                    } else {
                                        format!("\u{2194} {:.0} mm", dist)
                                    };
                                }
                            }
                        }

                        // Layer 3: Chips
                        if let Some(ref snap) = self.editor.snap_result {
                            if snap.snap_type != SnapType::None && snap.snap_type != SnapType::Grid {
                                self.editor.cursor_hint.chips.push((snap.snap_type.label().to_string(), false));
                            }
                        }
                        // Working plane chip
                        match self.editor.inference_ctx.working_plane {
                            crate::inference::WorkingPlane::Ground => {
                                self.editor.cursor_hint.chips.push(("Ground".to_string(), false));
                            }
                            crate::inference::WorkingPlane::FaceXZ(y) => {
                                self.editor.cursor_hint.chips.push((format!("Plane Y:{:.0}", y), false));
                            }
                            _ => {}
                        }
                        // AI inference chip
                        if let Some((ref label, ref source)) = self.editor.inference_label {
                            if *source != crate::inference::InferenceSource::Geometry {
                                self.editor.cursor_hint.chips.push((format!("\u{1f916} {}", label), true));
                                self.editor.cursor_hint.ai_suggestion = Some(label.clone());
                            }
                        }

                        // ── Inference Engine 2.0: score snap through formal pipeline ──
                        if let Some(ref snap) = self.editor.snap_result {
                            let engine_ctx = crate::inference_engine::InferenceContext {
                                current_tool: crate::inference_engine::tool_to_kind(self.editor.tool),
                                current_mode: if self.editor.work_mode == WorkMode::Steel {
                                    crate::inference_engine::AppMode::Steel
                                } else {
                                    crate::inference_engine::AppMode::Modeling
                                },
                                selected_ids: self.editor.selected_ids.clone(),
                                hover_id: self.editor.hovered_id.clone(),
                                last_direction: self.editor.last_line_dir,
                                last_action: self.editor.last_action_name.clone(),
                                working_plane_y: 0.0,
                                locked_axis: self.editor.locked_axis,
                                is_drawing: !matches!(self.editor.draw_state, DrawState::Idle),
                                consecutive_same_tool: 1,
                            };

                            let candidate = crate::inference_engine::InferenceCandidate {
                                id: "snap_0".into(),
                                inference_type: crate::inference_engine::snap_type_to_inference_type(&snap.snap_type),
                                position: snap.position,
                                source_object_id: None,
                                raw_distance: 5.0,
                            };

                            let scored = self.editor.inference_engine.score_candidates(&[candidate], &engine_ctx);
                            if let Some(top) = scored.first() {
                                if let Some(reason) = top.breakdown.reasons.first() {
                                    self.editor.cursor_hint.inference_label = format!(
                                        "{} [S:{:.0}]",
                                        reason,
                                        top.breakdown.total,
                                    );
                                }
                            }
                        }

                        // Ghost line: predicted direction from last drawn line
                        if let Some(dir) = self.editor.inference_ctx.last_direction {
                            if let Some(from) = self.get_drawing_origin() {
                                let extend = 2000.0;
                                let to = [from[0] + dir[0] * extend, from[1], from[2] + dir[1] * extend];
                                self.editor.cursor_hint.ghost_dir = Some((from, to));
                            }
                        }
                    }
                }

                // ── Cursor Hint Card ──
                if self.editor.cursor_hint.active && (!self.editor.cursor_hint.inference_label.is_empty() || !self.editor.cursor_hint.distance_text.is_empty()) {
                    let mx = self.editor.mouse_screen[0];
                    let my = self.editor.mouse_screen[1];
                    let card_x = rect.min.x + mx + 40.0;  // farther from cursor to avoid blocking
                    let card_y = rect.min.y + my - 60.0;  // above cursor, not overlapping

                    let painter = ui.painter();
                    let font_small = egui::FontId::proportional(11.0);
                    let font_chip = egui::FontId::proportional(10.0);

                    let text_dark = egui::Color32::from_rgb(31, 36, 48);
                    let text_muted = egui::Color32::from_rgb(110, 118, 135);
                    let brand = egui::Color32::from_rgb(76, 139, 245);
                    let bg = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 235);
                    let border = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200);

                    // Calculate card height
                    let mut card_h = 8.0; // top padding
                    let has_inference = !self.editor.cursor_hint.inference_label.is_empty();
                    let has_distance = !self.editor.cursor_hint.distance_text.is_empty();
                    let has_chips = !self.editor.cursor_hint.chips.is_empty();
                    let has_tab = self.editor.cursor_hint.ai_suggestion.is_some();

                    if has_inference { card_h += 18.0; }
                    if has_distance { card_h += 26.0; }
                    if has_chips { card_h += 22.0; }
                    if has_tab { card_h += 22.0; }
                    card_h += 8.0; // bottom padding

                    let card_w = 240.0;

                    // Clamp to viewport
                    let cx = card_x.min(rect.max.x - card_w - 10.0);
                    let cy = card_y.min(rect.max.y - card_h - 10.0).max(rect.min.y + 10.0);

                    let card_rect = egui::Rect::from_min_size(egui::pos2(cx, cy), egui::vec2(card_w, card_h));

                    // Shadow
                    painter.rect_filled(card_rect.translate(egui::vec2(2.0, 3.0)), 14.0,
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 20));
                    // Background
                    painter.rect_filled(card_rect, 14.0, bg);
                    painter.rect_stroke(card_rect, 14.0, egui::Stroke::new(1.0, border));

                    let mut y_pos = cy + 10.0;
                    let lx = cx + 14.0;

                    // Layer 1: Inference label
                    if has_inference {
                        painter.text(egui::pos2(lx, y_pos), egui::Align2::LEFT_TOP,
                            &self.editor.cursor_hint.inference_label, font_small.clone(), text_muted);
                        y_pos += 18.0;
                    }

                    // Layer 2: Distance (big, bold, brand color)
                    if has_distance {
                        painter.text(egui::pos2(lx, y_pos), egui::Align2::LEFT_TOP,
                            &self.editor.cursor_hint.distance_text,
                            egui::FontId { size: 18.0, family: egui::FontFamily::Proportional },
                            brand);
                        y_pos += 26.0;
                    }

                    // Layer 3: Chips
                    if has_chips {
                        let mut chip_x = lx;
                        for (label, is_ai) in &self.editor.cursor_hint.chips {
                            let galley = painter.layout_no_wrap(label.clone(), font_chip.clone(), text_dark);
                            let chip_w = galley.size().x + 12.0;
                            let chip_h = 18.0;
                            let chip_rect = egui::Rect::from_min_size(egui::pos2(chip_x, y_pos), egui::vec2(chip_w, chip_h));

                            let chip_bg = if *is_ai {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 30)
                            } else {
                                egui::Color32::from_rgb(240, 242, 248)
                            };
                            let chip_border = if *is_ai {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 80)
                            } else {
                                egui::Color32::from_rgb(229, 231, 239)
                            };

                            painter.rect_filled(chip_rect, 9.0, chip_bg);
                            painter.rect_stroke(chip_rect, 9.0, egui::Stroke::new(0.5, chip_border));
                            painter.galley(egui::pos2(chip_x + 6.0, y_pos + 1.0), galley,
                                if *is_ai { brand } else { text_muted });

                            chip_x += chip_w + 4.0;
                            if chip_x > cx + card_w - 20.0 { break; }
                        }
                        y_pos += 22.0;
                    }

                    // TAB hint (only when AI suggestion exists)
                    if has_tab {
                        let tab_text = "\u{6309} TAB \u{5957}\u{7528} AI \u{5efa}\u{8b70}";
                        let tab_bg = egui::Color32::from_rgba_unmultiplied(76, 139, 245, 20);
                        let tab_rect = egui::Rect::from_min_size(egui::pos2(lx, y_pos), egui::vec2(card_w - 28.0, 18.0));
                        painter.rect_filled(tab_rect, 9.0, tab_bg);
                        painter.text(tab_rect.center(), egui::Align2::CENTER_CENTER,
                            tab_text, font_chip.clone(), brand);
                    }
                }

                // ── Ghost line (predicted direction) ──
                if let Some((from, to)) = self.editor.cursor_hint.ghost_dir {
                    if let (Some(s1), Some(s2)) = (
                        self.world_to_screen(from, &rect),
                        self.world_to_screen(to, &rect),
                    ) {
                        let ghost_color = if let Some(ref snap) = self.editor.snap_result {
                            match snap.snap_type {
                                SnapType::AxisX => egui::Color32::from_rgba_unmultiplied(220, 60, 60, 80),
                                SnapType::AxisZ => egui::Color32::from_rgba_unmultiplied(60, 100, 220, 80),
                                _ => egui::Color32::from_rgba_unmultiplied(150, 200, 255, 60),
                            }
                        } else {
                            egui::Color32::from_rgba_unmultiplied(150, 200, 255, 60)
                        };
                        draw_dashed_line(ui.painter(), s1, s2, egui::Stroke::new(1.5, ghost_color), 8.0, 6.0);
                    }
                }

                // ── Collision warning overlay near cursor ──
                if let Some(ref warning) = self.editor.collision_warning {
                    let warn_pos = egui::pos2(
                        rect.min.x + self.editor.mouse_screen[0] + 20.0,
                        rect.min.y + self.editor.mouse_screen[1] + 30.0,
                    );
                    let font = egui::FontId::proportional(12.0);
                    let galley = ui.painter().layout_no_wrap(warning.clone(), font, egui::Color32::from_rgb(240, 70, 50));
                    let bg = egui::Rect::from_min_size(warn_pos, galley.size()).expand(4.0);
                    ui.painter().rect_filled(bg, 6.0, egui::Color32::from_rgba_unmultiplied(60, 10, 10, 220));
                    ui.painter().galley(warn_pos + egui::vec2(4.0, 4.0), galley, egui::Color32::from_rgb(255, 100, 80));
                }
                // Clear collision warning each frame
                self.editor.collision_warning = None;

                // ── A6: Move rubber band line from original position ──
                if let Some(origin) = self.editor.move_origin {
                    if !self.editor.selected_ids.is_empty() {
                        if let Some(obj) = self.scene.objects.get(&self.editor.selected_ids[0]) {
                            let p1 = self.world_to_screen(origin, &rect);
                            let p2 = self.world_to_screen(obj.position, &rect);
                            if let (Some(s1), Some(s2)) = (p1, p2) {
                                // Draw dashed rubber band line
                                let dir = s2 - s1;
                                let total = dir.length();
                                if total > 2.0 {
                                    let norm = dir / total;
                                    let step = 8.0;
                                    let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100, 150, 255, 150));
                                    let mut d_val = 0.0;
                                    while d_val < total {
                                        let a_pt = s1 + norm * d_val;
                                        let b_pt = s1 + norm * (d_val + step * 0.6).min(total);
                                        ui.painter().line_segment([a_pt, b_pt], stroke);
                                        d_val += step;
                                    }
                                }
                                // Show distance label at midpoint
                                let dx = obj.position[0] - origin[0];
                                let dy = obj.position[1] - origin[1];
                                let dz = obj.position[2] - origin[2];
                                let dist = (dx*dx + dy*dy + dz*dz).sqrt();
                                if dist > 1.0 {
                                    let mid = egui::pos2((s1.x+s2.x)*0.5, (s1.y+s2.y)*0.5 - 10.0);
                                    let label = if dist >= 1000.0 {
                                        format!("{:.2} m", dist / 1000.0)
                                    } else {
                                        format!("{:.0} mm", dist)
                                    };
                                    // Background
                                    let font = egui::FontId::proportional(11.0);
                                    let galley = ui.painter().layout_no_wrap(label, font, egui::Color32::from_gray(200));
                                    let bg_rect = egui::Rect::from_center_size(mid, galley.size()).expand(3.0);
                                    ui.painter().rect_filled(bg_rect, 2.0, egui::Color32::from_rgba_unmultiplied(30, 30, 40, 180));
                                    ui.painter().galley(bg_rect.min, galley, egui::Color32::from_gray(200));
                                }
                            }
                        }
                    }
                }

                // Show locked axis indicator with visual axis line through object
                if let Some(axis) = self.editor.locked_axis {
                    let (label, color) = match axis {
                        0 => ("X軸 ──", egui::Color32::from_rgb(240, 60, 60)),
                        1 => ("Y軸 │", egui::Color32::from_rgb(60, 200, 60)),
                        2 => ("Z軸 ──", egui::Color32::from_rgb(60, 60, 240)),
                        _ => ("", egui::Color32::WHITE),
                    };

                    // Text indicator at bottom-left
                    let pos = egui::pos2(rect.min.x + 10.0, rect.max.y - 30.0);
                    ui.painter().text(pos, egui::Align2::LEFT_BOTTOM, label,
                        egui::FontId::proportional(16.0), color);

                    // Draw axis line through selected object
                    if let Some(ref id) = self.editor.selected_ids.first() {
                        if let Some(obj) = self.scene.objects.get(*id) {
                            let p = obj.position;
                            let len = 5000.0;
                            let (a, b) = match axis {
                                0 => ([p[0]-len, p[1], p[2]], [p[0]+len, p[1], p[2]]),
                                1 => ([p[0], p[1]-len, p[2]], [p[0], p[1]+len, p[2]]),
                                2 => ([p[0], p[1], p[2]-len], [p[0], p[1], p[2]+len]),
                                _ => (p, p),
                            };
                            if let (Some(sa), Some(sb)) = (
                                self.world_to_screen(a, &rect),
                                self.world_to_screen(b, &rect),
                            ) {
                                let stroke = egui::Stroke::new(2.5, color);
                                // Dashed line
                                let dir = sb - sa;
                                let total = dir.length();
                                if total > 1.0 {
                                    let norm = dir / total;
                                    let step = 12.0;
                                    let mut d_val = 0.0;
                                    while d_val < total {
                                        let a_pt = sa + norm * d_val;
                                        let b_pt = sa + norm * (d_val + step * 0.6).min(total);
                                        ui.painter().line_segment([a_pt, b_pt], stroke);
                                        d_val += step;
                                    }
                                }
                            }
                        }
                    }
                }

                // Group isolation mode indicator + F3 exit button
                if let Some(ref gid) = self.editor.editing_group_id.clone() {
                    let label = if let Some(obj) = self.scene.objects.get(gid) {
                        format!("\u{1f512} 群組編輯: {}", obj.name)
                    } else {
                        "\u{1f512} 群組編輯模式".to_string()
                    };
                    let pos = egui::pos2(rect.center().x, rect.min.y + 25.0);
                    ui.painter().text(pos, egui::Align2::CENTER_TOP, &label,
                        egui::FontId::proportional(15.0),
                        egui::Color32::from_rgb(255, 200, 80));

                    // F3: "退出群組" floating button
                    let exit_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.center().x - 60.0, rect.top() + 50.0),
                        egui::vec2(120.0, 32.0),
                    );
                    ui.painter().rect_filled(exit_rect, 16.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 230));
                    ui.painter().rect_stroke(exit_rect, 16.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));
                    let exit_response = ui.allocate_rect(exit_rect, egui::Sense::click());
                    ui.painter().text(exit_rect.center(), egui::Align2::CENTER_CENTER,
                        "\u{21a9} \u{9000}\u{51fa}\u{7fa4}\u{7d44}", egui::FontId::proportional(12.0),
                        egui::Color32::from_rgb(76, 139, 245));
                    if exit_response.clicked() {
                        self.editor.editing_group_id = None;
                    }
                }

                // ── Floating material picker for PaintBucket ──
                if self.editor.tool == Tool::PaintBucket {
                    let swatch = 32.0_f32;
                    let gap = 4.0_f32;
                    let cols = 8_usize;
                    let all_mats = crate::scene::MaterialKind::ALL;
                    let rows = (all_mats.len() + cols - 1) / cols;
                    let panel_w = cols as f32 * (swatch + gap) + gap + 16.0;
                    let panel_h = rows as f32 * (swatch + gap) + gap + 36.0;
                    let panel_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.center().x - panel_w / 2.0, rect.bottom() - panel_h - 50.0),
                        egui::vec2(panel_w, panel_h),
                    );
                    ui.painter().rect_filled(panel_rect, 16.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 235));
                    ui.painter().rect_stroke(panel_rect, 16.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));
                    // Title
                    ui.painter().text(
                        egui::pos2(panel_rect.center().x, panel_rect.top() + 14.0),
                        egui::Align2::CENTER_CENTER,
                        format!("油漆桶 — 目前: {}", self.create_mat.label()),
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgb(110, 118, 135),
                    );
                    // Swatches
                    let start_x = panel_rect.left() + 8.0 + gap;
                    let start_y = panel_rect.top() + 28.0;
                    for (i, mat) in all_mats.iter().enumerate() {
                        let row = i / cols;
                        let col = i % cols;
                        let sx = start_x + col as f32 * (swatch + gap);
                        let sy = start_y + row as f32 * (swatch + gap);
                        let sr = egui::Rect::from_min_size(egui::pos2(sx, sy), egui::vec2(swatch, swatch));
                        let resp = ui.allocate_rect(sr, egui::Sense::click());
                        crate::panels::draw_material_swatch(
                            ui.painter(), sr, mat,
                            self.create_mat == *mat,
                            resp.hovered(),
                        );
                        if resp.clicked() {
                            self.create_mat = *mat;
                        }
                        resp.on_hover_text(mat.label());
                    }
                }

                // ── C3: Push/Pull reference dashed lines ──
                if self.editor.selected_face.is_some() && self.editor.drag_snapshot_taken {
                    if let Some((obj_id, _face)) = self.editor.selected_face.clone() {
                        if let (Some(orig_pos), Some(orig_dims), Some(obj)) = (
                            self.editor.pull_original_pos,
                            self.editor.pull_original_dims,
                            self.scene.objects.get(&obj_id),
                        ) {
                            if let Shape::Box { width, height, depth } = &obj.shape {
                                // 4 corners of the pulled face — compute in original and current positions
                                let ow = orig_dims[0];
                                let oh = orig_dims[1];
                                let od = orig_dims[2];
                                let op = orig_pos;
                                let cp = obj.position;
                                let cw = *width;
                                let ch = *height;
                                let cd = *depth;

                                // Draw lines from each original corner to current corner
                                let orig_corners = [
                                    [op[0], op[1], op[2]],
                                    [op[0]+ow, op[1], op[2]],
                                    [op[0]+ow, op[1]+oh, op[2]],
                                    [op[0], op[1]+oh, op[2]],
                                    [op[0], op[1], op[2]+od],
                                    [op[0]+ow, op[1], op[2]+od],
                                    [op[0]+ow, op[1]+oh, op[2]+od],
                                    [op[0], op[1]+oh, op[2]+od],
                                ];
                                let curr_corners = [
                                    [cp[0], cp[1], cp[2]],
                                    [cp[0]+cw, cp[1], cp[2]],
                                    [cp[0]+cw, cp[1]+ch, cp[2]],
                                    [cp[0], cp[1]+ch, cp[2]],
                                    [cp[0], cp[1], cp[2]+cd],
                                    [cp[0]+cw, cp[1], cp[2]+cd],
                                    [cp[0]+cw, cp[1]+ch, cp[2]+cd],
                                    [cp[0], cp[1]+ch, cp[2]+cd],
                                ];
                                let dash_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(200, 100, 100, 150));
                                for i in 0..8 {
                                    if let (Some(s1), Some(s2)) = (
                                        Self::world_to_screen_vp(orig_corners[i], &vp, &rect),
                                        Self::world_to_screen_vp(curr_corners[i], &vp, &rect),
                                    ) {
                                        let dist = ((s2.x-s1.x).powi(2) + (s2.y-s1.y).powi(2)).sqrt();
                                        if dist > 3.0 {
                                            draw_dashed_line(ui.painter(), s1, s2, dash_stroke, 6.0, 4.0);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ── D1: Protractor overlay during Rotate (3-step) ──
                // Step 1 (RotateRef): 量角器跟隨中心，虛線延伸到滑鼠
                if let DrawState::RotateRef { center, .. } = &self.editor.draw_state {
                    if let Some(sc) = Self::world_to_screen_vp(*center, &vp, &rect) {
                        let radius = 70.0;
                        let segments = 48;
                        // 量角器圓
                        let circle_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(76, 139, 245, 120));
                        for i in 0..segments {
                            let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                            let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
                            ui.painter().line_segment(
                                [egui::pos2(sc.x + radius * a0.cos(), sc.y + radius * a0.sin()),
                                 egui::pos2(sc.x + radius * a1.cos(), sc.y + radius * a1.sin())],
                                circle_stroke,
                            );
                        }
                        // 15° 刻度
                        for tick in 0..24 {
                            let angle = tick as f32 * 15.0_f32.to_radians();
                            let inner_r = radius - 4.0;
                            let outer_r = if tick % 6 == 0 { radius + 6.0 } else { radius + 3.0 };
                            let tick_color = if tick % 6 == 0 {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 180)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 60)
                            };
                            ui.painter().line_segment(
                                [egui::pos2(sc.x + inner_r * angle.cos(), sc.y + inner_r * angle.sin()),
                                 egui::pos2(sc.x + outer_r * angle.cos(), sc.y + outer_r * angle.sin())],
                                egui::Stroke::new(1.0, tick_color),
                            );
                        }
                        // 虛線到滑鼠位置（reference preview）
                        let mouse = egui::pos2(self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                        draw_dashed_line(ui.painter(), sc, mouse,
                            egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(200, 200, 200, 150)),
                            6.0, 4.0);
                        // 中心十字
                        let cs = 6.0;
                        let cc = egui::Color32::from_rgb(76, 139, 245);
                        ui.painter().line_segment([egui::pos2(sc.x - cs, sc.y), egui::pos2(sc.x + cs, sc.y)], egui::Stroke::new(2.0, cc));
                        ui.painter().line_segment([egui::pos2(sc.x, sc.y - cs), egui::pos2(sc.x, sc.y + cs)], egui::Stroke::new(2.0, cc));
                        // 提示文字
                        ui.painter().text(
                            egui::pos2(sc.x + radius + 10.0, sc.y - 10.0),
                            egui::Align2::LEFT_CENTER,
                            "設定參考方向",
                            egui::FontId::proportional(12.0),
                            egui::Color32::from_rgb(76, 139, 245),
                        );
                    }
                }
                // Step 2 (RotateAngle): 量角器 + 參考線 + 掃過弧 + 角度標籤
                if let DrawState::RotateAngle { center, ref_angle, current_angle, .. } = &self.editor.draw_state {
                    if let Some(sc) = Self::world_to_screen_vp(*center, &vp, &rect) {
                        let radius = 70.0;
                        let segments = 48;
                        let delta = current_angle - ref_angle;

                        // 量角器圓
                        let circle_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(76, 139, 245, 100));
                        for i in 0..segments {
                            let a0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                            let a1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
                            ui.painter().line_segment(
                                [egui::pos2(sc.x + radius * a0.cos(), sc.y + radius * a0.sin()),
                                 egui::pos2(sc.x + radius * a1.cos(), sc.y + radius * a1.sin())],
                                circle_stroke,
                            );
                        }
                        // 15° 刻度
                        for tick in 0..24 {
                            let angle = tick as f32 * 15.0_f32.to_radians();
                            let inner_r = radius - 4.0;
                            let outer_r = if tick % 6 == 0 { radius + 6.0 } else { radius + 3.0 };
                            let tick_color = if tick % 6 == 0 {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 180)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 60)
                            };
                            ui.painter().line_segment(
                                [egui::pos2(sc.x + inner_r * angle.cos(), sc.y + inner_r * angle.sin()),
                                 egui::pos2(sc.x + outer_r * angle.cos(), sc.y + outer_r * angle.sin())],
                                egui::Stroke::new(1.0, tick_color),
                            );
                        }
                        // 參考線（實線，灰白）— 從 center 沿 ref_angle 方向
                        // 注意：世界空間的 atan2(dz, dx) 需要投影到螢幕空間
                        // 簡化：用 ref_angle 在螢幕上畫（XZ 平面對應螢幕 X 方向）
                        let ref_end = egui::pos2(sc.x + radius * ref_angle.cos(), sc.y - radius * ref_angle.sin());
                        ui.painter().line_segment(
                            [sc, ref_end],
                            egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(200, 200, 200, 180)),
                        );
                        // 目標線（實線，藍色）— 從 center 到滑鼠
                        let mouse = egui::pos2(self.editor.mouse_screen[0], self.editor.mouse_screen[1]);
                        ui.painter().line_segment(
                            [sc, mouse],
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(76, 139, 245)),
                        );
                        // 掃過弧
                        if delta.abs() > 0.001 {
                            let arc_segments = 32;
                            let arc_stroke = egui::Stroke::new(3.0, egui::Color32::from_rgba_unmultiplied(76, 139, 245, 200));
                            for i in 0..arc_segments {
                                let t0 = ref_angle + delta * (i as f32 / arc_segments as f32);
                                let t1 = ref_angle + delta * ((i + 1) as f32 / arc_segments as f32);
                                ui.painter().line_segment(
                                    [egui::pos2(sc.x + radius * t0.cos(), sc.y - radius * t0.sin()),
                                     egui::pos2(sc.x + radius * t1.cos(), sc.y - radius * t1.sin())],
                                    arc_stroke,
                                );
                            }
                        }
                        // 中心十字
                        let cs = 6.0;
                        let cc = egui::Color32::from_rgb(76, 139, 245);
                        ui.painter().line_segment([egui::pos2(sc.x - cs, sc.y), egui::pos2(sc.x + cs, sc.y)], egui::Stroke::new(2.0, cc));
                        ui.painter().line_segment([egui::pos2(sc.x, sc.y - cs), egui::pos2(sc.x, sc.y + cs)], egui::Stroke::new(2.0, cc));
                        // 角度標籤
                        let delta_deg = delta.to_degrees();
                        let snap_deg = (delta_deg / 15.0).round() * 15.0;
                        let is_snapped = (delta_deg - snap_deg).abs() < 3.0;
                        let label = if is_snapped {
                            format!("{:.0}\u{00b0} \u{25cf}", snap_deg)
                        } else {
                            format!("{:.1}\u{00b0}", delta_deg)
                        };
                        let label_color = if is_snapped {
                            egui::Color32::from_rgb(60, 200, 60)
                        } else {
                            egui::Color32::from_rgb(76, 139, 245)
                        };
                        ui.painter().text(
                            egui::pos2(sc.x + radius + 10.0, sc.y - 10.0),
                            egui::Align2::LEFT_CENTER,
                            &label,
                            egui::FontId::proportional(if is_snapped { 15.0 } else { 13.0 }),
                            label_color,
                        );
                    }
                }

                // ── Move gizmo: 3D XYZ arrows ──
                if (self.editor.tool == Tool::Move || self.editor.tool == Tool::Select)
                    && !self.editor.selected_ids.is_empty()
                    && matches!(self.editor.draw_state, DrawState::Idle)
                {
                    if let Some(obj) = self.editor.selected_ids.first()
                        .and_then(|id| self.scene.objects.get(id))
                    {
                        let center = match &obj.shape {
                            Shape::Box { width, height, depth } =>
                                [obj.position[0] + width / 2.0, obj.position[1] + height / 2.0, obj.position[2] + depth / 2.0],
                            Shape::Cylinder { radius, height, .. } =>
                                [obj.position[0] + radius, obj.position[1] + height / 2.0, obj.position[2] + radius],
                            Shape::Sphere { radius, .. } =>
                                [obj.position[0] + radius, obj.position[1] + radius, obj.position[2] + radius],
                            _ => obj.position,
                        };
                        if let Some(sc) = Self::world_to_screen_vp(center, &vp, &rect) {
                            let axis_len = 50.0; // pixels
                            let head_sz = 8.0;
                            let axes = [
                                ([1.0_f32, 0.0, 0.0], egui::Color32::from_rgb(220, 60, 60), "X"),
                                ([0.0, 1.0, 0.0], egui::Color32::from_rgb(60, 180, 60), "Y"),
                                ([0.0, 0.0, 1.0], egui::Color32::from_rgb(60, 60, 220), "Z"),
                            ];
                            for (dir, color, label) in &axes {
                                let end_world = [
                                    center[0] + dir[0] * 800.0,
                                    center[1] + dir[1] * 800.0,
                                    center[2] + dir[2] * 800.0,
                                ];
                                if let Some(ep) = Self::world_to_screen_vp(end_world, &vp, &rect) {
                                    // 正規化到固定像素長度
                                    let dx = ep.x - sc.x;
                                    let dy = ep.y - sc.y;
                                    let len = (dx * dx + dy * dy).sqrt().max(1.0);
                                    let nx = dx / len;
                                    let ny = dy / len;
                                    let tip = egui::pos2(sc.x + nx * axis_len, sc.y + ny * axis_len);
                                    // 箭桿
                                    ui.painter().line_segment([sc, tip], egui::Stroke::new(2.5, *color));
                                    // 箭頭
                                    let perp_x = -ny;
                                    let perp_y = nx;
                                    let h1 = egui::pos2(tip.x - nx * head_sz + perp_x * head_sz * 0.4,
                                                        tip.y - ny * head_sz + perp_y * head_sz * 0.4);
                                    let h2 = egui::pos2(tip.x - nx * head_sz - perp_x * head_sz * 0.4,
                                                        tip.y - ny * head_sz - perp_y * head_sz * 0.4);
                                    ui.painter().add(egui::Shape::convex_polygon(
                                        vec![tip, h1, h2],
                                        *color,
                                        egui::Stroke::NONE,
                                    ));
                                    // 軸標籤
                                    ui.painter().text(
                                        egui::pos2(tip.x + nx * 6.0, tip.y + ny * 6.0),
                                        egui::Align2::CENTER_CENTER,
                                        label,
                                        egui::FontId::proportional(10.0),
                                        *color,
                                    );
                                }
                            }
                        }
                    }
                }

                // ── Scale handles: visible grip squares on bounding box ──
                if self.editor.tool == Tool::Scale && !self.editor.selected_ids.is_empty() {
                    if let Some(obj) = self.editor.selected_ids.first()
                        .and_then(|id| self.scene.objects.get(id))
                    {
                        let pos = glam::Vec3::from(obj.position);
                        let (sx, sy, sz) = match &obj.shape {
                            Shape::Box { width, height, depth } => (*width, *height, *depth),
                            Shape::Cylinder { radius, height, .. } => (*radius * 2.0, *height, *radius * 2.0),
                            Shape::Sphere { radius, .. } => (*radius * 2.0, *radius * 2.0, *radius * 2.0),
                            _ => (0.0, 0.0, 0.0),
                        };
                        if sx > 0.0 {
                            // 8 corners + 6 face centers = 14 grip points
                            let corners = [
                                [0.0, 0.0, 0.0], [sx, 0.0, 0.0], [sx, 0.0, sz], [0.0, 0.0, sz],
                                [0.0, sy, 0.0], [sx, sy, 0.0], [sx, sy, sz], [0.0, sy, sz],
                            ];
                            let face_centers = [
                                [sx / 2.0, sy / 2.0, 0.0],  // Front
                                [sx / 2.0, sy / 2.0, sz],   // Back
                                [0.0, sy / 2.0, sz / 2.0],  // Left
                                [sx, sy / 2.0, sz / 2.0],   // Right
                                [sx / 2.0, 0.0, sz / 2.0],  // Bottom
                                [sx / 2.0, sy, sz / 2.0],   // Top
                            ];
                            let grip_size = 4.0;
                            let corner_color = egui::Color32::from_rgb(80, 200, 120);
                            let face_color = egui::Color32::from_rgb(80, 200, 120);
                            // 角落 grip
                            for c in &corners {
                                let wp = [pos.x + c[0], pos.y + c[1], pos.z + c[2]];
                                if let Some(sp) = Self::world_to_screen_vp(wp, &vp, &rect) {
                                    let r = egui::Rect::from_center_size(sp, egui::vec2(grip_size * 2.0, grip_size * 2.0));
                                    ui.painter().rect_filled(r, 0.0, corner_color);
                                    ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                                }
                            }
                            // 面中心 grip（稍小）
                            for fc in &face_centers {
                                let wp = [pos.x + fc[0], pos.y + fc[1], pos.z + fc[2]];
                                if let Some(sp) = Self::world_to_screen_vp(wp, &vp, &rect) {
                                    let r = egui::Rect::from_center_size(sp, egui::vec2(grip_size * 1.5, grip_size * 1.5));
                                    ui.painter().rect_filled(r, 0.0, face_color);
                                    ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                                }
                            }
                            // 邊框連線
                            let edges: [(usize, usize); 12] = [
                                (0,1),(1,2),(2,3),(3,0), // bottom
                                (4,5),(5,6),(6,7),(7,4), // top
                                (0,4),(1,5),(2,6),(3,7), // vertical
                            ];
                            let edge_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(80, 200, 120, 120));
                            for (a, b) in &edges {
                                let wa = [pos.x + corners[*a][0], pos.y + corners[*a][1], pos.z + corners[*a][2]];
                                let wb = [pos.x + corners[*b][0], pos.y + corners[*b][1], pos.z + corners[*b][2]];
                                if let (Some(sa), Some(sb)) = (
                                    Self::world_to_screen_vp(wa, &vp, &rect),
                                    Self::world_to_screen_vp(wb, &vp, &rect),
                                ) {
                                    ui.painter().line_segment([sa, sb], edge_stroke);
                                }
                            }
                        }
                    }
                }

                // ── DXF Smart Import confirmation panel (legacy, redirects to review) ──
                if let Some(ir) = self.pending_ir.take() {
                    // Convert pending_ir into the new review panel
                    let entity_count = ir.columns.len() + ir.beams.len() + ir.base_plates.len();
                    let debug = ir.debug_report.clone();
                    self.import_review = Some(crate::import_review::ImportReview::from_drawing_ir(
                        &ir, &"DXF", entity_count, debug,
                    ));
                }

                // ── Import Review Panel (full-screen overlay) ──
                if let Some(ref mut review) = self.import_review {
                    if review.active {
                        let action = crate::import_review::draw_review_panel(ui, review, rect);
                        match action {
                            crate::import_review::ReviewAction::Confirm => {
                                let ir = review.to_drawing_ir();
                                self.scene.snapshot();
                                let result = crate::builders::steel_builder::build_from_ir(&mut self.scene, &ir);
                                self.editor.selected_ids.clear();
                                self.editor.selected_ids.extend(result.ids);
                                self.zoom_extents();
                                let msg = format!("建模完成: {} 柱 + {} 梁 + {} 底板",
                                    result.columns_created, result.beams_created, result.plates_created);
                                self.file_message = Some((msg, std::time::Instant::now()));
                                self.import_review = None;
                            }
                            crate::import_review::ReviewAction::Cancel => {
                                self.import_review = None;
                            }
                            _ => {}
                        }
                    }
                }

                // ── Unified Smart Import confirmation panel ──
                if let Some(ref ir) = self.pending_unified_ir.clone() {
                    let panel_w = 420.0;
                    let panel_h = 380.0;
                    let panel_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(panel_w, panel_h));

                    ui.painter().rect_filled(panel_rect, 16.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 245));
                    ui.painter().rect_stroke(panel_rect, 16.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));

                    let mut y_ir = panel_rect.top() + 20.0;
                    let x_ir = panel_rect.left() + 20.0;

                    ui.painter().text(egui::pos2(panel_rect.center().x, y_ir), egui::Align2::CENTER_TOP,
                        format!("智慧匯入結果 ({})", ir.source_format.to_uppercase()),
                        egui::FontId::proportional(16.0), egui::Color32::from_rgb(31, 36, 48));
                    y_ir += 30.0;

                    let info_lines = [
                        format!("來源檔案: {}", std::path::Path::new(&ir.source_file).file_name()
                            .map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
                        format!("網格數: {}", ir.stats.mesh_count),
                        format!("頂點數: {}", ir.stats.vertex_count),
                        format!("面數: {}", ir.stats.face_count),
                        format!("群組數: {}", ir.stats.group_count),
                        format!("構件數: {}", ir.stats.member_count),
                        format!("材質數: {}", ir.stats.material_count),
                    ];

                    for line_text in &info_lines {
                        ui.painter().text(egui::pos2(x_ir, y_ir), egui::Align2::LEFT_TOP,
                            line_text, egui::FontId::proportional(12.0), egui::Color32::from_rgb(60, 65, 80));
                        y_ir += 20.0;
                    }

                    y_ir += 15.0;

                    // Confirm button
                    let btn_confirm = egui::Rect::from_min_size(egui::pos2(panel_rect.center().x - 80.0, y_ir), egui::vec2(70.0, 32.0));
                    let btn_cancel = egui::Rect::from_min_size(egui::pos2(panel_rect.center().x + 10.0, y_ir), egui::vec2(70.0, 32.0));

                    ui.painter().rect_filled(btn_confirm, 8.0, egui::Color32::from_rgb(76, 139, 245));
                    ui.painter().text(btn_confirm.center(), egui::Align2::CENTER_CENTER, "確認建模",
                        egui::FontId::proportional(12.0), egui::Color32::WHITE);

                    ui.painter().rect_filled(btn_cancel, 8.0, egui::Color32::from_rgb(200, 200, 200));
                    ui.painter().text(btn_cancel.center(), egui::Align2::CENTER_CENTER, "取消",
                        egui::FontId::proportional(12.0), egui::Color32::from_rgb(60, 60, 60));

                    let confirm_resp = ui.allocate_rect(btn_confirm, egui::Sense::click());
                    let cancel_resp = ui.allocate_rect(btn_cancel, egui::Sense::click());

                    if confirm_resp.clicked() {
                        let ir_data = self.pending_unified_ir.take().unwrap();
                        self.scene.snapshot();
                        let result = crate::import::import_manager::build_scene_from_ir(&mut self.scene, &ir_data);
                        self.editor.selected_ids = result.ids;
                        self.zoom_extents();
                        if result.columns > 0 || result.beams > 0 {
                            self.console_push("INFO", format!(
                                "[SemanticDetector] Steel members created: {} columns, {} beams, {} plates",
                                result.columns, result.beams, result.plates
                            ));
                        }
                        self.file_message = Some((
                            format!("建模完成: {} 柱 + {} 梁 + {} 板 + {} 網格",
                                result.columns, result.beams, result.plates, result.meshes),
                            std::time::Instant::now()
                        ));
                    }
                    if cancel_resp.clicked() {
                        self.pending_unified_ir = None;
                    }
                }

                // Unsaved changes confirmation overlay
                if self.pending_action.is_some() {
                    let popup_rect = egui::Rect::from_center_size(
                        rect.center(),
                        egui::vec2(350.0, 100.0),
                    );
                    ui.painter().rect_filled(popup_rect, 8.0, egui::Color32::from_rgba_unmultiplied(30, 30, 40, 240));
                    ui.painter().rect_stroke(popup_rect, 8.0, egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 150, 220)));
                    ui.painter().text(popup_rect.center_top() + egui::vec2(0.0, 20.0),
                        egui::Align2::CENTER_TOP, "場景有未儲存的修改",
                        egui::FontId::proportional(15.0), egui::Color32::WHITE);
                    ui.painter().text(popup_rect.center() + egui::vec2(0.0, 5.0),
                        egui::Align2::CENTER_CENTER, "按 Y 繼續（放棄修改）/ N 取消",
                        egui::FontId::proportional(13.0), egui::Color32::from_gray(180));
                }

                // ── Floating view buttons (top-right of viewport) ──
                {
                    let view_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.right() - 250.0, rect.top() + 16.0),
                        egui::vec2(240.0, 44.0),
                    );

                    // Background pill
                    ui.painter().rect_filled(view_rect, 18.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 225));
                    ui.painter().rect_stroke(view_rect, 18.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200)));

                    // View buttons
                    let views = ["\u{900f}\u{8996}", "\u{6b63}\u{8996}", "\u{4fef}\u{8996}", "\u{5de6}\u{8996}"];
                    let btn_w = 50.0;
                    let padding = 8.0;
                    for (i, label) in views.iter().enumerate() {
                        let x = view_rect.left() + padding + i as f32 * (btn_w + 6.0);
                        let btn_rect = egui::Rect::from_min_size(
                            egui::pos2(x, view_rect.top() + 6.0),
                            egui::vec2(btn_w, 32.0),
                        );

                        let is_active = match i {
                            0 => !self.viewer.use_ortho,  // 透視
                            1 => self.viewer.use_ortho && self.viewer.camera.pitch.abs() < 0.1, // 正視
                            2 => self.viewer.use_ortho && self.viewer.camera.pitch < -1.0, // 俯視
                            3 => self.viewer.use_ortho && (self.viewer.camera.yaw + std::f32::consts::FRAC_PI_2).abs() < 0.1, // 左視
                            _ => false,
                        };

                        let response = ui.allocate_rect(btn_rect, egui::Sense::click());
                        let bg = if is_active {
                            egui::Color32::from_rgba_unmultiplied(76, 139, 245, 30)
                        } else if response.hovered() {
                            egui::Color32::from_rgb(240, 242, 248)
                        } else {
                            egui::Color32::WHITE
                        };
                        let text_color = if is_active {
                            egui::Color32::from_rgb(76, 139, 245)
                        } else {
                            egui::Color32::from_rgb(110, 118, 135)
                        };

                        ui.painter().rect_filled(btn_rect, 12.0, bg);
                        ui.painter().rect_stroke(btn_rect, 12.0,
                            egui::Stroke::new(1.0, if is_active {
                                egui::Color32::from_rgba_unmultiplied(76, 139, 245, 90)
                            } else {
                                egui::Color32::from_rgb(229, 231, 239)
                            }));
                        ui.painter().text(btn_rect.center(), egui::Align2::CENTER_CENTER,
                            label, egui::FontId::proportional(12.0), text_color);

                        if response.clicked() {
                            match i {
                                0 => self.viewer.use_ortho = false,
                                1 => { self.viewer.use_ortho = true; self.viewer.camera.set_front(); }
                                2 => { self.viewer.use_ortho = true; self.viewer.camera.set_top(); }
                                3 => { self.viewer.use_ortho = true; self.viewer.camera.set_left(); }
                                _ => {}
                            }
                        }
                    }
                }

                // ── Tool info card (top-left of viewport) ──
                {
                    let card_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + 16.0, rect.top() + 16.0),
                        egui::vec2(280.0, 60.0),
                    );
                    ui.painter().rect_filled(card_rect, 18.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 225));
                    ui.painter().rect_stroke(card_rect, 18.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200)));

                    let tool_name = match self.editor.tool {
                        Tool::Select => "\u{9078}\u{53d6} / Move-Ready",
                        Tool::Move => "\u{79fb}\u{52d5}\u{5de5}\u{5177}",
                        Tool::Rotate => "\u{65cb}\u{8f49}\u{5de5}\u{5177}",
                        Tool::CreateBox => "\u{65b9}\u{584a}\u{5de5}\u{5177}",
                        Tool::PushPull => "\u{63a8}\u{62c9}\u{5de5}\u{5177}",
                        Tool::Line => "\u{7dda}\u{6bb5}\u{5de5}\u{5177}",
                        _ => "\u{5de5}\u{5177}",
                    };
                    ui.painter().text(
                        egui::pos2(card_rect.left() + 14.0, card_rect.top() + 16.0),
                        egui::Align2::LEFT_TOP,
                        format!("\u{76ee}\u{524d}\u{5de5}\u{5177}\u{ff1a}{}", tool_name),
                        egui::FontId::proportional(13.0),
                        egui::Color32::from_rgb(31, 36, 48),
                    );
                    ui.painter().text(
                        egui::pos2(card_rect.left() + 14.0, card_rect.top() + 36.0),
                        egui::Align2::LEFT_TOP,
                        &self.status_text(),
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgb(110, 118, 135),
                    );
                }

                // ── Navigation pad (bottom-left of viewport) ──
                {
                    let pad_size = 130.0;
                    let pad_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + 16.0, rect.bottom() - pad_size - 60.0),
                        egui::vec2(pad_size, pad_size + 24.0),
                    );
                    ui.painter().rect_filled(pad_rect, 22.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 225));
                    ui.painter().rect_stroke(pad_rect, 22.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 210)));

                    // Title
                    ui.painter().text(
                        egui::pos2(pad_rect.left() + 12.0, pad_rect.top() + 10.0),
                        egui::Align2::LEFT_TOP, "\u{8996}\u{89d2} / \u{5e73}\u{79fb}",
                        egui::FontId::proportional(11.0), egui::Color32::from_rgb(110, 118, 135));

                    // 3x3 button grid
                    let arrows = ["", "\u{2191}", "", "\u{2190}", "\u{29bf}", "\u{2192}", "", "\u{2193}", ""];
                    let btn_size = 32.0;
                    let gap = 6.0;
                    let grid_start_x = pad_rect.center().x - (btn_size * 1.5 + gap);
                    let grid_start_y = pad_rect.top() + 28.0;

                    for (i, label) in arrows.iter().enumerate() {
                        if label.is_empty() { continue; }
                        let row = i / 3;
                        let col = i % 3;
                        let btn_rect = egui::Rect::from_min_size(
                            egui::pos2(
                                grid_start_x + col as f32 * (btn_size + gap),
                                grid_start_y + row as f32 * (btn_size + gap),
                            ),
                            egui::vec2(btn_size, btn_size),
                        );

                        let response = ui.allocate_rect(btn_rect, egui::Sense::click());
                        let bg = if response.hovered() {
                            egui::Color32::from_rgb(240, 242, 248)
                        } else {
                            egui::Color32::WHITE
                        };
                        ui.painter().rect_filled(btn_rect, 12.0, bg);
                        ui.painter().rect_stroke(btn_rect, 12.0,
                            egui::Stroke::new(1.0, egui::Color32::from_rgb(229, 231, 239)));
                        ui.painter().text(btn_rect.center(), egui::Align2::CENTER_CENTER,
                            label, egui::FontId::proportional(14.0), egui::Color32::from_rgb(110, 118, 135));

                        if response.clicked() {
                            let step = self.viewer.camera.distance * 0.1;
                            match i {
                                1 => self.viewer.camera.walk_forward(step),
                                3 => self.viewer.camera.walk_strafe(-step),
                                4 => self.viewer.camera.set_iso(),
                                5 => self.viewer.camera.walk_strafe(step),
                                7 => self.viewer.camera.walk_forward(-step),
                                _ => {}
                            }
                        }
                    }
                }

                // ── Coordinate chips (bottom-center of viewport) ──
                {
                    let chips_y = rect.bottom() - 40.0;
                    let chip_data = [
                        format!("X: {:.0}", self.editor.mouse_ground.map(|p| p[0]).unwrap_or(0.0)),
                        format!("Y: {:.0}", self.editor.mouse_ground.map(|p| p[1]).unwrap_or(0.0)),
                        format!("Z: {:.0}", self.editor.mouse_ground.map(|p| p[2]).unwrap_or(0.0)),
                        "Snap: ON".to_string(),
                        "Units: mm".to_string(),
                    ];

                    let chip_w = 70.0;
                    let total_w = chip_data.len() as f32 * (chip_w + 8.0);
                    let start_x = rect.center().x - total_w / 2.0;

                    for (i, text) in chip_data.iter().enumerate() {
                        let chip_rect = egui::Rect::from_min_size(
                            egui::pos2(start_x + i as f32 * (chip_w + 8.0), chips_y),
                            egui::vec2(chip_w, 28.0),
                        );
                        ui.painter().rect_filled(chip_rect, 999.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 215));
                        ui.painter().rect_stroke(chip_rect, 999.0,
                            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200)));
                        ui.painter().text(chip_rect.center(), egui::Align2::CENTER_CENTER,
                            text, egui::FontId::proportional(11.0), egui::Color32::from_rgb(31, 36, 48));
                    }
                }

                // ── Tool cursor icon (small tool icon follows mouse) ──
                if response.hovered() {
                    let mx = rect.min.x + self.editor.mouse_screen[0];
                    let my = rect.min.y + self.editor.mouse_screen[1];

                    // Draw mini tool icon (20x20) at cursor offset
                    let icon_size = 20.0;
                    let icon_rect = egui::Rect::from_min_size(
                        egui::pos2(mx + 14.0, my + 14.0),  // bottom-right of cursor
                        egui::vec2(icon_size, icon_size),
                    );

                    // Semi-transparent background circle per tool category
                    let bg_color = match self.editor.tool {
                        Tool::Select => egui::Color32::from_rgba_unmultiplied(76, 139, 245, 180),
                        Tool::Move => egui::Color32::from_rgba_unmultiplied(245, 166, 35, 180),
                        Tool::Rotate => egui::Color32::from_rgba_unmultiplied(180, 80, 220, 180),
                        Tool::Scale => egui::Color32::from_rgba_unmultiplied(80, 200, 120, 180),
                        Tool::Line | Tool::Arc | Tool::Rectangle | Tool::Circle => egui::Color32::from_rgba_unmultiplied(60, 60, 60, 180),
                        Tool::CreateBox | Tool::CreateCylinder | Tool::CreateSphere => egui::Color32::from_rgba_unmultiplied(76, 139, 245, 180),
                        Tool::PushPull => egui::Color32::from_rgba_unmultiplied(245, 100, 60, 180),
                        Tool::PaintBucket => egui::Color32::from_rgba_unmultiplied(220, 80, 160, 180),
                        Tool::Eraser => egui::Color32::from_rgba_unmultiplied(220, 50, 50, 180),
                        Tool::TapeMeasure | Tool::Dimension => egui::Color32::from_rgba_unmultiplied(100, 180, 100, 180),
                        Tool::Text => egui::Color32::from_rgba_unmultiplied(180, 140, 60, 180),
                        _ => egui::Color32::from_rgba_unmultiplied(100, 100, 100, 160),
                    };

                    // Draw circular background
                    let center = icon_rect.center();
                    ui.painter().circle_filled(center, icon_size * 0.55, bg_color);

                    // Draw the tool icon inside (shrunk)
                    let inner_rect = icon_rect.shrink(3.0);
                    crate::icons::draw_tool_icon(ui.painter(), inner_rect, self.editor.tool, egui::Color32::WHITE);
                }

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
                        ("F1", "\u{8aaa}\u{660e}\u{ff08}\u{672c}\u{9801}\u{9762}\u{ff09}"),
                        ("Delete", "\u{522a}\u{9664}\u{9078}\u{53d6}"),
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
                self.ai_log.log(&actor, "\u{6e05}\u{7a7a}\u{5834}\u{666f}", &format!("{} objects removed", count), vec![]);
                McpResult { success: true, data: json!({ "cleared": count }) }
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

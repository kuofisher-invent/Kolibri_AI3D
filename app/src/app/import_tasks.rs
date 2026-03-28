use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use eframe::{egui, wgpu};
use eframe::epaint::mutex::RwLock;
use serde::Serialize;

use crate::camera::{self, OrbitCamera};
use crate::renderer::ViewportRenderer;
use crate::scene::{MaterialKind, Scene, Shape};
use crate::app::{KolibriApp, Tool, WorkMode, DrawState, ScaleHandle, PullFace, SnapType, SnapResult, AiSuggestion, SuggestionAction, RightTab, CursorHint, EditorState, SelectionMode, RenderMode, ViewerState, BackgroundTaskResult, BackgroundSceneBuild, SpatialEntry};

impl KolibriApp {
    pub(crate) fn console_push(&mut self, level: &str, msg: String) {
        Self::append_import_audit_log(level, &msg);
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

    fn current_process_memory_mb() -> Option<u64> {
        use sysinfo::{Pid, System};

        let mut system = System::new();
        let pid = Pid::from_u32(std::process::id());
        system.refresh_process(pid);
        system.process(pid).map(|process| process.memory() / (1024 * 1024))
    }

    fn append_import_audit_log(level: &str, msg: &str) {
        if !(msg.contains("[Import") || msg.contains("[ImportPhase]")) {
            return;
        }

        use std::io::Write;

        let log_dir = std::path::Path::new("logs");
        if std::fs::create_dir_all(log_dir).is_err() {
            return;
        }

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let line = format!("[{}] [{}] {}\n", timestamp_ms, level, msg);

        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("import_audit.log"))
        {
            let _ = file.write_all(line.as_bytes());
        }
    }

    pub(crate) fn write_import_source_debug(
        object_debug: &std::collections::HashMap<String, crate::import::import_manager::ImportedObjectDebug>,
    ) {
        if object_debug.is_empty() {
            return;
        }
        let log_dir = std::path::Path::new("logs");
        if std::fs::create_dir_all(log_dir).is_err() {
            return;
        }
        let path = log_dir.join("import_source_debug.json");
        if let Ok(json) = serde_json::to_string_pretty(object_debug) {
            let _ = std::fs::write(path, json);
        }
    }

    pub(crate) fn log_import_phase(&mut self, phase: &str, detail: impl Into<String>) {
        let detail = detail.into();
        let mem_suffix = Self::current_process_memory_mb()
            .map(|mb| format!(" | mem={} MB", mb))
            .unwrap_or_default();
        self.console_push("INFO", format!("[ImportPhase] {} | {}{}", phase, detail, mem_suffix));
    }

    pub(crate) fn try_load_startup_scene(&mut self) {
        let Some(path) = self.startup_scene_path.clone() else {
            return;
        };
        self.log_import_phase("startup_scene_open", format!("path={}", path));
        match self.scene.load_from_file(&path) {
            Ok(count) => {
                self.current_file = Some(path.clone());
                self.last_saved_version = self.scene.version;
                self.editor.selected_ids.clear();
                self.viewer.hidden_tags.clear();
                self.editor.editing_group_id = None;
                self.editor.editing_component_def_id = None;
                self.zoom_extents();
                self.log_import_phase(
                    "startup_scene_loaded",
                    format!(
                        "objects={} groups={} component_defs={} path={}",
                        count,
                        self.scene.groups.len(),
                        self.scene.component_defs.len(),
                        path
                    ),
                );
                self.write_startup_scene_state(&path, None);
                if self.startup_screenshot_path.is_some() {
                    self.startup_screenshot_delay_frames = 120;
                    self.startup_screenshot_attempts = 0;
                    self.startup_screenshot_requested = false;
                    self.startup_screenshot_wait_logged = false;
                    self.startup_screenshot_missing_logged = false;
                    self.log_import_phase("startup_screenshot_armed", "delay_frames=120".to_string());
                } else {
                    self.log_import_phase("startup_screenshot_env_missing", "KOLIBRI_STARTUP_SCREENSHOT_OUT not set".to_string());
                }
            }
            Err(err) => {
                self.log_import_phase(
                    "startup_scene_failed",
                    format!("path={} error={}", path, err),
                );
                self.write_startup_scene_state(&path, Some(err.to_string()));
            }
        }
        self.startup_scene_attempted = true;
    }

    fn write_startup_scene_state(&self, path: &str, error: Option<String>) {
        #[derive(Serialize)]
        struct StartupSceneState<'a> {
            path: &'a str,
            objects: usize,
            groups: usize,
            component_defs: usize,
            version: u64,
            startup_screenshot_path: Option<&'a str>,
            error: Option<String>,
        }

        let state = StartupSceneState {
            path,
            objects: self.scene.objects.len(),
            groups: self.scene.groups.len(),
            component_defs: self.scene.component_defs.len(),
            version: self.scene.version,
            startup_screenshot_path: self.startup_screenshot_path.as_deref(),
            error,
        };

        let state_path = std::env::var("KOLIBRI_STARTUP_STATE_OUT")
            .unwrap_or_else(|_| "logs/startup_scene_state.json".to_string());
        if let Some(parent) = std::path::Path::new(&state_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&state) {
            let _ = std::fs::write(&state_path, json);
        }
    }

    pub(crate) fn maybe_capture_startup_screenshot(&mut self, ctx: &egui::Context) {
        if self.startup_screenshot_completed {
            return;
        }
        let Some(path) = self.startup_screenshot_path.clone() else {
            return;
        };
        if !self.startup_scene_attempted || self.scene.objects.is_empty() {
            return;
        }
        if self.viewport.size[0] < 2 || self.viewport.size[1] < 2 {
            if !self.startup_screenshot_wait_logged {
                self.log_import_phase(
                    "startup_screenshot_waiting_size",
                    format!("width={} height={}", self.viewport.size[0], self.viewport.size[1]),
                );
                self.startup_screenshot_wait_logged = true;
            }
            ctx.request_repaint();
            return;
        }
        if self.startup_screenshot_delay_frames > 0 {
            if self.startup_screenshot_delay_frames == 120
                || self.startup_screenshot_delay_frames == 60
                || self.startup_screenshot_delay_frames == 15
                || self.startup_screenshot_delay_frames == 1
            {
                self.log_import_phase(
                    "startup_screenshot_countdown",
                    format!("frames_remaining={}", self.startup_screenshot_delay_frames),
                );
            }
            self.startup_screenshot_delay_frames -= 1;
            ctx.request_repaint();
            return;
        }
        if !self.startup_screenshot_requested {
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            self.startup_screenshot_attempts += 1;
            self.startup_screenshot_requested = true;
            self.startup_screenshot_missing_logged = false;
            self.log_import_phase(
                "startup_screenshot_start",
                format!(
                    "attempt={} width={} height={} path={}",
                    self.startup_screenshot_attempts,
                    self.viewport.size[0],
                    self.viewport.size[1],
                    path
                ),
            );
            self.viewport.save_screenshot(&self.device, &self.queue, &path);
            self.startup_screenshot_delay_frames = 15;
            ctx.request_repaint();
            return;
        }
        if std::path::Path::new(&path).exists() {
            self.log_import_phase("startup_screenshot_saved", format!("path={}", path));
            self.startup_screenshot_completed = true;

            if std::env::var("KOLIBRI_EXIT_AFTER_STARTUP_SCREENSHOT").ok().as_deref() == Some("1") {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }
        if !self.startup_screenshot_missing_logged {
            self.log_import_phase(
                "startup_screenshot_missing_after_save",
                format!("attempt={} path={}", self.startup_screenshot_attempts, path),
            );
            self.startup_screenshot_missing_logged = true;
        }
        if self.startup_screenshot_attempts < 3 {
            self.startup_screenshot_requested = false;
            self.startup_screenshot_delay_frames = 60;
            self.log_import_phase(
                "startup_screenshot_retry_scheduled",
                format!("next_attempt={} delay_frames=60", self.startup_screenshot_attempts + 1),
            );
            ctx.request_repaint();
            return;
        }
        self.log_import_phase(
            "startup_screenshot_failed",
            format!("attempts={} path={}", self.startup_screenshot_attempts, path),
        );
        self.startup_screenshot_completed = true;
    }

    fn log_ir_summary(&mut self, phase: &str, ir: &crate::import::unified_ir::UnifiedIR) {
        self.log_import_phase(
            phase,
            format!(
                "format={} meshes={} instances={} groups={} component_defs={} materials={} vertices={} faces={}",
                ir.source_format.to_uppercase(),
                ir.stats.mesh_count,
                ir.stats.instance_count,
                ir.stats.group_count,
                ir.stats.component_count,
                ir.stats.material_count,
                ir.stats.vertex_count,
                ir.stats.face_count,
            ),
        );
    }

    fn log_scene_build_summary(
        &mut self,
        phase: &str,
        source_format: &str,
        result: &crate::import::import_manager::BuildResult,
        duration: std::time::Duration,
    ) {
        self.log_import_phase(
            phase,
            format!(
                "format={} scene_objects={} scene_groups={} scene_component_defs={} built_meshes={} columns={} beams={} plates={} elapsed_ms={}",
                source_format.to_uppercase(),
                self.scene.objects.len(),
                self.scene.groups.len(),
                self.scene.component_defs.len(),
                result.meshes,
                result.columns,
                result.beams,
                result.plates,
                duration.as_millis(),
            ),
        );
    }

    fn log_scene_build_timings(&mut self, result: &crate::import::import_manager::BuildResult) {
        for (phase, elapsed_ms) in &result.phase_timings_ms {
            self.log_import_phase("scene_build_phase", format!("phase={} elapsed_ms={}", phase, elapsed_ms));
        }
    }

    pub(crate) fn background_task_active(&self) -> bool {
        self.background_task_rx.is_some()
    }

    pub(crate) fn background_task_elapsed(&self) -> Option<std::time::Duration> {
        self.background_task_started_at.map(|started| started.elapsed())
    }

    pub(crate) fn auto_save_deferred(&self) -> bool {
        self.defer_auto_save_until
            .map(|deadline| std::time::Instant::now() < deadline)
            .unwrap_or(false)
    }

    pub(crate) fn is_heavy_import(ir: &crate::import::unified_ir::UnifiedIR) -> bool {
        ir.source_format.eq_ignore_ascii_case("skp")
            && (ir.stats.instance_count >= 2_000
                || ir.stats.vertex_count >= 1_000_000
                || ir.stats.face_count >= 300_000)
    }

    pub(crate) fn should_replace_scene_on_import(ir: &crate::import::unified_ir::UnifiedIR) -> bool {
        ir.source_format.eq_ignore_ascii_case("skp")
            || ir.source_format.eq_ignore_ascii_case("obj")
    }

    pub(crate) fn start_import_task(&mut self, path: String) {
        if self.background_task_active() {
            self.file_message = Some(("已有匯入工作進行中，請稍候".into(), std::time::Instant::now()));
            return;
        }

        let filename = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path.as_str())
            .to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        self.background_task_rx = Some(rx);
        self.background_task_label = Some(format!("匯入中: {}", filename));
        self.background_task_started_at = Some(std::time::Instant::now());
        self.viewer.show_console = true;
        self.console_push("INFO", format!("[Import] 背景匯入開始: {}", path));
        self.console_push("INFO", "[Import] Audit log: logs/import_audit.log".to_string());
        self.log_import_phase("import_start", format!("path={}", path));

        std::thread::spawn(move || {
            let result = crate::import::import_manager::import_file(&path);
            let _ = tx.send(BackgroundTaskResult::Import(result));
        });
    }

    pub(crate) fn start_scene_build_task(&mut self, ir: crate::import::unified_ir::UnifiedIR) {
        if self.background_task_active() {
            self.file_message = Some(("已有背景工作進行中，請稍候".into(), std::time::Instant::now()));
            return;
        }

        let heavy_import = Self::is_heavy_import(&ir);
        let replace_scene = Self::should_replace_scene_on_import(&ir);
        let should_snapshot = !replace_scene && !heavy_import && !self.scene.objects.is_empty();
        let mut scene = if replace_scene || self.scene.objects.is_empty() {
            Scene::default()
        } else {
            self.scene.clone()
        };
        let (tx, rx) = std::sync::mpsc::channel();
        self.background_task_rx = Some(rx);
        self.background_task_label = Some(format!("建構場景中: {}", ir.source_format.to_uppercase()));
        self.background_task_started_at = Some(std::time::Instant::now());
        self.console_push("INFO", format!("[Import] 背景建構場景開始: {}", ir.source_format.to_uppercase()));
        self.log_ir_summary("scene_build_start", &ir);
        self.log_import_phase(
            "import_protection_mode",
            format!(
                "format={} replace_scene={} heavy_mode={} snapshot={} skip_zoom_extents={} defer_autosave={} instances={} vertices={} faces={}",
                ir.source_format.to_uppercase(),
                replace_scene,
                heavy_import,
                should_snapshot,
                heavy_import,
                heavy_import,
                ir.stats.instance_count,
                ir.stats.vertex_count,
                ir.stats.face_count,
            ),
        );
        if heavy_import {
            self.console_push(
                "WARN",
                format!(
                    "[Import] 重型匯入保護已啟用: instances={} vertices={} faces={}",
                    ir.stats.instance_count,
                    ir.stats.vertex_count,
                    ir.stats.face_count,
                ),
            );
        }

        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            if should_snapshot {
                scene.snapshot();
            }
            let result = crate::import::import_manager::build_scene_from_ir(&mut scene, &ir);
            let _ = tx.send(BackgroundTaskResult::Build(Ok(BackgroundSceneBuild {
                scene,
                result,
                duration: started.elapsed(),
                replace_scene,
                skip_zoom_extents: heavy_import,
                defer_auto_save: heavy_import,
                source_format: ir.source_format.clone(),
            })));
        });
    }

    pub(crate) fn poll_background_task(&mut self) {
        let message = match self.background_task_rx.as_ref() {
            Some(rx) => match rx.try_recv() {
                Ok(message) => Some(message),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    self.background_task_rx = None;
                    self.background_task_label = None;
                    self.background_task_started_at = None;
                    self.file_message = Some(("背景工作中斷".into(), std::time::Instant::now()));
                    None
                }
            },
            None => None,
        };

        let Some(message) = message else { return; };
        self.background_task_rx = None;
        self.background_task_label = None;
        self.background_task_started_at = None;

        match message {
            BackgroundTaskResult::Import(Ok(ir)) => {
                self.log_ir_summary("import_parsed", &ir);
                self.log_import_phase(
                    "import_review_pending",
                    format!(
                        "format={} source_file={} heavy_mode={} meshes={} instances={} groups={} component_defs={}",
                        ir.source_format.to_uppercase(),
                        ir.source_file,
                        Self::is_heavy_import(&ir),
                        ir.stats.mesh_count,
                        ir.stats.instance_count,
                        ir.stats.group_count,
                        ir.stats.component_count,
                    ),
                );
                for line in &ir.debug_report {
                    let level = if line.contains("ERROR") || line.contains("WARN") { "WARN" } else { "INFO" };
                    self.console_push(level, line.clone());
                }
                if ir.debug_report.is_empty() {
                    self.console_push(
                        "INFO",
                        format!(
                            "[Import] 完成: {} | 頂點: {} | 面: {} | Mesh: {} | 實例: {} | 材質: {}",
                            ir.source_format.to_uppercase(),
                            ir.stats.vertex_count,
                            ir.stats.face_count,
                            ir.stats.mesh_count,
                            ir.stats.instance_count,
                            ir.stats.material_count,
                        ),
                    );
                }
                let summary = format!(
                    "匯入解析完成 ({})\n\n頂點: {}\n面: {}\nMesh: {}\n群組: {}\n實例: {}\n材質: {}",
                    ir.source_format.to_uppercase(),
                    ir.stats.vertex_count,
                    ir.stats.face_count,
                    ir.stats.mesh_count,
                    ir.stats.group_count,
                    ir.stats.instance_count,
                    ir.stats.material_count,
                );
                self.pending_unified_ir = Some(ir);
                self.viewer.show_console = true;
                self.file_message = Some((summary, std::time::Instant::now()));
            }
            BackgroundTaskResult::Import(Err(e)) => {
                self.log_import_phase("import_failed", e.clone());
                self.console_push("ERROR", format!("[Import] 匯入失敗: {}", e));
                self.file_message = Some((format!("匯入失敗:\n{}", e), std::time::Instant::now()));
            }
            BackgroundTaskResult::Build(Ok(done)) => {
                self.scene = done.scene;
                self.import_object_debug = done.result.object_debug.clone();
                Self::write_import_source_debug(&self.import_object_debug);
                if done.replace_scene {
                    self.viewer.hidden_tags.clear();
                    self.editor.editing_group_id = None;
                    self.editor.editing_component_def_id = None;
                }
                self.log_scene_build_summary("scene_build_complete", &done.source_format, &done.result, done.duration);
                self.log_scene_build_timings(&done.result);
                self.editor.selected_ids = done.result.ids;
                if done.skip_zoom_extents {
                    self.console_push(
                        "WARN",
                        format!(
                            "[Import] {} 建構完成後略過 zoom extents，以降低大型場景後處理成本",
                            done.source_format.to_uppercase(),
                        ),
                    );
                } else {
                    self.log_import_phase("zoom_extents_start", format!("format={}", done.source_format.to_uppercase()));
                    self.zoom_extents();
                    self.log_import_phase("zoom_extents_done", format!("format={}", done.source_format.to_uppercase()));
                }
                if done.defer_auto_save {
                    self.defer_auto_save_until =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(180));
                    self.console_push(
                        "WARN",
                        "[Import] 已暫停 autosave 3 分鐘，避免大型匯入完成後立即觸發高成本寫檔".to_string(),
                    );
                    self.log_import_phase("autosave_deferred", "duration_s=180".to_string());
                }
                self.console_push(
                    "INFO",
                    format!(
                        "[Import] 場景建構完成: {} | meshes={} groups={} component_defs={} | elapsed={} ms",
                        done.source_format.to_uppercase(),
                        done.result.meshes,
                        self.scene.groups.len(),
                        self.scene.component_defs.len(),
                        done.duration.as_millis(),
                    ),
                );
                if done.result.columns > 0 || done.result.beams > 0 {
                    self.console_push(
                        "INFO",
                        format!(
                            "[SemanticDetector] Steel members created: {} columns, {} beams, {} plates",
                            done.result.columns, done.result.beams, done.result.plates
                        ),
                    );
                }
                self.file_message = Some((
                    format!(
                        "場景建構完成: {} 柱 + {} 梁 + {} 板 + {} Mesh{}",
                        done.result.columns,
                        done.result.beams,
                        done.result.plates,
                        done.result.meshes,
                        if done.skip_zoom_extents { "（大型場景保護模式）" } else { "" },
                    ),
                    std::time::Instant::now(),
                ));
            }
            BackgroundTaskResult::Build(Err(e)) => {
                self.log_import_phase("scene_build_failed", e.clone());
                self.console_push("ERROR", format!("[Import] 場景建構失敗: {}", e));
                self.file_message = Some((format!("場景建構失敗:\n{}", e), std::time::Instant::now()));
            }
        }
    }

}

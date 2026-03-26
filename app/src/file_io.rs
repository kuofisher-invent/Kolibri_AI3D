use crate::app::KolibriApp;

impl KolibriApp {
    pub(crate) fn save_scene(&mut self) {
        let path = if let Some(ref p) = self.current_file {
            p.clone()
        } else {
            // Show save dialog
            let file = rfd::FileDialog::new()
                .set_title("儲存場景")
                .add_filter("Kolibri 3D", &["k3d"])
                .add_filter("所有檔案", &["*"])
                .set_file_name("scene.k3d")
                .save_file();
            match file {
                Some(p) => {
                    let path = p.to_string_lossy().to_string();
                    self.current_file = Some(path.clone());
                    path
                }
                None => return, // user cancelled
            }
        };
        match self.scene.save_to_file(&path) {
            Ok(()) => {
                self.add_recent_file(&path);
                self.last_saved_version = self.scene.version;
                self.console_push("INFO", format!("[File] 已儲存: {}", path));
                self.file_message = Some((format!("已儲存: {}", path), std::time::Instant::now()));
                tracing::info!("Scene saved to {}", path);
            }
            Err(e) => {
                self.console_push("ERROR", format!("[File] 儲存失敗: {}", e));
                self.file_message = Some((format!("儲存失敗: {}", e), std::time::Instant::now()));
                tracing::error!("Save failed: {}", e);
            }
        }
    }

    pub(crate) fn open_scene(&mut self) {
        let file = rfd::FileDialog::new()
            .set_title("開啟場景")
            .add_filter("Kolibri 3D", &["k3d"])
            .add_filter("OBJ 模型", &["obj"])
            .add_filter("所有檔案", &["*"])
            .pick_file();

        let path = match file {
            Some(p) => p.to_string_lossy().to_string(),
            None => return,
        };

        self.console_push("INFO", format!("[File] 開啟: {}", path));
        if path.ends_with(".obj") {
            match crate::obj_io::import_obj(&mut self.scene, &path) {
                Ok(count) => {
                    self.add_recent_file(&path);
                    self.editor.selected_ids.clear();
                    self.console_push("INFO", format!("[Import] OBJ 已匯入 {} 個物件", count));
                    self.file_message = Some((format!("已匯入 {} 個物件: {}", count, path), std::time::Instant::now()));
                }
                Err(e) => {
                    self.console_push("ERROR", format!("[Import] OBJ 匯入失敗: {}", e));
                    self.file_message = Some((format!("匯入失敗: {}", e), std::time::Instant::now()));
                }
            }
        } else {
            match self.scene.load_from_file(&path) {
                Ok(count) => {
                    self.current_file = Some(path.clone());
                    self.add_recent_file(&path);
                    self.editor.selected_ids.clear();
                    self.last_saved_version = self.scene.version;
                    self.console_push("INFO", format!("[File] 已載入 {} 個物件", count));
                    self.file_message = Some((format!("已載入 {} 個物件: {}", count, path), std::time::Instant::now()));
                    tracing::info!("Scene loaded from {}", path);
                }
                Err(e) => {
                    self.console_push("ERROR", format!("[File] 載入失敗: {}", e));
                    self.file_message = Some((format!("載入失敗: {}", e), std::time::Instant::now()));
                    tracing::error!("Open failed: {}", e);
                }
            }
        }
    }

    pub(crate) fn check_auto_save(&mut self) {
        if self.last_auto_save.elapsed().as_secs() >= 60
            && self.scene.version != self.auto_save_version
            && !self.scene.objects.is_empty()
        {
            let path = "D:\\AI_Design\\Kolibri_Ai3D\\app\\autosave.k3d";
            if let Ok(()) = self.scene.save_to_file(path) {
                self.auto_save_version = self.scene.version;
                self.last_auto_save = std::time::Instant::now();
                tracing::info!("Auto-saved to {}", path);
            }
        }
    }

    pub(crate) fn add_recent_file(&mut self, path: &str) {
        let path_str = path.to_string();
        self.recent_files.retain(|p| p != &path_str);
        self.recent_files.insert(0, path_str);
        if self.recent_files.len() > 5 {
            self.recent_files.truncate(5);
        }
    }

    pub(crate) fn poll_test_bridge(&mut self) {
        if let Some(input) = crate::test_bridge::check_pending() {
            tracing::info!("Test bridge: executing {} commands", input.commands.len());

            let device = self.device.clone();
            let queue = self.queue.clone();
            let viewport = &self.viewport;

            let mut screenshot_fn = |path: &str| {
                viewport.save_screenshot(&device, &queue, path);
            };

            // Bridge uses Option<String> for selected_id; adapt to/from Vec
            let mut bridge_selected: Option<String> = self.editor.selected_ids.first().cloned();
            crate::test_bridge::execute(
                input,
                &mut self.scene,
                &mut self.viewer.camera,
                &mut bridge_selected,
                &mut screenshot_fn,
            );
            self.editor.selected_ids = bridge_selected.into_iter().collect();

            crate::test_bridge::signal_done();
            tracing::info!("Test bridge: done");
        }
    }
}

use eframe::egui;

#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    None,
    // File
    NewScene,
    OpenScene,
    SaveScene,
    SaveAs,
    // Edit
    Undo,
    Redo,
    Delete,
    SelectAll,
    // View
    ViewFront,
    ViewBack,
    ViewLeft,
    ViewRight,
    ViewTop,
    ViewBottom,
    ViewIso,
    ZoomExtents,
    // Import/Export
    ExportObj,
    ImportObj,
    ExportStl,
    ImportStl,
    ExportGltf,
    ImportGltf,
    ExportDxf,
    ImportDxf,
    // CSG boolean
    CsgUnion,
    CsgSubtract,
    CsgIntersect,
    // Recent / Revert / Template
    OpenRecent(String),
    Revert,
    SaveTemplate,
    // 2D export / import
    ExportPng,
    ExportJpg,
    ExportPdf,
    ImportImage,
    // Context menu
    Duplicate,
    GroupSelected,
    ComponentSelected,
    Properties,
    // Render mode & background
    SetRenderMode(u32),
    ToggleBackground,
    // Camera modes
    ToggleOrtho,
    SaveCamera,
    SplitObject,
    ImportDxfSmart,
    SmartImport,
    #[cfg(feature = "drafting")]
    ImportDxfToDraft,   // DXF → 2D DraftDocument
    #[cfg(feature = "drafting")]
    ExportDraftDxf,     // 2D DraftDocument → DXF
    ToggleConsole,
    ToggleGrid,
    ToggleAxes,
    ToggleToolbar,
    ToggleRightPanel,
    ReverseFace,
}

/// Draw the top menu bar. Returns the action to execute.
pub fn draw_menu_bar(ui: &mut egui::Ui, has_selection: bool, can_undo: bool, can_redo: bool, _obj_count: usize, recent_files: &[String], has_current_file: bool, use_ortho: bool, saved_camera_count: usize) -> MenuAction {
    let mut action = MenuAction::None;

    let menu_font = 15.0; // 與 Ribbon tab 字體大小一致
    egui::menu::bar(ui, |ui| {
        ui.menu_button(egui::RichText::new("檔案").size(menu_font), |ui| {
            if ui.button("新建場景").clicked() { action = MenuAction::NewScene; ui.close_menu(); }
            if ui.button("開啟 (Ctrl+O)").clicked() { action = MenuAction::OpenScene; ui.close_menu(); }
            ui.menu_button("最近檔案", |ui| {
                if recent_files.is_empty() {
                    ui.label("(無)");
                } else {
                    for path in recent_files {
                        let short = path.rsplit(['\\', '/']).next().unwrap_or(path);
                        if ui.button(short).on_hover_text(path).clicked() {
                            action = MenuAction::OpenRecent(path.clone());
                            ui.close_menu();
                        }
                    }
                }
            });
            if ui.button("儲存 (Ctrl+S)").clicked() { action = MenuAction::SaveScene; ui.close_menu(); }
            if ui.button("另存新檔").clicked() { action = MenuAction::SaveAs; ui.close_menu(); }
            if ui.add_enabled(has_current_file, egui::Button::new("回復上次儲存")).clicked() {
                action = MenuAction::Revert;
                ui.close_menu();
            }
            if ui.button("存為範本").clicked() { action = MenuAction::SaveTemplate; ui.close_menu(); }
            ui.separator();
            ui.menu_button("匯出", |ui| {
                ui.label("3D 模型");
                if ui.button("OBJ 模型").clicked() { action = MenuAction::ExportObj; ui.close_menu(); }
                if ui.button("STL 模型").clicked() { action = MenuAction::ExportStl; ui.close_menu(); }
                if ui.button("GLTF 模型").clicked() { action = MenuAction::ExportGltf; ui.close_menu(); }
                if ui.button("DXF 圖面 (3D)").clicked() { action = MenuAction::ExportDxf; ui.close_menu(); }
                ui.separator();
                #[cfg(feature = "drafting")]
                if ui.button("DXF 圖面 (2D CAD)").clicked() { action = MenuAction::ExportDraftDxf; ui.close_menu(); }
                ui.separator();
                ui.label("2D 截圖");
                if ui.button("PNG 截圖").clicked() { action = MenuAction::ExportPng; ui.close_menu(); }
                if ui.button("JPG 截圖").clicked() { action = MenuAction::ExportJpg; ui.close_menu(); }
                if ui.button("PDF 文件").clicked() { action = MenuAction::ExportPdf; ui.close_menu(); }
            });
            ui.menu_button("匯入", |ui| {
                if ui.button("OBJ 模型").clicked() { action = MenuAction::ImportObj; ui.close_menu(); }
                if ui.button("STL 模型").clicked() { action = MenuAction::ImportStl; ui.close_menu(); }
                if ui.button("DXF/DWG 圖面 (自動偵測模式)").on_hover_text("3D 模式匯入到場景, 2D 模式匯入到出圖畫布").clicked() { action = MenuAction::ImportDxf; ui.close_menu(); }
                if ui.button("GLTF 模型").clicked() { action = MenuAction::ImportGltf; ui.close_menu(); }
                ui.separator();
                #[cfg(feature = "drafting")]
                if ui.button("DXF/DWG → 強制 2D CAD").on_hover_text("不論目前模式，強制匯入到 2D 出圖畫布").clicked() { action = MenuAction::ImportDxfToDraft; ui.close_menu(); }
                ui.separator();
                if ui.button("智慧匯入 (解析軸線/柱梁)").on_hover_text("DXF/DWG/PDF 結構解析, 2D 模式自動到畫布").clicked() { action = MenuAction::ImportDxfSmart; ui.close_menu(); }
                ui.separator();
                if ui.button("智慧匯入 (全格式)").on_hover_text("DXF/DWG/SKP/OBJ/PDF/STL, 2D 模式 DXF/DWG 自動到畫布").clicked() { action = MenuAction::SmartImport; ui.close_menu(); }
                ui.separator();
                if ui.button("參考圖片").clicked() { action = MenuAction::ImportImage; ui.close_menu(); }
            });
        });
        ui.menu_button(egui::RichText::new("編輯").size(menu_font), |ui| {
            if ui.add_enabled(can_undo, egui::Button::new("復原 (Ctrl+Z)")).clicked() { action = MenuAction::Undo; ui.close_menu(); }
            if ui.add_enabled(can_redo, egui::Button::new("重做 (Ctrl+Y)")).clicked() { action = MenuAction::Redo; ui.close_menu(); }
            ui.separator();
            if ui.add_enabled(has_selection, egui::Button::new("刪除 (Delete)")).clicked() { action = MenuAction::Delete; ui.close_menu(); }
            if ui.button("全選 (Ctrl+A)").clicked() { action = MenuAction::SelectAll; ui.close_menu(); }
        });
        ui.menu_button(egui::RichText::new("工具").size(menu_font), |ui| {
            ui.label("布林運算 (需選取2個方塊)");
            if ui.button("聯集 (A+B)").clicked() { action = MenuAction::CsgUnion; ui.close_menu(); }
            if ui.button("差集 (A-B)").clicked() { action = MenuAction::CsgSubtract; ui.close_menu(); }
            if ui.button("交集 (A∩B)").clicked() { action = MenuAction::CsgIntersect; ui.close_menu(); }
            ui.separator();
            ui.label("分割");
            if ui.button("分割物件 (沿最長軸)").clicked() { action = MenuAction::SplitObject; ui.close_menu(); }
        });
        ui.menu_button(egui::RichText::new("檢視").size(menu_font), |ui| {
            if ui.button("前視圖 (1)").clicked() { action = MenuAction::ViewFront; ui.close_menu(); }
            if ui.button("後視圖").clicked() { action = MenuAction::ViewBack; ui.close_menu(); }
            if ui.button("左視圖").clicked() { action = MenuAction::ViewLeft; ui.close_menu(); }
            if ui.button("右視圖").clicked() { action = MenuAction::ViewRight; ui.close_menu(); }
            if ui.button("上視圖 (2)").clicked() { action = MenuAction::ViewTop; ui.close_menu(); }
            if ui.button("下視圖").clicked() { action = MenuAction::ViewBottom; ui.close_menu(); }
            ui.separator();
            if ui.button("等角視圖 (3)").clicked() { action = MenuAction::ViewIso; ui.close_menu(); }
            if ui.button("全部顯示 (Z)").clicked() { action = MenuAction::ZoomExtents; ui.close_menu(); }
            ui.separator();
            ui.label("顯示模式");
            if ui.button("著色").clicked() { action = MenuAction::SetRenderMode(0); ui.close_menu(); }
            if ui.button("線框").clicked() { action = MenuAction::SetRenderMode(1); ui.close_menu(); }
            if ui.button("X光").clicked() { action = MenuAction::SetRenderMode(2); ui.close_menu(); }
            if ui.button("隱藏線").clicked() { action = MenuAction::SetRenderMode(3); ui.close_menu(); }
            if ui.button("單色").clicked() { action = MenuAction::SetRenderMode(4); ui.close_menu(); }
            if ui.button("草稿").clicked() { action = MenuAction::SetRenderMode(5); ui.close_menu(); }
            ui.separator();
            if ui.button("切換背景 (明/暗)").clicked() { action = MenuAction::ToggleBackground; ui.close_menu(); }
            ui.separator();
            let ortho_label = if use_ortho { "\u{2713} 平行投影 (5)" } else { "平行投影 (5)" };
            if ui.button(ortho_label).clicked() { action = MenuAction::ToggleOrtho; ui.close_menu(); }
            ui.separator();
            ui.label("場景");
            if ui.button("儲存目前視角").clicked() { action = MenuAction::SaveCamera; ui.close_menu(); }
            if saved_camera_count > 0 {
                ui.label(format!("已儲存 {} 個視角", saved_camera_count));
            }
            ui.separator();
            if ui.button("Console (F12)").clicked() { action = MenuAction::ToggleConsole; ui.close_menu(); }
        });
    });

    action
}

/// Draw context menu for right-click. Returns action to execute.
/// 右鍵選單回傳：MenuAction 或自訂指令名稱
pub fn draw_context_menu(ui: &mut egui::Ui, has_selection: bool) -> MenuAction {
    draw_context_menu_ext(ui, has_selection).0
}

/// 擴充右鍵選單，回傳 (MenuAction, Option<自訂指令>)
pub fn draw_context_menu_ext(ui: &mut egui::Ui, has_selection: bool) -> (MenuAction, Option<String>) {
    let mut action = MenuAction::None;
    let mut cmd: Option<String> = None;

    if has_selection {
        if ui.button("刪除").clicked() { action = MenuAction::Delete; ui.close_menu(); }
        if ui.button("複製 (Ctrl+D)").clicked() { cmd = Some("就地複製".into()); ui.close_menu(); }
        if ui.button("鏡射 X (Ctrl+M)").clicked() { cmd = Some("鏡射 X".into()); ui.close_menu(); }
        ui.separator();
        ui.menu_button("對齊", |ui| {
            if ui.button("左對齊").clicked() { cmd = Some("對齊左".into()); ui.close_menu(); }
            if ui.button("右對齊").clicked() { cmd = Some("對齊右".into()); ui.close_menu(); }
            if ui.button("上對齊").clicked() { cmd = Some("對齊上".into()); ui.close_menu(); }
            if ui.button("下對齊").clicked() { cmd = Some("對齊下".into()); ui.close_menu(); }
            ui.separator();
            if ui.button("X 中心").clicked() { cmd = Some("X中心對齊".into()); ui.close_menu(); }
            if ui.button("Y 中心").clicked() { cmd = Some("Y中心對齊".into()); ui.close_menu(); }
        });
        ui.menu_button("分佈", |ui| {
            if ui.button("X 等距").clicked() { cmd = Some("X等距分佈".into()); ui.close_menu(); }
            if ui.button("Y 等距").clicked() { cmd = Some("Y等距分佈".into()); ui.close_menu(); }
            if ui.button("Z 等距").clicked() { cmd = Some("Z等距分佈".into()); ui.close_menu(); }
        });
        ui.menu_button("CSG 布林", |ui| {
            if ui.button("聯集（合併）").clicked() { action = MenuAction::CsgUnion; ui.close_menu(); }
            if ui.button("差集（挖除）").clicked() { action = MenuAction::CsgSubtract; ui.close_menu(); }
            if ui.button("交集（保留重疊）").clicked() { action = MenuAction::CsgIntersect; ui.close_menu(); }
        });
        ui.separator();
        if ui.button("建立群組").clicked() { action = MenuAction::GroupSelected; ui.close_menu(); }
        if ui.button("建立元件").clicked() { action = MenuAction::ComponentSelected; ui.close_menu(); }
        if ui.button("反轉面").clicked() { action = MenuAction::ReverseFace; ui.close_menu(); }
        ui.separator();
        if ui.button("屬性").clicked() { action = MenuAction::Properties; ui.close_menu(); }
    } else {
        ui.label("(無選取物件)");
    }

    (action, cmd)
}

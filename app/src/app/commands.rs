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
            "顯示軸向" => self.viewer.show_axes = !self.viewer.show_axes,
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
            "隱藏選取" => {
                for id in &self.editor.selected_ids.clone() {
                    if let Some(obj) = self.scene.objects.get_mut(id) { obj.visible = false; }
                }
                self.scene.version += 1; self.editor.selected_ids.clear();
            }
            "顯示全部" => {
                for obj in self.scene.objects.values_mut() { obj.visible = true; }
                self.scene.version += 1;
            }
            "隔離顯示" => {
                let sel: std::collections::HashSet<String> = self.editor.selected_ids.iter().cloned().collect();
                for obj in self.scene.objects.values_mut() { obj.visible = sel.contains(&obj.id); }
                self.scene.version += 1;
            }
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

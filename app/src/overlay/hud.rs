//! HUD overlays: floor indicator, viewport axes, scale bar, viewport info,
//! import debug highlights, vertex debug, toasts

use eframe::egui;

use crate::app::{KolibriApp, CursorHint, DrawState, PullFace, RenderMode, ScaleHandle, SelectionMode, SnapType, Tool, WorkMode};
use crate::scene::Shape;

impl KolibriApp {
    /// Draw HUD overlays (floor indicator, axes, scale bar, viewport info, debug, toasts)
    pub(crate) fn draw_hud_overlays(
        &mut self,
        ui: &mut egui::Ui,
        vp: glam::Mat4,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
                // ── 樓層指示線 ──
                if self.viewer.current_floor != 0 {
                    let floor_y = self.viewer.current_floor as f32 * self.viewer.floor_height;
                    // 畫水平虛線標示樓層
                    let left_world = [-10000.0, floor_y, 0.0];
                    let right_world = [10000.0, floor_y, 0.0];
                    if let (Some(sl), Some(sr)) = (
                        Self::world_to_screen_vp(left_world, &vp, &rect),
                        Self::world_to_screen_vp(right_world, &vp, &rect),
                    ) {
                        let floor_color = egui::Color32::from_rgba_unmultiplied(76, 139, 245, 80);
                        super::draw_dashed_line(ui.painter(), sl, sr, egui::Stroke::new(1.0, floor_color), 8.0, 6.0);
                        let floor_name = match self.viewer.current_floor {
                            f if f < 0 => format!("B{}", -f),
                            0 => "GF".to_string(),
                            f => format!("{}F", f),
                        };
                        ui.painter().text(
                            egui::pos2(rect.min.x + 8.0, sl.y - 10.0),
                            egui::Align2::LEFT_BOTTOM,
                            &format!("── {} ({:.0}m) ──", floor_name, floor_y / 1000.0),
                            egui::FontId::proportional(10.0),
                            egui::Color32::from_rgba_unmultiplied(76, 139, 245, 120),
                        );
                    }
                }

                // ── Viewport axes indicator（右下角 XYZ 方向立方）──
                {
                    let ax_size = 40.0;
                    let ax_center = egui::pos2(rect.max.x - ax_size - 12.0, rect.max.y - ax_size - 40.0);
                    let ax_len = ax_size * 0.4;
                    // 從相機 view matrix 提取軸向（螢幕空間投影）
                    let view = self.viewer.camera.view();
                    let axes_3d = [
                        (glam::Vec3::X, egui::Color32::from_rgb(220, 60, 60), "X"),
                        (glam::Vec3::Y, egui::Color32::from_rgb(60, 180, 60), "Y"),
                        (glam::Vec3::Z, egui::Color32::from_rgb(60, 60, 220), "Z"),
                    ];
                    for (dir, color, label) in &axes_3d {
                        let view_dir = view.transform_vector3(*dir);
                        let sx = view_dir.x * ax_len;
                        let sy = -view_dir.y * ax_len; // screen Y is inverted
                        let tip = egui::pos2(ax_center.x + sx, ax_center.y + sy);
                        ui.painter().line_segment([ax_center, tip], egui::Stroke::new(2.0, *color));
                        ui.painter().text(
                            egui::pos2(tip.x + sx * 0.15, tip.y + sy * 0.15),
                            egui::Align2::CENTER_CENTER, label,
                            egui::FontId::proportional(9.0), *color,
                        );
                    }
                    // 中心圓
                    ui.painter().circle_filled(ax_center, 3.0, egui::Color32::from_rgba_unmultiplied(200, 200, 200, 150));
                    // 方位指示（N/S 對應 -Z/+Z）
                    let north_dir = view.transform_vector3(-glam::Vec3::Z); // -Z = North
                    let compass_len = ax_size * 0.52;
                    let nx = north_dir.x * compass_len;
                    let ny = -north_dir.y * compass_len;
                    let n_tip = egui::pos2(ax_center.x + nx, ax_center.y + ny);
                    ui.painter().text(n_tip, egui::Align2::CENTER_CENTER, "N",
                        egui::FontId::proportional(8.0), egui::Color32::from_rgba_unmultiplied(255, 80, 80, 180));
                }

                // ── Scale bar（左下角比例尺）──
                {
                    // 用兩個已知距離的 3D 點投影到螢幕，計算 pixel/mm
                    let origin = [0.0_f32, 0.0, 0.0];
                    let x1000 = [1000.0, 0.0, 0.0]; // 1m
                    if let (Some(sp0), Some(sp1)) = (
                        Self::world_to_screen_vp(origin, &vp, &rect),
                        Self::world_to_screen_vp(x1000, &vp, &rect),
                    ) {
                        let px_per_mm = ((sp1.x - sp0.x).powi(2) + (sp1.y - sp0.y).powi(2)).sqrt() / 1000.0;
                        if px_per_mm > 0.001 {
                            // 選擇適合的比例尺長度（50-150px）
                            let target_px = 100.0;
                            let mm_at_target = target_px / px_per_mm;
                            // 取整到好看的數字
                            let nice = [100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0];
                            let scale_mm = nice.iter().copied()
                                .min_by_key(|v| ((v - mm_at_target).abs() * 100.0) as i64)
                                .unwrap_or(1000.0);
                            let bar_px = scale_mm * px_per_mm;
                            let bar_y = rect.max.y - 32.0;
                            let bar_x = rect.min.x + 8.0;

                            let bar_color = egui::Color32::from_rgba_unmultiplied(160, 165, 180, 150);
                            // 橫線
                            ui.painter().line_segment(
                                [egui::pos2(bar_x, bar_y), egui::pos2(bar_x + bar_px, bar_y)],
                                egui::Stroke::new(2.0, bar_color),
                            );
                            // 左端帽
                            ui.painter().line_segment(
                                [egui::pos2(bar_x, bar_y - 4.0), egui::pos2(bar_x, bar_y + 4.0)],
                                egui::Stroke::new(1.5, bar_color),
                            );
                            // 右端帽
                            ui.painter().line_segment(
                                [egui::pos2(bar_x + bar_px, bar_y - 4.0), egui::pos2(bar_x + bar_px, bar_y + 4.0)],
                                egui::Stroke::new(1.5, bar_color),
                            );
                            // 標籤
                            let label = if scale_mm >= 1000.0 {
                                format!("{:.0} m", scale_mm / 1000.0)
                            } else {
                                format!("{:.0} mm", scale_mm)
                            };
                            ui.painter().text(
                                egui::pos2(bar_x + bar_px / 2.0, bar_y - 6.0),
                                egui::Align2::CENTER_BOTTOM, &label,
                                egui::FontId::proportional(9.0), bar_color,
                            );
                        }
                    }
                }

                // ── Viewport 資訊欄（左下角）──
                {
                    let obj_count = self.scene.objects.len();
                    let mode_name = match self.viewer.render_mode {
                        RenderMode::Shaded => "著色",
                        RenderMode::Wireframe => "線框",
                        RenderMode::XRay => "X光",
                        RenderMode::HiddenLine => "隱藏線",
                        RenderMode::Monochrome => "單色",
                        RenderMode::Sketch => "草稿",
                    };
                    let plane = match self.viewer.work_plane {
                        1 => "XY", 2 => "YZ", _ => "XZ",
                    };
                    // 選取物件的面積/體積
                    let measure_info = if self.editor.selected_ids.len() == 1 {
                        if let Some(obj) = self.editor.selected_ids.first()
                            .and_then(|id| self.scene.objects.get(id))
                        {
                            let area = kolibri_core::measure::surface_area(obj);
                            let vol = kolibri_core::measure::volume(obj);
                            format!(" | {} | {}", kolibri_core::measure::format_area(area), kolibri_core::measure::format_volume(vol))
                        } else { String::new() }
                    } else { String::new() };
                    let info = format!("{} 物件 | {} | 平面:{}{}", obj_count, mode_name, plane, measure_info);
                    ui.painter().text(
                        egui::pos2(rect.min.x + 8.0, rect.max.y - 8.0),
                        egui::Align2::LEFT_BOTTOM,
                        &info,
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgba_unmultiplied(160, 165, 180, 150),
                    );
                }

                if self.background_task_active() {
                    let overlay_rect = egui::Rect::from_center_size(
                        rect.center(),
                        egui::vec2(420.0, 150.0),
                    );
                    let elapsed = self
                        .background_task_elapsed()
                        .map(|d| format!("{:.1}s", d.as_secs_f32()))
                        .unwrap_or_else(|| "0.0s".to_string());
                    let label = self
                        .background_task_label
                        .clone()
                        .unwrap_or_else(|| "背景工作進行中".to_string());

                    ui.painter().rect_filled(
                        rect,
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(245, 246, 250, 180),
                    );
                    ui.painter().rect_filled(
                        overlay_rect,
                        18.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 245),
                    );
                    ui.painter().rect_stroke(
                        overlay_rect,
                        18.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 139, 245)),
                    );
                    ui.painter().circle_stroke(
                        egui::pos2(overlay_rect.center().x, overlay_rect.top() + 34.0),
                        14.0,
                        egui::Stroke::new(3.0, egui::Color32::from_rgb(76, 139, 245)),
                    );
                    ui.painter().text(
                        egui::pos2(overlay_rect.center().x, overlay_rect.top() + 58.0),
                        egui::Align2::CENTER_TOP,
                        label,
                        egui::FontId::proportional(18.0),
                        egui::Color32::from_rgb(31, 36, 48),
                    );
                    ui.painter().text(
                        egui::pos2(overlay_rect.center().x, overlay_rect.top() + 88.0),
                        egui::Align2::CENTER_TOP,
                        format!("已執行 {}", elapsed),
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_rgb(110, 118, 135),
                    );
                    ui.painter().text(
                        egui::pos2(overlay_rect.center().x, overlay_rect.top() + 112.0),
                        egui::Align2::CENTER_TOP,
                        "匯入期間已暫停互動與 autosave，請稍候。",
                        egui::FontId::proportional(12.0),
                        egui::Color32::from_rgb(110, 118, 135),
                    );
                }

                // ── 匯入來源面高亮（SDK provenance debug）──
                // 只在小場景（< 50 物件）且有 debug 資料時才畫（大場景下此 overlay 耗時 600ms+）
                if self.scene.objects.len() < 50 && !self.import_object_debug.is_empty() {
                    let target_faces = [
                        ("F14", egui::Color32::from_rgba_unmultiplied(255, 0, 0, 120)),
                        ("F35", egui::Color32::from_rgba_unmultiplied(0, 170, 255, 120)),
                    ];
                    let outline_colors = [
                        ("F14", egui::Color32::from_rgb(255, 240, 120)),
                        ("F35", egui::Color32::from_rgb(255, 255, 255)),
                    ];
                    let label_font = egui::FontId::monospace(16.0);
                    let label_bg = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180);

                    for obj in self.scene.objects.values() {
                        if !obj.visible { continue; }
                        let Some(debug) = self.import_object_debug.get(&obj.id) else { continue; };
                        let crate::scene::Shape::Mesh(ref mesh) = obj.shape else { continue; };

                        let mut ordered_vertices: Vec<_> = mesh.vertices.iter().collect();
                        ordered_vertices.sort_by_key(|(vid, _)| **vid);

                        let vertex_positions: Vec<[f32; 3]> = ordered_vertices
                            .iter()
                            .map(|(_, vert)| {
                                [
                                    vert.pos[0] + obj.position[0],
                                    vert.pos[1] + obj.position[1],
                                    vert.pos[2] + obj.position[2],
                                ]
                            })
                            .collect();

                        for (face_label, fill_color) in target_faces {
                            let mut label_anchor: Option<egui::Pos2> = None;
                            let outline_color = outline_colors
                                .iter()
                                .find(|(label, _)| *label == face_label)
                                .map(|(_, color)| *color)
                                .unwrap_or(egui::Color32::WHITE);
                            let outline_stroke = egui::Stroke::new(3.0, outline_color);
                            for tri in debug.triangle_debug.iter().filter(|tri| tri.source_face_label == face_label) {
                                let Some(&a) = tri.indices.get(0) else { continue; };
                                let Some(&b) = tri.indices.get(1) else { continue; };
                                let Some(&c) = tri.indices.get(2) else { continue; };
                                let (ai, bi, ci) = (a as usize, b as usize, c as usize);
                                let (Some(&pa), Some(&pb), Some(&pc)) = (
                                    vertex_positions.get(ai),
                                    vertex_positions.get(bi),
                                    vertex_positions.get(ci),
                                ) else { continue; };
                                let (Some(sa), Some(sb), Some(sc)) = (
                                    Self::world_to_screen_vp(pa, &vp, &rect),
                                    Self::world_to_screen_vp(pb, &vp, &rect),
                                    Self::world_to_screen_vp(pc, &vp, &rect),
                                ) else { continue; };

                                ui.painter().add(egui::Shape::convex_polygon(
                                    vec![sa, sb, sc],
                                    fill_color,
                                    outline_stroke,
                                ));
                                ui.painter().line_segment([sa, sb], outline_stroke);
                                ui.painter().line_segment([sb, sc], outline_stroke);
                                ui.painter().line_segment([sc, sa], outline_stroke);
                                ui.painter().circle_filled(sa, 5.0, outline_color);
                                ui.painter().circle_filled(sb, 5.0, outline_color);
                                ui.painter().circle_filled(sc, 5.0, outline_color);

                                if label_anchor.is_none() {
                                    label_anchor = Some(egui::pos2(
                                        (sa.x + sb.x + sc.x) / 3.0,
                                        (sa.y + sb.y + sc.y) / 3.0,
                                    ));
                                }
                            }

                            if let Some(anchor) = label_anchor {
                                ui.painter().circle_stroke(anchor, 14.0, egui::Stroke::new(3.0, outline_color));
                                ui.painter().line_segment(
                                    [anchor + egui::vec2(-18.0, 0.0), anchor + egui::vec2(18.0, 0.0)],
                                    egui::Stroke::new(2.0, outline_color),
                                );
                                ui.painter().line_segment(
                                    [anchor + egui::vec2(0.0, -18.0), anchor + egui::vec2(0.0, 18.0)],
                                    egui::Stroke::new(2.0, outline_color),
                                );
                                let galley = ui.painter().layout_no_wrap(
                                    face_label.to_string(),
                                    label_font.clone(),
                                    egui::Color32::WHITE,
                                );
                                let text_rect = egui::Rect::from_center_size(
                                    anchor + egui::vec2(0.0, -28.0),
                                    galley.size() + egui::vec2(12.0, 6.0),
                                );
                                ui.painter().rect_filled(text_rect, 4.0, label_bg);
                                ui.painter().rect_stroke(
                                    text_rect,
                                    4.0,
                                    egui::Stroke::new(2.0, outline_color),
                                );
                                ui.painter().galley(
                                    text_rect.center() - galley.size() * 0.5,
                                    galley,
                                    egui::Color32::WHITE,
                                );
                            }
                        }
                    }
                } // end if scene.objects.len() < 50

                // ── 頂點編號除錯（show_vertex_ids）──
                if self.viewer.show_vertex_ids && self.scene.objects.len() < 50 {
                    let label_font = egui::FontId::monospace(10.0);
                    let label_color = egui::Color32::from_rgb(255, 80, 80);
                    let bg_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160);
                    for obj in self.scene.objects.values() {
                        if !obj.visible { continue; }
                        if let crate::scene::Shape::Mesh(ref mesh) = obj.shape {
                            let pos = obj.position;
                            let mut ordered_vertices: Vec<_> = mesh.vertices.iter().collect();
                            ordered_vertices.sort_by_key(|(vid, _)| **vid);
                            let source_labels = self
                                .import_object_debug
                                .get(&obj.id)
                                .map(|debug| &debug.vertex_labels);
                            for (vertex_index, (&vid, vert)) in ordered_vertices.into_iter().enumerate() {
                                let wp = [
                                    vert.pos[0] + pos[0],
                                    vert.pos[1] + pos[1],
                                    vert.pos[2] + pos[2],
                                ];
                                if let Some(sp) = Self::world_to_screen_vp(wp, &vp, &rect) {
                                    let text = source_labels
                                        .and_then(|labels| labels.get(vertex_index))
                                        .cloned()
                                        .unwrap_or_else(|| format!("{}", vid));
                                    let galley = ui.painter().layout_no_wrap(text, label_font.clone(), label_color);
                                    let text_rect = egui::Rect::from_min_size(
                                        egui::pos2(sp.x + 3.0, sp.y - 6.0),
                                        galley.size(),
                                    ).expand(1.5);
                                    ui.painter().rect_filled(text_rect, 2.0, bg_color);
                                    ui.painter().galley(egui::pos2(sp.x + 3.0, sp.y - 6.0), galley, label_color);
                                    // 頂點小圓點
                                    ui.painter().circle_filled(sp, 2.5, egui::Color32::from_rgb(255, 200, 60));
                                }
                            }
                        }
                    }
                }

                // ── Toast 通知（右下角堆疊）──
                {
                    let now = std::time::Instant::now();
                    self.toasts.retain(|(_, t)| now.duration_since(*t).as_secs_f32() < 4.0);
                    let toast_w = 250.0;
                    let toast_h = 28.0;
                    let margin = 12.0;
                    for (i, (msg, when)) in self.toasts.iter().rev().enumerate() {
                        let age = now.duration_since(*when).as_secs_f32();
                        let alpha = if age > 3.0 { ((4.0 - age) * 255.0) as u8 } else { 230 };
                        let y = rect.max.y - margin - (i as f32) * (toast_h + 6.0) - toast_h;
                        let toast_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.max.x - margin - toast_w, y),
                            egui::vec2(toast_w, toast_h),
                        );
                        ui.painter().rect_filled(toast_rect, 8.0,
                            egui::Color32::from_rgba_unmultiplied(40, 42, 55, alpha));
                        ui.painter().rect_stroke(toast_rect, 8.0,
                            egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(76, 139, 245, alpha)));
                        ui.painter().text(
                            toast_rect.left_center() + egui::vec2(10.0, 0.0),
                            egui::Align2::LEFT_CENTER, msg,
                            egui::FontId::proportional(12.0),
                            egui::Color32::from_rgba_unmultiplied(230, 235, 245, alpha),
                        );
                    }
                }
    }
}

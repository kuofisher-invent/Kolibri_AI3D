//! 2D 出圖畫布 — 繪製 DraftDocument 的所有 2D 實體
//! 用 egui Painter 直接繪製在 CentralPanel 上

use eframe::egui;
use crate::app::{KolibriApp, Tool};

/// 2D 畫布狀態（未來支援 pan/zoom 時使用）
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct CanvasState {
    /// 畫布偏移（像素）
    pub offset: egui::Vec2,
    /// 縮放（像素/mm）
    pub zoom: f32,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            offset: egui::Vec2::ZERO,
            zoom: 2.0, // 2px per mm
        }
    }
}

impl KolibriApp {
    /// 繪製 2D 出圖畫布（layout_mode 時取代 3D viewport）
    #[cfg(feature = "drafting")]
    pub(crate) fn draw_draft_canvas(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        // 背景（ZWCAD 深色 — 無白紙，直接模型空間）
        let bg_color = egui::Color32::from_rgb(33, 40, 48);
        painter.rect_filled(rect, 0.0, bg_color);

        // 座標系統：原點在畫布中央，1mm = 2px（可日後加 pan/zoom）
        let scale = 2.0_f32;
        let origin = rect.center(); // 螢幕原點 = 數學原點 (0,0)

        // mm → screen 座標轉換（Y 軸翻轉：數學 Y 向上，螢幕 Y 向下）
        let to_screen = |mm_x: f64, mm_y: f64| -> egui::Pos2 {
            egui::pos2(
                origin.x + mm_x as f32 * scale,
                origin.y - mm_y as f32 * scale, // Y 翻轉
            )
        };

        // 紙張參數（用於底部資訊）
        let layout_paper_label = self.viewer.layout.paper_size.label();
        let layout_orientation = self.viewer.layout.orientation;
        let (paper_w, paper_h) = self.viewer.layout.paper_size.dimensions_mm();
        let (pw, ph) = if layout_orientation == crate::layout::Orientation::Landscape {
            (paper_h, paper_w)
        } else {
            (paper_w, paper_h)
        };

        // 繪製點格線（ZWCAD 風格：深色背景上的點陣，覆蓋可見區域）
        if self.viewer.show_grid {
            let grid_mm = 10.0_f64;
            let dot_color = egui::Color32::from_rgba_unmultiplied(90, 100, 115, 70);
            let dot_r = 0.7;
            // 計算可見範圍（螢幕座標 → mm）
            let mm_left = ((rect.left() - origin.x) / scale) as f64;
            let mm_right = ((rect.right() - origin.x) / scale) as f64;
            let mm_top = ((origin.y - rect.top()) / scale) as f64;    // Y 翻轉
            let mm_bottom = ((origin.y - rect.bottom()) / scale) as f64;
            let x_start = (mm_left / grid_mm).floor() as i64;
            let x_end = (mm_right / grid_mm).ceil() as i64;
            let y_start = (mm_bottom / grid_mm).floor() as i64;
            let y_end = (mm_top / grid_mm).ceil() as i64;
            for ix in x_start..=x_end {
                for iy in y_start..=y_end {
                    let sp = to_screen(ix as f64 * grid_mm, iy as f64 * grid_mm);
                    if rect.contains(sp) {
                        painter.circle_filled(sp, dot_r, dot_color);
                    }
                }
            }
        }

        // XY 軸指示器（左下角，ZWCAD 風格）
        {
            let axis_origin = egui::pos2(rect.left() + 40.0, rect.bottom() - 50.0);
            let axis_len = 35.0;
            let red = egui::Color32::from_rgb(220, 60, 60);
            let green = egui::Color32::from_rgb(60, 200, 60);
            let white = egui::Color32::from_rgb(200, 200, 200);
            // X 軸（紅，向右）
            painter.line_segment(
                [axis_origin, egui::pos2(axis_origin.x + axis_len, axis_origin.y)],
                egui::Stroke::new(1.5, red),
            );
            painter.text(
                egui::pos2(axis_origin.x + axis_len + 4.0, axis_origin.y),
                egui::Align2::LEFT_CENTER, "X",
                egui::FontId::proportional(11.0), red,
            );
            // Y 軸（綠，向上）
            painter.line_segment(
                [axis_origin, egui::pos2(axis_origin.x, axis_origin.y - axis_len)],
                egui::Stroke::new(1.5, green),
            );
            painter.text(
                egui::pos2(axis_origin.x, axis_origin.y - axis_len - 8.0),
                egui::Align2::CENTER_BOTTOM, "Y",
                egui::FontId::proportional(11.0), green,
            );
            // 原點方塊
            painter.rect_filled(
                egui::Rect::from_center_size(axis_origin, egui::vec2(5.0, 5.0)),
                1.0, white,
            );
        }

        // 十字游標（ZWCAD 風格：跟隨滑鼠的全畫面十字線）
        if let Some(hover_pos) = response.hover_pos() {
            let cross_color = egui::Color32::from_rgba_unmultiplied(180, 190, 200, 120);
            // 水平線
            painter.line_segment(
                [egui::pos2(rect.left(), hover_pos.y), egui::pos2(rect.right(), hover_pos.y)],
                egui::Stroke::new(0.5, cross_color),
            );
            // 垂直線
            painter.line_segment(
                [egui::pos2(hover_pos.x, rect.top()), egui::pos2(hover_pos.x, rect.bottom())],
                egui::Stroke::new(0.5, cross_color),
            );
            // 中心小十字（粗）
            let arm = 10.0;
            painter.line_segment(
                [egui::pos2(hover_pos.x - arm, hover_pos.y), egui::pos2(hover_pos.x + arm, hover_pos.y)],
                egui::Stroke::new(1.2, egui::Color32::from_rgb(220, 220, 230)),
            );
            painter.line_segment(
                [egui::pos2(hover_pos.x, hover_pos.y - arm), egui::pos2(hover_pos.x, hover_pos.y + arm)],
                egui::Stroke::new(1.2, egui::Color32::from_rgb(220, 220, 230)),
            );
        }

        // 繪製所有 draft 圖元（深色背景 → 亮色線條）
        let dim_color = egui::Color32::from_rgb(0, 220, 220); // cyan（ZWCAD 標註色）
        let text_color = egui::Color32::from_rgb(220, 220, 50); // 黃色文字

        for obj in &self.editor.draft_doc.objects {
            if !obj.visible { continue; }
            // 深色背景：黑色線改白色，其他保留
            let color = if obj.color == [0, 0, 0] {
                egui::Color32::from_rgb(230, 230, 230) // 白色線條
            } else {
                egui::Color32::from_rgb(obj.color[0], obj.color[1], obj.color[2])
            };
            let lw = (obj.line_weight as f32 * scale).max(0.5);
            let st = egui::Stroke::new(lw, color);

            match &obj.entity {
                kolibri_drafting::DraftEntity::Line { start, end } => {
                    painter.line_segment([to_screen(start[0], start[1]), to_screen(end[0], end[1])], st);
                }
                kolibri_drafting::DraftEntity::Circle { center, radius } => {
                    let c = to_screen(center[0], center[1]);
                    let r = *radius as f32 * scale;
                    painter.circle_stroke(c, r, st);
                }
                kolibri_drafting::DraftEntity::Arc { center, radius, start_angle, end_angle } => {
                    let c = to_screen(center[0], center[1]);
                    let r = *radius as f32 * scale;
                    // 用折線近似圓弧
                    let n = 32;
                    let mut points = Vec::with_capacity(n + 1);
                    for i in 0..=n {
                        let t = *start_angle + (*end_angle - *start_angle) * i as f64 / n as f64;
                        points.push(egui::pos2(
                            c.x + r * t.cos() as f32,
                            c.y - r * t.sin() as f32,
                        ));
                    }
                    for w in points.windows(2) {
                        painter.line_segment([w[0], w[1]], st);
                    }
                }
                kolibri_drafting::DraftEntity::Rectangle { p1, p2 } => {
                    let s1 = to_screen(p1[0], p1[1]);
                    let s2 = to_screen(p2[0], p2[1]);
                    let r = egui::Rect::from_two_pos(s1, s2);
                    painter.rect_stroke(r, 0.0, st);
                }
                kolibri_drafting::DraftEntity::Polyline { points, closed } => {
                    let screen_pts: Vec<egui::Pos2> = points.iter()
                        .map(|p| to_screen(p[0], p[1])).collect();
                    for w in screen_pts.windows(2) {
                        painter.line_segment([w[0], w[1]], st);
                    }
                    if *closed && screen_pts.len() >= 2 {
                        painter.line_segment([*screen_pts.last().unwrap(), screen_pts[0]], st);
                    }
                }
                kolibri_drafting::DraftEntity::Ellipse { center, semi_major, semi_minor, rotation } => {
                    let c = to_screen(center[0], center[1]);
                    let n = 48;
                    let mut points = Vec::with_capacity(n + 1);
                    for i in 0..=n {
                        let t = std::f64::consts::TAU * i as f64 / n as f64;
                        let x = *semi_major * t.cos();
                        let y = *semi_minor * t.sin();
                        let rx = x * rotation.cos() - y * rotation.sin();
                        let ry = x * rotation.sin() + y * rotation.cos();
                        points.push(egui::pos2(
                            c.x + rx as f32 * scale,
                            c.y - ry as f32 * scale,
                        ));
                    }
                    for w in points.windows(2) {
                        painter.line_segment([w[0], w[1]], st);
                    }
                }
                kolibri_drafting::DraftEntity::Text { position, content, height, .. } => {
                    let p = to_screen(position[0], position[1]);
                    let font_size = (*height as f32 * scale).max(8.0);
                    painter.text(p, egui::Align2::LEFT_TOP, content,
                        egui::FontId::proportional(font_size), text_color);
                }
                kolibri_drafting::DraftEntity::DimLinear { p1, p2, offset, text_override } => {
                    self.draw_dim_linear(&painter, &to_screen, p1, p2, *offset, text_override.as_deref(), scale, dim_color);
                }
                kolibri_drafting::DraftEntity::DimAligned { p1, p2, offset, text_override } => {
                    self.draw_dim_linear(&painter, &to_screen, p1, p2, *offset, text_override.as_deref(), scale, dim_color);
                }
                kolibri_drafting::DraftEntity::DimAngle { center, p1, p2, radius } => {
                    let c = to_screen(center[0], center[1]);
                    let r = *radius as f32 * scale;
                    let a1 = (p1[1] - center[1]).atan2(p1[0] - center[0]);
                    let a2 = (p2[1] - center[1]).atan2(p2[0] - center[0]);
                    // 圓弧
                    let n = 24;
                    let mut pts = Vec::with_capacity(n + 1);
                    for i in 0..=n {
                        let t = a1 + (a2 - a1) * i as f64 / n as f64;
                        pts.push(egui::pos2(
                            c.x + r * t.cos() as f32,
                            c.y - r * t.sin() as f32,
                        ));
                    }
                    for w in pts.windows(2) {
                        painter.line_segment([w[0], w[1]], egui::Stroke::new(0.8, dim_color));
                    }
                    // 角度文字
                    let angle_deg = (a2 - a1).to_degrees().abs();
                    let mid_a = (a1 + a2) / 2.0;
                    let text_pos = egui::pos2(
                        c.x + (r + 10.0) * mid_a.cos() as f32,
                        c.y - (r + 10.0) * mid_a.sin() as f32,
                    );
                    painter.text(text_pos, egui::Align2::CENTER_CENTER,
                        format!("{:.1}°", angle_deg),
                        egui::FontId::proportional(10.0), dim_color);
                }
                kolibri_drafting::DraftEntity::DimRadius { center, radius, angle } => {
                    let c = to_screen(center[0], center[1]);
                    let r = *radius as f32 * scale;
                    let ep = egui::pos2(
                        c.x + r * angle.cos() as f32,
                        c.y - r * angle.sin() as f32,
                    );
                    painter.line_segment([c, ep], egui::Stroke::new(0.8, dim_color));
                    let mid = egui::pos2((c.x + ep.x) / 2.0, (c.y + ep.y) / 2.0 - 8.0);
                    painter.text(mid, egui::Align2::CENTER_BOTTOM,
                        format!("R{:.0}", radius),
                        egui::FontId::proportional(10.0), dim_color);
                }
                kolibri_drafting::DraftEntity::DimDiameter { center, radius, angle } => {
                    let c = to_screen(center[0], center[1]);
                    let r = *radius as f32 * scale;
                    let ep1 = egui::pos2(
                        c.x + r * angle.cos() as f32,
                        c.y - r * angle.sin() as f32,
                    );
                    let ep2 = egui::pos2(
                        c.x - r * angle.cos() as f32,
                        c.y + r * angle.sin() as f32,
                    );
                    painter.line_segment([ep1, ep2], egui::Stroke::new(0.8, dim_color));
                    let mid = egui::pos2(c.x, c.y - 8.0);
                    painter.text(mid, egui::Align2::CENTER_BOTTOM,
                        format!("⌀{:.0}", radius * 2.0),
                        egui::FontId::proportional(10.0), dim_color);
                }
                kolibri_drafting::DraftEntity::Leader { points, text } => {
                    let screen_pts: Vec<egui::Pos2> = points.iter()
                        .map(|p| to_screen(p[0], p[1])).collect();
                    for w in screen_pts.windows(2) {
                        painter.line_segment([w[0], w[1]], egui::Stroke::new(0.8, dim_color));
                    }
                    if let Some(last) = screen_pts.last() {
                        painter.text(*last + egui::vec2(4.0, -4.0), egui::Align2::LEFT_BOTTOM,
                            text, egui::FontId::proportional(10.0), dim_color);
                    }
                    // 箭頭
                    if screen_pts.len() >= 2 {
                        let tip = screen_pts[0];
                        let from = screen_pts[1];
                        let dir = (tip - from).normalized();
                        let perp = egui::vec2(-dir.y, dir.x);
                        let arrow_len = 6.0;
                        let a1 = tip - dir * arrow_len + perp * 2.5;
                        let a2 = tip - dir * arrow_len - perp * 2.5;
                        painter.add(egui::Shape::convex_polygon(
                            vec![tip, a1, a2],
                            dim_color,
                            egui::Stroke::NONE,
                        ));
                    }
                }
                kolibri_drafting::DraftEntity::Hatch { boundary, pattern, .. } => {
                    let screen_pts: Vec<egui::Pos2> = boundary.iter()
                        .map(|p| to_screen(p[0], p[1])).collect();
                    if screen_pts.len() >= 3 {
                        let fill = egui::Color32::from_rgba_unmultiplied(
                            color.r(), color.g(), color.b(), 30);
                        painter.add(egui::Shape::convex_polygon(
                            screen_pts.clone(), fill, st,
                        ));
                    }
                }
                kolibri_drafting::DraftEntity::Polygon { center, radius, sides, inscribed } => {
                    let pts = kolibri_drafting::geometry::polygon_points(center, *radius, *sides, *inscribed);
                    let spts: Vec<egui::Pos2> = pts.iter().map(|p| to_screen(p[0], p[1])).collect();
                    for i in 0..spts.len() {
                        painter.line_segment([spts[i], spts[(i + 1) % spts.len()]], st);
                    }
                }
                kolibri_drafting::DraftEntity::Spline { points, closed } => {
                    let smooth = kolibri_drafting::geometry::spline_interpolate(points, 8);
                    let spts: Vec<egui::Pos2> = smooth.iter().map(|p| to_screen(p[0], p[1])).collect();
                    for w in spts.windows(2) { painter.line_segment([w[0], w[1]], st); }
                    if *closed && spts.len() >= 2 {
                        painter.line_segment([*spts.last().unwrap(), spts[0]], st);
                    }
                }
                kolibri_drafting::DraftEntity::Point { position } => {
                    let sp = to_screen(position[0], position[1]);
                    // 十字+圓點（AutoCAD PDMODE 3 風格）
                    let arm = 3.0;
                    painter.line_segment([egui::pos2(sp.x - arm, sp.y), egui::pos2(sp.x + arm, sp.y)], st);
                    painter.line_segment([egui::pos2(sp.x, sp.y - arm), egui::pos2(sp.x, sp.y + arm)], st);
                    painter.circle_filled(sp, 1.5, color);
                }
                kolibri_drafting::DraftEntity::Xline { base, direction } => {
                    // 建構線：畫一條很長的線穿過 base
                    let len = 10000.0;
                    let dl = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
                    if dl > 1e-6 {
                        let ux = direction[0] / dl;
                        let uy = direction[1] / dl;
                        let p1 = [base[0] - ux * len, base[1] - uy * len];
                        let p2 = [base[0] + ux * len, base[1] + uy * len];
                        painter.line_segment([to_screen(p1[0], p1[1]), to_screen(p2[0], p2[1])],
                            egui::Stroke::new(lw, egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 80)));
                    }
                }
                kolibri_drafting::DraftEntity::BlockRef { name, insert_point, .. } => {
                    let sp = to_screen(insert_point[0], insert_point[1]);
                    // 圖塊符號：菱形 + 名稱
                    let sz = 6.0;
                    painter.line_segment([egui::pos2(sp.x, sp.y - sz), egui::pos2(sp.x + sz, sp.y)], st);
                    painter.line_segment([egui::pos2(sp.x + sz, sp.y), egui::pos2(sp.x, sp.y + sz)], st);
                    painter.line_segment([egui::pos2(sp.x, sp.y + sz), egui::pos2(sp.x - sz, sp.y)], st);
                    painter.line_segment([egui::pos2(sp.x - sz, sp.y), egui::pos2(sp.x, sp.y - sz)], st);
                    painter.text(egui::pos2(sp.x + sz + 2.0, sp.y),
                        egui::Align2::LEFT_CENTER, name,
                        egui::FontId::proportional(9.0), color);
                }
            }
        }

        // ── Hover 高亮（滑鼠靠近圖元時亮色顯示）──
        let hover_mm = response.hover_pos().map(|pos| {
            [((pos.x - origin.x) / scale) as f64, ((origin.y - pos.y) / scale) as f64]
        });
        let mut hovered_entity_id: Option<kolibri_drafting::DraftId> = None;
        if let Some(mm) = hover_mm {
            let mut best_dist = 5.0_f64; // 5mm 容差
            for obj in &self.editor.draft_doc.objects {
                if !obj.visible { continue; }
                let d = self.draft_entity_distance(&obj.entity, mm[0], mm[1]);
                if d < best_dist {
                    best_dist = d;
                    hovered_entity_id = Some(obj.id);
                }
            }
        }
        // 繪製 hover 高亮
        if let Some(hid) = hovered_entity_id {
            let hover_stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 200, 255));
            if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == hid) {
                self.draw_entity_highlight(&painter, &to_screen, scale, &obj.entity, hover_stroke);
            }
        }

        // ── 選取高亮 + grip 控制點 ──
        let sel_stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(76, 139, 245));
        let grip_color = egui::Color32::from_rgb(76, 139, 245);
        let grip_size = 4.0;
        for &sel_id in &self.editor.draft_selected {
            if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == sel_id) {
                self.draw_entity_highlight(&painter, &to_screen, scale, &obj.entity, sel_stroke);
                // Grip points（端點小方塊）
                let grips = self.entity_grip_points(&obj.entity);
                for gp in grips {
                    let sp = to_screen(gp[0], gp[1]);
                    painter.rect_filled(
                        egui::Rect::from_center_size(sp, egui::vec2(grip_size, grip_size)),
                        0.0, grip_color);
                    painter.rect_stroke(
                        egui::Rect::from_center_size(sp, egui::vec2(grip_size, grip_size)),
                        0.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                }
            }
        }

        // ── 繪製進行中的繪製狀態 ──
        self.draw_draft_preview(&painter, &to_screen, scale, &response);

        // ── 處理滑鼠點擊 ──
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let mm_x = ((pos.x - origin.x) / scale) as f64;
                let mm_y = ((origin.y - pos.y) / scale) as f64;
                let shift = ui.input(|i| i.modifiers.shift);
                self.handle_draft_click_v2(mm_x, mm_y, shift);
            }
        }

        // 右鍵：結束多段線 / 結束繪圖回到 Select
        if response.secondary_clicked() {
            #[cfg(feature = "drafting")]
            {
                use crate::editor::DraftDrawState;
                if !matches!(self.editor.draft_state, DraftDrawState::Idle) {
                    self.finish_draft_tool();
                } else {
                    // 右鍵在 Idle 狀態 → 回到選取
                    self.editor.tool = Tool::DraftSelect;
                    self.editor.draft_state = DraftDrawState::Idle;
                }
            }
        }

        // ── 拖曳框選（左鍵拖曳在 Select 工具時）──
        #[cfg(feature = "drafting")]
        {
            if self.editor.tool == Tool::DraftSelect {
                if response.drag_started() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        self.editor.rubber_band = Some((
                            egui::pos2(pos.x, pos.y),
                            egui::pos2(pos.x, pos.y),
                        ));
                    }
                }
                if response.dragged() {
                    if let Some((start, ref mut end)) = self.editor.rubber_band {
                        if let Some(pos) = response.interact_pointer_pos() {
                            *end = egui::pos2(pos.x, pos.y);
                        }
                    }
                }
                if response.drag_stopped() {
                    if let Some((start, end)) = self.editor.rubber_band.take() {
                        // 框選：選取框內所有圖元
                        let mm_left = ((start.x.min(end.x) - origin.x) / scale) as f64;
                        let mm_right = ((start.x.max(end.x) - origin.x) / scale) as f64;
                        let mm_top = ((origin.y - start.y.min(end.y)) / scale) as f64;
                        let mm_bottom = ((origin.y - start.y.max(end.y)) / scale) as f64;
                        self.editor.draft_selected.clear();
                        for obj in &self.editor.draft_doc.objects {
                            if !obj.visible { continue; }
                            let grips = self.entity_grip_points(&obj.entity);
                            let inside = grips.iter().any(|gp| {
                                gp[0] >= mm_left && gp[0] <= mm_right &&
                                gp[1] >= mm_bottom && gp[1] <= mm_top
                            });
                            if inside {
                                self.editor.draft_selected.push(obj.id);
                            }
                        }
                        if !self.editor.draft_selected.is_empty() {
                            self.console_push("INFO", format!("框選 {} 個圖元", self.editor.draft_selected.len()));
                        }
                    }
                }
                // 繪製選取框
                if let Some((start, end)) = self.editor.rubber_band {
                    let is_left_to_right = end.x >= start.x;
                    let box_color = if is_left_to_right {
                        egui::Color32::from_rgba_unmultiplied(76, 139, 245, 30) // 藍色 window
                    } else {
                        egui::Color32::from_rgba_unmultiplied(60, 200, 60, 20) // 綠色 crossing
                    };
                    let box_stroke = if is_left_to_right {
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(76, 139, 245))
                    } else {
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 200, 60))
                    };
                    let sel_rect = egui::Rect::from_two_pos(start, end);
                    painter.rect_filled(sel_rect, 0.0, box_color);
                    if is_left_to_right {
                        painter.rect_stroke(sel_rect, 0.0, box_stroke);
                    } else {
                        // 虛線框（crossing）
                        let dash_stroke = box_stroke;
                        let corners = [sel_rect.left_top(), sel_rect.right_top(), sel_rect.right_bottom(), sel_rect.left_bottom()];
                        for i in 0..4 {
                            painter.line_segment([corners[i], corners[(i+1)%4]], dash_stroke);
                        }
                    }
                }
            }
        }

        // ESC: 1) 取消繪圖中 → 2) 回到 Select 工具 → 3) 清除選取
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            #[cfg(feature = "drafting")]
            {
                use crate::editor::DraftDrawState;
                if !matches!(self.editor.draft_state, DraftDrawState::Idle) {
                    // 取消進行中的繪圖
                    self.editor.draft_state = DraftDrawState::Idle;
                    self.console_push("INFO", "取消".into());
                } else if self.editor.tool != Tool::DraftSelect {
                    // 回到選取工具
                    self.editor.tool = Tool::DraftSelect;
                } else if !self.editor.draft_selected.is_empty() {
                    // 清除選取
                    self.editor.draft_selected.clear();
                }
            }
        }

        // Delete 鍵刪除選取圖元
        if ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            let to_remove: Vec<_> = self.editor.draft_selected.drain(..).collect();
            for id in &to_remove {
                self.editor.draft_doc.remove(*id);
            }
            if !to_remove.is_empty() {
                self.console_push("ACTION", format!("刪除 {} 個圖元", to_remove.len()));
            }
        }

        // Ctrl+C 複製 / Ctrl+X 剪下 / Ctrl+V 貼上
        {
            let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
            if ctrl && ui.input(|i| i.key_pressed(egui::Key::C)) && !self.editor.draft_selected.is_empty() {
                self.editor.clipboard = Vec::new(); // 清空 3D clipboard
                // 用 draft_selected 的 entity 作為 clipboard（存在 draft_doc 外）
                // 簡化：記錄選取的 entity 到 console
                self.console_push("INFO", format!("已複製 {} 個圖元", self.editor.draft_selected.len()));
            }
            if ctrl && ui.input(|i| i.key_pressed(egui::Key::V)) {
                // 貼上：複製選取圖元到偏移位置
                let ids: Vec<_> = self.editor.draft_selected.clone();
                let offset = 10.0; // 10mm 偏移
                for &id in &ids {
                    if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                        let copied = kolibri_drafting::geometry::translate_entity(&obj.entity, offset, -offset);
                        self.editor.draft_doc.add(copied);
                    }
                }
                if !ids.is_empty() {
                    self.console_push("ACTION", format!("貼上 {} 個圖元", ids.len()));
                }
            }
            if ctrl && ui.input(|i| i.key_pressed(egui::Key::X)) && !self.editor.draft_selected.is_empty() {
                let ids: Vec<_> = self.editor.draft_selected.drain(..).collect();
                for id in &ids {
                    self.editor.draft_doc.remove(*id);
                }
                self.console_push("ACTION", format!("剪下 {} 個圖元", ids.len()));
            }
        }

        // 持續 repaint（十字游標需要跟隨滑鼠）
        if response.hovered() {
            ui.ctx().request_repaint();
        }

        // ── 左上角資訊 ──
        painter.text(
            egui::pos2(rect.left() + 8.0, rect.top() + 8.0),
            egui::Align2::LEFT_TOP,
            "[上視][二維線框][WCS]",
            egui::FontId::proportional(10.0),
            egui::Color32::from_rgb(140, 145, 155),
        );

        // ── 游標座標顯示（左下角）──
        if let Some(mm) = hover_mm {
            painter.text(
                egui::pos2(rect.left() + 8.0, rect.bottom() - 8.0),
                egui::Align2::LEFT_BOTTOM,
                format!("X:{:.1}  Y:{:.1}", mm[0], mm[1]),
                egui::FontId::monospace(10.0),
                egui::Color32::from_rgb(180, 185, 195),
            );
        }

        // ── 當前工具狀態提示 ──
        {
            use crate::editor::DraftDrawState;
            let state_text = match &self.editor.draft_state {
                DraftDrawState::Idle => "",
                DraftDrawState::LineFrom { .. } => "指定下一點 (右鍵結束)",
                DraftDrawState::ArcCenter { .. } => "指定半徑點",
                DraftDrawState::ArcRadius { .. } => "指定終點角度",
                DraftDrawState::CircleCenter { .. } => "指定半徑",
                DraftDrawState::RectFrom { .. } => "指定對角點",
                DraftDrawState::PolylinePoints { .. } => "指定下一點 (右鍵結束)",
                DraftDrawState::DimP1 { .. } => "指定第二點",
                DraftDrawState::TextPlace => "點擊放置文字",
                DraftDrawState::LeaderPoints { .. } => "指定下一點 (右鍵結束)",
            };
            if !state_text.is_empty() {
                painter.text(
                    egui::pos2(rect.center().x, rect.bottom() - 8.0),
                    egui::Align2::CENTER_BOTTOM,
                    state_text,
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgb(0, 200, 200),
                );
            }
        }
    }

    /// 繪製線性標註（共用）
    #[cfg(feature = "drafting")]
    fn draw_dim_linear(
        &self,
        painter: &egui::Painter,
        to_screen: &impl Fn(f64, f64) -> egui::Pos2,
        p1: &[f64; 2], p2: &[f64; 2],
        offset: f64,
        text_override: Option<&str>,
        scale: f32,
        color: egui::Color32,
    ) {
        let s1 = to_screen(p1[0], p1[1]);
        let s2 = to_screen(p2[0], p2[1]);
        let off_px = offset as f32 * scale;

        // 尺寸線方向（垂直於 p1-p2 連線）
        let dx = s2.x - s1.x;
        let dy = s2.y - s1.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1.0 { return; }
        let nx = -dy / len * off_px;
        let ny = dx / len * off_px;

        let d1 = egui::pos2(s1.x + nx, s1.y + ny);
        let d2 = egui::pos2(s2.x + nx, s2.y + ny);

        // 尺寸線
        let dim_stroke = egui::Stroke::new(0.8, color);
        painter.line_segment([d1, d2], dim_stroke);
        // 延伸線
        painter.line_segment([s1, d1], egui::Stroke::new(0.4, color));
        painter.line_segment([s2, d2], egui::Stroke::new(0.4, color));

        // 箭頭
        let arrow_len = 5.0;
        let dir = (d2 - d1).normalized();
        let perp = egui::vec2(-dir.y, dir.x);
        // 左箭頭
        painter.add(egui::Shape::convex_polygon(
            vec![d1, d1 + dir * arrow_len + perp * 2.0, d1 + dir * arrow_len - perp * 2.0],
            color, egui::Stroke::NONE));
        // 右箭頭
        painter.add(egui::Shape::convex_polygon(
            vec![d2, d2 - dir * arrow_len + perp * 2.0, d2 - dir * arrow_len - perp * 2.0],
            color, egui::Stroke::NONE));

        // 文字
        let dist = kolibri_drafting::DraftDocument::distance(p1, p2);
        let label = text_override.map(|s| s.to_string())
            .unwrap_or_else(|| format!("{:.0}", dist));
        let mid = egui::pos2((d1.x + d2.x) / 2.0, (d1.y + d2.y) / 2.0 - 6.0);
        painter.text(mid, egui::Align2::CENTER_BOTTOM, label,
            egui::FontId::proportional(10.0), color);
    }

    /// 繪製進行中的預覽
    #[cfg(feature = "drafting")]
    fn draw_draft_preview(
        &self,
        painter: &egui::Painter,
        to_screen: &impl Fn(f64, f64) -> egui::Pos2,
        scale: f32,
        response: &egui::Response,
    ) {
        let preview_color = egui::Color32::from_rgb(76, 139, 245);
        let preview_stroke = egui::Stroke::new(1.0, preview_color);

        // 取得目前滑鼠位置（mm，原點在畫布中央，Y 向上）
        let mouse_mm = response.hover_pos().map(|pos| {
            let rect = response.rect;
            let org = rect.center();
            [((pos.x - org.x) / scale) as f64, ((org.y - pos.y) / scale) as f64]
        });

        match &self.editor.draft_state {
            crate::editor::DraftDrawState::LineFrom { p1 } => {
                if let Some(mm) = mouse_mm {
                    painter.line_segment(
                        [to_screen(p1[0], p1[1]), to_screen(mm[0], mm[1])],
                        preview_stroke,
                    );
                }
            }
            crate::editor::DraftDrawState::CircleCenter { center } => {
                if let Some(mm) = mouse_mm {
                    let r = kolibri_drafting::DraftDocument::distance(center, &mm);
                    painter.circle_stroke(
                        to_screen(center[0], center[1]),
                        r as f32 * scale,
                        preview_stroke,
                    );
                }
            }
            crate::editor::DraftDrawState::RectFrom { p1 } => {
                if let Some(mm) = mouse_mm {
                    let r = egui::Rect::from_two_pos(
                        to_screen(p1[0], p1[1]),
                        to_screen(mm[0], mm[1]),
                    );
                    painter.rect_stroke(r, 0.0, preview_stroke);
                }
            }
            crate::editor::DraftDrawState::ArcCenter { center } => {
                if let Some(mm) = mouse_mm {
                    let r = kolibri_drafting::DraftDocument::distance(center, &mm);
                    painter.circle_stroke(
                        to_screen(center[0], center[1]),
                        r as f32 * scale,
                        egui::Stroke::new(0.5, preview_color.linear_multiply(0.4)),
                    );
                }
            }
            crate::editor::DraftDrawState::PolylinePoints { points } => {
                let screen_pts: Vec<egui::Pos2> = points.iter()
                    .map(|p| to_screen(p[0], p[1])).collect();
                for w in screen_pts.windows(2) {
                    painter.line_segment([w[0], w[1]], preview_stroke);
                }
                if let (Some(last), Some(mm)) = (points.last(), mouse_mm) {
                    painter.line_segment(
                        [to_screen(last[0], last[1]), to_screen(mm[0], mm[1])],
                        egui::Stroke::new(0.8, preview_color.linear_multiply(0.6)),
                    );
                }
            }
            crate::editor::DraftDrawState::DimP1 { p1 } => {
                if let Some(mm) = mouse_mm {
                    painter.line_segment(
                        [to_screen(p1[0], p1[1]), to_screen(mm[0], mm[1])],
                        egui::Stroke::new(0.8, egui::Color32::from_rgb(200, 50, 50)),
                    );
                }
            }
            crate::editor::DraftDrawState::LeaderPoints { points } => {
                let screen_pts: Vec<egui::Pos2> = points.iter()
                    .map(|p| to_screen(p[0], p[1])).collect();
                for w in screen_pts.windows(2) {
                    painter.line_segment([w[0], w[1]], egui::Stroke::new(0.8, egui::Color32::from_rgb(200, 50, 50)));
                }
                if let (Some(last), Some(mm)) = (points.last(), mouse_mm) {
                    painter.line_segment(
                        [to_screen(last[0], last[1]), to_screen(mm[0], mm[1])],
                        egui::Stroke::new(0.6, egui::Color32::from_rgba_unmultiplied(200, 50, 50, 128)),
                    );
                }
            }
            _ => {}
        }
    }

    /// v2 點擊：Select 工具做選取，其他工具做繪圖，空白處取消選取
    #[cfg(feature = "drafting")]
    fn handle_draft_click_v2(&mut self, mm_x: f64, mm_y: f64, shift: bool) {
        // Select 工具 — 點選/多選
        if self.editor.tool == Tool::DraftSelect {
            let mut best_id = None;
            let mut best_dist = 5.0_f64;
            for obj in &self.editor.draft_doc.objects {
                if !obj.visible { continue; }
                let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                if d < best_dist {
                    best_dist = d;
                    best_id = Some(obj.id);
                }
            }
            if let Some(id) = best_id {
                if shift {
                    // Shift+click: toggle
                    if let Some(pos) = self.editor.draft_selected.iter().position(|&x| x == id) {
                        self.editor.draft_selected.remove(pos);
                    } else {
                        self.editor.draft_selected.push(id);
                    }
                } else {
                    self.editor.draft_selected = vec![id];
                }
                self.console_push("INFO", format!("選取圖元 #{}", id));
            } else {
                // 點空白處 → 清除選取
                if !shift {
                    self.editor.draft_selected.clear();
                }
            }
            return;
        }
        // 其他工具 → 原有繪圖邏輯
        self.handle_draft_click(mm_x, mm_y);
    }

    /// 處理 2D 繪圖點擊（原始邏輯）
    #[cfg(feature = "drafting")]
    fn handle_draft_click(&mut self, mm_x: f64, mm_y: f64) {
        use crate::editor::DraftDrawState;
        let p = [mm_x, mm_y];

        match self.editor.tool {
            // ── 繪圖工具 ──
            Tool::DraftLine => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::LineFrom { p1: p };
                    }
                    DraftDrawState::LineFrom { p1 } => {
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Line {
                            start: p1, end: p,
                        });
                        // 連續繪製：起點變為上一條終點
                        self.editor.draft_state = DraftDrawState::LineFrom { p1: p };
                        self.console_push("ACTION", format!("直線: ({:.0},{:.0}) → ({:.0},{:.0})", p1[0], p1[1], p[0], p[1]));
                    }
                    _ => { self.editor.draft_state = DraftDrawState::LineFrom { p1: p }; }
                }
            }
            Tool::DraftArc => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::ArcCenter { center: p };
                    }
                    DraftDrawState::ArcCenter { center } => {
                        let r = kolibri_drafting::DraftDocument::distance(&center, &p);
                        self.editor.draft_state = DraftDrawState::ArcRadius { center, radius: r };
                    }
                    DraftDrawState::ArcRadius { center, radius } => {
                        let start_angle = 0.0;
                        let end_angle = kolibri_drafting::DraftDocument::angle(&center, &p);
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Arc {
                            center, radius, start_angle, end_angle,
                        });
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.console_push("ACTION", format!("圓弧: 中心({:.0},{:.0}) R={:.0}", center[0], center[1], radius));
                    }
                    _ => { self.editor.draft_state = DraftDrawState::ArcCenter { center: p }; }
                }
            }
            Tool::DraftCircle => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::CircleCenter { center: p };
                    }
                    DraftDrawState::CircleCenter { center } => {
                        let r = kolibri_drafting::DraftDocument::distance(&center, &p);
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Circle {
                            center, radius: r,
                        });
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.console_push("ACTION", format!("圓: 中心({:.0},{:.0}) R={:.0}", center[0], center[1], r));
                    }
                    _ => { self.editor.draft_state = DraftDrawState::CircleCenter { center: p }; }
                }
            }
            Tool::DraftRectangle => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::RectFrom { p1: p };
                    }
                    DraftDrawState::RectFrom { p1 } => {
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Rectangle {
                            p1, p2: p,
                        });
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.console_push("ACTION", format!("矩形: ({:.0},{:.0}) → ({:.0},{:.0})", p1[0], p1[1], p[0], p[1]));
                    }
                    _ => { self.editor.draft_state = DraftDrawState::RectFrom { p1: p }; }
                }
            }
            Tool::DraftPolyline => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::PolylinePoints { points: vec![p] };
                    }
                    DraftDrawState::PolylinePoints { mut points } => {
                        points.push(p);
                        self.editor.draft_state = DraftDrawState::PolylinePoints { points };
                    }
                    _ => {
                        self.editor.draft_state = DraftDrawState::PolylinePoints { points: vec![p] };
                    }
                }
            }
            Tool::DraftEllipse => {
                // 簡化：兩點定義（中心+半長軸端點）
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::CircleCenter { center: p };
                    }
                    DraftDrawState::CircleCenter { center } => {
                        let dist = kolibri_drafting::DraftDocument::distance(&center, &p);
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Ellipse {
                            center,
                            semi_major: dist,
                            semi_minor: dist * 0.6,
                            rotation: kolibri_drafting::DraftDocument::angle(&center, &p),
                        });
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.console_push("ACTION", format!("橢圓: 中心({:.0},{:.0})", center[0], center[1]));
                    }
                    _ => { self.editor.draft_state = DraftDrawState::CircleCenter { center: p }; }
                }
            }

            // ── 標註工具 ──
            Tool::DraftDimLinear | Tool::DraftDimAligned => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::DimP1 { p1: p };
                    }
                    DraftDrawState::DimP1 { p1 } => {
                        let entity = if self.editor.tool == Tool::DraftDimLinear {
                            kolibri_drafting::DraftEntity::DimLinear {
                                p1, p2: p, offset: 8.0, text_override: None,
                            }
                        } else {
                            kolibri_drafting::DraftEntity::DimAligned {
                                p1, p2: p, offset: 8.0, text_override: None,
                            }
                        };
                        self.editor.draft_doc.add(entity);
                        self.editor.draft_state = DraftDrawState::Idle;
                        let dist = kolibri_drafting::DraftDocument::distance(&p1, &p);
                        self.console_push("ACTION", format!("標註: {:.0}mm", dist));
                    }
                    _ => { self.editor.draft_state = DraftDrawState::DimP1 { p1: p }; }
                }
            }
            Tool::DraftDimAngle => {
                // 簡化：三點（中心 + 兩端點）
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::ArcCenter { center: p };
                    }
                    DraftDrawState::ArcCenter { center } => {
                        self.editor.draft_state = DraftDrawState::DimP1 { p1: p };
                        // 儲存 center 到暫存...簡化：用 ArcRadius 暫存
                        self.editor.draft_state = DraftDrawState::ArcRadius { center, radius: 0.0 };
                    }
                    DraftDrawState::ArcRadius { center, .. } => {
                        let r = kolibri_drafting::DraftDocument::distance(&center, &p) * 0.6;
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::DimAngle {
                            center,
                            p1: center, // 簡化
                            p2: p,
                            radius: r,
                        });
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.console_push("ACTION", "角度標註".into());
                    }
                    _ => { self.editor.draft_state = DraftDrawState::ArcCenter { center: p }; }
                }
            }
            Tool::DraftDimRadius => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::CircleCenter { center: p };
                    }
                    DraftDrawState::CircleCenter { center } => {
                        let r = kolibri_drafting::DraftDocument::distance(&center, &p);
                        let angle = kolibri_drafting::DraftDocument::angle(&center, &p);
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::DimRadius {
                            center, radius: r, angle,
                        });
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.console_push("ACTION", format!("半徑標註: R{:.0}", r));
                    }
                    _ => { self.editor.draft_state = DraftDrawState::CircleCenter { center: p }; }
                }
            }
            Tool::DraftDimDiameter => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::CircleCenter { center: p };
                    }
                    DraftDrawState::CircleCenter { center } => {
                        let r = kolibri_drafting::DraftDocument::distance(&center, &p);
                        let angle = kolibri_drafting::DraftDocument::angle(&center, &p);
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::DimDiameter {
                            center, radius: r, angle,
                        });
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.console_push("ACTION", format!("直徑標註: ⌀{:.0}", r * 2.0));
                    }
                    _ => { self.editor.draft_state = DraftDrawState::CircleCenter { center: p }; }
                }
            }
            Tool::DraftText => {
                self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Text {
                    position: p,
                    content: "文字".into(),
                    height: 3.5,
                    rotation: 0.0,
                });
                self.console_push("ACTION", format!("文字: ({:.0},{:.0})", p[0], p[1]));
            }
            Tool::DraftLeader => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::LeaderPoints { points: vec![p] };
                    }
                    DraftDrawState::LeaderPoints { mut points } => {
                        points.push(p);
                        if points.len() >= 3 {
                            self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Leader {
                                points: points.clone(),
                                text: "標註".into(),
                            });
                            self.editor.draft_state = DraftDrawState::Idle;
                            self.console_push("ACTION", "引線標註".into());
                        } else {
                            self.editor.draft_state = DraftDrawState::LeaderPoints { points };
                        }
                    }
                    _ => {
                        self.editor.draft_state = DraftDrawState::LeaderPoints { points: vec![p] };
                    }
                }
            }
            Tool::DraftHatch => {
                // 簡化：點擊放置填充（使用預設邊界）
                let size = 20.0;
                self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Hatch {
                    boundary: vec![
                        [mm_x - size, mm_y - size],
                        [mm_x + size, mm_y - size],
                        [mm_x + size, mm_y + size],
                        [mm_x - size, mm_y + size],
                    ],
                    pattern: kolibri_drafting::HatchPattern::Lines,
                    scale: 1.0,
                    angle: 45.0,
                });
                self.console_push("ACTION", format!("填充: ({:.0},{:.0})", mm_x, mm_y));
            }

            // ── 選取 ──
            Tool::DraftSelect => {
                // 簡易碰撞偵測：找最近圖元
                let mut best_id = None;
                let mut best_dist = 5.0_f64; // 5mm 容差
                for obj in &self.editor.draft_doc.objects {
                    let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                    if d < best_dist {
                        best_dist = d;
                        best_id = Some(obj.id);
                    }
                }
                self.editor.draft_selected.clear();
                if let Some(id) = best_id {
                    self.editor.draft_selected.push(id);
                    self.console_push("INFO", format!("選取圖元 #{}", id));
                }
            }

            // ── 複製 COPY（選取物件 → 點擊放置）──
            Tool::DraftCopy => {
                if !self.editor.draft_selected.is_empty() {
                    match self.editor.draft_state.clone() {
                        DraftDrawState::Idle => {
                            self.editor.draft_state = DraftDrawState::LineFrom { p1: p };
                            self.console_push("INFO", "指定基點".into());
                        }
                        DraftDrawState::LineFrom { p1 } => {
                            let dx = p[0] - p1[0];
                            let dy = p[1] - p1[1];
                            let ids: Vec<_> = self.editor.draft_selected.clone();
                            for &id in &ids {
                                if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                                    let copied = kolibri_drafting::geometry::translate_entity(&obj.entity, dx, dy);
                                    self.editor.draft_doc.add(copied);
                                }
                            }
                            self.console_push("ACTION", format!("複製 {} 個圖元", ids.len()));
                            // 保持在 Copy 模式可連續貼
                            self.editor.draft_state = DraftDrawState::LineFrom { p1: p };
                        }
                        _ => { self.editor.draft_state = DraftDrawState::LineFrom { p1: p }; }
                    }
                }
            }

            // ── 分解 EXPLODE（點擊分解複合物件）──
            Tool::DraftExplode => {
                // 分解矩形→4線段，多段線→多線段
                let sel = self.editor.draft_selected.clone();
                let mut count = 0;
                for id in sel {
                    if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id).cloned() {
                        match &obj.entity {
                            kolibri_drafting::DraftEntity::Rectangle { p1, p2 } => {
                                let corners = [*p1, [p2[0], p1[1]], *p2, [p1[0], p2[1]]];
                                for i in 0..4 {
                                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Line {
                                        start: corners[i], end: corners[(i + 1) % 4],
                                    });
                                }
                                self.editor.draft_doc.remove(id);
                                count += 1;
                            }
                            kolibri_drafting::DraftEntity::Polyline { points, closed } => {
                                for w in points.windows(2) {
                                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Line {
                                        start: w[0], end: w[1],
                                    });
                                }
                                if *closed && points.len() >= 2 {
                                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Line {
                                        start: *points.last().unwrap(), end: points[0],
                                    });
                                }
                                self.editor.draft_doc.remove(id);
                                count += 1;
                            }
                            kolibri_drafting::DraftEntity::Polygon { center, radius, sides, inscribed } => {
                                let pts = kolibri_drafting::geometry::polygon_points(center, *radius, *sides, *inscribed);
                                for i in 0..pts.len() {
                                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Line {
                                        start: pts[i], end: pts[(i + 1) % pts.len()],
                                    });
                                }
                                self.editor.draft_doc.remove(id);
                                count += 1;
                            }
                            _ => {}
                        }
                    }
                }
                if count > 0 {
                    self.editor.draft_selected.clear();
                    self.console_push("ACTION", format!("分解 {} 個圖元", count));
                }
            }

            // ── 多邊形 POLYGON ──
            Tool::DraftPolygon => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::CircleCenter { center: p };
                        self.console_push("INFO", "指定中心點".into());
                    }
                    DraftDrawState::CircleCenter { center } => {
                        let r = kolibri_drafting::DraftDocument::distance(&center, &p);
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Polygon {
                            center, radius: r, sides: 6, inscribed: true,
                        });
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.console_push("ACTION", format!("六邊形: R={:.0}", r));
                    }
                    _ => { self.editor.draft_state = DraftDrawState::CircleCenter { center: p }; }
                }
            }

            // ── 雲形線 SPLINE ──
            Tool::DraftSpline => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::PolylinePoints { points: vec![p] };
                    }
                    DraftDrawState::PolylinePoints { mut points } => {
                        points.push(p);
                        self.editor.draft_state = DraftDrawState::PolylinePoints { points };
                    }
                    _ => { self.editor.draft_state = DraftDrawState::PolylinePoints { points: vec![p] }; }
                }
            }

            // ── 點 POINT ──
            Tool::DraftPoint => {
                self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Point { position: p });
                self.console_push("ACTION", format!("點: ({:.0},{:.0})", p[0], p[1]));
            }

            // ── 建構線 XLINE ──
            Tool::DraftXline => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::LineFrom { p1: p };
                        self.console_push("INFO", "指定通過點".into());
                    }
                    DraftDrawState::LineFrom { p1 } => {
                        let dir = [p[0] - p1[0], p[1] - p1[1]];
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Xline {
                            base: p1, direction: dir,
                        });
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.console_push("ACTION", "建構線".into());
                    }
                    _ => { self.editor.draft_state = DraftDrawState::LineFrom { p1: p }; }
                }
            }

            // ── 圓角 FILLET / 倒角 CHAMFER / 拉伸 STRETCH / 延伸 EXTEND ──
            // 這些需要選取兩個圖元，暫時用提示
            Tool::DraftFillet | Tool::DraftChamfer | Tool::DraftStretch | Tool::DraftExtend => {
                self.console_push("INFO", "請先選取兩個相鄰圖元（開發中）".into());
            }

            // ── 連續標註 / 基線標註 ──
            Tool::DraftDimContinue | Tool::DraftDimBaseline => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::DimP1 { p1: p };
                    }
                    DraftDrawState::DimP1 { p1 } => {
                        let entity = kolibri_drafting::DraftEntity::DimLinear {
                            p1, p2: p, offset: 8.0, text_override: None,
                        };
                        self.editor.draft_doc.add(entity);
                        // 連續模式：p2 變成下一個 p1
                        self.editor.draft_state = DraftDrawState::DimP1 { p1: p };
                        let dist = kolibri_drafting::DraftDocument::distance(&p1, &p);
                        self.console_push("ACTION", format!("連續標註: {:.0}mm", dist));
                    }
                    _ => { self.editor.draft_state = DraftDrawState::DimP1 { p1: p }; }
                }
            }

            // ── 圖塊 BLOCK/INSERT ──
            Tool::DraftBlock => {
                if !self.editor.draft_selected.is_empty() {
                    self.console_push("INFO", format!("已選取 {} 個圖元為圖塊（開發中）", self.editor.draft_selected.len()));
                } else {
                    self.console_push("INFO", "請先選取要建立圖塊的圖元".into());
                }
            }
            Tool::DraftInsert => {
                self.editor.draft_doc.add(kolibri_drafting::DraftEntity::BlockRef {
                    name: "Block1".into(),
                    insert_point: p,
                    scale: [1.0, 1.0],
                    rotation: 0.0,
                });
                self.console_push("ACTION", format!("插入圖塊: ({:.0},{:.0})", p[0], p[1]));
            }

            _ => {}
        }
    }

    /// 完成多段線/引線等多步驟工具（右鍵觸發）
    #[cfg(feature = "drafting")]
    fn finish_draft_tool(&mut self) {
        use crate::editor::DraftDrawState;
        match self.editor.draft_state.clone() {
            DraftDrawState::PolylinePoints { points } => {
                if points.len() >= 2 {
                    if self.editor.tool == Tool::DraftSpline {
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Spline {
                            points,
                            closed: false,
                        });
                        self.console_push("ACTION", "雲形線完成".into());
                    } else {
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Polyline {
                            points,
                            closed: false,
                        });
                        self.console_push("ACTION", "多段線完成".into());
                    }
                }
                self.editor.draft_state = DraftDrawState::Idle;
            }
            DraftDrawState::LeaderPoints { points } => {
                if points.len() >= 2 {
                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Leader {
                        points,
                        text: "標註".into(),
                    });
                    self.console_push("ACTION", "引線完成".into());
                }
                self.editor.draft_state = DraftDrawState::Idle;
            }
            DraftDrawState::LineFrom { .. } => {
                self.editor.draft_state = DraftDrawState::Idle;
            }
            _ => {
                self.editor.draft_state = DraftDrawState::Idle;
            }
        }
    }

    /// 計算滑鼠到圖元的距離（mm）
    #[cfg(feature = "drafting")]
    fn draft_entity_distance(&self, entity: &kolibri_drafting::DraftEntity, mx: f64, my: f64) -> f64 {
        match entity {
            kolibri_drafting::DraftEntity::Line { start, end } => {
                let nearest = kolibri_drafting::geometry::point_to_line_nearest(
                    &[mx, my], start, end);
                kolibri_drafting::DraftDocument::distance(&[mx, my], &nearest)
            }
            kolibri_drafting::DraftEntity::Circle { center, radius } => {
                let d = kolibri_drafting::DraftDocument::distance(center, &[mx, my]);
                (d - radius).abs()
            }
            kolibri_drafting::DraftEntity::Rectangle { p1, p2 } => {
                // 到四邊最近距離
                let edges = [
                    (*p1, [p2[0], p1[1]]),
                    ([p2[0], p1[1]], *p2),
                    (*p2, [p1[0], p2[1]]),
                    ([p1[0], p2[1]], *p1),
                ];
                edges.iter().map(|(a, b)| {
                    let n = kolibri_drafting::geometry::point_to_line_nearest(&[mx, my], a, b);
                    kolibri_drafting::DraftDocument::distance(&[mx, my], &n)
                }).fold(f64::MAX, f64::min)
            }
            kolibri_drafting::DraftEntity::Text { position, .. } => {
                kolibri_drafting::DraftDocument::distance(position, &[mx, my])
            }
            _ => f64::MAX,
        }
    }

    /// 繪製圖元高亮（選取/hover 共用）
    #[cfg(feature = "drafting")]
    fn draw_entity_highlight(
        &self,
        painter: &egui::Painter,
        to_screen: &impl Fn(f64, f64) -> egui::Pos2,
        scale: f32,
        entity: &kolibri_drafting::DraftEntity,
        stroke: egui::Stroke,
    ) {
        match entity {
            kolibri_drafting::DraftEntity::Line { start, end } => {
                painter.line_segment([to_screen(start[0], start[1]), to_screen(end[0], end[1])], stroke);
            }
            kolibri_drafting::DraftEntity::Circle { center, radius } => {
                painter.circle_stroke(to_screen(center[0], center[1]), *radius as f32 * scale, stroke);
            }
            kolibri_drafting::DraftEntity::Rectangle { p1, p2 } => {
                let r = egui::Rect::from_two_pos(to_screen(p1[0], p1[1]), to_screen(p2[0], p2[1]));
                painter.rect_stroke(r, 0.0, stroke);
            }
            kolibri_drafting::DraftEntity::Arc { center, radius, start_angle, end_angle } => {
                let c = to_screen(center[0], center[1]);
                let r = *radius as f32 * scale;
                let n = 32;
                let mut pts = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let t = *start_angle + (*end_angle - *start_angle) * i as f64 / n as f64;
                    pts.push(egui::pos2(c.x + r * t.cos() as f32, c.y - r * t.sin() as f32));
                }
                for w in pts.windows(2) { painter.line_segment([w[0], w[1]], stroke); }
            }
            kolibri_drafting::DraftEntity::Polyline { points, closed } => {
                let spts: Vec<egui::Pos2> = points.iter().map(|p| to_screen(p[0], p[1])).collect();
                for w in spts.windows(2) { painter.line_segment([w[0], w[1]], stroke); }
                if *closed && spts.len() >= 2 {
                    painter.line_segment([*spts.last().unwrap(), spts[0]], stroke);
                }
            }
            kolibri_drafting::DraftEntity::Text { position, content, height, .. } => {
                let p = to_screen(position[0], position[1]);
                let fs = (*height as f32 * scale).max(8.0);
                painter.text(p, egui::Align2::LEFT_TOP, content,
                    egui::FontId::proportional(fs), stroke.color);
            }
            _ => {}
        }
    }

    /// 取得圖元的 grip 控制點（端點）
    #[cfg(feature = "drafting")]
    fn entity_grip_points(&self, entity: &kolibri_drafting::DraftEntity) -> Vec<[f64; 2]> {
        match entity {
            kolibri_drafting::DraftEntity::Line { start, end } => vec![*start, *end],
            kolibri_drafting::DraftEntity::Circle { center, radius } => {
                vec![*center,
                    [center[0] + radius, center[1]],
                    [center[0], center[1] + radius],
                    [center[0] - radius, center[1]],
                    [center[0], center[1] - radius]]
            }
            kolibri_drafting::DraftEntity::Rectangle { p1, p2 } => {
                vec![*p1, [p2[0], p1[1]], *p2, [p1[0], p2[1]]]
            }
            kolibri_drafting::DraftEntity::Arc { center, radius, start_angle, end_angle } => {
                vec![*center,
                    [center[0] + radius * start_angle.cos(), center[1] + radius * start_angle.sin()],
                    [center[0] + radius * end_angle.cos(), center[1] + radius * end_angle.sin()]]
            }
            kolibri_drafting::DraftEntity::Polyline { points, .. } => points.clone(),
            kolibri_drafting::DraftEntity::Text { position, .. } => vec![*position],
            _ => vec![],
        }
    }
}

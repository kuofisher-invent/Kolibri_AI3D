//! 2D 出圖畫布 — 繪製 DraftDocument 的所有 2D 實體
//! 用 egui Painter 直接繪製在 CentralPanel 上

use eframe::egui;
use crate::app::{KolibriApp, Tool};

// 2D 畫布 zoom/offset 狀態存在 EditorState 的 draft_zoom / draft_offset 欄位

impl KolibriApp {
    /// 繪製 2D 出圖畫布（layout_mode 時取代 3D viewport）
    #[cfg(feature = "drafting")]
    pub(crate) fn draw_draft_canvas(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        // response.hovered()/hover_pos() 不可靠，改用 pointer 位置直接判斷
        let canvas_hover_pos: Option<egui::Pos2> = ui.input(|i| {
            i.pointer.latest_pos().filter(|p| rect.contains(*p))
        });
        let painter = ui.painter_at(rect);

        // 背景（ZWCAD 深色 — 無白紙，直接模型空間）
        let bg_color = egui::Color32::from_rgb(33, 40, 48);
        painter.rect_filled(rect, 0.0, bg_color);

        // 座標系統：原點 = 畫布中央 + offset，scale = zoom（像素/mm）
        let scale = self.editor.draft_zoom;
        let origin = rect.center() + self.editor.draft_offset;

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
            // 動態調整格線間距：縮小到太密時自動加大間距
            let grid_px = grid_mm as f32 * scale;
            let effective_grid = if grid_px < 4.0 {
                grid_mm * (4.0 / grid_px as f64).ceil() // 自動倍增到至少 4px 間距
            } else {
                grid_mm
            };
            let x_start = (mm_left / effective_grid).floor() as i64;
            let x_end = (mm_right / effective_grid).ceil() as i64;
            let y_start = (mm_bottom / effective_grid).floor() as i64;
            let y_end = (mm_top / effective_grid).ceil() as i64;
            // 限制最大格點數（避免縮太小時畫幾萬個點）
            let max_dots = 10000_i64;
            let dot_count = (x_end - x_start + 1) * (y_end - y_start + 1);
            if dot_count <= max_dots {
                for ix in x_start..=x_end {
                    for iy in y_start..=y_end {
                        let sp = to_screen(ix as f64 * effective_grid, iy as f64 * effective_grid);
                        if rect.contains(sp) {
                            painter.circle_filled(sp, dot_r, dot_color);
                        }
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

        // ── 2D Snap 偵測（端點/中點/圓心/交點/最近點/垂直）──
        let snap_threshold_mm = 5.0_f64;
        let mut snap_point: Option<([f64; 2], &str, egui::Color32)> = None;
        if let Some(hover_pos) = canvas_hover_pos {
            let mx = ((hover_pos.x - origin.x) / scale) as f64;
            let my = ((origin.y - hover_pos.y) / scale) as f64;

            let mut best_dist = snap_threshold_mm;
            // 收集所有線段端點（用於交點偵測）
            let mut line_segments: Vec<([f64; 2], [f64; 2])> = Vec::new();
            // snap 只檢查附近區域的圖元（效能優化）
            let snap_range = snap_threshold_mm * 5.0;
            let snap_left = mx - snap_range;
            let snap_right = mx + snap_range;
            let snap_bottom = my - snap_range;
            let snap_top = my + snap_range;
            for obj in &self.editor.draft_doc.objects {
                if !obj.visible { continue; }
                if !Self::entity_in_view(&obj.entity, snap_left, snap_right, snap_bottom, snap_top) { continue; }
                // 端點 Snap
                let grips = self.entity_grip_points(&obj.entity);
                for gp in &grips {
                    let d = ((gp[0] - mx).powi(2) + (gp[1] - my).powi(2)).sqrt();
                    if d < best_dist {
                        best_dist = d;
                        snap_point = Some((*gp, "端點", egui::Color32::from_rgb(60, 220, 60)));
                    }
                }
                // 中點 Snap（線段）
                if let kolibri_drafting::DraftEntity::Line { start, end } = &obj.entity {
                    let mid = [(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0];
                    let d = ((mid[0] - mx).powi(2) + (mid[1] - my).powi(2)).sqrt();
                    if d < best_dist {
                        best_dist = d;
                        snap_point = Some((mid, "中點", egui::Color32::from_rgb(60, 220, 220)));
                    }
                    line_segments.push((*start, *end));
                }
                // 圓心 Snap
                match &obj.entity {
                    kolibri_drafting::DraftEntity::Circle { center, .. }
                    | kolibri_drafting::DraftEntity::Arc { center, .. } => {
                        let d = ((center[0] - mx).powi(2) + (center[1] - my).powi(2)).sqrt();
                        if d < best_dist {
                            best_dist = d;
                            snap_point = Some((*center, "圓心", egui::Color32::from_rgb(220, 60, 60)));
                        }
                    }
                    _ => {}
                }
                // 最近點 Snap（線段上）
                if let kolibri_drafting::DraftEntity::Line { start, end } = &obj.entity {
                    let nearest = kolibri_drafting::geometry::point_to_line_nearest(&[mx, my], start, end);
                    let d = ((nearest[0] - mx).powi(2) + (nearest[1] - my).powi(2)).sqrt();
                    if d < best_dist && d < 3.0 {
                        best_dist = d;
                        snap_point = Some((nearest, "最近", egui::Color32::from_rgb(220, 180, 60)));
                    }
                }
                // 象限點 Snap（圓的 0°/90°/180°/270°）
                if let kolibri_drafting::DraftEntity::Circle { center, radius } = &obj.entity {
                    for (dx, dy) in [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)] {
                        let qp = [center[0] + radius * dx, center[1] + radius * dy];
                        let d = ((qp[0] - mx).powi(2) + (qp[1] - my).powi(2)).sqrt();
                        if d < best_dist {
                            best_dist = d;
                            snap_point = Some((qp, "象限", egui::Color32::from_rgb(220, 60, 220)));
                        }
                    }
                }
            }
            // 交點 Snap（線段×線段）
            for i in 0..line_segments.len() {
                for j in (i+1)..line_segments.len() {
                    if let Some(ip) = kolibri_drafting::geometry::line_intersection(
                        &line_segments[i].0, &line_segments[i].1,
                        &line_segments[j].0, &line_segments[j].1,
                    ) {
                        let d = ((ip[0] - mx).powi(2) + (ip[1] - my).powi(2)).sqrt();
                        if d < best_dist {
                            best_dist = d;
                            snap_point = Some((ip, "交點", egui::Color32::from_rgb(220, 220, 60)));
                        }
                    }
                }
            }
        }

        // 繪製 snap 指示器（依 snap 類型使用不同符號，AutoCAD 風格）
        if let Some((sp, label, color)) = &snap_point {
            let screen_sp = to_screen(sp[0], sp[1]);
            let sz = 5.0;
            let snap_stroke = egui::Stroke::new(2.0, *color);
            match *label {
                "端點" => {
                    // 實心方形
                    painter.rect_filled(
                        egui::Rect::from_center_size(screen_sp, egui::vec2(sz * 2.0, sz * 2.0)),
                        0.0, *color);
                }
                "中點" => {
                    // 實心三角形（尖朝上）
                    let top = egui::pos2(screen_sp.x, screen_sp.y - sz);
                    let bl = egui::pos2(screen_sp.x - sz, screen_sp.y + sz);
                    let br = egui::pos2(screen_sp.x + sz, screen_sp.y + sz);
                    painter.add(egui::Shape::convex_polygon(vec![top, bl, br], *color, egui::Stroke::NONE));
                }
                "圓心" => {
                    // 空心圓
                    painter.circle_stroke(screen_sp, sz, snap_stroke);
                }
                "交點" => {
                    // X 叉叉
                    painter.line_segment(
                        [egui::pos2(screen_sp.x - sz, screen_sp.y - sz),
                         egui::pos2(screen_sp.x + sz, screen_sp.y + sz)], snap_stroke);
                    painter.line_segment(
                        [egui::pos2(screen_sp.x + sz, screen_sp.y - sz),
                         egui::pos2(screen_sp.x - sz, screen_sp.y + sz)], snap_stroke);
                }
                "最近" => {
                    // 菱形（旋轉 45° 的正方形）
                    let pts = vec![
                        egui::pos2(screen_sp.x, screen_sp.y - sz),
                        egui::pos2(screen_sp.x + sz, screen_sp.y),
                        egui::pos2(screen_sp.x, screen_sp.y + sz),
                        egui::pos2(screen_sp.x - sz, screen_sp.y),
                    ];
                    painter.add(egui::Shape::convex_polygon(pts, egui::Color32::TRANSPARENT, snap_stroke));
                }
                "象限" => {
                    // 小菱形（實心）
                    let s = sz * 0.7;
                    let pts = vec![
                        egui::pos2(screen_sp.x, screen_sp.y - s),
                        egui::pos2(screen_sp.x + s, screen_sp.y),
                        egui::pos2(screen_sp.x, screen_sp.y + s),
                        egui::pos2(screen_sp.x - s, screen_sp.y),
                    ];
                    painter.add(egui::Shape::convex_polygon(pts, *color, egui::Stroke::NONE));
                }
                _ => {
                    // 預設：空心方形
                    painter.rect_stroke(
                        egui::Rect::from_center_size(screen_sp, egui::vec2(sz * 2.0, sz * 2.0)),
                        0.0, snap_stroke);
                }
            }
            // 標籤
            painter.text(
                egui::pos2(screen_sp.x + sz + 4.0, screen_sp.y - sz - 2.0),
                egui::Align2::LEFT_BOTTOM, *label,
                egui::FontId::proportional(9.0), *color);
        }

        // 十字游標（ZWCAD 風格：跟隨滑鼠的全畫面十字線）
        if let Some(hover_pos) = canvas_hover_pos {
            // 如果有 snap 點，十字游標吸附到 snap 位置
            let cursor_pos = if let Some((sp, _, _)) = &snap_point {
                to_screen(sp[0], sp[1])
            } else {
                hover_pos
            };

            let cross_color = egui::Color32::from_rgba_unmultiplied(180, 190, 200, 120);
            // 水平線
            painter.line_segment(
                [egui::pos2(rect.left(), cursor_pos.y), egui::pos2(rect.right(), cursor_pos.y)],
                egui::Stroke::new(0.5, cross_color),
            );
            // 垂直線
            painter.line_segment(
                [egui::pos2(cursor_pos.x, rect.top()), egui::pos2(cursor_pos.x, rect.bottom())],
                egui::Stroke::new(0.5, cross_color),
            );
            // 中心小十字（粗）
            let arm = 10.0;
            painter.line_segment(
                [egui::pos2(cursor_pos.x - arm, cursor_pos.y), egui::pos2(cursor_pos.x + arm, cursor_pos.y)],
                egui::Stroke::new(1.2, egui::Color32::from_rgb(220, 220, 230)),
            );
            painter.line_segment(
                [egui::pos2(cursor_pos.x, cursor_pos.y - arm), egui::pos2(cursor_pos.x, cursor_pos.y + arm)],
                egui::Stroke::new(1.2, egui::Color32::from_rgb(220, 220, 230)),
            );
        }

        // 繪製所有 draft 圖元（深色背景 → 亮色線條）
        let dim_color = egui::Color32::from_rgb(0, 220, 220); // cyan（ZWCAD 標註色）
        let text_color = egui::Color32::from_rgb(220, 220, 50); // 黃色文字

        // ── 可見區域（mm 座標，用於 frustum culling）──
        let vis_left = ((rect.left() - origin.x) / scale) as f64 - 50.0;
        let vis_right = ((rect.right() - origin.x) / scale) as f64 + 50.0;
        let vis_bottom = ((origin.y - rect.bottom()) / scale) as f64 - 50.0;
        let vis_top = ((origin.y - rect.top()) / scale) as f64 + 50.0;

        // Debug: 顯示可見區域和圖元數量（右上角）
        let obj_count = self.editor.draft_doc.objects.len();
        if obj_count > 0 {
            painter.text(
                egui::pos2(rect.right() - 10.0, rect.top() + 10.0),
                egui::Align2::RIGHT_TOP,
                format!("{} entities | zoom:{:.3} | vis:[{:.0},{:.0}]×[{:.0},{:.0}]",
                    obj_count, scale, vis_left, vis_right, vis_bottom, vis_top),
                egui::FontId::monospace(10.0),
                egui::Color32::from_rgb(255, 200, 60),
            );
        }

        let mut rendered_count = 0_usize;
        for obj in &self.editor.draft_doc.objects {
            if !obj.visible { continue; }
            // ── Frustum culling: 跳過完全不在螢幕內的圖元 ──
            let in_view = Self::entity_in_view(&obj.entity, vis_left, vis_right, vis_bottom, vis_top);
            if !in_view { continue; }
            rendered_count += 1;
            // 深色背景：黑色線改白色，其他保留
            let color = if obj.color == [0, 0, 0] {
                egui::Color32::from_rgb(230, 230, 230) // 白色線條
            } else {
                egui::Color32::from_rgb(obj.color[0], obj.color[1], obj.color[2])
            };
            // 統一細線（1px），ZWCAD 預設模式不顯示線寬差異
            let lw = 1.0_f32;
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
                    // ZWCAD 風格：圓心→圓周 + 箭頭 + 文字帶底線
                    let c = to_screen(center[0], center[1]);
                    let r = *radius as f32 * scale;
                    let ep = egui::pos2(
                        c.x + r * angle.cos() as f32,
                        c.y - r * angle.sin() as f32,
                    );
                    let dim_st = egui::Stroke::new(0.8, dim_color);
                    // 尺寸線
                    painter.line_segment([c, ep], dim_st);
                    // 箭頭（在圓周端）
                    let dir = (ep - c).normalized();
                    let perp = egui::vec2(-dir.y, dir.x);
                    painter.add(egui::Shape::convex_polygon(
                        vec![ep, ep - dir * 7.0 + perp * 2.5, ep - dir * 7.0 - perp * 2.5],
                        dim_color, egui::Stroke::NONE));
                    // 文字 + 底線（從圓周端向外延伸）
                    let text_start = ep + dir * 4.0;
                    let text_label = format!("R{:.0}", radius);
                    let text_width = text_label.len() as f32 * 6.5;
                    let text_end = egui::pos2(text_start.x + text_width, text_start.y);
                    painter.line_segment([ep, text_end], dim_st);
                    painter.text(
                        egui::pos2(text_start.x + 2.0, text_start.y - 3.0),
                        egui::Align2::LEFT_BOTTOM, text_label,
                        egui::FontId::proportional(11.0), dim_color);
                }
                kolibri_drafting::DraftEntity::DimDiameter { center, radius, angle } => {
                    // ZWCAD 風格：貫穿圓心 + 兩端箭頭 + 文字帶底線
                    let c = to_screen(center[0], center[1]);
                    let r = *radius as f32 * scale;
                    let ep1 = egui::pos2(c.x + r * angle.cos() as f32, c.y - r * angle.sin() as f32);
                    let ep2 = egui::pos2(c.x - r * angle.cos() as f32, c.y + r * angle.sin() as f32);
                    let dim_st = egui::Stroke::new(0.8, dim_color);
                    painter.line_segment([ep1, ep2], dim_st);
                    // 兩端箭頭
                    let dir = (ep1 - ep2).normalized();
                    let perp = egui::vec2(-dir.y, dir.x);
                    painter.add(egui::Shape::convex_polygon(
                        vec![ep1, ep1 - dir * 7.0 + perp * 2.5, ep1 - dir * 7.0 - perp * 2.5],
                        dim_color, egui::Stroke::NONE));
                    painter.add(egui::Shape::convex_polygon(
                        vec![ep2, ep2 + dir * 7.0 + perp * 2.5, ep2 + dir * 7.0 - perp * 2.5],
                        dim_color, egui::Stroke::NONE));
                    // 文字 + 底線（從 ep1 向外延伸）
                    let text_start = ep1 + dir * 4.0;
                    let text_label = format!("⌀{:.0}", radius * 2.0);
                    let text_width = text_label.len() as f32 * 6.5;
                    let text_end = egui::pos2(text_start.x + text_width, text_start.y);
                    painter.line_segment([ep1, text_end], dim_st);
                    painter.text(
                        egui::pos2(text_start.x + 2.0, text_start.y - 3.0),
                        egui::Align2::LEFT_BOTTOM, text_label,
                        egui::FontId::proportional(11.0), dim_color);
                }
                kolibri_drafting::DraftEntity::Leader { points, text } => {
                    // ZWCAD 風格：箭頭→折線→水平底線 + 文字在底線上方
                    let screen_pts: Vec<egui::Pos2> = points.iter()
                        .map(|p| to_screen(p[0], p[1])).collect();
                    let dim_st = egui::Stroke::new(0.8, dim_color);
                    for w in screen_pts.windows(2) {
                        painter.line_segment([w[0], w[1]], dim_st);
                    }
                    // 箭頭（在第一個點）
                    if screen_pts.len() >= 2 {
                        let tip = screen_pts[0];
                        let from = screen_pts[1];
                        let dir = (tip - from).normalized();
                        let perp = egui::vec2(-dir.y, dir.x);
                        painter.add(egui::Shape::convex_polygon(
                            vec![tip, tip - dir * 7.0 + perp * 2.5, tip - dir * 7.0 - perp * 2.5],
                            dim_color, egui::Stroke::NONE));
                    }
                    // 水平底線 + 文字（從最後一點向右延伸）
                    if let Some(last) = screen_pts.last() {
                        let text_width = text.len() as f32 * 7.0 + 10.0;
                        let line_end = egui::pos2(last.x + text_width, last.y);
                        painter.line_segment([*last, line_end], dim_st);
                        painter.text(
                            egui::pos2(last.x + 4.0, last.y - 3.0),
                            egui::Align2::LEFT_BOTTOM, text,
                            egui::FontId::proportional(11.0), dim_color);
                    }
                }
                kolibri_drafting::DraftEntity::Hatch { boundary, pattern, scale: h_scale, angle } => {
                    let screen_pts: Vec<egui::Pos2> = boundary.iter()
                        .map(|p| to_screen(p[0], p[1])).collect();
                    if screen_pts.len() >= 3 {
                        // 邊界填充（半透明底色）
                        let fill = egui::Color32::from_rgba_unmultiplied(
                            color.r(), color.g(), color.b(), 20);
                        painter.add(egui::Shape::convex_polygon(
                            screen_pts.clone(), fill, st,
                        ));
                        // 計算 bounding box（mm）
                        let min_x = boundary.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
                        let max_x = boundary.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
                        let min_y = boundary.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
                        let max_y = boundary.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
                        let spacing = 3.0 * *h_scale; // mm
                        let pat_stroke = egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(
                            color.r(), color.g(), color.b(), 100));
                        let ang = angle.to_radians();
                        let cos_a = ang.cos();
                        let sin_a = ang.sin();
                        let diag = ((max_x - min_x).powi(2) + (max_y - min_y).powi(2)).sqrt();
                        let cx = (min_x + max_x) / 2.0;
                        let cy = (min_y + max_y) / 2.0;
                        // 繪製填充線（沿 angle 方向平行線）
                        let draw_line_set = |ang_cos: f64, ang_sin: f64| {
                            let n = (diag / spacing).ceil() as i32 + 1;
                            for i in -n..=n {
                                let offset = i as f64 * spacing;
                                let lx1 = cx + offset * (-ang_sin) - diag * ang_cos;
                                let ly1 = cy + offset * ang_cos - diag * ang_sin;
                                let lx2 = cx + offset * (-ang_sin) + diag * ang_cos;
                                let ly2 = cy + offset * ang_cos + diag * ang_sin;
                                painter.line_segment(
                                    [to_screen(lx1, ly1), to_screen(lx2, ly2)],
                                    pat_stroke,
                                );
                            }
                        };
                        match pattern {
                            kolibri_drafting::HatchPattern::Lines => draw_line_set(cos_a, sin_a),
                            kolibri_drafting::HatchPattern::Cross => {
                                draw_line_set(cos_a, sin_a);
                                draw_line_set(-sin_a, cos_a); // 垂直方向
                            }
                            kolibri_drafting::HatchPattern::Dots => {
                                let n = (diag / spacing).ceil() as i32 + 1;
                                for ix in -n..=n {
                                    for iy in -n..=n {
                                        let dx = ix as f64 * spacing;
                                        let dy = iy as f64 * spacing;
                                        let px = cx + dx * cos_a - dy * sin_a;
                                        let py = cy + dx * sin_a + dy * cos_a;
                                        if px >= min_x && px <= max_x && py >= min_y && py <= max_y {
                                            painter.circle_filled(to_screen(px, py), 0.8, pat_stroke.color);
                                        }
                                    }
                                }
                            }
                            kolibri_drafting::HatchPattern::Solid => {
                                // 已用半透明填充
                                let solid_fill = egui::Color32::from_rgba_unmultiplied(
                                    color.r(), color.g(), color.b(), 60);
                                painter.add(egui::Shape::convex_polygon(
                                    screen_pts.clone(), solid_fill, egui::Stroke::NONE,
                                ));
                            }
                            _ => draw_line_set(cos_a, sin_a), // Brick, Concrete → 預設用線
                        }
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
                    let sz = 6.0;
                    painter.line_segment([egui::pos2(sp.x, sp.y - sz), egui::pos2(sp.x + sz, sp.y)], st);
                    painter.line_segment([egui::pos2(sp.x + sz, sp.y), egui::pos2(sp.x, sp.y + sz)], st);
                    painter.line_segment([egui::pos2(sp.x, sp.y + sz), egui::pos2(sp.x - sz, sp.y)], st);
                    painter.line_segment([egui::pos2(sp.x - sz, sp.y), egui::pos2(sp.x, sp.y - sz)], st);
                    painter.text(egui::pos2(sp.x + sz + 2.0, sp.y),
                        egui::Align2::LEFT_CENTER, name,
                        egui::FontId::proportional(9.0), color);
                }
                // ── 修訂雲形（波浪邊界）──
                kolibri_drafting::DraftEntity::Revcloud { points, arc_radius } => {
                    if points.len() >= 2 {
                        let cloud_color = egui::Color32::from_rgb(255, 80, 80); // 紅色
                        let arc_r_px = (*arc_radius as f32 * scale).max(3.0);
                        let spts: Vec<egui::Pos2> = points.iter().map(|p| to_screen(p[0], p[1])).collect();
                        // 繪製波浪線（每段用小弧近似）
                        for i in 0..spts.len() {
                            let a = spts[i];
                            let b = spts[(i + 1) % spts.len()];
                            let dx = b.x - a.x;
                            let dy = b.y - a.y;
                            let seg_len = (dx * dx + dy * dy).sqrt();
                            let n_arcs = (seg_len / (arc_r_px * 2.0)).ceil().max(1.0) as usize;
                            for j in 0..n_arcs {
                                let t0 = j as f32 / n_arcs as f32;
                                let t1 = (j + 1) as f32 / n_arcs as f32;
                                let mid_t = (t0 + t1) / 2.0;
                                let mx = a.x + dx * mid_t;
                                let my = a.y + dy * mid_t;
                                // 小弧偏移（法向方向交替）
                                let nx = -dy / seg_len * arc_r_px * 0.4;
                                let ny = dx / seg_len * arc_r_px * 0.4;
                                let sign = if j % 2 == 0 { 1.0 } else { -1.0 };
                                let ctrl = egui::pos2(mx + nx * sign, my + ny * sign);
                                let p0 = egui::pos2(a.x + dx * t0, a.y + dy * t0);
                                let p1 = egui::pos2(a.x + dx * t1, a.y + dy * t1);
                                // 用兩段線近似弧
                                painter.line_segment([p0, ctrl], egui::Stroke::new(lw, cloud_color));
                                painter.line_segment([ctrl, p1], egui::Stroke::new(lw, cloud_color));
                            }
                        }
                    }
                }
                // ── 表格 ──
                kolibri_drafting::DraftEntity::Table { position, rows, cols, row_height, col_width, cells } => {
                    let p0 = to_screen(position[0], position[1]);
                    let rh = *row_height as f32 * scale;
                    let cw = *col_width as f32 * scale;
                    let total_w = *cols as f32 * cw;
                    let total_h = *rows as f32 * rh;
                    let table_stroke = egui::Stroke::new(0.8, color);
                    // 外框
                    painter.rect_stroke(egui::Rect::from_min_size(p0, egui::vec2(total_w, total_h)), 0.0, table_stroke);
                    // 水平線
                    for r in 1..*rows {
                        let y = p0.y + r as f32 * rh;
                        painter.line_segment([egui::pos2(p0.x, y), egui::pos2(p0.x + total_w, y)], table_stroke);
                    }
                    // 垂直線
                    for c in 1..*cols {
                        let x = p0.x + c as f32 * cw;
                        painter.line_segment([egui::pos2(x, p0.y), egui::pos2(x, p0.y + total_h)], table_stroke);
                    }
                    // 儲存格文字
                    for (idx, text) in cells.iter().enumerate() {
                        if text.is_empty() { continue; }
                        let r = idx as u32 / cols;
                        let c = idx as u32 % cols;
                        let cx = p0.x + c as f32 * cw + cw / 2.0;
                        let cy = p0.y + r as f32 * rh + rh / 2.0;
                        painter.text(egui::pos2(cx, cy), egui::Align2::CENTER_CENTER, text,
                            egui::FontId::proportional((rh * 0.6).max(8.0)), color);
                    }
                }
            }
        }

        // ── Hover 高亮（ZWCAD 風格：加粗 + 虛線效果）──
        let hover_mm = canvas_hover_pos.map(|pos| {
            [((pos.x - origin.x) / scale) as f64, ((origin.y - pos.y) / scale) as f64]
        });
        let mut hovered_entity_id: Option<kolibri_drafting::DraftId> = None;
        if let Some(mm) = hover_mm {
            let mut best_dist = 5.0_f64;
            for obj in &self.editor.draft_doc.objects {
                if !obj.visible { continue; }
                if self.editor.draft_selected.contains(&obj.id) { continue; } // 已選的不重複 hover
                let d = self.draft_entity_distance(&obj.entity, mm[0], mm[1]);
                if d < best_dist {
                    best_dist = d;
                    hovered_entity_id = Some(obj.id);
                }
            }
        }
        // 繪製 hover 高亮（加粗藍白，模擬 previeweffect）
        if let Some(hid) = hovered_entity_id {
            let hover_stroke = egui::Stroke::new(2.5, egui::Color32::from_rgba_unmultiplied(100, 200, 255, 180));
            if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == hid) {
                self.draw_entity_highlight(&painter, &to_screen, scale, &obj.entity, hover_stroke);
                // hover 時也顯示淡色 grip（綠色，AutoCAD 風格）
                let grips = self.entity_grip_points(&obj.entity);
                for gp in &grips {
                    let sp = to_screen(gp[0], gp[1]);
                    painter.rect_filled(
                        egui::Rect::from_center_size(sp, egui::vec2(5.0, 5.0)),
                        0.0, egui::Color32::from_rgba_unmultiplied(60, 200, 60, 120));
                }
            }
        }

        // ── 選取高亮 + grip 控制點（ZWCAD 風格：cold=藍, hover=綠）──
        let sel_stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(76, 139, 245));
        let grip_cold = egui::Color32::from_rgb(30, 80, 220); // cold grip: 深藍
        let grip_hover = egui::Color32::from_rgb(60, 200, 60); // hover grip: 綠
        let grip_size = 6.0; // ZWCAD grip 比較大
        for &sel_id in &self.editor.draft_selected {
            if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == sel_id) {
                self.draw_entity_highlight(&painter, &to_screen, scale, &obj.entity, sel_stroke);
                let grips = self.entity_grip_points(&obj.entity);
                for gp in &grips {
                    let sp = to_screen(gp[0], gp[1]);
                    // 判斷滑鼠是否 hover 在此 grip 上
                    let is_grip_hovered = if let Some(hp) = canvas_hover_pos {
                        (hp.x - sp.x).abs() < grip_size && (hp.y - sp.y).abs() < grip_size
                    } else { false };
                    let gc = if is_grip_hovered { grip_hover } else { grip_cold };
                    // 填充方塊 + 白色邊框
                    painter.rect_filled(
                        egui::Rect::from_center_size(sp, egui::vec2(grip_size, grip_size)),
                        0.0, gc);
                    painter.rect_stroke(
                        egui::Rect::from_center_size(sp, egui::vec2(grip_size, grip_size)),
                        0.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                }
            }
        }

        // ── 繪製進行中的繪製狀態 ──
        self.draw_draft_preview(&painter, &to_screen, scale, &response, canvas_hover_pos);

        // ── 處理滑鼠點擊 ──
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                // 先用 snap 點，否則用原始座標
                let (mut mm_x, mut mm_y) = if let Some((sp, _, _)) = &snap_point {
                    (sp[0], sp[1])
                } else {
                    (((pos.x - origin.x) / scale) as f64,
                     ((origin.y - pos.y) / scale) as f64)
                };

                // ORTHO 約束：鎖定到 H 或 V 方向
                if self.editor.draft_ortho {
                    let from_point: Option<[f64; 2]> = match &self.editor.draft_state {
                        crate::editor::DraftDrawState::LineFrom { p1 } => Some(*p1),
                        crate::editor::DraftDrawState::RectFrom { p1 } => Some(*p1),
                        crate::editor::DraftDrawState::DimP1 { p1 } => Some(*p1),
                        _ => None,
                    };
                    if let Some(fp) = from_point {
                        let dx = (mm_x - fp[0]).abs();
                        let dy = (mm_y - fp[1]).abs();
                        if dx > dy {
                            mm_y = fp[1]; // 水平方向
                        } else {
                            mm_x = fp[0]; // 垂直方向
                        }
                    }
                }

                let shift = ui.input(|i| i.modifiers.shift);
                self.handle_draft_click_v2(mm_x, mm_y, shift);
            }
        }

        // 右鍵上下文選單（ZWCAD 風格）
        response.context_menu(|ui| {
            #[cfg(feature = "drafting")]
            {
                use crate::editor::DraftDrawState;
                let is_drawing = !matches!(self.editor.draft_state, DraftDrawState::Idle);
                let has_sel = !self.editor.draft_selected.is_empty();

                if is_drawing {
                    // 繪圖中的右鍵選單
                    if ui.button("確認 (Enter)").clicked() {
                        self.finish_draft_tool();
                        ui.close_menu();
                    }
                    if ui.button("取消 (Esc)").clicked() {
                        self.editor.draft_state = DraftDrawState::Idle;
                        ui.close_menu();
                    }
                    ui.separator();
                    if matches!(self.editor.draft_state, DraftDrawState::PolylinePoints { .. }) {
                        if ui.button("關閉 (C)").clicked() {
                            // 關閉多段線
                            if let DraftDrawState::PolylinePoints { mut points } = self.editor.draft_state.clone() {
                                if points.len() >= 2 {
                                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Polyline {
                                        points, closed: true,
                                    });
                                    self.console_push("ACTION", "多段線（關閉）完成".into());
                                }
                                self.editor.draft_state = DraftDrawState::Idle;
                            }
                            ui.close_menu();
                        }
                    }
                } else if has_sel {
                    // 有選取物件的右鍵選單
                    if ui.button("刪除 (Del)").clicked() {
                        let ids: Vec<_> = self.editor.draft_selected.drain(..).collect();
                        for id in &ids { self.editor.draft_doc.remove(*id); }
                        self.console_push("ACTION", format!("刪除 {} 個圖元", ids.len()));
                        ui.close_menu();
                    }
                    if ui.button("移動 (M)").clicked() {
                        self.editor.tool = Tool::DraftMove;
                        ui.close_menu();
                    }
                    if ui.button("複製 (CO)").clicked() {
                        self.editor.tool = Tool::DraftCopy;
                        ui.close_menu();
                    }
                    if ui.button("旋轉 (RO)").clicked() {
                        self.editor.tool = Tool::DraftRotate;
                        ui.close_menu();
                    }
                    if ui.button("比例 (SC)").clicked() {
                        self.editor.tool = Tool::DraftScale;
                        ui.close_menu();
                    }
                    if ui.button("鏡射 (MI)").clicked() {
                        self.editor.tool = Tool::DraftMirror;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("分解 (X)").clicked() {
                        self.editor.tool = Tool::DraftExplode;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("取消選取").clicked() {
                        self.editor.draft_selected.clear();
                        ui.close_menu();
                    }
                } else {
                    // 空白處右鍵
                    if ui.button("重複上一指令").clicked() {
                        // 保持目前工具
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("縮放全部 (Z+A)").clicked() {
                        self.editor.tool = Tool::DraftZoomAll;
                        ui.close_menu();
                    }
                    if ui.button("平移 (P)").clicked() {
                        self.editor.tool = Tool::DraftPan;
                        ui.close_menu();
                    }
                }
            }
        });

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

        // ── F-key toggles（ORTHO/POLAR/OSNAP/DYN）──
        if ui.input(|i| i.key_pressed(egui::Key::F3)) {
            self.editor.draft_osnap = !self.editor.draft_osnap;
            self.console_push("INFO", format!("物件鎖點: {}", if self.editor.draft_osnap { "ON" } else { "OFF" }));
        }
        if ui.input(|i| i.key_pressed(egui::Key::F8)) {
            self.editor.draft_ortho = !self.editor.draft_ortho;
            if self.editor.draft_ortho { self.editor.draft_polar = false; }
            self.console_push("INFO", format!("正交: {}", if self.editor.draft_ortho { "ON" } else { "OFF" }));
        }
        if ui.input(|i| i.key_pressed(egui::Key::F10)) {
            self.editor.draft_polar = !self.editor.draft_polar;
            if self.editor.draft_polar { self.editor.draft_ortho = false; }
            self.console_push("INFO", format!("極座標追蹤: {}", if self.editor.draft_polar { "ON" } else { "OFF" }));
        }
        if ui.input(|i| i.key_pressed(egui::Key::F12)) {
            self.editor.draft_dyn_input = !self.editor.draft_dyn_input;
            self.console_push("INFO", format!("動態輸入: {}", if self.editor.draft_dyn_input { "ON" } else { "OFF" }));
        }

        // ── Mouse Wheel Zoom（以游標為中心，ZWCAD 風格）──
        let canvas_center = rect.center();
        {
            let scroll = ui.input(|i| {
                let s = i.smooth_scroll_delta.y;
                if s.abs() > 0.1 { s } else { i.raw_scroll_delta.y }
            });
            if scroll.abs() > 0.1 && canvas_hover_pos.is_some() {
                self.draft_zoom_at_cursor(scroll, canvas_hover_pos, canvas_center);
            }
        }

        // ── 鍵盤 +/- Zoom（備用，與 ZWCAD 一致）──
        if ui.input(|i| i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)) {
            self.draft_zoom_at_cursor(100.0, canvas_hover_pos, canvas_center);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Minus)) {
            self.draft_zoom_at_cursor(-100.0, canvas_hover_pos, canvas_center);
        }

        // ── Middle Mouse Button Pan（中鍵拖曳平移，ZWCAD 風格）──
        {
            let middle_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle));
            if middle_down {
                if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                    if let Some(prev) = self.editor.draft_pan_drag {
                        let delta = pos - prev;
                        self.editor.draft_offset += delta;
                    }
                    self.editor.draft_pan_drag = Some(pos);
                }
            } else {
                self.editor.draft_pan_drag = None;
            }
        }

        // ── DraftPan tool（左鍵拖曳平移）──
        if self.editor.tool == Tool::DraftPan {
            if response.dragged_by(egui::PointerButton::Primary) {
                self.editor.draft_offset += response.drag_delta();
            }
            if response.drag_stopped_by(egui::PointerButton::Primary) {
                // 單次平移完成後回到 Select
                self.editor.tool = Tool::DraftSelect;
            }
        }

        // ── DraftZoomAll（縮放至全部圖元）──
        if self.editor.tool == Tool::DraftZoomAll {
            self.draft_zoom_all(rect);
            self.editor.tool = Tool::DraftSelect;
        }

        // ── DraftZoomWindow（目前先用 ZoomAll 替代）──
        if self.editor.tool == Tool::DraftZoomWindow {
            self.draft_zoom_all(rect);
            self.editor.tool = Tool::DraftSelect;
        }

        // ── 指令別名輸入（L=Line, C=Circle, PL=Polyline, REC=Rectangle...）──
        // 注意：正在數值輸入時（draft_num_input 非空）跳過指令別名
        #[cfg(feature = "drafting")]
        if self.editor.draft_num_input.is_empty() {
            // 超過 500ms 未按鍵，清除緩衝
            if self.editor.draft_cmd_time.elapsed() > std::time::Duration::from_millis(500) {
                if !self.editor.draft_cmd_buf.is_empty() {
                    self.editor.draft_cmd_buf.clear();
                }
            }
            // 收集本幀按下的字母鍵（不在文字編輯時）
            let pressed_char: Option<char> = ui.input(|i| {
                // 若有 modifier（Ctrl/Alt/Cmd）不觸發指令別名
                if i.modifiers.ctrl || i.modifiers.alt || i.modifiers.mac_cmd { return None; }
                for evt in &i.events {
                    if let egui::Event::Text(t) = evt {
                        let ch = t.chars().next();
                        if let Some(c) = ch {
                            if c.is_ascii_alphabetic() {
                                return Some(c.to_ascii_uppercase());
                            }
                        }
                    }
                }
                None
            });
            if let Some(ch) = pressed_char {
                self.editor.draft_cmd_buf.push(ch);
                self.editor.draft_cmd_time = std::time::Instant::now();
                // 嘗試匹配指令別名
                let buf = self.editor.draft_cmd_buf.as_str();
                let matched_tool: Option<crate::editor::Tool> = match buf {
                    "L" => Some(crate::editor::Tool::DraftLine),
                    "C" => Some(crate::editor::Tool::DraftCircle),
                    "A" => Some(crate::editor::Tool::DraftArc),
                    "PL" => Some(crate::editor::Tool::DraftPolyline),
                    "REC" => Some(crate::editor::Tool::DraftRectangle),
                    "M" => Some(crate::editor::Tool::DraftMove),
                    "CO" => Some(crate::editor::Tool::DraftCopy),
                    "RO" => Some(crate::editor::Tool::DraftRotate),
                    "MI" => Some(crate::editor::Tool::DraftMirror),
                    "SC" => Some(crate::editor::Tool::DraftScale),
                    "TR" => Some(crate::editor::Tool::DraftTrim),
                    "EX" => Some(crate::editor::Tool::DraftExtend),
                    "O" => Some(crate::editor::Tool::DraftOffset),
                    "F" => Some(crate::editor::Tool::DraftFillet),
                    "E" => Some(crate::editor::Tool::DraftErase),
                    "X" => Some(crate::editor::Tool::DraftExplode),
                    "H" => Some(crate::editor::Tool::DraftHatch),
                    "MT" => Some(crate::editor::Tool::DraftText),
                    "DI" => Some(crate::editor::Tool::DraftMeasureDist), // DI = distance measure
                    "AA" => Some(crate::editor::Tool::DraftMeasureArea), // AA = area
                    "MA" => Some(crate::editor::Tool::DraftMatchProp),   // MA = match properties
                    "LI" => Some(crate::editor::Tool::DraftList),        // LI = list
                    "ID" => Some(crate::editor::Tool::DraftIdPoint),     // ID = id point
                    "RAY" => Some(crate::editor::Tool::DraftRay),        // RAY = ray
                    "LEN" => Some(crate::editor::Tool::DraftLengthen),   // LEN = lengthen
                    "DCE" => Some(crate::editor::Tool::DraftCenterMark), // DCE = center mark
                    "AR" => Some(crate::editor::Tool::DraftArrayRect),   // AR = array rect
                    "AP" => Some(crate::editor::Tool::DraftArrayPolar),  // AP = array polar
                    "P" => Some(crate::editor::Tool::DraftPan),          // P = pan
                    "ZA" => Some(crate::editor::Tool::DraftZoomAll),     // ZA = zoom all
                    "ZW" => Some(crate::editor::Tool::DraftZoomWindow),  // ZW = zoom window
                    _ => None,
                };
                // 如果匹配成功，切換工具並清空緩衝
                if let Some(tool) = matched_tool {
                    self.editor.tool = tool;
                    self.editor.draft_state = crate::editor::DraftDrawState::Idle;
                    self.console_push("CMD", format!("指令: {}", buf));
                    self.editor.draft_cmd_buf.clear();
                } else {
                    // 若緩衝太長（>3 字元）且沒匹配，清空
                    if self.editor.draft_cmd_buf.len() > 3 {
                        self.editor.draft_cmd_buf.clear();
                    }
                    // 單字元指令（L/C/A/M/O/F/E/X/H）可能被多字元前綴遮住
                    // 不做額外處理：等 500ms 超時即清除
                }
            }
        }

        // ── AutoCAD/ZWCAD 風格數值輸入（畫線/圓/矩形時直接打數字 + Enter）──
        #[cfg(feature = "drafting")]
        {
            use crate::editor::DraftDrawState;
            let is_drawing = !matches!(self.editor.draft_state, DraftDrawState::Idle);
            // 收集數字/小數點/逗號/負號/@ 輸入
            let num_events: Vec<char> = ui.input(|i| {
                let mut chars = Vec::new();
                if i.modifiers.ctrl || i.modifiers.alt || i.modifiers.mac_cmd { return chars; }
                for evt in &i.events {
                    match evt {
                        egui::Event::Text(t) => {
                            for c in t.chars() {
                                if c.is_ascii_digit() || c == '.' || c == ',' || c == '-' || c == '@' || c == '<' {
                                    chars.push(c);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                chars
            });
            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let backspace = ui.input(|i| i.key_pressed(egui::Key::Backspace));
            let esc_pressed = ui.input(|i| i.key_pressed(egui::Key::Escape));

            // 數字鍵 → 加入緩衝
            for ch in &num_events {
                if is_drawing || !self.editor.draft_num_input.is_empty() {
                    self.editor.draft_num_input.push(*ch);
                }
            }
            if backspace && !self.editor.draft_num_input.is_empty() {
                self.editor.draft_num_input.pop();
            }
            if esc_pressed {
                self.editor.draft_num_input.clear();
            }

            // Enter 按下且有數值輸入 → 解析並套用
            if enter_pressed && !self.editor.draft_num_input.is_empty() && is_drawing {
                let input = self.editor.draft_num_input.clone();
                self.editor.draft_num_input.clear();

                // 解析格式：
                // "500"         → 距離 500mm（沿目前方向）
                // "500,300"     → 相對座標 dx=500, dy=300
                // "@500<45"     → 極座標 距離 500，角度 45°
                let from_point: Option<[f64; 2]> = match &self.editor.draft_state {
                    DraftDrawState::LineFrom { p1 } => Some(*p1),
                    DraftDrawState::RectFrom { p1 } => Some(*p1),
                    DraftDrawState::CircleCenter { center } => Some(*center),
                    DraftDrawState::PolylinePoints { points } => points.last().copied(),
                    _ => None,
                };

                if let Some(fp) = from_point {
                    let target: Option<[f64; 2]> = if input.contains(',') {
                        // "dx,dy" 相對座標
                        let parts: Vec<&str> = input.split(',').collect();
                        if parts.len() == 2 {
                            if let (Ok(dx), Ok(dy)) = (parts[0].trim().parse::<f64>(), parts[1].trim().parse::<f64>()) {
                                Some([fp[0] + dx, fp[1] + dy])
                            } else { None }
                        } else { None }
                    } else if input.contains('<') {
                        // "@dist<angle" 極座標
                        let clean = input.trim_start_matches('@');
                        let parts: Vec<&str> = clean.split('<').collect();
                        if parts.len() == 2 {
                            if let (Ok(dist), Ok(angle)) = (parts[0].trim().parse::<f64>(), parts[1].trim().parse::<f64>()) {
                                let rad = angle.to_radians();
                                Some([fp[0] + dist * rad.cos(), fp[1] + dist * rad.sin()])
                            } else { None }
                        } else { None }
                    } else {
                        // 純數字 → 距離（沿滑鼠方向）
                        if let Ok(dist) = input.parse::<f64>() {
                            if let Some(hover_pos) = canvas_hover_pos {
                                let mx = ((hover_pos.x - origin.x) / scale) as f64;
                                let my = ((origin.y - hover_pos.y) / scale) as f64;
                                let dx = mx - fp[0];
                                let dy = my - fp[1];
                                let len = (dx * dx + dy * dy).sqrt();
                                if len > 0.001 {
                                    Some([fp[0] + dx / len * dist, fp[1] + dy / len * dist])
                                } else {
                                    Some([fp[0] + dist, fp[1]]) // 預設向右
                                }
                            } else {
                                Some([fp[0] + dist, fp[1]])
                            }
                        } else { None }
                    };

                    if let Some(to) = target {
                        match &self.editor.draft_state.clone() {
                            DraftDrawState::LineFrom { p1 } => {
                                self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Line {
                                    start: *p1, end: to,
                                });
                                // 連續畫線：新起點 = 舊終點
                                self.editor.draft_state = DraftDrawState::LineFrom { p1: to };
                                self.console_push("ACTION", format!("LINE: ({:.1},{:.1})→({:.1},{:.1})", p1[0], p1[1], to[0], to[1]));
                            }
                            DraftDrawState::RectFrom { p1 } => {
                                self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Rectangle {
                                    p1: *p1, p2: to,
                                });
                                self.editor.draft_state = DraftDrawState::Idle;
                            }
                            DraftDrawState::CircleCenter { center } => {
                                // 數字 = 半徑
                                if let Ok(r) = input.parse::<f64>() {
                                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Circle {
                                        center: *center, radius: r,
                                    });
                                    self.editor.draft_state = DraftDrawState::Idle;
                                }
                            }
                            DraftDrawState::PolylinePoints { points } => {
                                let mut pts = points.clone();
                                pts.push(to);
                                self.editor.draft_state = DraftDrawState::PolylinePoints { points: pts };
                            }
                            _ => {}
                        }
                    }
                }
            }

            // 顯示數值輸入提示（游標旁的輸入框）
            if !self.editor.draft_num_input.is_empty() {
                if let Some(hover_pos) = canvas_hover_pos {
                    let input_rect = egui::Rect::from_min_size(
                        egui::pos2(hover_pos.x + 20.0, hover_pos.y + 45.0),
                        egui::vec2(100.0, 20.0),
                    );
                    let bg = egui::Color32::from_rgba_unmultiplied(20, 25, 35, 240);
                    let border = egui::Color32::from_rgb(76, 139, 245);
                    painter.rect_filled(input_rect, 3.0, bg);
                    painter.rect_stroke(input_rect, 3.0, egui::Stroke::new(1.5, border));
                    painter.text(
                        egui::pos2(input_rect.left() + 4.0, input_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &self.editor.draft_num_input,
                        egui::FontId::monospace(12.0),
                        egui::Color32::from_rgb(240, 240, 245),
                    );
                    // 閃爍游標
                    let blink = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() / 500) % 2 == 0;
                    if blink {
                        let galley = painter.layout_no_wrap(
                            self.editor.draft_num_input.clone(),
                            egui::FontId::monospace(12.0),
                            egui::Color32::WHITE,
                        );
                        let cursor_x = input_rect.left() + 4.0 + galley.size().x + 1.0;
                        painter.line_segment(
                            [egui::pos2(cursor_x, input_rect.top() + 3.0), egui::pos2(cursor_x, input_rect.bottom() - 3.0)],
                            egui::Stroke::new(1.0, egui::Color32::WHITE),
                        );
                    }
                }
            }
        }

        // 持續 repaint（十字游標需要跟隨滑鼠）
        if canvas_hover_pos.is_some() {
            ui.ctx().request_repaint();
        }

        // ── 動態輸入 (DYN) — 游標旁顯示距離/角度 tooltip ──
        if self.editor.draft_dyn_input {
            if let Some(hover_pos) = canvas_hover_pos {
                let mx = ((hover_pos.x - origin.x) / scale) as f64;
                let my = ((origin.y - hover_pos.y) / scale) as f64;
                let cursor_pos = if let Some((sp, _, _)) = &snap_point {
                    to_screen(sp[0], sp[1])
                } else {
                    hover_pos
                };

                // 取得起點（如果正在繪圖中）
                let from_point: Option<[f64; 2]> = match &self.editor.draft_state {
                    crate::editor::DraftDrawState::LineFrom { p1 } => Some(*p1),
                    crate::editor::DraftDrawState::DimP1 { p1 } => Some(*p1),
                    crate::editor::DraftDrawState::DimP2 { p1, .. } => Some(*p1),
                    crate::editor::DraftDrawState::RectFrom { p1 } => Some(*p1),
                    crate::editor::DraftDrawState::CircleCenter { center } => Some(*center),
                    crate::editor::DraftDrawState::ArcCenter { center } => Some(*center),
                    _ => None,
                };

                let tooltip_bg = egui::Color32::from_rgba_unmultiplied(30, 30, 35, 220);
                let tooltip_text = egui::Color32::from_rgb(230, 230, 235);
                let tooltip_dim = egui::Color32::from_rgb(140, 145, 155);
                let tooltip_y = cursor_pos.y + 24.0;
                let tooltip_x = cursor_pos.x + 20.0;

                if let Some(fp) = from_point {
                    // 有起點：顯示距離 + 角度
                    let snap_mm = if let Some((sp, _, _)) = &snap_point { *sp } else { [mx, my] };
                    let dx = snap_mm[0] - fp[0];
                    let dy = snap_mm[1] - fp[1];
                    let dist = (dx * dx + dy * dy).sqrt();
                    let angle_deg = dy.atan2(dx).to_degrees();
                    let angle_norm = if angle_deg < 0.0 { angle_deg + 360.0 } else { angle_deg };

                    let dist_text = format!("{:.1}", dist);
                    let angle_text = format!("{:.1}°", angle_norm);

                    // 距離 tooltip（上方）
                    let dist_rect = egui::Rect::from_min_size(
                        egui::pos2(tooltip_x, tooltip_y),
                        egui::vec2(70.0, 18.0));
                    painter.rect_filled(dist_rect, 3.0, tooltip_bg);
                    painter.rect_stroke(dist_rect, 3.0, egui::Stroke::new(0.5, egui::Color32::from_rgb(80, 85, 95)));
                    painter.text(egui::pos2(dist_rect.left() + 4.0, dist_rect.center().y),
                        egui::Align2::LEFT_CENTER, &dist_text,
                        egui::FontId::proportional(11.0), tooltip_text);

                    // 角度 tooltip（下方）
                    let angle_rect = egui::Rect::from_min_size(
                        egui::pos2(tooltip_x, tooltip_y + 20.0),
                        egui::vec2(70.0, 18.0));
                    painter.rect_filled(angle_rect, 3.0, tooltip_bg);
                    painter.rect_stroke(angle_rect, 3.0, egui::Stroke::new(0.5, egui::Color32::from_rgb(80, 85, 95)));
                    painter.text(egui::pos2(angle_rect.left() + 4.0, angle_rect.center().y),
                        egui::Align2::LEFT_CENTER, &angle_text,
                        egui::FontId::proportional(11.0), tooltip_dim);
                } else {
                    // 無起點：顯示絕對座標
                    let snap_mm = if let Some((sp, _, _)) = &snap_point { *sp } else { [mx, my] };
                    let coord_text = format!("{:.1}, {:.1}", snap_mm[0], snap_mm[1]);
                    let coord_rect = egui::Rect::from_min_size(
                        egui::pos2(tooltip_x, tooltip_y),
                        egui::vec2(100.0, 18.0));
                    painter.rect_filled(coord_rect, 3.0, tooltip_bg);
                    painter.rect_stroke(coord_rect, 3.0, egui::Stroke::new(0.5, egui::Color32::from_rgb(80, 85, 95)));
                    painter.text(egui::pos2(coord_rect.left() + 4.0, coord_rect.center().y),
                        egui::Align2::LEFT_CENTER, &coord_text,
                        egui::FontId::proportional(11.0), tooltip_text);
                }
            }
        }

        // ── POLAR tracking — 角度吸附線 ──
        if self.editor.draft_polar {
            if let Some(hover_pos) = canvas_hover_pos {
                let from_point: Option<[f64; 2]> = match &self.editor.draft_state {
                    crate::editor::DraftDrawState::LineFrom { p1 } => Some(*p1),
                    crate::editor::DraftDrawState::RectFrom { p1 } => Some(*p1),
                    _ => None,
                };
                if let Some(fp) = from_point {
                    let mx = ((hover_pos.x - origin.x) / scale) as f64;
                    let my = ((origin.y - hover_pos.y) / scale) as f64;
                    let dx = mx - fp[0];
                    let dy = my - fp[1];
                    let angle = dy.atan2(dx).to_degrees();
                    let angle_norm = if angle < 0.0 { angle + 360.0 } else { angle };
                    // 檢查是否接近 0/45/90/135/180/225/270/315
                    let polar_angles = [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0, 360.0];
                    for &pa in &polar_angles {
                        let diff = (angle_norm - pa).abs();
                        if diff < 5.0 || (360.0 - diff) < 5.0 {
                            // 繪製極座標追蹤線（虛線）
                            let dist = (dx * dx + dy * dy).sqrt();
                            let pa_rad = pa.to_radians();
                            let end_x = fp[0] + dist * pa_rad.cos();
                            let end_y = fp[1] + dist * pa_rad.sin();
                            let track_color = egui::Color32::from_rgba_unmultiplied(0, 200, 100, 120);
                            painter.line_segment(
                                [to_screen(fp[0], fp[1]), to_screen(end_x, end_y)],
                                egui::Stroke::new(0.5, track_color));
                            // 角度標籤
                            let label_pos = to_screen(end_x, end_y);
                            painter.text(
                                egui::pos2(label_pos.x + 8.0, label_pos.y - 8.0),
                                egui::Align2::LEFT_BOTTOM,
                                format!("{:.0}°", pa),
                                egui::FontId::proportional(9.0), track_color);
                            break;
                        }
                    }
                }
            }
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

        // ── 命令提示列（ZWCAD 風格：底部半透明黑底）──
        {
            use crate::editor::DraftDrawState;
            let tool_name = match self.editor.tool {
                Tool::DraftSelect => "SELECT",
                Tool::DraftLine => "LINE",
                Tool::DraftArc => "ARC",
                Tool::DraftCircle => "CIRCLE",
                Tool::DraftRectangle => "RECTANG",
                Tool::DraftPolyline => "PLINE",
                Tool::DraftEllipse => "ELLIPSE",
                Tool::DraftPolygon => "POLYGON",
                Tool::DraftSpline => "SPLINE",
                Tool::DraftDimLinear => "DIMLINEAR",
                Tool::DraftDimAligned => "DIMALIGNED",
                Tool::DraftDimAngle => "DIMANGULAR",
                Tool::DraftDimRadius => "DIMRADIUS",
                Tool::DraftDimDiameter => "DIMDIAMETER",
                Tool::DraftDimContinue => "DIMCONTINUE",
                Tool::DraftTrim => "TRIM",
                Tool::DraftExtend => "EXTEND",
                Tool::DraftOffset => "OFFSET",
                Tool::DraftFillet => "FILLET",
                Tool::DraftChamfer => "CHAMFER",
                Tool::DraftMove => "MOVE",
                Tool::DraftCopy => "COPY",
                Tool::DraftRotate => "ROTATE",
                Tool::DraftScale => "SCALE",
                Tool::DraftMirror => "MIRROR",
                Tool::DraftText => "MTEXT",
                Tool::DraftLeader => "LEADER",
                Tool::DraftHatch => "HATCH",
                Tool::DraftExplode => "EXPLODE",
                Tool::DraftStretch => "STRETCH",
                Tool::DraftErase => "ERASE",
                Tool::DraftBreak => "BREAK",
                Tool::DraftJoin => "JOIN",
                Tool::DraftRevcloud => "REVCLOUD",
                Tool::DraftTable => "TABLE",
                Tool::DraftCircle2P => "CIRCLE 2P",
                Tool::DraftCircle3P => "CIRCLE 3P",
                Tool::DraftArc3P => "ARC 3P",
                Tool::DraftArcSCE => "ARC SCE",
                _ => "",
            };
            let prompt = match &self.editor.draft_state {
                DraftDrawState::Idle => {
                    if tool_name.is_empty() { String::new() }
                    else { format!("命令: {}", tool_name) }
                }
                DraftDrawState::LineFrom { .. } => format!("{}: 指定下一點 [復原(U)]:", tool_name),
                DraftDrawState::ArcCenter { .. } => format!("{}: 指定半徑點:", tool_name),
                DraftDrawState::ArcRadius { .. } => format!("{}: 指定終點角度:", tool_name),
                DraftDrawState::CircleCenter { .. } => format!("{}: 指定半徑:", tool_name),
                DraftDrawState::RectFrom { .. } => format!("{}: 指定對角點:", tool_name),
                DraftDrawState::PolylinePoints { points } => {
                    if points.len() >= 2 {
                        format!("{}: 指定下一點 [關閉(C)/復原(U)]:", tool_name)
                    } else {
                        format!("{}: 指定下一點:", tool_name)
                    }
                }
                DraftDrawState::DimP1 { .. } => format!("{}: 指定第二個延伸線起點:", tool_name),
                DraftDrawState::DimP2 { .. } => format!("{}: 指定標註線位置 [多行文字(M)/文字(T)/角度(A)]:", tool_name),
                DraftDrawState::AngleP1 { .. } => format!("{}: 指定第二邊端點:", tool_name),
                DraftDrawState::TextPlace => format!("{}: 指定文字放置點:", tool_name),
                DraftDrawState::LeaderPoints { points } => {
                    format!("{}: 指定下一點 ({}點):", tool_name, points.len())
                }
            };
            if !prompt.is_empty() {
                // 半透明背景
                let cmd_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.bottom() - 24.0),
                    egui::vec2(rect.width(), 24.0));
                painter.rect_filled(cmd_rect, 0.0,
                    egui::Color32::from_rgba_unmultiplied(20, 22, 28, 200));
                painter.text(
                    egui::pos2(cmd_rect.left() + 8.0, cmd_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &prompt,
                    egui::FontId::monospace(11.0),
                    egui::Color32::from_rgb(200, 200, 210),
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
        let dim_stroke = egui::Stroke::new(0.6, color);
        painter.line_segment([d1, d2], dim_stroke);

        // 延伸線（含 gap 和 overshoot）
        // gap = 0.625mm（從原點開始留空隙），overshoot = 1.25mm（超出尺寸線）
        let ext_stroke = egui::Stroke::new(0.3, color);
        let gap_px = 0.625 * scale;
        let overshoot_px = 1.25 * scale;
        // 延伸線方向：從 s → d（法線方向）
        {
            let ext_dx = d1.x - s1.x;
            let ext_dy = d1.y - s1.y;
            let ext_len = (ext_dx * ext_dx + ext_dy * ext_dy).sqrt();
            if ext_len > 0.1 {
                let ux = ext_dx / ext_len;
                let uy = ext_dy / ext_len;
                let e1_start = egui::pos2(s1.x + ux * gap_px, s1.y + uy * gap_px);
                let e1_end = egui::pos2(d1.x + ux * overshoot_px, d1.y + uy * overshoot_px);
                painter.line_segment([e1_start, e1_end], ext_stroke);
            }
        }
        {
            let ext_dx = d2.x - s2.x;
            let ext_dy = d2.y - s2.y;
            let ext_len = (ext_dx * ext_dx + ext_dy * ext_dy).sqrt();
            if ext_len > 0.1 {
                let ux = ext_dx / ext_len;
                let uy = ext_dy / ext_len;
                let e2_start = egui::pos2(s2.x + ux * gap_px, s2.y + uy * gap_px);
                let e2_end = egui::pos2(d2.x + ux * overshoot_px, d2.y + uy * overshoot_px);
                painter.line_segment([e2_start, e2_end], ext_stroke);
            }
        }

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
        hover_pos: Option<egui::Pos2>,
    ) {
        let preview_color = egui::Color32::from_rgb(76, 139, 245);
        let preview_stroke = egui::Stroke::new(1.0, preview_color);

        // 取得目前滑鼠位置（mm，原點在畫布中央，Y 向上）
        let mouse_mm = hover_pos.map(|pos| {
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
                    // 紅色虛線：p1 到滑鼠
                    painter.line_segment(
                        [to_screen(p1[0], p1[1]), to_screen(mm[0], mm[1])],
                        egui::Stroke::new(0.8, egui::Color32::from_rgb(200, 50, 50)),
                    );
                    // 距離標籤
                    let dist = kolibri_drafting::DraftDocument::distance(p1, &mm);
                    let mid = to_screen((p1[0] + mm[0]) / 2.0, (p1[1] + mm[1]) / 2.0);
                    painter.text(mid, egui::Align2::CENTER_BOTTOM,
                        format!("{:.0}", dist),
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgb(200, 50, 50));
                }
            }
            crate::editor::DraftDrawState::DimP2 { p1, p2 } => {
                if let Some(mm) = mouse_mm {
                    // 即時預覽完整標註（含 offset）
                    let offset = self.point_to_line_signed_dist(&mm, p1, p2);
                    let dim_preview = egui::Color32::from_rgb(0, 200, 200);
                    self.draw_dim_linear(painter, to_screen, p1, p2, offset, None, scale, dim_preview);
                }
            }
            crate::editor::DraftDrawState::AngleP1 { center, p1 } => {
                if let Some(mm) = mouse_mm {
                    // 預覽角度弧
                    let r = kolibri_drafting::DraftDocument::distance(center, p1) * 0.6;
                    let a1 = kolibri_drafting::DraftDocument::angle(center, p1);
                    let a2 = kolibri_drafting::DraftDocument::angle(center, &mm);
                    let n = 24;
                    let mut pts = Vec::with_capacity(n + 1);
                    let sc = to_screen(center[0], center[1]);
                    let rpx = r as f32 * scale;
                    for i in 0..=n {
                        let t = a1 + (a2 - a1) * i as f64 / n as f64;
                        pts.push(egui::pos2(sc.x + rpx * t.cos() as f32, sc.y - rpx * t.sin() as f32));
                    }
                    for w in pts.windows(2) {
                        painter.line_segment([w[0], w[1]],
                            egui::Stroke::new(0.8, egui::Color32::from_rgb(0, 200, 200)));
                    }
                    // 兩條邊線
                    painter.line_segment([to_screen(center[0], center[1]), to_screen(p1[0], p1[1])],
                        egui::Stroke::new(0.4, egui::Color32::from_rgba_unmultiplied(0, 200, 200, 100)));
                    painter.line_segment([to_screen(center[0], center[1]), to_screen(mm[0], mm[1])],
                        egui::Stroke::new(0.4, egui::Color32::from_rgba_unmultiplied(0, 200, 200, 100)));
                    // 角度值
                    let deg = ((a2 - a1).to_degrees()).abs();
                    let mid_a = (a1 + a2) / 2.0;
                    let tp = to_screen(center[0] + r * 1.2 * mid_a.cos(), center[1] + r * 1.2 * mid_a.sin());
                    painter.text(tp, egui::Align2::CENTER_CENTER, format!("{:.1}°", deg),
                        egui::FontId::proportional(10.0), egui::Color32::from_rgb(0, 200, 200));
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

            // ── 標註工具（ZWCAD 3-click：p1→p2→拖曳 offset 位置）──
            Tool::DraftDimLinear | Tool::DraftDimAligned => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::DimP1 { p1: p };
                        self.console_push("INFO", "指定第二個延伸線起點".into());
                    }
                    DraftDrawState::DimP1 { p1 } => {
                        self.editor.draft_state = DraftDrawState::DimP2 { p1, p2: p };
                        self.console_push("INFO", "拖曳滑鼠放置標註線位置".into());
                    }
                    DraftDrawState::DimP2 { p1, p2 } => {
                        // 第 3 click：計算 offset = 滑鼠到 p1-p2 連線的垂直距離
                        let offset = self.point_to_line_signed_dist(&p, &p1, &p2);
                        let entity = if self.editor.tool == Tool::DraftDimLinear {
                            kolibri_drafting::DraftEntity::DimLinear {
                                p1, p2, offset, text_override: None,
                            }
                        } else {
                            kolibri_drafting::DraftEntity::DimAligned {
                                p1, p2, offset, text_override: None,
                            }
                        };
                        self.editor.draft_doc.add(entity);
                        let dist = kolibri_drafting::DraftDocument::distance(&p1, &p2);
                        self.console_push("ACTION", format!("標註: {:.0}mm (offset {:.0})", dist, offset));
                        self.editor.draft_state = DraftDrawState::Idle;
                    }
                    _ => { self.editor.draft_state = DraftDrawState::DimP1 { p1: p }; }
                }
            }
            Tool::DraftDimAngle => {
                // 三點角度：頂點 → 第一邊端點 → 第二邊端點
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::ArcCenter { center: p };
                        self.console_push("INFO", "指定角度頂點".into());
                    }
                    DraftDrawState::ArcCenter { center } => {
                        self.editor.draft_state = DraftDrawState::AngleP1 { center, p1: p };
                        self.console_push("INFO", "指定第二邊端點".into());
                    }
                    DraftDrawState::AngleP1 { center, p1 } => {
                        let r = kolibri_drafting::DraftDocument::distance(&center, &p1)
                            .max(kolibri_drafting::DraftDocument::distance(&center, &p)) * 0.6;
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::DimAngle {
                            center, p1, p2: p, radius: r,
                        });
                        let a1 = kolibri_drafting::DraftDocument::angle(&center, &p1);
                        let a2 = kolibri_drafting::DraftDocument::angle(&center, &p);
                        let deg = ((a2 - a1).to_degrees()).abs();
                        self.console_push("ACTION", format!("角度標註: {:.1}°", deg));
                        self.editor.draft_state = DraftDrawState::Idle;
                    }
                    _ => { self.editor.draft_state = DraftDrawState::ArcCenter { center: p }; }
                }
            }
            Tool::DraftDimRadius => {
                // 點擊圓/弧圖元 → 自動偵測圓心與半徑
                let mut found_circle: Option<([f64; 2], f64)> = None;
                for obj in &self.editor.draft_doc.objects {
                    if !obj.visible { continue; }
                    match &obj.entity {
                        kolibri_drafting::DraftEntity::Circle { center, radius }
                        | kolibri_drafting::DraftEntity::Arc { center, radius, .. } => {
                            let d = (kolibri_drafting::DraftDocument::distance(center, &p) - radius).abs();
                            if d < 5.0 {
                                found_circle = Some((*center, *radius));
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if let Some((center, radius)) = found_circle {
                    let angle = kolibri_drafting::DraftDocument::angle(&center, &p);
                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::DimRadius {
                        center, radius, angle,
                    });
                    self.console_push("ACTION", format!("半徑標註: R{:.0}", radius));
                } else {
                    // Fallback: 手動兩點
                    match self.editor.draft_state.clone() {
                        DraftDrawState::Idle => {
                            self.editor.draft_state = DraftDrawState::CircleCenter { center: p };
                            self.console_push("INFO", "未偵測到圓/弧，請手動指定圓心".into());
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
            }
            Tool::DraftDimDiameter => {
                // 點擊圓圖元 → 自動偵測
                let mut found_circle: Option<([f64; 2], f64)> = None;
                for obj in &self.editor.draft_doc.objects {
                    if !obj.visible { continue; }
                    if let kolibri_drafting::DraftEntity::Circle { center, radius } = &obj.entity {
                        let d = (kolibri_drafting::DraftDocument::distance(center, &p) - radius).abs();
                        if d < 5.0 {
                            found_circle = Some((*center, *radius));
                            break;
                        }
                    }
                }
                if let Some((center, radius)) = found_circle {
                    let angle = kolibri_drafting::DraftDocument::angle(&center, &p);
                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::DimDiameter {
                        center, radius, angle,
                    });
                    self.console_push("ACTION", format!("直徑標註: ⌀{:.0}", radius * 2.0));
                } else {
                    match self.editor.draft_state.clone() {
                        DraftDrawState::Idle => {
                            self.editor.draft_state = DraftDrawState::CircleCenter { center: p };
                            self.console_push("INFO", "未偵測到圓，請手動指定圓心".into());
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
            }
            Tool::DraftText => {
                // 開啟文字編輯器對話框
                self.editor.draft_text_place = Some(p);
                self.editor.show_text_editor = true;
                if self.editor.draft_text_input.is_empty() {
                    self.editor.draft_text_input = "文字".into();
                }
                self.console_push("INFO", "輸入文字內容...".into());
            }
            Tool::DraftLeader => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::LeaderPoints { points: vec![p] };
                        self.console_push("INFO", "指定引線下一點（右鍵結束）".into());
                    }
                    DraftDrawState::LeaderPoints { mut points } => {
                        points.push(p);
                        if points.len() >= 2 {
                            // 2 點以上 → 開啟文字輸入
                            let pts = points.clone();
                            self.editor.draft_state = DraftDrawState::Idle;
                            // 使用文字編輯器讓使用者輸入引線文字
                            self.editor.draft_text_place = Some(*pts.last().unwrap());
                            self.editor.show_text_editor = true;
                            self.editor.draft_text_input = String::new();
                            // 暫存引線點，在文字確認後建立 Leader
                            // 因為我們沒有專用暫存，直接建立帶空文字的 Leader 再更新
                            self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Leader {
                                points: pts,
                                text: String::new(),
                            });
                            self.console_push("INFO", "輸入引線文字...".into());
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

            // ── 修剪 TRIM：點擊要裁掉的部分 ──
            Tool::DraftTrim => {
                // 找最近的線段圖元
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
                    // 收集所有其他圖元作為裁剪邊界
                    let cutting: Vec<kolibri_drafting::DraftEntity> = self.editor.draft_doc.objects.iter()
                        .filter(|o| o.id != id && o.visible)
                        .map(|o| o.entity.clone())
                        .collect();
                    let entity = self.editor.draft_doc.objects.iter().find(|o| o.id == id).map(|o| o.entity.clone());
                    if let Some(kolibri_drafting::DraftEntity::Line { start, end }) = entity {
                        if let Some(trimmed) = kolibri_drafting::geometry::trim_line_at_boundary(
                            &start, &end, &cutting, &p) {
                            self.editor.draft_doc.remove(id);
                            self.editor.draft_doc.add(trimmed);
                            self.console_push("ACTION", "修剪完成".into());
                        } else {
                            self.console_push("WARN", "找不到交點可修剪".into());
                        }
                    } else {
                        self.console_push("INFO", "修剪目前僅支援線段".into());
                    }
                }
            }

            // ── 延伸 EXTEND：點擊要延伸的線段 ──
            Tool::DraftExtend => {
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
                    let boundary: Vec<kolibri_drafting::DraftEntity> = self.editor.draft_doc.objects.iter()
                        .filter(|o| o.id != id && o.visible)
                        .map(|o| o.entity.clone())
                        .collect();
                    let entity = self.editor.draft_doc.objects.iter().find(|o| o.id == id).map(|o| o.entity.clone());
                    if let Some(kolibri_drafting::DraftEntity::Line { start, end }) = entity {
                        if let Some(extended) = kolibri_drafting::geometry::extend_line_to_boundary(
                            &start, &end, &boundary) {
                            self.editor.draft_doc.remove(id);
                            self.editor.draft_doc.add(extended);
                            self.console_push("ACTION", "延伸完成".into());
                        } else {
                            self.console_push("WARN", "找不到邊界可延伸".into());
                        }
                    } else {
                        self.console_push("INFO", "延伸目前僅支援線段".into());
                    }
                }
            }

            // ── 偏移 OFFSET：選圖元 → 點擊偏移方向 ──
            Tool::DraftOffset => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        // 先選取要偏移的圖元
                        let mut best_id = None;
                        let mut best_dist = 5.0_f64;
                        for obj in &self.editor.draft_doc.objects {
                            if !obj.visible { continue; }
                            let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                            if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                        }
                        if let Some(id) = best_id {
                            self.editor.draft_selected = vec![id];
                            self.editor.draft_state = DraftDrawState::LineFrom { p1: p };
                            self.console_push("INFO", format!("已選取圖元 #{}，點擊指定偏移方向", id));
                        }
                    }
                    DraftDrawState::LineFrom { p1: _ } => {
                        if let Some(&sel_id) = self.editor.draft_selected.first() {
                            let entity = self.editor.draft_doc.objects.iter().find(|o| o.id == sel_id).map(|o| o.entity.clone());
                            if let Some(ent) = entity {
                                // 計算偏移距離（用 click 點到圖元的距離）
                                let dist = self.draft_entity_distance(&ent, mm_x, mm_y);
                                // 判斷方向：正/負偏移
                                let offset_dist = if dist > 0.0 { dist } else { 5.0 };
                                if let Some(offset_ent) = kolibri_drafting::geometry::offset_entity(&ent, offset_dist) {
                                    self.editor.draft_doc.add(offset_ent);
                                    self.console_push("ACTION", format!("偏移 {:.1}mm", offset_dist));
                                }
                            }
                        }
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.editor.draft_selected.clear();
                    }
                    _ => { self.editor.draft_state = DraftDrawState::Idle; }
                }
            }

            // ── 圓角 FILLET：點擊第一條線 → 點擊第二條線 ──
            Tool::DraftFillet => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        let mut best_id = None;
                        let mut best_dist = 5.0_f64;
                        for obj in &self.editor.draft_doc.objects {
                            if !obj.visible { continue; }
                            let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                            if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                        }
                        if let Some(id) = best_id {
                            self.editor.draft_selected = vec![id];
                            self.editor.draft_state = DraftDrawState::DimP1 { p1: p };
                            self.console_push("INFO", "已選第一條線，點擊第二條線".into());
                        }
                    }
                    DraftDrawState::DimP1 { .. } => {
                        let mut best_id = None;
                        let mut best_dist = 5.0_f64;
                        for obj in &self.editor.draft_doc.objects {
                            if !obj.visible { continue; }
                            if self.editor.draft_selected.contains(&obj.id) { continue; }
                            let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                            if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                        }
                        if let (Some(id1), Some(id2)) = (self.editor.draft_selected.first().copied(), best_id) {
                            let e1 = self.editor.draft_doc.objects.iter().find(|o| o.id == id1).map(|o| o.entity.clone());
                            let e2 = self.editor.draft_doc.objects.iter().find(|o| o.id == id2).map(|o| o.entity.clone());
                            if let (Some(kolibri_drafting::DraftEntity::Line { start: a1, end: a2 }),
                                    Some(kolibri_drafting::DraftEntity::Line { start: b1, end: b2 })) = (e1, e2) {
                                let radius = self.editor.draft_fillet_radius;
                                if let Some((new_a, new_b, arc)) = kolibri_drafting::geometry::fillet_lines(&a1, &a2, &b1, &b2, radius) {
                                    self.editor.draft_doc.remove(id1);
                                    self.editor.draft_doc.remove(id2);
                                    self.editor.draft_doc.add(new_a);
                                    self.editor.draft_doc.add(new_b);
                                    self.editor.draft_doc.add(arc);
                                    self.console_push("ACTION", format!("圓角 R={:.0}", radius));
                                } else {
                                    self.console_push("WARN", "無法套用圓角（線段不相交）".into());
                                }
                            } else {
                                self.console_push("INFO", "圓角目前僅支援兩條線段".into());
                            }
                        }
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.editor.draft_selected.clear();
                    }
                    _ => { self.editor.draft_state = DraftDrawState::Idle; }
                }
            }

            // ── 倒角 CHAMFER：同 Fillet 但用斜線 ──
            Tool::DraftChamfer => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        let mut best_id = None;
                        let mut best_dist = 5.0_f64;
                        for obj in &self.editor.draft_doc.objects {
                            if !obj.visible { continue; }
                            let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                            if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                        }
                        if let Some(id) = best_id {
                            self.editor.draft_selected = vec![id];
                            self.editor.draft_state = DraftDrawState::DimP1 { p1: p };
                            self.console_push("INFO", "已選第一條線，點擊第二條線".into());
                        }
                    }
                    DraftDrawState::DimP1 { .. } => {
                        let mut best_id = None;
                        let mut best_dist = 5.0_f64;
                        for obj in &self.editor.draft_doc.objects {
                            if !obj.visible { continue; }
                            if self.editor.draft_selected.contains(&obj.id) { continue; }
                            let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                            if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                        }
                        if let (Some(id1), Some(id2)) = (self.editor.draft_selected.first().copied(), best_id) {
                            let e1 = self.editor.draft_doc.objects.iter().find(|o| o.id == id1).map(|o| o.entity.clone());
                            let e2 = self.editor.draft_doc.objects.iter().find(|o| o.id == id2).map(|o| o.entity.clone());
                            if let (Some(kolibri_drafting::DraftEntity::Line { start: a1, end: a2 }),
                                    Some(kolibri_drafting::DraftEntity::Line { start: b1, end: b2 })) = (e1, e2) {
                                let dist = self.editor.draft_chamfer_dist;
                                if let Some((new_a, new_b, chamfer)) = kolibri_drafting::geometry::chamfer_lines(&a1, &a2, &b1, &b2, dist, dist) {
                                    self.editor.draft_doc.remove(id1);
                                    self.editor.draft_doc.remove(id2);
                                    self.editor.draft_doc.add(new_a);
                                    self.editor.draft_doc.add(new_b);
                                    self.editor.draft_doc.add(chamfer);
                                    self.console_push("ACTION", format!("倒角 D={:.0}", dist));
                                } else {
                                    self.console_push("WARN", "無法套用倒角（線段不相交）".into());
                                }
                            } else {
                                self.console_push("INFO", "倒角目前僅支援兩條線段".into());
                            }
                        }
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.editor.draft_selected.clear();
                    }
                    _ => { self.editor.draft_state = DraftDrawState::Idle; }
                }
            }

            // ── 拉伸 STRETCH（選取→基點→目標點，移動選取端點）──
            Tool::DraftStretch => {
                if self.editor.draft_selected.is_empty() {
                    let mut best_id = None;
                    let mut best_dist = 5.0_f64;
                    for obj in &self.editor.draft_doc.objects {
                        if !obj.visible { continue; }
                        let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                        if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                    }
                    if let Some(id) = best_id {
                        self.editor.draft_selected = vec![id];
                        self.console_push("INFO", "已選取，點擊指定基點".into());
                    }
                } else if self.editor.draft_transform_base.is_none() {
                    self.editor.draft_transform_base = Some(p);
                    self.console_push("INFO", "指定拉伸目標點".into());
                } else if let Some(base) = self.editor.draft_transform_base {
                    let dx = p[0] - base[0];
                    let dy = p[1] - base[1];
                    // 對選取圖元做端點拉伸（移動離基點最近的端點）
                    let ids: Vec<_> = self.editor.draft_selected.clone();
                    for &id in &ids {
                        if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                            let stretched = self.stretch_draft_entity(&obj.entity, &base, dx, dy);
                            if let Some(obj_mut) = self.editor.draft_doc.get_mut(id) {
                                obj_mut.entity = stretched;
                            }
                        }
                    }
                    self.console_push("ACTION", format!("拉伸 Δ({:.0},{:.0})", dx, dy));
                    self.editor.draft_transform_base = None;
                    self.editor.draft_selected.clear();
                    self.editor.draft_state = DraftDrawState::Idle;
                }
            }

            // ── 連續標註（3-click 流程，自動連鎖）──
            Tool::DraftDimContinue | Tool::DraftDimBaseline => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::DimP1 { p1: p };
                        self.console_push("INFO", "指定第二點".into());
                    }
                    DraftDrawState::DimP1 { p1 } => {
                        self.editor.draft_state = DraftDrawState::DimP2 { p1, p2: p };
                        self.console_push("INFO", "拖曳放置標註線位置".into());
                    }
                    DraftDrawState::DimP2 { p1, p2 } => {
                        let offset = self.point_to_line_signed_dist(&p, &p1, &p2);
                        let entity = kolibri_drafting::DraftEntity::DimLinear {
                            p1, p2, offset, text_override: None,
                        };
                        self.editor.draft_doc.add(entity);
                        let dist = kolibri_drafting::DraftDocument::distance(&p1, &p2);
                        self.console_push("ACTION", format!("連續標註: {:.0}mm", dist));
                        // 連續模式：p2 → 下一個 p1，保留同一 offset 高度
                        self.editor.draft_state = DraftDrawState::DimP1 { p1: p2 };
                    }
                    _ => { self.editor.draft_state = DraftDrawState::DimP1 { p1: p }; }
                }
            }

            // ── 移動 MOVE（選取 → 基點 → 目標點）──
            Tool::DraftMove => {
                if self.editor.draft_selected.is_empty() {
                    // 先選取
                    let mut best_id = None;
                    let mut best_dist = 5.0_f64;
                    for obj in &self.editor.draft_doc.objects {
                        if !obj.visible { continue; }
                        let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                        if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                    }
                    if let Some(id) = best_id {
                        self.editor.draft_selected = vec![id];
                        self.console_push("INFO", "已選取，點擊指定基點".into());
                    }
                } else if self.editor.draft_transform_base.is_none() {
                    self.editor.draft_transform_base = Some(p);
                    self.console_push("INFO", "指定目標點".into());
                } else if let Some(base) = self.editor.draft_transform_base {
                    let dx = p[0] - base[0];
                    let dy = p[1] - base[1];
                    let ids: Vec<_> = self.editor.draft_selected.clone();
                    for &id in &ids {
                        if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                            let moved = kolibri_drafting::geometry::translate_entity(&obj.entity, dx, dy);
                            if let Some(obj_mut) = self.editor.draft_doc.get_mut(id) {
                                obj_mut.entity = moved;
                            }
                        }
                    }
                    self.console_push("ACTION", format!("移動 {} 個圖元 Δ({:.0},{:.0})", ids.len(), dx, dy));
                    self.editor.draft_transform_base = None;
                    self.editor.draft_selected.clear();
                    self.editor.draft_state = DraftDrawState::Idle;
                }
            }

            // ── 旋轉 ROTATE（選取 → 基點 → 角度點）──
            Tool::DraftRotate => {
                if self.editor.draft_selected.is_empty() {
                    let mut best_id = None;
                    let mut best_dist = 5.0_f64;
                    for obj in &self.editor.draft_doc.objects {
                        if !obj.visible { continue; }
                        let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                        if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                    }
                    if let Some(id) = best_id {
                        self.editor.draft_selected = vec![id];
                        self.console_push("INFO", "已選取，點擊指定基點".into());
                    }
                } else if self.editor.draft_transform_base.is_none() {
                    self.editor.draft_transform_base = Some(p);
                    self.console_push("INFO", "指定旋轉角度（點擊第二點）".into());
                } else if let Some(base) = self.editor.draft_transform_base {
                    let angle = kolibri_drafting::DraftDocument::angle(&base, &p);
                    let ids: Vec<_> = self.editor.draft_selected.clone();
                    for &id in &ids {
                        if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                            let rotated = self.rotate_draft_entity(&obj.entity, &base, angle);
                            if let Some(obj_mut) = self.editor.draft_doc.get_mut(id) {
                                obj_mut.entity = rotated;
                            }
                        }
                    }
                    self.console_push("ACTION", format!("旋轉 {} 個圖元 {:.1}°", ids.len(), angle.to_degrees()));
                    self.editor.draft_transform_base = None;
                    self.editor.draft_selected.clear();
                    self.editor.draft_state = DraftDrawState::Idle;
                }
            }

            // ── 比例 SCALE（選取 → 基點 → 比例點）──
            Tool::DraftScale => {
                if self.editor.draft_selected.is_empty() {
                    let mut best_id = None;
                    let mut best_dist = 5.0_f64;
                    for obj in &self.editor.draft_doc.objects {
                        if !obj.visible { continue; }
                        let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                        if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                    }
                    if let Some(id) = best_id {
                        self.editor.draft_selected = vec![id];
                        self.console_push("INFO", "已選取，點擊指定基點".into());
                    }
                } else if self.editor.draft_transform_base.is_none() {
                    self.editor.draft_transform_base = Some(p);
                    self.console_push("INFO", "指定比例因子（拖曳距離 = 比例）".into());
                } else if let Some(base) = self.editor.draft_transform_base {
                    let dist = kolibri_drafting::DraftDocument::distance(&base, &p);
                    let factor = (dist / 50.0).max(0.1).min(10.0); // 50mm = 1x
                    let ids: Vec<_> = self.editor.draft_selected.clone();
                    for &id in &ids {
                        if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                            let scaled = self.scale_draft_entity(&obj.entity, &base, factor);
                            if let Some(obj_mut) = self.editor.draft_doc.get_mut(id) {
                                obj_mut.entity = scaled;
                            }
                        }
                    }
                    self.console_push("ACTION", format!("比例 {} 個圖元 ×{:.2}", ids.len(), factor));
                    self.editor.draft_transform_base = None;
                    self.editor.draft_selected.clear();
                    self.editor.draft_state = DraftDrawState::Idle;
                }
            }

            // ── 鏡射 MIRROR（選取 → 軸線兩點）──
            Tool::DraftMirror => {
                if self.editor.draft_selected.is_empty() {
                    let mut best_id = None;
                    let mut best_dist = 5.0_f64;
                    for obj in &self.editor.draft_doc.objects {
                        if !obj.visible { continue; }
                        let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                        if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                    }
                    if let Some(id) = best_id {
                        self.editor.draft_selected = vec![id];
                        self.console_push("INFO", "已選取，點擊鏡射軸第一點".into());
                    }
                } else if self.editor.draft_transform_base.is_none() {
                    self.editor.draft_transform_base = Some(p);
                    self.console_push("INFO", "點擊鏡射軸第二點".into());
                } else if let Some(base) = self.editor.draft_transform_base {
                    let ids: Vec<_> = self.editor.draft_selected.clone();
                    for &id in &ids {
                        if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                            let mirrored = kolibri_drafting::geometry::mirror_entity(&obj.entity, &base, &p);
                            self.editor.draft_doc.add(mirrored);
                        }
                    }
                    self.console_push("ACTION", format!("鏡射 {} 個圖元", ids.len()));
                    self.editor.draft_transform_base = None;
                    self.editor.draft_selected.clear();
                    self.editor.draft_state = DraftDrawState::Idle;
                }
            }

            // ── 圖塊 BLOCK（選取圖元 → 定義圖塊）──
            Tool::DraftBlock => {
                if !self.editor.draft_selected.is_empty() {
                    let name = format!("Block{}", self.editor.draft_blocks.len() + 1);
                    let objs: Vec<kolibri_drafting::DraftObject> = self.editor.draft_selected.iter()
                        .filter_map(|&id| self.editor.draft_doc.objects.iter().find(|o| o.id == id).cloned())
                        .collect();
                    let count = objs.len();
                    self.editor.draft_blocks.insert(name.clone(), objs);
                    self.editor.draft_block_name = name.clone();
                    self.console_push("ACTION", format!("建立圖塊 '{}' ({} 個圖元)", name, count));
                    self.editor.draft_selected.clear();
                } else {
                    self.console_push("INFO", "請先選取要建立圖塊的圖元".into());
                }
            }
            // ── 插入 INSERT（點擊放置已定義的圖塊）──
            Tool::DraftInsert => {
                if self.editor.draft_blocks.is_empty() {
                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::BlockRef {
                        name: "Block1".into(),
                        insert_point: p,
                        scale: [1.0, 1.0],
                        rotation: 0.0,
                    });
                    self.console_push("ACTION", format!("插入空圖塊參考: ({:.0},{:.0})", p[0], p[1]));
                } else {
                    // 插入最後定義的圖塊（複製所有圖元，平移到插入點）
                    let block_name = self.editor.draft_block_name.clone();
                    if let Some(block_objs) = self.editor.draft_blocks.get(&block_name).cloned() {
                        // 計算圖塊重心
                        let mut cx = 0.0_f64;
                        let mut cy = 0.0_f64;
                        let mut n = 0;
                        for obj in &block_objs {
                            let grips = self.entity_grip_points(&obj.entity);
                            for gp in &grips {
                                cx += gp[0]; cy += gp[1]; n += 1;
                            }
                        }
                        if n > 0 { cx /= n as f64; cy /= n as f64; }
                        let dx = p[0] - cx;
                        let dy = p[1] - cy;
                        for obj in &block_objs {
                            let translated = kolibri_drafting::geometry::translate_entity(&obj.entity, dx, dy);
                            self.editor.draft_doc.add(translated);
                        }
                        self.console_push("ACTION", format!("插入圖塊 '{}' ({} 個圖元)", block_name, block_objs.len()));
                    }
                }
            }

            // ── 刪除 ERASE ──
            Tool::DraftErase => {
                if !self.editor.draft_selected.is_empty() {
                    let ids: Vec<_> = self.editor.draft_selected.drain(..).collect();
                    for id in &ids { self.editor.draft_doc.remove(*id); }
                    self.console_push("ACTION", format!("刪除 {} 個圖元", ids.len()));
                } else {
                    // 點擊選取後刪除
                    let mut best_id = None;
                    let mut best_dist = 5.0_f64;
                    for obj in &self.editor.draft_doc.objects {
                        if !obj.visible { continue; }
                        let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                        if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                    }
                    if let Some(id) = best_id {
                        self.editor.draft_doc.remove(id);
                        self.console_push("ACTION", "刪除 1 個圖元".into());
                    }
                }
            }

            // ── 打斷 BREAK（在點擊位置將線段一分為二）──
            Tool::DraftBreak => {
                let mut best_id = None;
                let mut best_dist = 5.0_f64;
                for obj in &self.editor.draft_doc.objects {
                    if !obj.visible { continue; }
                    let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                    if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                }
                if let Some(id) = best_id {
                    let entity = self.editor.draft_doc.objects.iter().find(|o| o.id == id).map(|o| o.entity.clone());
                    if let Some(kolibri_drafting::DraftEntity::Line { start, end }) = entity {
                        // 找最近點作為斷點
                        let bp = kolibri_drafting::geometry::point_to_line_nearest(&p, &start, &end);
                        // 斷成兩段
                        self.editor.draft_doc.remove(id);
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Line { start, end: bp });
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Line { start: bp, end });
                        self.console_push("ACTION", "打斷完成".into());
                    } else if let Some(kolibri_drafting::DraftEntity::Polyline { points, closed }) = entity {
                        // 找最近的線段，在該處斷開
                        let mut best_seg = 0;
                        let mut best_d = f64::MAX;
                        for i in 0..points.len().saturating_sub(1) {
                            let np = kolibri_drafting::geometry::point_to_line_nearest(&p, &points[i], &points[i+1]);
                            let d = kolibri_drafting::DraftDocument::distance(&p, &np);
                            if d < best_d { best_d = d; best_seg = i; }
                        }
                        self.editor.draft_doc.remove(id);
                        // 前半段
                        if best_seg > 0 {
                            let pts1: Vec<_> = points[..=best_seg].to_vec();
                            self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Polyline { points: pts1, closed: false });
                        }
                        // 後半段
                        if best_seg + 1 < points.len() {
                            let pts2: Vec<_> = points[best_seg+1..].to_vec();
                            if pts2.len() >= 2 {
                                self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Polyline { points: pts2, closed: false });
                            }
                        }
                        self.console_push("ACTION", "多段線打斷完成".into());
                    } else {
                        self.console_push("INFO", "打斷目前支援線段和多段線".into());
                    }
                }
            }

            // ── 接合 JOIN（合併選取的共線線段為多段線）──
            Tool::DraftJoin => {
                if self.editor.draft_selected.len() >= 2 {
                    // 收集所有選取線段的端點
                    let mut all_points: Vec<[f64; 2]> = Vec::new();
                    let ids: Vec<_> = self.editor.draft_selected.clone();
                    for &id in &ids {
                        if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                            match &obj.entity {
                                kolibri_drafting::DraftEntity::Line { start, end } => {
                                    if all_points.is_empty() {
                                        all_points.push(*start);
                                    }
                                    all_points.push(*end);
                                }
                                kolibri_drafting::DraftEntity::Polyline { points, .. } => {
                                    if all_points.is_empty() {
                                        all_points.extend_from_slice(points);
                                    } else {
                                        all_points.extend_from_slice(&points[..]);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    if all_points.len() >= 2 {
                        for &id in &ids { self.editor.draft_doc.remove(id); }
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Polyline {
                            points: all_points, closed: false,
                        });
                        self.console_push("ACTION", format!("接合 {} 個圖元為多段線", ids.len()));
                    }
                    self.editor.draft_selected.clear();
                } else {
                    // 先選取
                    let mut best_id = None;
                    let mut best_dist = 5.0_f64;
                    for obj in &self.editor.draft_doc.objects {
                        if !obj.visible { continue; }
                        let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                        if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                    }
                    if let Some(id) = best_id {
                        self.editor.draft_selected.push(id);
                        self.console_push("INFO", format!("已選取 {}，繼續選取要接合的圖元", self.editor.draft_selected.len()));
                    }
                }
            }

            // ── 修訂雲形 REVCLOUD ──
            Tool::DraftRevcloud => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::PolylinePoints { points: vec![p] };
                        self.console_push("INFO", "指定雲形邊界點（右鍵結束）".into());
                    }
                    DraftDrawState::PolylinePoints { mut points } => {
                        points.push(p);
                        self.editor.draft_state = DraftDrawState::PolylinePoints { points };
                    }
                    _ => { self.editor.draft_state = DraftDrawState::PolylinePoints { points: vec![p] }; }
                }
            }

            // ── 表格 TABLE ──
            Tool::DraftTable => {
                self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Table {
                    position: p,
                    rows: 3,
                    cols: 3,
                    row_height: 8.0,
                    col_width: 25.0,
                    cells: vec![String::new(); 9],
                });
                self.console_push("ACTION", format!("表格 3×3: ({:.0},{:.0})", p[0], p[1]));
            }

            // ── 兩點圓 ──
            Tool::DraftCircle2P => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::LineFrom { p1: p };
                        self.console_push("INFO", "指定直徑第二點".into());
                    }
                    DraftDrawState::LineFrom { p1 } => {
                        let center = [(p1[0] + p[0]) / 2.0, (p1[1] + p[1]) / 2.0];
                        let r = kolibri_drafting::DraftDocument::distance(&p1, &p) / 2.0;
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Circle { center, radius: r });
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.console_push("ACTION", format!("兩點圓: R={:.0}", r));
                    }
                    _ => { self.editor.draft_state = DraftDrawState::LineFrom { p1: p }; }
                }
            }

            // ── 三點圓 ──
            Tool::DraftCircle3P => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::PolylinePoints { points: vec![p] };
                        self.console_push("INFO", "指定第二點".into());
                    }
                    DraftDrawState::PolylinePoints { mut points } => {
                        points.push(p);
                        if points.len() >= 3 {
                            // 三點求圓
                            if let Some((cx, cy, r)) = three_point_circle(&points[0], &points[1], &points[2]) {
                                self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Circle {
                                    center: [cx, cy], radius: r,
                                });
                                self.console_push("ACTION", format!("三點圓: R={:.0}", r));
                            } else {
                                self.console_push("WARN", "三點共線，無法定義圓".into());
                            }
                            self.editor.draft_state = DraftDrawState::Idle;
                        } else {
                            let n = points.len() + 1;
                            self.editor.draft_state = DraftDrawState::PolylinePoints { points };
                            self.console_push("INFO", format!("指定第 {} 點", n));
                        }
                    }
                    _ => { self.editor.draft_state = DraftDrawState::PolylinePoints { points: vec![p] }; }
                }
            }

            // ── 三點弧 ──
            Tool::DraftArc3P => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::PolylinePoints { points: vec![p] };
                        self.console_push("INFO", "指定第二點".into());
                    }
                    DraftDrawState::PolylinePoints { mut points } => {
                        points.push(p);
                        if points.len() >= 3 {
                            if let Some((cx, cy, r)) = three_point_circle(&points[0], &points[1], &points[2]) {
                                let a1 = (points[0][1] - cy).atan2(points[0][0] - cx);
                                let a2 = (points[2][1] - cy).atan2(points[2][0] - cx);
                                self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Arc {
                                    center: [cx, cy], radius: r,
                                    start_angle: a1, end_angle: a2,
                                });
                                self.console_push("ACTION", format!("三點弧: R={:.0}", r));
                            } else {
                                self.console_push("WARN", "三點共線，無法定義弧".into());
                            }
                            self.editor.draft_state = DraftDrawState::Idle;
                        } else {
                            self.editor.draft_state = DraftDrawState::PolylinePoints { points };
                        }
                    }
                    _ => { self.editor.draft_state = DraftDrawState::PolylinePoints { points: vec![p] }; }
                }
            }

            // ── 起點-圓心-終點弧 ──
            Tool::DraftArcSCE => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::LineFrom { p1: p };
                        self.console_push("INFO", "指定圓心".into());
                    }
                    DraftDrawState::LineFrom { p1 } => {
                        // p1 = 起點, p = 圓心
                        self.editor.draft_state = DraftDrawState::ArcRadius {
                            center: p,
                            radius: kolibri_drafting::DraftDocument::distance(&p, &p1),
                        };
                        self.console_push("INFO", "指定終點".into());
                    }
                    DraftDrawState::ArcRadius { center, radius } => {
                        let start_angle = kolibri_drafting::DraftDocument::angle(&center, &self.editor.draft_transform_base.unwrap_or([center[0] + radius, center[1]]));
                        let end_angle = kolibri_drafting::DraftDocument::angle(&center, &p);
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Arc {
                            center, radius, start_angle, end_angle,
                        });
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.console_push("ACTION", format!("弧: R={:.0}", radius));
                    }
                    _ => { self.editor.draft_state = DraftDrawState::LineFrom { p1: p }; }
                }
            }

            // ── 測量距離 ──
            Tool::DraftMeasureDist => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::LineFrom { p1: p };
                        self.console_push("INFO", "指定第二點".into());
                    }
                    DraftDrawState::LineFrom { p1 } => {
                        let dist = kolibri_drafting::DraftDocument::distance(&p1, &p);
                        let dx = (p[0] - p1[0]).abs();
                        let dy = (p[1] - p1[1]).abs();
                        let angle = (p[1] - p1[1]).atan2(p[0] - p1[0]).to_degrees();
                        self.console_push("ACTION", format!("距離: {:.2}mm  ΔX: {:.2}  ΔY: {:.2}  角度: {:.1}°", dist, dx, dy, angle));
                        self.file_message = Some((format!("距離: {:.2}mm", dist), std::time::Instant::now()));
                        self.editor.draft_state = DraftDrawState::Idle;
                    }
                    _ => { self.editor.draft_state = DraftDrawState::LineFrom { p1: p }; }
                }
            }

            // ── 測量面積（多邊形點擊，右鍵結束）──
            Tool::DraftMeasureArea => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::PolylinePoints { points: vec![p] };
                        self.console_push("INFO", "指定下一點（右鍵計算面積）".into());
                    }
                    DraftDrawState::PolylinePoints { mut points } => {
                        points.push(p);
                        self.editor.draft_state = DraftDrawState::PolylinePoints { points };
                    }
                    _ => { self.editor.draft_state = DraftDrawState::PolylinePoints { points: vec![p] }; }
                }
            }

            // ── 格式刷 Match Properties ──
            Tool::DraftMatchProp => {
                if self.editor.draft_selected.is_empty() {
                    // 先選來源
                    let mut best_id = None;
                    let mut best_dist = 5.0_f64;
                    for obj in &self.editor.draft_doc.objects {
                        if !obj.visible { continue; }
                        let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                        if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                    }
                    if let Some(id) = best_id {
                        self.editor.draft_selected = vec![id];
                        self.console_push("INFO", "已選來源，點擊目標圖元套用格式".into());
                    }
                } else if let Some(&src_id) = self.editor.draft_selected.first() {
                    // 套用格式到目標
                    let src_props = self.editor.draft_doc.objects.iter()
                        .find(|o| o.id == src_id)
                        .map(|o| (o.color, o.line_type, o.line_weight, o.layer.clone()));
                    if let Some((color, lt, lw, layer)) = src_props {
                        let mut best_id = None;
                        let mut best_dist = 5.0_f64;
                        for obj in &self.editor.draft_doc.objects {
                            if !obj.visible { continue; }
                            let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                            if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                        }
                        if let Some(target_id) = best_id {
                            if let Some(obj) = self.editor.draft_doc.get_mut(target_id) {
                                obj.color = color;
                                obj.line_type = lt;
                                obj.line_weight = lw;
                                obj.layer = layer;
                            }
                            self.console_push("ACTION", "格式已套用".into());
                        }
                    }
                }
            }

            // ── 物件資訊 LIST ──
            Tool::DraftList => {
                let mut best_id = None;
                let mut best_dist = 5.0_f64;
                for obj in &self.editor.draft_doc.objects {
                    if !obj.visible { continue; }
                    let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                    if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                }
                if let Some(id) = best_id {
                    if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                        let info = format!("#{} | 圖層: {} | 顏色: {:?} | 線型: {:?} | 線寬: {:.2} | {:?}",
                            obj.id, obj.layer, obj.color, obj.line_type, obj.line_weight,
                            std::mem::discriminant(&obj.entity));
                        self.console_push("INFO", info);
                        self.viewer.show_console = true;
                    }
                }
            }

            // ── ID Point 座標 ──
            Tool::DraftIdPoint => {
                self.console_push("ACTION", format!("X: {:.4}  Y: {:.4}  Z: 0.0000", p[0], p[1]));
            }

            // ── 射線 RAY ──
            Tool::DraftRay => {
                match self.editor.draft_state.clone() {
                    DraftDrawState::Idle => {
                        self.editor.draft_state = DraftDrawState::LineFrom { p1: p };
                        self.console_push("INFO", "指定通過點".into());
                    }
                    DraftDrawState::LineFrom { p1 } => {
                        let dir = [p[0] - p1[0], p[1] - p1[1]];
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Xline { base: p1, direction: dir });
                        self.editor.draft_state = DraftDrawState::Idle;
                        self.console_push("ACTION", "射線".into());
                    }
                    _ => { self.editor.draft_state = DraftDrawState::LineFrom { p1: p }; }
                }
            }

            // ── 加長 LENGTHEN ──
            Tool::DraftLengthen => {
                let mut best_id = None;
                let mut best_dist = 5.0_f64;
                for obj in &self.editor.draft_doc.objects {
                    if !obj.visible { continue; }
                    let d = self.draft_entity_distance(&obj.entity, mm_x, mm_y);
                    if d < best_dist { best_dist = d; best_id = Some(obj.id); }
                }
                if let Some(id) = best_id {
                    let entity = self.editor.draft_doc.objects.iter().find(|o| o.id == id).map(|o| o.entity.clone());
                    if let Some(kolibri_drafting::DraftEntity::Line { start, end }) = entity {
                        let d_start = kolibri_drafting::DraftDocument::distance(&start, &p);
                        let d_end = kolibri_drafting::DraftDocument::distance(&end, &p);
                        let dx = end[0] - start[0];
                        let dy = end[1] - start[1];
                        let len = (dx*dx + dy*dy).sqrt();
                        let ext = len * 0.1;
                        let new_entity = if d_end < d_start {
                            kolibri_drafting::DraftEntity::Line { start, end: [end[0] + dx/len*ext, end[1] + dy/len*ext] }
                        } else {
                            kolibri_drafting::DraftEntity::Line { start: [start[0] - dx/len*ext, start[1] - dy/len*ext], end }
                        };
                        if let Some(obj) = self.editor.draft_doc.get_mut(id) { obj.entity = new_entity; }
                        self.console_push("ACTION", format!("加長 10%: {:.0}mm -> {:.0}mm", len, len * 1.1));
                    }
                }
            }

            // ── 中心標記 ──
            Tool::DraftCenterMark => {
                let mut found: Option<([f64;2], f64)> = None;
                for obj in &self.editor.draft_doc.objects {
                    if !obj.visible { continue; }
                    match &obj.entity {
                        kolibri_drafting::DraftEntity::Circle { center, radius } |
                        kolibri_drafting::DraftEntity::Arc { center, radius, .. } => {
                            let d = kolibri_drafting::DraftDocument::distance(center, &p);
                            if d < radius + 5.0 {
                                found = Some((*center, *radius));
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if let Some((c, r)) = found {
                    let ext = r * 0.15;
                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Line { start: [c[0]-r-ext, c[1]], end: [c[0]+r+ext, c[1]] });
                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Line { start: [c[0], c[1]-r-ext], end: [c[0], c[1]+r+ext] });
                    self.console_push("ACTION", format!("中心標記: ({:.0},{:.0})", c[0], c[1]));
                } else {
                    self.console_push("INFO", "請點擊圓或弧".into());
                }
            }

            // ── 矩形陣列 ──
            Tool::DraftArrayRect => {
                if !self.editor.draft_selected.is_empty() {
                    let ids: Vec<_> = self.editor.draft_selected.clone();
                    let rows = 3; let cols = 3;
                    let dx = 30.0_f64; let dy = 30.0_f64;
                    // 先收集所有要新增的 entity，避免借用衝突
                    let mut new_entities = Vec::new();
                    for &id in &ids {
                        if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                            let entity = obj.entity.clone();
                            for r in 0..rows {
                                for c in 0..cols {
                                    if r == 0 && c == 0 { continue; }
                                    new_entities.push(kolibri_drafting::geometry::translate_entity(
                                        &entity, c as f64 * dx, r as f64 * dy));
                                }
                            }
                        }
                    }
                    for e in new_entities {
                        self.editor.draft_doc.add(e);
                    }
                    self.console_push("ACTION", format!("矩形陣列 {}x{}", rows, cols));
                    self.editor.draft_selected.clear();
                } else {
                    self.console_push("INFO", "請先選取要陣列的圖元".into());
                }
            }

            // ── 環形陣列 ──
            Tool::DraftArrayPolar => {
                if !self.editor.draft_selected.is_empty() {
                    let ids: Vec<_> = self.editor.draft_selected.clone();
                    let count = 6;
                    let angle_step = std::f64::consts::TAU / count as f64;
                    // 先收集所有要新增的 entity，避免借用衝突
                    let mut new_entities = Vec::new();
                    for &id in &ids {
                        if let Some(obj) = self.editor.draft_doc.objects.iter().find(|o| o.id == id) {
                            let entity = obj.entity.clone();
                            let grips = self.entity_grip_points(&entity);
                            let cx = grips.iter().map(|g| g[0]).sum::<f64>() / grips.len().max(1) as f64;
                            let cy = grips.iter().map(|g| g[1]).sum::<f64>() / grips.len().max(1) as f64;
                            for i in 1..count {
                                let angle = angle_step * i as f64;
                                new_entities.push(self.rotate_draft_entity(&entity, &[cx, cy], angle));
                            }
                        }
                    }
                    for e in new_entities {
                        self.editor.draft_doc.add(e);
                    }
                    self.console_push("ACTION", format!("環形陣列 {} 個", count));
                    self.editor.draft_selected.clear();
                } else {
                    self.console_push("INFO", "請先選取要陣列的圖元".into());
                }
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
                if self.editor.tool == Tool::DraftMeasureArea {
                    // Shoelace formula 計算多邊形面積
                    if points.len() >= 3 {
                        let n = points.len();
                        let mut area = 0.0_f64;
                        for i in 0..n {
                            let j = (i + 1) % n;
                            area += points[i][0] * points[j][1];
                            area -= points[j][0] * points[i][1];
                        }
                        let area = area.abs() / 2.0;
                        // 周長
                        let mut perimeter = 0.0_f64;
                        for i in 0..n {
                            let j = (i + 1) % n;
                            let dx = points[j][0] - points[i][0];
                            let dy = points[j][1] - points[i][1];
                            perimeter += (dx*dx + dy*dy).sqrt();
                        }
                        self.console_push("ACTION", format!(
                            "面積: {:.2} mm²  ({:.4} m²)  周長: {:.2} mm  頂點: {}",
                            area, area / 1_000_000.0, perimeter, n));
                        self.file_message = Some((format!("面積: {:.2} mm²", area), std::time::Instant::now()));
                    } else {
                        self.console_push("WARN", "至少需要 3 個點才能計算面積".into());
                    }
                } else if points.len() >= 2 {
                    if self.editor.tool == Tool::DraftSpline {
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Spline {
                            points, closed: false,
                        });
                        self.console_push("ACTION", "雲形線完成".into());
                    } else if self.editor.tool == Tool::DraftRevcloud {
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Revcloud {
                            points, arc_radius: 3.0,
                        });
                        self.console_push("ACTION", "修訂雲形完成".into());
                    } else {
                        self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Polyline {
                            points, closed: false,
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
            kolibri_drafting::DraftEntity::Polygon { center, radius, sides, inscribed } => {
                kolibri_drafting::geometry::polygon_points(center, *radius, *sides, *inscribed)
            }
            kolibri_drafting::DraftEntity::Spline { points, .. } => points.clone(),
            kolibri_drafting::DraftEntity::DimLinear { p1, p2, .. }
            | kolibri_drafting::DraftEntity::DimAligned { p1, p2, .. } => vec![*p1, *p2],
            kolibri_drafting::DraftEntity::Hatch { boundary, .. } => boundary.clone(),
            _ => vec![],
        }
    }

    /// 旋轉圖元（圍繞基點）
    #[cfg(feature = "drafting")]
    fn rotate_draft_entity(&self, entity: &kolibri_drafting::DraftEntity, center: &[f64; 2], angle: f64) -> kolibri_drafting::DraftEntity {
        let rp = |p: &[f64; 2]| -> [f64; 2] {
            kolibri_drafting::geometry::rotate_point(p, center, angle)
        };
        match entity {
            kolibri_drafting::DraftEntity::Line { start, end } => {
                kolibri_drafting::DraftEntity::Line { start: rp(start), end: rp(end) }
            }
            kolibri_drafting::DraftEntity::Circle { center: c, radius } => {
                kolibri_drafting::DraftEntity::Circle { center: rp(c), radius: *radius }
            }
            kolibri_drafting::DraftEntity::Arc { center: c, radius, start_angle, end_angle } => {
                kolibri_drafting::DraftEntity::Arc {
                    center: rp(c), radius: *radius,
                    start_angle: start_angle + angle, end_angle: end_angle + angle,
                }
            }
            kolibri_drafting::DraftEntity::Rectangle { p1, p2 } => {
                // 旋轉後變成多段線
                let corners = [*p1, [p2[0], p1[1]], *p2, [p1[0], p2[1]]];
                let rotated: Vec<[f64; 2]> = corners.iter().map(|p| rp(p)).collect();
                kolibri_drafting::DraftEntity::Polyline { points: rotated, closed: true }
            }
            kolibri_drafting::DraftEntity::Polyline { points, closed } => {
                kolibri_drafting::DraftEntity::Polyline {
                    points: points.iter().map(|p| rp(p)).collect(), closed: *closed,
                }
            }
            kolibri_drafting::DraftEntity::Text { position, content, height, rotation } => {
                kolibri_drafting::DraftEntity::Text {
                    position: rp(position), content: content.clone(),
                    height: *height, rotation: rotation + angle,
                }
            }
            other => other.clone(),
        }
    }

    /// 縮放圖元（圍繞基點）
    #[cfg(feature = "drafting")]
    fn scale_draft_entity(&self, entity: &kolibri_drafting::DraftEntity, center: &[f64; 2], factor: f64) -> kolibri_drafting::DraftEntity {
        let sp = |p: &[f64; 2]| -> [f64; 2] {
            [center[0] + (p[0] - center[0]) * factor,
             center[1] + (p[1] - center[1]) * factor]
        };
        match entity {
            kolibri_drafting::DraftEntity::Line { start, end } => {
                kolibri_drafting::DraftEntity::Line { start: sp(start), end: sp(end) }
            }
            kolibri_drafting::DraftEntity::Circle { center: c, radius } => {
                kolibri_drafting::DraftEntity::Circle { center: sp(c), radius: radius * factor }
            }
            kolibri_drafting::DraftEntity::Arc { center: c, radius, start_angle, end_angle } => {
                kolibri_drafting::DraftEntity::Arc {
                    center: sp(c), radius: radius * factor,
                    start_angle: *start_angle, end_angle: *end_angle,
                }
            }
            kolibri_drafting::DraftEntity::Rectangle { p1, p2 } => {
                kolibri_drafting::DraftEntity::Rectangle { p1: sp(p1), p2: sp(p2) }
            }
            kolibri_drafting::DraftEntity::Polyline { points, closed } => {
                kolibri_drafting::DraftEntity::Polyline {
                    points: points.iter().map(|p| sp(p)).collect(), closed: *closed,
                }
            }
            kolibri_drafting::DraftEntity::Text { position, content, height, rotation } => {
                kolibri_drafting::DraftEntity::Text {
                    position: sp(position), content: content.clone(),
                    height: height * factor, rotation: *rotation,
                }
            }
            other => other.clone(),
        }
    }

    /// 繪製文字編輯器對話框（MTEXT）
    #[cfg(feature = "drafting")]
    pub(crate) fn draw_text_editor(&mut self, ctx: &egui::Context) {
        if !self.editor.show_text_editor { return; }

        let mut confirmed = false;
        let mut cancelled = false;
        egui::Window::new("文字編輯器 (MTEXT)")
            .default_size([300.0, 200.0])
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("字高:");
                    let mut h = self.editor.draft_text_height as f32;
                    ui.add(egui::DragValue::new(&mut h).range(1.0..=100.0).suffix(" mm").speed(0.5));
                    self.editor.draft_text_height = h as f64;
                });
                ui.separator();
                ui.label("文字內容:");
                ui.add(
                    egui::TextEdit::multiline(&mut self.editor.draft_text_input)
                        .desired_width(f32::INFINITY)
                        .desired_rows(5)
                        .font(egui::FontId::proportional(14.0))
                );
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("確定").clicked() {
                        confirmed = true;
                    }
                    if ui.button("取消").clicked() {
                        cancelled = true;
                    }
                });
            });

        if confirmed {
            if let Some(pos) = self.editor.draft_text_place {
                let content = self.editor.draft_text_input.clone();
                let height = self.editor.draft_text_height;
                if !content.is_empty() {
                    self.editor.draft_doc.add(kolibri_drafting::DraftEntity::Text {
                        position: pos,
                        content,
                        height,
                        rotation: 0.0,
                    });
                    self.console_push("ACTION", format!("文字: ({:.0},{:.0}) H={:.1}", pos[0], pos[1], height));
                }
            }
            self.editor.show_text_editor = false;
            self.editor.draft_text_place = None;
            self.editor.draft_text_input.clear();
        }
        if cancelled {
            self.editor.show_text_editor = false;
            self.editor.draft_text_place = None;
        }
    }

    /// 計算點到線段的帶號垂直距離（正 = 左側/上方，負 = 右側/下方）
    #[cfg(feature = "drafting")]
    fn point_to_line_signed_dist(&self, p: &[f64; 2], a: &[f64; 2], b: &[f64; 2]) -> f64 {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-9 { return 8.0; }
        // 法向量（左手邊）
        let nx = -dy / len;
        let ny = dx / len;
        // 點到線的帶號投影
        let dp = (p[0] - a[0]) * nx + (p[1] - a[1]) * ny;
        if dp.abs() < 2.0 { return if dp >= 0.0 { 8.0 } else { -8.0 }; }
        dp
    }

    /// 拉伸圖元（移動離基點最近的端點）
    #[cfg(feature = "drafting")]
    fn stretch_draft_entity(&self, entity: &kolibri_drafting::DraftEntity, base: &[f64; 2], dx: f64, dy: f64) -> kolibri_drafting::DraftEntity {
        match entity {
            kolibri_drafting::DraftEntity::Line { start, end } => {
                let d_start = ((start[0] - base[0]).powi(2) + (start[1] - base[1]).powi(2)).sqrt();
                let d_end = ((end[0] - base[0]).powi(2) + (end[1] - base[1]).powi(2)).sqrt();
                if d_start < d_end {
                    kolibri_drafting::DraftEntity::Line { start: [start[0] + dx, start[1] + dy], end: *end }
                } else {
                    kolibri_drafting::DraftEntity::Line { start: *start, end: [end[0] + dx, end[1] + dy] }
                }
            }
            kolibri_drafting::DraftEntity::Polyline { points, closed } => {
                let mut pts = points.clone();
                // 找最近的頂點
                if let Some((idx, _)) = pts.iter().enumerate()
                    .min_by(|(_, a), (_, b)| {
                        let da = ((a[0] - base[0]).powi(2) + (a[1] - base[1]).powi(2)).sqrt();
                        let db = ((b[0] - base[0]).powi(2) + (b[1] - base[1]).powi(2)).sqrt();
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    })
                {
                    pts[idx] = [pts[idx][0] + dx, pts[idx][1] + dy];
                }
                kolibri_drafting::DraftEntity::Polyline { points: pts, closed: *closed }
            }
            kolibri_drafting::DraftEntity::Rectangle { p1, p2 } => {
                let corners = [*p1, [p2[0], p1[1]], *p2, [p1[0], p2[1]]];
                let (idx, _) = corners.iter().enumerate()
                    .min_by(|(_, a), (_, b)| {
                        let da = ((a[0] - base[0]).powi(2) + (a[1] - base[1]).powi(2)).sqrt();
                        let db = ((b[0] - base[0]).powi(2) + (b[1] - base[1]).powi(2)).sqrt();
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap();
                let mut np1 = *p1;
                let mut np2 = *p2;
                match idx {
                    0 => { np1[0] += dx; np1[1] += dy; }
                    1 => { np2[0] += dx; np1[1] += dy; }
                    2 => { np2[0] += dx; np2[1] += dy; }
                    3 => { np1[0] += dx; np2[1] += dy; }
                    _ => {}
                }
                kolibri_drafting::DraftEntity::Rectangle { p1: np1, p2: np2 }
            }
            other => other.clone(),
        }
    }

    /// 游標中心縮放（ZWCAD 風格：以滑鼠位置為錨點，滾動越多縮放越快）
    #[cfg(feature = "drafting")]
    fn draft_zoom_at_cursor(&mut self, scroll: f32, cursor_pos: Option<egui::Pos2>, canvas_center: egui::Pos2) {
        // ZWCAD 風格：每格 1.2x，滾動量影響步階數
        let steps = (scroll / 50.0).clamp(-3.0, 3.0); // 正規化滾動量為 ±1~3 步
        let zoom_factor = (1.2_f32).powf(steps);
        let old_zoom = self.editor.draft_zoom;
        let new_zoom = (old_zoom * zoom_factor).clamp(0.01, 500.0);

        // 以游標位置為縮放錨點：
        // 原理：縮放前後，游標指向的 mm 座標不變
        // screen_pos = canvas_center + offset + mm * zoom
        // 所以 mm = (cursor - canvas_center - offset) / zoom
        // 縮放後要保持 cursor 指向同一個 mm 點：
        // new_offset = cursor - canvas_center - mm * new_zoom
        if let Some(cursor) = cursor_pos {
            let mm_x = (cursor.x - canvas_center.x - self.editor.draft_offset.x) / old_zoom;
            let mm_y = (cursor.y - canvas_center.y - self.editor.draft_offset.y) / old_zoom;
            self.editor.draft_offset.x = cursor.x - canvas_center.x - mm_x * new_zoom;
            self.editor.draft_offset.y = cursor.y - canvas_center.y - mm_y * new_zoom;
        }
        self.editor.draft_zoom = new_zoom;
    }

    /// 判斷 2D 圖元是否在可見區域內（frustum culling，大幅提升大場景效能）
    #[cfg(feature = "drafting")]
    fn entity_in_view(entity: &kolibri_drafting::DraftEntity, left: f64, right: f64, bottom: f64, top: f64) -> bool {
        // AABB 測試：取圖元 bounding box，與可見區域做交叉測試
        let (min_x, min_y, max_x, max_y) = match entity {
            kolibri_drafting::DraftEntity::Line { start, end } => {
                (start[0].min(end[0]), start[1].min(end[1]),
                 start[0].max(end[0]), start[1].max(end[1]))
            }
            kolibri_drafting::DraftEntity::Circle { center, radius } => {
                (center[0] - radius, center[1] - radius,
                 center[0] + radius, center[1] + radius)
            }
            kolibri_drafting::DraftEntity::Arc { center, radius, .. } => {
                (center[0] - radius, center[1] - radius,
                 center[0] + radius, center[1] + radius)
            }
            kolibri_drafting::DraftEntity::Rectangle { p1, p2 } => {
                (p1[0].min(p2[0]), p1[1].min(p2[1]),
                 p1[0].max(p2[0]), p1[1].max(p2[1]))
            }
            kolibri_drafting::DraftEntity::Polyline { points, .. } => {
                if points.is_empty() { return false; }
                let mut mnx = f64::MAX; let mut mny = f64::MAX;
                let mut mxx = f64::MIN; let mut mxy = f64::MIN;
                for p in points {
                    mnx = mnx.min(p[0]); mny = mny.min(p[1]);
                    mxx = mxx.max(p[0]); mxy = mxy.max(p[1]);
                }
                (mnx, mny, mxx, mxy)
            }
            kolibri_drafting::DraftEntity::Ellipse { center, semi_major, semi_minor, .. } => {
                let r = semi_major.max(*semi_minor);
                (center[0] - r, center[1] - r, center[0] + r, center[1] + r)
            }
            kolibri_drafting::DraftEntity::Text { position, height, .. } => {
                // 文字大致寬度估計
                (position[0], position[1] - height, position[0] + height * 10.0, position[1] + height)
            }
            kolibri_drafting::DraftEntity::DimLinear { p1, p2, offset, .. } => {
                let off = offset.abs();
                (p1[0].min(p2[0]) - off, p1[1].min(p2[1]) - off,
                 p1[0].max(p2[0]) + off, p1[1].max(p2[1]) + off)
            }
            kolibri_drafting::DraftEntity::DimAligned { p1, p2, offset, .. } => {
                let off = offset.abs();
                (p1[0].min(p2[0]) - off, p1[1].min(p2[1]) - off,
                 p1[0].max(p2[0]) + off, p1[1].max(p2[1]) + off)
            }
            kolibri_drafting::DraftEntity::Leader { points, .. } => {
                if points.is_empty() { return true; } // 有文字，保守顯示
                let mut mnx = f64::MAX; let mut mny = f64::MAX;
                let mut mxx = f64::MIN; let mut mxy = f64::MIN;
                for p in points {
                    mnx = mnx.min(p[0]); mny = mny.min(p[1]);
                    mxx = mxx.max(p[0]); mxy = mxy.max(p[1]);
                }
                (mnx, mny, mxx, mxy)
            }
            kolibri_drafting::DraftEntity::Point { position } => {
                (position[0] - 1.0, position[1] - 1.0, position[0] + 1.0, position[1] + 1.0)
            }
            // 其他不常見型別：保守顯示（不跳過）
            _ => return true,
        };
        // AABB 交叉測試：圖元 bbox 和可見區域有無重疊
        !(max_x < left || min_x > right || max_y < bottom || min_y > top)
    }

    /// 縮放至全部圖元（Zoom Extents）
    #[cfg(feature = "drafting")]
    fn draft_zoom_all(&mut self, canvas_rect: egui::Rect) {
        if self.editor.draft_doc.objects.is_empty() {
            // 沒有圖元，重置到預設
            self.editor.draft_zoom = 2.0;
            self.editor.draft_offset = egui::Vec2::ZERO;
            return;
        }
        // 計算所有圖元的 bounding box（mm 座標）
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        for obj in &self.editor.draft_doc.objects {
            if !obj.visible { continue; }
            let grips = self.entity_grip_points(&obj.entity);
            for gp in &grips {
                if gp[0] < min_x { min_x = gp[0]; }
                if gp[1] < min_y { min_y = gp[1]; }
                if gp[0] > max_x { max_x = gp[0]; }
                if gp[1] > max_y { max_y = gp[1]; }
            }
            // 圓/弧需要考慮半徑
            match &obj.entity {
                kolibri_drafting::DraftEntity::Circle { center, radius } => {
                    min_x = min_x.min(center[0] - radius);
                    min_y = min_y.min(center[1] - radius);
                    max_x = max_x.max(center[0] + radius);
                    max_y = max_y.max(center[1] + radius);
                }
                _ => {}
            }
        }
        if min_x >= max_x || min_y >= max_y {
            // 只有點狀圖元
            self.editor.draft_zoom = 2.0;
            self.editor.draft_offset = egui::Vec2::ZERO;
            return;
        }
        let extent_w = (max_x - min_x) as f32;
        let extent_h = (max_y - min_y) as f32;
        let center_mm_x = (min_x + max_x) / 2.0;
        let center_mm_y = (min_y + max_y) / 2.0;

        let canvas_w = canvas_rect.width();
        let canvas_h = canvas_rect.height();
        let margin = 0.85; // 留 15% 邊距

        let zoom_x = canvas_w * margin / extent_w.max(1.0);
        let zoom_y = canvas_h * margin / extent_h.max(1.0);
        let new_zoom = zoom_x.min(zoom_y).clamp(0.05, 200.0);

        self.editor.draft_zoom = new_zoom;
        // offset 使得 center_mm 映射到 canvas 中央
        // screen_center = canvas_center + offset + center_mm * zoom → offset = -center_mm * zoom
        self.editor.draft_offset = egui::vec2(
            -(center_mm_x as f32) * new_zoom,
            (center_mm_y as f32) * new_zoom, // Y 翻轉
        );
        self.console_push("INFO", format!(
            "縮放全部：{:.1}x（範圍 {:.0}×{:.0} mm）",
            new_zoom, extent_w, extent_h
        ));
    }
}

/// 三點求圓心和半徑
#[cfg(feature = "drafting")]
fn three_point_circle(p1: &[f64; 2], p2: &[f64; 2], p3: &[f64; 2]) -> Option<(f64, f64, f64)> {
    let ax = p1[0]; let ay = p1[1];
    let bx = p2[0]; let by = p2[1];
    let cx = p3[0]; let cy = p3[1];
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-10 { return None; }
    let ux = ((ax * ax + ay * ay) * (by - cy) + (bx * bx + by * by) * (cy - ay) + (cx * cx + cy * cy) * (ay - by)) / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx) + (bx * bx + by * by) * (ax - cx) + (cx * cx + cy * cy) * (bx - ax)) / d;
    let r = ((ax - ux).powi(2) + (ay - uy).powi(2)).sqrt();
    Some((ux, uy, r))
}

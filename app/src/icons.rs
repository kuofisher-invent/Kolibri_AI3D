//! Hand-drawn vector icons in Heroicons outline style.
//! Each function draws an icon into a given rect using egui's Painter.

use eframe::egui::{self, pos2, vec2, Color32, Painter, Pos2, Rect, Stroke};

fn s(rect: Rect, x: f32, y: f32) -> Pos2 {
    pos2(
        rect.min.x + x * rect.width(),
        rect.min.y + y * rect.height(),
    )
}

fn stroke(color: Color32) -> Stroke {
    Stroke::new(1.6, color)
}

// ─── Select (cursor arrow) ───────────────────────────────────────────────────

pub fn select(p: &Painter, r: Rect, c: Color32) {
    let pts = vec![
        s(r, 0.2, 0.1),
        s(r, 0.2, 0.8),
        s(r, 0.4, 0.62),
        s(r, 0.65, 0.85),
        s(r, 0.75, 0.78),
        s(r, 0.52, 0.55),
        s(r, 0.72, 0.42),
    ];
    p.add(egui::Shape::convex_polygon(pts, c.linear_multiply(0.15), stroke(c)));
}

// ─── Move (cross with arrows) ────────────────────────────────────────────────

pub fn move_tool(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    let cx = r.center();
    // Cross
    p.line_segment([s(r, 0.5, 0.15), s(r, 0.5, 0.85)], st);
    p.line_segment([s(r, 0.15, 0.5), s(r, 0.85, 0.5)], st);
    // Arrow heads
    let a = 0.08;
    // Up
    p.line_segment([s(r, 0.5, 0.15), s(r, 0.5-a, 0.15+a*1.5)], st);
    p.line_segment([s(r, 0.5, 0.15), s(r, 0.5+a, 0.15+a*1.5)], st);
    // Down
    p.line_segment([s(r, 0.5, 0.85), s(r, 0.5-a, 0.85-a*1.5)], st);
    p.line_segment([s(r, 0.5, 0.85), s(r, 0.5+a, 0.85-a*1.5)], st);
    // Left
    p.line_segment([s(r, 0.15, 0.5), s(r, 0.15+a*1.5, 0.5-a)], st);
    p.line_segment([s(r, 0.15, 0.5), s(r, 0.15+a*1.5, 0.5+a)], st);
    // Right
    p.line_segment([s(r, 0.85, 0.5), s(r, 0.85-a*1.5, 0.5-a)], st);
    p.line_segment([s(r, 0.85, 0.5), s(r, 0.85-a*1.5, 0.5+a)], st);
    let _ = cx;
}

// ─── Rotate (circular arrow) ─────────────────────────────────────────────────

pub fn rotate(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    let cx = r.center();
    let rad = r.width() * 0.32;
    // Arc (draw as segments)
    let segments = 12;
    for i in 0..segments {
        let a0 = -0.3 + (i as f32 / segments as f32) * 4.8;
        let a1 = -0.3 + ((i + 1) as f32 / segments as f32) * 4.8;
        p.line_segment([
            pos2(cx.x + rad * a0.cos(), cx.y + rad * a0.sin()),
            pos2(cx.x + rad * a1.cos(), cx.y + rad * a1.sin()),
        ], st);
    }
    // Arrow head at end
    let end_a: f32 = -0.3 + 4.8;
    let ex = cx.x + rad * end_a.cos();
    let ey = cx.y + rad * end_a.sin();
    let sz = r.width() * 0.12;
    p.line_segment([pos2(ex, ey), pos2(ex + sz, ey - sz*0.5)], st);
    p.line_segment([pos2(ex, ey), pos2(ex + sz*0.3, ey + sz)], st);
}

// ─── Scale (corner arrows) ───────────────────────────────────────────────────

pub fn scale(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Diagonal line
    p.line_segment([s(r, 0.25, 0.75), s(r, 0.75, 0.25)], st);
    // Top-right arrow
    p.line_segment([s(r, 0.75, 0.25), s(r, 0.55, 0.25)], st);
    p.line_segment([s(r, 0.75, 0.25), s(r, 0.75, 0.45)], st);
    // Bottom-left arrow
    p.line_segment([s(r, 0.25, 0.75), s(r, 0.45, 0.75)], st);
    p.line_segment([s(r, 0.25, 0.75), s(r, 0.25, 0.55)], st);
}

// ─── Line (pencil/line) ──────────────────────────────────────────────────────

pub fn line(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    p.line_segment([s(r, 0.2, 0.8), s(r, 0.8, 0.2)], st);
    p.circle_filled(s(r, 0.2, 0.8), 2.5, c);
    p.circle_filled(s(r, 0.8, 0.2), 2.5, c);
}

// ─── Arc ─────────────────────────────────────────────────────────────────────

pub fn arc(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    let segments = 10;
    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        let x0 = 0.15 + t0 * 0.7;
        let x1 = 0.15 + t1 * 0.7;
        let y0 = 0.75 - (t0 * std::f32::consts::PI).sin() * 0.55;
        let y1 = 0.75 - (t1 * std::f32::consts::PI).sin() * 0.55;
        p.line_segment([s(r, x0, y0), s(r, x1, y1)], st);
    }
    p.circle_filled(s(r, 0.15, 0.75), 2.5, c);
    p.circle_filled(s(r, 0.85, 0.75), 2.5, c);
}

// ─── Arc 3-Point (三點弧：弧線 + 3 個端點) ───────────────────────────────────

pub fn arc_3point(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    let segments = 10;
    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        let x0 = 0.15 + t0 * 0.7;
        let x1 = 0.15 + t1 * 0.7;
        let y0 = 0.75 - (t0 * std::f32::consts::PI).sin() * 0.55;
        let y1 = 0.75 - (t1 * std::f32::consts::PI).sin() * 0.55;
        p.line_segment([s(r, x0, y0), s(r, x1, y1)], st);
    }
    // 三個點（起點、終點、弧上中點）
    p.circle_filled(s(r, 0.15, 0.75), 3.0, c);
    p.circle_filled(s(r, 0.85, 0.75), 3.0, c);
    p.circle_filled(s(r, 0.50, 0.20), 3.0, c);
    // 小 "3" 標記
    let font = egui::FontId::proportional(8.0);
    p.text(s(r, 0.85, 0.30), egui::Align2::CENTER_CENTER, "3", font, c);
}

// ─── Rectangle ───────────────────────────────────────────────────────────────

pub fn rectangle(p: &Painter, r: Rect, c: Color32) {
    let inner = Rect::from_min_max(s(r, 0.15, 0.25), s(r, 0.85, 0.75));
    p.rect_stroke(inner, 1.0, stroke(c));
}

// ─── Circle ──────────────────────────────────────────────────────────────────

pub fn circle(p: &Painter, r: Rect, c: Color32) {
    p.circle_stroke(r.center(), r.width() * 0.35, stroke(c));
}

// ─── Box 3D (isometric cube) ─────────────────────────────────────────────────

pub fn box3d(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Front face
    let f = [s(r,0.18,0.4), s(r,0.62,0.4), s(r,0.62,0.85), s(r,0.18,0.85)];
    p.line_segment([f[0], f[1]], st);
    p.line_segment([f[1], f[2]], st);
    p.line_segment([f[2], f[3]], st);
    p.line_segment([f[3], f[0]], st);
    // Top face
    let t = [s(r,0.18,0.4), s(r,0.42,0.15), s(r,0.85,0.15), s(r,0.62,0.4)];
    p.line_segment([t[0], t[1]], st);
    p.line_segment([t[1], t[2]], st);
    p.line_segment([t[2], t[3]], st);
    // Right face
    p.line_segment([s(r,0.62,0.4), s(r,0.85,0.15)], st);
    p.line_segment([s(r,0.85,0.15), s(r,0.85,0.6)], st);
    p.line_segment([s(r,0.85,0.6), s(r,0.62,0.85)], st);
}

// ─── Cylinder ────────────────────────────────────────────────────────────────

pub fn cylinder(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    let cx = r.center().x;
    let rx = r.width() * 0.33;
    let ry = r.height() * 0.12;
    // Top ellipse
    let ty = r.min.y + r.height() * 0.22;
    draw_ellipse(p, pos2(cx, ty), rx, ry, st);
    // Bottom ellipse
    let by = r.min.y + r.height() * 0.78;
    draw_ellipse(p, pos2(cx, by), rx, ry, st);
    // Side lines
    p.line_segment([pos2(cx - rx, ty), pos2(cx - rx, by)], st);
    p.line_segment([pos2(cx + rx, ty), pos2(cx + rx, by)], st);
}

fn draw_ellipse(p: &Painter, center: Pos2, rx: f32, ry: f32, st: Stroke) {
    let n = 16;
    for i in 0..n {
        let a0 = (i as f32 / n as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / n as f32) * std::f32::consts::TAU;
        p.line_segment([
            pos2(center.x + rx * a0.cos(), center.y + ry * a0.sin()),
            pos2(center.x + rx * a1.cos(), center.y + ry * a1.sin()),
        ], st);
    }
}

// ─── Sphere ──────────────────────────────────────────────────────────────────

pub fn sphere(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    let cx = r.center();
    let rad = r.width() * 0.36;
    p.circle_stroke(cx, rad, st);
    // Horizontal ellipse
    draw_ellipse(p, cx, rad, rad * 0.35, Stroke::new(1.0, c.linear_multiply(0.5)));
    // Vertical ellipse
    draw_ellipse_v(p, cx, rad * 0.35, rad, Stroke::new(1.0, c.linear_multiply(0.5)));
}

fn draw_ellipse_v(p: &Painter, center: Pos2, rx: f32, ry: f32, st: Stroke) {
    draw_ellipse(p, center, rx, ry, st);
}

// ─── Push/Pull (up-down arrows with bar) ─────────────────────────────────────

pub fn push_pull(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Vertical line
    p.line_segment([s(r, 0.5, 0.15), s(r, 0.5, 0.85)], st);
    // Top arrow
    p.line_segment([s(r, 0.5, 0.15), s(r, 0.35, 0.3)], st);
    p.line_segment([s(r, 0.5, 0.15), s(r, 0.65, 0.3)], st);
    // Bottom arrow
    p.line_segment([s(r, 0.5, 0.85), s(r, 0.35, 0.7)], st);
    p.line_segment([s(r, 0.5, 0.85), s(r, 0.65, 0.7)], st);
    // Middle bar
    p.line_segment([s(r, 0.25, 0.5), s(r, 0.75, 0.5)], st);
}

// ─── Offset (parallel lines) ─────────────────────────────────────────────────

pub fn offset(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Inner rectangle
    p.rect_stroke(Rect::from_min_max(s(r, 0.3, 0.3), s(r, 0.7, 0.7)), 1.0, st);
    // Outer rectangle (dashed feel)
    p.rect_stroke(Rect::from_min_max(s(r, 0.12, 0.12), s(r, 0.88, 0.88)), 1.0,
        Stroke::new(1.0, c.linear_multiply(0.5)));
    // Arrow between
    p.line_segment([s(r, 0.7, 0.3), s(r, 0.88, 0.12)], Stroke::new(1.0, c.linear_multiply(0.5)));
}

// ─── Follow Me (arrow along path) ────────────────────────────────────────────

pub fn follow_me(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Path
    p.line_segment([s(r, 0.15, 0.7), s(r, 0.5, 0.7)], st);
    p.line_segment([s(r, 0.5, 0.7), s(r, 0.5, 0.3)], st);
    p.line_segment([s(r, 0.5, 0.3), s(r, 0.85, 0.3)], st);
    // Arrow head
    p.line_segment([s(r, 0.85, 0.3), s(r, 0.72, 0.2)], st);
    p.line_segment([s(r, 0.85, 0.3), s(r, 0.72, 0.4)], st);
}

// ─── Tape Measure (ruler) ────────────────────────────────────────────────────

pub fn tape_measure(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Diagonal ruler body
    p.line_segment([s(r, 0.15, 0.85), s(r, 0.85, 0.15)], st);
    // Tick marks
    for i in 1..5 {
        let t = i as f32 / 5.0;
        let x = 0.15 + t * 0.7;
        let y = 0.85 - t * 0.7;
        let dx = 0.06;
        p.line_segment([s(r, x-dx, y-dx), s(r, x+dx, y+dx)], Stroke::new(1.0, c));
    }
    // Endpoints
    p.circle_filled(s(r, 0.15, 0.85), 2.5, c);
    p.circle_filled(s(r, 0.85, 0.15), 2.5, c);
}

// ─── Paint Bucket ────────────────────────────────────────────────────────────

pub fn paint_bucket(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);

    // Paint droplet shape (teardrop) — Figma-style color fill icon
    let drop_pts = vec![
        s(r, 0.5, 0.12),   // top point
        s(r, 0.72, 0.45),  // right curve
        s(r, 0.68, 0.65),  // right bottom
        s(r, 0.5, 0.78),   // bottom
        s(r, 0.32, 0.65),  // left bottom
        s(r, 0.28, 0.45),  // left curve
    ];
    p.add(egui::Shape::convex_polygon(drop_pts, c.linear_multiply(0.25), st));

    // Inner fill (bottom half filled to show "paint level")
    let fill_pts = vec![
        s(r, 0.34, 0.55),
        s(r, 0.66, 0.55),
        s(r, 0.68, 0.65),
        s(r, 0.5, 0.78),
        s(r, 0.32, 0.65),
    ];
    p.add(egui::Shape::convex_polygon(fill_pts, c.linear_multiply(0.6), Stroke::NONE));
}

// ─── Orbit (eye/globe with arrow) ────────────────────────────────────────────

pub fn orbit(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    let cx = r.center();
    let rad = r.width() * 0.3;
    // Circle
    p.circle_stroke(cx, rad, st);
    // Orbital arrow around
    let segments = 8;
    let outer = rad * 1.4;
    for i in 0..segments {
        let a0 = 0.5 + (i as f32 / segments as f32) * 4.5;
        let a1 = 0.5 + ((i + 1) as f32 / segments as f32) * 4.5;
        p.line_segment([
            pos2(cx.x + outer * a0.cos(), cx.y + outer * 0.5 * a0.sin()),
            pos2(cx.x + outer * a1.cos(), cx.y + outer * 0.5 * a1.sin()),
        ], Stroke::new(1.2, c.linear_multiply(0.7)));
    }
}

// ─── Pan (hand) ──────────────────────────────────────────────────────────────

pub fn pan(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Palm
    p.rect_stroke(Rect::from_min_max(s(r, 0.25, 0.45), s(r, 0.78, 0.85)), 3.0, st);
    // Fingers
    let fingers = [0.3, 0.42, 0.54, 0.66];
    for &fx in &fingers {
        p.line_segment([s(r, fx, 0.45), s(r, fx, 0.2)], st);
        p.line_segment([s(r, fx, 0.2), s(r, fx+0.06, 0.18)], Stroke::new(1.0, c));
    }
}

// ─── Zoom Extents (fit to screen) ────────────────────────────────────────────

pub fn zoom_extents(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    let d = 0.22; // corner length
    // Top-left corner
    p.line_segment([s(r, 0.12, 0.12+d), s(r, 0.12, 0.12)], st);
    p.line_segment([s(r, 0.12, 0.12), s(r, 0.12+d, 0.12)], st);
    // Top-right corner
    p.line_segment([s(r, 0.88-d, 0.12), s(r, 0.88, 0.12)], st);
    p.line_segment([s(r, 0.88, 0.12), s(r, 0.88, 0.12+d)], st);
    // Bottom-left corner
    p.line_segment([s(r, 0.12, 0.88-d), s(r, 0.12, 0.88)], st);
    p.line_segment([s(r, 0.12, 0.88), s(r, 0.12+d, 0.88)], st);
    // Bottom-right corner
    p.line_segment([s(r, 0.88, 0.88-d), s(r, 0.88, 0.88)], st);
    p.line_segment([s(r, 0.88-d, 0.88), s(r, 0.88, 0.88)], st);
    // Inner box
    p.rect_stroke(Rect::from_min_max(s(r, 0.3, 0.3), s(r, 0.7, 0.7)), 1.0,
        Stroke::new(1.0, c.linear_multiply(0.4)));
}

// ─── Eraser (X mark) ─────────────────────────────────────────────────────────

pub fn eraser(p: &Painter, r: Rect, c: Color32) {
    let st = Stroke::new(2.0, c);
    p.line_segment([s(r, 0.22, 0.22), s(r, 0.78, 0.78)], st);
    p.line_segment([s(r, 0.78, 0.22), s(r, 0.22, 0.78)], st);
}

// ─── Dimension (two points with arrow and measurement) ──────────────────────

pub fn dimension(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Two endpoints
    p.circle_filled(s(r, 0.12, 0.65), 2.5, c);
    p.circle_filled(s(r, 0.88, 0.65), 2.5, c);
    // Arrow line between
    p.line_segment([s(r, 0.12, 0.65), s(r, 0.88, 0.65)], st);
    // Arrow heads
    p.line_segment([s(r, 0.12, 0.65), s(r, 0.22, 0.58)], st);
    p.line_segment([s(r, 0.12, 0.65), s(r, 0.22, 0.72)], st);
    p.line_segment([s(r, 0.88, 0.65), s(r, 0.78, 0.58)], st);
    p.line_segment([s(r, 0.88, 0.65), s(r, 0.78, 0.72)], st);
    // Extension lines (vertical)
    p.line_segment([s(r, 0.12, 0.55), s(r, 0.12, 0.78)], Stroke::new(1.0, c.linear_multiply(0.5)));
    p.line_segment([s(r, 0.88, 0.55), s(r, 0.88, 0.78)], Stroke::new(1.0, c.linear_multiply(0.5)));
    // Measurement label "120"
    p.text(s(r, 0.5, 0.35), egui::Align2::CENTER_CENTER,
        "120", egui::FontId { size: r.width() * 0.22, family: egui::FontFamily::Proportional }, c);
}

// ─── Text (letter "A" in frame) ─────────────────────────────────────────────

pub fn text_tool(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Frame
    p.rect_stroke(Rect::from_min_max(s(r, 0.15, 0.15), s(r, 0.85, 0.85)), 2.0,
        Stroke::new(1.0, c.linear_multiply(0.4)));
    // Letter "A"
    p.line_segment([s(r, 0.35, 0.75), s(r, 0.5, 0.25)], st);
    p.line_segment([s(r, 0.5, 0.25), s(r, 0.65, 0.75)], st);
    // Crossbar
    p.line_segment([s(r, 0.40, 0.55), s(r, 0.60, 0.55)], st);
}

// ─── Lookup by tool ──────────────────────────────────────────────────────────

use crate::app::Tool;

// ─── Group (overlapping rectangles) ──────────────────────────────────────────

pub fn group(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Back rect
    p.rect_stroke(Rect::from_min_max(s(r, 0.3, 0.15), s(r, 0.88, 0.65)), 2.0,
        Stroke::new(1.2, c.linear_multiply(0.5)));
    // Front rect
    p.rect_stroke(Rect::from_min_max(s(r, 0.12, 0.35), s(r, 0.7, 0.85)), 2.0, st);
}

// ─── Component (cube with diamond badge) ────────────────────────────────────

pub fn component(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Box outline
    p.rect_stroke(Rect::from_min_max(s(r, 0.15, 0.25), s(r, 0.75, 0.85)), 2.0, st);
    // Diamond badge top-right
    let cx = s(r, 0.72, 0.22);
    let sz = r.width() * 0.15;
    let diamond = vec![
        pos2(cx.x, cx.y - sz),
        pos2(cx.x + sz, cx.y),
        pos2(cx.x, cx.y + sz),
        pos2(cx.x - sz, cx.y),
    ];
    p.add(egui::Shape::convex_polygon(diamond, c.linear_multiply(0.3), st));
}

// ─── Steel Grid (cross axis lines) ──────────────────────────
pub fn steel_grid(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Vertical line
    p.line_segment([s(r, 0.5, 0.1), s(r, 0.5, 0.9)], st);
    // Horizontal line
    p.line_segment([s(r, 0.1, 0.5), s(r, 0.9, 0.5)], st);
    // Grid label bubbles
    p.circle_stroke(s(r, 0.5, 0.08), r.width() * 0.08, Stroke::new(1.0, c));
    p.circle_stroke(s(r, 0.08, 0.5), r.width() * 0.08, Stroke::new(1.0, c));
    // Tick marks
    for i in 1..4 {
        let t = i as f32 * 0.2 + 0.1;
        p.line_segment([s(r, t, 0.47), s(r, t, 0.53)], Stroke::new(0.8, c.linear_multiply(0.5)));
        p.line_segment([s(r, 0.47, t), s(r, 0.53, t)], Stroke::new(0.8, c.linear_multiply(0.5)));
    }
}

// ─── Steel Column (vertical H-beam) ─────────────────────────
pub fn steel_column(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Flanges (top and bottom horizontal bars)
    p.rect_stroke(Rect::from_min_max(s(r, 0.25, 0.1), s(r, 0.75, 0.18)), 1.0, st);
    p.rect_stroke(Rect::from_min_max(s(r, 0.25, 0.82), s(r, 0.75, 0.9)), 1.0, st);
    // Web (vertical center bar)
    p.rect_filled(Rect::from_min_max(s(r, 0.45, 0.18), s(r, 0.55, 0.82)), 0.0, c.linear_multiply(0.3));
    p.rect_stroke(Rect::from_min_max(s(r, 0.45, 0.18), s(r, 0.55, 0.82)), 0.0, st);
}

// ─── Steel Beam (horizontal H-beam) ─────────────────────────
pub fn steel_beam(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Flanges (left and right vertical bars)
    p.rect_stroke(Rect::from_min_max(s(r, 0.1, 0.25), s(r, 0.18, 0.75)), 1.0, st);
    p.rect_stroke(Rect::from_min_max(s(r, 0.82, 0.25), s(r, 0.9, 0.75)), 1.0, st);
    // Web (horizontal center bar)
    p.rect_filled(Rect::from_min_max(s(r, 0.18, 0.45), s(r, 0.82, 0.55)), 0.0, c.linear_multiply(0.3));
    p.rect_stroke(Rect::from_min_max(s(r, 0.18, 0.45), s(r, 0.82, 0.55)), 0.0, st);
}

// ─── Steel Brace (diagonal line) ────────────────────────────
pub fn steel_brace(p: &Painter, r: Rect, c: Color32) {
    let st = Stroke::new(2.0, c);
    // Diagonal brace
    p.line_segment([s(r, 0.15, 0.85), s(r, 0.85, 0.15)], st);
    // Connection dots
    p.circle_filled(s(r, 0.15, 0.85), 3.0, c);
    p.circle_filled(s(r, 0.85, 0.15), 3.0, c);
    // Cross brace (lighter)
    p.line_segment([s(r, 0.15, 0.15), s(r, 0.85, 0.85)], Stroke::new(1.0, c.linear_multiply(0.4)));
}

// ─── Steel Plate (flat rectangle with thickness) ────────────
pub fn steel_plate(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // 3D-ish plate
    let main = Rect::from_min_max(s(r, 0.15, 0.3), s(r, 0.75, 0.8));
    p.rect_filled(main, 1.0, c.linear_multiply(0.15));
    p.rect_stroke(main, 1.0, st);
    // Thickness edge (top)
    let top = [s(r, 0.15, 0.3), s(r, 0.3, 0.15), s(r, 0.88, 0.15), s(r, 0.75, 0.3)];
    p.add(egui::Shape::convex_polygon(top.to_vec(), c.linear_multiply(0.25), st));
    // Thickness edge (right)
    let right = [s(r, 0.75, 0.3), s(r, 0.88, 0.15), s(r, 0.88, 0.65), s(r, 0.75, 0.8)];
    p.add(egui::Shape::convex_polygon(right.to_vec(), c.linear_multiply(0.1), st));
}

// ─── Steel Connection (bolted joint) ────────────────────────
pub fn steel_connection(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    // Two meeting plates
    p.line_segment([s(r, 0.1, 0.5), s(r, 0.5, 0.5)], Stroke::new(2.0, c));
    p.line_segment([s(r, 0.5, 0.5), s(r, 0.9, 0.5)], Stroke::new(2.0, c));
    // Gusset plate (triangle)
    let plate = [s(r, 0.35, 0.3), s(r, 0.65, 0.3), s(r, 0.5, 0.5)];
    p.add(egui::Shape::convex_polygon(plate.to_vec(), c.linear_multiply(0.2), st));
    // Bolts (small circles)
    p.circle_filled(s(r, 0.4, 0.4), 2.5, c);
    p.circle_filled(s(r, 0.5, 0.35), 2.5, c);
    p.circle_filled(s(r, 0.6, 0.4), 2.5, c);
}

// ─── Pie (fan/sector) ────────────────────────────────────────────────────────

pub fn pie(p: &Painter, r: Rect, c: Color32) {
    let st = stroke(c);
    let cx = r.center();
    let rad = r.width() * 0.35;
    // Draw sector: center → arc → center
    let a_start = -0.3_f32;
    let a_end = 1.8_f32;
    let segments = 10;
    // Radius line 1
    p.line_segment([cx, pos2(cx.x + rad * a_start.cos(), cx.y + rad * a_start.sin())], st);
    // Arc
    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        let ang0 = a_start + (a_end - a_start) * t0;
        let ang1 = a_start + (a_end - a_start) * t1;
        p.line_segment([
            pos2(cx.x + rad * ang0.cos(), cx.y + rad * ang0.sin()),
            pos2(cx.x + rad * ang1.cos(), cx.y + rad * ang1.sin()),
        ], st);
    }
    // Radius line 2
    p.line_segment([cx, pos2(cx.x + rad * a_end.cos(), cx.y + rad * a_end.sin())], st);
    // Center dot
    p.circle_filled(cx, 2.0, c);
}

pub fn draw_tool_icon(p: &Painter, r: Rect, tool: Tool, color: Color32) {
    match tool {
        Tool::Select         => select(p, r, color),
        Tool::Move           => move_tool(p, r, color),
        Tool::Rotate         => rotate(p, r, color),
        Tool::Scale          => offset(p, r, color),  // nested rectangles = scale
        Tool::Line           => line(p, r, color),
        Tool::Arc            => arc(p, r, color),
        Tool::Arc3Point      => arc_3point(p, r, color),
        Tool::Pie            => pie(p, r, color),
        Tool::Rectangle      => rectangle(p, r, color),
        Tool::Circle         => circle(p, r, color),
        Tool::CreateBox      => box3d(p, r, color),
        Tool::CreateCylinder => cylinder(p, r, color),
        Tool::CreateSphere   => sphere(p, r, color),
        Tool::PushPull       => push_pull(p, r, color),
        Tool::Offset         => scale(p, r, color),  // diagonal arrows = offset
        Tool::FollowMe       => follow_me(p, r, color),
        Tool::TapeMeasure    => tape_measure(p, r, color),
        Tool::Dimension      => dimension(p, r, color),
        Tool::Text           => text_tool(p, r, color),
        Tool::PaintBucket    => paint_bucket(p, r, color),
        Tool::Orbit          => orbit(p, r, color),
        Tool::Pan            => pan(p, r, color),
        Tool::ZoomExtents    => zoom_extents(p, r, color),
        Tool::Group          => group(p, r, color),
        Tool::Component      => component(p, r, color),
        Tool::Eraser         => eraser(p, r, color),
        Tool::SteelGrid       => steel_grid(p, r, color),
        Tool::SteelColumn     => steel_column(p, r, color),
        Tool::SteelBeam       => steel_beam(p, r, color),
        Tool::SteelBrace      => steel_brace(p, r, color),
        Tool::SteelPlate      => steel_plate(p, r, color),
        Tool::SteelConnection => steel_connection(p, r, color),
        Tool::Wall => { // 牆圖示：矩形 + 門洞
            p.line_segment([egui::pos2(r.left()+2.0, r.bottom()-2.0), egui::pos2(r.left()+2.0, r.top()+2.0)], egui::Stroke::new(2.0, color));
            p.line_segment([egui::pos2(r.left()+2.0, r.top()+2.0), egui::pos2(r.right()-2.0, r.top()+2.0)], egui::Stroke::new(2.0, color));
            p.line_segment([egui::pos2(r.right()-2.0, r.top()+2.0), egui::pos2(r.right()-2.0, r.bottom()-2.0)], egui::Stroke::new(2.0, color));
        }
        Tool::Slab => { // 板圖示：水平矩形
            p.rect_stroke(r.shrink(3.0), 2.0, egui::Stroke::new(1.5, color));
            p.line_segment([egui::pos2(r.left()+3.0, r.center().y), egui::pos2(r.right()-3.0, r.center().y)], egui::Stroke::new(1.0, color));
        }
    }
}

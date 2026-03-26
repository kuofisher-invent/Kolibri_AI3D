//! 2D overlay 繪圖工具（虛線、圓弧計算）
//! 從 app.rs 拆分出來

use eframe::egui;

// ─── Arc geometry（真圓弧，非 Bezier 近似）──────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct ArcInfo {
    pub center: [f32; 3],
    pub radius: f32,
    pub start_angle: f32,
    pub end_angle: f32,
    pub normal: [f32; 3],
    pub u_axis: [f32; 3],
    pub v_axis: [f32; 3],
}

impl ArcInfo {
    pub fn sweep_angle(&self) -> f32 {
        let mut sweep = self.end_angle - self.start_angle;
        if sweep < 0.0 { sweep += std::f32::consts::TAU; }
        sweep
    }
    pub fn sweep_degrees(&self) -> f32 {
        self.sweep_angle().to_degrees()
    }
    pub fn arc_length(&self) -> f32 {
        self.radius * self.sweep_angle()
    }
    pub fn is_semicircle(&self) -> bool {
        let deg = self.sweep_degrees();
        deg > 170.0 && deg < 190.0
    }
    pub fn points(&self, segments: usize) -> Vec<[f32; 3]> {
        let sweep = self.sweep_angle();
        let mut pts = Vec::with_capacity(segments + 1);
        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let angle = self.start_angle + sweep * t;
            let (sin_a, cos_a) = angle.sin_cos();
            pts.push([
                self.center[0] + self.radius * (cos_a * self.u_axis[0] + sin_a * self.v_axis[0]),
                self.center[1] + self.radius * (cos_a * self.u_axis[1] + sin_a * self.v_axis[1]),
                self.center[2] + self.radius * (cos_a * self.u_axis[2] + sin_a * self.v_axis[2]),
            ]);
        }
        pts
    }
}

/// 從兩端點 + 凸度點計算真圓弧（circumscribed circle）
pub(crate) fn compute_arc_info(p1: [f32; 3], p2: [f32; 3], p3: [f32; 3]) -> Option<ArcInfo> {
    let a = glam::Vec3::from(p1);
    let b = glam::Vec3::from(p2);
    let c = glam::Vec3::from(p3);

    let ab = b - a;
    let ac = c - a;
    let normal = ab.cross(ac);
    if normal.length_squared() < 1e-6 {
        return None;
    }
    let normal = normal.normalize();

    let mid_ab = (a + b) * 0.5;
    let mid_ac = (a + c) * 0.5;
    let dir_ab = ab.cross(normal).normalize();
    let dir_ac = ac.cross(normal).normalize();

    let d = mid_ac - mid_ab;
    let denom = dir_ab.cross(dir_ac).length_squared();
    if denom < 1e-10 { return None; }
    let t1 = d.cross(dir_ac).dot(dir_ab.cross(dir_ac)) / denom;
    let center = mid_ab + dir_ab * t1;
    let radius = (a - center).length();

    let u_axis = (a - center).normalize();
    let v_axis = normal.cross(u_axis).normalize();

    let angle_of = |p: glam::Vec3| -> f32 {
        let d = p - center;
        let u = d.dot(u_axis);
        let v = d.dot(v_axis);
        v.atan2(u)
    };

    let angle_a = angle_of(a);
    let angle_b = angle_of(b);
    let angle_c = angle_of(c);

    let mut end_angle = angle_b - angle_a;
    let mut mid_check = angle_c - angle_a;
    if mid_check < 0.0 { mid_check += std::f32::consts::TAU; }
    if end_angle < 0.0 { end_angle += std::f32::consts::TAU; }

    if mid_check > end_angle {
        end_angle = end_angle - std::f32::consts::TAU;
    }

    Some(ArcInfo {
        center: center.into(),
        radius,
        start_angle: angle_a,
        end_angle: angle_a + end_angle,
        normal: normal.into(),
        u_axis: u_axis.into(),
        v_axis: v_axis.into(),
    })
}

/// 向下相容包裝：回傳點陣列
pub(crate) fn compute_arc(p1: [f32; 3], p2: [f32; 3], p3: [f32; 3], segments: usize) -> Vec<[f32; 3]> {
    if let Some(info) = compute_arc_info(p1, p2, p3) {
        info.points(segments)
    } else {
        vec![p1, p2]
    }
}

// ─── Dashed line helper ──────────────────────────────────────────────────────

pub(crate) fn draw_dashed_line(
    painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2,
    stroke: egui::Stroke, dash: f32, gap: f32,
) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 { return; }
    let nx = dx / len;
    let ny = dy / len;
    let mut t = 0.0;
    while t < len {
        let t1 = t;
        let t2 = (t + dash).min(len);
        painter.line_segment(
            [
                egui::pos2(from.x + nx * t1, from.y + ny * t1),
                egui::pos2(from.x + nx * t2, from.y + ny * t2),
            ],
            stroke,
        );
        t += dash + gap;
    }
}

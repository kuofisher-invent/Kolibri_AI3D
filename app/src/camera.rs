use glam::{Mat4, Vec3, Vec4};

#[derive(Clone)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::new(0.0, 500.0, 0.0),
            distance: 8000.0,
            yaw: 0.8,
            pitch: -0.5,
            fov: 45.0_f32.to_radians(),
            near: 10.0,
            far: 200_000.0,
        }
    }
}

impl OrbitCamera {
    pub fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.target + Vec3::new(cy * cp, -sp, sy * cp) * self.distance
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }

    pub fn proj(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov, aspect, self.near, self.far)
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.proj(aspect) * self.view()
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * 0.005;   // match SketchUp orbit direction
        self.pitch = (self.pitch - dy * 0.005).clamp(-1.5, 1.5); // drag up → see top (match SketchUp)
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let (sy, cy) = self.yaw.sin_cos();
        let right = Vec3::new(-sy, 0.0, cy);
        let up = Vec3::Y;
        let scale = self.distance * 0.0008;
        self.target += right * (dx * scale) + up * (dy * scale); // match SketchUp pan direction
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance *= (1.0 - delta * 0.001).clamp(0.5, 2.0);
        self.distance = self.distance.clamp(10.0, 200_000.0);
    }

    /// Zoom toward a world point (cursor-centered zoom like SketchUp)
    pub fn zoom_toward(&mut self, delta: f32, world_point: Option<Vec3>) {
        let old_distance = self.distance;
        let factor = (1.0 - delta * 0.002).clamp(0.3, 3.0);
        self.distance = (self.distance * factor).clamp(10.0, 200_000.0);

        // Move target toward the world point proportionally
        if let Some(wp) = world_point {
            let zoom_ratio = 1.0 - self.distance / old_distance;
            self.target = self.target + (wp - self.target) * zoom_ratio * 0.5;
        }
    }

    pub fn set_front(&mut self) {
        self.yaw = 0.0;
        self.pitch = 0.0;
    }
    pub fn set_back(&mut self) {
        self.yaw = std::f32::consts::PI;
        self.pitch = 0.0;
    }
    pub fn set_left(&mut self) {
        self.yaw = -std::f32::consts::FRAC_PI_2;
        self.pitch = 0.0;
    }
    pub fn set_right(&mut self) {
        self.yaw = std::f32::consts::FRAC_PI_2;
        self.pitch = 0.0;
    }
    pub fn set_top(&mut self) {
        self.yaw = 0.0;
        self.pitch = -std::f32::consts::FRAC_PI_2 + 0.001;
    }
    pub fn set_bottom(&mut self) {
        self.yaw = 0.0;
        self.pitch = std::f32::consts::FRAC_PI_2 - 0.001;
    }
    pub fn set_iso(&mut self) {
        self.yaw = 0.8;
        self.pitch = -0.5;
    }

    /// Linearly interpolate between two cameras. `t` should be in [0, 1].
    pub fn lerp(a: &OrbitCamera, b: &OrbitCamera, t: f32) -> OrbitCamera {
        let t = t.clamp(0.0, 1.0);
        OrbitCamera {
            target: a.target + (b.target - a.target) * t,
            distance: a.distance + (b.distance - a.distance) * t,
            yaw: a.yaw + (b.yaw - a.yaw) * t,
            pitch: a.pitch + (b.pitch - a.pitch) * t,
            fov: a.fov + (b.fov - a.fov) * t,
            near: a.near,
            far: a.far,
        }
    }

    // ── Walk mode ────────────────────────────────────────────────────────

    /// Move forward/backward on the XZ plane (first-person walk).
    pub fn walk_forward(&mut self, amount: f32) {
        let (sy, cy) = self.yaw.sin_cos();
        let forward = Vec3::new(-cy, 0.0, -sy);
        self.target += forward * amount;
    }

    /// Strafe left/right on the XZ plane (first-person walk).
    pub fn walk_strafe(&mut self, amount: f32) {
        let (sy, cy) = self.yaw.sin_cos();
        let right = Vec3::new(-sy, 0.0, cy);
        self.target += right * amount;
    }

    /// Free-look: rotate yaw/pitch without moving the target.
    pub fn look_around(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * 0.003;
        self.pitch = (self.pitch + dy * 0.003).clamp(-1.4, 1.4);
    }

    // ── Orthographic projection ──────────────────────────────────────────

    /// Orthographic projection matrix. `aspect` = width / height.
    pub fn proj_ortho(&self, aspect: f32) -> Mat4 {
        let half_h = self.distance * 0.5;
        let half_w = half_h * aspect;
        Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, self.near, self.far)
    }

    /// Unproject screen coords to world-space ray (origin, direction)
    pub fn screen_ray(&self, mx: f32, my: f32, vw: f32, vh: f32) -> (Vec3, Vec3) {
        let inv = self.view_proj(vw / vh).inverse();
        let ndc_x = 2.0 * mx / vw - 1.0;
        let ndc_y = 1.0 - 2.0 * my / vh;
        let near = inv * Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
        let far = inv * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
        let near = near.truncate() / near.w;
        let far = far.truncate() / far.w;
        (near, (far - near).normalize())
    }
}

/// Ray-AABB intersection (slab method), returns hit distance or None
pub fn ray_aabb(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let inv = Vec3::ONE / dir;
    let t1 = (min - origin) * inv;
    let t2 = (max - origin) * inv;
    let tmin = t1.min(t2);
    let tmax = t1.max(t2);
    let enter = tmin.x.max(tmin.y).max(tmin.z);
    let exit = tmax.x.min(tmax.y).min(tmax.z);
    if exit >= enter && exit >= 0.0 {
        Some(enter.max(0.0))
    } else {
        None
    }
}

/// Ray-plane intersection at Y=0, returns XZ position
pub fn ray_ground(origin: Vec3, dir: Vec3) -> Option<Vec3> {
    if dir.y.abs() < 1e-6 {
        return None;
    }
    let t = -origin.y / dir.y;
    if t < 0.0 {
        return None;
    }
    Some(origin + dir * t)
}

/// Project mouse ray onto a vertical axis at `base` to get height
pub fn ray_vertical_height(origin: Vec3, dir: Vec3, base: Vec3) -> f32 {
    let dx = origin.x - base.x;
    let dz = origin.z - base.z;
    let denom = dir.x * dir.x + dir.z * dir.z;
    if denom < 1e-6 { return 0.0; }
    let s = -(dx * dir.x + dz * dir.z) / denom;
    let hit = origin + dir * s.max(0.0);
    (hit.y - base.y).max(10.0)  // min 10mm
}

/// 同上但回傳帶正負號的 Y 座標（用於 Move Y 方向）
pub fn ray_vertical_y(origin: Vec3, dir: Vec3, base: Vec3) -> f32 {
    let dx = origin.x - base.x;
    let dz = origin.z - base.z;
    let denom = dir.x * dir.x + dir.z * dir.z;
    if denom < 1e-6 { return base.y; }
    let s = -(dx * dir.x + dz * dir.z) / denom;
    let hit = origin + dir * s.max(0.0);
    hit.y
}

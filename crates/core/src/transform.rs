//! 3D Transform utilities — 完整 4x4 矩陣操作
//! 為未來 SceneObject 支援完整 transform 做準備

use glam::{Mat4, Vec3, Quat};

/// 從平移 + Y 軸旋轉建立 transform（目前 SceneObject 的格式）
pub fn from_position_rotation_y(position: [f32; 3], rotation_y: f32) -> Mat4 {
    let translation = Mat4::from_translation(Vec3::from(position));
    let rotation = Mat4::from_rotation_y(rotation_y);
    translation * rotation
}

/// 從 4x4 column-major 陣列建立 Mat4
pub fn from_array(m: [f32; 16]) -> Mat4 {
    Mat4::from_cols_array(&m)
}

/// Mat4 轉 column-major 陣列
pub fn to_array(m: Mat4) -> [f32; 16] {
    m.to_cols_array()
}

/// 從 Mat4 提取平移
pub fn get_translation(m: Mat4) -> [f32; 3] {
    let t = m.col(3).truncate();
    [t.x, t.y, t.z]
}

/// 從 Mat4 提取 Y 軸旋轉角度（弧度）
pub fn get_rotation_y(m: Mat4) -> f32 {
    // 從旋轉矩陣提取 Y 軸旋轉
    let forward = m.col(2).truncate().normalize();
    forward.x.atan2(forward.z)
}

/// 從 Mat4 提取均勻縮放
pub fn get_uniform_scale(m: Mat4) -> f32 {
    m.col(0).truncate().length()
}

/// 組合兩個 transform
pub fn combine(parent: Mat4, child: Mat4) -> Mat4 {
    parent * child
}

/// Transform 一個 3D 點
pub fn transform_point(m: Mat4, point: [f32; 3]) -> [f32; 3] {
    let p = m.transform_point3(Vec3::from(point));
    [p.x, p.y, p.z]
}

/// Transform 一個方向（不含平移）
pub fn transform_direction(m: Mat4, dir: [f32; 3]) -> [f32; 3] {
    let d = m.transform_vector3(Vec3::from(dir));
    [d.x, d.y, d.z]
}

/// 建立 look-at transform（相機用）
pub fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> Mat4 {
    Mat4::look_at_rh(Vec3::from(eye), Vec3::from(target), Vec3::from(up))
}

/// 建立正交投影
pub fn ortho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    Mat4::orthographic_rh(left, right, bottom, top, near, far)
}

/// 建立透視投影
pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    Mat4::perspective_rh(fov_y, aspect, near, far)
}

/// 從 TRS 組件建立 transform
pub fn from_trs(translation: [f32; 3], rotation: [f32; 4], scale: [f32; 3]) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from(scale),
        Quat::from_array(rotation),
        Vec3::from(translation),
    )
}

/// 分解 transform 為 TRS
pub fn decompose(m: Mat4) -> ([f32; 3], [f32; 4], [f32; 3]) {
    let (scale, rotation, translation) = m.to_scale_rotation_translation();
    (translation.into(), rotation.into(), scale.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_trs() {
        let t = [100.0, 200.0, 300.0];
        let r = [0.0, 0.0, 0.0, 1.0]; // identity quaternion
        let s = [1.0, 1.0, 1.0];
        let m = from_trs(t, r, s);
        let (t2, r2, s2) = decompose(m);
        assert!((t2[0] - t[0]).abs() < 0.01);
        assert!((t2[1] - t[1]).abs() < 0.01);
        assert!((t2[2] - t[2]).abs() < 0.01);
        assert!((s2[0] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_transform_point() {
        let m = from_position_rotation_y([1000.0, 0.0, 0.0], 0.0);
        let p = transform_point(m, [0.0, 0.0, 0.0]);
        assert!((p[0] - 1000.0).abs() < 0.01);
    }
}

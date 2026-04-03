//! 旋轉數學工具 — 四元數版
//! 使用 glam::Quat 避免萬向鎖，直接公轉 position + 四元數合成

use glam::{Quat, Vec3};

/// 從 SceneObject 的三個旋轉欄位取得有效四元數
/// 優先用 rotation_quat；若為 identity 則從 euler/legacy 建構
pub(crate) fn effective_quat(rot_quat: [f32; 4], rot_xyz: [f32; 3], rot_y_legacy: f32) -> [f32; 4] {
    let q = Quat::from_array(rot_quat);
    if !q.is_near_identity() {
        return rot_quat;
    }
    // Fallback: 從 euler angles 建構（Ry * Rx * Rz 順序，與 mesh_builder 一致）
    let [rx, ry, rz] = rot_xyz;
    let use_y_only = rx.abs() < 1e-6 && rz.abs() < 1e-6;
    let eff_ry = if use_y_only {
        if ry.abs() > 1e-6 { ry } else { rot_y_legacy }
    } else {
        ry
    };
    if rx.abs() < 1e-6 && eff_ry.abs() < 1e-6 && rz.abs() < 1e-6 {
        return [0.0, 0.0, 0.0, 1.0]; // identity
    }
    let qy = Quat::from_rotation_y(eff_ry);
    let qx = Quat::from_rotation_x(rx);
    let qz = Quat::from_rotation_z(rz);
    (qy * qx * qz).normalize().to_array()
}

/// 四元數 → 歐拉角 [rx, ry, rz]（Ry * Rx * Rz 順序分解）
/// 用於同步 rotation_xyz / rotation_y legacy 欄位
pub(crate) fn quat_to_euler(q: [f32; 4]) -> [f32; 3] {
    let q = Quat::from_array(q);
    let mat = glam::Mat3::from_quat(q);
    // R = Ry * Rx * Rz:
    //   R[1][2] = -sin(rx)
    //   R[0][2] = sin(ry)*cos(rx)
    //   R[2][2] = cos(ry)*cos(rx)
    //   R[1][0] = cos(rx)*sin(rz)
    //   R[1][1] = cos(rx)*cos(rz)
    let cols = [mat.x_axis, mat.y_axis, mat.z_axis]; // column-major
    // mat.col(j)[i] = R[i][j]
    let r12 = cols[2].y; // R[1][2]
    let sx = -r12;
    let rx = sx.clamp(-1.0, 1.0).asin();
    let cx = rx.cos();
    let (ry, rz);
    if cx.abs() > 1e-6 {
        ry = cols[2].x.atan2(cols[2].z); // atan2(R[0][2], R[2][2])
        rz = cols[0].y.atan2(cols[1].y); // atan2(R[1][0], R[1][1])
    } else {
        ry = (-cols[0].z).atan2(cols[0].x); // atan2(-R[2][0], R[0][0])
        rz = 0.0;
    }
    let mut result = [rx, ry, rz];
    for v in &mut result {
        if v.abs() < 1e-6 { *v = 0.0; }
    }
    result
}

/// 從 Shape 取得半尺寸（mesh_builder 旋轉中心偏移）
pub(crate) fn shape_half_dims(shape: &kolibri_core::scene::Shape) -> [f32; 3] {
    use kolibri_core::scene::Shape;
    match shape {
        Shape::Box { width, height, depth } => [*width / 2.0, *height / 2.0, *depth / 2.0],
        Shape::Cylinder { height, .. } => [0.0, *height / 2.0, 0.0],
        Shape::Sphere { radius, .. } => [*radius, *radius, *radius],
        _ => [0.0, 0.0, 0.0],
    }
}

/// 公轉旋轉（四元數版）：
/// 1. 物件幾何中心（position + half_dims）繞旋轉盤中心公轉
/// 2. 四元數合成 q_orbit * q_current
/// 3. new_pos = new_center - half_dims
///
/// half_dims: 物件半尺寸，mesh_builder 旋轉中心 = position + half_dims
/// 回傳 (new_position, new_rotation_quat)
pub(crate) fn orbit_object(
    orig_pos: [f32; 3],
    orig_quat: [f32; 4],
    half_dims: [f32; 3],
    center: [f32; 3],
    axis: u8,
    delta: f32,
) -> ([f32; 3], [f32; 4]) {
    let axis_vec = match axis {
        0 => Vec3::X,
        2 => Vec3::Z,
        _ => Vec3::Y,
    };
    let q_orbit = Quat::from_axis_angle(axis_vec, delta);
    let half = Vec3::from_array(half_dims);

    // 1. 公轉物件幾何中心（position + half）
    let obj_center = Vec3::from_array(orig_pos) + half;
    let d = obj_center - Vec3::from_array(center);
    let rd = q_orbit * d;
    let new_center = Vec3::from_array(center) + rd;
    let new_pos = new_center - half;

    // 2. 旋轉：四元數合成（無萬向鎖）
    let q_current = Quat::from_array(orig_quat);
    let q_new = (q_orbit * q_current).normalize();

    (new_pos.to_array(), q_new.to_array())
}

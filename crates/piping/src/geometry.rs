//! 管線幾何產生器
//! 用 HeMesh 產生任意方向的圓柱管段和管件

use kolibri_core::halfedge::HeMesh;
use kolibri_core::scene::{Scene, Shape, MaterialKind};
use crate::pipe_data::{PipeSpec, PipeSystem, FittingKind};

/// 管線系統 → MaterialKind 對應
fn pipe_material(system: PipeSystem) -> MaterialKind {
    match system {
        PipeSystem::PvcWater => MaterialKind::White,
        PipeSystem::PvcDrain => MaterialKind::Plaster,
        PipeSystem::ElectricalConduit => MaterialKind::Aluminum,
        PipeSystem::IronFireSprinkler => MaterialKind::Steel,
        PipeSystem::SteelProcess => MaterialKind::Metal,
        PipeSystem::StainlessSteel => MaterialKind::Aluminum,
        PipeSystem::Copper => MaterialKind::Copper,
    }
}

/// 產生任意方向的圓柱 HeMesh（起點到終點）
fn make_oriented_cylinder(start: [f32; 3], end: [f32; 3], radius: f32, segments: u32) -> HeMesh {
    let mut mesh = HeMesh::new();
    let seg = segments.max(8) as usize;

    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let dz = end[2] - start[2];
    let length = (dx * dx + dy * dy + dz * dz).sqrt();
    if length < 0.1 { return mesh; }

    // 管軸方向（單位向量）
    let ax = [dx / length, dy / length, dz / length];

    // 找一個不平行的向量來計算切線
    let up = if ax[1].abs() < 0.9 { [0.0_f32, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    // 叉積 ax × up = tangent1
    let t1 = cross(ax, up);
    let t1_len = vec_len(t1);
    let t1 = [t1[0] / t1_len, t1[1] / t1_len, t1[2] / t1_len];
    // 叉積 ax × t1 = tangent2
    let t2 = cross(ax, t1);

    // 產生頂點：兩圈圓（起點圈 + 終點圈）
    // 座標相對於起點（mesh position = start）
    let mut bottom_vids = Vec::with_capacity(seg);
    let mut top_vids = Vec::with_capacity(seg);

    for i in 0..seg {
        let angle = std::f32::consts::TAU * i as f32 / seg as f32;
        let (sin_a, cos_a) = angle.sin_cos();

        // 圓上的偏移 = cos * t1 * r + sin * t2 * r
        let cx = cos_a * t1[0] * radius + sin_a * t2[0] * radius;
        let cy = cos_a * t1[1] * radius + sin_a * t2[1] * radius;
        let cz = cos_a * t1[2] * radius + sin_a * t2[2] * radius;

        // 底圈（相對起點 = 0,0,0）
        let bv = mesh.add_vertex([cx, cy, cz]);
        bottom_vids.push(bv);

        // 頂圈（相對起點 + 管軸方向 * 長度）
        let tv = mesh.add_vertex([dx + cx, dy + cy, dz + cz]);
        top_vids.push(tv);
    }

    // 產生面：側面四邊形
    for i in 0..seg {
        let j = (i + 1) % seg;
        // 順序：b[i], b[j], t[j], t[i]（CCW from outside）
        mesh.add_face(&[bottom_vids[i], bottom_vids[j], top_vids[j], top_vids[i]]);
    }

    // 底面（圓盤）
    let mut bottom_ring: Vec<u32> = bottom_vids.iter().rev().copied().collect();
    mesh.add_face(&bottom_ring);

    // 頂面（圓盤）
    mesh.add_face(&top_vids);

    mesh
}

/// 產生 90° 彎頭的 HeMesh
fn make_elbow_90(position: [f32; 3], radius: f32, pipe_radius: f32, segments: u32) -> HeMesh {
    let mut mesh = HeMesh::new();
    let seg = segments.max(8) as usize;
    let bend_seg = 8; // 彎頭弧段數

    // 彎頭沿 XZ 平面彎曲，中心在 position
    // 從 -X 方向進入，往 +Z 方向出去（90° 彎）
    let bend_r = radius * 1.5; // 彎曲半徑

    let mut rings: Vec<Vec<u32>> = Vec::new();

    for b in 0..=bend_seg {
        let bend_angle = std::f32::consts::FRAC_PI_2 * b as f32 / bend_seg as f32;
        // 彎弧中心點
        let center_x = -bend_r * bend_angle.cos() + bend_r;
        let center_z = bend_r * bend_angle.sin();

        // 該截面的軸向（切線方向）
        let ax_x = bend_angle.sin();
        let ax_z = bend_angle.cos();

        let mut ring = Vec::with_capacity(seg);
        for i in 0..seg {
            let a = std::f32::consts::TAU * i as f32 / seg as f32;
            let (sin_a, cos_a) = a.sin_cos();
            // 圓上偏移：Y 方向 + 軸垂直方向
            let px = center_x + cos_a * (-ax_z) * pipe_radius;
            let py = sin_a * pipe_radius;
            let pz = center_z + cos_a * ax_x * pipe_radius;

            let vid = mesh.add_vertex([px, py, pz]);
            ring.push(vid);
        }
        rings.push(ring);
    }

    // 連接相鄰圈為側面
    for b in 0..bend_seg {
        for i in 0..seg {
            let j = (i + 1) % seg;
            mesh.add_face(&[rings[b][i], rings[b][j], rings[b + 1][j], rings[b + 1][i]]);
        }
    }

    // 兩端蓋面
    let first: Vec<u32> = rings[0].iter().rev().copied().collect();
    mesh.add_face(&first);
    mesh.add_face(&rings[bend_seg]);

    mesh
}

/// 建立直管段（任意方向圓柱）
pub fn create_pipe_segment(
    scene: &mut Scene,
    spec: &PipeSpec,
    start: [f32; 3],
    end: [f32; 3],
    name: String,
) -> String {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let dz = end[2] - start[2];
    let length = (dx * dx + dy * dy + dz * dz).sqrt();
    if length < 1.0 { return String::new(); }

    let radius = spec.outer_diameter / 2.0;
    let mat = pipe_material(spec.system);
    let segments = if radius > 40.0 { 24 } else { 16 };

    let mesh = make_oriented_cylinder(
        [0.0, 0.0, 0.0],
        [dx, dy, dz],
        radius,
        segments,
    );

    let id = scene.insert_mesh_raw(name, start, mesh, mat);
    if let Some(obj) = scene.objects.get_mut(&id) {
        // IFC 屬性（待 core 支援後啟用）
        // obj.ifc_class = "IfcPipeSegment".to_string();
        // obj.ifc_system = spec.system.label().to_string();
        // obj.ifc_material_name = spec.spec_name.clone();
        obj.tag = format!("管線:{}", spec.system.label());
    }
    scene.version += 1;
    id
}

/// 建立管件（彎頭、三通、閥門等）
pub fn create_fitting(
    scene: &mut Scene,
    kind: FittingKind,
    spec: &PipeSpec,
    position: [f32; 3],
    name: String,
) -> String {
    let r = spec.outer_diameter / 2.0;
    let mat = pipe_material(spec.system);
    let segments = if r > 40.0 { 24 } else { 16 };

    let mesh = match kind {
        FittingKind::Elbow90 => {
            make_elbow_90([0.0, 0.0, 0.0], r, r, segments)
        }
        FittingKind::Elbow45 => {
            // 45° 用較短的 elbow 近似
            make_elbow_90([0.0, 0.0, 0.0], r, r * 0.7, segments)
        }
        FittingKind::Tee => {
            // T 形：主管 + 分支管
            let mut m = make_oriented_cylinder([0.0, 0.0, 0.0], [r * 6.0, 0.0, 0.0], r, segments);
            let branch = make_oriented_cylinder([r * 3.0, 0.0, 0.0], [r * 3.0, r * 3.0, 0.0], r, segments);
            // 合併
            merge_mesh(&mut m, &branch);
            m
        }
        FittingKind::Valve => {
            // 閥門：粗圓柱 + 手輪（細圓柱）
            let mut m = make_oriented_cylinder([0.0, 0.0, 0.0], [r * 4.0, 0.0, 0.0], r * 1.3, segments);
            let handle = make_oriented_cylinder([r * 2.0, 0.0, 0.0], [r * 2.0, r * 3.0, 0.0], r * 0.4, 8);
            merge_mesh(&mut m, &handle);
            m
        }
        FittingKind::Reducer => {
            // 大小頭：錐形近似（兩端不同半徑）
            make_oriented_cylinder([0.0, 0.0, 0.0], [r * 3.0, 0.0, 0.0], r, segments)
        }
        _ => {
            // 其他管件用短圓柱
            let fitting_len = r * 2.5;
            make_oriented_cylinder([0.0, 0.0, 0.0], [fitting_len, 0.0, 0.0], r * 1.2, segments)
        }
    };

    let id = scene.insert_mesh_raw(name, position, mesh, mat);
    if let Some(obj) = scene.objects.get_mut(&id) {
        // IFC 屬性（待 core 支援後啟用）
        // obj.ifc_class = "IfcPipeFitting".to_string();
        // obj.ifc_system = spec.system.label().to_string();
        // obj.ifc_material_name = format!("{} {}", kind.label(), spec.spec_name);
        obj.tag = format!("管件:{}", spec.system.label());
    }
    scene.version += 1;
    id
}

// ── 向量工具 ──

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn vec_len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// 合併兩個 mesh（將 src 的頂點和面加入 dst）
fn merge_mesh(dst: &mut HeMesh, src: &HeMesh) {
    // 建立 src VId → dst VId 的對應
    let mut vid_map = std::collections::HashMap::new();
    for (&vid, vert) in &src.vertices {
        let new_vid = dst.add_vertex(vert.pos);
        vid_map.insert(vid, new_vid);
    }
    for (_, face) in &src.faces {
        if let Some(ref vids) = face.vert_ids {
            let mapped: Vec<u32> = vids.iter().filter_map(|v| vid_map.get(v).copied()).collect();
            if mapped.len() >= 3 {
                dst.add_face(&mapped);
            }
        }
    }
}

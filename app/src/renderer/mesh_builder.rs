use eframe::wgpu;
use crate::scene::{Scene, Shape, SceneObject};
use crate::texture_manager::TextureManager;
use glam::Mat4;
use super::shaders::*;
use super::primitives::*;

/// 計算 Shape 的半尺寸（幾何中心 = position + half_size）
pub fn shape_half_size(shape: &Shape) -> [f32; 3] {
    match shape {
        Shape::Box { width, height, depth } => [*width / 2.0, *height / 2.0, *depth / 2.0],
        Shape::Cylinder { radius, height, .. } => [*radius, *height / 2.0, *radius],
        Shape::Sphere { radius, .. } => [*radius, *radius, *radius],
        Shape::Line { .. } => [0.0, 0.0, 0.0],
        Shape::Mesh(ref mesh) => {
            let (min, max) = mesh.aabb();
            [(max[0]-min[0])/2.0, (max[1]-min[1])/2.0, (max[2]-min[2])/2.0]
        }
        Shape::SteelProfile { params, length, .. } => [params.b / 2.0, *length / 2.0, params.h / 2.0],
    }
}

/// 單一物件的快取資料（面 + 邊線分離）
pub(crate) struct ObjMeshCache {
    pub obj_version: u64,
    pub lod_bucket: u8,
    /// 面（三角形）頂點 + indices
    pub face_verts: Vec<Vertex>,
    pub face_idx: Vec<u32>,
    /// 邊線（LineList）頂點：每條邊 2 個頂點，無 index
    pub edge_verts: Vec<Vertex>,
}

/// 場景 mesh 建構結果（面 + 邊線分離）
pub(crate) struct SceneMeshResult {
    pub face_verts: Vec<Vertex>,
    pub face_idx: Vec<u32>,
    pub edge_verts: Vec<Vertex>,  // LineList: 每 2 個頂點一條邊
}

/// 增量建構場景 mesh：只重建版本變更的物件
pub(crate) fn build_scene_mesh_incremental(
    scene: &Scene,
    per_obj_cache: &mut std::collections::HashMap<String, ObjMeshCache>,
    selected_ids: &[String],
    hovered: Option<&str>,
    editing_group_id: Option<&str>,
    editing_component_def_id: Option<&str>,
    hovered_face: Option<(&str, u8)>,
    selected_face: Option<(&str, u8)>,
    edge_thickness_param: f32,
    render_mode: u32,
    texture_manager: &TextureManager,
    view_proj: glam::Mat4,
) -> SceneMeshResult {
    // 移除已刪除物件的快取
    per_obj_cache.retain(|id, _| scene.objects.contains_key(id));

    // 更新每個物件的快取（含 LOD 級別）
    let total_objs = scene.objects.len();
    for obj in scene.objects.values() {
        if !obj.visible {
            per_obj_cache.remove(&obj.id);
            continue;
        }
        // 計算 LOD bucket（依相機距離）
        let mesh_center = if let Shape::Mesh(ref mesh) = obj.shape {
            let (mn, mx) = mesh.aabb();
            glam::Vec3::new(
                (mn[0] + mx[0]) * 0.5 + obj.position[0],
                (mn[1] + mx[1]) * 0.5 + obj.position[1],
                (mn[2] + mx[2]) * 0.5 + obj.position[2],
            )
        } else {
            glam::Vec3::from(obj.position)
        };
        let clip = view_proj * glam::Vec4::new(mesh_center.x, mesh_center.y, mesh_center.z, 1.0);
        let screen_px = if clip.w > 0.0 { 500.0 / clip.w } else { 100.0 };
        let lod = if total_objs > 500 {
            if screen_px > 200.0 { 0 } else if screen_px > 30.0 { 1 } else { 2 }
        } else if total_objs > 100 {
            if screen_px > 50.0 { 0 } else { 1 }
        } else { 0 };

        let needs_rebuild = match per_obj_cache.get(&obj.id) {
            Some(cached) => cached.obj_version != obj.obj_version || cached.lod_bucket != lod,
            None => true,
        };
        if needs_rebuild {
            let result = build_single_object_mesh(obj, edge_thickness_param, render_mode, texture_manager, view_proj, total_objs);
            per_obj_cache.insert(obj.id.clone(), ObjMeshCache {
                obj_version: obj.obj_version,
                lod_bucket: lod,
                face_verts: result.face_verts,
                face_idx: result.face_idx,
                edge_verts: result.edge_verts,
            });
        }
    }

    // 預估總大小
    let total_face_verts: usize = per_obj_cache.values().map(|c| c.face_verts.len()).sum();
    let total_face_idx: usize = per_obj_cache.values().map(|c| c.face_idx.len()).sum();
    let total_edge_verts: usize = per_obj_cache.values().map(|c| c.edge_verts.len()).sum();
    let mut face_verts = Vec::with_capacity(total_face_verts + 2048);
    let mut face_idx = Vec::with_capacity(total_face_idx + 2048);
    let mut edge_verts = Vec::with_capacity(total_edge_verts + 2048);

    // 合併所有物件的 mesh — 快速路徑：無著色修改的物件用 bulk copy
    let sel_set: std::collections::HashSet<&str> = selected_ids.iter().map(|s| s.as_str()).collect();
    let has_editing = editing_group_id.is_some() || editing_component_def_id.is_some();

    let big_scene = scene.objects.len() > 100;

    for obj in scene.objects.values() {
        if !obj.visible { continue; }
        let Some(cached) = per_obj_cache.get(&obj.id) else { continue; };
        if cached.face_verts.is_empty() && cached.edge_verts.is_empty() { continue; }

        // ── 視錐剔除（合併階段）──
        if big_scene {
            let mesh_center = if let Shape::Mesh(ref mesh) = obj.shape {
                let (mn, mx) = mesh.aabb();
                glam::Vec3::new(
                    (mn[0] + mx[0]) * 0.5 + obj.position[0],
                    (mn[1] + mx[1]) * 0.5 + obj.position[1],
                    (mn[2] + mx[2]) * 0.5 + obj.position[2],
                )
            } else {
                glam::Vec3::from(obj.position)
            };
            let clip = view_proj * glam::Vec4::new(mesh_center.x, mesh_center.y, mesh_center.z, 1.0);
            if clip.w > 0.0 {
                let ndc_x = (clip.x / clip.w).abs();
                let ndc_y = (clip.y / clip.w).abs();
                if ndc_x > 2.0 || ndc_y > 2.0 { continue; }
            }
        }

        let is_selected = sel_set.contains(obj.id.as_str());
        let is_hovered = Some(obj.id.as_str()) == hovered;
        let needs_color_mod = is_selected || is_hovered || has_editing;

        // ── 面（triangles）──
        let face_base = face_verts.len() as u32;
        if !needs_color_mod {
            face_verts.extend_from_slice(&cached.face_verts);
        } else {
            let is_dimmed = editing_group_id.map_or(false, |gid| obj.id != gid)
                || editing_component_def_id.map_or(false, |did| obj.component_def_id.as_deref() != Some(did));
            for v in &cached.face_verts {
                let mut color = v.color;
                if is_selected {
                    color[0] = color[0] * 0.45 + 0.11;
                    color[1] = color[1] * 0.45 + 0.33;
                    color[2] = color[2] * 0.45 + 0.55;
                } else if is_hovered {
                    color[0] = (color[0] + 0.15).min(1.0);
                    color[1] = (color[1] + 0.15).min(1.0);
                    color[2] = (color[2] + 0.15).min(1.0);
                }
                if is_dimmed { color[0] *= 0.3; color[1] *= 0.3; color[2] *= 0.3; color[3] *= 0.3; }
                face_verts.push(Vertex { position: v.position, normal: v.normal, color });
            }
        }
        face_idx.extend(cached.face_idx.iter().map(|i| i + face_base));

        // ── 邊線（LineList）──
        edge_verts.extend_from_slice(&cached.edge_verts);

        // Selection outline
        if is_selected {
            build_selection_outline(obj, &mut face_verts, &mut face_idx, edge_thickness_param);
        }
    }

    // Free mesh
    build_free_mesh(scene, editing_group_id, editing_component_def_id, &mut face_verts, &mut face_idx, &mut edge_verts);

    SceneMeshResult { face_verts, face_idx, edge_verts }
}

/// Public wrapper for per-object GPU upload
pub(crate) fn build_single_object_mesh_pub(
    obj: &SceneObject,
    edge_thickness_param: f32,
    render_mode: u32,
    texture_manager: &TextureManager,
    view_proj: glam::Mat4,
    total_scene_objects: usize,
) -> SceneMeshResult {
    let cache = build_single_object_mesh(obj, edge_thickness_param, render_mode, texture_manager, view_proj, total_scene_objects);
    SceneMeshResult { face_verts: cache.face_verts, face_idx: cache.face_idx, edge_verts: cache.edge_verts }
}

/// 建構單一物件的基礎 mesh（面 + 邊線分離）
fn build_single_object_mesh(
    obj: &SceneObject,
    _edge_thickness_param: f32,
    render_mode: u32,
    texture_manager: &TextureManager,
    view_proj: glam::Mat4,
    total_scene_objects: usize,
) -> ObjMeshCache {
    let mut face_verts = Vec::new();
    let mut face_idx = Vec::new();
    let mut edge_verts = Vec::new();
    let mut color = if let Some(ref tex_path) = obj.texture_path {
        if texture_manager.is_loaded(tex_path) {
            texture_manager.average_color(tex_path)
        } else { obj.material.color() }
    } else { obj.material.color() };

    // SketchUp-style: 面預設不透明（alpha=1.0），只有真正半透明材質（Glass）才用低 alpha
    // 不再用 alpha 編碼 roughness（那會導致半透明渲染）
    if color[3] >= 0.9 {
        color[3] = 1.0; // 不透明
    }
    // Glass 等材質保留原始 alpha（0.3-0.6）

    let p = obj.position;

    // 視錐剔除：用 mesh AABB 中心而非 obj.position（頂點已 bake 到 world space）
    let mesh_center = if let Shape::Mesh(ref mesh) = obj.shape {
        let (mn, mx) = mesh.aabb();
        [
            (mn[0] + mx[0]) * 0.5 + p[0],
            (mn[1] + mx[1]) * 0.5 + p[1],
            (mn[2] + mx[2]) * 0.5 + p[2],
        ]
    } else { p };
    {
        let center = glam::Vec3::from(mesh_center);
        let clip = view_proj * glam::Vec4::new(center.x, center.y, center.z, 1.0);
        if clip.w > 0.0 {
            let ndc_x = (clip.x / clip.w).abs();
            let ndc_y = (clip.y / clip.w).abs();
            if ndc_x > 2.5 || ndc_y > 2.5 {
                return ObjMeshCache { obj_version: 0, lod_bucket: 0, face_verts, face_idx, edge_verts };
            }
        }
    }

    // LOD segments
    let lod_segments = |base_segs: u32| -> u32 {
        let center = glam::Vec3::from(p);
        let clip = view_proj * glam::Vec4::new(center.x, center.y, center.z, 1.0);
        if clip.w > 0.0 {
            let screen_size = 500.0 / clip.w;
            if screen_size < 20.0 { return (base_segs / 4).max(6); }
            if screen_size < 80.0 { return (base_segs / 2).max(8); }
        }
        base_segs
    };

    // LOD: 計算螢幕投影大小，太小的物件跳過邊線
    let screen_size = {
        let center = glam::Vec3::from(mesh_center);
        let clip = view_proj * glam::Vec4::new(center.x, center.y, center.z, 1.0);
        if clip.w > 0.0 { 500.0 / clip.w } else { 100.0 }
    };

    let edge_color = if render_mode == 5 { [0.0, 0.0, 0.0, 1.0] } else { [0.35, 0.35, 0.35, 1.0] };

    match &obj.shape {
        Shape::Box { width, height, depth } =>
            push_box(&mut face_verts, &mut face_idx, p, *width, *height, *depth, color),
        Shape::Cylinder { radius, height, segments } =>
            push_cylinder(&mut face_verts, &mut face_idx, p, *radius, *height, lod_segments(*segments), color),
        Shape::Sphere { radius, segments } =>
            push_sphere(&mut face_verts, &mut face_idx, p, *radius, lod_segments(*segments), color),
        Shape::Line { points, .. } => {
            // Line shape → 用 LineList
            for pair in points.windows(2) {
                edge_verts.push(Vertex { position: pair[0], normal: [0.0, 1.0, 0.0], color });
                edge_verts.push(Vertex { position: pair[1], normal: [0.0, 1.0, 0.0], color });
            }
        }
        Shape::Mesh(ref mesh) => {
            for (&fid, face) in &mesh.faces {
                let fv = mesh.face_vertices(fid);
                if fv.len() >= 3 {
                    let base = face_verts.len() as u32;
                    for v in &fv {
                        face_verts.push(Vertex {
                            position: [v[0] + p[0], v[1] + p[1], v[2] + p[2]], normal: face.normal, color,
                        });
                    }
                    for i in 1..fv.len()-1 {
                        face_idx.push(base);
                        face_idx.push(base + i as u32);
                        face_idx.push(base + (i+1) as u32);
                    }
                }
            }
            // 邊線：LineList（每條邊 2 頂點，比 quad strips 省 12 倍）
            let draw_edges = if total_scene_objects > 500 {
                screen_size > 200.0
            } else if total_scene_objects > 100 {
                screen_size > 50.0
            } else { true };
            if draw_edges {
                for &(p1, p2) in mesh.all_edge_segments() {
                    let ep1 = [p1[0] + p[0], p1[1] + p[1], p1[2] + p[2]];
                    let ep2 = [p2[0] + p[0], p2[1] + p[1], p2[2] + p[2]];
                    edge_verts.push(Vertex { position: ep1, normal: [0.0, 1.0, 0.0], color: edge_color });
                    edge_verts.push(Vertex { position: ep2, normal: [0.0, 1.0, 0.0], color: edge_color });
                }
            }
        }
        Shape::SteelProfile { profile_type, params, length } => {
            push_steel_profile(&mut face_verts, &mut face_idx, p, *profile_type, params, *length, color);
        }
    }

    // Triplanar texture sampling
    if let Some(ref tex_path) = obj.texture_path {
        if texture_manager.is_loaded(tex_path) {
            let scale = 0.001;
            for vert in &mut face_verts {
                if vert.color[0] < 0.2 && vert.color[1] < 0.2 && vert.color[2] < 0.2 { continue; }
                let tc = texture_manager.triplanar_sample(tex_path, vert.position, vert.normal, scale);
                vert.color = tc;
            }
        }
    }

    // Geometric edge lines for primitive shapes（LOD：遠處跳過）
    // Primitive shape 邊線（LineList）
    let draw_prim_edges = if total_scene_objects > 500 { screen_size > 200.0 } else if total_scene_objects > 100 { screen_size > 50.0 } else { true };
    if draw_prim_edges {
        let n0 = [0.0_f32, 1.0, 0.0];
        match &obj.shape {
            Shape::Box { width, height, depth } => {
                let (w, h, d) = (*width, *height, *depth);
                let box_edges: [([f32; 3], [f32; 3]); 12] = [
                    ([p[0],p[1],p[2]], [p[0]+w,p[1],p[2]]), ([p[0]+w,p[1],p[2]], [p[0]+w,p[1],p[2]+d]),
                    ([p[0]+w,p[1],p[2]+d], [p[0],p[1],p[2]+d]), ([p[0],p[1],p[2]+d], [p[0],p[1],p[2]]),
                    ([p[0],p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]]), ([p[0]+w,p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]+d]),
                    ([p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d]), ([p[0],p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]]),
                    ([p[0],p[1],p[2]], [p[0],p[1]+h,p[2]]), ([p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]]),
                    ([p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d]), ([p[0],p[1],p[2]+d], [p[0],p[1]+h,p[2]+d]),
                ];
                for (a, b) in &box_edges {
                    edge_verts.push(Vertex { position: *a, normal: n0, color: edge_color });
                    edge_verts.push(Vertex { position: *b, normal: n0, color: edge_color });
                }
            }
            Shape::Cylinder { radius, height, segments } => {
                let seg = (*segments).max(6);
                let (cx, cz, r, h) = (p[0], p[2], *radius, *height);
                for y_off in [0.0, h] {
                    for i in 0..seg {
                        let a0 = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        let a1 = ((i+1) as f32 / seg as f32) * std::f32::consts::TAU;
                        edge_verts.push(Vertex { position: [cx + r * a0.cos(), p[1] + y_off, cz + r * a0.sin()], normal: n0, color: edge_color });
                        edge_verts.push(Vertex { position: [cx + r * a1.cos(), p[1] + y_off, cz + r * a1.sin()], normal: n0, color: edge_color });
                    }
                }
                for i in [0, seg / 4, seg / 2, 3 * seg / 4] {
                    let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                    let px = cx + r * a.cos();
                    let pz = cz + r * a.sin();
                    edge_verts.push(Vertex { position: [px, p[1], pz], normal: n0, color: edge_color });
                    edge_verts.push(Vertex { position: [px, p[1] + h, pz], normal: n0, color: edge_color });
                }
            }
            _ => {}
        }
    }

    // Apply XYZ rotation
    apply_rotation(obj, 0, &mut face_verts);
    apply_rotation(obj, 0, &mut edge_verts);

    ObjMeshCache { obj_version: 0, lod_bucket: 0, face_verts, face_idx, edge_verts }
}

/// Selection outline (AABB box for mesh, shape-specific for others)
fn build_selection_outline(obj: &SceneObject, verts: &mut Vec<Vertex>, idx: &mut Vec<u32>, _edge_thickness: f32) {
    let sel_color = [0.2, 0.5, 1.0, 1.0];
    let edge_thickness = 6.0;
    let p = obj.position;
    if let Shape::Mesh(ref mesh) = obj.shape {
        let mut mn = [f32::MAX; 3];
        let mut mx = [f32::MIN; 3];
        for v in mesh.vertices.values() {
            for i in 0..3 { mn[i] = mn[i].min(v.pos[i] + p[i]); mx[i] = mx[i].max(v.pos[i] + p[i]); }
        }
        if mn[0] < f32::MAX {
            let box_edges = [
                (mn, [mx[0],mn[1],mn[2]]), ([mx[0],mn[1],mn[2]], [mx[0],mn[1],mx[2]]),
                ([mx[0],mn[1],mx[2]], [mn[0],mn[1],mx[2]]), ([mn[0],mn[1],mx[2]], mn),
                ([mn[0],mx[1],mn[2]], [mx[0],mx[1],mn[2]]), ([mx[0],mx[1],mn[2]], [mx[0],mx[1],mx[2]]),
                ([mx[0],mx[1],mx[2]], [mn[0],mx[1],mx[2]]), ([mn[0],mx[1],mx[2]], [mn[0],mx[1],mn[2]]),
                (mn, [mn[0],mx[1],mn[2]]), ([mx[0],mn[1],mn[2]], [mx[0],mx[1],mn[2]]),
                ([mx[0],mn[1],mx[2]], [mx[0],mx[1],mx[2]]), ([mn[0],mn[1],mx[2]], [mn[0],mx[1],mx[2]]),
            ];
            for (a, b) in &box_edges { push_line_segments(verts, idx, &[*a, *b], edge_thickness, sel_color); }
        }
    }
}

fn build_face_highlight(_obj: &SceneObject, _hf_idx: u8, _start_idx: usize, _verts: &mut Vec<Vertex>, _idx: &mut Vec<u32>, _selected_face: Option<(&str, u8)>) {
    // 簡化版：face highlight 在大場景中影響較小，暫時省略以減少複雜度
}

fn apply_rotation(obj: &SceneObject, start_idx: usize, verts: &mut [Vertex]) {
    // 四元數旋轉（無萬向鎖）
    let q_arr = crate::tools::rotation_math::effective_quat(
        obj.rotation_quat, obj.rotation_xyz, obj.rotation_y,
    );
    let q = glam::Quat::from_array(q_arr);
    if q.is_near_identity() { return; }

    // 計算物件中心
    let (coff_x, coff_y, coff_z) = match &obj.shape {
        Shape::Box { width, height, depth } => (*width / 2.0, *height / 2.0, *depth / 2.0),
        Shape::Cylinder { height, .. } => (0.0, *height / 2.0, 0.0),
        Shape::Sphere { radius, .. } => (*radius, *radius, *radius),
        Shape::Line { .. } => (0.0, 0.0, 0.0),
        Shape::Mesh(ref mesh) => {
            let (min, max) = mesh.aabb();
            ((max[0]-min[0])/2.0, (max[1]-min[1])/2.0, (max[2]-min[2])/2.0)
        }
        Shape::SteelProfile { length, .. } => (0.0, *length / 2.0, 0.0),
    };
    let center = glam::Vec3::new(
        obj.position[0] + coff_x,
        obj.position[1] + coff_y,
        obj.position[2] + coff_z,
    );
    let mat = glam::Mat3::from_quat(q);

    for v in &mut verts[start_idx..] {
        let d = glam::Vec3::from_array(v.position) - center;
        let r = mat * d;
        v.position = (center + r).to_array();
        let n = mat * glam::Vec3::from_array(v.normal);
        v.normal = n.to_array();
    }
}

fn build_free_mesh(scene: &Scene, editing_group_id: Option<&str>, editing_component_def_id: Option<&str>, face_verts: &mut Vec<Vertex>, face_idx: &mut Vec<u32>, edge_verts: &mut Vec<Vertex>) {
    let mesh = &scene.free_mesh;
    let mat_color = scene.free_mesh_material.color();
    let face_color = if editing_group_id.is_some() || editing_component_def_id.is_some() {
        [mat_color[0] * 0.3, mat_color[1] * 0.3, mat_color[2] * 0.3, mat_color[3]]
    } else { mat_color };
    for (&fid, face) in &mesh.faces {
        let fv = mesh.face_vertices(fid);
        if fv.len() >= 3 {
            let base = face_verts.len() as u32;
            for v in &fv { face_verts.push(Vertex { position: *v, normal: face.normal, color: face_color }); }
            for i in 1..fv.len()-1 { face_idx.push(base); face_idx.push(base + i as u32); face_idx.push(base + (i+1) as u32); }
        }
    }
    let ec = [0.1_f32, 0.1, 0.1, 1.0];
    for &(p1, p2) in mesh.all_edge_segments() {
        edge_verts.push(Vertex { position: p1, normal: [0.0, 1.0, 0.0], color: ec });
        edge_verts.push(Vertex { position: p2, normal: [0.0, 1.0, 0.0], color: ec });
    }
}

// ─── Legacy full rebuild（保留相容性）───────────────────────────────────────

pub(crate) fn build_scene_mesh(
    scene: &Scene, selected_ids: &[String], hovered: Option<&str>,
    editing_group_id: Option<&str>,
    editing_component_def_id: Option<&str>,
    hovered_face: Option<(&str, u8)>,
    selected_face: Option<(&str, u8)>,
    edge_thickness_param: f32,
    render_mode: u32,
    texture_manager: &TextureManager,
    view_proj: glam::Mat4,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idx = Vec::new();

    for obj in scene.objects.values() {
        if !obj.visible { continue; }

        // ── Frustum culling: 跳過完全在視錐外的物件 ──
        {
            let p = glam::Vec3::from(obj.position);
            let extent = match &obj.shape {
                Shape::Box { width, height, depth } => glam::Vec3::new(*width, *height, *depth),
                Shape::Cylinder { radius, height, .. } => glam::Vec3::new(*radius * 2.0, *height, *radius * 2.0),
                Shape::Sphere { radius, .. } => glam::Vec3::splat(*radius * 2.0),
                _ => glam::Vec3::splat(1000.0), // Line/Mesh 保守估計
            };
            let center = p + extent * 0.5;
            let radius = extent.length() * 0.5;
            // 球體 vs frustum 測試：投影到 clip space
            let clip = view_proj * glam::Vec4::new(center.x, center.y, center.z, 1.0);
            if clip.w > 0.0 {
                let ndc_x = clip.x / clip.w;
                let ndc_y = clip.y / clip.w;
                let ndc_r = radius / clip.w * 1.5; // 投影半徑（保守放大）
                // 如果球心 + 半徑完全在 NDC 範圍外，跳過
                if ndc_x - ndc_r > 1.5 || ndc_x + ndc_r < -1.5
                    || ndc_y - ndc_r > 1.5 || ndc_y + ndc_r < -1.5
                {
                    continue;
                }
            }
            // clip.w <= 0 表示在相機後方但可能很大（不 cull，安全起見）
        }

        // Use texture average color if a texture is loaded, otherwise material color
        let mut color = if let Some(ref tex_path) = obj.texture_path {
            if texture_manager.is_loaded(tex_path) {
                texture_manager.average_color(tex_path)
            } else {
                obj.material.color()
            }
        } else {
            obj.material.color()
        };
        if selected_ids.iter().any(|s| s == &obj.id) {
            // 選取高亮：材質色調 + 藍色淡化，保留材質可辨識性
            let sel = [0.2_f32, 0.6, 1.0];
            color[0] = color[0] * 0.45 + sel[0] * 0.55;
            color[1] = color[1] * 0.45 + sel[1] * 0.55;
            color[2] = color[2] * 0.45 + sel[2] * 0.55;
        } else if Some(obj.id.as_str()) == hovered {
            // lighten
            color[0] = (color[0] + 0.15).min(1.0);
            color[1] = (color[1] + 0.15).min(1.0);
            color[2] = (color[2] + 0.15).min(1.0);
        }

        // Group isolation: dim non-group objects
        if let Some(gid) = editing_group_id {
            if obj.id != gid {
                color[0] *= 0.3;
                color[1] *= 0.3;
                color[2] *= 0.3;
                color[3] *= 0.3;
            }
        }

        if let Some(def_id) = editing_component_def_id {
            if obj.component_def_id.as_deref() != Some(def_id) {
                color[0] *= 0.3;
                color[1] *= 0.3;
                color[2] *= 0.3;
                color[3] *= 0.3;
            }
        }

        // PBR: 編碼 roughness 到 alpha（非程序紋理材質時）
        if color[3] >= 0.99 || color[3] <= 0.0 {
            // 非 sentinel alpha → 用 roughness 值（0.0-0.89 範圍）
            color[3] = obj.roughness.clamp(0.05, 0.89);
        }

        let p = obj.position;
        let start_idx = verts.len();

        // LOD: 根據螢幕投影大小降低 segment 數
        let lod_segments = |base_segs: u32| -> u32 {
            let center = glam::Vec3::from(p);
            let clip = view_proj * glam::Vec4::new(center.x, center.y, center.z, 1.0);
            if clip.w > 0.0 {
                let screen_size = 500.0 / clip.w; // 粗估螢幕投影大小
                if screen_size < 20.0 { return (base_segs / 4).max(6); }
                if screen_size < 80.0 { return (base_segs / 2).max(8); }
            }
            base_segs
        };

        match &obj.shape {
            Shape::Box { width, height, depth } =>
                push_box(&mut verts, &mut idx, p, *width, *height, *depth, color),
            Shape::Cylinder { radius, height, segments } =>
                push_cylinder(&mut verts, &mut idx, p, *radius, *height, lod_segments(*segments), color),
            Shape::Sphere { radius, segments } =>
                push_sphere(&mut verts, &mut idx, p, *radius, lod_segments(*segments), color),
            Shape::Line { points, thickness, .. } =>
                push_line_segments(&mut verts, &mut idx, points, *thickness, color),
            Shape::Mesh(ref mesh) => {
                for (&fid, face) in &mesh.faces {
                    let face_verts = mesh.face_vertices(fid);
                    if face_verts.len() >= 3 {
                        let base = verts.len() as u32;
                        for fv in &face_verts {
                            verts.push(Vertex {
                                position: [fv[0] + p[0], fv[1] + p[1], fv[2] + p[2]], normal: face.normal, color,
                            });
                        }
                        for i in 1..face_verts.len()-1 {
                            idx.push(base);
                            idx.push(base + i as u32);
                            idx.push(base + (i+1) as u32);
                        }
                    }
                }
                for &(p1, p2) in mesh.all_edge_segments() {
                    let mesh_edge_color = if render_mode == 5 { [0.0, 0.0, 0.0, 1.0] } else { [0.35, 0.35, 0.35, 1.0] };
                    let mesh_edge_thick = if render_mode == 5 { edge_thickness_param * 1.5 } else { edge_thickness_param };
                    let ep1 = [p1[0] + p[0], p1[1] + p[1], p1[2] + p[2]];
                    let ep2 = [p2[0] + p[0], p2[1] + p[1], p2[2] + p[2]];
                    push_line_segments(&mut verts, &mut idx, &[ep1, ep2], mesh_edge_thick, mesh_edge_color);
                }
            }
            Shape::SteelProfile { params, length, .. } =>
                push_box(&mut verts, &mut idx, p, params.b, *length, params.h, color),
        }

        // ── Per-vertex triplanar texture sampling for textured objects ──
        if let Some(ref tex_path) = obj.texture_path {
            if texture_manager.is_loaded(tex_path) && !selected_ids.iter().any(|s| s == &obj.id) {
                // Recolor face vertices with triplanar-sampled texture color
                // Use a scale of 0.001 (1 texture repeat per 1000mm = 1m)
                let scale = 0.001;
                for vert in &mut verts[start_idx..] {
                    // Skip edge line vertices (very thin quads have small normals — skip if color is dark edge)
                    if vert.color[0] < 0.2 && vert.color[1] < 0.2 && vert.color[2] < 0.2 {
                        continue;
                    }
                    let tc = texture_manager.triplanar_sample(tex_path, vert.position, vert.normal, scale);
                    vert.color = tc;
                }
            }
        }

        // ── Explicit geometric edge lines for ALL objects (SketchUp-style) ──
        {
            let edge_color = if render_mode == 5 {
                [0.0, 0.0, 0.0, 1.0]  // pure black for sketch
            } else {
                [0.35, 0.35, 0.35, 1.0] // subtle gray edges (clay style)
            };
            let edge_thickness = if render_mode == 5 {
                edge_thickness_param * 1.5  // thicker in sketch mode
            } else {
                edge_thickness_param
            };
            match &obj.shape {
                Shape::Box { width, height, depth } => {
                    let (w, h, d) = (*width, *height, *depth);
                    let edges: [([f32; 3], [f32; 3]); 12] = [
                        // Bottom
                        ([p[0],p[1],p[2]], [p[0]+w,p[1],p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1],p[2]+d]),
                        ([p[0]+w,p[1],p[2]+d], [p[0],p[1],p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1],p[2]]),
                        // Top
                        ([p[0],p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d]),
                        ([p[0],p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]]),
                        // Verticals
                        ([p[0],p[1],p[2]], [p[0],p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1]+h,p[2]+d]),
                    ];
                    for (a, b) in &edges {
                        push_line_segments(&mut verts, &mut idx, &[*a, *b], edge_thickness, edge_color);
                    }
                }
                Shape::Cylinder { radius, height, segments } => {
                    let seg = (*segments).max(6);
                    let cx = p[0];
                    let cz = p[2];
                    let r = *radius;
                    let h = *height;
                    // Top and bottom circles
                    for y_off in [0.0, h] {
                        let mut circle_pts: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                        for i in 0..=seg {
                            let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                            circle_pts.push([cx + r * a.cos(), p[1] + y_off, cz + r * a.sin()]);
                        }
                        push_line_segments(&mut verts, &mut idx, &circle_pts, edge_thickness, edge_color);
                    }
                    // 4 vertical lines
                    for i in [0, seg / 4, seg / 2, 3 * seg / 4] {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        let px = cx + r * a.cos();
                        let pz = cz + r * a.sin();
                        push_line_segments(&mut verts, &mut idx,
                            &[[px, p[1], pz], [px, p[1] + h, pz]], edge_thickness, edge_color);
                    }
                }
                Shape::Sphere { radius, segments } => {
                    let seg = (*segments).max(6);
                    let r = *radius;
                    let cx = p[0];
                    let cy = p[1] + r;
                    let cz = p[2];
                    // Equator
                    let mut equator: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        equator.push([cx + r * a.cos(), cy, cz + r * a.sin()]);
                    }
                    push_line_segments(&mut verts, &mut idx, &equator, edge_thickness, edge_color);
                    // Meridian XY
                    let mut meridian: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        meridian.push([cx + r * a.cos(), cy + r * a.sin(), cz]);
                    }
                    push_line_segments(&mut verts, &mut idx, &meridian, edge_thickness, edge_color);
                    // Meridian YZ
                    let mut meridian2: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        meridian2.push([cx, cy + r * a.sin(), cz + r * a.cos()]);
                    }
                    push_line_segments(&mut verts, &mut idx, &meridian2, edge_thickness, edge_color);
                }
                Shape::SteelProfile { profile_type, params, length, .. } => {
                    // 用實際斷面輪廓畫邊線（底面+頂面+側邊連線）
                    let outline = super::primitives::profile_outline(*profile_type, params);
                    if outline.len() >= 3 {
                        let h = *length;
                        // 底面輪廓
                        let mut bot: Vec<[f32; 3]> = outline.iter()
                            .map(|pt| [p[0] + pt[0], p[1], p[2] + pt[1]])
                            .collect();
                        bot.push(bot[0]); // 閉合
                        push_line_segments(&mut verts, &mut idx, &bot, edge_thickness, edge_color);
                        // 頂面輪廓
                        let mut top: Vec<[f32; 3]> = outline.iter()
                            .map(|pt| [p[0] + pt[0], p[1] + h, p[2] + pt[1]])
                            .collect();
                        top.push(top[0]);
                        push_line_segments(&mut verts, &mut idx, &top, edge_thickness, edge_color);
                        // 側邊連線（每個輪廓點底→頂）
                        for pt in &outline {
                            push_line_segments(&mut verts, &mut idx,
                                &[[p[0] + pt[0], p[1], p[2] + pt[1]],
                                  [p[0] + pt[0], p[1] + h, p[2] + pt[1]]],
                                edge_thickness, edge_color);
                        }
                    }
                }
                _ => {} // Line and Mesh shapes handle their own edges
            }
        }

        // ── Selection outline (bright blue AABB for all shapes) ─────────────
        let is_selected = selected_ids.iter().any(|s| s == &obj.id);
        if is_selected {
            let sel_color = [0.2, 0.5, 1.0, 1.0]; // bright blue
            let edge_thickness = 6.0;

            // 通用 AABB 包圍框（所有 Shape 都適用）
            if let Shape::Mesh(ref mesh) = obj.shape {
                let mut mn = [f32::MAX; 3];
                let mut mx = [f32::MIN; 3];
                for v in mesh.vertices.values() {
                    for i in 0..3 { mn[i] = mn[i].min(v.pos[i] + p[i]); mx[i] = mx[i].max(v.pos[i] + p[i]); }
                }
                if mn[0] < f32::MAX {
                    let box_edges: Vec<([f32; 3], [f32; 3])> = vec![
                        ([mn[0],mn[1],mn[2]], [mx[0],mn[1],mn[2]]),
                        ([mx[0],mn[1],mn[2]], [mx[0],mn[1],mx[2]]),
                        ([mx[0],mn[1],mx[2]], [mn[0],mn[1],mx[2]]),
                        ([mn[0],mn[1],mx[2]], [mn[0],mn[1],mn[2]]),
                        ([mn[0],mx[1],mn[2]], [mx[0],mx[1],mn[2]]),
                        ([mx[0],mx[1],mn[2]], [mx[0],mx[1],mx[2]]),
                        ([mx[0],mx[1],mx[2]], [mn[0],mx[1],mx[2]]),
                        ([mn[0],mx[1],mx[2]], [mn[0],mx[1],mn[2]]),
                        ([mn[0],mn[1],mn[2]], [mn[0],mx[1],mn[2]]),
                        ([mx[0],mn[1],mn[2]], [mx[0],mx[1],mn[2]]),
                        ([mx[0],mn[1],mx[2]], [mx[0],mx[1],mx[2]]),
                        ([mn[0],mn[1],mx[2]], [mn[0],mx[1],mx[2]]),
                    ];
                    for (a, b) in &box_edges {
                        push_line_segments(&mut verts, &mut idx, &[*a, *b], edge_thickness, sel_color);
                    }
                }
            }

            match &obj.shape {
                Shape::Box { width, height, depth } => {
                    let (w, h, d) = (*width, *height, *depth);
                    let edges: Vec<([f32; 3], [f32; 3])> = vec![
                        // Bottom
                        ([p[0],p[1],p[2]], [p[0]+w,p[1],p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1],p[2]+d]),
                        ([p[0]+w,p[1],p[2]+d], [p[0],p[1],p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1],p[2]]),
                        // Top
                        ([p[0],p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d]),
                        ([p[0],p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]]),
                        // Verticals
                        ([p[0],p[1],p[2]], [p[0],p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1]+h,p[2]+d]),
                    ];
                    for (a, b) in &edges {
                        push_line_segments(&mut verts, &mut idx, &[*a, *b], edge_thickness, sel_color);
                    }
                }
                Shape::Cylinder { radius, height, segments } => {
                    let seg = (*segments).max(6);
                    let cx = p[0];
                    let cz = p[2];
                    let r = *radius;
                    let h = *height;
                    // Top and bottom circles
                    for y_off in [0.0, h] {
                        let mut circle_pts: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                        for i in 0..=seg {
                            let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                            circle_pts.push([cx + r * a.cos(), p[1] + y_off, cz + r * a.sin()]);
                        }
                        push_line_segments(&mut verts, &mut idx, &circle_pts, edge_thickness, sel_color);
                    }
                    // 4 vertical lines
                    for i in [0, seg / 4, seg / 2, 3 * seg / 4] {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        let px = cx + r * a.cos();
                        let pz = cz + r * a.sin();
                        push_line_segments(&mut verts, &mut idx,
                            &[[px, p[1], pz], [px, p[1] + h, pz]], edge_thickness, sel_color);
                    }
                }
                Shape::Sphere { radius, segments } => {
                    let seg = (*segments).max(6);
                    let r = *radius;
                    let cx = p[0];
                    let cy = p[1] + r; // sphere center is offset by radius
                    let cz = p[2];
                    // Equator (XZ circle at center Y)
                    let mut equator: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        equator.push([cx + r * a.cos(), cy, cz + r * a.sin()]);
                    }
                    push_line_segments(&mut verts, &mut idx, &equator, edge_thickness, sel_color);
                    // Meridian (XY circle)
                    let mut meridian: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        meridian.push([cx + r * a.cos(), cy + r * a.sin(), cz]);
                    }
                    push_line_segments(&mut verts, &mut idx, &meridian, edge_thickness, sel_color);
                    // Second meridian (YZ circle)
                    let mut meridian2: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        meridian2.push([cx, cy + r * a.sin(), cz + r * a.cos()]);
                    }
                    push_line_segments(&mut verts, &mut idx, &meridian2, edge_thickness, sel_color);
                }
                Shape::Mesh(ref mesh) => {
                    for &(p1, p2) in mesh.all_edge_segments() {
                        let ep1 = [p1[0] + p[0], p1[1] + p[1], p1[2] + p[2]];
                        let ep2 = [p2[0] + p[0], p2[1] + p[1], p2[2] + p[2]];
                        push_line_segments(&mut verts, &mut idx, &[ep1, ep2], edge_thickness, sel_color);
                    }
                }
                Shape::SteelProfile { params, length, .. } => {
                    let (w, h, d) = (params.b, *length, params.h);
                    let edges: Vec<([f32; 3], [f32; 3])> = vec![
                        // Bottom
                        ([p[0],p[1],p[2]], [p[0]+w,p[1],p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1],p[2]+d]),
                        ([p[0]+w,p[1],p[2]+d], [p[0],p[1],p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1],p[2]]),
                        // Top
                        ([p[0],p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d]),
                        ([p[0],p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]]),
                        // Verticals
                        ([p[0],p[1],p[2]], [p[0],p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1]+h,p[2]+d]),
                    ];
                    for (a, b) in &edges {
                        push_line_segments(&mut verts, &mut idx, &[*a, *b], edge_thickness, sel_color);
                    }
                }
                _ => {}
            }
        }

        // ── Face & edge hover highlighting ──────────────────────────────────
        // Use axis-aligned colors: X=Red, Y=Green, Z=Blue (matches SketchUp)
        let face_active = selected_face.or(hovered_face);
        if let Some((hf_id, hf_idx)) = face_active {
            if obj.id == hf_id {
                // Axis color: Front/Back(Z)=Blue, Top/Bottom(Y)=Green, Left/Right(X)=Red
                let (axis_tint, edge_color): ([f32; 3], [f32; 4]) = match hf_idx {
                    0 | 1 => ([0.3, 0.4, 0.95], [0.3, 0.5, 1.0, 1.0]),   // Front/Back → Z = Blue
                    2 | 3 => ([0.3, 0.85, 0.3], [0.2, 0.9, 0.2, 1.0]),   // Top/Bottom → Y = Green
                    4 | 5 => ([0.95, 0.3, 0.3], [1.0, 0.3, 0.3, 1.0]),   // Left/Right → X = Red
                    _     => ([0.5, 0.5, 0.5],  [0.8, 0.8, 0.8, 1.0]),
                };

                match &obj.shape {
                    Shape::Box { width, height, depth } => {
                        // Tint the face vertices with axis color
                        let face_start = start_idx + (hf_idx as usize) * 4;
                        if face_start + 4 <= verts.len() {
                            for i in face_start..face_start + 4 {
                                let c = &mut verts[i].color;
                                c[0] = c[0] * 0.25 + axis_tint[0] * 0.75;
                                c[1] = c[1] * 0.25 + axis_tint[1] * 0.75;
                                c[2] = c[2] * 0.25 + axis_tint[2] * 0.75;
                            }
                        }

                        // Draw edge outline in axis color
                        let px = obj.position;
                        let (w, h, d) = (*width, *height, *depth);
                        let corners: [[f32; 3]; 4] = match hf_idx {
                            0 => [ // Front (Z-)
                                [px[0],px[1],px[2]], [px[0]+w,px[1],px[2]],
                                [px[0]+w,px[1]+h,px[2]], [px[0],px[1]+h,px[2]],
                            ],
                            1 => [ // Back (Z+)
                                [px[0]+w,px[1],px[2]+d], [px[0],px[1],px[2]+d],
                                [px[0],px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]+d],
                            ],
                            2 => [ // Top (Y+)
                                [px[0],px[1]+h,px[2]], [px[0]+w,px[1]+h,px[2]],
                                [px[0]+w,px[1]+h,px[2]+d], [px[0],px[1]+h,px[2]+d],
                            ],
                            3 => [ // Bottom (Y-)
                                [px[0],px[1],px[2]+d], [px[0]+w,px[1],px[2]+d],
                                [px[0]+w,px[1],px[2]], [px[0],px[1],px[2]],
                            ],
                            4 => [ // Left (X-)
                                [px[0],px[1],px[2]+d], [px[0],px[1],px[2]],
                                [px[0],px[1]+h,px[2]], [px[0],px[1]+h,px[2]+d],
                            ],
                            5 => [ // Right (X+)
                                [px[0]+w,px[1],px[2]], [px[0]+w,px[1],px[2]+d],
                                [px[0]+w,px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]],
                            ],
                            _ => [[0.0;3];4],
                        };
                        // Draw closed edge loop (5 points = 4 segments forming a rectangle)
                        let edge_pts = [corners[0], corners[1], corners[2], corners[3], corners[0]];
                        push_line_segments(&mut verts, &mut idx, &edge_pts, 6.0, edge_color);
                    }
                    Shape::Cylinder { radius, height, .. } => {
                        // For cylinders, only top/bottom faces are pick-able
                        // hf_idx 2 = Top, 3 = Bottom (mapped from PullFace::Top/Bottom)
                        let is_top = hf_idx == 2;
                        let face_y = if is_top { obj.position[1] + *height } else { obj.position[1] };
                        // Highlight the cap by drawing a circle outline
                        let seg = 24u32;
                        let cx = obj.position[0] + *radius;
                        let cz = obj.position[2] + *radius;
                        let edge_color = [1.0, 0.9, 0.3, 1.0];
                        let mut circle_pts: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                        for i in 0..=seg {
                            let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                            circle_pts.push([cx + *radius * a.cos(), face_y, cz + *radius * a.sin()]);
                        }
                        push_line_segments(&mut verts, &mut idx, &circle_pts, 6.0, edge_color);
                    }
                    Shape::SteelProfile { profile_type, params, length, .. } => {
                        let px = obj.position;
                        let (w, h, d) = (params.b, *length, params.h);

                        // 面頂點染色（Top/Bottom 用實際斷面頂點）
                        let outline = super::primitives::profile_outline(*profile_type, params);
                        let n = outline.len();
                        match hf_idx {
                            2 => { // Top — 頂面頂點從 start_idx + n 開始
                                let top_start = start_idx + n;
                                let top_end = (start_idx + 2 * n).min(verts.len());
                                for i in top_start..top_end {
                                    let c = &mut verts[i].color;
                                    c[0] = c[0] * 0.25 + axis_tint[0] * 0.75;
                                    c[1] = c[1] * 0.25 + axis_tint[1] * 0.75;
                                    c[2] = c[2] * 0.25 + axis_tint[2] * 0.75;
                                }
                            }
                            3 => { // Bottom — 底面頂點從 start_idx 開始
                                let bot_end = (start_idx + n).min(verts.len());
                                for i in start_idx..bot_end {
                                    let c = &mut verts[i].color;
                                    c[0] = c[0] * 0.25 + axis_tint[0] * 0.75;
                                    c[1] = c[1] * 0.25 + axis_tint[1] * 0.75;
                                    c[2] = c[2] * 0.25 + axis_tint[2] * 0.75;
                                }
                            }
                            _ => { /* 側面用 bounding box 邊框表示 */ }
                        }

                        // 邊框（Top/Bottom 用實際斷面輪廓，側面用 bounding box）
                        match hf_idx {
                            2 => { // Top — 頂面輪廓
                                let mut pts: Vec<[f32; 3]> = outline.iter()
                                    .map(|pt| [px[0] + pt[0], px[1] + h, px[2] + pt[1]])
                                    .collect();
                                pts.push(pts[0]);
                                push_line_segments(&mut verts, &mut idx, &pts, 6.0, edge_color);
                            }
                            3 => { // Bottom — 底面輪廓
                                let mut pts: Vec<[f32; 3]> = outline.iter()
                                    .map(|pt| [px[0] + pt[0], px[1], px[2] + pt[1]])
                                    .collect();
                                pts.push(pts[0]);
                                push_line_segments(&mut verts, &mut idx, &pts, 6.0, edge_color);
                            }
                            _ => {
                                // 側面用 bounding box 邊框
                                let corners: [[f32; 3]; 4] = match hf_idx {
                                    0 => [[px[0],px[1],px[2]], [px[0]+w,px[1],px[2]],
                                          [px[0]+w,px[1]+h,px[2]], [px[0],px[1]+h,px[2]]],
                                    1 => [[px[0]+w,px[1],px[2]+d], [px[0],px[1],px[2]+d],
                                          [px[0],px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]+d]],
                                    4 => [[px[0],px[1],px[2]+d], [px[0],px[1],px[2]],
                                          [px[0],px[1]+h,px[2]], [px[0],px[1]+h,px[2]+d]],
                                    5 => [[px[0]+w,px[1],px[2]], [px[0]+w,px[1],px[2]+d],
                                          [px[0]+w,px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]]],
                                    _ => [[0.0;3];4],
                                };
                                let edge_pts = [corners[0], corners[1], corners[2], corners[3], corners[0]];
                                push_line_segments(&mut verts, &mut idx, &edge_pts, 6.0, edge_color);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // ── Click-locked face highlight (stronger than hover) ──────────────
        if let Some((sf_id, sf_idx)) = selected_face {
            if obj.id == sf_id {
                let px = obj.position;
                let edge_color = [0.2, 1.0, 1.0, 1.0]; // cyan

                match &obj.shape {
                    Shape::Box { width, height, depth } => {
                        let (w, h, d) = (*width, *height, *depth);
                        let face_start = start_idx + (sf_idx as usize) * 4;
                        if face_start + 4 <= verts.len() {
                            for i in face_start..face_start + 4 {
                                let c = &mut verts[i].color;
                                c[0] = c[0] * 0.2 + 0.2;
                                c[1] = c[1] * 0.2 + 0.7;
                                c[2] = c[2] * 0.2 + 1.0;
                            }
                        }
                        let corners: [[f32; 3]; 4] = match sf_idx {
                            0 => [[px[0],px[1],px[2]], [px[0]+w,px[1],px[2]], [px[0]+w,px[1]+h,px[2]], [px[0],px[1]+h,px[2]]],
                            1 => [[px[0]+w,px[1],px[2]+d], [px[0],px[1],px[2]+d], [px[0],px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]+d]],
                            2 => [[px[0],px[1]+h,px[2]], [px[0]+w,px[1]+h,px[2]], [px[0]+w,px[1]+h,px[2]+d], [px[0],px[1]+h,px[2]+d]],
                            3 => [[px[0],px[1],px[2]+d], [px[0]+w,px[1],px[2]+d], [px[0]+w,px[1],px[2]], [px[0],px[1],px[2]]],
                            4 => [[px[0],px[1],px[2]+d], [px[0],px[1],px[2]], [px[0],px[1]+h,px[2]], [px[0],px[1]+h,px[2]+d]],
                            5 => [[px[0]+w,px[1],px[2]], [px[0]+w,px[1],px[2]+d], [px[0]+w,px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]]],
                            _ => [[0.0;3];4],
                        };
                        let edge_pts = [corners[0], corners[1], corners[2], corners[3], corners[0]];
                        push_line_segments(&mut verts, &mut idx, &edge_pts, 8.0, edge_color);
                    }
                    Shape::SteelProfile { profile_type, params, length, .. } => {
                        let (w, h, d) = (params.b, *length, params.h);
                        let outline = super::primitives::profile_outline(*profile_type, params);
                        let n = outline.len();
                        // 面頂點染色
                        match sf_idx {
                            2 => { // Top
                                let top_start = start_idx + n;
                                let top_end = (start_idx + 2 * n).min(verts.len());
                                for i in top_start..top_end {
                                    let c = &mut verts[i].color;
                                    c[0] = c[0] * 0.2 + 0.2; c[1] = c[1] * 0.2 + 0.7; c[2] = c[2] * 0.2 + 1.0;
                                }
                            }
                            3 => { // Bottom
                                let bot_end = (start_idx + n).min(verts.len());
                                for i in start_idx..bot_end {
                                    let c = &mut verts[i].color;
                                    c[0] = c[0] * 0.2 + 0.2; c[1] = c[1] * 0.2 + 0.7; c[2] = c[2] * 0.2 + 1.0;
                                }
                            }
                            _ => {}
                        }
                        // 邊框
                        match sf_idx {
                            2 => {
                                let mut pts: Vec<[f32; 3]> = outline.iter()
                                    .map(|pt| [px[0]+pt[0], px[1]+h, px[2]+pt[1]]).collect();
                                pts.push(pts[0]);
                                push_line_segments(&mut verts, &mut idx, &pts, 8.0, edge_color);
                            }
                            3 => {
                                let mut pts: Vec<[f32; 3]> = outline.iter()
                                    .map(|pt| [px[0]+pt[0], px[1], px[2]+pt[1]]).collect();
                                pts.push(pts[0]);
                                push_line_segments(&mut verts, &mut idx, &pts, 8.0, edge_color);
                            }
                            _ => {
                                let corners: [[f32; 3]; 4] = match sf_idx {
                                    0 => [[px[0],px[1],px[2]], [px[0]+w,px[1],px[2]], [px[0]+w,px[1]+h,px[2]], [px[0],px[1]+h,px[2]]],
                                    1 => [[px[0]+w,px[1],px[2]+d], [px[0],px[1],px[2]+d], [px[0],px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]+d]],
                                    4 => [[px[0],px[1],px[2]+d], [px[0],px[1],px[2]], [px[0],px[1]+h,px[2]], [px[0],px[1]+h,px[2]+d]],
                                    5 => [[px[0]+w,px[1],px[2]], [px[0]+w,px[1],px[2]+d], [px[0]+w,px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]]],
                                    _ => [[0.0;3];4],
                                };
                                let edge_pts = [corners[0], corners[1], corners[2], corners[3], corners[0]];
                                push_line_segments(&mut verts, &mut idx, &edge_pts, 8.0, edge_color);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Apply XYZ rotation around object center
        apply_rotation(obj, start_idx, &mut verts);
    }

    // ── Render the shared free mesh ──────────────────────────────────────────
    {
        let mesh = &scene.free_mesh;
        let mat_color = scene.free_mesh_material.color();
        let face_color = if editing_group_id.is_some() || editing_component_def_id.is_some() {
            [mat_color[0] * 0.3, mat_color[1] * 0.3, mat_color[2] * 0.3, mat_color[3]]
        } else {
            mat_color
        };

        // Render faces
        for (&fid, face) in &mesh.faces {
            let face_verts = mesh.face_vertices(fid);
            if face_verts.len() >= 3 {
                let base = verts.len() as u32;
                for fv in &face_verts {
                    verts.push(Vertex {
                        position: *fv,
                        normal: face.normal,
                        color: face_color,
                    });
                }
                for i in 1..face_verts.len() - 1 {
                    idx.push(base);
                    idx.push(base + i as u32);
                    idx.push(base + (i + 1) as u32);
                }
            }
        }

        // Render edges as thin lines
        for &(p1, p2) in mesh.all_edge_segments() {
            push_line_segments(&mut verts, &mut idx, &[p1, p2], 5.0, [0.1, 0.1, 0.1, 1.0]);
        }
    }

    (verts, idx)
}


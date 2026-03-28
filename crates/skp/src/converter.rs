//! 將 SketchUp SDK model 轉換為 SkpScene 結構

use crate::ffi::*;
use crate::*;
use std::collections::HashMap;

/// 轉換整個 model 為 SkpScene
pub fn convert_model(sdk: &SkpSdk, model: &SkpModel) -> Result<SkpScene, SkpError> {
    let mut scene = SkpScene {
        meshes: Vec::new(),
        instances: Vec::new(),
        groups: Vec::new(),
        component_defs: Vec::new(),
        materials: Vec::new(),
        units: "inch".to_string(),
    };

    let mut state = ConvertState {
        mesh_counter: 0,
        instance_counter: 0,
        group_counter: 0,
        processed_defs: HashMap::new(),
    };

    let mut entities = SUEntitiesRef { ptr: std::ptr::null_mut() };
    let result = unsafe { (sdk.fn_model_get_entities)(model.model, &mut entities) };
    if result != SU_ERROR_NONE {
        return Err(SkpError::SdkError("Failed to get entities".into()));
    }

    // 根層級用 identity transform
    convert_entities(sdk, entities, None, IDENTITY_TRANSFORM, &mut scene, &mut state, false)?;

    Ok(scene)
}

const IDENTITY_TRANSFORM: [f64; 16] = [
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0,
];

struct ConvertState {
    mesh_counter: usize,
    instance_counter: usize,
    group_counter: usize,
    /// 已處理過的 entities（key=entities ptr）→ local space mesh 快取
    /// 避免重複呼叫 SUMeshHelperCreate 導致 SDK crash
    processed_defs: HashMap<usize, ProcessedDef>,
}

struct ProcessedDef {
    /// 原始頂點（未套用 instance transform，保持 local space）
    local_vertices: Vec<[f64; 3]>,  // f64 精度 in inches
    local_normals: Vec<[f64; 3]>,
    indices: Vec<u32>,
    edges: Vec<([f64; 3], [f64; 3])>,  // local space edges
    def_name: String,
    def_id: String,
}

/// 4x4 矩陣乘法（column-major）
fn mul_transform(a: &[f64; 16], b: &[f64; 16]) -> [f64; 16] {
    let mut r = [0.0f64; 16];
    for col in 0..4 {
        for row in 0..4 {
            r[col * 4 + row] =
                a[0 * 4 + row] * b[col * 4 + 0] +
                a[1 * 4 + row] * b[col * 4 + 1] +
                a[2 * 4 + row] * b[col * 4 + 2] +
                a[3 * 4 + row] * b[col * 4 + 3];
        }
    }
    r
}

/// 遞迴轉換 entities，帶入 parent 的 world transform
fn convert_entities(
    sdk: &SkpSdk,
    entities: SUEntitiesRef,
    parent_group_id: Option<&str>,
    world_transform: [f64; 16],
    scene: &mut SkpScene,
    state: &mut ConvertState,
    skip_faces: bool,
) -> Result<(), SkpError> {
    // ── 收集 faces → mesh（套用 world_transform）──
    // skip_faces=true 時跳過（component def 的 faces 已由呼叫者處理）
    // 用 entities ptr 做快取 key，避免相同 entities 被重複處理（group/component 共用 def 會 crash）
    let entities_key = entities.ptr as usize;
    if !skip_faces {
        // 如果已快取此 entities 的 faces，直接用快取的 local mesh
        if let Some(cached) = state.processed_defs.get(&entities_key) {
            if !cached.local_vertices.is_empty() {
                state.mesh_counter += 1;
                let mesh_id = format!("mesh_{}", state.mesh_counter);
                let vertices: Vec<[f32; 3]> = cached.local_vertices.iter().map(|v| {
                    let wx = (world_transform[0]*v[0] + world_transform[4]*v[1] + world_transform[8]*v[2] + world_transform[12]) * 25.4;
                    let wy = (world_transform[1]*v[0] + world_transform[5]*v[1] + world_transform[9]*v[2] + world_transform[13]) * 25.4;
                    let wz = (world_transform[2]*v[0] + world_transform[6]*v[1] + world_transform[10]*v[2] + world_transform[14]) * 25.4;
                    [-(wx as f32), wz as f32, wy as f32]
                }).collect();
                let normals: Vec<[f32; 3]> = cached.local_normals.iter().map(|n| {
                    let nx = (world_transform[0]*n[0] + world_transform[4]*n[1] + world_transform[8]*n[2]) as f32;
                    let ny = (world_transform[1]*n[0] + world_transform[5]*n[1] + world_transform[9]*n[2]) as f32;
                    let nz = (world_transform[2]*n[0] + world_transform[6]*n[1] + world_transform[10]*n[2]) as f32;
                    [-nx, nz, ny]
                }).collect();
                let edges: Vec<([f32; 3], [f32; 3])> = cached.edges.iter().map(|(p1, p2)| {
                    let xf = |v: &[f64; 3]| -> [f32; 3] {
                        let wx = (world_transform[0]*v[0] + world_transform[4]*v[1] + world_transform[8]*v[2] + world_transform[12]) * 25.4;
                        let wy = (world_transform[1]*v[0] + world_transform[5]*v[1] + world_transform[9]*v[2] + world_transform[13]) * 25.4;
                        let wz = (world_transform[2]*v[0] + world_transform[6]*v[1] + world_transform[10]*v[2] + world_transform[14]) * 25.4;
                        [-(wx as f32), wz as f32, wy as f32]
                    };
                    (xf(p1), xf(p2))
                }).collect();
                scene.meshes.push(SkpMesh {
                    id: mesh_id.clone(), name: format!("Mesh_{}", state.mesh_counter),
                    vertices, normals, indices: cached.indices.clone(),
                    material_id: None, source_vertex_labels: Vec::new(),
                    source_triangle_debug: Vec::new(), edges,
                });
                state.instance_counter += 1;
                scene.instances.push(SkpInstance {
                    id: format!("inst_{}", state.instance_counter), mesh_id,
                    component_def_id: None, transform: identity_transform(),
                    name: parent_group_id.unwrap_or("CachedGroup").to_string(), layer: String::new(),
                });
            }
        } else {
        // 第一次處理：做 SDK 呼叫並快取 local space 資料
        let faces = get_faces(sdk, entities)?;
        if !faces.is_empty() {
            // 快取 local space mesh
            let (local_verts, local_normals, local_indices, local_face_edges) = faces_to_local_mesh(sdk, &faces)?;
            // 優先使用 SUFaceGetEdges（面輪廓邊），fallback 到 SUEntitiesGetEdges
            let local_edges = if !local_face_edges.is_empty() { local_face_edges } else { get_local_edges(sdk, entities) };
            state.processed_defs.insert(entities_key, ProcessedDef {
                local_vertices: local_verts.clone(),
                local_normals: local_normals.clone(),
                indices: local_indices.clone(),
                edges: local_edges.clone(),
                def_name: String::new(),
                def_id: format!("entities_{:#x}", entities_key),
            });

            // 用 world_transform 建立 mesh
            state.mesh_counter += 1;
            let mesh_id = format!("mesh_{}", state.mesh_counter);
            let vertices: Vec<[f32; 3]> = local_verts.iter().map(|v| {
                let wx = (world_transform[0]*v[0] + world_transform[4]*v[1] + world_transform[8]*v[2] + world_transform[12]) * 25.4;
                let wy = (world_transform[1]*v[0] + world_transform[5]*v[1] + world_transform[9]*v[2] + world_transform[13]) * 25.4;
                let wz = (world_transform[2]*v[0] + world_transform[6]*v[1] + world_transform[10]*v[2] + world_transform[14]) * 25.4;
                [-(wx as f32), wz as f32, wy as f32]
            }).collect();
            let normals: Vec<[f32; 3]> = local_normals.iter().map(|n| {
                let nx = (world_transform[0]*n[0] + world_transform[4]*n[1] + world_transform[8]*n[2]) as f32;
                let ny = (world_transform[1]*n[0] + world_transform[5]*n[1] + world_transform[9]*n[2]) as f32;
                let nz = (world_transform[2]*n[0] + world_transform[6]*n[1] + world_transform[10]*n[2]) as f32;
                [-nx, nz, ny]
            }).collect();
            let edges: Vec<([f32; 3], [f32; 3])> = local_edges.iter().map(|(p1, p2)| {
                let xf = |v: &[f64; 3]| -> [f32; 3] {
                    let wx = (world_transform[0]*v[0] + world_transform[4]*v[1] + world_transform[8]*v[2] + world_transform[12]) * 25.4;
                    let wy = (world_transform[1]*v[0] + world_transform[5]*v[1] + world_transform[9]*v[2] + world_transform[13]) * 25.4;
                    let wz = (world_transform[2]*v[0] + world_transform[6]*v[1] + world_transform[10]*v[2] + world_transform[14]) * 25.4;
                    [-(wx as f32), wz as f32, wy as f32]
                };
                (xf(p1), xf(p2))
            }).collect();
            scene.meshes.push(SkpMesh {
                id: mesh_id.clone(), name: format!("Mesh_{}", state.mesh_counter),
                vertices, normals, indices: local_indices,
                material_id: None, source_vertex_labels: Vec::new(),
                source_triangle_debug: Vec::new(), edges,
            });

            state.instance_counter += 1;
            let inst = SkpInstance {
                id: format!("inst_{}", state.instance_counter),
                mesh_id,
                component_def_id: None,
                transform: identity_transform(),
                name: parent_group_id.unwrap_or("Root").to_string(),
                layer: String::new(),
            };
            if let Some(gid) = parent_group_id {
                if let Some(g) = scene.groups.iter_mut().find(|g| g.id == gid) {
                    g.children.push(inst.id.clone());
                }
            }
            scene.instances.push(inst);
        } else {
            // 空 entities，也快取（避免下次重新取）
            state.processed_defs.insert(entities_key, ProcessedDef {
                local_vertices: Vec::new(), local_normals: Vec::new(),
                indices: Vec::new(), edges: Vec::new(),
                def_name: String::new(), def_id: format!("entities_{:#x}", entities_key),
            });
        }
        } // end else (not cached)
    }

    // ── Groups（累積 transform）──
    let groups = get_groups(sdk, entities)?;
    for group_ref in &groups {
        state.group_counter += 1;
        let gid = format!("grp_{}", state.group_counter);
        let name = sdk.read_name(|s| unsafe { (sdk.fn_group_get_name)(*group_ref, s) });

        // 取得 group 的 local transform
        let mut local_xf = SUTransformation { values: [0.0; 16] };
        unsafe { (sdk.fn_group_get_transform)(*group_ref, &mut local_xf) };
        let child_world = mul_transform(&world_transform, &local_xf.values);

        scene.groups.push(SkpGroup {
            id: gid.clone(),
            name: if name.is_empty() { format!("Group_{}", state.group_counter) } else { name },
            children: Vec::new(),
            parent_id: parent_group_id.map(|s| s.to_string()),
        });

        let mut group_entities = SUEntitiesRef { ptr: std::ptr::null_mut() };
        unsafe { (sdk.fn_group_get_entities)(*group_ref, &mut group_entities) };
        convert_entities(sdk, group_entities, Some(&gid), child_world, scene, state, false)?;
    }

    // ── Component instances（快取 def，避免重複 SDK 呼叫導致 crash）──
    let instances = get_component_instances(sdk, entities)?;
    for inst_ref in &instances {
        let mut def_ref = SUComponentDefinitionRef { ptr: std::ptr::null_mut() };
        unsafe { (sdk.fn_comp_inst_get_definition)(*inst_ref, &mut def_ref) };

        let def_key = def_ref.ptr as usize;
        let def_name = sdk.read_name(|s| unsafe { (sdk.fn_comp_def_get_name)(def_ref, s) });
        let inst_name = sdk.read_name(|s| unsafe { (sdk.fn_comp_inst_get_name)(*inst_ref, s) });

        let mut local_xf = SUTransformation { values: [0.0; 16] };
        unsafe { (sdk.fn_comp_inst_get_transform)(*inst_ref, &mut local_xf) };
        let inst_world = mul_transform(&world_transform, &local_xf.values);

        // 第一次遇到此 def → 解析 faces/edges 並快取（local space）
        if !state.processed_defs.contains_key(&def_key) {
            let mut def_entities = SUEntitiesRef { ptr: std::ptr::null_mut() };
            unsafe { (sdk.fn_comp_def_get_entities)(def_ref, &mut def_entities) };

            let def_faces = get_faces(sdk, def_entities)?;
            let (local_verts, local_normals, local_indices, local_face_edges) = if !def_faces.is_empty() {
                faces_to_local_mesh(sdk, &def_faces)?
            } else {
                (Vec::new(), Vec::new(), Vec::new(), Vec::new())
            };
            let local_edges = if !local_face_edges.is_empty() { local_face_edges } else { get_local_edges(sdk, def_entities) };
            let def_id = format!("comp_{}", def_key);

            state.processed_defs.insert(def_key, ProcessedDef {
                local_vertices: local_verts,
                local_normals: local_normals,
                indices: local_indices,
                edges: local_edges,
                def_name: def_name.clone(),
                def_id: def_id.clone(),
            });

            // 遞迴 component 內的子 group/instance（跳過 faces，已處理）
            convert_entities(sdk, def_entities, parent_group_id, inst_world, scene, state, true)?;
        }

        // 從快取建立 mesh（套用 instance 的 world transform）
        let cached = match state.processed_defs.get(&def_key) {
            Some(c) => c,
            None => continue,
        };
        if cached.local_vertices.is_empty() { continue; }

        state.mesh_counter += 1;
        let mesh_id = format!("mesh_{}", state.mesh_counter);

        let vertices: Vec<[f32; 3]> = cached.local_vertices.iter().map(|v| {
            let wx = (inst_world[0]*v[0] + inst_world[4]*v[1] + inst_world[8]*v[2] + inst_world[12]) * 25.4;
            let wy = (inst_world[1]*v[0] + inst_world[5]*v[1] + inst_world[9]*v[2] + inst_world[13]) * 25.4;
            let wz = (inst_world[2]*v[0] + inst_world[6]*v[1] + inst_world[10]*v[2] + inst_world[14]) * 25.4;
            [-(wx as f32), wz as f32, wy as f32]
        }).collect();

        let normals: Vec<[f32; 3]> = cached.local_normals.iter().map(|n| {
            let nx = (inst_world[0]*n[0] + inst_world[4]*n[1] + inst_world[8]*n[2]) as f32;
            let ny = (inst_world[1]*n[0] + inst_world[5]*n[1] + inst_world[9]*n[2]) as f32;
            let nz = (inst_world[2]*n[0] + inst_world[6]*n[1] + inst_world[10]*n[2]) as f32;
            [-nx, nz, ny]
        }).collect();

        let edges: Vec<([f32; 3], [f32; 3])> = cached.edges.iter().map(|(p1, p2)| {
            let xf = |v: &[f64; 3]| -> [f32; 3] {
                let wx = (inst_world[0]*v[0] + inst_world[4]*v[1] + inst_world[8]*v[2] + inst_world[12]) * 25.4;
                let wy = (inst_world[1]*v[0] + inst_world[5]*v[1] + inst_world[9]*v[2] + inst_world[13]) * 25.4;
                let wz = (inst_world[2]*v[0] + inst_world[6]*v[1] + inst_world[10]*v[2] + inst_world[14]) * 25.4;
                [-(wx as f32), wz as f32, wy as f32]
            };
            (xf(p1), xf(p2))
        }).collect();

        let def_id = cached.def_id.clone();
        let cached_def_name = cached.def_name.clone();

        if !scene.component_defs.iter().any(|d| d.id == def_id) {
            scene.component_defs.push(SkpComponentDef {
                id: def_id.clone(),
                name: cached_def_name.clone(),
                mesh_ids: vec![mesh_id.clone()],
                instance_count: 0,
            });
        }
        if let Some(d) = scene.component_defs.iter_mut().find(|d| d.id == def_id) {
            d.instance_count += 1;
        }

        scene.meshes.push(SkpMesh {
            id: mesh_id.clone(),
            name: format!("Mesh_{}", state.mesh_counter),
            vertices,
            normals,
            indices: cached.indices.clone(),
            material_id: None,
            source_vertex_labels: Vec::new(),
            source_triangle_debug: Vec::new(),
            edges,
        });

        state.instance_counter += 1;
        let inst = SkpInstance {
            id: format!("inst_{}", state.instance_counter),
            mesh_id,
            component_def_id: Some(def_id),
            transform: identity_transform(),
            name: if inst_name.is_empty() { cached_def_name } else { inst_name },
            layer: String::new(),
        };
        if let Some(gid) = parent_group_id {
            if let Some(g) = scene.groups.iter_mut().find(|g| g.id == gid) {
                g.children.push(inst.id.clone());
            }
        }
        scene.instances.push(inst);
    }

    Ok(())
}

// ─── 輔助 ──────────────────────────────────────────────────────

fn get_faces(sdk: &SkpSdk, entities: SUEntitiesRef) -> Result<Vec<SUFaceRef>, SkpError> {
    let mut count = 0usize;
    unsafe { (sdk.fn_entities_get_num_faces)(entities, &mut count) };
    if count == 0 { return Ok(Vec::new()); }
    let mut faces = vec![SUFaceRef { ptr: std::ptr::null_mut() }; count];
    let mut actual = 0usize;
    unsafe { (sdk.fn_entities_get_faces)(entities, count, faces.as_mut_ptr(), &mut actual) };
    faces.truncate(actual);
    Ok(faces)
}

fn get_groups(sdk: &SkpSdk, entities: SUEntitiesRef) -> Result<Vec<SUGroupRef>, SkpError> {
    let mut count = 0usize;
    unsafe { (sdk.fn_entities_get_num_groups)(entities, &mut count) };
    if count == 0 { return Ok(Vec::new()); }
    let mut groups = vec![SUGroupRef { ptr: std::ptr::null_mut() }; count];
    let mut actual = 0usize;
    unsafe { (sdk.fn_entities_get_groups)(entities, count, groups.as_mut_ptr(), &mut actual) };
    groups.truncate(actual);
    Ok(groups)
}

fn get_component_instances(sdk: &SkpSdk, entities: SUEntitiesRef) -> Result<Vec<SUComponentInstanceRef>, SkpError> {
    let mut count = 0usize;
    unsafe { (sdk.fn_entities_get_num_instances)(entities, &mut count) };
    if count == 0 { return Ok(Vec::new()); }
    let mut instances = vec![SUComponentInstanceRef { ptr: std::ptr::null_mut() }; count];
    let mut actual = 0usize;
    unsafe { (sdk.fn_entities_get_instances)(entities, count, instances.as_mut_ptr(), &mut actual) };
    instances.truncate(actual);
    Ok(instances)
}

/// 將面轉為 mesh，使用 SUMeshHelper 正確三角化（支援凹多邊形）
fn faces_to_mesh(sdk: &SkpSdk, faces: &[SUFaceRef], world_xf: &[f64; 16], state: &mut ConvertState) -> Result<SkpMesh, SkpError> {
    state.mesh_counter += 1;
    let mesh_id = format!("mesh_{}", state.mesh_counter);
    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    let mut source_vertex_labels = Vec::new();
    let mut source_triangle_debug = Vec::new();

    for (fi, face) in faces.iter().enumerate() {
        // 跳過無效的 face ref
        if face.ptr.is_null() { continue; }
        // 用 MeshHelper 取得正確三角化
        let mut helper = SUMeshHelperRef { ptr: std::ptr::null_mut() };
        // 先嘗試取得 face 的頂點數來驗證 face ref 是否有效
        let mut nv_check = 0usize;
        let check_rc = unsafe { (sdk.fn_face_get_num_vertices)(*face, &mut nv_check) };
        if check_rc != SU_ERROR_NONE || nv_check == 0 {
            continue;
        }
        let rc = unsafe { (sdk.fn_mesh_helper_create)(&mut helper, *face) };

        let mut num_tris = 0usize;
        let mut num_verts = 0usize;
        unsafe {
            (sdk.fn_mesh_helper_get_num_triangles)(helper, &mut num_tris);
            (sdk.fn_mesh_helper_get_num_vertices)(helper, &mut num_verts);
        }
        if num_tris == 0 || num_verts == 0 {
            unsafe { (sdk.fn_mesh_helper_release)(&mut helper) };
            continue;
        }

        let mut pts = vec![SUPoint3D { x: 0.0, y: 0.0, z: 0.0 }; num_verts];
        let mut actual_verts = 0usize;
        unsafe { (sdk.fn_mesh_helper_get_vertices)(helper, num_verts, pts.as_mut_ptr(), &mut actual_verts) };

        let mut nrms = vec![SUVector3D { x: 0.0, y: 0.0, z: 0.0 }; num_verts];
        let mut actual_normals = 0usize;
        unsafe { (sdk.fn_mesh_helper_get_normals)(helper, num_verts, nrms.as_mut_ptr(), &mut actual_normals) };

        let idx_count = num_tris * 3;
        let mut tri_indices = vec![0usize; idx_count];
        let mut actual_idx = 0usize;
        unsafe { (sdk.fn_mesh_helper_get_vertex_indices)(helper, idx_count, tri_indices.as_mut_ptr(), &mut actual_idx) };

        unsafe { (sdk.fn_mesh_helper_release)(&mut helper) };

        let base = vertices.len() as u32;
        for i in 0..actual_verts {
            let pos = &pts[i];
            let wx = (world_xf[0]*pos.x + world_xf[4]*pos.y + world_xf[8]*pos.z + world_xf[12]) * 25.4;
            let wy = (world_xf[1]*pos.x + world_xf[5]*pos.y + world_xf[9]*pos.z + world_xf[13]) * 25.4;
            let wz = (world_xf[2]*pos.x + world_xf[6]*pos.y + world_xf[10]*pos.z + world_xf[14]) * 25.4;
            // SU(X,Y,Z) → Kolibri(X,Z,Y)
            vertices.push([-(wx as f32), wz as f32, wy as f32]);

            let n = if i < actual_normals { &nrms[i] } else { &SUVector3D { x: 0.0, y: 0.0, z: 1.0 } };
            let nx = (world_xf[0]*n.x + world_xf[4]*n.y + world_xf[8]*n.z) as f32;
            let ny = (world_xf[1]*n.x + world_xf[5]*n.y + world_xf[9]*n.z) as f32;
            let nz = (world_xf[2]*n.x + world_xf[6]*n.y + world_xf[10]*n.z) as f32;
            normals.push([-nx, nz, ny]);
        }

        // 索引（0-based）— 逐三角形檢查 winding
        // 用 cross product 算出的法線跟 MeshHelper 法線比較，不一致就翻轉
        for tri in 0..(actual_idx / 3) {
            let i0 = tri_indices[tri * 3];
            let i1 = tri_indices[tri * 3 + 1];
            let i2 = tri_indices[tri * 3 + 2];
            // 取已轉換的頂點（在 vertices 裡）
            let vi0 = (base as usize) + i0;
            let vi1 = (base as usize) + i1;
            let vi2 = (base as usize) + i2;
            if vi0 < vertices.len() && vi1 < vertices.len() && vi2 < vertices.len() {
                let v0 = vertices[vi0];
                let v1 = vertices[vi1];
                let v2 = vertices[vi2];
                // cross product 法線
                let a = [v1[0]-v0[0], v1[1]-v0[1], v1[2]-v0[2]];
                let b = [v2[0]-v0[0], v2[1]-v0[1], v2[2]-v0[2]];
                let cx = a[1]*b[2] - a[2]*b[1];
                let cy = a[2]*b[0] - a[0]*b[2];
                let cz = a[0]*b[1] - a[1]*b[0];
                // MeshHelper 法線（已轉換，取第一個頂點的）
                let mn = if vi0 < normals.len() { normals[vi0] } else { [0.0, 1.0, 0.0] };
                let dot = cx * mn[0] + cy * mn[1] + cz * mn[2];
                if dot < 0.0 {
                    // negate-X 後手性反轉，dot < 0 表示方向一致
                    indices.push(base + i0 as u32);
                    indices.push(base + i1 as u32);
                    indices.push(base + i2 as u32);
                } else {
                    // 翻轉 winding
                    indices.push(base + i0 as u32);
                    indices.push(base + i2 as u32);
                    indices.push(base + i1 as u32);
                }
            }
        }
    }

    Ok(SkpMesh {
        id: mesh_id,
        name: format!("Mesh_{}", state.mesh_counter),
        vertices,
        normals,
        indices,
        material_id: None,
        source_vertex_labels,
        source_triangle_debug,
        edges: Vec::new(), // 由呼叫者填入 SDK 原始邊線
    })
}

/// 將 faces 轉為 local space mesh（不套用 transform，用於 component def 快取）
/// 回傳 (vertices, normals, indices, face_edges)
/// face_edges 是面的真正輪廓邊（SUFaceGetEdges），不含三角化邊
fn faces_to_local_mesh(sdk: &SkpSdk, faces: &[SUFaceRef]) -> Result<(Vec<[f64; 3]>, Vec<[f64; 3]>, Vec<u32>, Vec<([f64; 3], [f64; 3])>), SkpError> {
    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    let mut face_edges = Vec::new();

    for face in faces {
        // 讀取面的輪廓邊（SUFaceGetEdges — 官方範例推薦方式）
        let mut ne = 0usize;
        unsafe { (sdk.fn_face_get_num_edges)(*face, &mut ne) };
        if ne > 0 {
            let mut edges = vec![SUEdgeRef { ptr: std::ptr::null_mut() }; ne];
            let mut actual_e = 0usize;
            unsafe { (sdk.fn_face_get_edges)(*face, ne, edges.as_mut_ptr(), &mut actual_e) };
            for edge in &edges[..actual_e] {
                // 跳過 soft/smooth 邊
                let mut soft = false;
                let mut smooth = false;
                unsafe {
                    (sdk.fn_edge_get_soft)(*edge, &mut soft);
                    (sdk.fn_edge_get_smooth)(*edge, &mut smooth);
                }
                if soft || smooth { continue; }

                let mut sv = SUVertexRef { ptr: std::ptr::null_mut() };
                let mut ev = SUVertexRef { ptr: std::ptr::null_mut() };
                unsafe {
                    (sdk.fn_edge_get_start_vertex)(*edge, &mut sv);
                    (sdk.fn_edge_get_end_vertex)(*edge, &mut ev);
                }
                let mut sp = SUPoint3D { x: 0.0, y: 0.0, z: 0.0 };
                let mut ep = SUPoint3D { x: 0.0, y: 0.0, z: 0.0 };
                unsafe {
                    (sdk.fn_vertex_get_position)(sv, &mut sp);
                    (sdk.fn_vertex_get_position)(ev, &mut ep);
                }
                face_edges.push(([sp.x, sp.y, sp.z], [ep.x, ep.y, ep.z]));
            }
        }

        let mut helper = SUMeshHelperRef { ptr: std::ptr::null_mut() };
        let rc = unsafe { (sdk.fn_mesh_helper_create)(&mut helper, *face) };
        if rc != SU_ERROR_NONE { continue; }

        let mut num_tris = 0usize;
        let mut num_verts = 0usize;
        unsafe {
            (sdk.fn_mesh_helper_get_num_triangles)(helper, &mut num_tris);
            (sdk.fn_mesh_helper_get_num_vertices)(helper, &mut num_verts);
        }
        if num_tris == 0 || num_verts == 0 {
            unsafe { (sdk.fn_mesh_helper_release)(&mut helper) };
            continue;
        }

        let mut pts = vec![SUPoint3D { x: 0.0, y: 0.0, z: 0.0 }; num_verts];
        let mut actual_verts = 0usize;
        unsafe { (sdk.fn_mesh_helper_get_vertices)(helper, num_verts, pts.as_mut_ptr(), &mut actual_verts) };

        let mut nrms = vec![SUVector3D { x: 0.0, y: 0.0, z: 0.0 }; num_verts];
        let mut actual_normals = 0usize;
        unsafe { (sdk.fn_mesh_helper_get_normals)(helper, num_verts, nrms.as_mut_ptr(), &mut actual_normals) };

        let idx_count = num_tris * 3;
        let mut tri_indices = vec![0usize; idx_count];
        let mut actual_idx = 0usize;
        unsafe { (sdk.fn_mesh_helper_get_vertex_indices)(helper, idx_count, tri_indices.as_mut_ptr(), &mut actual_idx) };

        unsafe { (sdk.fn_mesh_helper_release)(&mut helper) };

        let base = vertices.len() as u32;
        for i in 0..actual_verts {
            vertices.push([pts[i].x, pts[i].y, pts[i].z]);
            let n = if i < actual_normals { &nrms[i] } else { &SUVector3D { x: 0.0, y: 0.0, z: 1.0 } };
            normals.push([n.x, n.y, n.z]);
        }
        // 翻轉 winding order（因為 negate X 改變了手性）
        let end = actual_idx.min(tri_indices.len());
        for tri in 0..(end / 3) {
            let i0 = tri_indices[tri * 3];
            let i1 = tri_indices[tri * 3 + 1];
            let i2 = tri_indices[tri * 3 + 2];
            indices.push(base + i0 as u32);
            indices.push(base + i2 as u32);  // 交換 i1, i2
            indices.push(base + i1 as u32);
        }
    }
    // 去重 face_edges（相同邊可能被多個面引用）
    face_edges.sort_by(|a, b| {
        let key = |e: &([f64; 3], [f64; 3])| -> u64 {
            let h1 = (e.0[0] * 1000.0) as i64;
            let h2 = (e.0[1] * 1000.0) as i64;
            (h1.wrapping_mul(31) ^ h2) as u64
        };
        key(a).cmp(&key(b))
    });
    face_edges.dedup_by(|a, b| {
        let close = |x: f64, y: f64| (x - y).abs() < 0.01;
        (close(a.0[0], b.0[0]) && close(a.0[1], b.0[1]) && close(a.0[2], b.0[2])
         && close(a.1[0], b.1[0]) && close(a.1[1], b.1[1]) && close(a.1[2], b.1[2]))
        || (close(a.0[0], b.1[0]) && close(a.0[1], b.1[1]) && close(a.0[2], b.1[2])
         && close(a.1[0], b.0[0]) && close(a.1[1], b.0[1]) && close(a.1[2], b.0[2]))
    });
    Ok((vertices, normals, indices, face_edges))
}

/// 讀取 SDK 原始邊線（local space，不套用 transform）
fn get_local_edges(sdk: &SkpSdk, entities: SUEntitiesRef) -> Vec<([f64; 3], [f64; 3])> {
    let mut count = 0usize;
    unsafe { (sdk.fn_entities_get_num_edges)(entities, &mut count) };
    if count == 0 { return Vec::new(); }

    let mut edges = vec![SUEdgeRef { ptr: std::ptr::null_mut() }; count];
    let mut actual = 0usize;
    unsafe { (sdk.fn_entities_get_edges)(entities, count, edges.as_mut_ptr(), &mut actual) };

    let mut segments = Vec::new();
    for edge in &edges[..actual] {
        let mut soft = false;
        let mut smooth = false;
        unsafe {
            (sdk.fn_edge_get_soft)(*edge, &mut soft);
            (sdk.fn_edge_get_smooth)(*edge, &mut smooth);
        }
        if soft || smooth { continue; }

        let mut sv = SUVertexRef { ptr: std::ptr::null_mut() };
        let mut ev = SUVertexRef { ptr: std::ptr::null_mut() };
        unsafe {
            (sdk.fn_edge_get_start_vertex)(*edge, &mut sv);
            (sdk.fn_edge_get_end_vertex)(*edge, &mut ev);
        }
        let mut sp = SUPoint3D { x: 0.0, y: 0.0, z: 0.0 };
        let mut ep = SUPoint3D { x: 0.0, y: 0.0, z: 0.0 };
        unsafe {
            (sdk.fn_vertex_get_position)(sv, &mut sp);
            (sdk.fn_vertex_get_position)(ev, &mut ep);
        }
        segments.push(([sp.x, sp.y, sp.z], [ep.x, ep.y, ep.z]));
    }
    segments
}

/// 從 entities 讀取 SDK 原始邊線（不含 soft/smooth 邊）
fn get_sdk_edges(sdk: &SkpSdk, entities: SUEntitiesRef, world_xf: &[f64; 16]) -> Vec<([f32; 3], [f32; 3])> {
    let mut count = 0usize;
    unsafe { (sdk.fn_entities_get_num_edges)(entities, &mut count) };
    if count == 0 { return Vec::new(); }

    let mut edges = vec![SUEdgeRef { ptr: std::ptr::null_mut() }; count];
    let mut actual = 0usize;
    unsafe { (sdk.fn_entities_get_edges)(entities, count, edges.as_mut_ptr(), &mut actual) };

    let mut segments = Vec::new();
    for edge in &edges[..actual] {
        // 跳過 soft/smooth 邊（SketchUp 也不顯示）
        let mut soft = false;
        let mut smooth = false;
        unsafe {
            (sdk.fn_edge_get_soft)(*edge, &mut soft);
            (sdk.fn_edge_get_smooth)(*edge, &mut smooth);
        }
        if soft || smooth { continue; }

        let mut sv = SUVertexRef { ptr: std::ptr::null_mut() };
        let mut ev = SUVertexRef { ptr: std::ptr::null_mut() };
        unsafe {
            (sdk.fn_edge_get_start_vertex)(*edge, &mut sv);
            (sdk.fn_edge_get_end_vertex)(*edge, &mut ev);
        }
        let mut sp = SUPoint3D { x: 0.0, y: 0.0, z: 0.0 };
        let mut ep = SUPoint3D { x: 0.0, y: 0.0, z: 0.0 };
        unsafe {
            (sdk.fn_vertex_get_position)(sv, &mut sp);
            (sdk.fn_vertex_get_position)(ev, &mut ep);
        }

        // 套用 world transform + inch→mm + 座標軸交換
        let transform_pt = |p: &SUPoint3D| -> [f32; 3] {
            let wx = (world_xf[0]*p.x + world_xf[4]*p.y + world_xf[8]*p.z + world_xf[12]) * 25.4;
            let wy = (world_xf[1]*p.x + world_xf[5]*p.y + world_xf[9]*p.z + world_xf[13]) * 25.4;
            let wz = (world_xf[2]*p.x + world_xf[6]*p.y + world_xf[10]*p.z + world_xf[14]) * 25.4;
            [-(wx as f32), wz as f32, wy as f32] // SU(X,Y,Z) → Kolibri(X,Z,Y)
        };

        segments.push((transform_pt(&sp), transform_pt(&ep)));
    }
    segments
}

fn identity_transform() -> [f32; 16] {
    [1.0, 0.0, 0.0, 0.0,
     0.0, 1.0, 0.0, 0.0,
     0.0, 0.0, 1.0, 0.0,
     0.0, 0.0, 0.0, 1.0]
}

fn transform_point(world_xf: &[f64; 16], pos: &SUPoint3D) -> [f32; 3] {
    let wx = (world_xf[0] * pos.x + world_xf[4] * pos.y + world_xf[8] * pos.z + world_xf[12]) * 25.4;
    let wy = (world_xf[1] * pos.x + world_xf[5] * pos.y + world_xf[9] * pos.z + world_xf[13]) * 25.4;
    let wz = (world_xf[2] * pos.x + world_xf[6] * pos.y + world_xf[10] * pos.z + world_xf[14]) * 25.4;
    [-(wx as f32), wz as f32, wy as f32]
}

fn transform_normal(world_xf: &[f64; 16], n: &SUVector3D) -> [f32; 3] {
    let nx = (world_xf[0] * n.x + world_xf[4] * n.y + world_xf[8] * n.z) as f32;
    let ny = (world_xf[1] * n.x + world_xf[5] * n.y + world_xf[9] * n.z) as f32;
    let nz = (world_xf[2] * n.x + world_xf[6] * n.y + world_xf[10] * n.z) as f32;
    normalize3([-nx, nz, ny])
}

fn triangulate_polygon(vertices: &[[f32; 3]], normal: [f32; 3]) -> Vec<usize> {
    if vertices.len() < 3 {
        return Vec::new();
    }
    if vertices.len() == 3 {
        return vec![0, 1, 2];
    }

    let projected: Vec<[f32; 2]> = vertices.iter().map(|v| project_to_plane(*v, normal)).collect();
    let signed_area = polygon_area_2d(&projected);
    if signed_area.abs() < 1e-5 {
        return Vec::new();
    }

    let mut remaining: Vec<usize> = if signed_area > 0.0 {
        (0..vertices.len()).collect()
    } else {
        (0..vertices.len()).rev().collect()
    };
    let mut out = Vec::with_capacity((vertices.len() - 2) * 3);
    let mut guard = 0usize;

    while remaining.len() > 3 && guard < vertices.len() * vertices.len() {
        guard += 1;
        let mut ear_found = false;
        for i in 0..remaining.len() {
            let prev = remaining[(i + remaining.len() - 1) % remaining.len()];
            let curr = remaining[i];
            let next = remaining[(i + 1) % remaining.len()];
            if !is_ear(prev, curr, next, &remaining, &projected) {
                continue;
            }
            out.extend_from_slice(&[prev, curr, next]);
            remaining.remove(i);
            ear_found = true;
            break;
        }
        if !ear_found {
            return Vec::new();
        }
    }

    if remaining.len() == 3 {
        out.extend_from_slice(&[remaining[0], remaining[1], remaining[2]]);
    }
    out
}

fn project_to_plane(v: [f32; 3], normal: [f32; 3]) -> [f32; 2] {
    let ax = normal[0].abs();
    let ay = normal[1].abs();
    let az = normal[2].abs();
    if ax >= ay && ax >= az {
        [v[1], v[2]]
    } else if ay >= az {
        [v[0], v[2]]
    } else {
        [v[0], v[1]]
    }
}

fn polygon_area_2d(points: &[[f32; 2]]) -> f32 {
    let mut area = 0.0f32;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        area += a[0] * b[1] - b[0] * a[1];
    }
    area * 0.5
}

fn is_ear(prev: usize, curr: usize, next: usize, polygon: &[usize], projected: &[[f32; 2]]) -> bool {
    let a = projected[prev];
    let b = projected[curr];
    let c = projected[next];
    if cross2(a, b, c) <= 1e-5 {
        return false;
    }
    for &idx in polygon {
        if idx == prev || idx == curr || idx == next {
            continue;
        }
        if point_in_triangle(projected[idx], a, b, c) {
            return false;
        }
    }
    true
}

fn cross2(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn point_in_triangle(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let ab = cross2(a, b, p);
    let bc = cross2(b, c, p);
    let ca = cross2(c, a, p);
    (ab >= -1e-5 && bc >= -1e-5 && ca >= -1e-5) || (ab <= 1e-5 && bc <= 1e-5 && ca <= 1e-5)
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len <= 1e-8 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

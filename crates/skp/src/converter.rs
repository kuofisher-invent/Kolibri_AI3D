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
        units: "inch".to_string(), // SU 預設 inch，轉換時再 scale 到 mm
    };

    let mut state = ConvertState {
        material_map: HashMap::new(),
        mesh_counter: 0,
        instance_counter: 0,
        group_counter: 0,
    };

    // 取得根 entities
    let mut entities = SUEntitiesRef { ptr: std::ptr::null_mut() };
    let result = unsafe { (sdk.fn_model_get_entities)(model.model, &mut entities) };
    if result != SU_ERROR_NONE {
        return Err(SkpError::SdkError("Failed to get entities".into()));
    }

    // 轉換根層級的面、群組、元件實例
    convert_entities(sdk, entities, None, &mut scene, &mut state)?;

    Ok(scene)
}

struct ConvertState {
    material_map: HashMap<usize, String>, // material ptr → material id
    mesh_counter: usize,
    instance_counter: usize,
    group_counter: usize,
}

/// 遞迴轉換 entities（面 → mesh，群組 → group，元件 → instance）
fn convert_entities(
    sdk: &SkpSdk,
    entities: SUEntitiesRef,
    parent_group_id: Option<&str>,
    scene: &mut SkpScene,
    state: &mut ConvertState,
) -> Result<(), SkpError> {
    // ── 收集 faces → mesh ──
    let faces = get_faces(sdk, entities)?;
    if !faces.is_empty() {
        let mesh = faces_to_mesh(sdk, &faces, state)?;
        let mesh_id = mesh.id.clone();
        scene.meshes.push(mesh);

        // 建立 instance
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
            // 加入群組的 children
            if let Some(g) = scene.groups.iter_mut().find(|g| g.id == gid) {
                g.children.push(inst.id.clone());
            }
        }
        scene.instances.push(inst);
    }

    // ── 收集 groups ──
    let groups = get_groups(sdk, entities)?;
    for group_ref in &groups {
        state.group_counter += 1;
        let gid = format!("grp_{}", state.group_counter);
        let name = sdk.read_name(|s| unsafe { (sdk.fn_group_get_name)(*group_ref, s) });

        scene.groups.push(SkpGroup {
            id: gid.clone(),
            name: if name.is_empty() { format!("Group_{}", state.group_counter) } else { name },
            children: Vec::new(),
            parent_id: parent_group_id.map(|s| s.to_string()),
        });

        // 取得群組的 entities 遞迴
        let mut group_entities = SUEntitiesRef { ptr: std::ptr::null_mut() };
        unsafe { (sdk.fn_group_get_entities)(*group_ref, &mut group_entities) };
        convert_entities(sdk, group_entities, Some(&gid), scene, state)?;
    }

    // ── 收集 component instances ──
    let instances = get_component_instances(sdk, entities)?;
    for inst_ref in &instances {
        // 取得 definition
        let mut def_ref = SUComponentDefinitionRef { ptr: std::ptr::null_mut() };
        unsafe { (sdk.fn_comp_inst_get_definition)(*inst_ref, &mut def_ref) };

        let def_name = sdk.read_name(|s| unsafe { (sdk.fn_comp_def_get_name)(def_ref, s) });
        let inst_name = sdk.read_name(|s| unsafe { (sdk.fn_comp_inst_get_name)(*inst_ref, s) });

        // 取得 transform
        let mut transform = SUTransformation { values: [0.0; 16] };
        unsafe { (sdk.fn_comp_inst_get_transform)(*inst_ref, &mut transform) };

        // 取得 definition 的 entities → mesh
        let mut def_entities = SUEntitiesRef { ptr: std::ptr::null_mut() };
        unsafe { (sdk.fn_comp_def_get_entities)(def_ref, &mut def_entities) };

        let def_faces = get_faces(sdk, def_entities)?;
        if !def_faces.is_empty() {
            let mesh = faces_to_mesh(sdk, &def_faces, state)?;
            let mesh_id = mesh.id.clone();
            let def_id = format!("comp_{}", def_ref.ptr as usize);

            // 加入 component def（如果還沒有）
            if !scene.component_defs.iter().any(|d| d.id == def_id) {
                scene.component_defs.push(SkpComponentDef {
                    id: def_id.clone(),
                    name: def_name.clone(),
                    mesh_ids: vec![mesh_id.clone()],
                    instance_count: 0,
                });
            }
            // 增加 instance count
            if let Some(d) = scene.component_defs.iter_mut().find(|d| d.id == def_id) {
                d.instance_count += 1;
            }

            scene.meshes.push(mesh);

            // Instance with transform
            state.instance_counter += 1;
            let transform_f32: [f32; 16] = std::array::from_fn(|i| transform.values[i] as f32);
            let inst = SkpInstance {
                id: format!("inst_{}", state.instance_counter),
                mesh_id,
                component_def_id: Some(def_id),
                transform: transform_f32,
                name: if inst_name.is_empty() { def_name } else { inst_name },
                layer: String::new(),
            };
            if let Some(gid) = parent_group_id {
                if let Some(g) = scene.groups.iter_mut().find(|g| g.id == gid) {
                    g.children.push(inst.id.clone());
                }
            }
            scene.instances.push(inst);
        }
    }

    Ok(())
}

// ─── 輔助函式 ─────────────────────────────────────────────────────────────

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

/// 將多個 face 合成一個 mesh
fn faces_to_mesh(sdk: &SkpSdk, faces: &[SUFaceRef], state: &mut ConvertState) -> Result<SkpMesh, SkpError> {
    state.mesh_counter += 1;
    let mesh_id = format!("mesh_{}", state.mesh_counter);
    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    for face in faces {
        // 取得頂點數
        let mut vert_count = 0usize;
        unsafe { (sdk.fn_face_get_num_vertices)(*face, &mut vert_count) };
        if vert_count < 3 { continue; }

        // 取得頂點
        let mut verts = vec![SUVertexRef { ptr: std::ptr::null_mut() }; vert_count];
        let mut actual = 0usize;
        unsafe { (sdk.fn_face_get_vertices)(*face, vert_count, verts.as_mut_ptr(), &mut actual) };

        // 取得法線
        let mut normal = SUVector3D { x: 0.0, y: 0.0, z: 0.0 };
        unsafe { (sdk.fn_face_get_normal)(*face, &mut normal) };

        let base = vertices.len() as u32;
        for v in &verts[..actual] {
            let mut pos = SUPoint3D { x: 0.0, y: 0.0, z: 0.0 };
            unsafe { (sdk.fn_vertex_get_position)(*v, &mut pos) };
            // SU 用 inch，轉 mm (* 25.4)
            vertices.push([
                (pos.x * 25.4) as f32,
                (pos.z * 25.4) as f32, // SU Z → our Y
                (pos.y * 25.4) as f32, // SU Y → our Z（negated for right-hand）
            ]);
            normals.push([
                normal.x as f32,
                normal.z as f32,
                normal.y as f32,
            ]);
        }

        // Fan triangulation
        for i in 1..(actual as u32 - 1) {
            indices.push(base);
            indices.push(base + i);
            indices.push(base + i + 1);
        }
    }

    Ok(SkpMesh {
        id: mesh_id,
        name: format!("Mesh_{}", state.mesh_counter),
        vertices,
        normals,
        indices,
        material_id: None, // TODO: 從 face material 取得
    })
}

fn identity_transform() -> [f32; 16] {
    [1.0, 0.0, 0.0, 0.0,
     0.0, 1.0, 0.0, 0.0,
     0.0, 0.0, 1.0, 0.0,
     0.0, 0.0, 0.0, 1.0]
}

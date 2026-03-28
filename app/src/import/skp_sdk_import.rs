//! SKP SDK → Kolibri Scene 轉接層
//! 將 kolibri_skp::SkpScene 轉為 UnifiedIR，再建入 Scene
//! 獨立於 GPT 的 skp_importer.rs / sketchup_bridge_backend.rs

use super::unified_ir::*;

/// 將 SDK 匯入的 SkpScene 轉為 UnifiedIR
pub fn skp_scene_to_ir(skp: &kolibri_skp::SkpScene, source_file: &str) -> UnifiedIR {
    let unit_scale = match skp.units.as_str() {
        "mm" => 1.0_f32,
        "cm" => 10.0,
        "m" => 1000.0,
        "foot" | "feet" => 304.8,
        _ => 25.4, // inch (SketchUp default)
    };

    let mut ir = UnifiedIR {
        source_format: "skp-sdk".into(),
        source_file: source_file.to_string(),
        units: "mm".into(),
        ..Default::default()
    };

    // Meshes（SDK 已做 inch→mm 轉換，但 UnifiedIR 的 build 流程不會再轉）
    for mesh in &skp.meshes {
        ir.meshes.push(IrMesh {
            id: mesh.id.clone(),
            name: mesh.name.clone(),
            vertices: mesh.vertices.clone(), // 已經是 mm
            normals: mesh.normals.clone(),
            indices: mesh.indices.clone(),
            material_id: mesh.material_id.clone(),
        });
    }

    // Instances
    for inst in &skp.instances {
        ir.instances.push(IrInstance {
            id: inst.id.clone(),
            mesh_id: inst.mesh_id.clone(),
            component_def_id: inst.component_def_id.clone(),
            transform: inst.transform,
            name: inst.name.clone(),
            layer: inst.layer.clone(),
        });
    }

    // Groups
    for group in &skp.groups {
        ir.groups.push(IrGroup {
            id: group.id.clone(),
            name: group.name.clone(),
            children: group.children.clone(),
            parent_id: group.parent_id.clone(),
        });
    }

    // Component definitions
    for def in &skp.component_defs {
        ir.component_defs.push(IrComponentDef {
            id: def.id.clone(),
            name: def.name.clone(),
            mesh_ids: def.mesh_ids.clone(),
            instance_count: def.instance_count,
        });
    }

    // Materials
    for mat in &skp.materials {
        ir.materials.push(IrMaterial {
            id: mat.id.clone(),
            name: mat.name.clone(),
            color: mat.color,
            texture_path: mat.texture_path.clone(),
            opacity: mat.opacity,
        });
    }

    // Stats
    ir.stats = ImportStats {
        mesh_count: ir.meshes.len(),
        face_count: ir.meshes.iter().map(|m| m.indices.len() / 3).sum(),
        vertex_count: ir.meshes.iter().map(|m| m.vertices.len()).sum(),
        instance_count: ir.instances.len(),
        group_count: ir.groups.len(),
        component_count: ir.component_defs.len(),
        material_count: ir.materials.len(),
        member_count: 0,
    };

    ir.debug_report = vec![
        "[SKP SDK Import]".into(),
        format!("Source: {}", source_file),
        format!("Meshes: {}", ir.stats.mesh_count),
        format!("Vertices: {}", ir.stats.vertex_count),
        format!("Triangles: {}", ir.stats.face_count),
        format!("Instances: {}", ir.stats.instance_count),
        format!("Groups: {}", ir.stats.group_count),
        format!("Components: {}", ir.stats.component_count),
        format!("Materials: {}", ir.stats.material_count),
    ];

    ir
}

/// 一鍵匯入：SDK 讀取 → IR → Scene
/// 回傳匯入的物件數量
pub fn import_skp_to_scene(
    scene: &mut crate::scene::Scene,
    path: &str,
) -> Result<(usize, Vec<String>), String> {
    // 檢查 SDK
    if !kolibri_skp::sdk_available() {
        return Err("SketchUp SDK DLL 不可用".into());
    }

    // 讀取 SKP
    let skp_scene = kolibri_skp::import_skp(path)
        .map_err(|e| format!("SKP SDK 讀取失敗: {}", e))?;

    // 轉為 IR
    let ir = skp_scene_to_ir(&skp_scene, path);

    // 建入 Scene
    let result = super::import_manager::build_scene_from_ir(scene, &ir);

    Ok((result.meshes + result.columns + result.beams + result.plates, result.ids))
}

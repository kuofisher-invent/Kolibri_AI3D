//! SketchUp SKP file importer
//!
//! Preferred path:
//! 1. Install a small Ruby bridge into the user's SketchUp Plugins folder.
//! 2. Launch SketchUp with the target `.skp`.
//! 3. Let the Ruby bridge export a structured JSON scene graph.
//! 4. Convert that JSON into Kolibri `UnifiedIR`.
//!
//! A coarse heuristic fallback remains for environments where SketchUp is not
//! available, but the bridge path is the only route that preserves components,
//! groups, instances, materials, and transforms with reasonable fidelity.

use super::unified_ir::*;
use std::collections::HashMap;
use super::skp_backend::SkpBackend;
use super::sketchup_bridge_backend::SketchUpBridgeBackend;

/// Import a SketchUp .skp file
pub fn import_skp(path: &str) -> Result<UnifiedIR, String> {
    let backends: [&dyn SkpBackend; 1] = [&SketchUpBridgeBackend];
    let mut backend_errors = Vec::new();
    for backend in backends {
        match backend.import(path) {
            Ok(mut ir) => {
                ir.debug_report.insert(1, format!("Importer backend: {}", backend.name()));
                return Ok(ir);
            }
            Err(e) => backend_errors.push(format!("{}: {}", backend.name(), e)),
        }
    }

    // Heuristic fallback: this remains useful for development, but it is not
    // considered a complete SKP parser.
    let file = std::fs::File::open(path).map_err(|e| format!("無法開啟 SKP: {}", e))?;
    if let Ok(mut archive) = zip::ZipArchive::new(file) {
        return import_skp_zip(&mut archive, path).map_err(|fallback_err| {
            format!("{}\nFallback failed: {}", backend_errors.join("\n"), fallback_err)
        });
    }
    let mut msg = backend_errors.join("\n");
    if !msg.is_empty() {
        msg.push('\n');
    }
    msg.push_str(&import_skp_legacy(path)?);
    Err(msg)
}

fn import_skp_zip(archive: &mut zip::ZipArchive<std::fs::File>, path: &str) -> Result<UnifiedIR, String> {
    let mut ir = UnifiedIR {
        source_format: "skp".into(),
        source_file: path.into(),
        units: "mm".into(),
        ..Default::default()
    };

    let mut entry_names = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            entry_names.push((i, entry.name().to_string(), entry.size()));
        }
    }

    let mut found_geometry = false;
    for (idx, name, _size) in &entry_names {
        if name.ends_with(".bin") || name.contains("geometry") || name.contains("model") {
            if let Ok(mut entry) = archive.by_index(*idx) {
                let mut data = Vec::new();
                use std::io::Read;
                let _ = entry.read_to_end(&mut data);
                if !data.is_empty() {
                    if let Some(mesh) = try_parse_skp_binary(&data, name) {
                        ir.meshes.push(mesh);
                        found_geometry = true;
                    }
                }
            }
        }
    }

    if !found_geometry {
        return Err(format!(
            "SKP bridge 無法使用，且 heuristic ZIP fallback 也未找到幾何。ZIP entries: {}",
            entry_names.len()
        ));
    }

    ir.stats.mesh_count = ir.meshes.len();
    ir.stats.vertex_count = ir.meshes.iter().map(|m| m.vertices.len()).sum();
    ir.stats.face_count = ir.meshes.iter().map(|m| m.indices.len() / 3).sum();
    Ok(ir)
}

fn try_parse_skp_binary(data: &[u8], _name: &str) -> Option<IrMesh> {
    if data.len() < 36 {
        return None;
    }

    let mut vertices = Vec::new();
    let mut i = 0usize;
    while i + 12 <= data.len() {
        let x = f32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        let y = f32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]);
        let z = f32::from_le_bytes([data[i + 8], data[i + 9], data[i + 10], data[i + 11]]);
        if x.is_finite() && y.is_finite() && z.is_finite()
            && x.abs() < 1_000_000.0 && y.abs() < 1_000_000.0 && z.abs() < 1_000_000.0
        {
            vertices.push([x, y, z]);
        }
        i += 12;
    }

    if vertices.len() < 3 {
        return None;
    }

    let mut indices = Vec::new();
    for tri in 1..vertices.len() - 1 {
        indices.push(0);
        indices.push(tri as u32);
        indices.push((tri + 1) as u32);
    }

    Some(IrMesh {
        id: format!("skp_mesh_{}", vertices.len()),
        name: "SKP Geometry".into(),
        vertices,
        normals: Vec::new(),
        indices,
        material_id: None,
    })
}

fn import_skp_legacy(_path: &str) -> Result<String, String> {
    Ok("無法完整解析此 SKP。請確認本機已安裝 SketchUp 2025，Kolibri 會使用 Ruby bridge 匯出 scene graph。".into())
}

/// Import OBJ file as a SKP workflow alternative
pub fn import_obj_to_ir(path: &str) -> Result<UnifiedIR, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("讀取 OBJ 失敗: {}", e))?;

    let mut ir = UnifiedIR {
        source_format: "obj".into(),
        source_file: path.into(),
        units: "mm".into(),
        ..Default::default()
    };

    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut current_group = "default".to_string();
    let mut group_faces: HashMap<String, Vec<[u32; 3]>> = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("v ") {
            let parts: Vec<f32> = line[2..].split_whitespace().filter_map(|s| s.parse().ok()).collect();
            if parts.len() >= 3 {
                vertices.push([parts[0], parts[1], parts[2]]);
            }
        } else if line.starts_with("vn ") {
            let parts: Vec<f32> = line[3..].split_whitespace().filter_map(|s| s.parse().ok()).collect();
            if parts.len() >= 3 {
                normals.push([parts[0], parts[1], parts[2]]);
            }
        } else if line.starts_with("f ") {
            let face_verts: Vec<u32> = line[2..].split_whitespace()
                .filter_map(|s| {
                    let idx_str = s.split('/').next()?;
                    let idx: i32 = idx_str.parse().ok()?;
                    Some(if idx > 0 { (idx - 1) as u32 } else { 0 })
                })
                .collect();
            if face_verts.len() >= 3 {
                for fi in 1..face_verts.len() - 1 {
                    group_faces.entry(current_group.clone()).or_default()
                        .push([face_verts[0], face_verts[fi], face_verts[fi + 1]]);
                }
            }
        } else if line.starts_with("g ") || line.starts_with("o ") {
            current_group = line[2..].trim().to_string();
        }
    }

    if vertices.is_empty() {
        return Err("OBJ 沒有可用頂點資料".into());
    }

    if normals.len() != vertices.len() {
        normals = vec![[0.0, 1.0, 0.0]; vertices.len()];
    }

    let mut mesh_index = 0usize;
    for (group_name, tris) in group_faces {
        if tris.is_empty() {
            continue;
        }
        let mesh_id = format!("obj_mesh_{}", mesh_index);
        mesh_index += 1;

        let mut local_vertices = Vec::with_capacity(tris.len() * 3);
        let mut local_indices = Vec::with_capacity(tris.len() * 3);
        for tri in tris {
            let base = local_vertices.len() as u32;
            local_vertices.push(vertices[tri[0] as usize]);
            local_vertices.push(vertices[tri[1] as usize]);
            local_vertices.push(vertices[tri[2] as usize]);
            local_indices.extend_from_slice(&[base, base + 1, base + 2]);
        }

        ir.meshes.push(IrMesh {
            id: mesh_id.clone(),
            name: group_name.clone(),
            vertices: local_vertices,
            normals: Vec::new(),
            indices: local_indices,
            material_id: None,
        });

        let inst_id = format!("obj_inst_{}", mesh_index);
        ir.instances.push(IrInstance {
            id: inst_id.clone(),
            mesh_id: mesh_id.clone(),
            component_def_id: None,
            transform: [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ],
            name: group_name.clone(),
            layer: String::new(),
        });

        ir.groups.push(IrGroup {
            id: format!("grp_{}", group_name),
            name: group_name.clone(),
            children: vec![inst_id],
            parent_id: None,
        });
    }

    ir.stats.vertex_count = ir.meshes.iter().map(|m| m.vertices.len()).sum();
    ir.stats.face_count = ir.meshes.iter().map(|m| m.indices.len() / 3).sum();
    ir.stats.mesh_count = ir.meshes.len();
    ir.stats.instance_count = ir.instances.len();
    ir.stats.group_count = ir.groups.len();

    Ok(ir)
}

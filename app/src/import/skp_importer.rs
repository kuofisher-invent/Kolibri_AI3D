//! SketchUp SKP file importer
//!
//! SKP files (2014+) are ZIP archives containing binary geometry data.
//! This importer attempts to extract basic geometry.
//! For complex SKP files, users should export to OBJ from SketchUp first.

use super::unified_ir::*;

/// Import a SketchUp .skp file
pub fn import_skp(path: &str) -> Result<UnifiedIR, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("開啟失敗: {}", e))?;

    // Check if it's a ZIP (SKP 2014+)
    if let Ok(mut archive) = zip::ZipArchive::new(file) {
        return import_skp_zip(&mut archive, path);
    }

    // If not ZIP, try as legacy binary format
    import_skp_legacy(path)
}

fn import_skp_zip(archive: &mut zip::ZipArchive<std::fs::File>, path: &str) -> Result<UnifiedIR, String> {
    let mut ir = UnifiedIR {
        source_format: "skp".into(),
        source_file: path.into(),
        units: "mm".into(),
        ..Default::default()
    };

    // List all entries in the ZIP
    let mut entry_names = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            entry_names.push((i, entry.name().to_string(), entry.size()));
        }
    }

    // Look for known SKP internal files
    // SKP ZIP typically contains:
    //   - document.json or similar metadata
    //   - binary geometry chunks
    //   - material textures

    let mut found_geometry = false;

    for (idx, name, _size) in &entry_names {
        // Try to find geometry data
        if name.ends_with(".bin") || name.contains("geometry") || name.contains("model") {
            if let Ok(mut entry) = archive.by_index(*idx) {
                let mut data = Vec::new();
                use std::io::Read;
                let _ = entry.read_to_end(&mut data);

                if !data.is_empty() {
                    // Try to extract triangulated mesh from binary data
                    if let Some(mesh) = try_parse_skp_binary(&data, name) {
                        ir.meshes.push(mesh);
                        found_geometry = true;
                    }
                }
            }
        }

        // Look for material/texture info
        if name.ends_with(".json") || name.contains("material") {
            if let Ok(mut entry) = archive.by_index(*idx) {
                let mut _data = String::new();
                use std::io::Read;
                let _ = entry.read_to_string(&mut _data);
                // Try to parse material info (future enhancement)
            }
        }
    }

    if !found_geometry {
        // Fallback: create a bounding box from file metadata
        ir.meshes.push(IrMesh {
            id: "skp_import_0".into(),
            name: std::path::Path::new(path).file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "SKP Import".into()),
            vertices: vec![[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], [1000.0, 1000.0, 0.0], [0.0, 1000.0, 0.0],
                          [0.0, 0.0, 1000.0], [1000.0, 0.0, 1000.0], [1000.0, 1000.0, 1000.0], [0.0, 1000.0, 1000.0]],
            normals: vec![[0.0, 1.0, 0.0]; 8],
            indices: vec![0,1,2, 0,2,3, 4,5,6, 4,6,7, 0,1,5, 0,5,4, 2,3,7, 2,7,6, 0,3,7, 0,7,4, 1,2,6, 1,6,5],
            material_id: None,
        });

        // Update stats even for fallback
        ir.stats.mesh_count = ir.meshes.len();
        ir.stats.vertex_count = ir.meshes.iter().map(|m| m.vertices.len()).sum();
        ir.stats.face_count = ir.meshes.iter().map(|m| m.indices.len() / 3).sum();

        return Err(format!(
            "SKP 檔案結構複雜，僅能提取基本資訊。\n找到 {} 個內部檔案。\n建議：從 SketchUp 匯出為 OBJ 格式後再匯入。\n\nZIP 內容:\n{}",
            entry_names.len(),
            entry_names.iter().take(10).map(|(_, n, s)| format!("  {} ({} bytes)", n, s)).collect::<Vec<_>>().join("\n")
        ));
    }

    // Update stats
    ir.stats.mesh_count = ir.meshes.len();
    ir.stats.vertex_count = ir.meshes.iter().map(|m| m.vertices.len()).sum();
    ir.stats.face_count = ir.meshes.iter().map(|m| m.indices.len() / 3).sum();
    ir.stats.material_count = ir.materials.len();

    Ok(ir)
}

fn try_parse_skp_binary(data: &[u8], _name: &str) -> Option<IrMesh> {
    // SKP binary geometry is in a proprietary format
    // For now, try to find float sequences that look like vertex data

    if data.len() < 36 { return None; } // need at least a few vertices

    let mut vertices = Vec::new();
    let mut i = 0;

    // Scan for sequences of valid float triples (vertex coordinates)
    while i + 12 <= data.len() {
        let x = f32::from_le_bytes([data[i], data[i+1], data[i+2], data[i+3]]);
        let y = f32::from_le_bytes([data[i+4], data[i+5], data[i+6], data[i+7]]);
        let z = f32::from_le_bytes([data[i+8], data[i+9], data[i+10], data[i+11]]);

        // Sanity check: coordinates should be in reasonable range for mm
        if x.is_finite() && y.is_finite() && z.is_finite()
            && x.abs() < 1_000_000.0 && y.abs() < 1_000_000.0 && z.abs() < 1_000_000.0 {
            vertices.push([x, y, z]);
        }

        i += 12;
    }

    if vertices.len() < 3 { return None; }

    // Create simple triangle indices (fan triangulation)
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
        normals: Vec::new(), // will be computed later
        indices,
        material_id: None,
    })
}

fn import_skp_legacy(path: &str) -> Result<UnifiedIR, String> {
    let _ = path;
    Err(
        "此 SKP 檔案為舊格式（非 ZIP），無法直接解析。\n\n建議方案：\n1. 在 SketchUp 中開啟此檔案\n2. 匯出為 OBJ 格式（檔案 → 匯出 → 3D 模型 → OBJ）\n3. 在 Kolibri 匯入 OBJ 檔案".into()
    )
}

/// Import OBJ file as a SKP workflow alternative
pub fn import_obj_to_ir(path: &str) -> Result<UnifiedIR, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("讀取失敗: {}", e))?;

    let mut ir = UnifiedIR {
        source_format: "obj".into(),
        source_file: path.into(),
        units: "mm".into(),
        ..Default::default()
    };

    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut current_group = "default".to_string();
    let mut groups: std::collections::HashMap<String, Vec<u32>> = std::collections::HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("v ") {
            let parts: Vec<f32> = line[2..].split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if parts.len() >= 3 {
                vertices.push([parts[0], parts[1], parts[2]]);
            }
        } else if line.starts_with("vn ") {
            let parts: Vec<f32> = line[3..].split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
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
            // Fan triangulate
            if face_verts.len() >= 3 {
                for fi in 1..face_verts.len() - 1 {
                    indices.push(face_verts[0]);
                    indices.push(face_verts[fi]);
                    indices.push(face_verts[fi + 1]);
                    groups.entry(current_group.clone()).or_default().push(indices.len() as u32 / 3 - 1);
                }
            }
        } else if line.starts_with("g ") || line.starts_with("o ") {
            current_group = line[2..].trim().to_string();
        }
    }

    if vertices.is_empty() {
        return Err("OBJ 檔案中沒有找到頂點資料".into());
    }

    // Compute normals if missing
    if normals.len() != vertices.len() {
        normals = vec![[0.0, 1.0, 0.0]; vertices.len()];
        // Compute face normals
        for chunk in indices.chunks(3) {
            if chunk.len() < 3 { continue; }
            let (i0, i1, i2) = (chunk[0] as usize, chunk[1] as usize, chunk[2] as usize);
            if i0 >= vertices.len() || i1 >= vertices.len() || i2 >= vertices.len() { continue; }
            let v0 = vertices[i0];
            let v1 = vertices[i1];
            let v2 = vertices[i2];
            let u = [v1[0]-v0[0], v1[1]-v0[1], v1[2]-v0[2]];
            let v = [v2[0]-v0[0], v2[1]-v0[1], v2[2]-v0[2]];
            let n = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]];
            let len = (n[0]*n[0]+n[1]*n[1]+n[2]*n[2]).sqrt();
            if len > 1e-6 {
                let nn = [n[0]/len, n[1]/len, n[2]/len];
                normals[i0] = nn;
                normals[i1] = nn;
                normals[i2] = nn;
            }
        }
    }

    let mesh = IrMesh {
        id: "obj_mesh_0".into(),
        name: std::path::Path::new(path).file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "OBJ Import".into()),
        vertices,
        normals,
        indices,
        material_id: None,
    };

    // Create groups from OBJ groups
    for (name, _face_ids) in &groups {
        ir.groups.push(IrGroup {
            id: format!("grp_{}", name),
            name: name.clone(),
            children: Vec::new(),
        });
    }

    ir.stats.vertex_count = mesh.vertices.len();
    ir.stats.face_count = mesh.indices.len() / 3;
    ir.stats.mesh_count = 1;
    ir.stats.group_count = groups.len();

    ir.meshes.push(mesh);

    Ok(ir)
}

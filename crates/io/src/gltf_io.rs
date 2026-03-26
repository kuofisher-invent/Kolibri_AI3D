//! Minimal GLTF export (JSON .gltf + .bin)

use kolibri_core::scene::{Scene, Shape};

/// Export scene to GLTF (JSON .gltf + separate .bin buffer)
pub fn export_gltf(scene: &Scene, path: &str) -> Result<(), String> {
    // Collect all triangles with positions and normals
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for obj in scene.objects.values() {
        if !obj.visible { continue; }
        let p = obj.position;
        let base = positions.len() as u32;

        match &obj.shape {
            Shape::Box { width, height, depth } => {
                let (w, h, d) = (*width, *height, *depth);
                // 24 vertices (4 per face with unique normals)
                let face_data: &[([f32;3], [[f32;3];4])] = &[
                    // front (z-)
                    ([0.0,0.0,-1.0], [[p[0],p[1],p[2]], [p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]], [p[0],p[1]+h,p[2]]]),
                    // back (z+)
                    ([0.0,0.0,1.0], [[p[0]+w,p[1],p[2]+d], [p[0],p[1],p[2]+d], [p[0],p[1]+h,p[2]+d], [p[0]+w,p[1]+h,p[2]+d]]),
                    // top (y+)
                    ([0.0,1.0,0.0], [[p[0],p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d]]),
                    // bottom (y-)
                    ([0.0,-1.0,0.0], [[p[0],p[1],p[2]+d], [p[0]+w,p[1],p[2]+d], [p[0]+w,p[1],p[2]], [p[0],p[1],p[2]]]),
                    // left (x-)
                    ([-1.0,0.0,0.0], [[p[0],p[1],p[2]+d], [p[0],p[1],p[2]], [p[0],p[1]+h,p[2]], [p[0],p[1]+h,p[2]+d]]),
                    // right (x+)
                    ([1.0,0.0,0.0], [[p[0]+w,p[1],p[2]], [p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d], [p[0]+w,p[1]+h,p[2]]]),
                ];
                let fb = positions.len() as u32;
                for (n, verts) in face_data {
                    for v in verts {
                        positions.push(*v);
                        normals.push(*n);
                    }
                }
                for i in 0..6u32 {
                    let b = fb + i * 4;
                    indices.extend_from_slice(&[b, b+1, b+2, b, b+2, b+3]);
                }
            }
            Shape::Cylinder { radius, height, segments } => {
                let segs = *segments as usize;
                let r = *radius;
                let h = *height;
                let cx = p[0] + r;
                let cz = p[2] + r;
                for i in 0..segs {
                    let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
                    let a1 = ((i+1) as f32 / segs as f32) * std::f32::consts::TAU;
                    let (s0,c0) = a0.sin_cos();
                    let (s1,c1) = a1.sin_cos();
                    let b0 = [cx+r*c0, p[1], cz+r*s0];
                    let b1 = [cx+r*c1, p[1], cz+r*s1];
                    let t0 = [cx+r*c0, p[1]+h, cz+r*s0];
                    let t1 = [cx+r*c1, p[1]+h, cz+r*s1];

                    // Side quad (2 triangles)
                    let sb = positions.len() as u32;
                    let n_side = [(c0+c1)*0.5, 0.0, (s0+s1)*0.5];
                    positions.extend_from_slice(&[b0, b1, t1, t0]);
                    normals.extend_from_slice(&[n_side, n_side, n_side, n_side]);
                    indices.extend_from_slice(&[sb, sb+1, sb+2, sb, sb+2, sb+3]);

                    // Bottom triangle
                    let bb = positions.len() as u32;
                    let center_b = [cx, p[1], cz];
                    positions.extend_from_slice(&[center_b, b1, b0]);
                    normals.extend_from_slice(&[[0.0,-1.0,0.0]; 3]);
                    indices.extend_from_slice(&[bb, bb+1, bb+2]);

                    // Top triangle
                    let tb = positions.len() as u32;
                    let center_t = [cx, p[1]+h, cz];
                    positions.extend_from_slice(&[center_t, t0, t1]);
                    normals.extend_from_slice(&[[0.0,1.0,0.0]; 3]);
                    indices.extend_from_slice(&[tb, tb+1, tb+2]);
                }
            }
            Shape::Sphere { radius, segments } => {
                let segs = *segments as usize;
                let rings = segs / 2;
                let r = *radius;
                let cx = p[0]+r; let cy = p[1]+r; let cz = p[2]+r;
                for j in 0..rings {
                    let phi0 = (j as f32 / rings as f32) * std::f32::consts::PI;
                    let phi1 = ((j+1) as f32 / rings as f32) * std::f32::consts::PI;
                    for i in 0..segs {
                        let th0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
                        let th1 = ((i+1) as f32 / segs as f32) * std::f32::consts::TAU;
                        let mk = |phi: f32, th: f32| -> ([f32;3], [f32;3]) {
                            let nx = phi.sin()*th.cos();
                            let ny = phi.cos();
                            let nz = phi.sin()*th.sin();
                            ([cx+r*nx, cy+r*ny, cz+r*nz], [nx, ny, nz])
                        };
                        let (p00, n00) = mk(phi0, th0);
                        let (p10, n10) = mk(phi0, th1);
                        let (p01, n01) = mk(phi1, th0);
                        let (p11, n11) = mk(phi1, th1);
                        let sb = positions.len() as u32;
                        positions.extend_from_slice(&[p00, p10, p11, p01]);
                        normals.extend_from_slice(&[n00, n10, n11, n01]);
                        indices.extend_from_slice(&[sb, sb+1, sb+2, sb, sb+2, sb+3]);
                    }
                }
            }
            _ => { let _ = base; }
        }
    }

    if positions.is_empty() { return Err("No geometry to export".into()); }

    write_gltf_json(path, &positions, &normals, &indices)
}

fn write_gltf_json(path: &str, positions: &[[f32; 3]], normals: &[[f32; 3]], indices: &[u32]) -> Result<(), String> {
    let bin_path = path.replace(".gltf", ".bin").replace(".glb", ".bin");

    // Write binary buffer
    let mut bin = Vec::new();
    // Indices
    for &i in indices { bin.extend_from_slice(&i.to_le_bytes()); }
    let idx_byte_len = indices.len() * 4;
    // Pad to 4 bytes
    while bin.len() % 4 != 0 { bin.push(0); }
    let pos_offset = bin.len();
    // Positions (convert mm to meters)
    for p in positions {
        for &v in p { bin.extend_from_slice(&(v / 1000.0).to_le_bytes()); }
    }
    let pos_byte_len = positions.len() * 12;
    let norm_offset = bin.len();
    // Normals
    for n in normals {
        for &v in n { bin.extend_from_slice(&v.to_le_bytes()); }
    }
    let norm_byte_len = normals.len() * 12;

    std::fs::write(&bin_path, &bin).map_err(|e| e.to_string())?;

    // Compute bounds
    let mut min_p = [f32::MAX; 3];
    let mut max_p = [f32::MIN; 3];
    for p in positions {
        for i in 0..3 {
            min_p[i] = min_p[i].min(p[i] / 1000.0);
            max_p[i] = max_p[i].max(p[i] / 1000.0);
        }
    }

    let bin_filename = std::path::Path::new(&bin_path).file_name()
        .map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "scene.bin".into());

    let json = format!(r#"{{
  "asset": {{ "version": "2.0", "generator": "Kolibri_Ai3D" }},
  "scene": 0,
  "scenes": [{{ "nodes": [0] }}],
  "nodes": [{{ "mesh": 0 }}],
  "meshes": [{{ "primitives": [{{ "attributes": {{ "POSITION": 1, "NORMAL": 2 }}, "indices": 0 }}] }}],
  "accessors": [
    {{ "bufferView": 0, "componentType": 5125, "count": {idx_count}, "type": "SCALAR" }},
    {{ "bufferView": 1, "componentType": 5126, "count": {pos_count}, "type": "VEC3", "min": [{min0},{min1},{min2}], "max": [{max0},{max1},{max2}] }},
    {{ "bufferView": 2, "componentType": 5126, "count": {norm_count}, "type": "VEC3" }}
  ],
  "bufferViews": [
    {{ "buffer": 0, "byteOffset": 0, "byteLength": {idx_len}, "target": 34963 }},
    {{ "buffer": 0, "byteOffset": {pos_off}, "byteLength": {pos_len}, "target": 34962 }},
    {{ "buffer": 0, "byteOffset": {norm_off}, "byteLength": {norm_len}, "target": 34962 }}
  ],
  "buffers": [{{ "uri": "{bin_uri}", "byteLength": {bin_len} }}]
}}"#,
        idx_count = indices.len(), pos_count = positions.len(), norm_count = normals.len(),
        min0 = min_p[0], min1 = min_p[1], min2 = min_p[2],
        max0 = max_p[0], max1 = max_p[1], max2 = max_p[2],
        idx_len = idx_byte_len, pos_off = pos_offset, pos_len = pos_byte_len,
        norm_off = norm_offset, norm_len = norm_byte_len,
        bin_uri = bin_filename, bin_len = bin.len(),
    );

    std::fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

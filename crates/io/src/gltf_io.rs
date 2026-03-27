//! glTF 2.0 export — per-object nodes with PBR materials

use kolibri_core::scene::{Scene, Shape};
use std::io::Write;

/// Per-object mesh data for glTF export
struct GltfMesh {
    name: String,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
    color: [f32; 4],
    roughness: f32,
    metallic: f32,
}

/// Export scene to glTF (JSON .gltf + separate .bin)
pub fn export_gltf(scene: &Scene, path: &str) -> Result<(), String> {
    let mut meshes: Vec<GltfMesh> = Vec::new();

    for obj in scene.objects.values() {
        if !obj.visible { continue; }
        let p = obj.position;
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();

        match &obj.shape {
            Shape::Box { width, height, depth } => {
                let (w, h, d) = (*width, *height, *depth);
                let face_data: &[([f32;3], [[f32;3];4])] = &[
                    ([0.0,0.0,-1.0], [[p[0],p[1],p[2]], [p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]], [p[0],p[1]+h,p[2]]]),
                    ([0.0,0.0,1.0],  [[p[0]+w,p[1],p[2]+d], [p[0],p[1],p[2]+d], [p[0],p[1]+h,p[2]+d], [p[0]+w,p[1]+h,p[2]+d]]),
                    ([0.0,1.0,0.0],  [[p[0],p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d]]),
                    ([0.0,-1.0,0.0], [[p[0],p[1],p[2]+d], [p[0]+w,p[1],p[2]+d], [p[0]+w,p[1],p[2]], [p[0],p[1],p[2]]]),
                    ([-1.0,0.0,0.0], [[p[0],p[1],p[2]+d], [p[0],p[1],p[2]], [p[0],p[1]+h,p[2]], [p[0],p[1]+h,p[2]+d]]),
                    ([1.0,0.0,0.0],  [[p[0]+w,p[1],p[2]], [p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d], [p[0]+w,p[1]+h,p[2]]]),
                ];
                for (n, verts) in face_data {
                    let b = positions.len() as u32;
                    for v in verts { positions.push(*v); normals.push(*n); }
                    indices.extend_from_slice(&[b, b+1, b+2, b, b+2, b+3]);
                }
            }
            Shape::Cylinder { radius, height, segments } => {
                let segs = *segments as usize;
                let (r, h) = (*radius, *height);
                let (cx, cz) = (p[0]+r, p[2]+r);
                for i in 0..segs {
                    let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
                    let a1 = ((i+1) as f32 / segs as f32) * std::f32::consts::TAU;
                    let (s0,c0) = a0.sin_cos();
                    let (s1,c1) = a1.sin_cos();
                    let b0 = [cx+r*c0, p[1], cz+r*s0];
                    let b1 = [cx+r*c1, p[1], cz+r*s1];
                    let t0 = [cx+r*c0, p[1]+h, cz+r*s0];
                    let t1 = [cx+r*c1, p[1]+h, cz+r*s1];
                    let ns = [(c0+c1)*0.5, 0.0, (s0+s1)*0.5];
                    let sb = positions.len() as u32;
                    positions.extend_from_slice(&[b0,b1,t1,t0]);
                    normals.extend_from_slice(&[ns;4]);
                    indices.extend_from_slice(&[sb,sb+1,sb+2, sb,sb+2,sb+3]);
                    // caps
                    let bb = positions.len() as u32;
                    positions.extend_from_slice(&[[cx,p[1],cz], b1, b0]);
                    normals.extend_from_slice(&[[0.0,-1.0,0.0];3]);
                    indices.extend_from_slice(&[bb,bb+1,bb+2]);
                    let tb = positions.len() as u32;
                    positions.extend_from_slice(&[[cx,p[1]+h,cz], t0, t1]);
                    normals.extend_from_slice(&[[0.0,1.0,0.0];3]);
                    indices.extend_from_slice(&[tb,tb+1,tb+2]);
                }
            }
            Shape::Sphere { radius, segments } => {
                let segs = *segments as usize;
                let rings = segs / 2;
                let r = *radius;
                let (cx,cy,cz) = (p[0]+r, p[1]+r, p[2]+r);
                for j in 0..rings {
                    let phi0 = (j as f32 / rings as f32) * std::f32::consts::PI;
                    let phi1 = ((j+1) as f32 / rings as f32) * std::f32::consts::PI;
                    for i in 0..segs {
                        let th0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
                        let th1 = ((i+1) as f32 / segs as f32) * std::f32::consts::TAU;
                        let mk = |phi: f32, th: f32| {
                            let (nx,ny,nz) = (phi.sin()*th.cos(), phi.cos(), phi.sin()*th.sin());
                            ([cx+r*nx, cy+r*ny, cz+r*nz], [nx, ny, nz])
                        };
                        let (p00,n00) = mk(phi0,th0); let (p10,n10) = mk(phi0,th1);
                        let (p01,n01) = mk(phi1,th0); let (p11,n11) = mk(phi1,th1);
                        let sb = positions.len() as u32;
                        positions.extend_from_slice(&[p00,p10,p11,p01]);
                        normals.extend_from_slice(&[n00,n10,n11,n01]);
                        indices.extend_from_slice(&[sb,sb+1,sb+2, sb,sb+2,sb+3]);
                    }
                }
            }
            Shape::Mesh(ref mesh) => {
                for (&fid, face) in &mesh.faces {
                    let fv = mesh.face_vertices(fid);
                    if fv.len() >= 3 {
                        let b = positions.len() as u32;
                        for v in &fv { positions.push([p[0]+v[0], p[1]+v[1], p[2]+v[2]]); normals.push(face.normal); }
                        for i in 1..fv.len()-1 { indices.extend_from_slice(&[b, b+i as u32, b+(i+1) as u32]); }
                    }
                }
            }
            _ => {}
        }

        if !positions.is_empty() {
            let c = obj.material.color();
            meshes.push(GltfMesh {
                name: obj.name.clone(),
                positions, normals, indices,
                color: [c[0], c[1], c[2], 1.0],
                roughness: obj.roughness,
                metallic: obj.metallic,
            });
        }
    }

    if meshes.is_empty() { return Err("No geometry to export".into()); }

    write_gltf(path, &meshes)
}

fn write_gltf(path: &str, meshes: &[GltfMesh]) -> Result<(), String> {
    let bin_path = path.replace(".gltf", ".bin").replace(".glb", ".bin");

    // Build binary buffer: for each mesh → indices, positions, normals
    let mut bin = Vec::new();
    struct MeshView { idx_off: usize, idx_len: usize, pos_off: usize, pos_len: usize, nor_off: usize, nor_len: usize, vert_count: usize, idx_count: usize, min_p: [f32;3], max_p: [f32;3] }
    let mut views: Vec<MeshView> = Vec::new();

    for m in meshes {
        let idx_off = bin.len();
        for &i in &m.indices { bin.extend_from_slice(&i.to_le_bytes()); }
        let idx_len = bin.len() - idx_off;
        while bin.len() % 4 != 0 { bin.push(0); }

        let pos_off = bin.len();
        let mut min_p = [f32::MAX;3]; let mut max_p = [f32::MIN;3];
        for p in &m.positions {
            for j in 0..3 { let v = p[j]/1000.0; min_p[j]=min_p[j].min(v); max_p[j]=max_p[j].max(v); }
            for &v in p { bin.extend_from_slice(&(v/1000.0).to_le_bytes()); }
        }
        let pos_len = bin.len() - pos_off;

        let nor_off = bin.len();
        for n in &m.normals { for &v in n { bin.extend_from_slice(&v.to_le_bytes()); } }
        let nor_len = bin.len() - nor_off;

        views.push(MeshView { idx_off, idx_len, pos_off, pos_len, nor_off, nor_len,
            vert_count: m.positions.len(), idx_count: m.indices.len(), min_p, max_p });
    }

    std::fs::write(&bin_path, &bin).map_err(|e| e.to_string())?;
    let bin_filename = std::path::Path::new(&bin_path).file_name()
        .map(|s| s.to_string_lossy().to_string()).unwrap_or("scene.bin".into());

    // Build JSON
    let n = meshes.len();
    let mut nodes = Vec::new();
    let mut mesh_json = Vec::new();
    let mut material_json = Vec::new();
    let mut accessor_json = Vec::new();
    let mut buffer_view_json = Vec::new();
    let node_indices: Vec<usize> = (0..n).collect();

    for (i, (m, v)) in meshes.iter().zip(views.iter()).enumerate() {
        let bv_base = i * 3;
        let ac_base = i * 3;

        // Buffer views: indices, positions, normals
        buffer_view_json.push(format!(
            r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":34963}}"#, v.idx_off, v.idx_len));
        buffer_view_json.push(format!(
            r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":34962}}"#, v.pos_off, v.pos_len));
        buffer_view_json.push(format!(
            r#"{{"buffer":0,"byteOffset":{},"byteLength":{},"target":34962}}"#, v.nor_off, v.nor_len));

        // Accessors
        accessor_json.push(format!(
            r#"{{"bufferView":{},"componentType":5125,"count":{},"type":"SCALAR"}}"#, bv_base, v.idx_count));
        accessor_json.push(format!(
            r#"{{"bufferView":{},"componentType":5126,"count":{},"type":"VEC3","min":[{},{},{}],"max":[{},{},{}]}}"#,
            bv_base+1, v.vert_count, v.min_p[0],v.min_p[1],v.min_p[2], v.max_p[0],v.max_p[1],v.max_p[2]));
        accessor_json.push(format!(
            r#"{{"bufferView":{},"componentType":5126,"count":{},"type":"VEC3"}}"#, bv_base+2, v.vert_count));

        // Material
        material_json.push(format!(
            r#"{{"name":"{}","pbrMetallicRoughness":{{"baseColorFactor":[{:.3},{:.3},{:.3},{:.3}],"metallicFactor":{:.2},"roughnessFactor":{:.2}}},"doubleSided":true}}"#,
            m.name, m.color[0], m.color[1], m.color[2], m.color[3], m.metallic, m.roughness));

        // Mesh
        mesh_json.push(format!(
            r#"{{"name":"{}","primitives":[{{"attributes":{{"POSITION":{},"NORMAL":{}}},"indices":{},"material":{}}}]}}"#,
            m.name, ac_base+1, ac_base+2, ac_base, i));

        // Node
        nodes.push(format!(r#"{{"name":"{}","mesh":{}}}"#, m.name, i));
    }

    let json = format!(r#"{{
  "asset":{{"version":"2.0","generator":"Kolibri_Ai3D"}},
  "scene":0,
  "scenes":[{{"nodes":[{}]}}],
  "nodes":[{}],
  "meshes":[{}],
  "materials":[{}],
  "accessors":[{}],
  "bufferViews":[{}],
  "buffers":[{{"uri":"{}","byteLength":{}}}]
}}"#,
        node_indices.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","),
        nodes.join(","),
        mesh_json.join(","),
        material_json.join(","),
        accessor_json.join(","),
        buffer_view_json.join(","),
        bin_filename, bin.len(),
    );

    std::fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

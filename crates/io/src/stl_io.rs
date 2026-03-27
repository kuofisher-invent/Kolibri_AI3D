//! STL file import/export

use kolibri_core::scene::{Scene, Shape, MaterialKind};
use std::io::Write;

/// Export scene to binary STL (mm)
pub fn export_stl(scene: &Scene, path: &str) -> Result<(), String> {
    export_stl_options(scene, path, 1.0, false)
}

/// Export STL with options: scale (1.0=mm, 0.001=m), ascii flag
pub fn export_stl_options(scene: &Scene, path: &str, scale: f32, ascii: bool) -> Result<(), String> {
    let mut triangles: Vec<([f32; 3], [[f32; 3]; 3])> = Vec::new(); // (normal, [v1,v2,v3])

    for obj in scene.objects.values() {
        if !obj.visible { continue; }
        let p = obj.position;
        match &obj.shape {
            Shape::Box { width, height, depth } => {
                let (w, h, d) = (*width, *height, *depth);
                // 12 triangles (2 per face)
                let v = [
                    [p[0],p[1],p[2]], [p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]], [p[0],p[1]+h,p[2]],
                    [p[0],p[1],p[2]+d], [p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d],
                ];
                let faces: &[([f32;3], [usize;4])] = &[
                    ([0.0,0.0,-1.0], [0,1,2,3]), ([0.0,0.0,1.0], [5,4,7,6]),
                    ([0.0,1.0,0.0], [3,2,6,7]), ([0.0,-1.0,0.0], [4,5,1,0]),
                    ([-1.0,0.0,0.0], [4,0,3,7]), ([1.0,0.0,0.0], [1,5,6,2]),
                ];
                for (n, idx) in faces {
                    triangles.push((*n, [v[idx[0]], v[idx[1]], v[idx[2]]]));
                    triangles.push((*n, [v[idx[0]], v[idx[2]], v[idx[3]]]));
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
                    let n = [c0, 0.0, s0];
                    // Side
                    triangles.push((n, [b0, b1, t1]));
                    triangles.push((n, [b0, t1, t0]));
                    // Bottom
                    triangles.push(([0.0,-1.0,0.0], [[cx,p[1],cz], b1, b0]));
                    // Top
                    triangles.push(([0.0,1.0,0.0], [[cx,p[1]+h,cz], t0, t1]));
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
                        let v00 = sphere_pt(cx,cy,cz,r,phi0,th0);
                        let v10 = sphere_pt(cx,cy,cz,r,phi0,th1);
                        let v01 = sphere_pt(cx,cy,cz,r,phi1,th0);
                        let v11 = sphere_pt(cx,cy,cz,r,phi1,th1);
                        let n = sphere_n(phi0,th0);
                        triangles.push((n, [v00, v10, v11]));
                        triangles.push((n, [v00, v11, v01]));
                    }
                }
            }
            Shape::Mesh(ref mesh) => {
                // 匯出所有 mesh 面為三角面
                for (&fid, face) in &mesh.faces {
                    let verts = mesh.face_vertices(fid);
                    if verts.len() >= 3 {
                        let n = face.normal;
                        // Fan triangulation
                        for i in 1..verts.len()-1 {
                            triangles.push((n, [
                                [p[0]+verts[0][0], p[1]+verts[0][1], p[2]+verts[0][2]],
                                [p[0]+verts[i][0], p[1]+verts[i][1], p[2]+verts[i][2]],
                                [p[0]+verts[i+1][0], p[1]+verts[i+1][1], p[2]+verts[i+1][2]],
                            ]));
                        }
                    }
                }
            }
            Shape::Line { .. } => {} // STL 不支援線段
        }
    }

    // Apply scale
    if (scale - 1.0).abs() > 0.0001 {
        for (_, verts) in &mut triangles {
            for v in verts.iter_mut() {
                v[0] *= scale; v[1] *= scale; v[2] *= scale;
            }
        }
    }

    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;

    if ascii {
        // ASCII STL
        writeln!(file, "solid Kolibri_Ai3D").map_err(|e| e.to_string())?;
        for (n, verts) in &triangles {
            writeln!(file, "  facet normal {:.6} {:.6} {:.6}", n[0], n[1], n[2]).map_err(|e| e.to_string())?;
            writeln!(file, "    outer loop").map_err(|e| e.to_string())?;
            for v in verts {
                writeln!(file, "      vertex {:.6} {:.6} {:.6}", v[0], v[1], v[2]).map_err(|e| e.to_string())?;
            }
            writeln!(file, "    endloop").map_err(|e| e.to_string())?;
            writeln!(file, "  endfacet").map_err(|e| e.to_string())?;
        }
        writeln!(file, "endsolid Kolibri_Ai3D").map_err(|e| e.to_string())?;
    } else {
        // Binary STL
        let mut header = [0u8; 80];
        let tag = b"Kolibri_Ai3D STL Export";
        header[..tag.len()].copy_from_slice(tag);
        file.write_all(&header).map_err(|e| e.to_string())?;
        file.write_all(&(triangles.len() as u32).to_le_bytes()).map_err(|e| e.to_string())?;
        for (n, verts) in &triangles {
            for f in n { file.write_all(&f.to_le_bytes()).map_err(|e| e.to_string())?; }
            for v in verts {
                for f in v { file.write_all(&f.to_le_bytes()).map_err(|e| e.to_string())?; }
            }
            file.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn sphere_pt(cx:f32,cy:f32,cz:f32,r:f32,phi:f32,theta:f32) -> [f32;3] {
    [cx+r*phi.sin()*theta.cos(), cy+r*phi.cos(), cz+r*phi.sin()*theta.sin()]
}
fn sphere_n(phi:f32,theta:f32) -> [f32;3] {
    [phi.sin()*theta.cos(), phi.cos(), phi.sin()*theta.sin()]
}

/// Import binary STL — creates real Shape::Mesh geometry
pub fn import_stl(scene: &mut Scene, path: &str) -> Result<usize, String> {
    use kolibri_core::halfedge::HeMesh;

    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    if data.len() < 84 { return Err("File too small".into()); }

    let tri_count = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
    let expected = 84 + tri_count * 50;
    if data.len() < expected { return Err("Truncated file".into()); }

    // 讀取所有三角面頂點
    let mut mesh = HeMesh::new();
    let mut vert_map: std::collections::HashMap<[i32; 3], u32> = std::collections::HashMap::new();
    let mut min_pos = [f32::MAX; 3];

    let read_f32 = |off: usize| -> f32 {
        f32::from_le_bytes([data[off], data[off+1], data[off+2], data[off+3]])
    };

    // First pass: collect all vertices and find min
    let mut offset = 84usize;
    let mut all_tris: Vec<[[f32; 3]; 3]> = Vec::with_capacity(tri_count);
    for _ in 0..tri_count {
        offset += 12; // skip normal
        let mut tri = [[0.0_f32; 3]; 3];
        for v in 0..3 {
            for j in 0..3 {
                tri[v][j] = read_f32(offset);
                min_pos[j] = min_pos[j].min(tri[v][j]);
                offset += 4;
            }
        }
        all_tris.push(tri);
        offset += 2; // skip attribute
    }

    // Second pass: build mesh with deduped vertices
    for tri in &all_tris {
        let mut vids = [0u32; 3];
        for v in 0..3 {
            // 量化到 0.01mm 精度做去重
            let key = [
                (tri[v][0] * 100.0) as i32,
                (tri[v][1] * 100.0) as i32,
                (tri[v][2] * 100.0) as i32,
            ];
            let vid = *vert_map.entry(key).or_insert_with(|| {
                mesh.add_vertex([
                    tri[v][0] - min_pos[0],
                    tri[v][1] - min_pos[1],
                    tri[v][2] - min_pos[2],
                ])
            });
            vids[v] = vid;
        }
        mesh.add_face(&vids);
    }

    let name = std::path::Path::new(path).file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "STL_Import".to_string());

    let id = scene.next_id_pub();
    scene.objects.insert(id.clone(), kolibri_core::scene::SceneObject {
        id,
        name,
        shape: Shape::Mesh(mesh),
        position: min_pos,
        material: MaterialKind::White,
        rotation_y: 0.0,
        tag: "匯入".to_string(),
        visible: true,
        roughness: 0.5,
        metallic: 0.0,
        texture_path: None,
        component_kind: Default::default(),
        parent_id: None,
        locked: false,
    });
    scene.version += 1;
    Ok(1)
}

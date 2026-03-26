//! OBJ file import/export for Kolibri CAD

use crate::scene::{Scene, SceneObject, Shape, MaterialKind};
use std::io::{Write, BufRead, BufReader};

/// Export scene to OBJ format
pub fn export_obj(scene: &Scene, path: &str) -> Result<(), String> {
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;

    writeln!(file, "# Kolibri_Ai3D OBJ Export").map_err(|e| e.to_string())?;
    writeln!(file, "# Objects: {}", scene.objects.len()).map_err(|e| e.to_string())?;
    writeln!(file, "").map_err(|e| e.to_string())?;

    let mut vertex_offset = 1u32; // OBJ is 1-indexed
    let mut normal_offset = 1u32;

    for obj in scene.objects.values() {
        writeln!(file, "o {}", obj.name).map_err(|e| e.to_string())?;

        // Write material color as a comment
        let c = obj.material.color();
        writeln!(file, "# material: {:.3} {:.3} {:.3}", c[0], c[1], c[2]).map_err(|e| e.to_string())?;

        let (verts, normals, faces) = generate_obj_mesh(obj);

        // Write vertices
        for v in &verts {
            // Convert mm to meters for standard OBJ (divide by 1000)
            writeln!(file, "v {:.6} {:.6} {:.6}", v[0] / 1000.0, v[1] / 1000.0, v[2] / 1000.0)
                .map_err(|e| e.to_string())?;
        }

        // Write normals
        for n in &normals {
            writeln!(file, "vn {:.6} {:.6} {:.6}", n[0], n[1], n[2])
                .map_err(|e| e.to_string())?;
        }

        // Write faces (vertex//normal format)
        for f in &faces {
            let face_str: Vec<String> = f.iter()
                .map(|(vi, ni)| format!("{}//{}", vi + vertex_offset, ni + normal_offset))
                .collect();
            writeln!(file, "f {}", face_str.join(" ")).map_err(|e| e.to_string())?;
        }

        vertex_offset += verts.len() as u32;
        normal_offset += normals.len() as u32;
        writeln!(file, "").map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Generate OBJ mesh data for a single object
/// Returns (vertices, normals, faces) where faces reference vertex/normal indices (0-based)
fn generate_obj_mesh(obj: &SceneObject) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<Vec<(u32, u32)>>) {
    let p = obj.position;

    match &obj.shape {
        Shape::Box { width, height, depth } => {
            let (w, h, d) = (*width, *height, *depth);
            // 8 corners
            let verts = vec![
                [p[0],   p[1],   p[2]],     // 0: front-bottom-left
                [p[0]+w, p[1],   p[2]],     // 1: front-bottom-right
                [p[0]+w, p[1]+h, p[2]],     // 2: front-top-right
                [p[0],   p[1]+h, p[2]],     // 3: front-top-left
                [p[0],   p[1],   p[2]+d],   // 4: back-bottom-left
                [p[0]+w, p[1],   p[2]+d],   // 5: back-bottom-right
                [p[0]+w, p[1]+h, p[2]+d],   // 6: back-top-right
                [p[0],   p[1]+h, p[2]+d],   // 7: back-top-left
            ];
            let normals = vec![
                [0.0, 0.0, -1.0],  // 0: front
                [0.0, 0.0,  1.0],  // 1: back
                [0.0,  1.0, 0.0],  // 2: top
                [0.0, -1.0, 0.0],  // 3: bottom
                [-1.0, 0.0, 0.0],  // 4: left
                [ 1.0, 0.0, 0.0],  // 5: right
            ];
            let faces = vec![
                vec![(0,0), (1,0), (2,0), (3,0)],  // front
                vec![(5,1), (4,1), (7,1), (6,1)],  // back
                vec![(3,2), (2,2), (6,2), (7,2)],  // top
                vec![(4,3), (5,3), (1,3), (0,3)],  // bottom
                vec![(4,4), (0,4), (3,4), (7,4)],  // left
                vec![(1,5), (5,5), (6,5), (2,5)],  // right
            ];
            (verts, normals, faces)
        }
        Shape::Cylinder { radius, height, segments } => {
            let segs = *segments as usize;
            let r = *radius;
            let h = *height;
            let cx = p[0] + r;
            let cz = p[2] + r;

            let mut verts = Vec::new();
            let mut normals = Vec::new();
            let mut faces = Vec::new();

            // Bottom center (0) and top center (1)
            verts.push([cx, p[1], cz]);
            verts.push([cx, p[1]+h, cz]);
            normals.push([0.0, -1.0, 0.0]); // bottom normal
            normals.push([0.0,  1.0, 0.0]); // top normal

            // Ring vertices: bottom ring starts at index 2, top ring at 2+segs
            for i in 0..segs {
                let angle = (i as f32 / segs as f32) * std::f32::consts::TAU;
                let (sin, cos) = angle.sin_cos();
                let x = cx + r * cos;
                let z = cz + r * sin;
                verts.push([x, p[1], z]);       // bottom ring
                verts.push([x, p[1]+h, z]);     // top ring
                normals.push([cos, 0.0, sin]);   // side normal
            }

            // Bottom face (fan)
            let mut bottom = Vec::new();
            for i in (0..segs).rev() {
                bottom.push((2 + i as u32 * 2, 0u32)); // bottom ring, bottom normal
            }
            faces.push(bottom);

            // Top face (fan)
            let mut top = Vec::new();
            for i in 0..segs {
                top.push((2 + i as u32 * 2 + 1, 1u32)); // top ring, top normal
            }
            faces.push(top);

            // Side faces (quads)
            for i in 0..segs {
                let i0 = i as u32;
                let i1 = ((i + 1) % segs) as u32;
                let ni = 2 + i as u32; // side normal index
                let ni1 = 2 + ((i + 1) % segs) as u32;
                faces.push(vec![
                    (2 + i0*2,     ni),   // bottom-current
                    (2 + i1*2,     ni1),  // bottom-next
                    (2 + i1*2 + 1, ni1),  // top-next
                    (2 + i0*2 + 1, ni),   // top-current
                ]);
            }

            (verts, normals, faces)
        }
        Shape::Sphere { radius, segments } => {
            let segs = *segments as usize;
            let rings = segs / 2;
            let r = *radius;
            let cx = p[0] + r;
            let cy = p[1] + r;
            let cz = p[2] + r;

            let mut verts = Vec::new();
            let mut normals = Vec::new();
            let mut faces = Vec::new();

            // Generate vertices
            for j in 0..=rings {
                let phi = (j as f32 / rings as f32) * std::f32::consts::PI;
                let (sp, cp) = phi.sin_cos();
                for i in 0..=segs {
                    let theta = (i as f32 / segs as f32) * std::f32::consts::TAU;
                    let (st, ct) = theta.sin_cos();
                    let nx = ct * sp;
                    let ny = cp;
                    let nz = st * sp;
                    verts.push([cx + r * nx, cy + r * ny, cz + r * nz]);
                    normals.push([nx, ny, nz]);
                }
            }

            // Generate faces
            let cols = segs + 1;
            for j in 0..rings {
                for i in 0..segs {
                    let a = (j * cols + i) as u32;
                    let b = (j * cols + i + 1) as u32;
                    let c = ((j+1) * cols + i + 1) as u32;
                    let d = ((j+1) * cols + i) as u32;
                    faces.push(vec![(a, a), (b, b), (c, c), (d, d)]);
                }
            }

            (verts, normals, faces)
        }
        Shape::Line { points, thickness, .. } => {
            let mut all_verts = Vec::new();
            let mut all_normals = Vec::new();
            let mut all_faces = Vec::new();

            // Export each segment as a small box
            let t = *thickness * 0.5;
            for pair in points.windows(2) {
                let a = pair[0];
                let b = pair[1];
                let base_v = all_verts.len() as u32;
                let base_n = all_normals.len() as u32;
                all_verts.extend_from_slice(&[
                    [a[0]-t, a[1]-t, a[2]-t], [a[0]+t, a[1]-t, a[2]-t],
                    [a[0]+t, a[1]+t, a[2]-t], [a[0]-t, a[1]+t, a[2]-t],
                    [b[0]-t, b[1]-t, b[2]+t], [b[0]+t, b[1]-t, b[2]+t],
                    [b[0]+t, b[1]+t, b[2]+t], [b[0]-t, b[1]+t, b[2]+t],
                ]);
                all_normals.extend_from_slice(&[
                    [0.0,0.0,-1.0],[0.0,0.0,1.0],[0.0,1.0,0.0],
                    [0.0,-1.0,0.0],[-1.0,0.0,0.0],[1.0,0.0,0.0],
                ]);
                all_faces.extend_from_slice(&[
                    vec![(base_v,base_n),(base_v+1,base_n),(base_v+2,base_n),(base_v+3,base_n)],
                    vec![(base_v+5,base_n+1),(base_v+4,base_n+1),(base_v+7,base_n+1),(base_v+6,base_n+1)],
                    vec![(base_v+3,base_n+2),(base_v+2,base_n+2),(base_v+6,base_n+2),(base_v+7,base_n+2)],
                    vec![(base_v+4,base_n+3),(base_v+5,base_n+3),(base_v+1,base_n+3),(base_v,base_n+3)],
                    vec![(base_v+4,base_n+4),(base_v,base_n+4),(base_v+3,base_n+4),(base_v+7,base_n+4)],
                    vec![(base_v+1,base_n+5),(base_v+5,base_n+5),(base_v+6,base_n+5),(base_v+2,base_n+5)],
                ]);
            }

            (all_verts, all_normals, all_faces)
        }
        Shape::Mesh(ref mesh) => {
            let mut all_verts = Vec::new();
            let mut all_normals = Vec::new();
            let mut all_faces = Vec::new();

            // Build vertex index map (HeMesh VId -> OBJ 0-based index)
            let mut vid_map = std::collections::HashMap::new();
            for (&vid, vertex) in &mesh.vertices {
                let idx = all_verts.len();
                all_verts.push(vertex.pos);
                vid_map.insert(vid, idx as u32);
            }

            for (&fid, face) in &mesh.faces {
                let face_verts = mesh.face_vertices(fid);
                let n_idx = all_normals.len() as u32;
                all_normals.push(face.normal);

                // Build face from vertex positions by finding matching VIds
                let mut obj_face = Vec::new();
                for fv in &face_verts {
                    // Find VId for this position
                    if let Some((&vid, _)) = mesh.vertices.iter()
                        .find(|(_, v)| v.pos == *fv)
                    {
                        if let Some(&vi) = vid_map.get(&vid) {
                            obj_face.push((vi, n_idx));
                        }
                    }
                }
                if obj_face.len() >= 3 {
                    all_faces.push(obj_face);
                }
            }

            (all_verts, all_normals, all_faces)
        }
    }
}

/// Import OBJ file into scene (basic: creates one box per object based on bounding box)
pub fn import_obj(scene: &mut Scene, path: &str) -> Result<usize, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut current_name = String::from("imported");
    let mut object_verts: Vec<usize> = Vec::new(); // vertex indices for current object
    let mut objects_created = 0usize;

    let flush_object = |scene: &mut Scene, name: &str, verts: &[[f32; 3]], indices: &[usize], count: &mut usize| {
        if indices.is_empty() { return; }

        // Compute bounding box
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for &idx in indices {
            if idx < verts.len() {
                let v = verts[idx];
                for i in 0..3 {
                    min[i] = min[i].min(v[i]);
                    max[i] = max[i].max(v[i]);
                }
            }
        }

        // Convert from meters to mm (* 1000)
        let pos = [min[0] * 1000.0, min[1] * 1000.0, min[2] * 1000.0];
        let w = ((max[0] - min[0]) * 1000.0).max(10.0);
        let h = ((max[1] - min[1]) * 1000.0).max(10.0);
        let d = ((max[2] - min[2]) * 1000.0).max(10.0);

        scene.add_box(name.to_string(), pos, w, h, d, MaterialKind::White);
        *count += 1;
    };

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        let line = line.trim();

        if line.starts_with("v ") {
            let parts: Vec<f32> = line[2..].split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if parts.len() >= 3 {
                vertices.push([parts[0], parts[1], parts[2]]);
            }
        } else if line.starts_with("o ") || line.starts_with("g ") {
            // Flush previous object
            flush_object(scene, &current_name, &vertices, &object_verts, &mut objects_created);
            object_verts.clear();
            current_name = line[2..].trim().to_string();
        } else if line.starts_with("f ") {
            // Parse face - collect vertex indices
            for part in line[2..].split_whitespace() {
                if let Some(vi) = part.split('/').next().and_then(|s| s.parse::<usize>().ok()) {
                    if vi > 0 {
                        object_verts.push(vi - 1); // OBJ is 1-indexed
                    }
                }
            }
        }
    }

    // Flush last object
    flush_object(scene, &current_name, &vertices, &object_verts, &mut objects_created);

    Ok(objects_created)
}

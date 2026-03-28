//! OBJ file import/export for Kolibri CAD

use kolibri_core::scene::{Scene, SceneObject, Shape, MaterialKind};
use std::io::{Write, BufRead, BufReader};

/// Export scene to OBJ format
pub fn export_obj(scene: &Scene, path: &str) -> Result<(), String> {
    // 生成 .mtl 檔案路徑
    let mtl_path = path.replace(".obj", ".mtl");
    let mtl_filename = mtl_path.rsplit(['/', '\\']).next().unwrap_or("materials.mtl");

    // 寫入 .mtl 材質檔
    write_mtl_file(&mtl_path, scene).map_err(|e| e.to_string())?;

    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;

    writeln!(file, "# Kolibri_Ai3D OBJ Export").map_err(|e| e.to_string())?;
    writeln!(file, "# Objects: {}", scene.objects.len()).map_err(|e| e.to_string())?;
    writeln!(file, "mtllib {}", mtl_filename).map_err(|e| e.to_string())?;
    writeln!(file, "").map_err(|e| e.to_string())?;

    let mut vertex_offset = 1u32;
    let mut normal_offset = 1u32;
    let mut uv_offset = 1u32;

    for obj in scene.objects.values() {
        writeln!(file, "o {}", obj.name).map_err(|e| e.to_string())?;

        // 引用材質
        let mat_name = material_name(&obj.material);
        writeln!(file, "usemtl {}", mat_name).map_err(|e| e.to_string())?;

        let (verts, normals, faces) = generate_obj_mesh(obj);

        // Write vertices
        for v in &verts {
            writeln!(file, "v {:.6} {:.6} {:.6}", v[0] / 1000.0, v[1] / 1000.0, v[2] / 1000.0)
                .map_err(|e| e.to_string())?;
        }

        // Write UVs (triplanar: world-space / 1m)
        for (v, n) in verts.iter().zip(normals.iter()) {
            let (u, vt) = triplanar_uv(v, n);
            writeln!(file, "vt {:.6} {:.6}", u, vt).map_err(|e| e.to_string())?;
        }

        // Write normals
        for n in &normals {
            writeln!(file, "vn {:.6} {:.6} {:.6}", n[0], n[1], n[2])
                .map_err(|e| e.to_string())?;
        }

        // Write faces (vertex/uv/normal format)
        for f in &faces {
            let face_str: Vec<String> = f.iter()
                .map(|(vi, ni)| format!("{}/{}/{}", vi + vertex_offset, vi + uv_offset, ni + normal_offset))
                .collect();
            writeln!(file, "f {}", face_str.join(" ")).map_err(|e| e.to_string())?;
        }

        vertex_offset += verts.len() as u32;
        uv_offset += verts.len() as u32;
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

/// Import OBJ file into scene — creates real Shape::Mesh geometry with .mtl materials
pub fn import_obj(scene: &mut Scene, path: &str) -> Result<usize, String> {
    use kolibri_core::halfedge::HeMesh;
    use std::collections::HashMap;

    // 嘗試讀取 .mtl 材質檔
    let mtl_materials = {
        let mtl_path = path.replace(".obj", ".mtl");
        parse_mtl_file(&mtl_path).unwrap_or_default()
    };

    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut current_name = String::from("imported");
    let mut current_material = MaterialKind::White;
    let mut object_faces: Vec<Vec<usize>> = Vec::new();
    let mut objects_created = 0usize;

    let flush_mesh = |scene: &mut Scene, name: &str, verts: &[[f32; 3]],
                      faces: &[Vec<usize>], count: &mut usize, mat: MaterialKind| {
        if faces.is_empty() { return; }

        // 收集此 object 用到的頂點，建立 local index mapping
        let mut used: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
        let mut local_verts: Vec<[f32; 3]> = Vec::new();
        for face in faces {
            for &vi in face {
                if !used.contains_key(&vi) && vi < verts.len() {
                    let v = verts[vi];
                    // OBJ 通常是 meters，轉 mm
                    used.insert(vi, local_verts.len() as u32);
                    local_verts.push([v[0] * 1000.0, v[1] * 1000.0, v[2] * 1000.0]);
                }
            }
        }
        if local_verts.is_empty() { return; }

        // 計算 bounding box 找位置偏移
        let mut min = [f32::MAX; 3];
        for v in &local_verts {
            for i in 0..3 { min[i] = min[i].min(v[i]); }
        }

        // 建立 HeMesh
        let mut mesh = HeMesh::new();
        for v in &local_verts {
            mesh.add_vertex([v[0] - min[0], v[1] - min[1], v[2] - min[2]]);
        }
        for face in faces {
            let local_ids: Vec<u32> = face.iter()
                .filter_map(|vi| used.get(vi).copied())
                .collect();
            if local_ids.len() >= 3 {
                mesh.add_face(&local_ids);
            }
        }

        let id = scene.next_id_pub();
        scene.objects.insert(id.clone(), kolibri_core::scene::SceneObject {
            id,
            name: name.to_string(),
            shape: Shape::Mesh(mesh),
            position: min,
            material: mat,
            rotation_y: 0.0,
            tag: "匯入".to_string(),
            visible: true,
            roughness: 0.5,
            metallic: 0.0,
            texture_path: None,
            component_kind: Default::default(),
            parent_id: None,
            component_def_id: None,
            locked: false,
        });
        scene.version += 1;
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
        } else if line.starts_with("usemtl ") {
            // 材質切換
            let mtl_name = line[7..].trim();
            current_material = mtl_materials.get(mtl_name)
                .copied()
                .unwrap_or(MaterialKind::White);
        } else if line.starts_with("o ") || line.starts_with("g ") {
            flush_mesh(scene, &current_name, &vertices, &object_faces, &mut objects_created, current_material);
            object_faces.clear();
            current_name = line[2..].trim().to_string();
        } else if line.starts_with("f ") {
            let face_verts: Vec<usize> = line[2..].split_whitespace()
                .filter_map(|part| part.split('/').next().and_then(|s| s.parse::<usize>().ok()))
                .filter(|&vi| vi > 0)
                .map(|vi| vi - 1)
                .collect();
            if face_verts.len() >= 3 {
                object_faces.push(face_verts);
            }
        }
    }

    flush_mesh(scene, &current_name, &vertices, &object_faces, &mut objects_created, current_material);

    Ok(objects_created)
}

/// Parse .mtl file and return material name → MaterialKind mapping
fn parse_mtl_file(path: &str) -> Result<std::collections::HashMap<String, MaterialKind>, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut materials = std::collections::HashMap::new();
    let mut current_name = String::new();
    let mut current_kd = [0.8_f32, 0.8, 0.8];

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("newmtl ") {
            // 儲存前一個材質
            if !current_name.is_empty() {
                materials.insert(current_name.clone(), color_to_material(current_kd));
            }
            current_name = line[7..].trim().to_string();
            current_kd = [0.8, 0.8, 0.8];
        } else if line.starts_with("Kd ") {
            let parts: Vec<f32> = line[3..].split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if parts.len() >= 3 {
                current_kd = [parts[0], parts[1], parts[2]];
            }
        }
    }
    if !current_name.is_empty() {
        materials.insert(current_name, color_to_material(current_kd));
    }
    Ok(materials)
}

/// 從 Kd 顏色推斷最接近的 MaterialKind
fn color_to_material(kd: [f32; 3]) -> MaterialKind {
    // 比對所有材質顏色，找最接近的
    let candidates: &[(MaterialKind, [f32; 3])] = &[
        (MaterialKind::Concrete, [0.55, 0.55, 0.55]),
        (MaterialKind::Wood, [0.60, 0.40, 0.20]),
        (MaterialKind::Metal, [0.72, 0.72, 0.78]),
        (MaterialKind::Steel, [0.62, 0.63, 0.65]),
        (MaterialKind::Brick, [0.72, 0.35, 0.22]),
        (MaterialKind::Glass, [0.70, 0.85, 0.95]),
        (MaterialKind::White, [0.95, 0.95, 0.95]),
        (MaterialKind::Black, [0.10, 0.10, 0.10]),
        (MaterialKind::Marble, [0.92, 0.90, 0.88]),
        (MaterialKind::Grass, [0.35, 0.55, 0.25]),
        (MaterialKind::Copper, [0.72, 0.45, 0.20]),
        (MaterialKind::Gold, [0.83, 0.69, 0.22]),
        (MaterialKind::Aluminum, [0.80, 0.81, 0.83]),
    ];
    let mut best = MaterialKind::White;
    let mut best_dist = f32::MAX;
    for (mat, c) in candidates {
        let d = (kd[0]-c[0]).powi(2) + (kd[1]-c[1]).powi(2) + (kd[2]-c[2]).powi(2);
        if d < best_dist {
            best_dist = d;
            best = *mat;
        }
    }
    best
}

/// Triplanar UV 投影（世界空間 / 1000mm = 1m 一個 repeat）
fn triplanar_uv(pos: &[f32; 3], normal: &[f32; 3]) -> (f32, f32) {
    let scale = 0.001; // 1m repeat
    let ax = normal[0].abs();
    let ay = normal[1].abs();
    let az = normal[2].abs();
    if ay > ax && ay > az {
        (pos[0] * scale, pos[2] * scale) // Y-dominant: XZ
    } else if ax > az {
        (pos[1] * scale, pos[2] * scale) // X-dominant: YZ
    } else {
        (pos[0] * scale, pos[1] * scale) // Z-dominant: XY
    }
}

// ─── MTL 材質檔生成 ─────────────────────────────────────────────────────────

fn material_name(mat: &MaterialKind) -> String {
    mat.label().replace(' ', "_").replace('/', "_")
}

fn write_mtl_file(path: &str, scene: &Scene) -> Result<(), std::io::Error> {
    use std::collections::HashSet;
    let mut file = std::fs::File::create(path)?;
    writeln!(file, "# Kolibri_Ai3D MTL Material Library")?;

    let mut written: HashSet<String> = HashSet::new();
    for obj in scene.objects.values() {
        let name = material_name(&obj.material);
        if written.contains(&name) { continue; }
        written.insert(name.clone());

        let c = obj.material.color();
        writeln!(file)?;
        writeln!(file, "newmtl {}", name)?;
        writeln!(file, "Ka {:.4} {:.4} {:.4}", c[0] * 0.2, c[1] * 0.2, c[2] * 0.2)?; // ambient
        writeln!(file, "Kd {:.4} {:.4} {:.4}", c[0], c[1], c[2])?; // diffuse
        writeln!(file, "Ks 0.2000 0.2000 0.2000")?; // specular
        writeln!(file, "Ns {:.1}", (1.0 - obj.roughness) * 200.0)?; // shininess from roughness
        // 透明度
        if c[3] < 0.9 {
            writeln!(file, "d {:.4}", c[3])?;
        } else {
            writeln!(file, "d 1.0000")?;
        }
        writeln!(file, "illum 2")?; // Blinn-Phong
    }
    Ok(())
}

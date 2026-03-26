//! Minimal DXF export (3DFACE entities) and import

use kolibri_core::scene::{Scene, Shape};
use std::io::Write;

pub fn export_dxf(scene: &Scene, path: &str) -> Result<(), String> {
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;

    // Header
    write!(file, "0\nSECTION\n2\nHEADER\n0\nENDSEC\n").map_err(|e| e.to_string())?;

    // Entities section
    write!(file, "0\nSECTION\n2\nENTITIES\n").map_err(|e| e.to_string())?;

    for obj in scene.objects.values() {
        if !obj.visible { continue; }
        let p = obj.position;
        match &obj.shape {
            Shape::Box { width, height, depth } => {
                let (w, h, d) = (*width, *height, *depth);
                let v = [
                    [p[0],p[1],p[2]], [p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]], [p[0],p[1]+h,p[2]],
                    [p[0],p[1],p[2]+d], [p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d],
                ];
                // 6 faces as 3DFACE
                let faces = [
                    [0,1,2,3], [5,4,7,6], [3,2,6,7], [4,5,1,0], [4,0,3,7], [1,5,6,2],
                ];
                for f in &faces {
                    write_3dface(&mut file, &obj.name, v[f[0]], v[f[1]], v[f[2]], v[f[3]])?;
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
                    // Side quad
                    write_3dface(&mut file, &obj.name, b0, b1, t1, t0)?;
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
                        let mk = |phi: f32, th: f32| -> [f32;3] {
                            [cx+r*phi.sin()*th.cos(), cy+r*phi.cos(), cz+r*phi.sin()*th.sin()]
                        };
                        write_3dface(&mut file, &obj.name, mk(phi0,th0), mk(phi0,th1), mk(phi1,th1), mk(phi1,th0))?;
                    }
                }
            }
            _ => {}
        }
    }

    write!(file, "0\nENDSEC\n0\nEOF\n").map_err(|e| e.to_string())?;
    Ok(())
}

fn write_3dface(f: &mut std::fs::File, layer: &str, v1: [f32;3], v2: [f32;3], v3: [f32;3], v4: [f32;3]) -> Result<(), String> {
    write!(f, "0\n3DFACE\n8\n{}\n", layer).map_err(|e| e.to_string())?;
    // First vertex (10,20,30)
    write!(f, "10\n{:.6}\n20\n{:.6}\n30\n{:.6}\n", v1[0], v1[1], v1[2]).map_err(|e| e.to_string())?;
    // Second (11,21,31)
    write!(f, "11\n{:.6}\n21\n{:.6}\n31\n{:.6}\n", v2[0], v2[1], v2[2]).map_err(|e| e.to_string())?;
    // Third (12,22,32)
    write!(f, "12\n{:.6}\n22\n{:.6}\n32\n{:.6}\n", v3[0], v3[1], v3[2]).map_err(|e| e.to_string())?;
    // Fourth (13,23,33)
    write!(f, "13\n{:.6}\n23\n{:.6}\n33\n{:.6}\n", v4[0], v4[1], v4[2]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Import DXF — parses LINE, 3DFACE, CIRCLE, ARC entities into real geometry
pub fn import_dxf(scene: &mut Scene, path: &str) -> Result<usize, String> {
    use kolibri_core::halfedge::HeMesh;

    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let lines: Vec<&str> = content.lines().collect();

    let mut line_segments: Vec<([f32; 3], [f32; 3])> = Vec::new();
    let mut faces_3d: Vec<[[f32; 3]; 4]> = Vec::new();
    let mut circles: Vec<([f32; 3], f32)> = Vec::new(); // center, radius

    // DXF parser state
    let mut i = 0;
    let mut in_entities = false;
    let mut current_entity = String::new();
    let mut coords: std::collections::HashMap<i32, f32> = std::collections::HashMap::new();

    while i < lines.len().saturating_sub(1) {
        let code = lines[i].trim();
        let value = lines[i + 1].trim();
        i += 2;

        if value == "ENTITIES" && code == "2" { in_entities = true; continue; }
        if value == "ENDSEC" && code == "0" && in_entities { in_entities = false; continue; }
        if !in_entities { continue; }

        if code == "0" {
            // 處理前一個 entity
            match current_entity.as_str() {
                "LINE" => {
                    let p1 = [coords.get(&10).copied().unwrap_or(0.0),
                              coords.get(&30).copied().unwrap_or(0.0),  // DXF Z → our Y
                              coords.get(&20).copied().unwrap_or(0.0)]; // DXF Y → our Z
                    let p2 = [coords.get(&11).copied().unwrap_or(0.0),
                              coords.get(&31).copied().unwrap_or(0.0),
                              coords.get(&21).copied().unwrap_or(0.0)];
                    line_segments.push((p1, p2));
                }
                "3DFACE" => {
                    let mut face = [[0.0_f32; 3]; 4];
                    for j in 0..4 {
                        face[j] = [
                            coords.get(&(10 + j as i32)).copied().unwrap_or(0.0),
                            coords.get(&(30 + j as i32)).copied().unwrap_or(0.0),
                            coords.get(&(20 + j as i32)).copied().unwrap_or(0.0),
                        ];
                    }
                    faces_3d.push(face);
                }
                "CIRCLE" => {
                    let center = [coords.get(&10).copied().unwrap_or(0.0),
                                  coords.get(&30).copied().unwrap_or(0.0),
                                  coords.get(&20).copied().unwrap_or(0.0)];
                    let radius = coords.get(&40).copied().unwrap_or(100.0);
                    circles.push((center, radius));
                }
                _ => {}
            }
            current_entity = value.to_string();
            coords.clear();
            continue;
        }

        if let Ok(c) = code.parse::<i32>() {
            if let Ok(v) = value.parse::<f32>() {
                coords.insert(c, v);
            }
        }
    }
    // 處理最後一個 entity
    if current_entity == "LINE" {
        let p1 = [coords.get(&10).copied().unwrap_or(0.0),
                   coords.get(&30).copied().unwrap_or(0.0),
                   coords.get(&20).copied().unwrap_or(0.0)];
        let p2 = [coords.get(&11).copied().unwrap_or(0.0),
                   coords.get(&31).copied().unwrap_or(0.0),
                   coords.get(&21).copied().unwrap_or(0.0)];
        line_segments.push((p1, p2));
    }

    let mut count = 0;
    let base_name = std::path::Path::new(path).file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "DXF".into());

    // LINE entities → Shape::Line
    if !line_segments.is_empty() {
        for (idx, (p1, p2)) in line_segments.iter().enumerate() {
            let id = scene.next_id_pub();
            scene.objects.insert(id.clone(), kolibri_core::scene::SceneObject {
                id,
                name: format!("{}_line_{}", base_name, idx),
                shape: Shape::Line {
                    points: vec![*p1, *p2],
                    thickness: 2.0,
                    arc_center: None,
                    arc_radius: None,
                    arc_angle_deg: None,
                },
                position: [0.0; 3],
                material: kolibri_core::scene::MaterialKind::White,
                rotation_y: 0.0,
                tag: "匯入".to_string(),
                visible: true,
                roughness: 0.5,
                metallic: 0.0,
                texture_path: None,
                component_kind: Default::default(),
                parent_id: None,
            });
            count += 1;
        }
        scene.version += 1;
    }

    // 3DFACE entities → Shape::Mesh
    if !faces_3d.is_empty() {
        let mut mesh = HeMesh::new();
        let mut vert_map: std::collections::HashMap<[i32; 3], u32> = std::collections::HashMap::new();
        let mut min_pos = [f32::MAX; 3];
        // 先找 min
        for face in &faces_3d {
            for v in face {
                for j in 0..3 { min_pos[j] = min_pos[j].min(v[j]); }
            }
        }
        for face in &faces_3d {
            let mut vids = Vec::new();
            for v in face {
                let key = [(v[0] * 100.0) as i32, (v[1] * 100.0) as i32, (v[2] * 100.0) as i32];
                let vid = *vert_map.entry(key).or_insert_with(|| {
                    mesh.add_vertex([v[0] - min_pos[0], v[1] - min_pos[1], v[2] - min_pos[2]])
                });
                vids.push(vid);
            }
            // 去除重複頂點（3DFACE 第4點可能等於第3點）
            vids.dedup();
            if vids.len() >= 3 {
                mesh.add_face(&vids);
            }
        }
        let id = scene.next_id_pub();
        scene.objects.insert(id.clone(), kolibri_core::scene::SceneObject {
            id,
            name: format!("{}_mesh", base_name),
            shape: Shape::Mesh(mesh),
            position: min_pos,
            material: kolibri_core::scene::MaterialKind::White,
            rotation_y: 0.0,
            tag: "匯入".to_string(),
            visible: true,
            roughness: 0.5,
            metallic: 0.0,
            texture_path: None,
            component_kind: Default::default(),
            parent_id: None,
        });
        scene.version += 1;
        count += 1;
    }

    // CIRCLE entities → Shape::Cylinder (thin disk approximation)
    for (idx, (center, radius)) in circles.iter().enumerate() {
        scene.add_cylinder(
            format!("{}_circle_{}", base_name, idx),
            [center[0] - radius, center[1], center[2] - radius],
            *radius, 10.0, 32,
            kolibri_core::scene::MaterialKind::White,
        );
        count += 1;
    }

    if count == 0 { return Err("No geometry found in DXF".into()); }
    Ok(count)
}

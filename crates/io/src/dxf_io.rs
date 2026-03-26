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

/// Import DXF (basic: reads 3DFACE and LINE entities, creates bounding box)
pub fn import_dxf(scene: &mut Scene, path: &str) -> Result<usize, String> {
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let lines: Vec<&str> = content.lines().collect();

    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    let mut has_geom = false;

    let mut i = 0;
    while i < lines.len().saturating_sub(1) {
        let code = lines[i].trim();
        let value = lines[i+1].trim();

        // Look for vertex coordinates
        if let Ok(c) = code.parse::<i32>() {
            if (10..=13).contains(&c) || (110..=113).contains(&c) {
                if let Ok(x) = value.parse::<f32>() {
                    min[0] = min[0].min(x); max[0] = max[0].max(x); has_geom = true;
                }
            }
            if (20..=23).contains(&c) || (120..=123).contains(&c) {
                if let Ok(y) = value.parse::<f32>() {
                    min[1] = min[1].min(y); max[1] = max[1].max(y);
                }
            }
            if (30..=33).contains(&c) || (130..=133).contains(&c) {
                if let Ok(z) = value.parse::<f32>() {
                    min[2] = min[2].min(z); max[2] = max[2].max(z);
                }
            }
        }
        i += 2;
    }

    if !has_geom { return Err("No geometry found in DXF".into()); }

    let name = std::path::Path::new(path).file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "DXF_Import".into());

    let w = (max[0] - min[0]).max(1.0);
    let h = (max[1] - min[1]).max(1.0);
    let d = (max[2] - min[2]).max(1.0);
    scene.add_box(name, min, w, h, d, kolibri_core::scene::MaterialKind::White);
    Ok(1)
}

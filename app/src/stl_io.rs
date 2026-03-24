//! STL file import/export

use crate::scene::{Scene, Shape, MaterialKind};
use std::io::Write;

/// Export scene to binary STL
pub fn export_stl(scene: &Scene, path: &str) -> Result<(), String> {
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
            _ => {}
        }
    }

    // Write binary STL
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    // Header: exactly 80 bytes
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
        file.write_all(&0u16.to_le_bytes()).map_err(|e| e.to_string())?; // attribute
    }
    Ok(())
}

fn sphere_pt(cx:f32,cy:f32,cz:f32,r:f32,phi:f32,theta:f32) -> [f32;3] {
    [cx+r*phi.sin()*theta.cos(), cy+r*phi.cos(), cz+r*phi.sin()*theta.sin()]
}
fn sphere_n(phi:f32,theta:f32) -> [f32;3] {
    [phi.sin()*theta.cos(), phi.cos(), phi.sin()*theta.sin()]
}

/// Import binary STL (creates boxes from bounding regions)
pub fn import_stl(scene: &mut Scene, path: &str) -> Result<usize, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    if data.len() < 84 { return Err("File too small".into()); }

    let tri_count = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
    let expected = 84 + tri_count * 50;
    if data.len() < expected { return Err("Truncated file".into()); }

    // Compute overall bounding box
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    let mut offset = 84usize;
    for _ in 0..tri_count {
        offset += 12; // skip normal
        for _ in 0..3 {
            for j in 0..3 {
                let f = f32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
                min[j] = min[j].min(f);
                max[j] = max[j].max(f);
                offset += 4;
            }
        }
        offset += 2; // skip attribute
    }

    let w = (max[0] - min[0]).max(1.0);
    let h = (max[1] - min[1]).max(1.0);
    let d = (max[2] - min[2]).max(1.0);
    let name = std::path::Path::new(path).file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "STL_Import".to_string());

    scene.add_box(name, [min[0], min[1], min[2]], w, h, d, MaterialKind::White);
    Ok(1)
}

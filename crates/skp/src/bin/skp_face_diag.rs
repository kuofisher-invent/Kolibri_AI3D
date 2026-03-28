//! 分析每個面的法線和位置，找出異常面片

fn main() {
    let path = std::env::args().nth(1).unwrap_or("docs/sample/SKP_IMPORT.skp".into());
    let skp = kolibri_skp::import_skp(&path).expect("import failed");

    for m in &skp.meshes {
        println!("Mesh {} — {} verts, {} tris", m.id, m.vertices.len(), m.indices.len()/3);
        
        // 分析每個三角形的中心位置和法線
        for t in 0..m.indices.len()/3 {
            let i0 = m.indices[t*3] as usize;
            let i1 = m.indices[t*3+1] as usize;
            let i2 = m.indices[t*3+2] as usize;
            if i0 >= m.vertices.len() || i1 >= m.vertices.len() || i2 >= m.vertices.len() { continue; }
            let v0 = m.vertices[i0];
            let v1 = m.vertices[i1];
            let v2 = m.vertices[i2];
            // 中心
            let cx = (v0[0]+v1[0]+v2[0])/3.0;
            let cy = (v0[1]+v1[1]+v2[1])/3.0;
            let cz = (v0[2]+v1[2]+v2[2])/3.0;
            // 法線 (cross product)
            let a = [v1[0]-v0[0], v1[1]-v0[1], v1[2]-v0[2]];
            let b = [v2[0]-v0[0], v2[1]-v0[1], v2[2]-v0[2]];
            let nx = a[1]*b[2]-a[2]*b[1];
            let ny = a[2]*b[0]-a[0]*b[2];
            let nz = a[0]*b[1]-a[1]*b[0];
            let len = (nx*nx+ny*ny+nz*nz).sqrt().max(1e-10);
            println!("  tri[{:3}] center=({:7.1},{:7.1},{:7.1}) normal=({:5.2},{:5.2},{:5.2})",
                t, cx, cy, cz, nx/len, ny/len, nz/len);
        }
        
        // 按 Y 座標分組統計
        println!("\n  === Y 座標分佈（截面高度）===");
        let mut y_values: Vec<f32> = m.vertices.iter().map(|v| v[1]).collect();
        y_values.sort_by(|a,b| a.partial_cmp(b).unwrap());
        y_values.dedup_by(|a,b| (*a - *b).abs() < 0.5);
        for y in &y_values {
            let count = m.vertices.iter().filter(|v| (v[1] - y).abs() < 0.5).count();
            println!("    Y={:7.1} — {} 個頂點", y, count);
        }
    }
}

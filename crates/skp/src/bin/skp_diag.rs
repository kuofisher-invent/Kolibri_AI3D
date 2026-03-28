//! 診斷 SKP 匯入結構

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        "docs/sample/SKP_IMPORT.skp".to_string()
    });

    // MeshHelper 原始索引診斷
    diag_mesh_helper_raw(&path);

    let skp = kolibri_skp::import_skp(&path).expect("import failed");

    println!("\n=== SkpScene 診斷 ===\n");

    println!("Meshes: {}", skp.meshes.len());
    for (i, m) in skp.meshes.iter().enumerate() {
        let (mut min_x, mut min_y, mut min_z) = (f32::MAX, f32::MAX, f32::MAX);
        let (mut max_x, mut max_y, mut max_z) = (f32::MIN, f32::MIN, f32::MIN);
        for v in &m.vertices {
            min_x = min_x.min(v[0]); min_y = min_y.min(v[1]); min_z = min_z.min(v[2]);
            max_x = max_x.max(v[0]); max_y = max_y.max(v[1]); max_z = max_z.max(v[2]);
        }
        println!("  [{}] id={} name={} verts={} tris={}",
            i, m.id, m.name, m.vertices.len(), m.indices.len()/3);
        println!("      AABB: ({:.1},{:.1},{:.1}) → ({:.1},{:.1},{:.1})",
            min_x, min_y, min_z, max_x, max_y, max_z);
        println!("      size: ({:.1},{:.1},{:.1})",
            max_x-min_x, max_y-min_y, max_z-min_z);

        // 顯示前 5 個三角形的索引和頂點
        println!("      前 5 個三角:");
        for t in 0..5.min(m.indices.len()/3) {
            let i0 = m.indices[t*3] as usize;
            let i1 = m.indices[t*3+1] as usize;
            let i2 = m.indices[t*3+2] as usize;
            let v0 = if i0 < m.vertices.len() { m.vertices[i0] } else { [f32::NAN; 3] };
            let v1 = if i1 < m.vertices.len() { m.vertices[i1] } else { [f32::NAN; 3] };
            let v2 = if i2 < m.vertices.len() { m.vertices[i2] } else { [f32::NAN; 3] };
            println!("        tri[{}]: idx=({},{},{}) v0=({:.1},{:.1},{:.1}) v1=({:.1},{:.1},{:.1}) v2=({:.1},{:.1},{:.1})",
                t, i0, i1, i2, v0[0], v0[1], v0[2], v1[0], v1[1], v1[2], v2[0], v2[1], v2[2]);
        }
    }

    println!("\nInstances: {}", skp.instances.len());
    for (i, inst) in skp.instances.iter().enumerate() {
        let t = &inst.transform;
        println!("  [{}] id={} name={} mesh_id={} comp_def={:?}",
            i, inst.id, inst.name, inst.mesh_id, inst.component_def_id);
        println!("      transform: [{:.2},{:.2},{:.2},{:.2}]", t[0], t[1], t[2], t[3]);
        println!("                 [{:.2},{:.2},{:.2},{:.2}]", t[4], t[5], t[6], t[7]);
        println!("                 [{:.2},{:.2},{:.2},{:.2}]", t[8], t[9], t[10], t[11]);
        println!("                 [{:.2},{:.2},{:.2},{:.2}]", t[12], t[13], t[14], t[15]);
    }

    println!("\nGroups: {}", skp.groups.len());
    println!("\nComponentDefs: {}", skp.component_defs.len());
}

/// 直接用 SDK 讀 MeshHelper 的原始索引值
fn diag_mesh_helper_raw(path: &str) {
    println!("=== MeshHelper 原始索引診斷 ===\n");

    // 用 kolibri_skp 的公開 API 匯入，但我們需要看原始索引
    // 改用 export_raw 看 indices
    let skp = match kolibri_skp::import_skp(path) {
        Ok(s) => s,
        Err(e) => { println!("FAIL: {}", e); return; }
    };

    for m in &skp.meshes {
        println!("Mesh {} — {} verts, {} indices", m.id, m.vertices.len(), m.indices.len());
        // 檢查索引範圍
        let max_idx = m.indices.iter().max().copied().unwrap_or(0);
        let min_idx = m.indices.iter().min().copied().unwrap_or(0);
        println!("  index range: {} .. {} (vertex count: {})", min_idx, max_idx, m.vertices.len());
        if max_idx >= m.vertices.len() as u32 {
            println!("  ⚠ INDEX OUT OF BOUNDS! max_idx={} >= vert_count={}", max_idx, m.vertices.len());
        }
        // 顯示前 10 個索引
        let show = 15.min(m.indices.len());
        println!("  first {} indices: {:?}", show, &m.indices[..show]);
    }
    println!();
}

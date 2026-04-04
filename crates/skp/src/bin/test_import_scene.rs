//! 完整測試：SKP SDK → UnifiedIR → Scene → 驗證

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        "docs/sample/component_sample.skp".to_string()
    });

    println!("=== SKP → Scene 完整匯入測試 ===\n");

    // Step 1: SDK 讀取
    println!("[1] SDK 讀取 SKP...");
    let skp_scene = match kolibri_skp::import_skp(&path) {
        Ok(s) => s,
        Err(e) => { println!("FAILED: {}", e); return; }
    };
    println!("    SDK 結果:");
    println!("      Meshes: {}", skp_scene.meshes.len());
    println!("      Instances: {}", skp_scene.instances.len());
    println!("      Groups: {}", skp_scene.groups.len());
    println!("      ComponentDefs: {}", skp_scene.component_defs.len());
    println!("      Materials: {}", skp_scene.materials.len());
    let sdk_verts: usize = skp_scene.meshes.iter().map(|m| m.vertices.len()).sum();
    let sdk_tris: usize = skp_scene.meshes.iter().map(|m| m.indices.len() / 3).sum();
    println!("      Total vertices: {}", sdk_verts);
    println!("      Total triangles: {}", sdk_tris);

    // Step 2: 建入 Scene
    println!("\n[2] 建入 Kolibri Scene...");
    let mut scene = kolibri_core::scene::Scene::default();

    // 手動做 IR 轉換（跟 app 裡的 skp_sdk_import 相同邏輯）
    let ir = sdk_to_ir(&skp_scene, &path);
    println!("    IR 結果:");
    println!("      Meshes: {}", ir.meshes.len());
    println!("      Instances: {}", ir.instances);
    println!("      Groups: {}", ir.groups);
    println!("      ComponentDefs: {}", ir.component_defs);

    // 用簡化版 build（不依賴 app crate 的 import_manager）
    let mut obj_count = 0;
    for mesh_ir in &ir.meshes {
        if mesh_ir.vertices.len() < 3 { continue; }
        let mut he = kolibri_core::halfedge::HeMesh::new();
        let vids: Vec<u32> = mesh_ir.vertices.iter()
            .map(|v| he.add_vertex(*v))
            .collect();
        for tri in mesh_ir.indices.chunks(3) {
            if tri.len() < 3 { continue; }
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            if i0 < vids.len() && i1 < vids.len() && i2 < vids.len() {
                he.add_face(&[vids[i0], vids[i1], vids[i2]]);
            }
        }
        let id = scene.insert_mesh_raw(
            mesh_ir.name.clone(), [0.0, 0.0, 0.0],
            he, kolibri_core::scene::MaterialKind::White,
        );
        obj_count += 1;
    }
    scene.version += 1;

    println!("\n[3] Scene 結果:");
    println!("      Objects: {}", scene.objects.len());
    println!("      Groups: {}", scene.groups.len());
    println!("      ComponentDefs: {}", scene.component_defs.len());

    // 驗證每個物件
    let mut total_mesh_verts = 0usize;
    let mut total_mesh_faces = 0usize;
    let mut shape_counts = std::collections::HashMap::new();
    for obj in scene.objects.values() {
        let shape_type = match &obj.shape {
            kolibri_core::scene::Shape::Box { .. } => "Box",
            kolibri_core::scene::Shape::Cylinder { .. } => "Cylinder",
            kolibri_core::scene::Shape::Sphere { .. } => "Sphere",
            kolibri_core::scene::Shape::Line { .. } => "Line",
            kolibri_core::scene::Shape::Mesh(m) => {
                total_mesh_verts += m.vertices.len();
                total_mesh_faces += m.faces.len();
                "Mesh"
            }
            kolibri_core::scene::Shape::SteelProfile { params, length, profile_type } => {
                println!("        SteelProfile: {:?} H={} B={} L={}", profile_type, params.h, params.b, length);
                "SteelProfile"
            }
        };
        *shape_counts.entry(shape_type).or_insert(0) += 1;
    }
    println!("      Shape 分佈: {:?}", shape_counts);
    println!("      HeMesh 總頂點: {}", total_mesh_verts);
    println!("      HeMesh 總面: {}", total_mesh_faces);

    // 比對
    println!("\n[4] 比對:");
    println!("      SDK meshes ({}) → Scene objects ({})", skp_scene.meshes.len(), scene.objects.len());
    println!("      SDK vertices ({}) → HeMesh vertices ({})", sdk_verts, total_mesh_verts);
    println!("      SDK triangles ({}) → HeMesh faces ({})", sdk_tris, total_mesh_faces);
    let vert_ratio = if sdk_verts > 0 { total_mesh_verts as f64 / sdk_verts as f64 * 100.0 } else { 0.0 };
    let face_ratio = if sdk_tris > 0 { total_mesh_faces as f64 / sdk_tris as f64 * 100.0 } else { 0.0 };
    println!("      頂點保留率: {:.1}%", vert_ratio);
    println!("      面保留率: {:.1}%", face_ratio);

    // 顯示前 5 個物件
    println!("\n[5] 前 5 個物件:");
    for (i, obj) in scene.objects.values().take(5).enumerate() {
        let info = match &obj.shape {
            kolibri_core::scene::Shape::Mesh(m) => format!("Mesh({} verts, {} faces)", m.vertices.len(), m.faces.len()),
            other => format!("{:?}", other).chars().take(40).collect(),
        };
        println!("      [{}] {} — {}", i, obj.name, info);
    }

    println!("\n=== 完成 ===");
}

fn sdk_to_ir(skp: &kolibri_skp::SkpScene, source: &str) -> SimpleIR {
    SimpleIR {
        meshes: skp.meshes.iter().map(|m| IrMesh {
            name: m.name.clone(),
            vertices: m.vertices.clone(),
            indices: m.indices.clone(),
        }).collect(),
        instances: skp.instances.len(),
        groups: skp.groups.len(),
        component_defs: skp.component_defs.len(),
    }
}

struct SimpleIR {
    meshes: Vec<IrMesh>,
    instances: usize,
    groups: usize,
    component_defs: usize,
}

struct IrMesh {
    name: String,
    vertices: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

//! 測試 SketchUp SDK DLL 載入 + SKP 檔案讀取

fn main() {
    println!("=== Kolibri SKP SDK Test ===\n");

    // 1. 檢查 SDK 可用性
    println!("[1] Checking SDK availability...");
    match kolibri_skp::sdk_available() {
        true => println!("    SDK DLL found!"),
        false => {
            println!("    SDK DLL NOT found. Exiting.");
            std::process::exit(1);
        }
    }

    // 2. 如果有 .skp 檔案參數，嘗試讀取
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("\n[2] No .skp file specified. Usage: test_sdk <file.skp>");
        println!("    SDK is available but no file to test.");
        return;
    }

    let path = &args[1];
    println!("\n[2] Importing: {}", path);

    match kolibri_skp::import_skp(path) {
        Ok(scene) => {
            println!("    SUCCESS!");
            println!("    Units: {}", scene.units);
            println!("    Meshes: {}", scene.meshes.len());
            println!("    Instances: {}", scene.instances.len());
            println!("    Groups: {}", scene.groups.len());
            println!("    ComponentDefs: {}", scene.component_defs.len());
            println!("    Materials: {}", scene.materials.len());

            // 顯示前幾個 mesh 的詳情
            for (i, mesh) in scene.meshes.iter().take(5).enumerate() {
                println!("    Mesh[{}]: {} — {} verts, {} tris",
                    i, mesh.name, mesh.vertices.len(), mesh.indices.len() / 3);
            }

            // 輸出 JSON
            let json_path = format!("{}.kolibri_sdk_export.json", path);
            if let Ok(json) = serde_json::to_string_pretty(&scene) {
                if std::fs::write(&json_path, &json).is_ok() {
                    println!("\n    Exported to: {}", json_path);
                }
            }
        }
        Err(e) => {
            println!("    FAILED: {}", e);
        }
    }

    println!("\n=== Done ===");
}

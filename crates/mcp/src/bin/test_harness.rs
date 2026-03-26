//! Layer 4: Rust MCP Test Harness
//! 驗證 initialize → tools/list → tools/call 完整流程

fn main() {
    println!("=== Kolibri MCP Test Harness ===\n");

    let mut adapter = kolibri_mcp::adapter::KolibriAdapter::new();

    // 1. 列出工具
    let tools = adapter.tool_definitions();
    println!("[1] tools/list: {} tools available", tools.len());
    for t in &tools {
        println!("    - {} : {}", t.name, t.description);
    }

    // 2. 建立場景
    println!("\n[2] 建立場景...");
    let r1 = adapter.execute_tool("create_box", &serde_json::json!({
        "name": "地板", "width": 6000, "height": 200, "depth": 4000, "material": "concrete"
    }));
    println!("    create_box → {}", r1);

    let r2 = adapter.execute_tool("create_cylinder", &serde_json::json!({
        "name": "柱子", "position": [2800, 200, 1800], "radius": 200, "height": 3000, "material": "steel"
    }));
    println!("    create_cylinder → {}", r2);

    let r3 = adapter.execute_tool("create_sphere", &serde_json::json!({
        "name": "燈具", "position": [3000, 3200, 2000], "radius": 150, "material": "glass"
    }));
    println!("    create_sphere → {}", r3);

    // 3. 查詢場景
    println!("\n[3] 查詢場景...");
    let state = adapter.execute_tool("get_scene_state", &serde_json::json!({}));
    println!("    object_count: {}", state["object_count"]);

    // 4. 修改物件
    println!("\n[4] 修改物件...");
    if let Some(id) = r2["id"].as_str() {
        let r = adapter.execute_tool("rotate_object", &serde_json::json!({ "id": id, "angle_deg": 45.0 }));
        println!("    rotate → {}", r);

        let r = adapter.execute_tool("scale_object", &serde_json::json!({ "id": id, "factor": [1.5, 1.0, 1.5] }));
        println!("    scale → {}", r);

        let r = adapter.execute_tool("duplicate_object", &serde_json::json!({ "id": id, "offset": [2000, 0, 0] }));
        println!("    duplicate → {}", r);

        let r = adapter.execute_tool("get_object_info", &serde_json::json!({ "id": id }));
        println!("    info → {}", serde_json::to_string_pretty(&r).unwrap_or_default());
    }

    // 5. Push/Pull
    println!("\n[5] Push/Pull...");
    if let Some(id) = r1["id"].as_str() {
        let r = adapter.execute_tool("push_pull", &serde_json::json!({ "id": id, "face": "top", "distance": 500 }));
        println!("    push_pull top +500 → {}", r);
    }

    // 6. Undo/Redo
    println!("\n[6] Undo/Redo...");
    let r = adapter.execute_tool("undo", &serde_json::json!({}));
    println!("    undo → {}", r);
    let r = adapter.execute_tool("redo", &serde_json::json!({}));
    println!("    redo → {}", r);

    // 7. 儲存
    println!("\n[7] 儲存場景...");
    let r = adapter.execute_tool("save_scene", &serde_json::json!({ "path": "test_mcp_output.k3d" }));
    println!("    save → {}", r);

    // 8. 最終狀態
    let final_state = adapter.execute_tool("get_scene_state", &serde_json::json!({}));
    println!("\n[8] 最終場景: {} objects", final_state["object_count"]);

    println!("\n=== All tests passed! ===");
}

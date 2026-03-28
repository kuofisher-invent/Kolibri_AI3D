// Windows: 隱藏 console 視窗（僅 release 模式）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai_assist;
mod ai_log;
mod app;
mod builders;
mod editor;   // EditorState, Tool, DrawState 等型別定義
mod cad_import;
mod camera;
mod collision;
mod csg;
mod dimensions;
mod dwg_parser;
mod dxf_io;
mod file_io;
mod gltf_io;
mod halfedge;
mod hybrid;
mod import;
mod import_review;
mod inference;
mod inference_engine;
mod icons;
mod layout;
mod mcp_server;
mod mcp_http_bridge;
mod measure;
mod menu;
mod obj_io;
mod overlay;  // ArcInfo, compute_arc, draw_dashed_line
mod panels;
mod preview;
mod renderer;
mod scene;
mod scene_hierarchy;
mod snap;
mod stl_io;
mod test_bridge;
mod texture_manager;
mod tools;
mod viewer;   // ViewerState, RenderMode

use eframe::egui;
use serde::Serialize;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--mcp".to_string()) {
        // MCP stdio mode (no GUI) — standalone scene
        eprintln!("Kolibri_Ai3D MCP Server starting (stdio mode)");
        crate::mcp_server::run_mcp_standalone();
        return Ok(());
    }

    if let Some(path) = cli_arg_value(&args, "--verify-skp") {
        return run_verify_import("skp", path);
    }

    if let Some(path) = cli_arg_value(&args, "--verify-import") {
        return run_verify_import("auto", path);
    }

    if let Some(path) = cli_arg_value(&args, "--export-skp-bridge-json") {
        return run_export_skp_bridge_json(&args, path);
    }

    if let Some(path) = cli_arg_value(&args, "--verify-bridge-json") {
        return run_verify_bridge_json(&args, path);
    }

    if let Some(path) = cli_arg_value(&args, "--import-scene-out") {
        return run_import_scene_out(&args, path);
    }

    if let Some(path) = cli_arg_value(&args, "--open-scene") {
        std::env::set_var("KOLIBRI_STARTUP_SCENE", path);
    }

    // 結構化日誌（tracing 取代 env_logger）
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let icon = load_icon_from_file()
        .unwrap_or_else(|| create_app_icon());

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 800.0])
            .with_title("Kolibri_Ai3D")
            .with_icon(std::sync::Arc::new(icon)),
        ..Default::default()
    };

    eframe::run_native(
        "Kolibri_Ai3D",
        options,
        Box::new(|cc| Ok(Box::new(app::KolibriApp::new(cc)))),
    )
}

/// Generate a 64x64 app icon: blue-teal gradient cube with "K" letter
fn create_app_icon() -> egui::IconData {
    let size = 64u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;

            // Rounded rectangle mask
            let margin = 0.08;
            let corner = 0.18;
            let in_rect = fx >= margin && fx <= 1.0 - margin && fy >= margin && fy <= 1.0 - margin;
            let in_corner = {
                let cx = if fx < margin + corner { margin + corner } else if fx > 1.0 - margin - corner { 1.0 - margin - corner } else { fx };
                let cy = if fy < margin + corner { margin + corner } else if fy > 1.0 - margin - corner { 1.0 - margin - corner } else { fy };
                let dx = fx - cx;
                let dy = fy - cy;
                (dx * dx + dy * dy).sqrt() <= corner
            };

            if !in_rect && !in_corner {
                // Transparent
                continue;
            }

            // Blue-teal gradient background
            let r = (30.0 + fx * 20.0) as u8;
            let g = (80.0 + fy * 60.0 + fx * 30.0) as u8;
            let b = (160.0 + fx * 40.0 + fy * 30.0) as u8;

            // Draw "K" letter (white)
            let is_k = {
                // Vertical bar of K
                let bar = fx >= 0.25 && fx <= 0.35 && fy >= 0.2 && fy <= 0.8;
                // Upper diagonal of K
                let diag1 = {
                    let dx = fx - 0.35;
                    let dy = fy - 0.5;
                    dx >= 0.0 && dy <= 0.0 && (dx + dy).abs() < 0.07 && fx <= 0.72
                };
                // Lower diagonal of K
                let diag2 = {
                    let dx = fx - 0.35;
                    let dy = fy - 0.5;
                    dx >= 0.0 && dy >= 0.0 && (dx - dy).abs() < 0.07 && fx <= 0.72
                };
                bar || diag1 || diag2
            };

            if is_k {
                rgba[idx]     = 255; // R
                rgba[idx + 1] = 255; // G
                rgba[idx + 2] = 255; // B
                rgba[idx + 3] = 240; // A
            } else {
                rgba[idx]     = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }
    }

    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}

/// Load icon from icon.png file next to the executable or in the app directory
fn load_icon_from_file() -> Option<egui::IconData> {
    // Try multiple locations
    let paths = [
        "icon.png".to_string(),
        "D:\\AI_Design\\Kolibri_Ai3D\\app\\icon.png".to_string(),
    ];

    for path in &paths {
        if let Ok(img) = image::open(path) {
            let img = img.resize(64, 64, image::imageops::FilterType::Lanczos3);
            let rgba_img = img.to_rgba8();
            let (w, h) = rgba_img.dimensions();
            return Some(egui::IconData {
                rgba: rgba_img.into_raw(),
                width: w,
                height: h,
            });
        }
    }
    None
}

fn cli_arg_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}

fn run_verify_import(kind: &str, path: &str) -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let ir = crate::import::import_manager::import_file(path)
        .map_err(io_error_to_eframe)?;

    if kind == "skp" && ir.source_format != "skp" {
        return Err(io_error_to_eframe(format!(
            "Expected SKP import, got {}",
            ir.source_format
        )));
    }

    let report = build_verify_report(
        kind.to_string(),
        ir,
        std::env::args().any(|arg| arg == "--verify-no-scene"),
    );
    print_verify_report(&report);

    if let Some(output_path) = cli_arg_value(&std::env::args().collect::<Vec<_>>(), "--verify-out") {
        let json = serde_json::to_string_pretty(&report).map_err(|e| io_error_to_eframe(e.to_string()))?;
        std::fs::write(output_path, json).map_err(|e| io_error_to_eframe(e.to_string()))?;
    }

    Ok(())
}

fn run_export_skp_bridge_json(args: &[String], path: &str) -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let export_path = crate::import::sketchup_bridge_backend::export_bridge_json(path)
        .map_err(io_error_to_eframe)?;

    if let Some(output_path) = cli_arg_value(args, "--verify-out") {
        std::fs::copy(&export_path, output_path).map_err(|e| io_error_to_eframe(e.to_string()))?;
        println!("bridge_json={}", output_path);
    } else {
        println!("bridge_json={}", export_path.display());
    }

    Ok(())
}

fn run_verify_bridge_json(args: &[String], json_path: &str) -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let source_file = cli_arg_value(args, "--source-file");
    let ir = crate::import::sketchup_bridge_backend::import_bridge_json_file(json_path, source_file)
        .map_err(io_error_to_eframe)?;

    let report = build_verify_report(
        "bridge_json".to_string(),
        ir,
        args.iter().any(|arg| arg == "--verify-no-scene"),
    );
    print_verify_report(&report);

    if let Some(output_path) = cli_arg_value(args, "--verify-out") {
        let json = serde_json::to_string_pretty(&report).map_err(|e| io_error_to_eframe(e.to_string()))?;
        std::fs::write(output_path, json).map_err(|e| io_error_to_eframe(e.to_string()))?;
    }

    Ok(())
}

fn run_import_scene_out(args: &[String], output_path: &str) -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .init();

    let input_path = cli_arg_value(args, "--source-file");
    let bridge_json_path = cli_arg_value(args, "--bridge-json");

    let ir = if let Some(json_path) = bridge_json_path {
        crate::import::sketchup_bridge_backend::import_bridge_json_file(json_path, input_path)
            .map_err(io_error_to_eframe)?
    } else {
        let input_path = input_path
            .ok_or_else(|| io_error_to_eframe("Missing --source-file or --bridge-json for --import-scene-out"))?;
        crate::import::import_manager::import_file(input_path)
            .map_err(io_error_to_eframe)?
    };
    let report = build_verify_report("scene_export".to_string(), ir.clone(), false);
    print_verify_report(&report);

    let mut scene = crate::scene::Scene::default();
    println!("scene_export_phase=build_scene_start");
    let build = crate::import::import_manager::build_scene_from_ir(&mut scene, &ir);
    println!(
        "scene_export_phase=build_scene_done objects={} groups={} component_defs={} built_meshes={}",
        scene.objects.len(),
        scene.groups.len(),
        scene.component_defs.len(),
        build.meshes
    );
    let debug_path = std::path::Path::new(output_path)
        .with_file_name("import_source_debug_cli.json");
    if let Ok(json) = serde_json::to_string_pretty(&build.object_debug) {
        let _ = std::fs::write(&debug_path, json);
        println!("scene_export_phase=debug_dump_done path={}", debug_path.display());
    }
    println!("scene_export_phase=save_scene_start path={}", output_path);
    scene
        .save_to_file(output_path)
        .map_err(|e| io_error_to_eframe(e.to_string()))?;
    println!("scene_export_phase=save_scene_done path={}", output_path);
    println!("scene_json={}", output_path);

    Ok(())
}

fn build_verify_report(
    verify_kind: String,
    ir: crate::import::unified_ir::UnifiedIR,
    skip_scene: bool,
) -> VerifyReport {
    let (scene_objects, scene_groups, scene_component_defs, built_meshes, built_columns, built_beams, build_timings_ms) =
        if skip_scene {
            (0, 0, 0, 0, 0, 0, Vec::new())
        } else {
            let mut scene = crate::scene::Scene::default();
            let build = crate::import::import_manager::build_scene_from_ir(&mut scene, &ir);
            (
                scene.objects.len(),
                scene.groups.len(),
                scene.component_defs.len(),
                build.meshes,
                build.columns,
                build.beams,
                build.phase_timings_ms,
            )
        };

    VerifyReport {
        verify_kind,
        source_format: ir.source_format.clone(),
        source_file: ir.source_file.clone(),
        units: ir.units.clone(),
        meshes: ir.stats.mesh_count,
        instances: ir.stats.instance_count,
        groups: ir.stats.group_count,
        component_defs: ir.stats.component_count,
        materials: ir.stats.material_count,
        vertices: ir.stats.vertex_count,
        faces: ir.stats.face_count,
        scene_objects,
        scene_groups,
        scene_component_defs,
        built_meshes,
        built_columns,
        built_beams,
        build_timings_ms,
        debug: ir.debug_report.clone(),
    }
}

fn print_verify_report(report: &VerifyReport) {
    println!("verify_kind={}", report.verify_kind);
    println!("source_format={}", report.source_format);
    println!("source_file={}", report.source_file);
    println!("units={}", report.units);
    println!("meshes={}", report.meshes);
    println!("instances={}", report.instances);
    println!("groups={}", report.groups);
    println!("component_defs={}", report.component_defs);
    println!("materials={}", report.materials);
    println!("vertices={}", report.vertices);
    println!("faces={}", report.faces);
    println!("scene_objects={}", report.scene_objects);
    println!("scene_groups={}", report.scene_groups);
    println!("scene_component_defs={}", report.scene_component_defs);
    println!("built_meshes={}", report.built_meshes);
    println!("built_columns={}", report.built_columns);
    println!("built_beams={}", report.built_beams);
    for (phase, elapsed_ms) in &report.build_timings_ms {
        println!("build_timing={}={}", phase, elapsed_ms);
    }
    for line in &report.debug {
        println!("debug={}", line);
    }
}

fn io_error_to_eframe(message: impl Into<String>) -> eframe::Error {
    eframe::Error::AppCreation(Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        message.into(),
    )))
}

#[derive(Serialize)]
struct VerifyReport {
    verify_kind: String,
    source_format: String,
    source_file: String,
    units: String,
    meshes: usize,
    instances: usize,
    groups: usize,
    component_defs: usize,
    materials: usize,
    vertices: usize,
    faces: usize,
    scene_objects: usize,
    scene_groups: usize,
    scene_component_defs: usize,
    built_meshes: usize,
    built_columns: usize,
    built_beams: usize,
    build_timings_ms: Vec<(String, u128)>,
    debug: Vec<String>,
}

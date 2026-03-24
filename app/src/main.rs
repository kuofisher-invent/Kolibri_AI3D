// Windows: 隱藏 console 視窗（僅 release 模式）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai_assist;
mod ai_log;
mod app;
mod builders;
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
mod inference;
mod icons;
mod mcp_server;
mod measure;
mod menu;
mod obj_io;
mod panels;
mod preview;
mod renderer;
mod scene;
mod snap;
mod stl_io;
mod test_bridge;
mod tools;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--mcp".to_string()) {
        // MCP stdio mode (no GUI) — standalone scene
        eprintln!("Kolibri_Ai3D MCP Server starting (stdio mode)");
        crate::mcp_server::run_mcp_standalone();
        return Ok(());
    }

    env_logger::init();

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

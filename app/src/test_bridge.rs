//! File-based test bridge for automated testing.
//!
//! Protocol:
//!   1. Tester writes commands to `test_input.json`
//!   2. App detects, executes, writes `test_output.json`
//!   3. App saves `test_screenshot.png` if requested
//!   4. App deletes `test_input.json` to signal completion
//!
//! This lets Claude read scene state + screenshots to verify behavior.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::camera::OrbitCamera;
use crate::scene::{MaterialKind, Scene, Shape};

/// Base directory for test files — next to the executable
fn test_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        // Fallback: look in common project locations
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn input_path() -> std::path::PathBuf {
    // Check multiple locations
    let candidates = [
        std::path::PathBuf::from("test_input.json"),
        std::path::PathBuf::from("d:/AI_Design/Kolibri_Ai3D/app/test_input.json"),
    ];
    for c in &candidates {
        if c.exists() { return c.clone(); }
    }
    candidates[0].clone()
}

const SCREENSHOT_FILE: &str = "D:\\AI_Design\\Kolibri_Ai3D\\app\\test_screenshot.png";
const OUTPUT_FILE: &str = "D:\\AI_Design\\Kolibri_Ai3D\\app\\test_output.json";
const INPUT_FILE: &str = "D:\\AI_Design\\Kolibri_Ai3D\\app\\test_input.json";

// ─── Command types ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TestInput {
    pub commands: Vec<TestCommand>,
}

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum TestCommand {
    CreateBox {
        name: Option<String>,
        #[serde(default)]
        position: [f32; 3],
        width: f32,
        height: f32,
        depth: f32,
        material: Option<String>,
    },
    CreateCylinder {
        name: Option<String>,
        #[serde(default)]
        position: [f32; 3],
        radius: f32,
        height: f32,
        #[serde(default = "default_seg")]
        segments: u32,
        material: Option<String>,
    },
    CreateSphere {
        name: Option<String>,
        #[serde(default)]
        position: [f32; 3],
        radius: f32,
        #[serde(default = "default_seg")]
        segments: u32,
        material: Option<String>,
    },
    MoveObject {
        id: String,
        position: [f32; 3],
    },
    SetMaterial {
        id: String,
        material: String,
    },
    Resize {
        id: String,
        width: Option<f32>,
        height: Option<f32>,
        depth: Option<f32>,
        radius: Option<f32>,
    },
    Delete { id: String },
    Clear,
    Select { id: Option<String> },
    SetCamera {
        target: Option<[f32; 3]>,
        distance: Option<f32>,
        yaw: Option<f32>,
        pitch: Option<f32>,
    },
    Screenshot {
        path: Option<String>,
    },
    /// Full window screenshot using OS-level capture
    WindowScreenshot {
        path: Option<String>,
    },
    CreateLine {
        name: Option<String>,
        points: Vec<[f32; 3]>,
        #[serde(default = "default_thickness")]
        thickness: f32,
        material: Option<String>,
    },
    RotateObject { id: String },
    ScaleObject { id: String, factor: f32 },
    CloneObject { id: String, offset: [f32; 3] },
    GetState,
    Undo,
    Redo,
    /// Returns undo/redo stack sizes.
    GetUndoState,
    /// Smart import a file (DXF/DWG/SKP/OBJ/PDF)
    SmartImport { path: String },
}

fn default_seg() -> u32 { 48 }
fn default_thickness() -> f32 { 20.0 }

// ─── Result types ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct TestOutput {
    pub success: bool,
    pub results: Vec<CmdResult>,
}

#[derive(Serialize)]
pub struct CmdResult {
    pub cmd: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<SceneSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_path: Option<String>,
}

#[derive(Serialize)]
pub struct SceneSnapshot {
    pub object_count: usize,
    pub objects: Vec<ObjectInfo>,
    pub camera: CameraInfo,
}

#[derive(Serialize)]
pub struct ObjectInfo {
    pub id: String,
    pub name: String,
    pub shape_type: String,
    pub position: [f32; 3],
    pub dimensions: String,
    pub material: String,
}

#[derive(Serialize)]
pub struct CameraInfo {
    pub target: [f32; 3],
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

// ─── Check for pending commands ──────────────────────────────────────────────

pub fn check_pending() -> Option<TestInput> {
    // Try to read directly — if it fails, file doesn't exist
    match std::fs::read_to_string(INPUT_FILE) {
        Ok(content) if !content.trim().is_empty() => {
            match serde_json::from_str(&content) {
                Ok(input) => Some(input),
                Err(e) => {
                    log::error!("Test input parse error: {}", e);
                    // Write error and remove input
                    let _ = write_error(&format!("Parse error: {}", e));
                    let _ = std::fs::remove_file(INPUT_FILE);
                    None
                }
            }
        }
        _ => None,
    }
}

pub fn signal_done() {
    let _ = std::fs::remove_file(INPUT_FILE);
}

// ─── Execute commands ────────────────────────────────────────────────────────

pub fn execute(
    input: TestInput,
    scene: &mut Scene,
    camera: &mut OrbitCamera,
    selected_id: &mut Option<String>,
    screenshot_fn: &mut dyn FnMut(&str),
) -> TestOutput {
    let mut results = Vec::new();

    for cmd in input.commands {
        let result = execute_one(cmd, scene, camera, selected_id, screenshot_fn);
        results.push(result);
    }

    let output = TestOutput { success: true, results };

    // Write output
    if let Ok(json) = serde_json::to_string_pretty(&output) {
        let _ = std::fs::write(OUTPUT_FILE, json);
    }

    output
}

fn execute_one(
    cmd: TestCommand,
    scene: &mut Scene,
    camera: &mut OrbitCamera,
    selected_id: &mut Option<String>,
    screenshot_fn: &mut dyn FnMut(&str),
) -> CmdResult {
    match cmd {
        TestCommand::CreateBox { name, position, width, height, depth, material } => {
            let mat = parse_material(material.as_deref());
            let n = name.unwrap_or_else(|| format!("TestBox_{}", scene.objects.len()+1));
            let id = scene.add_box(n, position, width, height, depth, mat);
            CmdResult { cmd: "create_box".into(), success: true, id: Some(id),
                message: Some(format!("{}x{}x{}", width, height, depth)),
                scene: None, screenshot_path: None }
        }

        TestCommand::CreateCylinder { name, position, radius, height, segments, material } => {
            let mat = parse_material(material.as_deref());
            let n = name.unwrap_or_else(|| format!("TestCyl_{}", scene.objects.len()+1));
            let id = scene.add_cylinder(n, position, radius, height, segments, mat);
            CmdResult { cmd: "create_cylinder".into(), success: true, id: Some(id),
                message: Some(format!("r={} h={}", radius, height)),
                scene: None, screenshot_path: None }
        }

        TestCommand::CreateSphere { name, position, radius, segments, material } => {
            let mat = parse_material(material.as_deref());
            let n = name.unwrap_or_else(|| format!("TestSph_{}", scene.objects.len()+1));
            let id = scene.add_sphere(n, position, radius, segments, mat);
            CmdResult { cmd: "create_sphere".into(), success: true, id: Some(id),
                message: Some(format!("r={}", radius)),
                scene: None, screenshot_path: None }
        }

        TestCommand::MoveObject { id, position } => {
            if scene.objects.contains_key(&id) {
                scene.snapshot();
                let obj = scene.objects.get_mut(&id).unwrap();
                obj.position = position;
                ok("move_object", Some(&id), "Moved")
            } else {
                err("move_object", &format!("Object '{}' not found", id))
            }
        }

        TestCommand::SetMaterial { id, material } => {
            if scene.objects.contains_key(&id) {
                scene.snapshot();
                let obj = scene.objects.get_mut(&id).unwrap();
                obj.material = parse_material(Some(&material));
                ok("set_material", Some(&id), &format!("Set to {}", material))
            } else {
                err("set_material", &format!("Object '{}' not found", id))
            }
        }

        TestCommand::Resize { id, width, height, depth, radius } => {
            if scene.objects.contains_key(&id) {
                scene.snapshot();
            }
            if let Some(obj) = scene.objects.get_mut(&id) {
                match &mut obj.shape {
                    Shape::Box { width: w, height: h, depth: d } => {
                        if let Some(v) = width { *w = v; }
                        if let Some(v) = height { *h = v; }
                        if let Some(v) = depth { *d = v; }
                    }
                    Shape::Cylinder { radius: r, height: h, .. } => {
                        if let Some(v) = radius { *r = v; }
                        if let Some(v) = height { *h = v; }
                    }
                    Shape::Sphere { radius: r, .. } => {
                        if let Some(v) = radius { *r = v; }
                    }
                    Shape::Line { thickness, .. } => {
                        if let Some(v) = width { *thickness = v; }
                    }
                    Shape::Mesh(_) => {}
                }
                ok("resize", Some(&id), "Resized")
            } else {
                err("resize", &format!("Object '{}' not found", id))
            }
        }

        TestCommand::Delete { id } => {
            if scene.delete(&id) {
                if selected_id.as_deref() == Some(&id) { *selected_id = None; }
                ok("delete", Some(&id), "Deleted")
            } else {
                err("delete", &format!("Object '{}' not found", id))
            }
        }

        TestCommand::Clear => {
            scene.clear();
            *selected_id = None;
            ok("clear", None, "Scene cleared")
        }

        TestCommand::Select { id } => {
            *selected_id = id.clone();
            ok("select", id.as_deref(), "Selected")
        }

        TestCommand::SetCamera { target, distance, yaw, pitch } => {
            if let Some(t) = target { camera.target = glam::Vec3::from(t); }
            if let Some(d) = distance { camera.distance = d; }
            if let Some(y) = yaw { camera.yaw = y; }
            if let Some(p) = pitch { camera.pitch = p; }
            ok("set_camera", None, "Camera updated")
        }

        TestCommand::Screenshot { path } => {
            let p = path.unwrap_or_else(|| SCREENSHOT_FILE.into());
            screenshot_fn(&p);
            CmdResult { cmd: "screenshot".into(), success: true, id: None,
                message: Some("Saved".into()), scene: None, screenshot_path: Some(p) }
        }

        TestCommand::CreateLine { name, points, thickness, material } => {
            let mat = parse_material(material.as_deref());
            let n = name.unwrap_or_else(|| format!("TestLine_{}", scene.objects.len()+1));
            let id = scene.add_line(n, points.clone(), thickness, mat);
            CmdResult { cmd: "create_line".into(), success: true, id: Some(id),
                message: Some(format!("{} points, {}mm thick", points.len(), thickness)),
                scene: None, screenshot_path: None }
        }

        TestCommand::RotateObject { id } => {
            if scene.objects.contains_key(&id) {
                scene.snapshot();
                let obj = scene.objects.get_mut(&id).unwrap();
                if let Shape::Box { ref mut width, ref mut depth, .. } = obj.shape {
                    std::mem::swap(width, depth);
                }
                ok("rotate_object", Some(&id), "Rotated 90°")
            } else { err("rotate_object", &format!("Not found: {}", id)) }
        }

        TestCommand::ScaleObject { id, factor } => {
            if scene.objects.contains_key(&id) {
                scene.snapshot();
            }
            if let Some(obj) = scene.objects.get_mut(&id) {
                match &mut obj.shape {
                    Shape::Box { width, height, depth } => {
                        *width *= factor; *height *= factor; *depth *= factor;
                    }
                    Shape::Cylinder { radius, height, .. } => {
                        *radius *= factor; *height *= factor;
                    }
                    Shape::Sphere { radius, .. } => *radius *= factor,
                    Shape::Line { thickness, .. } => *thickness *= factor,
                    Shape::Mesh(_) => {}
                }
                ok("scale_object", Some(&id), &format!("Scaled by {}", factor))
            } else { err("scale_object", &format!("Not found: {}", id)) }
        }

        TestCommand::CloneObject { id, offset } => {
            if scene.objects.contains_key(&id) {
                scene.snapshot();
            }
            if let Some(obj) = scene.objects.get(&id) {
                let mut clone = obj.clone();
                clone.id = scene.next_id_pub();
                clone.name = format!("{}_copy", clone.name);
                clone.position[0] += offset[0];
                clone.position[1] += offset[1];
                clone.position[2] += offset[2];
                let new_id = clone.id.clone();
                scene.objects.insert(new_id.clone(), clone);
                scene.version += 1;
                ok("clone_object", Some(&new_id), "Cloned")
            } else { err("clone_object", &format!("Not found: {}", id)) }
        }

        TestCommand::WindowScreenshot { path } => {
            let p = path.unwrap_or_else(|| "D:\\AI_Design\\Kolibri_Ai3D\\app\\test_window.png".into());
            // Use PowerShell to capture the active window
            let ps_script = format!(
                r#"Add-Type -AssemblyName System.Windows.Forms; Start-Sleep -Milliseconds 200; [System.Windows.Forms.Screen]::PrimaryScreen | Out-Null; Add-Type -AssemblyName System.Drawing; $bmp = New-Object System.Drawing.Bitmap([System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Width, [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Height); $g = [System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen(0,0,0,0,$bmp.Size); $bmp.Save('{}'); $g.Dispose(); $bmp.Dispose()"#,
                p.replace('\\', "\\\\")
            );
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps_script])
                .output();
            CmdResult { cmd: "window_screenshot".into(), success: true, id: None,
                message: Some("Full screen captured".into()), scene: None,
                screenshot_path: Some(p) }
        }

        TestCommand::GetState => {
            let objects: Vec<ObjectInfo> = scene.objects.values().map(|o| {
                let (shape_type, dims) = match &o.shape {
                    Shape::Box { width, height, depth } =>
                        ("box", format!("{}x{}x{}", width, height, depth)),
                    Shape::Cylinder { radius, height, segments } =>
                        ("cylinder", format!("r={} h={} seg={}", radius, height, segments)),
                    Shape::Sphere { radius, segments } =>
                        ("sphere", format!("r={} seg={}", radius, segments)),
                    Shape::Line { points, thickness, .. } =>
                        ("line", format!("{} pts, {}mm", points.len(), thickness)),
                    Shape::Mesh(ref mesh) =>
                        ("mesh", format!("{} verts, {} faces", mesh.vertices.len(), mesh.faces.len())),
                };
                ObjectInfo {
                    id: o.id.clone(), name: o.name.clone(),
                    shape_type: shape_type.into(), position: o.position,
                    dimensions: dims, material: o.material.label().into(),
                }
            }).collect();

            let snapshot = SceneSnapshot {
                object_count: objects.len(),
                objects,
                camera: CameraInfo {
                    target: [camera.target.x, camera.target.y, camera.target.z],
                    distance: camera.distance,
                    yaw: camera.yaw,
                    pitch: camera.pitch,
                },
            };

            CmdResult { cmd: "get_state".into(), success: true, id: None,
                message: None, scene: Some(snapshot), screenshot_path: None }
        }

        TestCommand::Undo => {
            if scene.undo() {
                ok("undo", None, &format!("Undone ({}  undo steps remaining)", scene.undo_count()))
            } else {
                err("undo", "Nothing to undo")
            }
        }

        TestCommand::Redo => {
            if scene.redo() {
                ok("redo", None, &format!("Redone ({} redo steps remaining)", scene.redo_count()))
            } else {
                err("redo", "Nothing to redo")
            }
        }

        TestCommand::GetUndoState => {
            ok("get_undo_state", None, &format!(
                "undo_count={}, redo_count={}", scene.undo_count(), scene.redo_count()
            ))
        }

        TestCommand::SmartImport { path } => {
            match crate::import::import_manager::import_file(&path) {
                Ok(ir) => {
                    let summary = format!(
                        "format={}, vertices={}, meshes={}, curves={}, members={}",
                        ir.source_format, ir.stats.vertex_count,
                        ir.stats.mesh_count, ir.curves.len(), ir.stats.member_count
                    );
                    let build = crate::import::import_manager::build_scene_from_ir(scene, &ir);
                    let msg = format!(
                        "Imported: {} | Built: meshes={}, columns={}, beams={}",
                        summary, build.meshes, build.columns, build.beams
                    );
                    CmdResult {
                        cmd: "smart_import".into(),
                        success: true,
                        id: build.ids.first().cloned(),
                        message: Some(msg),
                        scene: None,
                        screenshot_path: None,
                    }
                }
                Err(e) => err("smart_import", &e),
            }
        }
    }
}

fn ok(cmd: &str, id: Option<&str>, msg: &str) -> CmdResult {
    CmdResult { cmd: cmd.into(), success: true, id: id.map(|s| s.into()),
        message: Some(msg.into()), scene: None, screenshot_path: None }
}

fn err(cmd: &str, msg: &str) -> CmdResult {
    CmdResult { cmd: cmd.into(), success: false, id: None,
        message: Some(msg.into()), scene: None, screenshot_path: None }
}

fn write_error(msg: &str) -> std::io::Result<()> {
    let output = TestOutput { success: false, results: vec![
        CmdResult { cmd: "error".into(), success: false, id: None,
            message: Some(msg.into()), scene: None, screenshot_path: None }
    ]};
    std::fs::write(OUTPUT_FILE, serde_json::to_string_pretty(&output).unwrap_or_default())
}

fn parse_material(name: Option<&str>) -> MaterialKind {
    match name.unwrap_or("concrete") {
        "concrete" | "混凝土" => MaterialKind::Concrete,
        "wood" | "木材"       => MaterialKind::Wood,
        "glass" | "玻璃"     => MaterialKind::Glass,
        "metal" | "金屬"     => MaterialKind::Metal,
        "brick" | "磚"       => MaterialKind::Brick,
        "white" | "白色"     => MaterialKind::White,
        _ => MaterialKind::Concrete,
    }
}

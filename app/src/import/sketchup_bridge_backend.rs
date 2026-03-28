use super::skp_backend::SkpBackend;
use super::unified_ir::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const BRIDGE_LOADER: &str = include_str!("../../assets/sketchup_bridge/kolibri_skp_bridge.rb");
const BRIDGE_MAIN: &str = include_str!("../../assets/sketchup_bridge/kolibri_skp_bridge/main.rb");

pub struct SketchUpBridgeBackend;

impl SkpBackend for SketchUpBridgeBackend {
    fn name(&self) -> &'static str {
        "sketchup_bridge"
    }

    fn import(&self, path: &str) -> Result<UnifiedIR, String> {
        import_via_sketchup_bridge(path)
    }
}

pub fn export_bridge_json(path: &str) -> Result<PathBuf, String> {
    ensure_bridge_plugin_installed()?;
    let sketchup_exe = find_sketchup_exe()?;

    let input_path = std::fs::canonicalize(path)
        .map_err(|e| format!("Failed to resolve SKP path: {}", e))?;
    let output_path =
        std::env::temp_dir().join(format!("kolibri_skp_export_{}.json", uuid::Uuid::new_v4()));
    if output_path.exists() {
        let _ = std::fs::remove_file(&output_path);
    }

    let mut child = Command::new(&sketchup_exe)
        .arg(&input_path)
        .env("KOLIBRI_SKP_EXPORT_IN", &input_path)
        .env("KOLIBRI_SKP_EXPORT_OUT", &output_path)
        .spawn()
        .map_err(|e| format!("Failed to launch SketchUp bridge: {}", e))?;

    let deadline = Instant::now() + Duration::from_secs(120);
    let mut child_status = None;
    while Instant::now() < deadline {
        if output_path.exists() {
            wait_for_stable_file(&output_path, deadline)?;
            return Ok(output_path);
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("Failed while waiting for SketchUp bridge: {}", e))?
        {
            child_status = Some(status);
            if output_path.exists() {
                wait_for_stable_file(&output_path, deadline)?;
                return Ok(output_path);
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    let _ = child.kill();
    let status_suffix = child_status
        .map(|s| format!(" (exit code: {:?})", s.code()))
        .unwrap_or_default();
    Err(format!(
        "SketchUp bridge did not produce export JSON within 120s{}",
        status_suffix
    ))
}

pub fn import_bridge_json_file(
    json_path: &str,
    source_file: Option<&str>,
) -> Result<UnifiedIR, String> {
    let source_path = match source_file {
        Some(path) => std::fs::canonicalize(path)
            .map_err(|e| format!("Failed to resolve source SKP path: {}", e))?,
        None => std::fs::canonicalize(json_path)
            .map_err(|e| format!("Failed to resolve bridge JSON path: {}", e))?,
    };

    let json = std::fs::read_to_string(json_path)
        .map_err(|e| format!("Failed to read SketchUp bridge JSON: {}", e))?;
    bridge_json_to_ir(&json, source_path)
}

fn wait_for_stable_file(path: &Path, deadline: Instant) -> Result<(), String> {
    let mut stable_polls = 0usize;
    let mut last_len = None;
    let mut last_modified = None;

    while Instant::now() < deadline {
        let metadata = std::fs::metadata(path)
            .map_err(|e| format!("Failed to inspect SketchUp bridge JSON: {}", e))?;
        let len = metadata.len();
        let modified = metadata.modified().ok();

        if len > 0 && Some(len) == last_len && modified == last_modified {
            stable_polls += 1;
            if stable_polls >= 6 {
                return Ok(());
            }
        } else {
            stable_polls = 0;
            last_len = Some(len);
            last_modified = modified;
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    Err(format!(
        "SketchUp bridge JSON did not stabilize within the allotted time: {}",
        path.display()
    ))
}

#[derive(Debug, Deserialize)]
struct BridgeExport {
    error: Option<String>,
    units: Option<String>,
    materials: Vec<BridgeMaterial>,
    meshes: Vec<BridgeMesh>,
    instances: Vec<BridgeInstance>,
    groups: Vec<BridgeGroup>,
    component_defs: Vec<BridgeComponentDef>,
}

#[derive(Debug, Deserialize)]
struct BridgeMaterial {
    id: String,
    name: String,
    color: [f32; 4],
    texture_path: Option<String>,
    opacity: f32,
}

#[derive(Debug, Deserialize)]
struct BridgeMesh {
    id: String,
    name: String,
    vertices: Vec<[f32; 3]>,
    #[serde(default)]
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
    material_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BridgeInstance {
    id: String,
    mesh_id: String,
    component_def_id: Option<String>,
    transform: [f32; 16],
    name: String,
    #[serde(default)]
    layer: String,
}

#[derive(Debug, Deserialize)]
struct BridgeGroup {
    id: String,
    name: String,
    #[serde(default)]
    children: Vec<String>,
    #[serde(default)]
    parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BridgeComponentDef {
    id: String,
    name: String,
    #[serde(default)]
    mesh_ids: Vec<String>,
    #[serde(default)]
    instance_count: usize,
}

fn import_via_sketchup_bridge(path: &str) -> Result<UnifiedIR, String> {
    let input_path = std::fs::canonicalize(path)
        .map_err(|e| format!("Failed to resolve SKP path: {}", e))?;
    let output_path = export_bridge_json(path)?;

    let json = std::fs::read_to_string(&output_path)
        .map_err(|e| format!("Failed to read SketchUp bridge JSON: {}", e))?;
    let _ = std::fs::remove_file(&output_path);

    bridge_json_to_ir(&json, input_path)
}

fn ensure_bridge_plugin_installed() -> Result<(), String> {
    let plugin_root = sketchup_plugin_dir()?;
    let bridge_dir = plugin_root.join("kolibri_skp_bridge");
    std::fs::create_dir_all(&bridge_dir)
        .map_err(|e| format!("Failed to create SketchUp plugin directory: {}", e))?;

    write_if_changed(&plugin_root.join("kolibri_skp_bridge.rb"), BRIDGE_LOADER)?;
    write_if_changed(&bridge_dir.join("main.rb"), BRIDGE_MAIN)?;
    Ok(())
}

fn write_if_changed(path: &Path, content: &str) -> Result<(), String> {
    let needs_write = match std::fs::read_to_string(path) {
        Ok(existing) => existing != content,
        Err(_) => true,
    };
    if needs_write {
        std::fs::write(path, content)
            .map_err(|e| format!("Failed to write {:?}: {}", path, e))?;
    }
    Ok(())
}

fn sketchup_plugin_dir() -> Result<PathBuf, String> {
    let appdata = std::env::var_os("APPDATA")
        .ok_or_else(|| "APPDATA is not set; cannot install SketchUp bridge plugin".to_string())?;
    Ok(PathBuf::from(appdata)
        .join("SketchUp")
        .join("SketchUp 2025")
        .join("SketchUp")
        .join("Plugins"))
}

fn find_sketchup_exe() -> Result<PathBuf, String> {
    let candidates = [
        PathBuf::from(r"C:\Program Files\SketchUp\SketchUp 2025\SketchUp\SketchUp.exe"),
        PathBuf::from(r"C:\Program Files\SketchUp\SketchUp 2024\SketchUp\SketchUp.exe"),
    ];
    candidates
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| "SketchUp.exe was not found; cannot run SKP bridge".to_string())
}

fn bridge_export_to_ir(export: BridgeExport, source_file: PathBuf) -> UnifiedIR {
    let mut ir = UnifiedIR {
        source_format: "skp".into(),
        source_file: source_file.to_string_lossy().to_string(),
        units: normalize_units(export.units.as_deref()),
        ..Default::default()
    };

    ir.materials = export
        .materials
        .into_iter()
        .map(|m| IrMaterial {
            id: m.id,
            name: m.name,
            color: m.color,
            texture_path: m.texture_path,
            opacity: m.opacity,
        })
        .collect();

    ir.meshes = export
        .meshes
        .into_iter()
        .map(|m| IrMesh {
            id: m.id,
            name: m.name,
            vertices: convert_vertices_to_mm(&ir.units, m.vertices),
            normals: m.normals,
            indices: m.indices,
            material_id: m.material_id,
            source_vertex_labels: vec![],
            source_triangle_debug: vec![],
        })
        .collect();

    ir.instances = export
        .instances
        .into_iter()
        .map(|inst| {
            let mut transform = inst.transform;
            scale_translation_to_mm(&ir.units, &mut transform);
            IrInstance {
                id: inst.id,
                mesh_id: inst.mesh_id,
                component_def_id: inst.component_def_id,
                transform,
                name: inst.name,
                layer: inst.layer,
            }
        })
        .collect();

    ir.groups = export
        .groups
        .into_iter()
        .map(|g| IrGroup {
            id: g.id,
            name: g.name,
            children: g.children,
            parent_id: g.parent_id,
        })
        .collect();

    ir.component_defs = export
        .component_defs
        .into_iter()
        .map(|c| IrComponentDef {
            id: c.id,
            name: c.name,
            mesh_ids: c.mesh_ids,
            instance_count: c.instance_count,
        })
        .collect();

    ir.stats.mesh_count = ir.meshes.len();
    ir.stats.face_count = ir.meshes.iter().map(|m| m.indices.len() / 3).sum();
    ir.stats.vertex_count = ir.meshes.iter().map(|m| m.vertices.len()).sum();
    ir.stats.instance_count = ir.instances.len();
    ir.stats.group_count = ir.groups.len();
    ir.stats.component_count = ir.component_defs.len();
    ir.stats.material_count = ir.materials.len();

    let layer_count = ir
        .instances
        .iter()
        .map(|i| i.layer.clone())
        .filter(|l| !l.is_empty())
        .collect::<std::collections::HashSet<_>>()
        .len();
    ir.debug_report = vec![
        "[SKP Bridge Import]".into(),
        format!("Source: {}", ir.source_file),
        format!("Backend: {}", SketchUpBridgeBackend.name()),
        format!("Units: {}", ir.units),
        format!("Meshes: {}", ir.stats.mesh_count),
        format!("Instances: {}", ir.stats.instance_count),
        format!("Groups: {}", ir.stats.group_count),
        format!("ComponentDefs: {}", ir.stats.component_count),
        format!("Materials: {}", ir.stats.material_count),
        format!("Layers/Tags: {}", layer_count),
    ];

    ir
}

fn bridge_json_to_ir(json: &str, source_file: PathBuf) -> Result<UnifiedIR, String> {
    let export: BridgeExport = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse SketchUp bridge JSON: {}", e))?;
    if let Some(err) = export.error {
        return Err(format!("SketchUp bridge reported an error: {}", err));
    }
    Ok(bridge_export_to_ir(export, source_file))
}

fn normalize_units(units: Option<&str>) -> String {
    match units.unwrap_or("inch").to_ascii_lowercase().as_str() {
        "mm" | "millimeter" | "millimeters" => "mm".into(),
        "cm" | "centimeter" | "centimeters" => "cm".into(),
        "m" | "meter" | "meters" => "m".into(),
        "foot" | "feet" | "ft" => "foot".into(),
        _ => "inch".into(),
    }
}

fn unit_scale_to_mm(units: &str) -> f32 {
    match units {
        "mm" => 1.0,
        "cm" => 10.0,
        "m" => 1000.0,
        "foot" => 304.8,
        _ => 25.4,
    }
}

fn convert_vertices_to_mm(units: &str, vertices: Vec<[f32; 3]>) -> Vec<[f32; 3]> {
    let scale = unit_scale_to_mm(units);
    vertices
        .into_iter()
        .map(|v| [v[0] * scale, v[1] * scale, v[2] * scale])
        .collect()
}

fn scale_translation_to_mm(units: &str, transform: &mut [f32; 16]) {
    let scale = unit_scale_to_mm(units);
    transform[12] *= scale;
    transform[13] *= scale;
    transform[14] *= scale;
}

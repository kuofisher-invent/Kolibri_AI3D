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
    ensure_bridge_plugin_installed()?;
    let sketchup_exe = find_sketchup_exe()?;

    let input_path = std::fs::canonicalize(path)
        .map_err(|e| format!("找不到 SKP 檔案: {}", e))?;
    let output_path = std::env::temp_dir().join(format!("kolibri_skp_export_{}.json", uuid::Uuid::new_v4()));
    if output_path.exists() {
        let _ = std::fs::remove_file(&output_path);
    }

    let mut child = Command::new(&sketchup_exe)
        .arg(&input_path)
        .env("KOLIBRI_SKP_EXPORT_IN", &input_path)
        .env("KOLIBRI_SKP_EXPORT_OUT", &output_path)
        .spawn()
        .map_err(|e| format!("無法啟動 SketchUp bridge: {}", e))?;

    let deadline = Instant::now() + Duration::from_secs(120);
    let mut child_status = None;
    while Instant::now() < deadline {
        if output_path.exists() {
            break;
        }
        if let Some(status) = child.try_wait().map_err(|e| format!("等待 SketchUp bridge 失敗: {}", e))? {
            child_status = Some(status);
            if output_path.exists() {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    if !output_path.exists() {
        let _ = child.kill();
        let status_suffix = child_status
            .map(|s| format!(" (exit code: {:?})", s.code()))
            .unwrap_or_default();
        return Err(format!(
            "SketchUp bridge 未在時間內輸出 JSON。請確認 SketchUp 2025 可正常開啟檔案{}。",
            status_suffix
        ));
    }

    let json = std::fs::read_to_string(&output_path)
        .map_err(|e| format!("讀取 SketchUp bridge JSON 失敗: {}", e))?;
    let _ = std::fs::remove_file(&output_path);

    let export: BridgeExport = serde_json::from_str(&json)
        .map_err(|e| format!("解析 SketchUp bridge JSON 失敗: {}", e))?;
    if let Some(err) = export.error {
        return Err(format!("SketchUp bridge 匯出失敗: {}", err));
    }

    Ok(bridge_export_to_ir(export, input_path))
}

fn ensure_bridge_plugin_installed() -> Result<(), String> {
    let plugin_root = sketchup_plugin_dir()?;
    let bridge_dir = plugin_root.join("kolibri_skp_bridge");
    std::fs::create_dir_all(&bridge_dir)
        .map_err(|e| format!("建立 SketchUp plugin 目錄失敗: {}", e))?;

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
        std::fs::write(path, content).map_err(|e| format!("寫入 {:?} 失敗: {}", path, e))?;
    }
    Ok(())
}

fn sketchup_plugin_dir() -> Result<PathBuf, String> {
    let appdata = std::env::var_os("APPDATA")
        .ok_or_else(|| "找不到 APPDATA，無法安裝 SketchUp bridge plugin".to_string())?;
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
        .ok_or_else(|| "找不到 SketchUp.exe，無法使用 SKP bridge".to_string())
}

fn bridge_export_to_ir(export: BridgeExport, source_file: PathBuf) -> UnifiedIR {
    let mut ir = UnifiedIR {
        source_format: "skp".into(),
        source_file: source_file.to_string_lossy().to_string(),
        units: normalize_units(export.units.as_deref()),
        ..Default::default()
    };

    ir.materials = export.materials.into_iter().map(|m| IrMaterial {
        id: m.id,
        name: m.name,
        color: m.color,
        texture_path: m.texture_path,
        opacity: m.opacity,
    }).collect();

    ir.meshes = export.meshes.into_iter().map(|m| IrMesh {
        id: m.id,
        name: m.name,
        vertices: convert_vertices_to_mm(&ir.units, m.vertices),
        normals: m.normals,
        indices: m.indices,
        material_id: m.material_id,
    }).collect();

    ir.instances = export.instances.into_iter().map(|inst| {
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
    }).collect();

    ir.groups = export.groups.into_iter().map(|g| IrGroup {
        id: g.id,
        name: g.name,
        children: g.children,
        parent_id: g.parent_id,
    }).collect();

    ir.component_defs = export.component_defs.into_iter().map(|c| IrComponentDef {
        id: c.id,
        name: c.name,
        mesh_ids: c.mesh_ids,
        instance_count: c.instance_count,
    }).collect();

    ir.stats.mesh_count = ir.meshes.len();
    ir.stats.face_count = ir.meshes.iter().map(|m| m.indices.len() / 3).sum();
    ir.stats.vertex_count = ir.meshes.iter().map(|m| m.vertices.len()).sum();
    ir.stats.instance_count = ir.instances.len();
    ir.stats.group_count = ir.groups.len();
    ir.stats.component_count = ir.component_defs.len();
    ir.stats.material_count = ir.materials.len();

    let layer_count = ir.instances.iter()
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
    vertices.into_iter().map(|v| [v[0] * scale, v[1] * scale, v[2] * scale]).collect()
}

fn scale_translation_to_mm(units: &str, transform: &mut [f32; 16]) {
    let scale = unit_scale_to_mm(units);
    transform[12] *= scale;
    transform[13] *= scale;
    transform[14] *= scale;
}

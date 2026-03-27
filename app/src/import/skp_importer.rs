//! SketchUp SKP file importer
//!
//! Preferred path:
//! 1. Install a small Ruby bridge into the user's SketchUp Plugins folder.
//! 2. Launch SketchUp with the target `.skp`.
//! 3. Let the Ruby bridge export a structured JSON scene graph.
//! 4. Convert that JSON into Kolibri `UnifiedIR`.
//!
//! A coarse heuristic fallback remains for environments where SketchUp is not
//! available, but the bridge path is the only route that preserves components,
//! groups, instances, materials, and transforms with reasonable fidelity.

use super::unified_ir::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const BRIDGE_LOADER: &str = include_str!("../../assets/sketchup_bridge/kolibri_skp_bridge.rb");
const BRIDGE_MAIN: &str = include_str!("../../assets/sketchup_bridge/kolibri_skp_bridge/main.rb");

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

/// Import a SketchUp .skp file
pub fn import_skp(path: &str) -> Result<UnifiedIR, String> {
    if let Ok(ir) = import_skp_via_sketchup_bridge(path) {
        return Ok(ir);
    }

    // Heuristic fallback: this remains useful for development, but it is not
    // considered a complete SKP parser.
    let file = std::fs::File::open(path).map_err(|e| format!("無法開啟 SKP: {}", e))?;
    if let Ok(mut archive) = zip::ZipArchive::new(file) {
        return import_skp_zip(&mut archive, path);
    }
    import_skp_legacy(path)
}

fn import_skp_via_sketchup_bridge(path: &str) -> Result<UnifiedIR, String> {
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

fn import_skp_zip(archive: &mut zip::ZipArchive<std::fs::File>, path: &str) -> Result<UnifiedIR, String> {
    let mut ir = UnifiedIR {
        source_format: "skp".into(),
        source_file: path.into(),
        units: "mm".into(),
        ..Default::default()
    };

    let mut entry_names = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            entry_names.push((i, entry.name().to_string(), entry.size()));
        }
    }

    let mut found_geometry = false;
    for (idx, name, _size) in &entry_names {
        if name.ends_with(".bin") || name.contains("geometry") || name.contains("model") {
            if let Ok(mut entry) = archive.by_index(*idx) {
                let mut data = Vec::new();
                use std::io::Read;
                let _ = entry.read_to_end(&mut data);
                if !data.is_empty() {
                    if let Some(mesh) = try_parse_skp_binary(&data, name) {
                        ir.meshes.push(mesh);
                        found_geometry = true;
                    }
                }
            }
        }
    }

    if !found_geometry {
        return Err(format!(
            "SKP bridge 無法使用，且 heuristic ZIP fallback 也未找到幾何。ZIP entries: {}",
            entry_names.len()
        ));
    }

    ir.stats.mesh_count = ir.meshes.len();
    ir.stats.vertex_count = ir.meshes.iter().map(|m| m.vertices.len()).sum();
    ir.stats.face_count = ir.meshes.iter().map(|m| m.indices.len() / 3).sum();
    Ok(ir)
}

fn try_parse_skp_binary(data: &[u8], _name: &str) -> Option<IrMesh> {
    if data.len() < 36 {
        return None;
    }

    let mut vertices = Vec::new();
    let mut i = 0usize;
    while i + 12 <= data.len() {
        let x = f32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        let y = f32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]);
        let z = f32::from_le_bytes([data[i + 8], data[i + 9], data[i + 10], data[i + 11]]);
        if x.is_finite() && y.is_finite() && z.is_finite()
            && x.abs() < 1_000_000.0 && y.abs() < 1_000_000.0 && z.abs() < 1_000_000.0
        {
            vertices.push([x, y, z]);
        }
        i += 12;
    }

    if vertices.len() < 3 {
        return None;
    }

    let mut indices = Vec::new();
    for tri in 1..vertices.len() - 1 {
        indices.push(0);
        indices.push(tri as u32);
        indices.push((tri + 1) as u32);
    }

    Some(IrMesh {
        id: format!("skp_mesh_{}", vertices.len()),
        name: "SKP Geometry".into(),
        vertices,
        normals: Vec::new(),
        indices,
        material_id: None,
    })
}

fn import_skp_legacy(_path: &str) -> Result<UnifiedIR, String> {
    Err(
        "無法完整解析此 SKP。請確認本機已安裝 SketchUp 2025，Kolibri 會使用 Ruby bridge 匯出 scene graph。".into()
    )
}

/// Import OBJ file as a SKP workflow alternative
pub fn import_obj_to_ir(path: &str) -> Result<UnifiedIR, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("讀取 OBJ 失敗: {}", e))?;

    let mut ir = UnifiedIR {
        source_format: "obj".into(),
        source_file: path.into(),
        units: "mm".into(),
        ..Default::default()
    };

    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut current_group = "default".to_string();
    let mut group_faces: HashMap<String, Vec<[u32; 3]>> = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("v ") {
            let parts: Vec<f32> = line[2..].split_whitespace().filter_map(|s| s.parse().ok()).collect();
            if parts.len() >= 3 {
                vertices.push([parts[0], parts[1], parts[2]]);
            }
        } else if line.starts_with("vn ") {
            let parts: Vec<f32> = line[3..].split_whitespace().filter_map(|s| s.parse().ok()).collect();
            if parts.len() >= 3 {
                normals.push([parts[0], parts[1], parts[2]]);
            }
        } else if line.starts_with("f ") {
            let face_verts: Vec<u32> = line[2..].split_whitespace()
                .filter_map(|s| {
                    let idx_str = s.split('/').next()?;
                    let idx: i32 = idx_str.parse().ok()?;
                    Some(if idx > 0 { (idx - 1) as u32 } else { 0 })
                })
                .collect();
            if face_verts.len() >= 3 {
                for fi in 1..face_verts.len() - 1 {
                    group_faces.entry(current_group.clone()).or_default()
                        .push([face_verts[0], face_verts[fi], face_verts[fi + 1]]);
                }
            }
        } else if line.starts_with("g ") || line.starts_with("o ") {
            current_group = line[2..].trim().to_string();
        }
    }

    if vertices.is_empty() {
        return Err("OBJ 沒有可用頂點資料".into());
    }

    if normals.len() != vertices.len() {
        normals = vec![[0.0, 1.0, 0.0]; vertices.len()];
    }

    let mut mesh_index = 0usize;
    for (group_name, tris) in group_faces {
        if tris.is_empty() {
            continue;
        }
        let mesh_id = format!("obj_mesh_{}", mesh_index);
        mesh_index += 1;

        let mut local_vertices = Vec::with_capacity(tris.len() * 3);
        let mut local_indices = Vec::with_capacity(tris.len() * 3);
        for tri in tris {
            let base = local_vertices.len() as u32;
            local_vertices.push(vertices[tri[0] as usize]);
            local_vertices.push(vertices[tri[1] as usize]);
            local_vertices.push(vertices[tri[2] as usize]);
            local_indices.extend_from_slice(&[base, base + 1, base + 2]);
        }

        ir.meshes.push(IrMesh {
            id: mesh_id.clone(),
            name: group_name.clone(),
            vertices: local_vertices,
            normals: Vec::new(),
            indices: local_indices,
            material_id: None,
        });

        let inst_id = format!("obj_inst_{}", mesh_index);
        ir.instances.push(IrInstance {
            id: inst_id.clone(),
            mesh_id: mesh_id.clone(),
            component_def_id: None,
            transform: [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0,
            ],
            name: group_name.clone(),
            layer: String::new(),
        });

        ir.groups.push(IrGroup {
            id: format!("grp_{}", group_name),
            name: group_name.clone(),
            children: vec![inst_id],
            parent_id: None,
        });
    }

    ir.stats.vertex_count = ir.meshes.iter().map(|m| m.vertices.len()).sum();
    ir.stats.face_count = ir.meshes.iter().map(|m| m.indices.len() / 3).sum();
    ir.stats.mesh_count = ir.meshes.len();
    ir.stats.instance_count = ir.instances.len();
    ir.stats.group_count = ir.groups.len();

    Ok(ir)
}

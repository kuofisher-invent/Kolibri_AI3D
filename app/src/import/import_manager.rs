//! Import Manager — detects file format and routes to appropriate importer

use super::import_cache::ImportCache;
use super::unified_ir::UnifiedIR;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

pub enum ImportFormat {
    Dxf,
    Dwg,
    Skp,
    Obj,
    Stl,
    Pdf,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImportedObjectDebug {
    pub mesh_id: String,
    pub mesh_name: String,
    pub vertex_labels: Vec<String>,
    pub triangle_debug: Vec<super::unified_ir::IrTriangleDebug>,
}

pub fn detect_format(path: &str) -> ImportFormat {
    let ext = std::path::Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "dxf" => ImportFormat::Dxf,
        "dwg" => ImportFormat::Dwg,
        "skp" => ImportFormat::Skp,
        "obj" => ImportFormat::Obj,
        "stl" => ImportFormat::Stl,
        "pdf" => ImportFormat::Pdf,
        _ => ImportFormat::Unknown,
    }
}

/// Universal import: auto-detect format and convert to IR
pub fn import_file(path: &str) -> Result<UnifiedIR, String> {
    let format = detect_format(path);

    match format {
        ImportFormat::Dxf => super::dwg_importer::import_dxf_to_unified_ir(path),
        ImportFormat::Dwg => super::dwg_parser::parse_dwg(path),
        ImportFormat::Skp => {
            // 優先使用 SDK 讀取（子進程隔離，防 DLL 崩潰）
            if kolibri_skp::sdk_available() {
                match kolibri_skp::import_skp_subprocess(path) {
                    Ok(skp_scene) => Ok(super::skp_sdk_import::skp_scene_to_ir(&skp_scene, path)),
                    Err(e) => {
                        tracing::warn!("SKP SDK failed: {}, falling back to bridge/heuristic", e);
                        super::skp_importer::import_skp(path)
                    }
                }
            } else {
                super::skp_importer::import_skp(path)
            }
        }
        ImportFormat::Obj => super::skp_importer::import_obj_to_ir(path),
        ImportFormat::Pdf => super::pdf_parser::parse_pdf(path),
        ImportFormat::Stl => {
            Err("STL 匯入尚未支援，請使用 STL 專用匯入".into())
        }
        ImportFormat::Unknown => {
            Err(format!("不支援的檔案格式: {}", path))
        }
    }
}

/// Build scene from unified IR
pub fn build_scene_from_ir(scene: &mut crate::scene::Scene, ir: &UnifiedIR) -> BuildResult {
    let cache = ImportCache::from_ir(ir);
    build_scene_from_cache(scene, ir, &cache)
}

pub fn build_scene_from_cache(
    scene: &mut crate::scene::Scene,
    ir: &UnifiedIR,
    cache: &ImportCache,
) -> BuildResult {
    let mut result = BuildResult::default();
    let build_started = Instant::now();

    // ── 平行建置 HeMesh 快取（rayon）──
    let phase_started = Instant::now();
    let mesh_entries: Vec<_> = cache.meshes_in_order()
        .map(|m| m.ir.clone())
        .collect();
    let parallel_meshes: Vec<(String, Option<crate::halfedge::HeMesh>)> = {
        use rayon::prelude::*;
        mesh_entries.par_iter()
            .map(|ir_mesh| {
                let he = ir_mesh_to_he_mesh(ir_mesh);
                (ir_mesh.id.clone(), he)
            })
            .collect()
    };
    let mut mesh_cache: HashMap<String, crate::halfedge::HeMesh> = HashMap::new();
    for (id, he) in parallel_meshes {
        if let Some(he) = he {
            mesh_cache.insert(id, he);
        }
    }
    result.phase_timings_ms.push(("parallel_he_mesh_build".into(), phase_started.elapsed().as_millis()));

    // Build members (columns, beams) using steel builder
    let phase_started = Instant::now();
    if !ir.members.is_empty() {
        let drawing_ir = convert_to_drawing_ir(ir);
        let steel_result = crate::builders::steel_builder::build_from_ir(scene, &drawing_ir);
        result.columns += steel_result.columns_created;
        result.beams += steel_result.beams_created;
        result.plates += steel_result.plates_created;
        result.ids.extend(steel_result.ids);
    }

    // Fallback: if no members but we have curves, try geometry-based semantic detection
    if ir.members.is_empty() && !ir.curves.is_empty() {
        let mut raw_geom = crate::cad_import::geometry_parser::RawGeometry {
            lines: Vec::new(),
            polylines: Vec::new(),
            texts: Vec::new(),
            dimensions: Vec::new(),
            blocks: Vec::new(),
            circles: Vec::new(),
        };
        for curve in &ir.curves {
            if curve.points.len() >= 2 {
                if curve.is_closed && curve.points.len() > 2 {
                    raw_geom.polylines.push(crate::cad_import::geometry_parser::RawPolyline {
                        points: curve.points.clone(),
                        closed: true,
                        layer: curve.layer.clone(),
                    });
                }
                for w in curve.points.windows(2) {
                    raw_geom.lines.push(crate::cad_import::geometry_parser::RawLine {
                        start: w[0],
                        end: w[1],
                        layer: curve.layer.clone(),
                        linetype: "CONTINUOUS".into(),
                    });
                }
            }
        }

        let semantic = crate::cad_import::semantic_detector::detect_from_geometry(&raw_geom);
        for line in &semantic.debug_lines {
            eprintln!("[SemanticDetector/IR] {}", line);
        }

        if !semantic.columns.is_empty() || !semantic.beams.is_empty() {
            let mut drawing_ir = crate::cad_import::ir::DrawingIR::default();
            drawing_ir.grids = semantic.grids;
            drawing_ir.columns = semantic.columns;
            drawing_ir.beams = semantic.beams;
            drawing_ir.base_plates = semantic.plates;
            drawing_ir.levels = semantic.levels;

            let steel_result = crate::builders::steel_builder::build_from_ir(scene, &drawing_ir);
            result.columns += steel_result.columns_created;
            result.beams += steel_result.beams_created;
            result.plates += steel_result.plates_created;
            result.ids.extend(steel_result.ids);
        }
    }
    result.phase_timings_ms.push(("build_members".into(), phase_started.elapsed().as_millis()));

    let phase_started = Instant::now();
    for comp in cache.component_defs_in_order() {
        let mut objects = Vec::new();
        for mesh_id in &comp.ir.mesh_ids {
            let Some(mesh) = cache.mesh(mesh_id) else { continue; };
            let Some(he_mesh) = cached_he_mesh(&mut mesh_cache, &mesh.ir) else { continue; };
            let mut obj = imported_mesh_object(
                scene.next_id_pub(),
                mesh.ir.name.clone(),
                [0.0, 0.0, 0.0],
                he_mesh.clone(),
                material_for_mesh(&mesh.ir, cache),
            );
            obj.tag = format!("元件:{}", comp.ir.id);
            objects.push(obj);
        }
        scene.component_defs.insert(
            comp.ir.id.clone(),
            crate::scene::ComponentDef {
                id: comp.ir.id.clone(),
                name: comp.ir.name.clone(),
                objects,
            },
        );
    }
    result.phase_timings_ms.push(("build_component_defs".into(), phase_started.elapsed().as_millis()));

    let mut instance_to_object: HashMap<String, String> = HashMap::new();
    let mut referenced_meshes: HashSet<String> = HashSet::new();

    let phase_started = Instant::now();
    for inst in cache.instances_in_order() {
        let Some(mesh) = cache.mesh(&inst.ir.mesh_id) else { continue; };
        let Some(base_mesh) = cached_he_mesh(&mut mesh_cache, &mesh.ir) else { continue; };
        let he_mesh = transformed_he_mesh(base_mesh, inst.ir.transform);
        referenced_meshes.insert(mesh.ir.id.clone());

        // 頂點已在 world space（transform 已 bake），不做 per-instance 偏移
        // 全域置中在最後統一處理（line 274+）
        let mut obj = imported_mesh_object(
            scene.next_id_pub(),
            if inst.ir.name.is_empty() {
                mesh.ir.name.clone()
            } else {
                inst.ir.name.clone()
            },
            [0.0, 0.0, 0.0],
            he_mesh,
            material_for_mesh(&mesh.ir, cache),
        );
        obj.tag = if let Some(def_id) = &inst.ir.component_def_id {
            format!("元件:{}", def_id)
        } else if inst.ir.layer.is_empty() {
            "匯入".into()
        } else {
            inst.ir.layer.clone()
        };
        obj.component_def_id = inst.ir.component_def_id.clone();

        // Debug: log material assignment
        if result.meshes < 5 {
            eprintln!("[MAT-DEBUG] obj={} mesh={} mat_id={:?} → material={:?} color={:?}",
                obj.id, mesh.ir.id, mesh.ir.material_id, obj.material, obj.material.color());
        }

        scene.objects.insert(obj.id.clone(), obj.clone());
        result.object_debug.insert(obj.id.clone(), imported_object_debug(&mesh.ir));
        result.ids.push(obj.id.clone());
        result.meshes += 1;
        instance_to_object.insert(inst.ir.id.clone(), obj.id.clone());
    }
    result.phase_timings_ms.push(("build_instances".into(), phase_started.elapsed().as_millis()));

    let phase_started = Instant::now();
    for group in cache.groups_in_order() {
        let children: Vec<String> = group
            .ir
            .children
            .iter()
            .filter_map(|child| instance_to_object.get(child).cloned())
            .collect();
        for child_id in &children {
            if let Some(obj) = scene.objects.get_mut(child_id) {
                obj.parent_id = Some(group.ir.id.clone());
            }
        }

        scene.groups.insert(
            group.ir.id.clone(),
            crate::scene::GroupDef {
                id: group.ir.id.clone(),
                name: group.ir.name.clone(),
                children,
                parent_id: group.ir.parent_id.clone(),
                position: [0.0; 3],
                rotation_y: 0.0,
            },
        );
    }
    result.phase_timings_ms.push(("build_groups".into(), phase_started.elapsed().as_millis()));

    let phase_started = Instant::now();
    for mesh in cache.meshes_in_order() {
        if referenced_meshes.contains(&mesh.ir.id) {
            continue;
        }
        let Some(he_mesh) = cached_he_mesh(&mut mesh_cache, &mesh.ir) else { continue; };
        let obj = imported_mesh_object(
            scene.next_id_pub(),
            mesh.ir.name.clone(),
            [0.0, 0.0, 0.0],
            he_mesh.clone(),
            material_for_mesh(&mesh.ir, cache),
        );
        scene.objects.insert(obj.id.clone(), obj.clone());
        result.object_debug.insert(obj.id.clone(), imported_object_debug(&mesh.ir));
        result.ids.push(obj.id.clone());
        result.meshes += 1;
    }
    result.phase_timings_ms.push(("build_standalone_meshes".into(), phase_started.elapsed().as_millis()));

    // ── Center imported objects at origin ──
    let phase_started = Instant::now();
    if !result.ids.is_empty() {
        let mut global_min = [f32::MAX; 3];
        let mut global_max = [f32::MIN; 3];
        let mut has_verts = false;

        for obj_id in &result.ids {
            if let Some(obj) = scene.objects.get(obj_id) {
                if let crate::scene::Shape::Mesh(ref he) = obj.shape {
                    for v in he.vertices.values() {
                        // Vertices are baked in world space; add obj.position to get rendered pos
                        let wx = v.pos[0] + obj.position[0];
                        let wy = v.pos[1] + obj.position[1];
                        let wz = v.pos[2] + obj.position[2];
                        if wx < global_min[0] { global_min[0] = wx; }
                        if wy < global_min[1] { global_min[1] = wy; }
                        if wz < global_min[2] { global_min[2] = wz; }
                        if wx > global_max[0] { global_max[0] = wx; }
                        if wy > global_max[1] { global_max[1] = wy; }
                        if wz > global_max[2] { global_max[2] = wz; }
                        has_verts = true;
                    }
                }
            }
        }

        if has_verts && global_min[0].is_finite() && global_max[0].is_finite() {
            let center_x = (global_min[0] + global_max[0]) / 2.0;
            let bottom_y = global_min[1];
            let center_z = (global_min[2] + global_max[2]) / 2.0;

            // 直接偏移頂點座標（不是 obj.position），讓物件幾何在原點附近
            for obj_id in &result.ids {
                if let Some(obj) = scene.objects.get_mut(obj_id) {
                    if let crate::scene::Shape::Mesh(ref mut he) = obj.shape {
                        for v in he.vertices.values_mut() {
                            v.pos[0] -= center_x;
                            v.pos[1] -= bottom_y;
                            v.pos[2] -= center_z;
                        }
                        // 偏移 SDK edge segments
                        for seg in &mut he.sdk_edge_segments {
                            seg.0[0] -= center_x; seg.0[1] -= bottom_y; seg.0[2] -= center_z;
                            seg.1[0] -= center_x; seg.1[1] -= bottom_y; seg.1[2] -= center_z;
                        }
                        he.invalidate_cache();
                    }
                    // position 保持 [0,0,0]
                }
            }

            // 記錄場景範圍（用於相機自動 zoom）
            let scene_extent = [
                global_max[0] - global_min[0],
                global_max[1] - global_min[1],
                global_max[2] - global_min[2],
            ];
            result.scene_extent = Some(scene_extent);

            tracing::info!(
                "[Import] Centered {} objects: offset=({:.1}, {:.1}, {:.1}), extent=({:.0}x{:.0}x{:.0})",
                result.ids.len(), -center_x, -bottom_y, -center_z,
                scene_extent[0], scene_extent[1], scene_extent[2],
            );
        }
    }
    result.phase_timings_ms.push(("center_at_origin".into(), phase_started.elapsed().as_millis()));

    let phase_started = Instant::now();
    if result.meshes > 0 {
        scene.version += 1;
    }
    result.phase_timings_ms.push(("scene_version_update".into(), phase_started.elapsed().as_millis()));
    result.phase_timings_ms.push(("build_scene_total".into(), build_started.elapsed().as_millis()));

    result
}

fn imported_mesh_object(
    id: String,
    name: String,
    position: [f32; 3],
    mesh: crate::halfedge::HeMesh,
    material: crate::scene::MaterialKind,
) -> crate::scene::SceneObject {
    crate::scene::SceneObject {
        id,
        name,
        shape: crate::scene::Shape::Mesh(mesh),
        position,
        material,
        rotation_y: 0.0, rotation_xyz: [0.0; 3],
        tag: "匯入".into(),
        visible: true,
        roughness: 0.5,
        metallic: 0.0,
        texture_path: None,
        component_kind: Default::default(),
        parent_id: None,
        component_def_id: None,
        locked: false,
        obj_version: 0,
        base_level_idx: None,
        top_level_idx: None,
    }
}

fn imported_object_debug(mesh: &super::unified_ir::IrMesh) -> ImportedObjectDebug {
    ImportedObjectDebug {
        mesh_id: mesh.id.clone(),
        mesh_name: mesh.name.clone(),
        vertex_labels: mesh.source_vertex_labels.clone(),
        triangle_debug: mesh.source_triangle_debug.clone(),
    }
}

fn ir_mesh_to_he_mesh(
    mesh: &super::unified_ir::IrMesh,
) -> Option<crate::halfedge::HeMesh> {
    if mesh.vertices.len() < 3 {
        return None;
    }

    let mut he = crate::halfedge::HeMesh::new();
    let mut vertex_ids = Vec::with_capacity(mesh.vertices.len());
    for v in &mesh.vertices {
        vertex_ids.push(he.add_vertex(*v));
    }

    if !mesh.indices.is_empty() {
        for tri in mesh.indices.chunks(3) {
            if tri.len() < 3 { continue; }
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;
            if i0 >= vertex_ids.len() || i1 >= vertex_ids.len() || i2 >= vertex_ids.len() { continue; }
            he.add_face(&[vertex_ids[i0], vertex_ids[i1], vertex_ids[i2]]);
        }
    } else {
        he.add_face(&vertex_ids);
    }

    // 填入 SDK 原始邊線（避免 fallback 到 vert_ids 路徑產生三角化對角線）
    if !mesh.edges.is_empty() {
        he.sdk_edge_segments = mesh.edges.clone();
    }

    Some(he)
}

fn cached_he_mesh<'a>(
    mesh_cache: &'a HashMap<String, crate::halfedge::HeMesh>,
    mesh: &super::unified_ir::IrMesh,
) -> Option<&'a crate::halfedge::HeMesh> {
    mesh_cache.get(&mesh.id)
}

fn transformed_he_mesh(
    source: &crate::halfedge::HeMesh,
    transform: [f32; 16],
) -> crate::halfedge::HeMesh {
    let mut mesh = source.clone();
    for vertex in mesh.vertices.values_mut() {
        vertex.pos = apply_transform(vertex.pos, transform);
    }
    for face in mesh.faces.values_mut() {
        face.normal = apply_normal_transform(face.normal, transform);
    }
    // SDK 邊線也需套用 instance transform
    for seg in &mut mesh.sdk_edge_segments {
        seg.0 = apply_transform(seg.0, transform);
        seg.1 = apply_transform(seg.1, transform);
    }
    mesh
}

/// 將 4x4 column-major transform 拆解為 position + scale
/// TODO: 讓 SceneObject 支援完整 transform 矩陣
fn apply_transform(vertex: [f32; 3], m: [f32; 16]) -> [f32; 3] {
    let x = vertex[0];
    let y = vertex[1];
    let z = vertex[2];
    [
        m[0] * x + m[4] * y + m[8] * z + m[12],
        m[1] * x + m[5] * y + m[9] * z + m[13],
        m[2] * x + m[6] * y + m[10] * z + m[14],
    ]
}

fn apply_normal_transform(normal: [f32; 3], m: [f32; 16]) -> [f32; 3] {
    let x = normal[0];
    let y = normal[1];
    let z = normal[2];
    let nx = m[0] * x + m[4] * y + m[8] * z;
    let ny = m[1] * x + m[5] * y + m[9] * z;
    let nz = m[2] * x + m[6] * y + m[10] * z;
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        [nx / len, ny / len, nz / len]
    }
}

fn material_for_mesh(
    mesh: &super::unified_ir::IrMesh,
    cache: &ImportCache,
) -> crate::scene::MaterialKind {
    mesh.material_id
        .as_ref()
        .and_then(|id| cache.material(id))
        .map(|mat| material_from_ir(&mat.ir))
        // SKP 匯入預設 Clay 灰色（SketchUp 風格）
        .unwrap_or(crate::scene::MaterialKind::Custom([0.85, 0.85, 0.83, 1.0]))
}

fn material_from_ir(mat: &super::unified_ir::IrMaterial) -> crate::scene::MaterialKind {
    let r = (mat.color[0].clamp(0.0, 1.0) * 255.0).round() as u32;
    let g = (mat.color[1].clamp(0.0, 1.0) * 255.0).round() as u32;
    let b = (mat.color[2].clamp(0.0, 1.0) * 255.0).round() as u32;
    let a = mat.opacity.clamp(0.0, 1.0);
    if a < 0.999 {
        crate::scene::MaterialKind::Custom([
            mat.color[0].clamp(0.0, 1.0),
            mat.color[1].clamp(0.0, 1.0),
            mat.color[2].clamp(0.0, 1.0),
            a,
        ])
    } else {
        crate::scene::MaterialKind::Paint((r << 16) | (g << 8) | b)
    }
}

fn convert_to_drawing_ir(ir: &UnifiedIR) -> crate::cad_import::ir::DrawingIR {
    let mut drawing = crate::cad_import::ir::DrawingIR::default();
    drawing.units = ir.units.clone();
    drawing.grids = ir.grids.clone().unwrap_or_default();
    drawing.levels = ir.levels.clone();

    for member in &ir.members {
        match member.member_type {
            super::unified_ir::MemberType::Column => {
                drawing.columns.push(crate::cad_import::ir::ColumnDef {
                    id: member.id.clone(),
                    grid_x: String::new(),
                    grid_y: String::new(),
                    position: [member.start[0], member.start[1]],
                    base_level: member.start[2],
                    top_level: member.end[2],
                    profile: member.profile.clone(),
                });
            }
            super::unified_ir::MemberType::Beam => {
                drawing.beams.push(crate::cad_import::ir::BeamDef {
                    id: member.id.clone(),
                    from_grid: String::new(),
                    to_grid: String::new(),
                    elevation: member.start[2],
                    start_pos: [member.start[0], member.start[1]],
                    end_pos: [member.end[0], member.end[1]],
                    profile: member.profile.clone(),
                });
            }
            _ => {}
        }
    }

    drawing
}

#[derive(Default)]
pub struct BuildResult {
    pub columns: usize,
    pub beams: usize,
    pub plates: usize,
    pub meshes: usize,
    pub ids: Vec<String>,
    pub phase_timings_ms: Vec<(String, u128)>,
    pub object_debug: HashMap<String, ImportedObjectDebug>,
    /// 場景範圍 [width, height, depth]（用於相機自動 zoom）
    pub scene_extent: Option<[f32; 3]>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::unified_ir;

    #[test]
    fn import_cache_tracks_ir_records_by_id() {
        let ir = unified_ir::UnifiedIR {
            source_format: "skp".into(),
            source_file: "test.skp".into(),
            units: "mm".into(),
            meshes: vec![unified_ir::IrMesh {
                id: "mesh_1".into(),
                name: "Mesh A".into(),
                vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![],
                indices: vec![0, 1, 2],
                material_id: Some("mat_1".into()),
                source_vertex_labels: vec![],
                source_triangle_debug: vec![],
                edges: vec![],
            }],
            instances: vec![unified_ir::IrInstance {
                id: "inst_1".into(),
                mesh_id: "mesh_1".into(),
                component_def_id: Some("comp_1".into()),
                transform: [
                    1.0, 0.0, 0.0, 0.0,
                    0.0, 1.0, 0.0, 0.0,
                    0.0, 0.0, 1.0, 0.0,
                    0.0, 0.0, 0.0, 1.0,
                ],
                name: "Inst A".into(),
                layer: "Layer1".into(),
            }],
            groups: vec![unified_ir::IrGroup {
                id: "grp_1".into(),
                name: "Group A".into(),
                children: vec!["inst_1".into()],
                parent_id: None,
            }],
            component_defs: vec![unified_ir::IrComponentDef {
                id: "comp_1".into(),
                name: "Comp A".into(),
                mesh_ids: vec!["mesh_1".into()],
                instance_count: 1,
            }],
            materials: vec![unified_ir::IrMaterial {
                id: "mat_1".into(),
                name: "Red".into(),
                color: [1.0, 0.0, 0.0, 1.0],
                texture_path: None,
                opacity: 1.0,
            }],
            curves: vec![],
            grids: None,
            members: vec![],
            levels: vec![],
            stats: Default::default(),
            debug_report: vec![],
        };

        let cache = ImportCache::from_ir(&ir);
        assert_eq!(cache.meshes.len(), 1);
        assert_eq!(cache.instances.len(), 1);
        assert_eq!(cache.groups.len(), 1);
        assert_eq!(cache.component_defs.len(), 1);
        assert_eq!(cache.materials.len(), 1);
        assert_eq!(cache.mesh("mesh_1").unwrap().label, "ir_mesh:mesh_1");
        assert_eq!(cache.material("mat_1").unwrap().label, "ir_material:mat_1");
    }

    #[test]
    fn builds_mesh_instances_groups_and_component_defs() {
        let mut scene = crate::scene::Scene::default();
        let ir = unified_ir::UnifiedIR {
            source_format: "skp".into(),
            source_file: "test.skp".into(),
            units: "mm".into(),
            meshes: vec![unified_ir::IrMesh {
                id: "mesh_1".into(),
                name: "Cube".into(),
                vertices: vec![
                    [0.0, 0.0, 0.0],
                    [100.0, 0.0, 0.0],
                    [0.0, 100.0, 0.0],
                ],
                normals: vec![],
                indices: vec![0, 1, 2],
                material_id: None,
                source_vertex_labels: vec![],
                source_triangle_debug: vec![],
                edges: vec![],
            }],
            instances: vec![unified_ir::IrInstance {
                id: "inst_1".into(),
                mesh_id: "mesh_1".into(),
                component_def_id: Some("comp_1".into()),
                transform: [
                    1.0, 0.0, 0.0, 0.0,
                    0.0, 1.0, 0.0, 0.0,
                    0.0, 0.0, 1.0, 0.0,
                    250.0, 0.0, 500.0, 1.0,
                ],
                name: "Instance A".into(),
                layer: "Layer1".into(),
            }],
            groups: vec![unified_ir::IrGroup {
                id: "grp_1".into(),
                name: "Main Group".into(),
                children: vec!["inst_1".into()],
                parent_id: None,
            }],
            component_defs: vec![unified_ir::IrComponentDef {
                id: "comp_1".into(),
                name: "Comp A".into(),
                mesh_ids: vec!["mesh_1".into()],
                instance_count: 1,
            }],
            materials: vec![],
            curves: vec![],
            grids: None,
            members: vec![],
            levels: vec![],
            stats: Default::default(),
            debug_report: vec![],
        };

        let result = build_scene_from_ir(&mut scene, &ir);

        assert_eq!(result.meshes, 1);
        assert_eq!(scene.objects.len(), 1);
        assert!(scene.component_defs.contains_key("comp_1"));
        assert!(scene.groups.contains_key("grp_1"));

        let obj = scene.objects.values().next().expect("expected imported object");
        // 置中改為直接偏移頂點，position 保持 [0,0,0]
        assert!((obj.position[0]).abs() < 0.01, "position X should be 0");
        assert!((obj.position[1]).abs() < 0.01, "position Y should be 0");
        assert!((obj.position[2]).abs() < 0.01, "position Z should be 0");
        assert_eq!(obj.parent_id.as_deref(), Some("grp_1"));
        assert!(matches!(obj.shape, crate::scene::Shape::Mesh(_)));
        assert_eq!(obj.tag, "元件:comp_1");
        if let crate::scene::Shape::Mesh(ref mesh) = obj.shape {
            let v = mesh.vertices.values().next().expect("mesh should have vertices");
            // 頂點經 transform + 置中偏移後，應在原點附近
            // 原始頂點 [250..350, 0..100, 500]，center=(300,0,500) → 偏移後 [-50..50, 0..100, 0]
            assert!(v.pos[0].abs() < 60.0, "Vertex X should be centered near origin, got {}", v.pos[0]);
        }
    }
}

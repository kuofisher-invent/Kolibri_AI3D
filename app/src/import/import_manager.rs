//! Import Manager ??detects file format and routes to appropriate importer

use super::import_cache::ImportCache;
use super::unified_ir::UnifiedIR;
use std::collections::{HashMap, HashSet};

pub enum ImportFormat {
    Dxf,
    Dwg,
    Skp,
    Obj,
    Stl,
    Pdf,
    Unknown,
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
        ImportFormat::Skp => super::skp_importer::import_skp(path),
        ImportFormat::Obj => super::skp_importer::import_obj_to_ir(path),
        ImportFormat::Pdf => super::pdf_parser::parse_pdf(path),
        ImportFormat::Stl => {
            Err("STL ?臬隢蝙??瑼? ???臬 ??STL 璅∪?".into())
        }
        ImportFormat::Unknown => {
            Err(format!("銝?渡?瑼??澆?: {}", path))
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

    // Build members (columns, beams) using steel builder
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

    for comp in cache.component_defs_in_order() {
        let mut objects = Vec::new();
        for mesh_id in &comp.ir.mesh_ids {
            let Some(mesh) = cache.mesh(mesh_id) else { continue; };
            let Some(he_mesh) = ir_mesh_to_he_mesh(&mesh.ir, None) else { continue; };
            let mut obj = imported_mesh_object(
                scene.next_id_pub(),
                mesh.ir.name.clone(),
                [0.0, 0.0, 0.0],
                he_mesh,
                material_for_mesh(&mesh.ir, cache),
            );
            obj.tag = format!("?辣:{}", comp.ir.id);
            objects.push(obj);
        }
        if !objects.is_empty() {
            scene.component_defs.insert(
                comp.ir.id.clone(),
                crate::scene::ComponentDef {
                    id: comp.ir.id.clone(),
                    name: comp.ir.name.clone(),
                    objects,
                },
            );
        }
    }

    let mut instance_to_object: HashMap<String, String> = HashMap::new();
    let mut referenced_meshes: HashSet<String> = HashSet::new();

    for inst in cache.instances_in_order() {
        let Some(mesh) = cache.mesh(&inst.ir.mesh_id) else { continue; };
        let Some(he_mesh) = ir_mesh_to_he_mesh(&mesh.ir, Some(inst.ir.transform)) else { continue; };
        referenced_meshes.insert(mesh.ir.id.clone());

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
            format!("?辣:{}", def_id)
        } else if inst.ir.layer.is_empty() {
            "?臬".into()
        } else {
            inst.ir.layer.clone()
        };

        scene.objects.insert(obj.id.clone(), obj.clone());
        result.ids.push(obj.id.clone());
        result.meshes += 1;
        instance_to_object.insert(inst.ir.id.clone(), obj.id.clone());
    }

    for group in cache.groups_in_order() {
        let children: Vec<String> = group
            .ir
            .children
            .iter()
            .filter_map(|child| instance_to_object.get(child).cloned())
            .collect();
        if children.is_empty() {
            continue;
        }

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

    for mesh in cache.meshes_in_order() {
        if referenced_meshes.contains(&mesh.ir.id) {
            continue;
        }
        let Some(he_mesh) = ir_mesh_to_he_mesh(&mesh.ir, None) else { continue; };
        let obj = imported_mesh_object(
            scene.next_id_pub(),
            mesh.ir.name.clone(),
            [0.0, 0.0, 0.0],
            he_mesh,
            material_for_mesh(&mesh.ir, cache),
        );
        scene.objects.insert(obj.id.clone(), obj.clone());
        result.ids.push(obj.id.clone());
        result.meshes += 1;
    }

    if result.meshes > 0 {
        scene.version += 1;
    }

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
        rotation_y: 0.0,
        tag: "?臬".into(),
        visible: true,
        roughness: 0.5,
        metallic: 0.0,
        texture_path: None,
        component_kind: Default::default(),
        parent_id: None,
        locked: false,
    }
}

fn ir_mesh_to_he_mesh(
    mesh: &super::unified_ir::IrMesh,
    transform: Option<[f32; 16]>,
) -> Option<crate::halfedge::HeMesh> {
    if mesh.vertices.len() < 3 {
        return None;
    }

    let mut he = crate::halfedge::HeMesh::new();
    let mut vertex_ids = Vec::with_capacity(mesh.vertices.len());
    for v in &mesh.vertices {
        vertex_ids.push(he.add_vertex(apply_transform(*v, transform)));
    }

    if !mesh.indices.is_empty() {
        for tri in mesh.indices.chunks(3) {
            if tri.len() < 3 {
                continue;
            }
            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;
            if i0 >= vertex_ids.len() || i1 >= vertex_ids.len() || i2 >= vertex_ids.len() {
                continue;
            }
            he.add_face(&[vertex_ids[i0], vertex_ids[i1], vertex_ids[i2]]);
        }
    } else {
        he.add_face(&vertex_ids);
    }

    Some(he)
}

/// 敺?4x4 column-major transform ?拚??撟喟宏??
/// TODO: ?芯? SceneObject ?舀摰 transform ??????頧?蝮格
fn apply_transform(vertex: [f32; 3], transform: Option<[f32; 16]>) -> [f32; 3] {
    let Some(m) = transform else { return vertex; };
    let x = vertex[0];
    let y = vertex[1];
    let z = vertex[2];
    [
        m[0] * x + m[4] * y + m[8] * z + m[12],
        m[1] * x + m[5] * y + m[9] * z + m[13],
        m[2] * x + m[6] * y + m[10] * z + m[14],
    ]
}

fn material_for_mesh(
    mesh: &super::unified_ir::IrMesh,
    cache: &ImportCache,
) -> crate::scene::MaterialKind {
    mesh.material_id
        .as_ref()
        .and_then(|id| cache.material(id))
        .map(|mat| material_from_ir(&mat.ir))
        .unwrap_or(crate::scene::MaterialKind::White)
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
        assert_eq!(obj.position, [0.0, 0.0, 0.0]);
        assert_eq!(obj.parent_id.as_deref(), Some("grp_1"));
        assert!(matches!(obj.shape, crate::scene::Shape::Mesh(_)));
        assert_eq!(obj.tag, "?辣:comp_1");
        if let crate::scene::Shape::Mesh(ref mesh) = obj.shape {
            let v = mesh.vertices.values().next().expect("mesh should have vertices");
            assert!(
                (v.pos[0] - 250.0).abs() < 0.01 || (v.pos[2] - 500.0).abs() < 0.01,
                "Transform should be baked into vertices"
            );
        }
    }
}

//! Import Manager — detects file format and routes to appropriate importer

use super::unified_ir::UnifiedIR;

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
            Err("STL 匯入請使用 檔案 → 匯入 → STL 模型".into())
        }
        ImportFormat::Unknown => {
            Err(format!("不支援的檔案格式: {}", path))
        }
    }
}

/// Build scene from unified IR
pub fn build_scene_from_ir(scene: &mut kolibri_core::scene::Scene, ir: &UnifiedIR) -> BuildResult {
    let mut result = BuildResult::default();

    // Build members (columns, beams) using steel builder
    if !ir.members.is_empty() {
        // Convert back to DrawingIR for steel_builder compatibility
        let drawing_ir = convert_to_drawing_ir(ir);
        // TODO: steel_builder 整合在 app 層呼叫，io crate 不直接依賴 builders
        // build_from_ir 的結果由 app 層在 import 完成後處理
        let _ = &drawing_ir;
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
                // Add each segment as a line
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

            // TODO: steel_builder 整合在 app 層呼叫
            let _ = &drawing_ir;
        }
    }

    // Build meshes as Box approximations
    for mesh in &ir.meshes {
        if mesh.vertices.len() < 3 { continue; }

        // Compute bounding box
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for v in &mesh.vertices {
            for i in 0..3 {
                min[i] = min[i].min(v[i]);
                max[i] = max[i].max(v[i]);
            }
        }
        let w = (max[0] - min[0]).max(1.0);
        let h = (max[1] - min[1]).max(1.0);
        let d = (max[2] - min[2]).max(1.0);

        // Create as box (simplified)
        let id = scene.add_box(
            mesh.name.clone(),
            min,
            w, h, d,
            kolibri_core::scene::MaterialKind::White,
        );
        result.ids.push(id);
        result.meshes += 1;
    }

    result
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

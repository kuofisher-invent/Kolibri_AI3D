pub mod ir;
pub mod geometry_parser;
pub mod drawing_classifier;
pub mod grid_parser;
pub mod steel_parser;
pub mod elevation_parser;
#[allow(dead_code)]
pub mod preprocessor;
pub mod semantic_detector;
#[allow(dead_code)]
pub mod dxf_importer;
#[allow(dead_code)]
pub mod import_validator;

use ir::DrawingIR;

/// Full pipeline: DXF path -> DrawingIR
/// Uses the improved DXF importer with validation, falling back to the basic parser on error.
pub fn import_dxf_to_ir(path: &str) -> Result<DrawingIR, String> {
    // Try the improved importer first
    let config = dxf_importer::DxfImportConfig::default();
    match dxf_importer::import_dxf(path, &config) {
        Ok(geom_ir) => {
            // Validate the import
            let snapshot = create_validation_snapshot(&geom_ir);
            let validation = import_validator::validate_import(
                &snapshot,
                &import_validator::ImportValidationConfig::default(),
            );

            if validation.health == import_validator::ImportHealth::Broken {
                let issues: Vec<String> = validation.issues.iter().map(|i| i.message.clone()).collect();
                return Err(format!("Import validation failed:\n{}", issues.join("\n")));
            }

            // Convert GeometryIr to our DrawingIR
            convert_geometry_ir_to_drawing_ir(geom_ir, validation, path)
        }
        Err(e) => {
            // Fallback to old parser
            eprintln!("[DXF] New importer failed ({:?}), using fallback parser", e);
            fallback_import(path)
        }
    }
}

/// Fallback: use the original basic geometry parser pipeline
fn fallback_import(path: &str) -> Result<DrawingIR, String> {
    let geom = geometry_parser::parse_dxf(path)?;

    let drawing_type = drawing_classifier::classify_drawing(&geom);
    let grids = grid_parser::parse_grids(&geom);
    let levels = elevation_parser::parse_elevations(&geom);
    let (columns, beams, base_plates) = steel_parser::parse_steel_elements(&geom, &grids, &levels);

    let page = ir::PageInfo {
        page_number: 1,
        drawing_type: drawing_type.clone(),
        title: std::path::Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
    };

    let mut drawing = DrawingIR {
        units: "mm".into(),
        drawing_type,
        pages: vec![page],
        grids,
        columns,
        beams,
        levels,
        base_plates,
        debug_report: Vec::new(),
    };

    // Fallback: if existing parsers found nothing, try geometry-based semantic detection v2
    if drawing.columns.is_empty() && drawing.beams.is_empty() {
        let prep = preprocessor::preprocess(&geom);
        let semantic = semantic_detector::detect_v2(&prep, &geom.texts);
        for line in &semantic.debug_lines {
            eprintln!("[SemanticDetector] {}", line);
        }
        if !semantic.columns.is_empty() || !semantic.beams.is_empty() {
            drawing.grids = semantic.grids;
            drawing.columns = semantic.columns;
            drawing.beams = semantic.beams;
            drawing.base_plates = semantic.plates;
            drawing.levels = semantic.levels;
        }
    }

    // Normalize coordinates to origin
    normalize_coordinates(&mut drawing);

    Ok(drawing)
}

/// Convert GeometryIr from the new importer into our DrawingIR
fn convert_geometry_ir_to_drawing_ir(
    geom: dxf_importer::GeometryIr,
    validation: import_validator::ImportValidationReport,
    path: &str,
) -> Result<DrawingIR, String> {
    let mut drawing = DrawingIR::default();

    // Units
    drawing.units = match geom.units {
        dxf_importer::Unit::Millimeter => "mm".into(),
        dxf_importer::Unit::Centimeter => "cm".into(),
        dxf_importer::Unit::Meter => "m".into(),
        dxf_importer::Unit::Inch => "inch".into(),
        dxf_importer::Unit::Foot => "foot".into(),
        _ => "mm".into(),
    };

    // Convert curves to RawGeometry for grid/steel parsers
    let mut raw_geom = geometry_parser::RawGeometry {
        lines: Vec::new(),
        polylines: Vec::new(),
        texts: Vec::new(),
        dimensions: Vec::new(),
        blocks: Vec::new(),
        circles: Vec::new(),
    };

    for curve in &geom.curves {
        match curve {
            dxf_importer::CurveIr::Line(l) => {
                raw_geom.lines.push(geometry_parser::RawLine {
                    start: [l.start[0] as f64, l.start[1] as f64],
                    end: [l.end[0] as f64, l.end[1] as f64],
                    layer: l.layer.clone(),
                    linetype: "CONTINUOUS".into(),
                });
            }
            dxf_importer::CurveIr::Polyline(p) => {
                raw_geom.polylines.push(geometry_parser::RawPolyline {
                    points: p.points.iter().map(|pt| [pt[0] as f64, pt[1] as f64]).collect(),
                    closed: p.is_closed,
                    layer: p.layer.clone(),
                });
            }
            dxf_importer::CurveIr::Circle(c) => {
                raw_geom.circles.push(geometry_parser::RawCircle {
                    center: [c.center[0] as f64, c.center[1] as f64],
                    radius: c.radius as f64,
                    layer: c.layer.clone(),
                });
            }
            dxf_importer::CurveIr::Arc(_) => {
                // Arcs are not represented in old RawGeometry; skip for now
            }
        }
    }

    for text in &geom.texts {
        raw_geom.texts.push(geometry_parser::RawText {
            content: text.value.clone(),
            position: [text.position[0] as f64, text.position[1] as f64],
            height: text.height as f64,
            layer: text.layer.clone(),
        });
    }

    for dim in &geom.dimensions {
        if dim.definition_points.len() >= 2 {
            raw_geom.dimensions.push(geometry_parser::RawDimension {
                start: [dim.definition_points[0][0] as f64, dim.definition_points[0][1] as f64],
                end: [dim.definition_points[1][0] as f64, dim.definition_points[1][1] as f64],
                value: 0.0,
                text: dim.value_text.clone().unwrap_or_default(),
                layer: dim.layer.clone(),
            });
        }
    }

    for ins in &geom.inserts {
        raw_geom.blocks.push(geometry_parser::RawBlock {
            name: ins.block_name.clone(),
            insert_point: [ins.position[0] as f64, ins.position[1] as f64],
            layer: ins.layer.clone(),
        });
    }

    // Now run our existing semantic parsers on the converted geometry
    let drawing_type = drawing_classifier::classify_drawing(&raw_geom);
    let grids = grid_parser::parse_grids(&raw_geom);
    let levels = elevation_parser::parse_elevations(&raw_geom);
    let (columns, beams, base_plates) = steel_parser::parse_steel_elements(&raw_geom, &grids, &levels);

    drawing.drawing_type = drawing_type;
    drawing.grids = grids;
    drawing.levels = levels;
    drawing.columns = columns;
    drawing.beams = beams;
    drawing.base_plates = base_plates;

    // Fallback: if existing parsers found nothing, try geometry-based semantic detection v2
    if drawing.columns.is_empty() && drawing.beams.is_empty() {
        let prep = preprocessor::preprocess(&raw_geom);
        let semantic = semantic_detector::detect_v2(&prep, &raw_geom.texts);
        for line in &semantic.debug_lines {
            eprintln!("[SemanticDetector] {}", line);
        }
        if !semantic.columns.is_empty() || !semantic.beams.is_empty() {
            drawing.grids = semantic.grids;
            drawing.columns = semantic.columns;
            drawing.beams = semantic.beams;
            drawing.base_plates = semantic.plates;
            drawing.levels = semantic.levels;
        }
    }

    // Normalize coordinates to origin
    normalize_coordinates(&mut drawing);

    // Build comprehensive debug report
    build_debug_report(&mut drawing, &geom, &validation, path);

    // Add validation info as a page
    if let Some(bbox) = validation.bbox {
        let report = format!(
            "Import: {:?} | Bounds: {:.0}x{:.0}x{:.0} | Layers: {} | Curves: {} | Texts: {} | Dims: {} | Blocks: {}",
            validation.health,
            bbox.size()[0],
            bbox.size()[1],
            bbox.size()[2],
            geom.layers.len(),
            geom.curves.len(),
            geom.texts.len(),
            geom.dimensions.len(),
            geom.blocks.len(),
        );
        drawing.pages.push(ir::PageInfo {
            page_number: 1,
            drawing_type: drawing.drawing_type.clone(),
            title: report,
        });
    }

    Ok(drawing)
}

/// Normalize all coordinates so the model center is near world origin (0,0,0)
fn normalize_coordinates(drawing: &mut DrawingIR) {
    // Compute offsets from grid positions (grids are already internally normalized by grid_parser)
    // But columns/beams may still use absolute DXF coordinates if they came from grid positions
    // that were normalized. The grid_parser already normalizes to origin, so columns/beams
    // built from normalized grids should be fine.
    //
    // However, if grids were NOT found and semantic detector was used, or if the grid_parser
    // normalization didn't catch all elements, we normalize here as a safety net.

    // Find the bounding box of all positioned elements
    let mut all_x: Vec<f64> = Vec::new();
    let mut all_y: Vec<f64> = Vec::new();

    for c in &drawing.columns {
        all_x.push(c.position[0]);
        all_y.push(c.position[1]);
    }
    for b in &drawing.beams {
        all_x.push(b.start_pos[0]);
        all_x.push(b.end_pos[0]);
        all_y.push(b.start_pos[1]);
        all_y.push(b.end_pos[1]);
    }
    for bp in &drawing.base_plates {
        all_x.push(bp.position[0]);
        all_y.push(bp.position[1]);
    }

    if all_x.is_empty() { return; }

    let min_x = all_x.iter().cloned().fold(f64::MAX, f64::min);
    let max_x = all_x.iter().cloned().fold(f64::MIN, f64::max);
    let min_y = all_y.iter().cloned().fold(f64::MAX, f64::min);
    let max_y = all_y.iter().cloned().fold(f64::MIN, f64::max);

    // If coordinates are already near origin (within 50m), skip
    let center_x = (min_x + max_x) / 2.0;
    let center_y = (min_y + max_y) / 2.0;
    if center_x.abs() < 50000.0 && center_y.abs() < 50000.0 {
        return;
    }

    // Offset to bring min corner to origin
    let x_offset = min_x;
    let y_offset = min_y;

    eprintln!("[DXF] Normalizing origin: offset=({:.0}, {:.0})", x_offset, y_offset);

    for g in &mut drawing.grids.x_grids { g.position -= x_offset; }
    for g in &mut drawing.grids.y_grids { g.position -= y_offset; }
    for c in &mut drawing.columns {
        c.position[0] -= x_offset;
        c.position[1] -= y_offset;
    }
    for b in &mut drawing.beams {
        b.start_pos[0] -= x_offset;
        b.end_pos[0] -= x_offset;
        b.start_pos[1] -= y_offset;
        b.end_pos[1] -= y_offset;
    }
    for bp in &mut drawing.base_plates {
        bp.position[0] -= x_offset;
        bp.position[1] -= y_offset;
    }
}

/// Build a comprehensive debug report for console output
fn build_debug_report(
    drawing: &mut DrawingIR,
    geom: &dxf_importer::GeometryIr,
    validation: &import_validator::ImportValidationReport,
    path: &str,
) {
    let r = &mut drawing.debug_report;

    // Get file size
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let file_size_str = if file_size > 1_000_000 {
        format!("{:.1} MB", file_size as f64 / 1_000_000.0)
    } else if file_size > 1_000 {
        format!("{:.0} KB", file_size as f64 / 1_000.0)
    } else {
        format!("{} B", file_size)
    };

    // Get DXF version from metadata
    let dxf_ver = geom.metadata.get("dxf_version").cloned().unwrap_or_default();
    let ver_name = match dxf_ver.as_str() {
        "AC1032" => "AC1032 (AutoCAD 2018)",
        "AC1027" => "AC1027 (AutoCAD 2013)",
        "AC1024" => "AC1024 (AutoCAD 2010)",
        "AC1021" => "AC1021 (AutoCAD 2007)",
        "AC1018" => "AC1018 (AutoCAD 2004)",
        "AC1015" => "AC1015 (AutoCAD 2000)",
        other if other.is_empty() => "Unknown",
        other => other,
    };

    r.push("=".repeat(50));
    r.push("  [DXF Import Report]".to_string());
    r.push(format!("  Format: {}", ver_name));
    r.push(format!("  File Size: {}", file_size_str));
    r.push("  Mode: Full Entity Parsing".to_string());
    r.push("-".repeat(50));

    // Entity counts from metadata
    r.push("  ENTITIES PARSED:".to_string());
    if let Some(counts_str) = geom.metadata.get("entity_counts") {
        for pair in counts_str.split(',') {
            let parts: Vec<&str> = pair.split(':').collect();
            if parts.len() == 2 {
                r.push(format!("    {}: {}", parts[0], parts[1]));
            }
        }
    } else {
        r.push(format!("    Curves: {}", geom.curves.len()));
        r.push(format!("    Texts: {}", geom.texts.len()));
        r.push(format!("    Dimensions: {}", geom.dimensions.len()));
        r.push(format!("    Inserts: {}", geom.inserts.len()));
    }

    r.push("-".repeat(50));
    r.push("  SEMANTIC DETECTION:".to_string());
    r.push(format!("    Drawing Type: {:?}", drawing.drawing_type));

    let x_names: Vec<&str> = drawing.grids.x_grids.iter().map(|g| g.name.as_str()).collect();
    let y_names: Vec<&str> = drawing.grids.y_grids.iter().map(|g| g.name.as_str()).collect();
    r.push(format!("    X Grids: {} ({})", x_names.len(), x_names.join(", ")));
    r.push(format!("    Y Grids: {} ({})", y_names.len(), y_names.join(", ")));

    // Spans
    if drawing.grids.x_grids.len() > 1 {
        let spans: Vec<String> = drawing.grids.x_grids.windows(2)
            .map(|w| format!("{:.0}", (w[1].position - w[0].position).abs()))
            .collect();
        r.push(format!("    Spans: {}", spans.join(", ")));
    }

    r.push(format!("    Columns: {}", drawing.columns.len()));
    r.push(format!("    Beams: {}", drawing.beams.len()));

    let level_strs: Vec<String> = drawing.levels.iter()
        .map(|l| format!("{}({:.0})", l.name, l.elevation))
        .collect();
    r.push(format!("    Levels: {}", level_strs.join(", ")));

    r.push("-".repeat(50));
    r.push("  BUILDING:".to_string());

    let top_level = drawing.levels.iter().map(|l| l.elevation).fold(0.0_f64, f64::max);
    r.push(format!("    H-Columns planned: {} (300x{:.0})", drawing.columns.len(), top_level));
    r.push(format!("    Beams planned: {}", drawing.beams.len()));
    r.push(format!("    Base plates: {}", drawing.base_plates.len()));

    // Normalization info
    if let Some(bbox) = &validation.bbox {
        r.push("  NORMALIZATION:".to_string());
        r.push(format!("    BBox center: {:.0}, {:.0}", (bbox.min[0] + bbox.max[0]) / 2.0, (bbox.min[1] + bbox.max[1]) / 2.0));
        r.push(format!("    BBox size: {:.0} x {:.0} mm", bbox.size()[0], bbox.size()[1]));
    }

    // Model extents after normalization
    if !drawing.columns.is_empty() {
        let mut mx = f64::MIN;
        let mut my = f64::MIN;
        for c in &drawing.columns {
            mx = mx.max(c.position[0]);
            my = my.max(c.position[1]);
        }
        r.push(format!("    Model size: {:.0} x {:.0} mm", mx, my));
    }

    r.push("=".repeat(50));
}

/// Build an ImportSnapshot from GeometryIr for the validator
fn create_validation_snapshot(geom: &dxf_importer::GeometryIr) -> import_validator::ImportSnapshot {
    let mut points = Vec::new();
    for curve in &geom.curves {
        match curve {
            dxf_importer::CurveIr::Line(l) => {
                points.push(l.start);
                points.push(l.end);
            }
            dxf_importer::CurveIr::Polyline(p) => {
                points.extend_from_slice(&p.points);
            }
            dxf_importer::CurveIr::Circle(c) => {
                points.push(c.center);
            }
            dxf_importer::CurveIr::Arc(a) => {
                points.push(a.center);
            }
        }
    }

    let bbox = import_validator::analyze_bbox(&points);

    import_validator::ImportSnapshot {
        source_name: geom.source_path.to_string_lossy().to_string(),
        units: format!("{:?}", geom.units),
        curve_count: geom.curves.len(),
        text_count: geom.texts.len(),
        dimension_count: geom.dimensions.len(),
        block_count: geom.blocks.len(),
        insert_count: geom.inserts.len(),
        mesh_count: 0,
        object_count: 0,
        bbox,
        points,
        metadata: std::collections::HashMap::new(),
    }
}

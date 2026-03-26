//! Native DWG Parser — reads AutoCAD DWG files without external dependencies
//!
//! Based on OpenDesign Specification (reverse-engineered DWG format docs).
//! Supports R13/R14/R2000/R2004/R2007/R2010/R2013/R2018.
//!
//! Architecture:
//!   BitReader → Section Parser → Object Map → Entity Decoder → GeometryIR

pub mod bitreader;
pub mod version;
pub mod header;
pub mod sections;
pub mod objects;
pub mod entities;
pub mod decompress;
pub mod r2018;

use crate::cad_import::dxf_importer::{GeometryIr, SourceFormat, Unit, ImportResult, ImportError, CurveIr, LineIr, TextIr};

/// Main entry: parse a DWG file and return normalized geometry IR
pub fn parse_dwg(path: &str) -> ImportResult<GeometryIr> {
    let data = std::fs::read(path).map_err(|e| ImportError::Io(e.to_string()))?;
    parse_dwg_bytes(&data, path)
}

pub fn parse_dwg_bytes(data: &[u8], source_path: &str) -> ImportResult<GeometryIr> {
    if data.len() < 6 {
        return Err(ImportError::InvalidFormat("File too small".into()));
    }

    // Step 1: Detect version
    let ver = version::detect_version(data)?;
    tracing::info!("DWG version: {:?} ({})", ver.version, ver.version_string);

    // For R2018+ (AC1032), use the specialized R2018 parser
    // because the section encryption is too complex for generic parsing
    if ver.version == version::DwgVersion::R2018 {
        return parse_r2018(data, source_path, &ver);
    }

    // Step 2: Parse file structure based on version
    let sections = sections::parse_sections(data, &ver)?;
    tracing::info!("Sections found: {}", sections.len());

    // Step 3: Parse header variables
    let header_vars = header::parse_header(&sections, &ver)?;
    tracing::info!("Header variables: {}", header_vars.len());

    // Step 4: Parse object map
    let object_map = objects::parse_object_map(&sections, &ver)?;
    tracing::info!("Object map entries: {}", object_map.len());

    // Step 5: Parse entities
    let entities = entities::parse_entities(data, &object_map, &sections, &ver)?;
    tracing::info!("Entities parsed: {}", entities.len());

    // Step 6: Convert to GeometryIR
    let mut ir = GeometryIr::new(
        std::path::PathBuf::from(source_path),
        SourceFormat::Dxf, // reuse format since IR is the same
        Unit::Millimeter,
    );

    entities::fill_geometry_ir(&entities, &mut ir);

    // Add metadata
    ir.metadata.insert("dwg_version".into(), ver.version_string.clone());
    ir.metadata.insert("parser".into(), "kolibri_native_dwg".into());

    Ok(ir)
}

/// Specialized R2018 (AC1032) parser
/// Uses enhanced coordinate scanning since full section decryption is not feasible
fn parse_r2018(data: &[u8], source_path: &str, ver: &version::DwgVersionInfo) -> ImportResult<GeometryIr> {
    tracing::info!("Using R2018 enhanced parser (AC1032 encrypted sections)");

    let result = r2018::extract_r2018_geometry(data);
    let report = r2018::generate_debug_report(data, &result);
    for line in &report {
        tracing::info!("{}", line);
    }

    let mut ir = GeometryIr::new(
        std::path::PathBuf::from(source_path),
        SourceFormat::Dxf,
        Unit::Millimeter,
    );

    // Convert extracted points to LINE entities (connect sequential nearby points)
    if result.points.len() >= 2 {
        // Cluster points and create lines between nearby sequential points
        let mut points = result.points.clone();
        points.sort_by(|a, b| a.offset.cmp(&b.offset));

        for pair in points.windows(2) {
            let p1 = &pair[0];
            let p2 = &pair[1];
            // Only connect points that were close in the file (likely same entity)
            let file_dist = (p2.offset as i64 - p1.offset as i64).abs();
            if file_dist <= 24 { // 3 consecutive f64s = 24 bytes
                let dist = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt();
                if dist > 0.1 && dist < 1e6 {
                    ir.curves.push(CurveIr::Line(LineIr {
                        layer: "DWG_SCAN".into(),
                        start: [p1.x as f32, p1.y as f32, p1.z as f32],
                        end: [p2.x as f32, p2.y as f32, p2.z as f32],
                        color: None,
                    }));
                }
            }
        }
    }

    // Convert extracted texts
    for text in &result.texts {
        ir.texts.push(TextIr {
            layer: "DWG_SCAN".into(),
            value: text.content.clone(),
            position: [0.0, 0.0, 0.0], // position unknown from binary scan
            height: 2.5,
            rotation_deg: 0.0,
        });
    }

    // Add debug report to metadata
    ir.metadata.insert("dwg_version".into(), ver.version_string.clone());
    ir.metadata.insert("parser".into(), "kolibri_r2018_enhanced".into());
    ir.metadata.insert("debug_report".into(), report.join("\n"));
    ir.metadata.insert("encryption".into(), "AC1032 section-level".into());
    ir.metadata.insert("recommendation".into(),
        "Save as DXF from ZWCAD/AutoCAD for full entity parsing".into());

    Ok(ir)
}

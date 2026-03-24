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

use crate::cad_import::dxf_importer::{GeometryIr, SourceFormat, Unit, ImportResult, ImportError};

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
    log::info!("DWG version: {:?} ({})", ver.version, ver.version_string);

    // Step 2: Parse file structure based on version
    let sections = sections::parse_sections(data, &ver)?;
    log::info!("Sections found: {}", sections.len());

    // Step 3: Parse header variables
    let header_vars = header::parse_header(&sections, &ver)?;
    log::info!("Header variables: {}", header_vars.len());

    // Step 4: Parse object map
    let object_map = objects::parse_object_map(&sections, &ver)?;
    log::info!("Object map entries: {}", object_map.len());

    // Step 5: Parse entities
    let entities = entities::parse_entities(data, &object_map, &sections, &ver)?;
    log::info!("Entities parsed: {}", entities.len());

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

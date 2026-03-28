pub mod types;
mod parser;
mod entity_parsers;
#[cfg(test)]
mod tests;

pub use types::*;

use std::fs;
use std::path::{Path, PathBuf};
use parser::*;

/// CHANGELOG: v0.1.0 - Added importer entry point by file path.
pub fn import_dxf(path: impl AsRef<Path>, config: &DxfImportConfig) -> ImportResult<GeometryIr> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)?;
    parse_text_dxf(path.to_path_buf(), &content, config)
}

/// CHANGELOG: v0.1.0 - Added importer entry point by in-memory text.
pub fn parse_text_dxf(
    source_path: PathBuf,
    content: &str,
    config: &DxfImportConfig,
) -> ImportResult<GeometryIr> {
    let unit = detect_units(content).unwrap_or(config.assume_units);
    let mut ir = GeometryIr::new(source_path, SourceFormat::Dxf, unit);

    let sections = split_sections(content);
    let tables = sections
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("TABLES"));
    let entities = sections
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("ENTITIES"));
    let blocks = sections
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("BLOCKS"));

    if let Some(tables_section) = tables {
        ir.layers = parse_layers_from_tables(tables_section)?;
    }

    if let Some(blocks_section) = blocks {
        ir.blocks = parse_block_definitions(blocks_section)?;
    }

    if let Some(entity_section) = entities {
        parse_entities(entity_section, config, &mut ir)?;
    } else {
        return Err(ImportError::InvalidFormat(
            "DXF ENTITIES section not found".to_string(),
        ));
    }

    Ok(ir)
}

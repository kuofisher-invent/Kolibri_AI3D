//! DWG Header Variable Parser
//!
//! The header section contains system variables like $INSUNITS, $EXTMIN, $EXTMAX, etc.

use std::collections::HashMap;
use super::bitreader::BitReader;
use super::sections::{DwgSection, SectionType};
use super::version::DwgVersionInfo;
use crate::cad_import::dxf_importer::ImportError;

/// Parse header variables from the header section
pub fn parse_header(
    sections: &[DwgSection],
    ver: &DwgVersionInfo,
) -> Result<HashMap<String, HeaderValue>, ImportError> {
    let header_section = sections.iter().find(|s| s.section_type == SectionType::Header);

    let mut vars = HashMap::new();

    if let Some(section) = header_section {
        let mut reader = BitReader::new(&section.data);

        // Skip sentinel (16 bytes for R2000)
        if section.data.len() > 16 {
            reader.seek(16);
        }

        // Read size of header data
        if let Ok(size) = reader.read_rl() {
            tracing::info!("Header data size: {} bytes", size);
        }

        // Parse known header variables using bit-coded reads
        // These are version-dependent; for R2000 the order is fixed

        // Try to read key variables
        if let Ok(val) = reader.read_bd() {
            vars.insert("$UNKNOWN_1".into(), HeaderValue::Double(val));
        }
        if let Ok(val) = reader.read_bd() {
            vars.insert("$UNKNOWN_2".into(), HeaderValue::Double(val));
        }

        // TODO: Full header variable parsing requires the exact sequence
        // defined in the OpenDesign Specification for each version.
        // For now, we extract what we can from the raw data.
    }

    // Also scan for recognizable patterns in raw data
    for section in sections {
        scan_header_values(&section.data, &mut vars);
    }

    Ok(vars)
}

/// Scan raw section data for recognizable header-like values
fn scan_header_values(data: &[u8], vars: &mut HashMap<String, HeaderValue>) {
    // Look for extents (bounding box) — typically two 3D points
    // These are the most useful header variables for import

    // Scan for pairs of f64 that look like coordinates
    let mut coords: Vec<[f64; 3]> = Vec::new();
    let mut i = 0;
    while i + 24 <= data.len() {
        let x = f64::from_le_bytes(data[i..i+8].try_into().unwrap_or([0; 8]));
        let y = f64::from_le_bytes(data[i+8..i+16].try_into().unwrap_or([0; 8]));
        let z = f64::from_le_bytes(data[i+16..i+24].try_into().unwrap_or([0; 8]));

        if x.is_finite() && y.is_finite() && z.is_finite()
            && x.abs() < 1e8 && y.abs() < 1e8 && z.abs() < 1e8
            && (x.abs() > 0.01 || y.abs() > 0.01)
        {
            coords.push([x, y, z]);
        }
        i += 8;
    }

    // Try to identify EXTMIN/EXTMAX from coordinate pairs
    if coords.len() >= 2 {
        // Find the most likely min/max pair
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for c in &coords {
            min_x = min_x.min(c[0]);
            min_y = min_y.min(c[1]);
            max_x = max_x.max(c[0]);
            max_y = max_y.max(c[1]);
        }

        if max_x > min_x && max_y > min_y {
            vars.insert("$EXTMIN".into(), HeaderValue::Point3D([min_x, min_y, 0.0]));
            vars.insert("$EXTMAX".into(), HeaderValue::Point3D([max_x, max_y, 0.0]));
        }
    }
}

/// Header variable value types
#[derive(Debug, Clone)]
pub enum HeaderValue {
    Double(f64),
    Int(i32),
    Short(i16),
    Text(String),
    Point2D([f64; 2]),
    Point3D([f64; 3]),
    Handle(u32),
}

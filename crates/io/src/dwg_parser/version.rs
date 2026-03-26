//! DWG Version Detection
//!
//! DWG files start with a 6-byte ASCII version string (the "magic number").
//! This module identifies the version and selects the correct parsing strategy.

use super::bitreader::DwgReadError;
use crate::cad_import::dxf_importer::ImportError;

/// DWG format version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DwgVersion {
    R13,      // AC1012
    R14,      // AC1014
    R2000,    // AC1015
    R2004,    // AC1018
    R2007,    // AC1021
    R2010,    // AC1024
    R2013,    // AC1027
    R2018,    // AC1032
    Unknown,
}

/// Parsed version info
#[derive(Debug, Clone)]
pub struct DwgVersionInfo {
    pub version: DwgVersion,
    pub version_string: String,
    pub is_r2004_plus: bool,    // uses section page compression
    pub is_r2007_plus: bool,    // uses UTF-16 strings
    pub is_r2010_plus: bool,    // uses enhanced compression
}

/// Detect DWG version from the first 6 bytes
pub fn detect_version(data: &[u8]) -> Result<DwgVersionInfo, ImportError> {
    if data.len() < 6 {
        return Err(ImportError::InvalidFormat("File too small for DWG".into()));
    }

    let magic = std::str::from_utf8(&data[0..6])
        .map_err(|_| ImportError::InvalidFormat("Invalid DWG magic bytes".into()))?;

    let version = match magic {
        "AC1012" => DwgVersion::R13,
        "AC1014" => DwgVersion::R14,
        "AC1015" => DwgVersion::R2000,
        "AC1018" => DwgVersion::R2004,
        "AC1021" => DwgVersion::R2007,
        "AC1024" => DwgVersion::R2010,
        "AC1027" => DwgVersion::R2013,
        "AC1032" => DwgVersion::R2018,
        _ => {
            // Check if it looks like DWG at all
            if magic.starts_with("AC") {
                DwgVersion::Unknown
            } else {
                return Err(ImportError::InvalidFormat(
                    format!("Not a DWG file (magic: {:?})", magic)
                ));
            }
        }
    };

    let is_r2004_plus = matches!(version,
        DwgVersion::R2004 | DwgVersion::R2007 | DwgVersion::R2010 |
        DwgVersion::R2013 | DwgVersion::R2018
    );
    let is_r2007_plus = matches!(version,
        DwgVersion::R2007 | DwgVersion::R2010 | DwgVersion::R2013 | DwgVersion::R2018
    );
    let is_r2010_plus = matches!(version,
        DwgVersion::R2010 | DwgVersion::R2013 | DwgVersion::R2018
    );

    Ok(DwgVersionInfo {
        version,
        version_string: magic.to_string(),
        is_r2004_plus,
        is_r2007_plus,
        is_r2010_plus,
    })
}

/// DWG file layout offsets (R13-R2000)
/// The file header contains pointers to each section.
#[derive(Debug, Clone)]
pub struct R2000FileHeader {
    pub image_seeker: u32,
    pub codepage: u16,
    pub section_count: u32,
    pub section_records: Vec<SectionRecord>,
}

#[derive(Debug, Clone)]
pub struct SectionRecord {
    pub record_number: u8,
    pub seeker: u32,     // byte offset in file
    pub size: u32,       // section size in bytes
}

/// Parse R13/R14/R2000 file header (starts at byte 6)
pub fn parse_r2000_file_header(data: &[u8]) -> Result<R2000FileHeader, ImportError> {
    if data.len() < 25 {
        return Err(ImportError::InvalidFormat("File too small for R2000 header".into()));
    }

    // Bytes 0-5: version string (already parsed)
    // Bytes 6-10: zeros (maintenance release, etc.)
    // Byte 13: image seeker (4 bytes, LE)
    let image_seeker = u32::from_le_bytes([data[13], data[14], data[15], data[16]]);

    // Byte 19: codepage
    let codepage = u16::from_le_bytes([data[19], data[20]]);

    // Byte 21: section count (RL)
    let section_count = u32::from_le_bytes([data[21], data[22], data[23], data[24]]) as u32;

    // Section records follow (each 9 bytes: record_number(1) + seeker(4) + size(4))
    let mut records = Vec::new();
    let mut pos = 25;
    for _ in 0..section_count.min(20) { // sanity limit
        if pos + 9 > data.len() { break; }
        records.push(SectionRecord {
            record_number: data[pos],
            seeker: u32::from_le_bytes([data[pos+1], data[pos+2], data[pos+3], data[pos+4]]),
            size: u32::from_le_bytes([data[pos+5], data[pos+6], data[pos+7], data[pos+8]]),
        });
        pos += 9;
    }

    Ok(R2000FileHeader {
        image_seeker,
        codepage,
        section_count,
        section_records: records,
    })
}

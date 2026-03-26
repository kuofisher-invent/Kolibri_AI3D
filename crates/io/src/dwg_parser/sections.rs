//! DWG Section Parser
//!
//! A DWG file is divided into sections:
//!   - Header (system variables)
//!   - Classes (custom object types)
//!   - Object Map (handle→offset mapping)
//!   - Objects/Entities (the actual geometry data)
//!   - Preview Image
//!
//! R13-R2000: sections are pointed to by the file header
//! R2004+: sections use a page-based system with compression

use super::version::{DwgVersionInfo, DwgVersion};
use super::decompress;
use crate::cad_import::dxf_importer::ImportError;

/// A parsed DWG section
#[derive(Debug, Clone)]
pub struct DwgSection {
    pub section_type: SectionType,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionType {
    Header,
    Classes,
    ObjectMap,
    Objects,
    Preview,
    Unknown(u8),
}

/// Parse all sections from the DWG file
pub fn parse_sections(data: &[u8], ver: &DwgVersionInfo) -> Result<Vec<DwgSection>, ImportError> {
    if ver.is_r2004_plus {
        parse_sections_r2004(data, ver)
    } else {
        parse_sections_r2000(data, ver)
    }
}

/// R13/R14/R2000: sections are at fixed offsets from the file header
fn parse_sections_r2000(data: &[u8], _ver: &DwgVersionInfo) -> Result<Vec<DwgSection>, ImportError> {
    let file_header = super::version::parse_r2000_file_header(data)?;
    let mut sections = Vec::new();

    for record in &file_header.section_records {
        let start = record.seeker as usize;
        let size = record.size as usize;

        if start + size > data.len() {
            tracing::warn!("Section {} at offset {} overflows file (size {})", record.record_number, start, size);
            continue;
        }

        let section_data = data[start..start + size].to_vec();
        let section_type = match record.record_number {
            0 => SectionType::Header,
            1 => SectionType::Classes,
            2 => SectionType::ObjectMap,
            _ => SectionType::Unknown(record.record_number),
        };

        sections.push(DwgSection {
            section_type,
            data: section_data,
        });
    }

    Ok(sections)
}

/// R2004+: page-based section system with compression
fn parse_sections_r2004(data: &[u8], _ver: &DwgVersionInfo) -> Result<Vec<DwgSection>, ImportError> {
    // R2004+ has a complex section page system:
    // 1. Encrypted file header (0x100 bytes at offset 0x80)
    // 2. Section page map (at offset stored in file header)
    // 3. Each section page is optionally compressed

    let mut sections = Vec::new();

    // For R2004+, try to find sections by scanning for known patterns
    // This is a simplified approach — full implementation needs the encrypted header

    // Read the section map pointer from the encrypted header area
    if data.len() < 0x100 {
        return Err(ImportError::InvalidFormat("File too small for R2004 header".into()));
    }

    // The encrypted header starts at offset 0x80
    // We need to decrypt it with a simple XOR pattern
    let header_start = 0x80;
    if header_start + 0x6C > data.len() {
        return Err(ImportError::InvalidFormat("R2004 header area truncated".into()));
    }

    // Decrypt the header (XOR with magic seed derived from version)
    let encrypted = &data[header_start..header_start + 0x6C];
    let decrypted = decrypt_r2004_header(encrypted);

    // Parse decrypted header fields
    // Offset 0x00 in decrypted: file header size
    // Offset 0x04: unknown
    // Offset 0x0C: section page map offset
    // Offset 0x14: section page map size
    // Offset 0x2C: section page data offset

    if decrypted.len() >= 0x30 {
        let _page_map_offset = u64::from_le_bytes(
            decrypted[0x0C..0x14].try_into().unwrap_or([0; 8])
        ) as usize;
        let _page_map_size = u32::from_le_bytes(
            decrypted[0x14..0x18].try_into().unwrap_or([0; 4])
        ) as usize;

        tracing::info!("R2004 section page map at offset {:#X}, size {}", _page_map_offset, _page_map_size);

        // For now, create a single "objects" section from the entire data body
        // A full implementation would parse the page map and decompress each page
        if data.len() > 0x100 {
            sections.push(DwgSection {
                section_type: SectionType::Objects,
                data: data[0x100..].to_vec(),
            });
        }
    }

    // If we couldn't parse the section map, fall back to whole-file scanning
    if sections.is_empty() {
        sections.push(DwgSection {
            section_type: SectionType::Objects,
            data: data.to_vec(),
        });
    }

    Ok(sections)
}

/// Decrypt R2004+ file header (simple XOR cipher)
fn decrypt_r2004_header(encrypted: &[u8]) -> Vec<u8> {
    let mut decrypted = encrypted.to_vec();
    let mut seed: u32 = 0x4164536B; // "AdSk" in ASCII, the Autodesk magic seed

    for byte in &mut decrypted {
        let mask = (seed & 0xFF) as u8;
        *byte ^= mask;
        seed = seed.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
    }

    decrypted
}

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
    let mut sections = Vec::new();

    if data.len() < 0x100 {
        return Err(ImportError::InvalidFormat("File too small for R2004 header".into()));
    }

    let header_start = 0x80;
    if header_start + 0x6C > data.len() {
        return Err(ImportError::InvalidFormat("R2004 header area truncated".into()));
    }

    let encrypted = &data[header_start..header_start + 0x6C];
    let decrypted = decrypt_r2004_header(encrypted);

    if decrypted.len() >= 0x30 {
        let page_map_offset = u64::from_le_bytes(
            decrypted[0x0C..0x14].try_into().unwrap_or([0; 8])
        ) as usize;
        let _page_map_size = u32::from_le_bytes(
            decrypted[0x14..0x18].try_into().unwrap_or([0; 4])
        ) as usize;

        tracing::info!("R2004: page_map offset={:#X}", page_map_offset);

        // 掃描 18CF 壓縮頁面標記
        let mut all_page_data = Vec::new();
        let mut scan_pos = 0x100;
        while scan_pos + 32 < data.len() {
            if data[scan_pos] == 0x18 && data[scan_pos + 1] == 0xCF {
                let decomp_size = u32::from_le_bytes(
                    data[scan_pos + 4..scan_pos + 8].try_into().unwrap_or([0; 4])
                ) as usize;
                let comp_size = u32::from_le_bytes(
                    data[scan_pos + 8..scan_pos + 12].try_into().unwrap_or([0; 4])
                ) as usize;
                if comp_size > 0 && comp_size < 1_000_000 && decomp_size > 0 && decomp_size < 10_000_000 {
                    let page_total = 32 + comp_size;
                    if scan_pos + page_total <= data.len() {
                        match decompress::decompress_r2004(&data[scan_pos + 32..scan_pos + 32 + comp_size], decomp_size) {
                            Ok(decompressed) => { all_page_data.extend_from_slice(&decompressed); }
                            Err(_) => { all_page_data.extend_from_slice(&data[scan_pos..scan_pos + page_total]); }
                        }
                        scan_pos += page_total;
                        continue;
                    }
                }
            }
            scan_pos += 1;
        }

        if !all_page_data.is_empty() {
            sections.push(DwgSection {
                section_type: SectionType::Objects,
                data: all_page_data,
            });
        }
    }

    // 備援：用整個檔案體做 scan
    if sections.is_empty() {
        if data.len() > 0x100 {
            sections.push(DwgSection {
                section_type: SectionType::Objects,
                data: data[0x100..].to_vec(),
            });
        } else {
            sections.push(DwgSection {
                section_type: SectionType::Objects,
                data: data.to_vec(),
            });
        }
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

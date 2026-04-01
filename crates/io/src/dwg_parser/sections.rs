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
fn parse_sections_r2004(data: &[u8], ver: &DwgVersionInfo) -> Result<Vec<DwgSection>, ImportError> {
    let mut sections = Vec::new();

    if data.len() < 0x100 {
        return Err(ImportError::InvalidFormat("File too small for R2004 header".into()));
    }

    // ── 解密 R2004 File Header（0x80 起始，XOR 加密）──
    let header_start = 0x80;
    if header_start + 0x6C > data.len() {
        return Err(ImportError::InvalidFormat("R2004 header area truncated".into()));
    }

    let encrypted = &data[header_start..header_start + 0x6C];
    let decrypted = decrypt_r2004_header(encrypted);

    if decrypted.len() >= 0x30 {
        // Section Page Map offset & size
        let page_map_offset = u64::from_le_bytes(
            decrypted[0x0C..0x14].try_into().unwrap_or([0; 8])
        ) as usize;
        let page_map_size = u32::from_le_bytes(
            decrypted[0x14..0x18].try_into().unwrap_or([0; 4])
        ) as usize;
        // Section Info offset（section 描述表）
        let section_info_id = i32::from_le_bytes(
            decrypted[0x2C..0x30].try_into().unwrap_or([0; 4])
        );

        tracing::info!("R2004: page_map offset={:#X} size={} info_id={}",
            page_map_offset, page_map_size, section_info_id);

        // ── 解析 Section Page Map ──
        // Page map 告訴我們每一頁的 offset + size
        let pages = parse_r2004_page_map(data, page_map_offset, page_map_size);
        tracing::info!("R2004: parsed {} section pages", pages.len());

        // ── 收集所有頁面的原始資料 ──
        let mut all_page_data = Vec::new();
        for page in &pages {
            if page.offset + page.size <= data.len() {
                let page_data = &data[page.offset..page.offset + page.size];
                // 嘗試解壓（檢查 18CF magic）
                if page_data.len() >= 4 && page_data[0] == 0x18 && page_data[1] == 0xCF {
                    // 壓縮頁面：header(32 bytes) + compressed data
                    if page_data.len() >= 32 {
                        let decomp_size = u32::from_le_bytes(
                            page_data[4..8].try_into().unwrap_or([0; 4])
                        ) as usize;
                        let comp_size = u32::from_le_bytes(
                            page_data[8..12].try_into().unwrap_or([0; 4])
                        ) as usize;
                        if comp_size > 0 && decomp_size > 0 && 32 + comp_size <= page_data.len() {
                            match decompress::decompress_r2004(&page_data[32..32 + comp_size], decomp_size) {
                                Ok(decompressed) => {
                                    all_page_data.extend_from_slice(&decompressed);
                                    continue;
                                }
                                Err(e) => {
                                    tracing::warn!("R2004 解壓失敗 offset={:#X}: {}", page.offset, e);
                                }
                            }
                        }
                    }
                }
                // 非壓縮（或解壓失敗）：直接使用
                all_page_data.extend_from_slice(page_data);
            }
        }

        if !all_page_data.is_empty() {
            // 嘗試在合併資料中找到 Header / Classes / ObjectMap 區段
            // 先整體作為 Objects section（entity scan 會處理）
            sections.push(DwgSection {
                section_type: SectionType::Objects,
                data: all_page_data,
            });
        }
    }

    // ── 備援：如果 page map 解析失敗，用整個檔案體做 scan ──
    if sections.is_empty() {
        tracing::warn!("R2004: page map 解析失敗，使用全檔掃描");
        // 跳過 header 區域（0x100），其餘都當資料區
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

/// R2004 Section Page 描述
#[derive(Debug, Clone)]
struct R2004Page {
    offset: usize,
    size: usize,
    _page_id: i32,
}

/// 解析 R2004 Section Page Map
fn parse_r2004_page_map(data: &[u8], map_offset: usize, map_size: usize) -> Vec<R2004Page> {
    let mut pages = Vec::new();

    if map_offset >= data.len() || map_offset + map_size > data.len() {
        return pages;
    }

    // Page Map 格式：每個 entry = page_number(4) + size(4) + offset(8) 或類似
    // 實際上 Page Map 本身也可能是壓縮的，先嘗試直接解析
    let map_data = &data[map_offset..map_offset + map_size.min(data.len() - map_offset)];

    // 每個 page map entry 約 16 bytes: page_id(4) + size(4) + parent(4) + unused(4)
    // 但 offset 通常是隱含的（連續排列從 0x100 開始）
    let mut pos = 0;
    let page_start_offset = 0x100_usize; // R2004 資料頁從 0x100 開始

    // 簡化做法：掃描已知結構標記
    // Section Page Map 的每個 section 以 2-byte size 開頭
    while pos + 8 <= map_data.len() {
        let section_size = u16::from_le_bytes(
            [map_data[pos], map_data[pos + 1]]
        ) as usize;

        if section_size <= 2 || section_size > 0x10000 { break; }

        let section_end = (pos + section_size).min(map_data.len());
        let mut entry_pos = pos + 2;

        while entry_pos + 8 <= section_end {
            // 每個 entry: page_number(MC) + size(MC)
            // 簡化：讀取兩個 i32 作為 page_id 和 page_size
            let page_id = i32::from_le_bytes(
                map_data[entry_pos..entry_pos+4].try_into().unwrap_or([0; 4])
            );
            let page_size = i32::from_le_bytes(
                map_data[entry_pos+4..entry_pos+8].try_into().unwrap_or([0; 4])
            ) as usize;

            if page_id > 0 && page_size > 0 && page_size < data.len() {
                pages.push(R2004Page {
                    offset: page_start_offset + pages.len() * 0x1000, // 估算偏移
                    size: page_size.min(data.len()),
                    _page_id: page_id,
                });
            }
            entry_pos += 8;
        }

        pos = section_end + 2; // skip CRC
    }

    // 如果 page map 解析結果太少，用另一種啟發式方法
    if pages.is_empty() && data.len() > 0x100 {
        // 掃描整個檔案找 18CF 壓縮頁面標記
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
                        pages.push(R2004Page {
                            offset: scan_pos,
                            size: page_total,
                            _page_id: pages.len() as i32 + 1,
                        });
                        scan_pos += page_total;
                        continue;
                    }
                }
            }
            scan_pos += 1;
        }
    }

    pages
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

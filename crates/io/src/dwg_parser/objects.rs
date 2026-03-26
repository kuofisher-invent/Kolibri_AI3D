//! DWG Object Map Parser
//!
//! The object map is a table mapping handle values to byte offsets in the file.
//! This allows random access to any object by its handle.

use std::collections::HashMap;
use super::bitreader::{BitReader, DwgHandle};
use super::sections::{DwgSection, SectionType};
use super::version::DwgVersionInfo;
use crate::cad_import::dxf_importer::ImportError;

/// Object map: handle → file offset
pub type ObjectMap = HashMap<u32, u32>;

/// Parse the object map section
pub fn parse_object_map(
    sections: &[DwgSection],
    ver: &DwgVersionInfo,
) -> Result<ObjectMap, ImportError> {
    let map_section = sections.iter().find(|s| s.section_type == SectionType::ObjectMap);
    let mut map = ObjectMap::new();

    if let Some(section) = map_section {
        parse_object_map_data(&section.data, &mut map)?;
    }

    // If no explicit object map, scan the objects section for entity markers
    if map.is_empty() {
        for section in sections {
            if section.section_type == SectionType::Objects {
                scan_for_objects(&section.data, &mut map);
            }
        }
    }

    Ok(map)
}

/// Parse R2000 object map format
/// The map consists of sections, each starting with a 2-byte size.
/// Within each section: pairs of (handle_offset, location_offset) as MC values.
fn parse_object_map_data(data: &[u8], map: &mut ObjectMap) -> Result<(), ImportError> {
    let mut pos = 0usize;
    let mut last_handle = 0u32;
    let mut last_location = 0u32;

    while pos + 2 < data.len() {
        // Read section size (2 bytes LE)
        let section_size = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;

        if section_size <= 2 {
            break; // End marker or empty section
        }

        let section_end = pos + section_size - 2; // -2 for CRC at end
        if section_end > data.len() { break; }

        let mut reader = BitReader::from_offset(data, pos);

        while reader.pos_bytes() < section_end {
            // Read handle offset (MC)
            let handle_offset = match reader.read_mc() {
                Ok(v) => v,
                Err(_) => break,
            };
            // Read location offset (MC)
            let location_offset = match reader.read_mc() {
                Ok(v) => v,
                Err(_) => break,
            };

            last_handle = (last_handle as i32 + handle_offset) as u32;
            last_location = (last_location as i32 + location_offset) as u32;

            if last_location > 0 && last_handle > 0 {
                map.insert(last_handle, last_location);
            }
        }

        pos = section_end + 2; // skip CRC
    }

    Ok(())
}

/// Scan raw data for object-like patterns when no object map is available (R2004+)
fn scan_for_objects(data: &[u8], map: &mut ObjectMap) {
    // This is a heuristic scan for DWG objects in the data stream
    // Each object starts with a size (MS) followed by type (BS) and data

    let mut handle_counter = 1u32;
    let mut pos = 0usize;

    while pos + 4 < data.len() {
        // Look for patterns that could be object headers
        // Objects typically have a size in the range 10-10000 bytes
        let potential_size = u16::from_le_bytes([data[pos], data[pos + 1]]) as u32;

        if potential_size >= 10 && potential_size <= 10000 && pos + potential_size as usize <= data.len() {
            // Verify with a simple heuristic: the data after should also look like an object
            let next_pos = pos + potential_size as usize;
            if next_pos + 2 < data.len() {
                let next_size = u16::from_le_bytes([data[next_pos], data[next_pos + 1]]) as u32;
                if next_size >= 10 && next_size <= 10000 {
                    // Likely found a valid object boundary
                    map.insert(handle_counter, pos as u32);
                    handle_counter += 1;
                    pos = next_pos;
                    continue;
                }
            }
        }

        pos += 1;
    }
}

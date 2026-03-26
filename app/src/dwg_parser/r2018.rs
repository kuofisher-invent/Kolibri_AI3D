//! R2018 (AC1032) DWG Parser — enhanced approach
//!
//! AC1032 uses advanced encryption that makes direct binary parsing impractical.
//! This module implements a hybrid strategy:
//!
//! 1. Extract what we CAN read (preview, summary, AcDb metadata)
//! 2. Use the coordinate scanner as primary geometry source
//! 3. Provide clear feedback about encryption limitations
//! 4. Offer ODA/LibreDWG conversion path as alternative

use super::bitreader::BitReader;
use crate::cad_import::dxf_importer::*;

/// R2018 file structure offsets (from pre-header, unencrypted)
#[derive(Debug)]
pub struct R2018PreHeader {
    pub version: String,
    pub preview_address: u32,
    pub codepage: u16,
    pub security_flags: u32,
    pub summary_address: u32,
    pub vba_address: u32,
    pub encrypted_header_address: u32,  // always 0x80
}

/// Parse R2018 pre-header (bytes 0x00-0x80, unencrypted)
pub fn parse_pre_header(data: &[u8]) -> Option<R2018PreHeader> {
    if data.len() < 0x80 { return None; }

    Some(R2018PreHeader {
        version: String::from_utf8_lossy(&data[0..6]).to_string(),
        preview_address: u32::from_le_bytes([data[0x0D], data[0x0E], data[0x0F], data[0x10]]),
        codepage: u16::from_le_bytes([data[0x13], data[0x14]]),
        security_flags: u32::from_le_bytes([data[0x18], data[0x19], data[0x1A], data[0x1B]]),
        summary_address: u32::from_le_bytes([data[0x20], data[0x21], data[0x22], data[0x23]]),
        vba_address: u32::from_le_bytes([data[0x24], data[0x25], data[0x26], data[0x27]]),
        encrypted_header_address: u32::from_le_bytes([data[0x28], data[0x29], data[0x2A], data[0x2B]]),
    })
}

/// Enhanced coordinate extraction for R2018
/// Uses multiple strategies to find geometry in the encrypted stream
pub fn extract_r2018_geometry(data: &[u8]) -> ExtractResult {
    let mut result = ExtractResult::default();

    let pre = parse_pre_header(data);
    if let Some(ref ph) = pre {
        result.debug_lines.push(format!("R2018 Pre-header:"));
        result.debug_lines.push(format!("  Version: {}", ph.version));
        result.debug_lines.push(format!("  Preview: 0x{:X}", ph.preview_address));
        result.debug_lines.push(format!("  Summary: 0x{:X}", ph.summary_address));
        result.debug_lines.push(format!("  Security: {}", ph.security_flags));
        result.debug_lines.push(format!("  Codepage: {}", ph.codepage));
    }

    // Strategy 1: Extract coordinates using double-precision scanner
    // Skip the large zero gap (0xA08-0x6140) and scan the data sections
    let scan_regions = find_data_regions(data);
    result.debug_lines.push(format!("Data regions: {} found", scan_regions.len()));

    for (start, end) in &scan_regions {
        scan_coordinates(&data[*start..*end], *start, &mut result);
    }

    // Strategy 2: Extract ASCII text strings
    extract_text_strings(data, &mut result);

    // Strategy 3: Try to find dimension values from numeric patterns
    extract_dimension_patterns(&result.texts, &mut result.dimensions);

    // Summary
    result.debug_lines.push(format!("Extracted: {} points, {} texts, {} dimensions",
        result.points.len(), result.texts.len(), result.dimensions.len()));

    result
}

/// Find non-zero data regions (skip large zero gaps)
fn find_data_regions(data: &[u8]) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    let mut region_start = None;
    let mut zero_count = 0;

    for i in 0..data.len() {
        if data[i] == 0 {
            zero_count += 1;
            if zero_count > 64 && region_start.is_some() {
                // End of data region
                if let Some(start) = region_start {
                    regions.push((start, i - zero_count));
                }
                region_start = None;
            }
        } else {
            if region_start.is_none() {
                region_start = Some(i);
            }
            zero_count = 0;
        }
    }

    if let Some(start) = region_start {
        regions.push((start, data.len()));
    }

    regions
}

/// Scan a data region for coordinate pairs (f64)
fn scan_coordinates(data: &[u8], base_offset: usize, result: &mut ExtractResult) {
    if data.len() < 16 { return; }

    let mut i = 0;
    while i + 16 <= data.len() {
        let x = f64::from_le_bytes(data[i..i+8].try_into().unwrap_or([0; 8]));
        let y = f64::from_le_bytes(data[i+8..i+16].try_into().unwrap_or([0; 8]));

        if x.is_finite() && y.is_finite()
            && x.abs() < 1e6 && y.abs() < 1e6
            && (x.abs() > 1.0 || y.abs() > 1.0)
        {
            // Also try to read Z if available
            let z = if i + 24 <= data.len() {
                let z = f64::from_le_bytes(data[i+16..i+24].try_into().unwrap_or([0; 8]));
                if z.is_finite() && z.abs() < 1e6 { z } else { 0.0 }
            } else {
                0.0
            };

            result.points.push(CoordPoint {
                x, y, z,
                offset: base_offset + i,
            });
            i += 16; // skip past this coordinate pair
            continue;
        }

        i += 8;
    }
}

/// Extract ASCII text strings from the entire file
fn extract_text_strings(data: &[u8], result: &mut ExtractResult) {
    let mut i = 0;
    while i < data.len() {
        let mut end = i;
        while end < data.len() && data[end] >= 0x20 && data[end] < 0x7F {
            end += 1;
        }
        let len = end - i;
        if len >= 1 && len <= 200 {
            if let Ok(s) = std::str::from_utf8(&data[i..end]) {
                let s = s.trim();
                if !s.is_empty() {
                    let is_meaningful =
                        // Grid labels
                        (s.len() <= 3 && s.chars().all(|c| c.is_ascii_uppercase()))
                        // Numbers (dimensions, elevations)
                        || s.parse::<f64>().is_ok()
                        || s.starts_with('+') || s.starts_with('-')
                        // Layer/block names
                        || s.contains("Layer") || s.contains("LAYER")
                        || s.contains("AcDb")
                        // Chinese text
                        || s.chars().any(|c| c as u32 > 0x4E00);

                    if is_meaningful {
                        result.texts.push(ExtractedText {
                            content: s.to_string(),
                            offset: i,
                        });
                    }
                }
            }
        }
        i = if end > i { end + 1 } else { i + 1 };
    }
}

/// Try to extract dimension values from text patterns
fn extract_dimension_patterns(texts: &[ExtractedText], dims: &mut Vec<f64>) {
    for text in texts {
        let s = text.content.replace("+", "").replace(",", "");
        if let Ok(v) = s.parse::<f64>() {
            if v > 10.0 && v < 100000.0 {
                if !dims.iter().any(|&d| (d - v).abs() < 0.1) {
                    dims.push(v);
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct ExtractResult {
    pub points: Vec<CoordPoint>,
    pub texts: Vec<ExtractedText>,
    pub dimensions: Vec<f64>,
    pub debug_lines: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CoordPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub offset: usize,
}

#[derive(Debug, Clone)]
pub struct ExtractedText {
    pub content: String,
    pub offset: usize,
}

/// Generate a detailed debug report for console output
pub fn generate_debug_report(data: &[u8], result: &ExtractResult) -> Vec<String> {
    let pre = parse_pre_header(data);
    let mut report = Vec::new();

    report.push("═══════════════════════════════════════".into());
    report.push("  [DWG Native Parser Report]".into());
    report.push(format!("  Format: {} (R2018/R2019/R2020)",
        pre.as_ref().map(|p| p.version.as_str()).unwrap_or("?")));
    report.push(format!("  File Size: {:.1} KB", data.len() as f64 / 1024.0));
    report.push(format!("  Parser Mode: Enhanced Coordinate Scan"));
    report.push(format!("  Encryption: AC1032 (section-level, not fully decryptable)"));
    report.push("───────────────────────────────────────".into());
    report.push(format!("  EXTRACTION RESULTS:"));
    report.push(format!("    Coordinate Points: {}", result.points.len()));
    report.push(format!("    Text Strings: {}", result.texts.len()));
    report.push(format!("    Dimension Values: {}", result.dimensions.len()));

    if !result.dimensions.is_empty() {
        let dim_str: Vec<String> = result.dimensions.iter().take(10).map(|d| format!("{:.0}", d)).collect();
        report.push(format!("    Values: {}", dim_str.join(", ")));
    }

    // Coordinate bounds
    if !result.points.is_empty() {
        let min_x = result.points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
        let max_x = result.points.iter().map(|p| p.x).fold(f64::MIN, f64::max);
        let min_y = result.points.iter().map(|p| p.y).fold(f64::MAX, f64::min);
        let max_y = result.points.iter().map(|p| p.y).fold(f64::MIN, f64::max);
        report.push(format!("    X Range: {:.0} ~ {:.0} mm", min_x, max_x));
        report.push(format!("    Y Range: {:.0} ~ {:.0} mm", min_y, max_y));
        report.push(format!("    Drawing Size: {:.0} x {:.0} mm", max_x - min_x, max_y - min_y));
    }

    report.push("───────────────────────────────────────".into());
    report.push(format!("  RECOMMENDATION:"));
    report.push(format!("    AC1032 encryption limits direct parsing."));
    report.push(format!("    For full entity data, please:"));
    report.push(format!("    1. Open in ZWCAD/AutoCAD"));
    report.push(format!("    2. Save As → DXF format"));
    report.push(format!("    3. Import the DXF in Kolibri"));
    report.push("═══════════════════════════════════════".into());

    report
}

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
/// Attempts external DWG→DXF conversion first, falls back to coordinate scanning
fn parse_r2018(data: &[u8], source_path: &str, ver: &version::DwgVersionInfo) -> ImportResult<GeometryIr> {
    tracing::info!("DWG R2018+ (AC1032): encrypted sections");

    // 嘗試外部轉換 DWG → DXF（LibreDWG / ODA）
    if let Some(dxf_path) = try_convert_dwg_to_dxf(source_path) {
        // 用 DXF parser（cad_import）讀取轉換後的檔案
        tracing::info!("R2018 DWG converted to DXF: {}", dxf_path);
        // 注意：這裡回傳的是 GeometryIr（3D），2D 路徑由上層 import_dwg_to_draft 處理
        // 清理臨時檔案由呼叫者處理
    }

    // Fallback: 座標掃描（有限精確度）
    tracing::warn!("R2018 DWG: using heuristic scan (limited accuracy). Save as DXF for best results.");

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

    // 改善的座標配對：只取 offset 恰好相鄰 16 bytes 的配對（一個 LINE = 兩組 [x,y] 各 8+8 bytes）
    if result.points.len() >= 2 {
        let mut points = result.points.clone();
        points.sort_by(|a, b| a.offset.cmp(&b.offset));

        let mut i = 0;
        while i + 1 < points.len() {
            let p1 = &points[i];
            let p2 = &points[i + 1];
            let gap = p2.offset as i64 - p1.offset as i64;
            // LINE entity: start(x,y) 緊接 end(x,y)，各佔 16 bytes（含 z）或 24 bytes
            if gap == 16 || gap == 24 || gap == 48 {
                let dist = ((p2.x - p1.x).powi(2) + (p2.y - p1.y).powi(2)).sqrt();
                // 過濾極端值：距離太短（重複點）或太長（不相關座標）
                if dist > 1.0 && dist < 50000.0 {
                    ir.curves.push(CurveIr::Line(LineIr {
                        layer: "DWG_SCAN".into(),
                        start: [p1.x as f32, p1.y as f32, p1.z as f32],
                        end: [p2.x as f32, p2.y as f32, p2.z as f32],
                        color: None,
                    }));
                }
                i += 2; // 跳過已配對的兩點
                continue;
            }
            i += 1;
        }
    }

    // Add metadata
    ir.metadata.insert("dwg_version".into(), ver.version_string.clone());
    ir.metadata.insert("parser".into(), "kolibri_r2018_heuristic".into());
    ir.metadata.insert("warning".into(),
        "R2018+ DWG uses encrypted sections. Result may be inaccurate. For best results, save as DXF from ZWCAD/AutoCAD.".into());

    Ok(ir)
}

/// 嘗試用外部工具將 DWG 轉 DXF
/// 回傳轉換後 DXF 路徑，失敗回傳 None
pub fn try_convert_dwg_to_dxf(dwg_path: &str) -> Option<String> {
    let dxf_path = format!("{}.tmp.dxf", dwg_path);

    // 策略 1: ZWCAD COM Automation（透過 PowerShell）
    if let Some(result) = try_zwcad_com(dwg_path, &dxf_path) {
        return Some(result);
    }

    // 策略 2: LibreDWG dwg2dxf
    if let Ok(output) = std::process::Command::new("dwg2dxf")
        .arg("-o").arg(&dxf_path).arg(dwg_path)
        .output()
    {
        if output.status.success() && std::path::Path::new(&dxf_path).exists() {
            tracing::info!("DWG→DXF via LibreDWG dwg2dxf");
            return Some(dxf_path);
        }
    }

    // 策略 3: ODA File Converter
    let oda_paths = [
        "C:/Program Files/ODA/ODAFileConverter.exe",
        "C:/Program Files (x86)/ODA/ODAFileConverter/ODAFileConverter.exe",
    ];
    for oda in &oda_paths {
        if std::path::Path::new(oda).exists() {
            let input_dir = std::path::Path::new(dwg_path).parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());
            let file_name = std::path::Path::new(dwg_path).file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Ok(_) = std::process::Command::new(oda)
                .arg(&input_dir).arg(&input_dir)
                .arg("ACAD2018").arg("DXF").arg("0").arg("1")
                .arg("*.dwg")
                .output()
            {
                let oda_dxf = format!("{}/{}.dxf", input_dir, file_name);
                if std::path::Path::new(&oda_dxf).exists() {
                    tracing::info!("DWG→DXF via ODA FileConverter");
                    return Some(oda_dxf);
                }
            }
        }
    }

    None
}

/// 用 ZWCAD COM Automation 做 DWG → DXF 轉換
fn try_zwcad_com(dwg_path: &str, dxf_path: &str) -> Option<String> {
    // 檢查 ZWCAD 是否安裝
    let zwcad_exists = std::path::Path::new("C:/Program Files/ZWCAD/ZWCAD.exe").exists();
    if !zwcad_exists { return None; }

    tracing::info!("Attempting ZWCAD COM conversion: {} → {}", dwg_path, dxf_path);

    // 用 PowerShell 呼叫 ZWCAD COM Automation 靜默轉換
    let ps_script = format!(
        r#"
try {{
    $zwcad = New-Object -ComObject 'ZWCAD.Application'
    $zwcad.Visible = $false
    $doc = $zwcad.Documents.Open('{}')
    $doc.SaveAs('{}', 1)
    $doc.Close($false)
    if ($zwcad.Documents.Count -eq 0) {{ $zwcad.Quit() }}
    Write-Host 'OK'
}} catch {{
    Write-Host "ERR: $_"
}}
"#,
        dwg_path.replace('\\', "\\\\").replace('\'', "\\'"),
        dxf_path.replace('\\', "\\\\").replace('\'', "\\'"),
    );

    match std::process::Command::new("powershell")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(&ps_script)
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            tracing::info!("ZWCAD COM result: {}", stdout.trim());
            if stdout.contains("OK") && std::path::Path::new(dxf_path).exists() {
                tracing::info!("DWG→DXF via ZWCAD COM: success");
                return Some(dxf_path.to_string());
            }
            tracing::warn!("ZWCAD COM conversion failed: {}", stdout.trim());
            None
        }
        Err(e) => {
            tracing::warn!("PowerShell launch failed: {}", e);
            None
        }
    }
}

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

    // ── 策略 0: 檢查同目錄是否已有同名 DXF（使用者可能已手動轉換）──
    let dxf_sibling = source_path.rsplit_once('.').map(|(base, _)| format!("{}.dxf", base));
    if let Some(ref dxf_path) = dxf_sibling {
        if std::path::Path::new(dxf_path).exists() {
            tracing::info!("R2018: 找到同名 DXF: {}，使用 DXF 解析", dxf_path);
            return parse_dxf_as_geometry_ir(dxf_path);
        }
    }
    // 也檢查 .tmp.dxf（之前轉換的快取）
    let tmp_dxf = format!("{}.tmp.dxf", source_path);
    if std::path::Path::new(&tmp_dxf).exists() {
        tracing::info!("R2018: 找到快取 DXF: {}，使用 DXF 解析", tmp_dxf);
        return parse_dxf_as_geometry_ir(&tmp_dxf);
    }

    // ── 策略 1: 嘗試外部轉換 DWG → DXF（ZWCAD COM / LibreDWG / ODA）──
    if let Some(dxf_path) = try_convert_dwg_to_dxf(source_path) {
        tracing::info!("R2018 DWG converted to DXF: {}", dxf_path);
        match parse_dxf_as_geometry_ir(&dxf_path) {
            Ok(ir) => return Ok(ir),
            Err(e) => {
                tracing::warn!("轉換後的 DXF 解析失敗: {:?}，繼續使用 heuristic scan", e);
            }
        }
    }

    // ── 策略 2: Fallback 座標掃描（有限精確度）──
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

/// 用 DXF 解析器讀取 DXF 檔案並轉為 GeometryIr
/// 用於 R2018+ DWG 外部轉換後的 DXF 回讀
fn parse_dxf_as_geometry_ir(dxf_path: &str) -> ImportResult<GeometryIr> {
    use crate::cad_import::dxf_importer::*;

    let content = std::fs::read_to_string(dxf_path)
        .map_err(|e| ImportError::Io(format!("讀取 DXF 失敗: {}", e)))?;

    let mut ir = GeometryIr::new(
        std::path::PathBuf::from(dxf_path),
        SourceFormat::Dxf,
        Unit::Millimeter,
    );

    // 簡易 DXF entity 解析（與 dxf_io 解析邏輯一致）
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut in_entities = false;

    while i + 1 < lines.len() {
        let code_str = lines[i].trim();
        let value = lines[i + 1].trim();

        if let Ok(code) = code_str.parse::<i32>() {
            if code == 2 && value == "ENTITIES" {
                in_entities = true;
            } else if code == 0 && value == "ENDSEC" {
                in_entities = false;
            }

            if in_entities && code == 0 {
                // 找到 entity 起始，收集它的所有 group codes
                let entity_type = value.to_string();
                let mut coords: std::collections::HashMap<i32, f64> = std::collections::HashMap::new();
                let mut text_val = String::new();
                let mut poly_pts: Vec<[f64; 2]> = Vec::new();
                let mut poly_closed = false;

                let mut j = i + 2;
                while j + 1 < lines.len() {
                    let gc_str = lines[j].trim();
                    let gv = lines[j + 1].trim();
                    if let Ok(gc) = gc_str.parse::<i32>() {
                        if gc == 0 { break; } // 下一個 entity
                        match gc {
                            10 | 11 | 12 | 13 | 20 | 21 | 22 | 23 | 30 | 31 | 40 | 41 | 42 | 50 | 51 => {
                                if let Ok(v) = gv.parse::<f64>() {
                                    // LWPOLYLINE 的多個 10/20 組
                                    if entity_type == "LWPOLYLINE" && gc == 10 {
                                        // 讀取對應的 20（Y 座標）
                                        let y = if j + 3 < lines.len() {
                                            let next_gc = lines[j + 2].trim().parse::<i32>().unwrap_or(-1);
                                            if next_gc == 20 {
                                                lines[j + 3].trim().parse::<f64>().unwrap_or(0.0)
                                            } else { 0.0 }
                                        } else { 0.0 };
                                        poly_pts.push([v, y]);
                                    }
                                    coords.insert(gc, v);
                                }
                            }
                            1 => { text_val = gv.to_string(); }
                            70 => {
                                if let Ok(flags) = gv.parse::<i32>() {
                                    if entity_type == "LWPOLYLINE" { poly_closed = (flags & 1) != 0; }
                                }
                            }
                            _ => {}
                        }
                    }
                    j += 2;
                }

                // 轉為 GeometryIr
                match entity_type.as_str() {
                    "LINE" => {
                        ir.curves.push(CurveIr::Line(LineIr {
                            layer: "0".into(),
                            start: [*coords.get(&10).unwrap_or(&0.0) as f32, *coords.get(&20).unwrap_or(&0.0) as f32, *coords.get(&30).unwrap_or(&0.0) as f32],
                            end: [*coords.get(&11).unwrap_or(&0.0) as f32, *coords.get(&21).unwrap_or(&0.0) as f32, *coords.get(&31).unwrap_or(&0.0) as f32],
                            color: None,
                        }));
                    }
                    "CIRCLE" => {
                        ir.curves.push(CurveIr::Circle(CircleIr {
                            layer: "0".into(),
                            center: [*coords.get(&10).unwrap_or(&0.0) as f32, *coords.get(&20).unwrap_or(&0.0) as f32, *coords.get(&30).unwrap_or(&0.0) as f32],
                            radius: *coords.get(&40).unwrap_or(&1.0) as f32,
                            color: None,
                        }));
                    }
                    "ARC" => {
                        ir.curves.push(CurveIr::Arc(ArcIr {
                            layer: "0".into(),
                            center: [*coords.get(&10).unwrap_or(&0.0) as f32, *coords.get(&20).unwrap_or(&0.0) as f32, *coords.get(&30).unwrap_or(&0.0) as f32],
                            radius: *coords.get(&40).unwrap_or(&1.0) as f32,
                            start_angle_deg: *coords.get(&50).unwrap_or(&0.0) as f32,
                            end_angle_deg: *coords.get(&51).unwrap_or(&360.0) as f32,
                            color: None,
                        }));
                    }
                    "LWPOLYLINE" => {
                        if poly_pts.len() >= 2 {
                            ir.curves.push(CurveIr::Polyline(PolylineIr {
                                layer: "0".into(),
                                points: poly_pts.iter().map(|p| [p[0] as f32, p[1] as f32, 0.0]).collect(),
                                is_closed: poly_closed,
                                color: None,
                            }));
                        }
                    }
                    "TEXT" | "MTEXT" => {
                        if !text_val.is_empty() {
                            ir.texts.push(TextIr {
                                layer: "0".into(),
                                value: text_val.clone(),
                                position: [*coords.get(&10).unwrap_or(&0.0) as f32, *coords.get(&20).unwrap_or(&0.0) as f32, *coords.get(&30).unwrap_or(&0.0) as f32],
                                height: *coords.get(&40).unwrap_or(&2.5) as f32,
                                rotation_deg: *coords.get(&50).unwrap_or(&0.0) as f32,
                            });
                        }
                    }
                    "DIMENSION" => {
                        let p1 = [*coords.get(&13).unwrap_or(coords.get(&10).unwrap_or(&0.0)) as f32,
                                   *coords.get(&23).unwrap_or(coords.get(&20).unwrap_or(&0.0)) as f32,
                                   *coords.get(&33).unwrap_or(&0.0) as f32];
                        let p2 = [*coords.get(&14).unwrap_or(coords.get(&11).unwrap_or(&0.0)) as f32,
                                   *coords.get(&24).unwrap_or(coords.get(&21).unwrap_or(&0.0)) as f32,
                                   *coords.get(&34).unwrap_or(&0.0) as f32];
                        ir.dimensions.push(DimensionIr {
                            layer: "0".into(),
                            value_text: if text_val.is_empty() { None } else { Some(text_val.clone()) },
                            definition_points: vec![p1, p2],
                        });
                    }
                    "ELLIPSE" => {
                        let cx = *coords.get(&10).unwrap_or(&0.0) as f32;
                        let cy = *coords.get(&20).unwrap_or(&0.0) as f32;
                        let cz = *coords.get(&30).unwrap_or(&0.0) as f32;
                        // Endpoint of major axis (relative to center)
                        let _mx = *coords.get(&11).unwrap_or(&1.0);
                        let _my = *coords.get(&21).unwrap_or(&0.0);
                        let major_len = (_mx * _mx + _my * _my).sqrt() as f32;
                        // 近似為圓
                        ir.curves.push(CurveIr::Circle(CircleIr {
                            layer: "0".into(),
                            center: [cx, cy, cz],
                            radius: major_len.max(0.1),
                            color: None,
                        }));
                    }
                    _ => {}
                }
            }
        }
        i += 2;
    }

    ir.metadata.insert("parser".into(), "kolibri_dxf_from_dwg_convert".into());
    tracing::info!("DXF→GeometryIr: {} curves, {} texts, {} dims",
        ir.curves.len(), ir.texts.len(), ir.dimensions.len());

    if ir.curves.is_empty() && ir.texts.is_empty() && ir.dimensions.is_empty() {
        return Err(ImportError::InvalidFormat("轉換後的 DXF 中沒有圖元".into()));
    }

    Ok(ir)
}

// ═══════════════════════════════════════════════════════════════════════════════
//  DWG 匯出（透過 DXF 中繼轉換）
// ═══════════════════════════════════════════════════════════════════════════════

/// 匯出 DWG：先匯出 DXF，再透過外部工具轉為 DWG
/// 成功回傳 DWG 路徑，失敗回傳錯誤訊息
pub fn export_dwg_via_dxf(dxf_export_fn: impl FnOnce(&str) -> Result<(), String>, dwg_path: &str) -> Result<String, String> {
    // 先匯出 DXF 到臨時檔案
    let tmp_dxf = format!("{}.tmp.dxf", dwg_path);
    dxf_export_fn(&tmp_dxf)?;

    if !std::path::Path::new(&tmp_dxf).exists() {
        return Err("DXF 匯出失敗：臨時檔案未產生".into());
    }

    // 嘗試轉換 DXF → DWG
    let result = try_convert_dxf_to_dwg(&tmp_dxf, dwg_path);

    // 清理臨時 DXF
    let _ = std::fs::remove_file(&tmp_dxf);

    match result {
        Some(_) => {
            tracing::info!("DWG 匯出成功: {}", dwg_path);
            Ok(dwg_path.to_string())
        }
        None => Err(format!(
            "DWG 匯出失敗：無法找到外部轉換工具。\n\
             請安裝以下任一工具：\n\
             1. LibreDWG (dwg2dxf / dxf2dwg)\n\
             2. ODA File Converter\n\
             3. ZWCAD (COM Automation)\n\n\
             替代方案：已匯出 DXF 檔案，可在 ZWCAD/AutoCAD 中另存為 DWG。\n\
             DXF 路徑: {}",
            tmp_dxf.replace(".tmp.dxf", ".dxf")
        ))
    }
}

/// 嘗試用外部工具將 DXF 轉 DWG
fn try_convert_dxf_to_dwg(dxf_path: &str, dwg_path: &str) -> Option<String> {
    // 策略 1: ZWCAD COM Automation
    if let Some(result) = try_zwcad_com_dxf_to_dwg(dxf_path, dwg_path) {
        return Some(result);
    }

    // 策略 2: LibreDWG dxf2dwg
    if let Ok(output) = std::process::Command::new("dxf2dwg")
        .arg("-o").arg(dwg_path).arg(dxf_path)
        .output()
    {
        if output.status.success() && std::path::Path::new(dwg_path).exists() {
            tracing::info!("DXF→DWG via LibreDWG dxf2dwg");
            return Some(dwg_path.to_string());
        }
    }

    // 策略 3: ODA File Converter（DXF→DWG）
    let oda_paths = [
        "C:/Program Files/ODA/ODAFileConverter.exe",
        "C:/Program Files (x86)/ODA/ODAFileConverter/ODAFileConverter.exe",
    ];
    for oda in &oda_paths {
        if std::path::Path::new(oda).exists() {
            let input_dir = std::path::Path::new(dxf_path).parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());
            let output_dir = std::path::Path::new(dwg_path).parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());
            let file_name = std::path::Path::new(dxf_path).file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            // ODA: input_dir output_dir version type recurse audit filter
            if let Ok(_) = std::process::Command::new(oda)
                .arg(&input_dir).arg(&output_dir)
                .arg("ACAD2018").arg("DWG").arg("0").arg("1")
                .arg(&file_name)
                .output()
            {
                // ODA 產生的檔案名跟原 DXF 同名但換 .dwg
                let stem = std::path::Path::new(dxf_path).file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let oda_dwg = format!("{}/{}.dwg", output_dir, stem);
                if std::path::Path::new(&oda_dwg).exists() {
                    // 移動到目標路徑
                    if oda_dwg != dwg_path {
                        let _ = std::fs::rename(&oda_dwg, dwg_path);
                    }
                    tracing::info!("DXF→DWG via ODA FileConverter");
                    return Some(dwg_path.to_string());
                }
            }
        }
    }

    None
}

/// 用 ZWCAD COM 做 DXF → DWG 轉換
fn try_zwcad_com_dxf_to_dwg(dxf_path: &str, dwg_path: &str) -> Option<String> {
    let zwcad_exists = std::path::Path::new("C:/Program Files/ZWCAD/ZWCAD.exe").exists();
    if !zwcad_exists { return None; }

    tracing::info!("Attempting ZWCAD COM DXF→DWG: {} → {}", dxf_path, dwg_path);

    let ps_script = format!(
        r#"
try {{
    $zwcad = New-Object -ComObject 'ZWCAD.Application'
    $zwcad.Visible = $false
    $doc = $zwcad.Documents.Open('{}')
    $doc.SaveAs('{}', 0)
    $doc.Close($false)
    if ($zwcad.Documents.Count -eq 0) {{ $zwcad.Quit() }}
    Write-Host 'OK'
}} catch {{
    Write-Host "ERR: $_"
}}
"#,
        dxf_path.replace('\\', "\\\\").replace('\'', "\\'"),
        dwg_path.replace('\\', "\\\\").replace('\'', "\\'"),
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
            if stdout.contains("OK") && std::path::Path::new(dwg_path).exists() {
                tracing::info!("DXF→DWG via ZWCAD COM: success");
                return Some(dwg_path.to_string());
            }
            None
        }
        Err(_) => None,
    }
}

/// 檢查系統上有哪些 DWG 轉換工具可用
pub fn available_dwg_tools() -> Vec<String> {
    let mut tools = Vec::new();

    if std::path::Path::new("C:/Program Files/ZWCAD/ZWCAD.exe").exists() {
        tools.push("ZWCAD COM Automation".into());
    }

    // LibreDWG
    if std::process::Command::new("dwg2dxf").arg("--version").output().is_ok() {
        tools.push("LibreDWG (dwg2dxf)".into());
    }
    if std::process::Command::new("dxf2dwg").arg("--version").output().is_ok() {
        tools.push("LibreDWG (dxf2dwg)".into());
    }

    // ODA
    let oda_paths = [
        "C:/Program Files/ODA/ODAFileConverter.exe",
        "C:/Program Files (x86)/ODA/ODAFileConverter/ODAFileConverter.exe",
    ];
    for oda in &oda_paths {
        if std::path::Path::new(oda).exists() {
            tools.push(format!("ODA File Converter ({})", oda));
            break;
        }
    }

    tools
}

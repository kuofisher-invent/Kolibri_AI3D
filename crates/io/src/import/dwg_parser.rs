//! Minimal DWG binary format parser
//! Extracts basic geometry (coordinate points, text annotations) from AutoCAD DWG files.
//!
//! DWG is a complex proprietary binary format. This parser performs best-effort
//! extraction by scanning for coordinate pairs (IEEE 754 doubles) and ASCII text
//! sequences. For full fidelity, convert to DXF in AutoCAD first.

use super::unified_ir::*;

pub fn parse_dwg(path: &str) -> Result<UnifiedIR, String> {
    let data = std::fs::read(path).map_err(|e| format!("讀取失敗: {}", e))?;

    if data.len() < 100 {
        return Err("檔案太小，不是有效的 DWG".into());
    }

    // Check version magic (first 6 bytes)
    let version = std::str::from_utf8(&data[0..6]).unwrap_or("");
    let version_name = match version {
        "AC1015" => "AutoCAD 2000",
        "AC1018" => "AutoCAD 2004",
        "AC1021" => "AutoCAD 2007",
        "AC1024" => "AutoCAD 2010",
        "AC1027" => "AutoCAD 2013",
        "AC1032" => "AutoCAD 2018",
        _ => return Err(format!("不支援的 DWG 版本: {:?}", version)),
    };

    let mut ir = UnifiedIR {
        source_format: "dwg".into(),
        source_file: path.into(),
        units: "mm".into(),
        ..Default::default()
    };

    // ── Extract coordinate pairs ────────────────────────────────────────────
    let mut points: Vec<[f64; 2]> = Vec::new();
    let mut raw_scan_count = 0usize;

    let mut i = 64; // skip header area
    while i + 16 <= data.len() {
        let x = f64::from_le_bytes(data[i..i + 8].try_into().unwrap_or([0; 8]));
        let y = f64::from_le_bytes(data[i + 8..i + 16].try_into().unwrap_or([0; 8]));

        // Filter: reasonable CAD coordinates (allow integers, wider range)
        if x.is_finite()
            && y.is_finite()
            && x.abs() < 100_000.0
            && y.abs() < 100_000.0
            && (x.abs() > 10.0 || y.abs() > 10.0)  // skip near-zero noise
        {
            points.push([x, y]);
            raw_scan_count += 1;
        }
        i += 8; // overlapping scan
    }

    // ── Outlier removal: find the largest cluster of points ────────────────
    // Skip points that are far from the densest region
    if points.len() > 10 {
        let mut xs: Vec<f64> = points.iter().map(|p| p[0]).collect();
        let mut ys: Vec<f64> = points.iter().map(|p| p[1]).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Use percentile range (10th-90th) to find the main cluster
        let p10 = xs.len() / 10;
        let p90 = xs.len() * 9 / 10;
        let x_low = xs[p10];
        let x_high = xs[p90];
        let y_low = ys[p10];
        let y_high = ys[p90];
        let range_x = (x_high - x_low).max(1000.0); // min 1m range
        let range_y = (y_high - y_low).max(1000.0);
        let med_x = (x_low + x_high) / 2.0;
        let med_y = (y_low + y_high) / 2.0;
        let before = points.len();
        points.retain(|p| {
            (p[0] - med_x).abs() < range_x * 1.5 && (p[1] - med_y).abs() < range_y * 1.5
        });
        tracing::info!("DWG outlier removal: {} → {} points (cluster center: {:.0},{:.0}, range: {:.0}x{:.0})",
            before, points.len(), med_x, med_y, range_x, range_y);
    }

    // ── Extract ASCII text sequences ────────────────────────────────────────
    let mut texts: Vec<String> = Vec::new();
    let mut ti = 0usize;
    while ti < data.len() {
        let mut end = ti;
        while end < data.len() && data[end] >= 0x20 && data[end] < 0x7F {
            end += 1;
        }
        let len = end - ti;
        if len >= 2 && len <= 100 {
            if let Ok(s) = std::str::from_utf8(&data[ti..end]) {
                let s = s.trim();
                if !s.is_empty()
                    && (s.chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
                        || s.starts_with('+')
                        || s.starts_with('-')
                        || s.contains("EL")
                        || s.contains("FL")
                        || s.parse::<f64>().is_ok())
                {
                    texts.push(s.to_string());
                }
            }
        }
        ti = end + 1;
    }
    texts.sort();
    texts.dedup();

    // ── Deduplicate coordinate points ───────────────────────────────────────
    points.sort_by(|a, b| {
        a[0].partial_cmp(&b[0])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a[1].partial_cmp(&b[1]).unwrap_or(std::cmp::Ordering::Equal))
    });
    points.dedup_by(|a, b| (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01);

    // ── Build bounding-box mesh from extracted points ───────────────────────
    // Normalize to origin (0,0) so the model appears at the grid center
    if points.len() >= 2 {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        for p in &points {
            min_x = min_x.min(p[0]);
            min_y = min_y.min(p[1]);
            max_x = max_x.max(p[0]);
            max_y = max_y.max(p[1]);
        }

        // Shift to origin
        let w = (max_x - min_x) as f32;
        let d = (max_y - min_y) as f32;
        let h = 100.0_f32; // flat plate representation

        ir.meshes.push(IrMesh {
            id: "dwg_bounds".into(),
            name: std::path::Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "DWG Import".into()),
            vertices: vec![
                [0.0, 0.0, 0.0],
                [w, 0.0, 0.0],
                [w, 0.0, d],
                [0.0, 0.0, d],
                [0.0, h, 0.0],
                [w, h, 0.0],
                [w, h, d],
                [0.0, h, d],
            ],
            normals: vec![[0.0, 1.0, 0.0]; 8],
            indices: vec![
                0, 1, 2, 0, 2, 3, // bottom
                4, 6, 5, 4, 7, 6, // top
            ],
            material_id: None,
        });

        tracing::info!(
            "DWG bounds: ({:.0},{:.0}) - ({:.0},{:.0}), size {:.0}x{:.0} mm",
            min_x, min_y, max_x, max_y, w, d
        );
    }

    // Store extracted texts as curve annotations for downstream use
    for text in &texts {
        ir.curves.push(IrCurve {
            id: format!("text_{}", text),
            points: vec![[0.0, 0.0]],
            layer: format!("TEXT: {}", text),
            is_closed: false,
        });
    }

    ir.stats.vertex_count = points.len();
    ir.stats.mesh_count = ir.meshes.len();

    // ── Structured debug report ──────────────────────────────────────────
    let bbox_str = if let Some(mesh) = ir.meshes.first() {
        let dx = mesh.vertices.get(1).map(|v| v[0]).unwrap_or(0.0);
        let dz = mesh.vertices.get(2).map(|v| v[2]).unwrap_or(0.0);
        format!("{:.0} × {:.0} mm", dx, dz)
    } else {
        "N/A".to_string()
    };

    ir.debug_report = vec![
        "═══════════════════════════════════════".into(),
        format!("  [DWG Import Report]"),
        format!("  Format: {} ({})", version, version_name),
        format!("  File Size: {} bytes", data.len()),
        format!("  Mode: Binary Basic Scan"),
        "───────────────────────────────────────".into(),
        format!("  Raw Coordinate Pairs Scanned: {}", raw_scan_count),
        format!("  After Outlier Removal: {}", points.len()),
        format!("  Text Annotations Found: {}", texts.len()),
        format!("  BBox: {}", bbox_str),
        "───────────────────────────────────────".into(),
        format!("  Geometry Reconstruction: ❌ Unsupported in binary mode"),
        format!("  Entity Parsing (LINE/ARC): ❌ Requires DXF format"),
        format!("  Layer Detection: ❌ Requires DXF format"),
        "───────────────────────────────────────".into(),
        format!("  ⚠ Recommendation:"),
        format!("    在 AutoCAD/ZWCAD 中另存為 DXF 格式"),
        format!("    使用「DXF 智慧匯入」獲得完整解析"),
        format!("    (軸線/柱梁/標高自動辨識)"),
        "═══════════════════════════════════════".into(),
    ];

    // Also log to standard log
    for line in &ir.debug_report {
        tracing::info!("{}", line);
    }

    Ok(ir)
}

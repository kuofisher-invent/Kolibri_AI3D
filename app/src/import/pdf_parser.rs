//! PDF vector geometry extractor
//! Reads PDF content streams and extracts line/path coordinates from
//! engineering drawings. Supports FlateDecode-compressed streams.

use super::unified_ir::*;

pub fn parse_pdf(path: &str) -> Result<UnifiedIR, String> {
    let data = std::fs::read(path).map_err(|e| format!("讀取失敗: {}", e))?;

    if data.len() < 20 || &data[0..5] != b"%PDF-" {
        return Err("不是有效的 PDF 檔案".into());
    }

    let content = String::from_utf8_lossy(&data);

    let mut ir = UnifiedIR {
        source_format: "pdf".into(),
        source_file: path.into(),
        units: "mm".into(), // we convert from PDF points (1pt = 0.3528mm)
        ..Default::default()
    };

    let mut all_points: Vec<[f64; 2]> = Vec::new();
    let mut lines: Vec<([f64; 2], [f64; 2])> = Vec::new();

    // ── Find and decode all content streams ─────────────────────────────────
    let mut search_from = 0usize;
    while search_from < data.len() {
        // Look for "stream\r\n" or "stream\n"
        let stream_marker = find_stream_start(&data, search_from);
        let (abs_start, content_start) = match stream_marker {
            Some(v) => v,
            None => break,
        };

        // Find matching "endstream"
        let end_marker = find_bytes(&data[content_start..], b"endstream");
        let abs_end = match end_marker {
            Some(offset) => content_start + offset,
            None => break,
        };

        let stream_data = &data[content_start..abs_end];

        // Try to decompress (FlateDecode), fall back to raw
        let decoded = try_inflate(stream_data).unwrap_or_else(|_| stream_data.to_vec());

        // Parse PDF drawing operators from decoded stream
        if let Ok(text) = std::str::from_utf8(&decoded) {
            parse_pdf_operators(text, &mut all_points, &mut lines);
        }

        search_from = abs_end + 9; // skip past "endstream"
    }

    // ── Extract text from BT..ET blocks ─────────────────────────────────────
    let mut texts: Vec<String> = Vec::new();
    let mut bt_search = 0usize;
    while let Some(bt_rel) = content[bt_search..].find("BT") {
        let abs_bt = bt_search + bt_rel;
        if let Some(et_rel) = content[abs_bt..].find("ET") {
            let text_block = &content[abs_bt..abs_bt + et_rel];
            for line in text_block.lines() {
                let line = line.trim();
                if line.ends_with("Tj") || line.ends_with("TJ") {
                    if let (Some(start), Some(end)) = (line.find('('), line.rfind(')')) {
                        if start < end {
                            let text = &line[start + 1..end];
                            if !text.is_empty() && text.len() < 200 {
                                texts.push(text.to_string());
                            }
                        }
                    }
                }
            }
            bt_search = abs_bt + et_rel + 2;
        } else {
            break;
        }
    }

    // ── Convert points to geometry ──────────────────────────────────────────
    let scale = 0.3528_f64; // PDF points → mm

    if all_points.len() >= 2 {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        for p in &all_points {
            min_x = min_x.min(p[0]);
            min_y = min_y.min(p[1]);
            max_x = max_x.max(p[0]);
            max_y = max_y.max(p[1]);
        }

        let w = ((max_x - min_x) * scale) as f32;
        let d = ((max_y - min_y) * scale) as f32;
        let h = w.min(d) * 0.02; // thin slab for plan

        ir.meshes.push(IrMesh {
            id: "pdf_bounds".into(),
            name: std::path::Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "PDF Import".into()),
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
            indices: vec![0, 1, 2, 0, 2, 3, 4, 6, 5, 4, 7, 6],
            material_id: None,
            source_vertex_labels: vec![],
            source_triangle_debug: vec![],
            edges: vec![],
        });

        // Create line segments from extracted paths
        for (start, end) in &lines {
            let s0 = ((start[0] - min_x) * scale) as f32;
            let s1 = ((start[1] - min_y) * scale) as f32;
            let e0 = ((end[0] - min_x) * scale) as f32;
            let e1 = ((end[1] - min_y) * scale) as f32;
            let len = ((e0 - s0).powi(2) + (e1 - s1).powi(2)).sqrt();
            if len > 1.0 {
                ir.curves.push(IrCurve {
                    id: format!("line_{}", ir.curves.len()),
                    points: vec![
                        [start[0] * scale, start[1] * scale],
                        [end[0] * scale, end[1] * scale],
                    ],
                    layer: "PDF".into(),
                    is_closed: false,
                });
            }
        }

        tracing::info!(
            "PDF bounds: ({:.0},{:.0})-({:.0},{:.0}) pts => {:.0}x{:.0} mm",
            min_x, min_y, max_x, max_y, w, d
        );
    }

    ir.stats.vertex_count = all_points.len();
    ir.stats.mesh_count = ir.meshes.len();

    tracing::info!(
        "PDF parsed: size={} bytes, points={}, lines={}, texts={}",
        data.len(),
        all_points.len(),
        lines.len(),
        texts.len()
    );

    Ok(ir)
}

// ─── PDF operator parser ────────────────────────────────────────────────────

fn parse_pdf_operators(
    text: &str,
    points: &mut Vec<[f64; 2]>,
    lines: &mut Vec<([f64; 2], [f64; 2])>,
) {
    let mut current_point: Option<[f64; 2]> = None;
    let mut number_stack: Vec<f64> = Vec::new();

    for token in text.split_whitespace() {
        // Try to parse as number first
        if let Ok(num) = token.parse::<f64>() {
            number_stack.push(num);
            continue;
        }

        match token {
            "m" => {
                // moveto: x y m
                if number_stack.len() >= 2 {
                    let y = number_stack.pop().unwrap_or(0.0);
                    let x = number_stack.pop().unwrap_or(0.0);
                    current_point = Some([x, y]);
                    points.push([x, y]);
                }
                number_stack.clear();
            }
            "l" => {
                // lineto: x y l
                if number_stack.len() >= 2 {
                    let y = number_stack.pop().unwrap_or(0.0);
                    let x = number_stack.pop().unwrap_or(0.0);
                    let new_point = [x, y];
                    if let Some(prev) = current_point {
                        lines.push((prev, new_point));
                    }
                    current_point = Some(new_point);
                    points.push(new_point);
                }
                number_stack.clear();
            }
            "re" => {
                // rectangle: x y w h re
                if number_stack.len() >= 4 {
                    let h = number_stack.pop().unwrap_or(0.0);
                    let w = number_stack.pop().unwrap_or(0.0);
                    let y = number_stack.pop().unwrap_or(0.0);
                    let x = number_stack.pop().unwrap_or(0.0);
                    points.push([x, y]);
                    points.push([x + w, y + h]);
                    lines.push(([x, y], [x + w, y]));
                    lines.push(([x + w, y], [x + w, y + h]));
                    lines.push(([x + w, y + h], [x, y + h]));
                    lines.push(([x, y + h], [x, y]));
                    current_point = Some([x, y]);
                }
                number_stack.clear();
            }
            "c" | "v" | "y" => {
                // curveto variants — take last pair as endpoint
                if number_stack.len() >= 2 {
                    let ey = number_stack.pop().unwrap_or(0.0);
                    let ex = number_stack.pop().unwrap_or(0.0);
                    let new_point = [ex, ey];
                    if let Some(prev) = current_point {
                        lines.push((prev, new_point));
                    }
                    current_point = Some(new_point);
                    points.push(new_point);
                }
                number_stack.clear();
            }
            "h" => {
                // closepath
                number_stack.clear();
            }
            "S" | "s" | "f" | "F" | "B" | "b" | "n" | "W" | "W*" | "f*" | "B*" | "b*" => {
                // stroke / fill / clip — clear state
                number_stack.clear();
            }
            "cm" | "Tm" | "Td" | "TD" => {
                // matrix / text positioning operators — consume numbers
                number_stack.clear();
            }
            _ => {
                // Unknown operator — if it looks like an operator (short, non-numeric), clear stack
                if token.len() <= 3
                    && !token
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
                {
                    number_stack.clear();
                }
            }
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn find_stream_start(data: &[u8], from: usize) -> Option<(usize, usize)> {
    if from >= data.len() {
        return None;
    }
    let haystack = &data[from..];
    // Find "stream" followed by \r\n or \n
    for i in 0..haystack.len().saturating_sub(8) {
        if &haystack[i..i + 6] == b"stream" {
            let after = i + 6;
            if after < haystack.len() && haystack[after] == b'\r' {
                if after + 1 < haystack.len() && haystack[after + 1] == b'\n' {
                    return Some((from + i, from + after + 2));
                }
            } else if after < haystack.len() && haystack[after] == b'\n' {
                return Some((from + i, from + after + 1));
            }
        }
    }
    None
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

fn try_inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut decoder = flate2::read::ZlibDecoder::new(data);
    let mut result = Vec::new();
    decoder
        .read_to_end(&mut result)
        .map_err(|e| e.to_string())?;
    Ok(result)
}

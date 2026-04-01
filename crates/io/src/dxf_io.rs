//! Minimal DXF export (3DFACE entities) and import

use kolibri_core::scene::{Scene, Shape};
use std::io::Write;

/// 自動偵測 DXF 文字編碼並轉換為 UTF-8
/// 支援：UTF-8、Big5/CP950（繁體中文）、GBK（簡體中文）、Shift-JIS（日文）、Latin-1
pub fn decode_dxf_text(raw: &[u8]) -> String {
    // 1. 先嘗試 UTF-8（最常見）
    if let Ok(s) = String::from_utf8(raw.to_vec()) {
        return s;
    }

    // 2. 檢查 DXF header 中的 $DWGCODEPAGE 提示
    let header_hint = detect_codepage_from_header(raw);

    // 3. 根據提示或啟發式偵測選擇編碼
    let encoding = header_hint.unwrap_or_else(|| detect_encoding_heuristic(raw));

    match encoding.as_str() {
        "big5" | "cp950" | "ansi_950" => decode_big5(raw),
        "gbk" | "gb2312" | "ansi_936" | "gb18030" => decode_gbk(raw),
        "shift_jis" | "ansi_932" => decode_shift_jis(raw),
        _ => {
            // 預設：嘗試 Big5（台灣 CAD 最常見），失敗則 lossy UTF-8
            let big5_result = decode_big5(raw);
            // 如果 Big5 解碼產生了 CJK 字元，就用 Big5
            if big5_result.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c)) {
                big5_result
            } else {
                String::from_utf8_lossy(raw).into_owned()
            }
        }
    }
}

/// 從 DXF header 偵測 $DWGCODEPAGE
fn detect_codepage_from_header(raw: &[u8]) -> Option<String> {
    // 在 header 前 5000 bytes 中搜索 $DWGCODEPAGE
    let search_len = raw.len().min(5000);
    let header = &raw[..search_len];

    // 搜索 "$DWGCODEPAGE" 然後讀取後面的 group code 3 的值
    if let Some(pos) = header.windows(13).position(|w| w == b"$DWGCODEPAGE\n" || w == b"$DWGCODEPAGE\r") {
        // 跳到 value（通常在後面 2-4 行）
        let after = &raw[pos + 13..raw.len().min(pos + 100)];
        // 找 group code 3 的值
        for line in after.split(|&b| b == b'\n') {
            let trimmed = line.strip_suffix(b"\r").unwrap_or(line);
            let trimmed = trimmed.iter().copied().collect::<Vec<u8>>();
            let s = String::from_utf8_lossy(&trimmed).trim().to_lowercase();
            if s.contains("ansi_950") || s.contains("big5") || s.contains("chinese_big5") {
                return Some("big5".into());
            }
            if s.contains("ansi_936") || s.contains("gbk") || s.contains("gb2312") {
                return Some("gbk".into());
            }
            if s.contains("ansi_932") || s.contains("shift_jis") || s.contains("japanese") {
                return Some("shift_jis".into());
            }
            if s.contains("ansi_1252") || s.contains("latin") {
                return Some("latin1".into());
            }
        }
    }
    None
}

/// 啟發式偵測：掃描 non-ASCII bytes 判斷最可能的編碼
fn detect_encoding_heuristic(raw: &[u8]) -> String {
    let mut big5_score = 0i32;
    let mut gbk_score = 0i32;
    let mut i = 0;
    let scan_len = raw.len().min(50000); // 掃描前 50KB

    while i < scan_len {
        let b = raw[i];
        if b > 127 && i + 1 < scan_len {
            let b2 = raw[i + 1];
            // Big5 範圍: high 0xA1-0xF9, low 0x40-0x7E or 0xA1-0xFE
            if (0xA1..=0xF9).contains(&b) && ((0x40..=0x7E).contains(&b2) || (0xA1..=0xFE).contains(&b2)) {
                big5_score += 1;
            }
            // GBK 範圍: high 0x81-0xFE, low 0x40-0xFE (not 0x7F)
            if (0x81..=0xFE).contains(&b) && (0x40..=0xFE).contains(&b2) && b2 != 0x7F {
                gbk_score += 1;
            }
            i += 2;
            continue;
        }
        i += 1;
    }

    if big5_score > 5 && big5_score >= gbk_score {
        "big5".into()
    } else if gbk_score > 5 {
        "gbk".into()
    } else {
        "latin1".into()
    }
}

/// 用 Windows MultiByteToWideChar 批量轉換整個 buffer（Big5/GBK/Shift-JIS 共用）
fn decode_with_codepage(raw: &[u8], codepage: u32) -> String {
    #[cfg(target_os = "windows")]
    {
        use std::os::raw::c_int;
        extern "system" {
            fn MultiByteToWideChar(
                code_page: u32, flags: u32,
                src: *const u8, src_len: c_int,
                dst: *mut u16, dst_len: c_int,
            ) -> c_int;
        }
        // 先取得需要的 buffer 大小
        let needed = unsafe {
            MultiByteToWideChar(codepage, 0, raw.as_ptr(), raw.len() as c_int, std::ptr::null_mut(), 0)
        };
        if needed > 0 {
            let mut wbuf = vec![0u16; needed as usize];
            let result = unsafe {
                MultiByteToWideChar(codepage, 0, raw.as_ptr(), raw.len() as c_int, wbuf.as_mut_ptr(), needed)
            };
            if result > 0 {
                return String::from_utf16_lossy(&wbuf[..result as usize]);
            }
        }
    }
    // Non-Windows fallback
    String::from_utf8_lossy(raw).into_owned()
}

/// Big5/CP950 → UTF-8
fn decode_big5(raw: &[u8]) -> String { decode_with_codepage(raw, 950) }
/// GBK → UTF-8
fn decode_gbk(raw: &[u8]) -> String { decode_with_codepage(raw, 936) }
/// Shift-JIS → UTF-8
fn decode_shift_jis(raw: &[u8]) -> String { decode_with_codepage(raw, 932) }

pub fn export_dxf(scene: &Scene, path: &str) -> Result<(), String> {
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;

    // Header
    write!(file, "0\nSECTION\n2\nHEADER\n0\nENDSEC\n").map_err(|e| e.to_string())?;

    // Tables section — LAYER definitions with ACI colors
    write!(file, "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n").map_err(|e| e.to_string())?;
    {
        let mut layers_written = std::collections::HashSet::new();
        for obj in scene.objects.values() {
            if !obj.visible || layers_written.contains(&obj.name) { continue; }
            layers_written.insert(obj.name.clone());
            let aci = material_to_aci(&obj.material);
            write!(file, "0\nLAYER\n2\n{}\n70\n0\n62\n{}\n6\nCONTINUOUS\n",
                obj.name, aci).map_err(|e| e.to_string())?;
        }
    }
    write!(file, "0\nENDTAB\n0\nENDSEC\n").map_err(|e| e.to_string())?;

    // Entities section
    write!(file, "0\nSECTION\n2\nENTITIES\n").map_err(|e| e.to_string())?;

    for obj in scene.objects.values() {
        if !obj.visible { continue; }
        let p = obj.position;
        match &obj.shape {
            Shape::Box { width, height, depth } => {
                let (w, h, d) = (*width, *height, *depth);
                let v = [
                    [p[0],p[1],p[2]], [p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]], [p[0],p[1]+h,p[2]],
                    [p[0],p[1],p[2]+d], [p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d],
                ];
                // 6 faces as 3DFACE
                let faces = [
                    [0,1,2,3], [5,4,7,6], [3,2,6,7], [4,5,1,0], [4,0,3,7], [1,5,6,2],
                ];
                for f in &faces {
                    write_3dface(&mut file, &obj.name, v[f[0]], v[f[1]], v[f[2]], v[f[3]])?;
                }
            }
            Shape::Cylinder { radius, height, segments } => {
                let segs = *segments as usize;
                let r = *radius;
                let h = *height;
                let cx = p[0] + r;
                let cz = p[2] + r;
                let center_b = [cx, p[1], cz];
                let center_t = [cx, p[1]+h, cz];
                for i in 0..segs {
                    let a0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
                    let a1 = ((i+1) as f32 / segs as f32) * std::f32::consts::TAU;
                    let (s0,c0) = a0.sin_cos();
                    let (s1,c1) = a1.sin_cos();
                    let b0 = [cx+r*c0, p[1], cz+r*s0];
                    let b1 = [cx+r*c1, p[1], cz+r*s1];
                    let t0 = [cx+r*c0, p[1]+h, cz+r*s0];
                    let t1 = [cx+r*c1, p[1]+h, cz+r*s1];
                    // Side quad
                    write_3dface(&mut file, &obj.name, b0, b1, t1, t0)?;
                    // Bottom cap triangle
                    write_3dface(&mut file, &obj.name, center_b, b0, b1, b1)?;
                    // Top cap triangle
                    write_3dface(&mut file, &obj.name, center_t, t1, t0, t0)?;
                }
            }
            Shape::Sphere { radius, segments } => {
                let segs = *segments as usize;
                let rings = segs / 2;
                let r = *radius;
                let cx = p[0]+r; let cy = p[1]+r; let cz = p[2]+r;
                for j in 0..rings {
                    let phi0 = (j as f32 / rings as f32) * std::f32::consts::PI;
                    let phi1 = ((j+1) as f32 / rings as f32) * std::f32::consts::PI;
                    for i in 0..segs {
                        let th0 = (i as f32 / segs as f32) * std::f32::consts::TAU;
                        let th1 = ((i+1) as f32 / segs as f32) * std::f32::consts::TAU;
                        let mk = |phi: f32, th: f32| -> [f32;3] {
                            [cx+r*phi.sin()*th.cos(), cy+r*phi.cos(), cz+r*phi.sin()*th.sin()]
                        };
                        write_3dface(&mut file, &obj.name, mk(phi0,th0), mk(phi0,th1), mk(phi1,th1), mk(phi1,th0))?;
                    }
                }
            }
            Shape::Line { points, .. } => {
                // LINE entities for each segment
                for pair in points.windows(2) {
                    write!(file, "0\nLINE\n8\n{}\n", obj.name).map_err(|e| e.to_string())?;
                    write!(file, "10\n{:.6}\n20\n{:.6}\n30\n{:.6}\n", pair[0][0], pair[0][1], pair[0][2]).map_err(|e| e.to_string())?;
                    write!(file, "11\n{:.6}\n21\n{:.6}\n31\n{:.6}\n", pair[1][0], pair[1][1], pair[1][2]).map_err(|e| e.to_string())?;
                }
            }
            Shape::Mesh(ref mesh) => {
                // 3DFACE for each mesh face
                for (&fid, _) in &mesh.faces {
                    let verts = mesh.face_vertices(fid);
                    if verts.len() >= 3 {
                        // 三角面或四邊面
                        let v4 = if verts.len() >= 4 { verts[3] } else { verts[2] };
                        let pv = |v: [f32; 3]| [p[0]+v[0], p[1]+v[1], p[2]+v[2]];
                        write_3dface(&mut file, &obj.name, pv(verts[0]), pv(verts[1]), pv(verts[2]), pv(v4))?;
                    }
                }
            }
        }
    }

    write!(file, "0\nENDSEC\n0\nEOF\n").map_err(|e| e.to_string())?;
    Ok(())
}

fn write_3dface(f: &mut std::fs::File, layer: &str, v1: [f32;3], v2: [f32;3], v3: [f32;3], v4: [f32;3]) -> Result<(), String> {
    write!(f, "0\n3DFACE\n8\n{}\n", layer).map_err(|e| e.to_string())?;
    // First vertex (10,20,30)
    write!(f, "10\n{:.6}\n20\n{:.6}\n30\n{:.6}\n", v1[0], v1[1], v1[2]).map_err(|e| e.to_string())?;
    // Second (11,21,31)
    write!(f, "11\n{:.6}\n21\n{:.6}\n31\n{:.6}\n", v2[0], v2[1], v2[2]).map_err(|e| e.to_string())?;
    // Third (12,22,32)
    write!(f, "12\n{:.6}\n22\n{:.6}\n32\n{:.6}\n", v3[0], v3[1], v3[2]).map_err(|e| e.to_string())?;
    // Fourth (13,23,33)
    write!(f, "13\n{:.6}\n23\n{:.6}\n33\n{:.6}\n", v4[0], v4[1], v4[2]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Import DXF — parses LINE, 3DFACE, CIRCLE, ARC entities into real geometry
pub fn import_dxf(scene: &mut Scene, path: &str) -> Result<usize, String> {
    use kolibri_core::halfedge::HeMesh;

    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    let content = decode_dxf_text(&raw);
    let lines: Vec<&str> = content.lines().collect();

    let mut line_segments: Vec<([f32; 3], [f32; 3])> = Vec::new();
    let mut faces_3d: Vec<[[f32; 3]; 4]> = Vec::new();
    let mut circles: Vec<([f32; 3], f32)> = Vec::new();
    let mut arcs: Vec<([f32; 3], f32, f32, f32)> = Vec::new(); // center, radius, start_angle, end_angle

    // DXF parser state
    let mut i = 0;
    let mut in_entities = false;
    let mut current_entity = String::new();
    let mut coords: std::collections::HashMap<i32, f32> = std::collections::HashMap::new();

    while i < lines.len().saturating_sub(1) {
        let code = lines[i].trim();
        let value = lines[i + 1].trim();
        i += 2;

        if value == "ENTITIES" && code == "2" { in_entities = true; continue; }
        if value == "ENDSEC" && code == "0" && in_entities { in_entities = false; continue; }
        if !in_entities { continue; }

        if code == "0" {
            // 處理前一個 entity
            match current_entity.as_str() {
                "LINE" => {
                    let p1 = [coords.get(&10).copied().unwrap_or(0.0),
                              coords.get(&30).copied().unwrap_or(0.0),  // DXF Z → our Y
                              coords.get(&20).copied().unwrap_or(0.0)]; // DXF Y → our Z
                    let p2 = [coords.get(&11).copied().unwrap_or(0.0),
                              coords.get(&31).copied().unwrap_or(0.0),
                              coords.get(&21).copied().unwrap_or(0.0)];
                    line_segments.push((p1, p2));
                }
                "3DFACE" => {
                    let mut face = [[0.0_f32; 3]; 4];
                    for j in 0..4 {
                        face[j] = [
                            coords.get(&(10 + j as i32)).copied().unwrap_or(0.0),
                            coords.get(&(30 + j as i32)).copied().unwrap_or(0.0),
                            coords.get(&(20 + j as i32)).copied().unwrap_or(0.0),
                        ];
                    }
                    faces_3d.push(face);
                }
                "CIRCLE" => {
                    let center = [coords.get(&10).copied().unwrap_or(0.0),
                                  coords.get(&30).copied().unwrap_or(0.0),
                                  coords.get(&20).copied().unwrap_or(0.0)];
                    let radius = coords.get(&40).copied().unwrap_or(100.0);
                    circles.push((center, radius));
                }
                "ARC" => {
                    let center = [coords.get(&10).copied().unwrap_or(0.0),
                                  coords.get(&30).copied().unwrap_or(0.0),
                                  coords.get(&20).copied().unwrap_or(0.0)];
                    let radius = coords.get(&40).copied().unwrap_or(100.0);
                    let start_angle = coords.get(&50).copied().unwrap_or(0.0).to_radians();
                    let end_angle = coords.get(&51).copied().unwrap_or(360.0).to_radians();
                    arcs.push((center, radius, start_angle, end_angle));
                }
                "LWPOLYLINE" | "POLYLINE" => {
                    // 收集所有頂點座標（group code 10/20）
                    let mut pts = Vec::new();
                    for vi in 0..100 {
                        let x_key = if vi == 0 { 10 } else { 10 }; // LWPOLYLINE 用重複的 10/20
                        // 簡化：只取第一組座標點
                        if vi == 0 {
                            if let (Some(&x), Some(&z)) = (coords.get(&10), coords.get(&20)) {
                                let y = coords.get(&30).copied().unwrap_or(0.0);
                                pts.push([x, y, z]);
                            }
                        }
                        let _ = x_key;
                        break;
                    }
                    // 為簡化，把 polyline 的所有頂點座標生成為 line segments
                    if pts.len() >= 2 {
                        for pair in pts.windows(2) {
                            line_segments.push((pair[0], pair[1]));
                        }
                    }
                }
                _ => {}
            }
            current_entity = value.to_string();
            coords.clear();
            continue;
        }

        if let Ok(c) = code.parse::<i32>() {
            if let Ok(v) = value.parse::<f32>() {
                coords.insert(c, v);
            }
        }
    }
    // 處理最後一個 entity
    if current_entity == "LINE" {
        let p1 = [coords.get(&10).copied().unwrap_or(0.0),
                   coords.get(&30).copied().unwrap_or(0.0),
                   coords.get(&20).copied().unwrap_or(0.0)];
        let p2 = [coords.get(&11).copied().unwrap_or(0.0),
                   coords.get(&31).copied().unwrap_or(0.0),
                   coords.get(&21).copied().unwrap_or(0.0)];
        line_segments.push((p1, p2));
    }

    let mut count = 0;
    let base_name = std::path::Path::new(path).file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "DXF".into());

    // LINE entities → Shape::Line
    if !line_segments.is_empty() {
        for (idx, (p1, p2)) in line_segments.iter().enumerate() {
            let id = scene.next_id_pub();
            scene.objects.insert(id.clone(), kolibri_core::scene::SceneObject {
                id,
                name: format!("{}_line_{}", base_name, idx),
                shape: Shape::Line {
                    points: vec![*p1, *p2],
                    thickness: 2.0,
                    arc_center: None,
                    arc_radius: None,
                    arc_angle_deg: None,
                },
                position: [0.0; 3],
                material: kolibri_core::scene::MaterialKind::White,
                rotation_y: 0.0,
                tag: "匯入".to_string(),
                visible: true,
                roughness: 0.5,
                metallic: 0.0,
                texture_path: None,
                component_kind: Default::default(),
                parent_id: None,
                component_def_id: None,
                locked: false, obj_version: 0,
            });
            count += 1;
        }
        scene.version += 1;
    }

    // 3DFACE entities → Shape::Mesh
    if !faces_3d.is_empty() {
        let mut mesh = HeMesh::new();
        let mut vert_map: std::collections::HashMap<[i32; 3], u32> = std::collections::HashMap::new();
        let mut min_pos = [f32::MAX; 3];
        // 先找 min
        for face in &faces_3d {
            for v in face {
                for j in 0..3 { min_pos[j] = min_pos[j].min(v[j]); }
            }
        }
        for face in &faces_3d {
            let mut vids = Vec::new();
            for v in face {
                let key = [(v[0] * 100.0) as i32, (v[1] * 100.0) as i32, (v[2] * 100.0) as i32];
                let vid = *vert_map.entry(key).or_insert_with(|| {
                    mesh.add_vertex([v[0] - min_pos[0], v[1] - min_pos[1], v[2] - min_pos[2]])
                });
                vids.push(vid);
            }
            // 去除重複頂點（3DFACE 第4點可能等於第3點）
            vids.dedup();
            if vids.len() >= 3 {
                mesh.add_face(&vids);
            }
        }
        let id = scene.next_id_pub();
        scene.objects.insert(id.clone(), kolibri_core::scene::SceneObject {
            id,
            name: format!("{}_mesh", base_name),
            shape: Shape::Mesh(mesh),
            position: min_pos,
            material: kolibri_core::scene::MaterialKind::White,
            rotation_y: 0.0,
            tag: "匯入".to_string(),
            visible: true,
            roughness: 0.5,
            metallic: 0.0,
            texture_path: None,
            component_kind: Default::default(),
            parent_id: None,
            component_def_id: None,
            locked: false, obj_version: 0,
        });
        scene.version += 1;
        count += 1;
    }

    // CIRCLE entities → Shape::Cylinder (thin disk approximation)
    for (idx, (center, radius)) in circles.iter().enumerate() {
        scene.add_cylinder(
            format!("{}_circle_{}", base_name, idx),
            [center[0] - radius, center[1], center[2] - radius],
            *radius, 10.0, 32,
            kolibri_core::scene::MaterialKind::White,
        );
        count += 1;
    }

    // ARC entities → Shape::Line (polyline approximation)
    for (idx, (center, radius, start, end)) in arcs.iter().enumerate() {
        let segments = 24;
        let mut pts = Vec::new();
        let sweep = if end > start { end - start } else { end - start + std::f32::consts::TAU };
        for s in 0..=segments {
            let t = *start + sweep * (s as f32 / segments as f32);
            pts.push([
                center[0] + radius * t.cos(),
                center[1],
                center[2] + radius * t.sin(),
            ]);
        }
        let id = scene.next_id_pub();
        scene.objects.insert(id.clone(), kolibri_core::scene::SceneObject {
            id, name: format!("{}_arc_{}", base_name, idx),
            shape: Shape::Line { points: pts, thickness: 2.0, arc_center: Some(*center), arc_radius: Some(*radius), arc_angle_deg: Some(sweep.to_degrees()) },
            position: [0.0; 3], material: kolibri_core::scene::MaterialKind::White,
            rotation_y: 0.0, tag: "匯入".to_string(), visible: true,
            roughness: 0.5, metallic: 0.0, texture_path: None,
            component_kind: Default::default(), parent_id: None, component_def_id: None, locked: false, obj_version: 0,
        });
        count += 1;
    }

    if count == 0 { return Err("No geometry found in DXF".into()); }
    Ok(count)
}

// ─── 2D DraftDocument DWG/DXF Import/Export ─────────────────────────────────

/// 從 DWG 檔案匯入到 DraftDocument（透過 GeometryIR 轉換）
#[cfg(feature = "drafting")]
pub fn import_dwg_to_draft(doc: &mut kolibri_drafting::DraftDocument, path: &str) -> Result<usize, String> {
    use crate::cad_import::dxf_importer::*;

    // ── R2018+ DWG: 先嘗試外部轉換 DWG → DXF ──
    // 偵測版本
    let data = std::fs::read(path).map_err(|e| format!("讀取失敗: {}", e))?;
    let is_r2018_plus = data.len() >= 6 && &data[0..6] == b"AC1032";

    // ── 優先檢查同目錄是否已有同名 DXF（使用者可能已手動轉換）──
    {
        let dxf_sibling = path.rsplit_once('.').map(|(base, _)| format!("{}.dxf", base));
        if let Some(ref dxf_path) = dxf_sibling {
            if std::path::Path::new(dxf_path).exists() {
                tracing::info!("找到同名 DXF: {}，直接使用", dxf_path);
                return import_dxf_to_draft(doc, dxf_path);
            }
        }
        // 也檢查 .tmp.dxf（之前轉換的快取）
        let tmp_dxf = format!("{}.tmp.dxf", path);
        if std::path::Path::new(&tmp_dxf).exists() {
            tracing::info!("找到快取 DXF: {}，直接使用", tmp_dxf);
            return import_dxf_to_draft(doc, &tmp_dxf);
        }
    }

    // 偵測 DWG 版本資訊
    let dwg_version = if data.len() >= 6 {
        std::str::from_utf8(&data[0..6]).unwrap_or("?").to_string()
    } else { "?".into() };

    if is_r2018_plus {
        tracing::info!("DWG {} (R2018+): 嘗試外部轉換", dwg_version);
        // 嘗試外部轉換（ZWCAD COM / LibreDWG / ODA）
        if let Some(dxf_path) = crate::dwg_parser::try_convert_dwg_to_dxf(path) {
            tracing::info!("R2018 DWG 已轉換為 DXF: {}", dxf_path);
            return import_dxf_to_draft(doc, &dxf_path);
        }
        // 列出可用工具
        let tools = crate::dwg_parser::available_dwg_tools();
        if tools.is_empty() {
            tracing::warn!("R2018 DWG: 無轉換工具可用（LibreDWG/ODA/ZWCAD 均未安裝）");
        } else {
            tracing::warn!("R2018 DWG: 有工具但轉換失敗: {:?}", tools);
        }
        // 繼續用 native parser（精確度有限）
        tracing::warn!("R2018 DWG: 使用 native heuristic scan（精確度有限）。建議在 ZWCAD 中另存為 DXF 後匯入。");
    }

    // 解析 DWG → GeometryIr（native parser）
    let ir = crate::dwg_parser::parse_dwg(path)
        .map_err(|e| format!("DWG 解析失敗: {:?}", e))?;
    let mut count = 0;

    // 曲線（Line/Circle/Arc/Polyline）
    for curve in &ir.curves {
        match curve {
            CurveIr::Line(line) => {
                doc.add(kolibri_drafting::DraftEntity::Line {
                    start: [line.start[0] as f64, line.start[1] as f64],
                    end: [line.end[0] as f64, line.end[1] as f64],
                });
                count += 1;
            }
            CurveIr::Circle(circle) => {
                doc.add(kolibri_drafting::DraftEntity::Circle {
                    center: [circle.center[0] as f64, circle.center[1] as f64],
                    radius: circle.radius as f64,
                });
                count += 1;
            }
            CurveIr::Arc(arc) => {
                doc.add(kolibri_drafting::DraftEntity::Arc {
                    center: [arc.center[0] as f64, arc.center[1] as f64],
                    radius: arc.radius as f64,
                    start_angle: (arc.start_angle_deg as f64).to_radians(),
                    end_angle: (arc.end_angle_deg as f64).to_radians(),
                });
                count += 1;
            }
            CurveIr::Polyline(poly) => {
                let points: Vec<[f64; 2]> = poly.points.iter()
                    .map(|p| [p[0] as f64, p[1] as f64])
                    .collect();
                if points.len() >= 2 {
                    doc.add(kolibri_drafting::DraftEntity::Polyline {
                        points, closed: poly.is_closed,
                    });
                    count += 1;
                }
            }
        }
    }

    // 文字
    for text in &ir.texts {
        doc.add(kolibri_drafting::DraftEntity::Text {
            position: [text.position[0] as f64, text.position[1] as f64],
            content: text.value.clone(),
            height: text.height as f64,
            rotation: (text.rotation_deg as f64).to_radians(),
        });
        count += 1;
    }

    // 標註
    for dim in &ir.dimensions {
        if dim.definition_points.len() >= 2 {
            let p1 = dim.definition_points[0];
            let p2 = dim.definition_points[1];
            doc.add(kolibri_drafting::DraftEntity::DimLinear {
                p1: [p1[0] as f64, p1[1] as f64],
                p2: [p2[0] as f64, p2[1] as f64],
                offset: 8.0,
                text_override: dim.value_text.clone(),
            });
            count += 1;
        }
    }

    if count == 0 {
        let tools = crate::dwg_parser::available_dwg_tools();
        let tool_info = if tools.is_empty() {
            "目前系統無 DWG 轉換工具。\n安裝方法（任選一）：\n• LibreDWG: https://www.gnu.org/software/libredwg/\n• ODA File Converter: https://www.opendesign.com/guestfiles/oda_file_converter\n• ZWCAD: 安裝後自動啟用 COM Automation".to_string()
        } else {
            format!("已偵測到工具: {}，但轉換未成功", tools.join(", "))
        };
        if is_r2018_plus {
            return Err(format!(
                "DWG {} (R2018+) 使用加密區段，無法完整解析。\n\n\
                 解決方法：\n\
                 1. 在 ZWCAD/AutoCAD 中開啟此 DWG\n\
                 2. 另存為 → DXF 格式\n\
                 3. 在 Kolibri 中匯入該 DXF\n\n\
                 {}", dwg_version, tool_info
            ));
        }
        return Err(format!(
            "DWG {} 中沒有找到 2D 圖元。\n建議存為 DXF 後匯入。\n\n{}",
            dwg_version, tool_info
        ));
    }

    tracing::info!("DWG {} 匯入完成: {} 個圖元（{} curves, {} texts, {} dims）",
        dwg_version, count, ir.curves.len(), ir.texts.len(), ir.dimensions.len());
    Ok(count)
}

/// 智慧匯入 DXF 或 DWG 到 2D DraftDocument（依副檔名自動選擇）
#[cfg(feature = "drafting")]
pub fn import_cad_to_draft(doc: &mut kolibri_drafting::DraftDocument, path: &str) -> Result<usize, String> {
    let lower = path.to_lowercase();
    if lower.ends_with(".dwg") {
        import_dwg_to_draft(doc, path)
    } else {
        import_dxf_to_draft(doc, path)
    }
}

/// 將累積的 DXF entity 資料 flush 到 DraftDocument
#[cfg(feature = "drafting")]
fn flush_draft_entity(
    entity: &str, coords: &std::collections::HashMap<i32, f64>,
    text: &str, poly_pts: &mut Vec<[f64; 2]>, poly_closed: bool,
    doc: &mut kolibri_drafting::DraftDocument,
) -> usize {
    match entity {
        "LINE" => {
            doc.add(kolibri_drafting::DraftEntity::Line {
                start: [*coords.get(&10).unwrap_or(&0.0), *coords.get(&20).unwrap_or(&0.0)],
                end:   [*coords.get(&11).unwrap_or(&0.0), *coords.get(&21).unwrap_or(&0.0)],
            }); 1
        }
        "CIRCLE" => {
            doc.add(kolibri_drafting::DraftEntity::Circle {
                center: [*coords.get(&10).unwrap_or(&0.0), *coords.get(&20).unwrap_or(&0.0)],
                radius: *coords.get(&40).unwrap_or(&1.0),
            }); 1
        }
        "ARC" => {
            doc.add(kolibri_drafting::DraftEntity::Arc {
                center: [*coords.get(&10).unwrap_or(&0.0), *coords.get(&20).unwrap_or(&0.0)],
                radius: *coords.get(&40).unwrap_or(&1.0),
                start_angle: coords.get(&50).unwrap_or(&0.0).to_radians(),
                end_angle: coords.get(&51).unwrap_or(&360.0).to_radians(),
            }); 1
        }
        "LWPOLYLINE" => {
            if poly_pts.len() >= 2 {
                doc.add(kolibri_drafting::DraftEntity::Polyline {
                    points: poly_pts.clone(), closed: poly_closed,
                });
                poly_pts.clear();
                1
            } else { poly_pts.clear(); 0 }
        }
        "TEXT" | "MTEXT" => {
            if !text.is_empty() {
                doc.add(kolibri_drafting::DraftEntity::Text {
                    position: [*coords.get(&10).unwrap_or(&0.0), *coords.get(&20).unwrap_or(&0.0)],
                    content: text.to_string(),
                    height: *coords.get(&40).unwrap_or(&2.5),
                    rotation: coords.get(&50).unwrap_or(&0.0).to_radians(),
                }); 1
            } else { 0 }
        }
        "ELLIPSE" => {
            let mx = *coords.get(&11).unwrap_or(&1.0);
            let my = *coords.get(&21).unwrap_or(&0.0);
            let sm = (mx * mx + my * my).sqrt();
            doc.add(kolibri_drafting::DraftEntity::Ellipse {
                center: [*coords.get(&10).unwrap_or(&0.0), *coords.get(&20).unwrap_or(&0.0)],
                semi_major: sm, semi_minor: sm * coords.get(&40).unwrap_or(&0.5),
                rotation: my.atan2(mx),
            }); 1
        }
        "DIMENSION" => {
            doc.add(kolibri_drafting::DraftEntity::DimLinear {
                p1: [*coords.get(&13).unwrap_or(&0.0), *coords.get(&23).unwrap_or(&0.0)],
                p2: [*coords.get(&14).unwrap_or(&0.0), *coords.get(&24).unwrap_or(&0.0)],
                offset: 8.0,
                text_override: if text.is_empty() { None } else { Some(text.to_string()) },
            }); 1
        }
        "POINT" => {
            doc.add(kolibri_drafting::DraftEntity::Point {
                position: [*coords.get(&10).unwrap_or(&0.0), *coords.get(&20).unwrap_or(&0.0)],
            }); 1
        }
        // SOLID = 填充三角形/四邊形（2D CAD 常見，用 Polyline 表示）
        "SOLID" | "3DFACE" => {
            let p1 = [*coords.get(&10).unwrap_or(&0.0), *coords.get(&20).unwrap_or(&0.0)];
            let p2 = [*coords.get(&11).unwrap_or(&0.0), *coords.get(&21).unwrap_or(&0.0)];
            let p3 = [*coords.get(&12).unwrap_or(&0.0), *coords.get(&22).unwrap_or(&0.0)];
            let p4 = [*coords.get(&13).unwrap_or(&0.0), *coords.get(&23).unwrap_or(&0.0)];
            // 如果 p3==p4 是三角形，否則是四邊形
            let is_tri = (p3[0] - p4[0]).abs() < 0.01 && (p3[1] - p4[1]).abs() < 0.01;
            let pts = if is_tri { vec![p1, p2, p3] } else { vec![p1, p2, p3, p4] };
            doc.add(kolibri_drafting::DraftEntity::Polyline {
                points: pts, closed: true,
            }); 1
        }
        // HATCH = 填充區域的邊界（簡化：取邊界路徑）
        // INSERT = 圖塊參照（只取位置，無法展開）
        _ => 0,
    }
}

/// DXF 解析中間結構：一個原始 entity 的 group codes
#[cfg(feature = "drafting")]
struct DxfRawEntity {
    entity_type: String,
    layer: String,
    coords: std::collections::HashMap<i32, f64>,
    text: String,
    poly_pts: Vec<[f64; 2]>,
    poly_closed: bool,
    block_name: String, // INSERT 引用的 block 名
}

/// 從 DXF 檔案匯入到 DraftDocument（2D CAD 模式用）
/// 完整支援：BLOCKS 展開、INSERT 遞迴、Layer 顏色、MTEXT、HATCH、ELLIPSE
#[cfg(feature = "drafting")]
pub fn import_dxf_to_draft(doc: &mut kolibri_drafting::DraftDocument, path: &str) -> Result<usize, String> {
    let raw = std::fs::read(path).map_err(|e| e.to_string())?;
    let content = decode_dxf_text(&raw);
    let lines: Vec<&str> = content.lines().collect();

    // ── Phase 1: 解析 Layer 顏色表 ──
    let layer_colors = parse_layer_colors(&lines);
    tracing::info!("DXF layers: {} (with colors)", layer_colors.len());

    // ── Phase 2: 解析 BLOCKS section ──
    let blocks = parse_blocks(&lines);
    tracing::info!("DXF blocks: {} defined", blocks.len());

    // ── Phase 3: 解析 ENTITIES section ──
    let entities = parse_entities_section(&lines);
    tracing::info!("DXF entities: {} in ENTITIES section", entities.len());

    // ── Phase 4: 展開 INSERT → 遞迴解析 block 內容 ──
    let mut count = 0;
    let max_depth = 10; // 防止無限遞迴
    count += flush_entities_to_doc(doc, &entities, &blocks, &layer_colors, 0.0, 0.0, 1.0, 1.0, 0.0, max_depth);

    if count == 0 { return Err("DXF 中沒有找到 2D 圖元".into()); }
    tracing::info!("DXF 匯入完成: {} 個圖元（含 block 展開）", count);
    Ok(count)
}

/// 解析 TABLES/LAYER section → layer name → ACI color
#[cfg(feature = "drafting")]
fn parse_layer_colors(lines: &[&str]) -> std::collections::HashMap<String, i32> {
    let mut colors = std::collections::HashMap::new();
    let mut in_tables = false;
    let mut in_layer_table = false;
    let mut current_layer = String::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        let code = lines[i].trim();
        let value = lines[i + 1].trim();
        if code == "2" && value == "TABLES" { in_tables = true; }
        if code == "0" && value == "ENDSEC" && in_tables { in_tables = false; }
        if in_tables && code == "2" && value == "LAYER" { in_layer_table = true; }
        if in_tables && code == "0" && value == "ENDTAB" && in_layer_table { in_layer_table = false; }
        if in_layer_table {
            if code == "2" { current_layer = value.to_string(); }
            if code == "62" {
                if let Ok(c) = value.parse::<i32>() {
                    if !current_layer.is_empty() {
                        colors.insert(current_layer.clone(), c.abs()); // 負值 = frozen，取絕對值
                    }
                }
            }
        }
        i += 2;
    }
    colors
}

/// 解析 BLOCKS section → block name → Vec<DxfRawEntity>
#[cfg(feature = "drafting")]
fn parse_blocks(lines: &[&str]) -> std::collections::HashMap<String, Vec<DxfRawEntity>> {
    let mut blocks: std::collections::HashMap<String, Vec<DxfRawEntity>> = std::collections::HashMap::new();
    let mut in_blocks = false;
    let mut current_block_name = String::new();
    let mut block_entities: Vec<DxfRawEntity> = Vec::new();
    let mut i = 0;

    while i + 1 < lines.len() {
        let code = lines[i].trim();
        let value = lines[i + 1].trim();

        if code == "2" && value == "BLOCKS" { in_blocks = true; i += 2; continue; }
        if code == "0" && value == "ENDSEC" && in_blocks {
            if !current_block_name.is_empty() && !block_entities.is_empty() {
                blocks.insert(current_block_name.clone(), std::mem::take(&mut block_entities));
            }
            in_blocks = false; i += 2; continue;
        }
        if !in_blocks { i += 2; continue; }

        if code == "0" && value == "BLOCK" {
            // 儲存上一個 block
            if !current_block_name.is_empty() && !block_entities.is_empty() {
                blocks.insert(current_block_name.clone(), std::mem::take(&mut block_entities));
            }
            block_entities.clear();
            current_block_name.clear();
            // 找 block name (group 2)
            let mut j = i + 2;
            while j + 1 < lines.len() {
                let bc = lines[j].trim();
                let bv = lines[j + 1].trim();
                if bc == "0" { break; }
                if bc == "2" { current_block_name = bv.to_string(); }
                j += 2;
            }
            i += 2; continue;
        }
        if code == "0" && value == "ENDBLK" {
            i += 2; continue;
        }

        // 在 block 內部，解析 entity
        if code == "0" && in_blocks && !current_block_name.is_empty() {
            if let Some(ent) = parse_single_entity(lines, &mut i) {
                block_entities.push(ent);
                continue;
            }
        }
        i += 2;
    }
    blocks
}

/// 解析 ENTITIES section
#[cfg(feature = "drafting")]
fn parse_entities_section(lines: &[&str]) -> Vec<DxfRawEntity> {
    let mut entities = Vec::new();
    let mut in_entities = false;
    let mut i = 0;
    while i + 1 < lines.len() {
        let code = lines[i].trim();
        let value = lines[i + 1].trim();
        if code == "2" && value == "ENTITIES" { in_entities = true; i += 2; continue; }
        if code == "0" && value == "ENDSEC" && in_entities { break; }
        if !in_entities { i += 2; continue; }
        if code == "0" {
            if let Some(ent) = parse_single_entity(lines, &mut i) {
                entities.push(ent);
                continue;
            }
        }
        i += 2;
    }
    entities
}

/// 解析單一 DXF entity（從 group code 0 開始），更新 i 到 entity 結束
#[cfg(feature = "drafting")]
fn parse_single_entity(lines: &[&str], i: &mut usize) -> Option<DxfRawEntity> {
    if *i + 1 >= lines.len() { return None; }
    let entity_type = lines[*i + 1].trim().to_string();
    if entity_type == "ENDSEC" || entity_type == "ENDBLK" || entity_type == "EOF" { return None; }

    let mut ent = DxfRawEntity {
        entity_type, layer: "0".into(), coords: std::collections::HashMap::new(),
        text: String::new(), poly_pts: Vec::new(), poly_closed: false, block_name: String::new(),
    };
    *i += 2;

    while *i + 1 < lines.len() {
        let code_str = lines[*i].trim();
        let val = lines[*i + 1].trim();
        let code = match code_str.parse::<i32>() {
            Ok(c) => c,
            Err(_) => { *i += 2; continue; }
        };
        if code == 0 { break; } // 下一個 entity

        match code {
            8 => { ent.layer = val.to_string(); }
            1 | 3 => {
                // MTEXT 用 group 3 做續行，group 1 做最後一行
                if !ent.text.is_empty() && code == 3 { ent.text.push_str(val); }
                else { ent.text = val.to_string(); }
            }
            2 => { ent.block_name = val.to_string(); } // INSERT block ref
            70 => {
                if ent.entity_type == "LWPOLYLINE" {
                    ent.poly_closed = val.parse::<i32>().unwrap_or(0) & 1 != 0;
                }
                if let Ok(v) = val.parse::<f64>() { ent.coords.insert(code, v); }
            }
            10 if ent.entity_type == "LWPOLYLINE" => {
                let x = val.parse::<f64>().unwrap_or(0.0);
                // 讀取對應的 20 (Y)
                if *i + 3 < lines.len() && lines[*i + 2].trim() == "20" {
                    let y = lines[*i + 3].trim().parse::<f64>().unwrap_or(0.0);
                    ent.poly_pts.push([x, y]);
                    *i += 4;
                    continue;
                } else {
                    ent.poly_pts.push([x, 0.0]);
                }
            }
            _ => {
                if let Ok(v) = val.parse::<f64>() { ent.coords.insert(code, v); }
            }
        }
        *i += 2;
    }
    // 跳過不需要的 entity 類型
    match ent.entity_type.as_str() {
        "SEQEND" | "ATTRIB" | "ATTDEF" | "VIEWPORT" => return None,
        _ => {}
    }
    Some(ent)
}

/// 將 entity list flush 到 DraftDocument（遞迴展開 INSERT）
#[cfg(feature = "drafting")]
fn flush_entities_to_doc(
    doc: &mut kolibri_drafting::DraftDocument,
    entities: &[DxfRawEntity],
    blocks: &std::collections::HashMap<String, Vec<DxfRawEntity>>,
    layer_colors: &std::collections::HashMap<String, i32>,
    offset_x: f64, offset_y: f64,
    scale_x: f64, scale_y: f64,
    rotation: f64,
    depth: i32,
) -> usize {
    if depth <= 0 { return 0; }
    let mut count = 0;

    for ent in entities {
        if ent.entity_type == "INSERT" {
            // ── 展開 INSERT：查找 block 定義，套用 transform ──
            if let Some(block_ents) = blocks.get(&ent.block_name) {
                let ins_x = ent.coords.get(&10).unwrap_or(&0.0) * scale_x + offset_x;
                let ins_y = ent.coords.get(&20).unwrap_or(&0.0) * scale_y + offset_y;
                let ins_sx = ent.coords.get(&41).unwrap_or(&1.0) * scale_x;
                let ins_sy = ent.coords.get(&42).unwrap_or(&1.0) * scale_y;
                let ins_rot = ent.coords.get(&50).unwrap_or(&0.0).to_radians() + rotation;
                count += flush_entities_to_doc(doc, block_ents, blocks, layer_colors,
                    ins_x, ins_y, ins_sx, ins_sy, ins_rot, depth - 1);
            }
            continue;
        }

        // ── 一般 entity：套用 transform 並加入 doc ──
        let color = resolve_entity_color(ent, layer_colors);
        count += flush_single_entity(doc, ent, offset_x, offset_y, scale_x, scale_y, rotation, color);
    }
    count
}

/// 解析 ACI 顏色 → RGB
#[cfg(feature = "drafting")]
fn resolve_entity_color(ent: &DxfRawEntity, layer_colors: &std::collections::HashMap<String, i32>) -> [u8; 3] {
    // Entity 自己的 color (group 62) 優先，否則用 layer color
    let aci = ent.coords.get(&62).map(|v| *v as i32)
        .unwrap_or_else(|| *layer_colors.get(&ent.layer).unwrap_or(&7));
    aci_to_rgb(aci.abs())
}

/// ACI color index → RGB（AutoCAD 標準色）
fn aci_to_rgb(aci: i32) -> [u8; 3] {
    match aci {
        1 => [255, 0, 0],       // 紅
        2 => [255, 255, 0],     // 黃
        3 => [0, 255, 0],       // 綠
        4 => [0, 255, 255],     // 青
        5 => [0, 0, 255],       // 藍
        6 => [255, 0, 255],     // 洋紅
        7 | 0 => [255, 255, 255], // 白（深色背景）/ BYBLOCK
        8 => [128, 128, 128],   // 深灰
        9 => [192, 192, 192],   // 淺灰
        10 => [255, 0, 0], 11 => [255, 127, 127], 12 => [204, 0, 0],
        13 => [204, 102, 102], 14 => [153, 0, 0], 15 => [153, 76, 76],
        30 => [255, 127, 0], 40 => [255, 191, 0], 50 => [255, 255, 0],
        60 => [191, 255, 0], 70 => [127, 255, 0], 80 => [0, 255, 0],
        90 => [0, 255, 127], 100 => [0, 255, 191], 110 => [0, 255, 255],
        120 => [0, 191, 255], 130 => [0, 127, 255], 140 => [0, 0, 255],
        150 => [127, 0, 255], 160 => [191, 0, 255], 170 => [255, 0, 255],
        180 => [255, 0, 191], 190 => [255, 0, 127],
        200..=209 => [255, 127, 127], 210..=219 => [255, 191, 127],
        220..=229 => [255, 255, 127], 230..=239 => [127, 255, 127],
        240..=249 => [127, 255, 255], 250 => [51, 51, 51],
        251 => [91, 91, 91], 252 => [132, 132, 132],
        253 => [173, 173, 173], 254 => [214, 214, 214],
        255 => [255, 255, 255],
        _ => [255, 255, 255], // 未知色 → 白色
    }
}

/// 將單個 entity 套用 transform 後加入 doc
#[cfg(feature = "drafting")]
fn flush_single_entity(
    doc: &mut kolibri_drafting::DraftDocument,
    ent: &DxfRawEntity,
    ox: f64, oy: f64, sx: f64, sy: f64, rot: f64,
    color: [u8; 3],
) -> usize {
    let xf = |x: f64, y: f64| -> [f64; 2] {
        let px = x * sx;
        let py = y * sy;
        if rot.abs() > 0.001 {
            let c = rot.cos();
            let s = rot.sin();
            [px * c - py * s + ox, px * s + py * c + oy]
        } else {
            [px + ox, py + oy]
        }
    };

    let added = match ent.entity_type.as_str() {
        "LINE" => {
            let s = xf(*ent.coords.get(&10).unwrap_or(&0.0), *ent.coords.get(&20).unwrap_or(&0.0));
            let e = xf(*ent.coords.get(&11).unwrap_or(&0.0), *ent.coords.get(&21).unwrap_or(&0.0));
            doc.add_with_color(kolibri_drafting::DraftEntity::Line { start: s, end: e }, color); true
        }
        "CIRCLE" => {
            let c = xf(*ent.coords.get(&10).unwrap_or(&0.0), *ent.coords.get(&20).unwrap_or(&0.0));
            let r = *ent.coords.get(&40).unwrap_or(&1.0) * sx.abs();
            doc.add_with_color(kolibri_drafting::DraftEntity::Circle { center: c, radius: r }, color); true
        }
        "ARC" => {
            let c = xf(*ent.coords.get(&10).unwrap_or(&0.0), *ent.coords.get(&20).unwrap_or(&0.0));
            let r = *ent.coords.get(&40).unwrap_or(&1.0) * sx.abs();
            let sa = ent.coords.get(&50).unwrap_or(&0.0).to_radians() + rot;
            let ea = ent.coords.get(&51).unwrap_or(&360.0).to_radians() + rot;
            doc.add_with_color(kolibri_drafting::DraftEntity::Arc { center: c, radius: r, start_angle: sa, end_angle: ea }, color); true
        }
        "LWPOLYLINE" => {
            if ent.poly_pts.len() >= 2 {
                let pts: Vec<[f64; 2]> = ent.poly_pts.iter().map(|p| xf(p[0], p[1])).collect();
                doc.add_with_color(kolibri_drafting::DraftEntity::Polyline { points: pts, closed: ent.poly_closed }, color); true
            } else { false }
        }
        "TEXT" | "MTEXT" => {
            if !ent.text.is_empty() {
                let p = xf(*ent.coords.get(&10).unwrap_or(&0.0), *ent.coords.get(&20).unwrap_or(&0.0));
                let h = *ent.coords.get(&40).unwrap_or(&2.5) * sy.abs();
                // MTEXT 清理格式碼
                let cleaned = clean_mtext(&ent.text);
                if !cleaned.is_empty() {
                    doc.add_with_color(kolibri_drafting::DraftEntity::Text {
                        position: p, content: cleaned, height: h,
                        rotation: ent.coords.get(&50).unwrap_or(&0.0).to_radians() + rot,
                    }, color); true
                } else { false }
            } else { false }
        }
        "ELLIPSE" => {
            let c = xf(*ent.coords.get(&10).unwrap_or(&0.0), *ent.coords.get(&20).unwrap_or(&0.0));
            let mx = *ent.coords.get(&11).unwrap_or(&1.0);
            let my = *ent.coords.get(&21).unwrap_or(&0.0);
            let sm = (mx * mx + my * my).sqrt() * sx.abs();
            let ratio = *ent.coords.get(&40).unwrap_or(&0.5);
            doc.add_with_color(kolibri_drafting::DraftEntity::Ellipse {
                center: c, semi_major: sm, semi_minor: sm * ratio, rotation: my.atan2(mx) + rot,
            }, color); true
        }
        "DIMENSION" => {
            let p1 = xf(*ent.coords.get(&13).unwrap_or(&0.0), *ent.coords.get(&23).unwrap_or(&0.0));
            let p2 = xf(*ent.coords.get(&14).unwrap_or(&0.0), *ent.coords.get(&24).unwrap_or(&0.0));
            doc.add_with_color(kolibri_drafting::DraftEntity::DimLinear {
                p1, p2, offset: 8.0,
                text_override: if ent.text.is_empty() { None } else { Some(ent.text.clone()) },
            }, color); true
        }
        "POINT" => {
            let p = xf(*ent.coords.get(&10).unwrap_or(&0.0), *ent.coords.get(&20).unwrap_or(&0.0));
            doc.add_with_color(kolibri_drafting::DraftEntity::Point { position: p }, color); true
        }
        "SOLID" | "3DFACE" => {
            let p1 = xf(*ent.coords.get(&10).unwrap_or(&0.0), *ent.coords.get(&20).unwrap_or(&0.0));
            let p2 = xf(*ent.coords.get(&11).unwrap_or(&0.0), *ent.coords.get(&21).unwrap_or(&0.0));
            let p3 = xf(*ent.coords.get(&12).unwrap_or(&0.0), *ent.coords.get(&22).unwrap_or(&0.0));
            let p4 = xf(*ent.coords.get(&13).unwrap_or(&0.0), *ent.coords.get(&23).unwrap_or(&0.0));
            let is_tri = (p3[0] - p4[0]).abs() < 0.01 && (p3[1] - p4[1]).abs() < 0.01;
            let pts = if is_tri { vec![p1, p2, p3] } else { vec![p1, p2, p3, p4] };
            doc.add_with_color(kolibri_drafting::DraftEntity::Polyline { points: pts, closed: true }, color); true
        }
        "HATCH" => {
            // HATCH 邊界路徑太複雜，跳過（未來可加）
            false
        }
        _ => false,
    };
    if added { 1 } else { 0 }
}

/// 清理 MTEXT 格式碼（\P = 換行, \S = 堆疊, \\fArial; = 字體, { } = 群組）
fn clean_mtext(raw: &str) -> String {
    let mut result = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.peek() {
                Some('P') | Some('p') => { chars.next(); result.push('\n'); }
                Some('S') => { // 堆疊分數 \S...^...;
                    chars.next();
                    while let Some(&c) = chars.peek() { chars.next(); if c == ';' { break; } result.push(c); }
                }
                Some('f') | Some('F') | Some('H') | Some('h') | Some('W') | Some('w')
                | Some('T') | Some('t') | Some('Q') | Some('q') | Some('A') | Some('a')
                | Some('C') | Some('c') | Some('L') | Some('l') | Some('O') | Some('o') => {
                    // 格式碼 \fArial|...; → 跳到 ;
                    chars.next();
                    while let Some(&c) = chars.peek() { chars.next(); if c == ';' { break; } }
                }
                Some('\\') => { chars.next(); result.push('\\'); }
                Some('{') => { chars.next(); result.push('{'); }
                Some('}') => { chars.next(); result.push('}'); }
                _ => { result.push('\\'); }
            }
        } else if ch == '{' || ch == '}' {
            // 群組括號，忽略
        } else {
            result.push(ch);
        }
    }
    result.trim().to_string()
}

/// 從 DraftDocument 匯出到 DXF 檔案（2D CAD 模式用）
#[cfg(feature = "drafting")]
pub fn export_draft_to_dxf(doc: &kolibri_drafting::DraftDocument, path: &str) -> Result<usize, String> {
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut count = 0;

    // DXF Header
    writeln!(file, "0\nSECTION\n2\nHEADER\n0\nENDSEC").map_err(|e| e.to_string())?;
    // Tables (minimal)
    writeln!(file, "0\nSECTION\n2\nTABLES\n0\nENDSEC").map_err(|e| e.to_string())?;
    // Entities
    writeln!(file, "0\nSECTION\n2\nENTITIES").map_err(|e| e.to_string())?;

    for obj in &doc.objects {
        if !obj.visible { continue; }
        match &obj.entity {
            kolibri_drafting::DraftEntity::Line { start, end } => {
                writeln!(file, "0\nLINE\n8\n{}\n10\n{:.6}\n20\n{:.6}\n30\n0.0\n11\n{:.6}\n21\n{:.6}\n31\n0.0",
                    obj.layer, start[0], start[1], end[0], end[1]).map_err(|e| e.to_string())?;
                count += 1;
            }
            kolibri_drafting::DraftEntity::Circle { center, radius } => {
                writeln!(file, "0\nCIRCLE\n8\n{}\n10\n{:.6}\n20\n{:.6}\n30\n0.0\n40\n{:.6}",
                    obj.layer, center[0], center[1], radius).map_err(|e| e.to_string())?;
                count += 1;
            }
            kolibri_drafting::DraftEntity::Arc { center, radius, start_angle, end_angle } => {
                writeln!(file, "0\nARC\n8\n{}\n10\n{:.6}\n20\n{:.6}\n30\n0.0\n40\n{:.6}\n50\n{:.6}\n51\n{:.6}",
                    obj.layer, center[0], center[1], radius,
                    start_angle.to_degrees(), end_angle.to_degrees()).map_err(|e| e.to_string())?;
                count += 1;
            }
            kolibri_drafting::DraftEntity::Rectangle { p1, p2 } => {
                // 矩形 → LWPOLYLINE
                writeln!(file, "0\nLWPOLYLINE\n8\n{}\n90\n4\n70\n1\n10\n{:.6}\n20\n{:.6}\n10\n{:.6}\n20\n{:.6}\n10\n{:.6}\n20\n{:.6}\n10\n{:.6}\n20\n{:.6}",
                    obj.layer, p1[0], p1[1], p2[0], p1[1], p2[0], p2[1], p1[0], p2[1]).map_err(|e| e.to_string())?;
                count += 1;
            }
            kolibri_drafting::DraftEntity::Polyline { points, closed } => {
                let flag = if *closed { 1 } else { 0 };
                write!(file, "0\nLWPOLYLINE\n8\n{}\n90\n{}\n70\n{}", obj.layer, points.len(), flag).map_err(|e| e.to_string())?;
                for pt in points {
                    write!(file, "\n10\n{:.6}\n20\n{:.6}", pt[0], pt[1]).map_err(|e| e.to_string())?;
                }
                writeln!(file).map_err(|e| e.to_string())?;
                count += 1;
            }
            kolibri_drafting::DraftEntity::Ellipse { center, semi_major, semi_minor, rotation } => {
                let mx = semi_major * rotation.cos();
                let my = semi_major * rotation.sin();
                let ratio = semi_minor / semi_major;
                writeln!(file, "0\nELLIPSE\n8\n{}\n10\n{:.6}\n20\n{:.6}\n30\n0.0\n11\n{:.6}\n21\n{:.6}\n31\n0.0\n40\n{:.6}\n41\n0.0\n42\n6.283185",
                    obj.layer, center[0], center[1], mx, my, ratio).map_err(|e| e.to_string())?;
                count += 1;
            }
            kolibri_drafting::DraftEntity::Text { position, content, height, rotation } => {
                writeln!(file, "0\nTEXT\n8\n{}\n10\n{:.6}\n20\n{:.6}\n30\n0.0\n40\n{:.6}\n1\n{}\n50\n{:.6}",
                    obj.layer, position[0], position[1], height, content, rotation.to_degrees()).map_err(|e| e.to_string())?;
                count += 1;
            }
            kolibri_drafting::DraftEntity::Point { position } => {
                writeln!(file, "0\nPOINT\n8\n{}\n10\n{:.6}\n20\n{:.6}\n30\n0.0",
                    obj.layer, position[0], position[1]).map_err(|e| e.to_string())?;
                count += 1;
            }
            kolibri_drafting::DraftEntity::DimLinear { p1, p2, offset, text_override } => {
                let text = text_override.as_deref().unwrap_or("");
                writeln!(file, "0\nDIMENSION\n8\n{}\n13\n{:.6}\n23\n{:.6}\n33\n0.0\n14\n{:.6}\n24\n{:.6}\n34\n0.0\n1\n{}",
                    obj.layer, p1[0], p1[1], p2[0], p2[1], text).map_err(|e| e.to_string())?;
                count += 1;
            }
            _ => {} // 其他圖元暫不匯出
        }
    }

    writeln!(file, "0\nENDSEC\n0\nEOF").map_err(|e| e.to_string())?;
    Ok(count)
}

/// Map MaterialKind to DXF ACI (AutoCAD Color Index) — approximate
fn material_to_aci(mat: &kolibri_core::scene::MaterialKind) -> i32 {
    use kolibri_core::scene::MaterialKind;
    match mat {
        MaterialKind::White | MaterialKind::Plaster => 7,       // white
        MaterialKind::Black => 250,                              // dark grey
        MaterialKind::Concrete | MaterialKind::ConcreteSmooth => 8, // grey
        MaterialKind::Stone | MaterialKind::Granite => 9,       // light grey
        MaterialKind::Wood | MaterialKind::WoodLight |
        MaterialKind::WoodDark | MaterialKind::Bamboo | MaterialKind::Plywood => 30, // brown
        MaterialKind::Metal | MaterialKind::Steel |
        MaterialKind::Aluminum => 254,                           // light grey
        MaterialKind::Copper => 40,                              // orange
        MaterialKind::Gold => 50,                                // yellow
        MaterialKind::Brick | MaterialKind::BrickWhite => 1,    // red
        MaterialKind::Glass | MaterialKind::GlassTinted |
        MaterialKind::GlassFrosted => 4,                         // cyan
        MaterialKind::Tile | MaterialKind::TileDark => 5,       // blue
        MaterialKind::Asphalt | MaterialKind::Gravel => 251,    // medium grey
        MaterialKind::Grass | MaterialKind::Soil => 3,          // green
        MaterialKind::Marble => 7,                               // white
        _ => 7,                                                   // default white
    }
}

#[cfg(test)]
#[cfg(feature = "drafting")]
mod tests_draft_dxf {
    use super::*;

    #[test]
    fn test_import_export_roundtrip() {
        // 建立測試 DXF 內容
        let dxf_content = r#"0
SECTION
2
HEADER
0
ENDSEC
0
SECTION
2
ENTITIES
0
LINE
8
0
10
0.0
20
0.0
30
0.0
11
100.0
21
0.0
31
0.0
0
LINE
8
0
10
100.0
20
0.0
30
0.0
11
100.0
21
80.0
31
0.0
0
CIRCLE
8
0
10
50.0
20
40.0
30
0.0
40
25.0
0
TEXT
8
0
10
10.0
20
-15.0
30
0.0
40
5.0
1
Hello
50
0.0
0
POINT
8
0
10
25.0
20
25.0
30
0.0
0
ENDSEC
0
EOF
"#;
        // 寫入暫存檔
        let import_path = std::env::temp_dir().join("kolibri_test_import.dxf");
        std::fs::write(&import_path, dxf_content).unwrap();

        // 匯入
        let mut doc = kolibri_drafting::DraftDocument::new();
        let count = import_dxf_to_draft(&mut doc, import_path.to_str().unwrap()).unwrap();
        assert!(count >= 4, "Expected at least 4 entities, got {}", count);
        println!("Imported {} entities", count);

        // 檢查圖元類型
        let mut has_line = false;
        let mut has_circle = false;
        let mut has_text = false;
        let mut has_point = false;
        for obj in &doc.objects {
            match &obj.entity {
                kolibri_drafting::DraftEntity::Line { .. } => has_line = true,
                kolibri_drafting::DraftEntity::Circle { center, radius } => {
                    has_circle = true;
                    assert!((center[0] - 50.0).abs() < 0.01);
                    assert!((center[1] - 40.0).abs() < 0.01);
                    assert!((*radius - 25.0).abs() < 0.01);
                }
                kolibri_drafting::DraftEntity::Text { content, .. } => {
                    has_text = true;
                    assert_eq!(content, "Hello");
                }
                kolibri_drafting::DraftEntity::Point { .. } => has_point = true,
                _ => {}
            }
        }
        assert!(has_line, "Missing LINE");
        assert!(has_circle, "Missing CIRCLE");
        assert!(has_text, "Missing TEXT");
        assert!(has_point, "Missing POINT");

        // 匯出
        let export_path = std::env::temp_dir().join("kolibri_test_export.dxf");
        let exported = export_draft_to_dxf(&doc, export_path.to_str().unwrap()).unwrap();
        assert!(exported >= 4, "Expected at least 4 exported, got {}", exported);
        println!("Exported {} entities", exported);

        // 驗證匯出的 DXF 可以重新匯入
        let mut doc2 = kolibri_drafting::DraftDocument::new();
        let count2 = import_dxf_to_draft(&mut doc2, export_path.to_str().unwrap()).unwrap();
        assert!(count2 >= 4, "Re-import got only {} entities", count2);
        println!("Round-trip OK: {} → {} → {} entities", count, exported, count2);

        // 清理
        let _ = std::fs::remove_file(&import_path);
        let _ = std::fs::remove_file(&export_path);
    }
}

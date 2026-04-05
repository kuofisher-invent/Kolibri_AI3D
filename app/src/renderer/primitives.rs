use super::shaders::Vertex;

pub fn push_line_pub(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    points: &[[f32; 3]], thickness: f32, c: [f32; 4],
) { push_line_segments(v, idx, points, thickness, c); }

pub fn push_box_pub(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], w: f32, h: f32, d: f32, c: [f32; 4],
) { push_box(v, idx, p, w, h, d, c); }

pub fn push_cylinder_pub(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], r: f32, h: f32, seg: u32, c: [f32; 4],
) { push_cylinder(v, idx, p, r, h, seg, c); }

pub fn push_sphere_pub(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], r: f32, seg: u32, c: [f32; 4],
) { push_sphere(v, idx, p, r, seg, c); }

pub(crate) fn push_box(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], w: f32, h: f32, d: f32, c: [f32; 4],
) {
    let [x, y, z] = p;
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([0.0,0.0,-1.0], [[x,y,z],[x+w,y,z],[x+w,y+h,z],[x,y+h,z]]),
        ([0.0,0.0, 1.0], [[x+w,y,z+d],[x,y,z+d],[x,y+h,z+d],[x+w,y+h,z+d]]),
        ([0.0, 1.0,0.0], [[x,y+h,z],[x+w,y+h,z],[x+w,y+h,z+d],[x,y+h,z+d]]),
        ([0.0,-1.0,0.0], [[x,y,z+d],[x+w,y,z+d],[x+w,y,z],[x,y,z]]),
        ([-1.0,0.0,0.0], [[x,y,z+d],[x,y,z],[x,y+h,z],[x,y+h,z+d]]),
        ([ 1.0,0.0,0.0], [[x+w,y,z],[x+w,y,z+d],[x+w,y+h,z+d],[x+w,y+h,z]]),
    ];
    for (n, vs) in &faces {
        let base = v.len() as u32;
        for p in vs {
            v.push(Vertex { position: *p, normal: *n, color: c });
        }
        idx.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
    }
}

pub(crate) fn push_cylinder(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], r: f32, h: f32, seg: u32, c: [f32; 4],
) {
    let [cx, cy, cz] = p;
    let seg = seg.max(6);

    // Side faces
    for i in 0..seg {
        let a0 = (i as f32 / seg as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / seg as f32) * std::f32::consts::TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        // Smooth per-vertex normals: each vertex gets its own radial normal
        let n0 = [c0, 0.0, s0]; // normal for vertices at angle0
        let n1 = [c1, 0.0, s1]; // normal for vertices at angle1
        let base = v.len() as u32;
        v.push(Vertex { position: [cx + r*c0, cy,     cz + r*s0], normal: n0, color: c });
        v.push(Vertex { position: [cx + r*c1, cy,     cz + r*s1], normal: n1, color: c });
        v.push(Vertex { position: [cx + r*c1, cy + h, cz + r*s1], normal: n1, color: c });
        v.push(Vertex { position: [cx + r*c0, cy + h, cz + r*s0], normal: n0, color: c });
        idx.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
    }

    // Top & bottom caps
    let top_n = [0.0, 1.0, 0.0];
    let bot_n = [0.0, -1.0, 0.0];
    let top_center = v.len() as u32;
    v.push(Vertex { position: [cx, cy + h, cz], normal: top_n, color: c });
    let bot_center = v.len() as u32;
    v.push(Vertex { position: [cx, cy, cz], normal: bot_n, color: c });

    for i in 0..seg {
        let a0 = (i as f32 / seg as f32) * std::f32::consts::TAU;
        let a1 = ((i + 1) as f32 / seg as f32) * std::f32::consts::TAU;
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();

        // top
        let b = v.len() as u32;
        v.push(Vertex { position: [cx + r*c0, cy+h, cz + r*s0], normal: top_n, color: c });
        v.push(Vertex { position: [cx + r*c1, cy+h, cz + r*s1], normal: top_n, color: c });
        idx.extend_from_slice(&[top_center, b, b+1]);

        // bottom
        let b = v.len() as u32;
        v.push(Vertex { position: [cx + r*c1, cy, cz + r*s1], normal: bot_n, color: c });
        v.push(Vertex { position: [cx + r*c0, cy, cz + r*s0], normal: bot_n, color: c });
        idx.extend_from_slice(&[bot_center, b, b+1]);
    }
}

pub(crate) fn push_sphere(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], r: f32, seg: u32, c: [f32; 4],
) {
    let [cx, cy, cz] = p;
    let rings = seg.max(4);
    let slices = seg.max(6);

    let base = v.len() as u32;

    for ring in 0..=rings {
        let phi = std::f32::consts::PI * ring as f32 / rings as f32;
        let (sp, cp) = phi.sin_cos();
        for slice in 0..=slices {
            let theta = std::f32::consts::TAU * slice as f32 / slices as f32;
            let (st, ct) = theta.sin_cos();
            let nx = sp * ct;
            let ny = cp;
            let nz = sp * st;
            v.push(Vertex {
                position: [cx + r*nx, cy + r + r*ny, cz + r*nz],
                normal: [nx, ny, nz],
                color: c,
            });
        }
    }

    for ring in 0..rings {
        for slice in 0..slices {
            let a = base + ring * (slices + 1) + slice;
            let b = a + slices + 1;
            idx.extend_from_slice(&[a, a+1, b,  b, a+1, b+1]);
        }
    }
}

pub(crate) fn push_line_segments(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    points: &[[f32; 3]], thickness: f32, c: [f32; 4],
) {
    let half = thickness * 0.5;
    for pair in points.windows(2) {
        let a = glam::Vec3::from(pair[0]);
        let b = glam::Vec3::from(pair[1]);
        let dir = b - a;
        if dir.length_squared() < 0.01 { continue; }

        // Build a thin box along the segment
        let fwd = dir.normalize();
        let up = if fwd.y.abs() > 0.99 { glam::Vec3::Z } else { glam::Vec3::Y };
        let right = fwd.cross(up).normalize() * half;
        let up2 = right.cross(fwd).normalize() * half;

        let corners = [
            a - right - up2, a + right - up2, a + right + up2, a - right + up2,
            b - right - up2, b + right - up2, b + right + up2, b - right + up2,
        ];

        let base = v.len() as u32;
        let faces: [([f32; 3], [usize; 4]); 6] = [
            ((-fwd).into(), [0,3,2,1]),   // front
            (fwd.into(),    [4,5,6,7]),    // back
            (up2.normalize().into(),  [3,7,6,2]),   // top
            ((-up2).normalize().into(), [0,1,5,4]), // bottom
            ((-right).normalize().into(), [0,4,7,3]), // left
            (right.normalize().into(), [1,2,6,5]),    // right
        ];

        for (n, fi) in &faces {
            let i = v.len() as u32;
            for &ci in fi {
                v.push(Vertex { position: corners[ci].into(), normal: *n, color: c });
            }
            idx.extend_from_slice(&[i, i+1, i+2, i, i+2, i+3]);
        }
        let _ = base;
    }
}

// ─── Steel Profile Extrusion ────────────────────────────────────────────────

use crate::scene::{SteelProfileType, SteelProfileParams};

/// 鋼構斷面擠出實體三角化
/// position p = 構件底部起點（local origin），沿 +Y 擠出 length
pub(crate) fn push_steel_profile(
    v: &mut Vec<Vertex>, idx: &mut Vec<u32>,
    p: [f32; 3], profile_type: SteelProfileType, params: &SteelProfileParams,
    length: f32, c: [f32; 4],
) {
    // 1. 生成 2D 截面輪廓（XZ 平面，原點 = 截面中心）+ 強制 CCW
    let mut outline = profile_outline(profile_type, params);
    if outline.len() < 3 { return; }
    ensure_ccw(&mut outline);

    let n = outline.len();
    let [px, py, pz] = p;

    // 2. 底面（Y = py，法線 -Y）— ear-clipping 三角化（支援凹多邊形）
    let tris = ear_clip_2d(&outline);
    let expected = n.saturating_sub(2);
    if tris.len() != expected {
        tracing::warn!(
            "SteelProfile triangulation incomplete: verts={}, tris={}, expected={}",
            n, tris.len(), expected
        );
    }
    {
        let base = v.len() as u32;
        for pt in &outline {
            v.push(Vertex {
                position: [px + pt[0], py, pz + pt[1]],
                normal: [0.0, -1.0, 0.0],
                color: c,
            });
        }
        // 底面法線 -Y → 反轉 winding (c,b,a)
        for (a, b, cc) in &tris {
            idx.extend_from_slice(&[base + *cc as u32, base + *b as u32, base + *a as u32]);
        }
    }

    // 3. 頂面（Y = py + length，法線 +Y）— 共用同一份三角化結果
    {
        let base = v.len() as u32;
        for pt in &outline {
            v.push(Vertex {
                position: [px + pt[0], py + length, pz + pt[1]],
                normal: [0.0, 1.0, 0.0],
                color: c,
            });
        }
        // 頂面正常 winding (a,b,c)
        for (a, b, cc) in &tris {
            idx.extend_from_slice(&[base + *a as u32, base + *b as u32, base + *cc as u32]);
        }
    }

    // 4. 側面（每對相鄰輪廓點 → 矩形 → 2 三角形）
    for i in 0..n {
        let j = (i + 1) % n;
        let a_bot = [px + outline[i][0], py, pz + outline[i][1]];
        let b_bot = [px + outline[j][0], py, pz + outline[j][1]];
        let a_top = [px + outline[i][0], py + length, pz + outline[i][1]];
        let b_top = [px + outline[j][0], py + length, pz + outline[j][1]];

        // 側面法線
        let dx = outline[j][0] - outline[i][0];
        let dz = outline[j][1] - outline[i][1];
        let len = (dx * dx + dz * dz).sqrt().max(0.001);
        let normal = [dz / len, 0.0, -dx / len];

        let base = v.len() as u32;
        v.push(Vertex { position: a_bot, normal, color: c });
        v.push(Vertex { position: b_bot, normal, color: c });
        v.push(Vertex { position: b_top, normal, color: c });
        v.push(Vertex { position: a_top, normal, color: c });
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

/// 生成 2D 截面輪廓點（XZ 平面，逆時針，原點 = 截面中心）
pub(crate) fn profile_outline(pt: SteelProfileType, p: &SteelProfileParams) -> Vec<[f32; 2]> {
    match pt {
        SteelProfileType::H => h_profile_outline(p),
        SteelProfileType::C => c_profile_outline(p),
        SteelProfileType::L => l_profile_outline(p),
    }
}

/// H 型鋼截面輪廓（12 點，無填角時；含填角可增加弧線點）
///
/// ```text
///   ┌──────────┐  ← 頂翼板
///   │          │
///   └──┐    ┌──┘
///      │    │     ← 腹板
///   ┌──┘    └──┐
///   │          │
///   └──────────┘  ← 底翼板
/// ```
fn h_profile_outline(p: &SteelProfileParams) -> Vec<[f32; 2]> {
    let hh = p.h.abs() / 2.0;  // 半高
    let bh = p.b.abs() / 2.0;  // 半寬
    let tw = p.tw.abs() / 2.0;  // 腹板半厚
    let tf = p.tf.abs();

    let mut pts = Vec::with_capacity(32);

    if p.r > 0.0 {
        // 安全限制 r：不超過腹板半高和翼板懸臂，且至少 0.1mm
        let web_half = (p.h - 2.0 * tf) / 2.0;
        let flange_overhang = bh - tw;
        let r = p.r.min((web_half - 0.1).max(0.1)).min((flange_overhang - 0.1).max(0.1));
        let arc_n = 8;

        // 整體路徑 CCW（逆時針），從底翼板左下開始
        // 4 個內填角（凹角）用 CW 短弧描述凹入形狀
        //
        //  ┌──────────────────┐  ← 頂翼板
        //  └──[LT]─┐   ┌─[RT]──┘
        //           │   │        ← 腹板
        //  ┌──[LB]─┘   └─[RB]──┐
        //  └──────────────────┘  ← 底翼板

        // ── 底翼板 ──
        pts.push([-bh, -hh]);
        pts.push([ bh, -hh]);
        pts.push([ bh, -hh + tf]);
        pts.push([ tw + r, -hh + tf]);   // 右下 fillet 翼板切點

        // ── 右下 fillet（CW 短弧：翼板→腹板）──
        // center=(tw+r, -hh+tf+r), 從 -π/2 到 -π
        arc_fillet(&mut pts, [tw + r, -hh + tf + r], r,
            -std::f32::consts::FRAC_PI_2, -std::f32::consts::PI, arc_n);

        // ── 腹板右面（上行）──
        pts.push([ tw,  hh - tf - r]);   // 右上 fillet 腹板切點

        // ── 右上 fillet（CW 短弧：腹板→翼板）──
        // center=(tw+r, hh-tf-r), 從 π 到 π/2
        arc_fillet(&mut pts, [tw + r, hh - tf - r], r,
            std::f32::consts::PI, std::f32::consts::FRAC_PI_2, arc_n);

        pts.push([ bh,  hh - tf]);       // 頂翼板右下

        // ── 頂翼板 ──
        pts.push([ bh,  hh]);
        pts.push([-bh,  hh]);
        pts.push([-bh,  hh - tf]);
        pts.push([-tw - r, hh - tf]);    // 左上 fillet 翼板切點

        // ── 左上 fillet（CW 短弧：翼板→腹板）──
        // center=(-tw-r, hh-tf-r), 從 π/2 到 0
        arc_fillet(&mut pts, [-tw - r, hh - tf - r], r,
            std::f32::consts::FRAC_PI_2, 0.0, arc_n);

        // ── 腹板左面（下行）──
        pts.push([-tw, -hh + tf + r]);   // 左下 fillet 腹板切點

        // ── 左下 fillet（CW 短弧：腹板→翼板）──
        // center=(-tw-r, -hh+tf+r), 從 0 到 -π/2
        arc_fillet(&mut pts, [-tw - r, -hh + tf + r], r,
            0.0, -std::f32::consts::FRAC_PI_2, arc_n);

        pts.push([-bh, -hh + tf]);       // 底翼板左上
        // 回到起點 [-bh, -hh] 由 side-face loop 的 (i+1)%n 自動閉合
    } else {
        // 無填角：12 點 CCW
        pts.push([-bh, -hh]);        // 底翼板左下
        pts.push([ bh, -hh]);        // 底翼板右下
        pts.push([ bh, -hh + tf]);   // 底翼板右上
        pts.push([ tw, -hh + tf]);   // 腹板右下
        pts.push([ tw,  hh - tf]);   // 腹板右上
        pts.push([ bh,  hh - tf]);   // 頂翼板右下
        pts.push([ bh,  hh]);        // 頂翼板右上
        pts.push([-bh,  hh]);        // 頂翼板左上
        pts.push([-bh,  hh - tf]);   // 頂翼板左下
        pts.push([-tw,  hh - tf]);   // 腹板左上
        pts.push([-tw, -hh + tf]);   // 腹板左下
        pts.push([-bh, -hh + tf]);   // 底翼板左上
    }
    pts
}

/// C 型鋼（槽鋼）截面輪廓
///
/// ```text
///   ┌──────┐  ← 頂翼板
///   └──┐   │
///      │   │  ← 腹板
///   ┌──┘   │
///   └──────┘  ← 底翼板
/// ```
fn c_profile_outline(p: &SteelProfileParams) -> Vec<[f32; 2]> {
    let hh = p.h.abs() / 2.0;
    let tw = p.tw.abs();
    let tf = p.tf.abs();
    let b = p.b.abs();

    // 原點 = 截面重心（近似在腹板附近）
    // 簡化：原點在截面幾何中心
    let cx = b / 2.0; // X 偏移讓截面置中

    vec![
        [-cx,       -hh],         // 底翼板左下
        [-cx + b,   -hh],         // 底翼板右下
        [-cx + b,   -hh + tf],    // 底翼板右上
        [-cx + tw,  -hh + tf],    // 腹板右下
        [-cx + tw,   hh - tf],    // 腹板右上
        [-cx + b,    hh - tf],    // 頂翼板右下
        [-cx + b,    hh],         // 頂翼板右上
        [-cx,        hh],         // 頂翼板左上
        [-cx,        hh - tf],    // 腹板外左上
        [-cx,       -hh + tf],    // 腹板外左下
    ]
}

/// L 型鋼（等邊角鋼）截面輪廓
///
/// ```text
///   ┌──┐
///   │  │
///   │  └──────┐
///   └─────────┘
/// ```
fn l_profile_outline(p: &SteelProfileParams) -> Vec<[f32; 2]> {
    let leg = p.h.abs(); // 腿長
    let t = p.tw.abs();  // 板厚

    // 原點 = 角落（內角）
    vec![
        [0.0,  0.0],
        [leg,  0.0],
        [leg,  t],
        [t,    t],
        [t,    leg],
        [0.0,  leg],
    ]
}

/// 在輪廓上加入圓弧填角點（跳過 i=0 避免和前一點重複）
fn arc_fillet(pts: &mut Vec<[f32; 2]>, center: [f32; 2], r: f32, start: f32, end: f32, n: usize) {
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let angle = start + (end - start) * t;
        pts.push([
            center[0] + r * angle.cos(),
            center[1] + r * angle.sin(),
        ]);
    }
}

// ─── Polygon Utilities ──────────────────────────────────────────────────────

/// 計算 2D 多邊形的 signed area（正=CCW，負=CW）
fn polygon_area_2d(pts: &[[f32; 2]]) -> f32 {
    let mut sum = 0.0f32;
    for i in 0..pts.len() {
        let j = (i + 1) % pts.len();
        sum += pts[i][0] * pts[j][1] - pts[j][0] * pts[i][1];
    }
    sum * 0.5
}

/// 確保輪廓為 CCW（逆時針），如果是 CW 則反轉
fn ensure_ccw(pts: &mut Vec<[f32; 2]>) {
    if polygon_area_2d(pts) < 0.0 {
        pts.reverse();
    }
}

// ─── Ear-Clipping Triangulation ─────────────────────────────────────────────

/// 2D 多邊形 ear-clipping 三角化（支援凹多邊形）
/// 輸入：CCW 輪廓點陣列
/// 輸出：三角形索引列表 (a, b, c)，CCW winding
/// 若 ear-clipping 失敗（退化情況），fallback 到 centroid-fan 保證封閉
fn ear_clip_2d(polygon: &[[f32; 2]]) -> Vec<(usize, usize, usize)> {
    let n = polygon.len();
    if n < 3 { return vec![]; }
    if n == 3 { return vec![(0, 1, 2)]; }

    // 判斷 polygon 方向（signed area）
    let mut area2 = 0.0f32;
    for i in 0..n {
        let j = (i + 1) % n;
        area2 += polygon[i][0] * polygon[j][1] - polygon[j][0] * polygon[i][1];
    }
    let ccw = area2 > 0.0;

    let mut indices: Vec<usize> = (0..n).collect();
    let mut result = Vec::with_capacity(n - 2);

    let mut safety = n * n * 2; // 加大安全上限
    while indices.len() > 3 && safety > 0 {
        safety -= 1;
        let len = indices.len();
        let mut found_ear = false;

        for i in 0..len {
            let prev = indices[(i + len - 1) % len];
            let curr = indices[i];
            let next = indices[(i + 1) % len];

            let a = polygon[prev];
            let b = polygon[curr];
            let c = polygon[next];

            // 凸角判定（含容差：近共線點視為凸角）
            let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            let is_convex = if ccw { cross > -1e-6 } else { cross < 1e-6 };
            if !is_convex { continue; }

            // 跳過退化三角形（面積過小）
            if cross.abs() < 1e-10 { continue; }

            // 檢查沒有其他頂點在三角形嚴格內部
            let mut ear = true;
            for j in 0..len {
                let idx = indices[j];
                if idx == prev || idx == curr || idx == next { continue; }
                if point_in_triangle_strict(polygon[idx], a, b, c) {
                    ear = false;
                    break;
                }
            }

            if ear {
                if ccw {
                    result.push((prev, curr, next));
                } else {
                    result.push((next, curr, prev));
                }
                indices.remove(i);
                found_ear = true;
                break;
            }
        }

        if !found_ear {
            // Fallback：對剩餘頂點用 centroid-fan 三角化
            // 計算剩餘頂點的質心
            let mut cx = 0.0f32;
            let mut cy = 0.0f32;
            for &idx in &indices {
                cx += polygon[idx][0];
                cy += polygon[idx][1];
            }
            cx /= indices.len() as f32;
            cy /= indices.len() as f32;

            // 將質心作為虛擬頂點，用 fan 三角化剩餘邊
            // 注意：質心不在原始頂點列表中，需要回傳特殊索引
            // → 改用簡單 fan：以第一個剩餘頂點為中心
            let anchor = indices[0];
            for k in 1..indices.len() - 1 {
                let ia = indices[k];
                let ib = indices[k + 1];
                if ccw {
                    result.push((anchor, ia, ib));
                } else {
                    result.push((ib, ia, anchor));
                }
            }
            break;
        }
    }

    // 剩餘 3 點
    if indices.len() == 3 {
        let (a, b, c) = (indices[0], indices[1], indices[2]);
        if ccw {
            result.push((a, b, c));
        } else {
            result.push((c, b, a));
        }
    }

    result
}

/// 點是否嚴格在三角形內部（排除邊界，避免阻擋合法 ear）
fn point_in_triangle_strict(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let v0 = [c[0] - a[0], c[1] - a[1]];
    let v1 = [b[0] - a[0], b[1] - a[1]];
    let v2 = [p[0] - a[0], p[1] - a[1]];

    let dot00 = v0[0] * v0[0] + v0[1] * v0[1];
    let dot01 = v0[0] * v1[0] + v0[1] * v1[1];
    let dot02 = v0[0] * v2[0] + v0[1] * v2[1];
    let dot11 = v1[0] * v1[0] + v1[1] * v1[1];
    let dot12 = v1[0] * v2[0] + v1[1] * v2[1];

    let denom = dot00 * dot11 - dot01 * dot01;
    if denom.abs() < 1e-10 { return false; } // 退化三角形
    let inv_denom = 1.0 / denom;
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

    // 嚴格內部（加容差排除邊界上的點）
    let eps = 1e-4;
    u > eps && v > eps && (u + v) < 1.0 - eps
}

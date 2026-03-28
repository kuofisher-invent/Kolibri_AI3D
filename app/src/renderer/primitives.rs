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

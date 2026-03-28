use eframe::wgpu;
use crate::scene::{Scene, Shape};
use crate::texture_manager::TextureManager;
use glam::Mat4;
use super::shaders::*;
use super::primitives::*;

// ─── Scene mesh generation ───────────────────────────────────────────────────

pub(crate) fn build_scene_mesh(
    scene: &Scene, selected_ids: &[String], hovered: Option<&str>,
    editing_group_id: Option<&str>,
    editing_component_def_id: Option<&str>,
    hovered_face: Option<(&str, u8)>,
    selected_face: Option<(&str, u8)>,
    edge_thickness_param: f32,
    render_mode: u32,
    texture_manager: &TextureManager,
    view_proj: glam::Mat4,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idx = Vec::new();

    for obj in scene.objects.values() {
        if !obj.visible { continue; }

        // ── Frustum culling: 跳過完全在視錐外的物件 ──
        {
            let p = glam::Vec3::from(obj.position);
            let extent = match &obj.shape {
                Shape::Box { width, height, depth } => glam::Vec3::new(*width, *height, *depth),
                Shape::Cylinder { radius, height, .. } => glam::Vec3::new(*radius * 2.0, *height, *radius * 2.0),
                Shape::Sphere { radius, .. } => glam::Vec3::splat(*radius * 2.0),
                _ => glam::Vec3::splat(1000.0), // Line/Mesh 保守估計
            };
            let center = p + extent * 0.5;
            let radius = extent.length() * 0.5;
            // 球體 vs frustum 測試：投影到 clip space
            let clip = view_proj * glam::Vec4::new(center.x, center.y, center.z, 1.0);
            if clip.w > 0.0 {
                let ndc_x = clip.x / clip.w;
                let ndc_y = clip.y / clip.w;
                let ndc_r = radius / clip.w * 1.5; // 投影半徑（保守放大）
                // 如果球心 + 半徑完全在 NDC 範圍外，跳過
                if ndc_x - ndc_r > 1.5 || ndc_x + ndc_r < -1.5
                    || ndc_y - ndc_r > 1.5 || ndc_y + ndc_r < -1.5
                {
                    continue;
                }
            }
            // clip.w <= 0 表示在相機後方但可能很大（不 cull，安全起見）
        }

        // Use texture average color if a texture is loaded, otherwise material color
        let mut color = if let Some(ref tex_path) = obj.texture_path {
            if texture_manager.is_loaded(tex_path) {
                texture_manager.average_color(tex_path)
            } else {
                obj.material.color()
            }
        } else {
            obj.material.color()
        };
        if selected_ids.iter().any(|s| s == &obj.id) {
            // 選取高亮：材質色調 + 藍色淡化，保留材質可辨識性
            let sel = [0.2_f32, 0.6, 1.0];
            color[0] = color[0] * 0.45 + sel[0] * 0.55;
            color[1] = color[1] * 0.45 + sel[1] * 0.55;
            color[2] = color[2] * 0.45 + sel[2] * 0.55;
        } else if Some(obj.id.as_str()) == hovered {
            // lighten
            color[0] = (color[0] + 0.15).min(1.0);
            color[1] = (color[1] + 0.15).min(1.0);
            color[2] = (color[2] + 0.15).min(1.0);
        }

        // Group isolation: dim non-group objects
        if let Some(gid) = editing_group_id {
            if obj.id != gid {
                color[0] *= 0.3;
                color[1] *= 0.3;
                color[2] *= 0.3;
                color[3] *= 0.3;
            }
        }

        if let Some(def_id) = editing_component_def_id {
            if obj.component_def_id.as_deref() != Some(def_id) {
                color[0] *= 0.3;
                color[1] *= 0.3;
                color[2] *= 0.3;
                color[3] *= 0.3;
            }
        }

        // PBR: 編碼 roughness 到 alpha（非程序紋理材質時）
        if color[3] >= 0.99 || color[3] <= 0.0 {
            // 非 sentinel alpha → 用 roughness 值（0.0-0.89 範圍）
            color[3] = obj.roughness.clamp(0.05, 0.89);
        }

        let p = obj.position;
        let start_idx = verts.len();

        // LOD: 根據螢幕投影大小降低 segment 數
        let lod_segments = |base_segs: u32| -> u32 {
            let center = glam::Vec3::from(p);
            let clip = view_proj * glam::Vec4::new(center.x, center.y, center.z, 1.0);
            if clip.w > 0.0 {
                let screen_size = 500.0 / clip.w; // 粗估螢幕投影大小
                if screen_size < 20.0 { return (base_segs / 4).max(6); }
                if screen_size < 80.0 { return (base_segs / 2).max(8); }
            }
            base_segs
        };

        match &obj.shape {
            Shape::Box { width, height, depth } =>
                push_box(&mut verts, &mut idx, p, *width, *height, *depth, color),
            Shape::Cylinder { radius, height, segments } =>
                push_cylinder(&mut verts, &mut idx, p, *radius, *height, lod_segments(*segments), color),
            Shape::Sphere { radius, segments } =>
                push_sphere(&mut verts, &mut idx, p, *radius, lod_segments(*segments), color),
            Shape::Line { points, thickness, .. } =>
                push_line_segments(&mut verts, &mut idx, points, *thickness, color),
            Shape::Mesh(ref mesh) => {
                for (&fid, face) in &mesh.faces {
                    let face_verts = mesh.face_vertices(fid);
                    if face_verts.len() >= 3 {
                        let base = verts.len() as u32;
                        for fv in &face_verts {
                            verts.push(Vertex {
                                position: [fv[0] + p[0], fv[1] + p[1], fv[2] + p[2]], normal: face.normal, color,
                            });
                        }
                        for i in 1..face_verts.len()-1 {
                            idx.push(base);
                            idx.push(base + i as u32);
                            idx.push(base + (i+1) as u32);
                        }
                    }
                }
                for (p1, p2) in mesh.all_edge_segments() {
                    let mesh_edge_color = if render_mode == 5 { [0.0, 0.0, 0.0, 1.0] } else { [0.15, 0.15, 0.15, 1.0] };
                    let mesh_edge_thick = if render_mode == 5 { edge_thickness_param * 1.5 } else { edge_thickness_param.max(3.0) };
                    let ep1 = [p1[0] + p[0], p1[1] + p[1], p1[2] + p[2]];
                    let ep2 = [p2[0] + p[0], p2[1] + p[1], p2[2] + p[2]];
                    push_line_segments(&mut verts, &mut idx, &[ep1, ep2], mesh_edge_thick, mesh_edge_color);
                }
            }
        }

        // ── Per-vertex triplanar texture sampling for textured objects ──
        if let Some(ref tex_path) = obj.texture_path {
            if texture_manager.is_loaded(tex_path) && !selected_ids.iter().any(|s| s == &obj.id) {
                // Recolor face vertices with triplanar-sampled texture color
                // Use a scale of 0.001 (1 texture repeat per 1000mm = 1m)
                let scale = 0.001;
                for vert in &mut verts[start_idx..] {
                    // Skip edge line vertices (very thin quads have small normals — skip if color is dark edge)
                    if vert.color[0] < 0.2 && vert.color[1] < 0.2 && vert.color[2] < 0.2 {
                        continue;
                    }
                    let tc = texture_manager.triplanar_sample(tex_path, vert.position, vert.normal, scale);
                    vert.color = tc;
                }
            }
        }

        // ── Explicit geometric edge lines for ALL objects (SketchUp-style) ──
        {
            let edge_color = if render_mode == 5 {
                [0.0, 0.0, 0.0, 1.0]  // pure black for sketch
            } else {
                [0.15, 0.15, 0.15, 1.0] // dark edges
            };
            let edge_thickness = if render_mode == 5 {
                edge_thickness_param * 1.5  // thicker in sketch mode
            } else {
                edge_thickness_param
            };
            match &obj.shape {
                Shape::Box { width, height, depth } => {
                    let (w, h, d) = (*width, *height, *depth);
                    let edges: [([f32; 3], [f32; 3]); 12] = [
                        // Bottom
                        ([p[0],p[1],p[2]], [p[0]+w,p[1],p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1],p[2]+d]),
                        ([p[0]+w,p[1],p[2]+d], [p[0],p[1],p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1],p[2]]),
                        // Top
                        ([p[0],p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d]),
                        ([p[0],p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]]),
                        // Verticals
                        ([p[0],p[1],p[2]], [p[0],p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1]+h,p[2]+d]),
                    ];
                    for (a, b) in &edges {
                        push_line_segments(&mut verts, &mut idx, &[*a, *b], edge_thickness, edge_color);
                    }
                }
                Shape::Cylinder { radius, height, segments } => {
                    let seg = (*segments).max(6);
                    let cx = p[0];
                    let cz = p[2];
                    let r = *radius;
                    let h = *height;
                    // Top and bottom circles
                    for y_off in [0.0, h] {
                        let mut circle_pts: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                        for i in 0..=seg {
                            let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                            circle_pts.push([cx + r * a.cos(), p[1] + y_off, cz + r * a.sin()]);
                        }
                        push_line_segments(&mut verts, &mut idx, &circle_pts, edge_thickness, edge_color);
                    }
                    // 4 vertical lines
                    for i in [0, seg / 4, seg / 2, 3 * seg / 4] {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        let px = cx + r * a.cos();
                        let pz = cz + r * a.sin();
                        push_line_segments(&mut verts, &mut idx,
                            &[[px, p[1], pz], [px, p[1] + h, pz]], edge_thickness, edge_color);
                    }
                }
                Shape::Sphere { radius, segments } => {
                    let seg = (*segments).max(6);
                    let r = *radius;
                    let cx = p[0];
                    let cy = p[1] + r;
                    let cz = p[2];
                    // Equator
                    let mut equator: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        equator.push([cx + r * a.cos(), cy, cz + r * a.sin()]);
                    }
                    push_line_segments(&mut verts, &mut idx, &equator, edge_thickness, edge_color);
                    // Meridian XY
                    let mut meridian: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        meridian.push([cx + r * a.cos(), cy + r * a.sin(), cz]);
                    }
                    push_line_segments(&mut verts, &mut idx, &meridian, edge_thickness, edge_color);
                    // Meridian YZ
                    let mut meridian2: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        meridian2.push([cx, cy + r * a.sin(), cz + r * a.cos()]);
                    }
                    push_line_segments(&mut verts, &mut idx, &meridian2, edge_thickness, edge_color);
                }
                _ => {} // Line and Mesh shapes handle their own edges
            }
        }

        // ── Selection outline (bright blue AABB for all shapes) ─────────────
        let is_selected = selected_ids.iter().any(|s| s == &obj.id);
        if is_selected {
            let sel_color = [0.2, 0.5, 1.0, 1.0]; // bright blue
            let edge_thickness = 6.0;

            // 通用 AABB 包圍框（所有 Shape 都適用）
            if let Shape::Mesh(ref mesh) = obj.shape {
                let mut mn = [f32::MAX; 3];
                let mut mx = [f32::MIN; 3];
                for v in mesh.vertices.values() {
                    for i in 0..3 { mn[i] = mn[i].min(v.pos[i] + p[i]); mx[i] = mx[i].max(v.pos[i] + p[i]); }
                }
                if mn[0] < f32::MAX {
                    let box_edges: Vec<([f32; 3], [f32; 3])> = vec![
                        ([mn[0],mn[1],mn[2]], [mx[0],mn[1],mn[2]]),
                        ([mx[0],mn[1],mn[2]], [mx[0],mn[1],mx[2]]),
                        ([mx[0],mn[1],mx[2]], [mn[0],mn[1],mx[2]]),
                        ([mn[0],mn[1],mx[2]], [mn[0],mn[1],mn[2]]),
                        ([mn[0],mx[1],mn[2]], [mx[0],mx[1],mn[2]]),
                        ([mx[0],mx[1],mn[2]], [mx[0],mx[1],mx[2]]),
                        ([mx[0],mx[1],mx[2]], [mn[0],mx[1],mx[2]]),
                        ([mn[0],mx[1],mx[2]], [mn[0],mx[1],mn[2]]),
                        ([mn[0],mn[1],mn[2]], [mn[0],mx[1],mn[2]]),
                        ([mx[0],mn[1],mn[2]], [mx[0],mx[1],mn[2]]),
                        ([mx[0],mn[1],mx[2]], [mx[0],mx[1],mx[2]]),
                        ([mn[0],mn[1],mx[2]], [mn[0],mx[1],mx[2]]),
                    ];
                    for (a, b) in &box_edges {
                        push_line_segments(&mut verts, &mut idx, &[*a, *b], edge_thickness, sel_color);
                    }
                }
            }

            match &obj.shape {
                Shape::Box { width, height, depth } => {
                    let (w, h, d) = (*width, *height, *depth);
                    let edges: Vec<([f32; 3], [f32; 3])> = vec![
                        // Bottom
                        ([p[0],p[1],p[2]], [p[0]+w,p[1],p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1],p[2]+d]),
                        ([p[0]+w,p[1],p[2]+d], [p[0],p[1],p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1],p[2]]),
                        // Top
                        ([p[0],p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1]+h,p[2]], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0]+w,p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]+d]),
                        ([p[0],p[1]+h,p[2]+d], [p[0],p[1]+h,p[2]]),
                        // Verticals
                        ([p[0],p[1],p[2]], [p[0],p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]], [p[0]+w,p[1]+h,p[2]]),
                        ([p[0]+w,p[1],p[2]+d], [p[0]+w,p[1]+h,p[2]+d]),
                        ([p[0],p[1],p[2]+d], [p[0],p[1]+h,p[2]+d]),
                    ];
                    for (a, b) in &edges {
                        push_line_segments(&mut verts, &mut idx, &[*a, *b], edge_thickness, sel_color);
                    }
                }
                Shape::Cylinder { radius, height, segments } => {
                    let seg = (*segments).max(6);
                    let cx = p[0];
                    let cz = p[2];
                    let r = *radius;
                    let h = *height;
                    // Top and bottom circles
                    for y_off in [0.0, h] {
                        let mut circle_pts: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                        for i in 0..=seg {
                            let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                            circle_pts.push([cx + r * a.cos(), p[1] + y_off, cz + r * a.sin()]);
                        }
                        push_line_segments(&mut verts, &mut idx, &circle_pts, edge_thickness, sel_color);
                    }
                    // 4 vertical lines
                    for i in [0, seg / 4, seg / 2, 3 * seg / 4] {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        let px = cx + r * a.cos();
                        let pz = cz + r * a.sin();
                        push_line_segments(&mut verts, &mut idx,
                            &[[px, p[1], pz], [px, p[1] + h, pz]], edge_thickness, sel_color);
                    }
                }
                Shape::Sphere { radius, segments } => {
                    let seg = (*segments).max(6);
                    let r = *radius;
                    let cx = p[0];
                    let cy = p[1] + r; // sphere center is offset by radius
                    let cz = p[2];
                    // Equator (XZ circle at center Y)
                    let mut equator: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        equator.push([cx + r * a.cos(), cy, cz + r * a.sin()]);
                    }
                    push_line_segments(&mut verts, &mut idx, &equator, edge_thickness, sel_color);
                    // Meridian (XY circle)
                    let mut meridian: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        meridian.push([cx + r * a.cos(), cy + r * a.sin(), cz]);
                    }
                    push_line_segments(&mut verts, &mut idx, &meridian, edge_thickness, sel_color);
                    // Second meridian (YZ circle)
                    let mut meridian2: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                    for i in 0..=seg {
                        let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                        meridian2.push([cx, cy + r * a.sin(), cz + r * a.cos()]);
                    }
                    push_line_segments(&mut verts, &mut idx, &meridian2, edge_thickness, sel_color);
                }
                Shape::Mesh(ref mesh) => {
                    for (p1, p2) in mesh.all_edge_segments() {
                        let ep1 = [p1[0] + p[0], p1[1] + p[1], p1[2] + p[2]];
                        let ep2 = [p2[0] + p[0], p2[1] + p[1], p2[2] + p[2]];
                        push_line_segments(&mut verts, &mut idx, &[ep1, ep2], edge_thickness, sel_color);
                    }
                }
                _ => {}
            }
        }

        // ── Face & edge hover highlighting ──────────────────────────────────
        // Use axis-aligned colors: X=Red, Y=Green, Z=Blue (matches SketchUp)
        let face_active = selected_face.or(hovered_face);
        if let Some((hf_id, hf_idx)) = face_active {
            if obj.id == hf_id {
                // Axis color: Front/Back(Z)=Blue, Top/Bottom(Y)=Green, Left/Right(X)=Red
                let (axis_tint, edge_color): ([f32; 3], [f32; 4]) = match hf_idx {
                    0 | 1 => ([0.3, 0.4, 0.95], [0.3, 0.5, 1.0, 1.0]),   // Front/Back → Z = Blue
                    2 | 3 => ([0.3, 0.85, 0.3], [0.2, 0.9, 0.2, 1.0]),   // Top/Bottom → Y = Green
                    4 | 5 => ([0.95, 0.3, 0.3], [1.0, 0.3, 0.3, 1.0]),   // Left/Right → X = Red
                    _     => ([0.5, 0.5, 0.5],  [0.8, 0.8, 0.8, 1.0]),
                };

                match &obj.shape {
                    Shape::Box { width, height, depth } => {
                        // Tint the face vertices with axis color
                        let face_start = start_idx + (hf_idx as usize) * 4;
                        if face_start + 4 <= verts.len() {
                            for i in face_start..face_start + 4 {
                                let c = &mut verts[i].color;
                                c[0] = c[0] * 0.25 + axis_tint[0] * 0.75;
                                c[1] = c[1] * 0.25 + axis_tint[1] * 0.75;
                                c[2] = c[2] * 0.25 + axis_tint[2] * 0.75;
                            }
                        }

                        // Draw edge outline in axis color
                        let px = obj.position;
                        let (w, h, d) = (*width, *height, *depth);
                        let corners: [[f32; 3]; 4] = match hf_idx {
                            0 => [ // Front (Z-)
                                [px[0],px[1],px[2]], [px[0]+w,px[1],px[2]],
                                [px[0]+w,px[1]+h,px[2]], [px[0],px[1]+h,px[2]],
                            ],
                            1 => [ // Back (Z+)
                                [px[0]+w,px[1],px[2]+d], [px[0],px[1],px[2]+d],
                                [px[0],px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]+d],
                            ],
                            2 => [ // Top (Y+)
                                [px[0],px[1]+h,px[2]], [px[0]+w,px[1]+h,px[2]],
                                [px[0]+w,px[1]+h,px[2]+d], [px[0],px[1]+h,px[2]+d],
                            ],
                            3 => [ // Bottom (Y-)
                                [px[0],px[1],px[2]+d], [px[0]+w,px[1],px[2]+d],
                                [px[0]+w,px[1],px[2]], [px[0],px[1],px[2]],
                            ],
                            4 => [ // Left (X-)
                                [px[0],px[1],px[2]+d], [px[0],px[1],px[2]],
                                [px[0],px[1]+h,px[2]], [px[0],px[1]+h,px[2]+d],
                            ],
                            5 => [ // Right (X+)
                                [px[0]+w,px[1],px[2]], [px[0]+w,px[1],px[2]+d],
                                [px[0]+w,px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]],
                            ],
                            _ => [[0.0;3];4],
                        };
                        // Draw closed edge loop (5 points = 4 segments forming a rectangle)
                        let edge_pts = [corners[0], corners[1], corners[2], corners[3], corners[0]];
                        push_line_segments(&mut verts, &mut idx, &edge_pts, 6.0, edge_color);
                    }
                    Shape::Cylinder { radius, height, .. } => {
                        // For cylinders, only top/bottom faces are pick-able
                        // hf_idx 2 = Top, 3 = Bottom (mapped from PullFace::Top/Bottom)
                        let is_top = hf_idx == 2;
                        let face_y = if is_top { obj.position[1] + *height } else { obj.position[1] };
                        // Highlight the cap by drawing a circle outline
                        let seg = 24u32;
                        let cx = obj.position[0] + *radius;
                        let cz = obj.position[2] + *radius;
                        let edge_color = [1.0, 0.9, 0.3, 1.0];
                        let mut circle_pts: Vec<[f32; 3]> = Vec::with_capacity(seg as usize + 1);
                        for i in 0..=seg {
                            let a = (i as f32 / seg as f32) * std::f32::consts::TAU;
                            circle_pts.push([cx + *radius * a.cos(), face_y, cz + *radius * a.sin()]);
                        }
                        push_line_segments(&mut verts, &mut idx, &circle_pts, 6.0, edge_color);
                    }
                    _ => {}
                }
            }
        }

        // ── Click-locked face highlight (stronger than hover) ──────────────
        if let Some((sf_id, sf_idx)) = selected_face {
            if obj.id == sf_id {
                if let Shape::Box { width, height, depth } = &obj.shape {
                    let face_start = start_idx + (sf_idx as usize) * 4;
                    if face_start + 4 <= verts.len() {
                        for i in face_start..face_start + 4 {
                            let c = &mut verts[i].color;
                            c[0] = c[0] * 0.2 + 0.2;
                            c[1] = c[1] * 0.2 + 0.7;
                            c[2] = c[2] * 0.2 + 1.0;
                        }
                    }
                    // Bright cyan edge outline
                    let px = obj.position;
                    let (w, h, d) = (*width, *height, *depth);
                    let edge_color = [0.2, 1.0, 1.0, 1.0]; // cyan
                    let corners: [[f32; 3]; 4] = match sf_idx {
                        0 => [[px[0],px[1],px[2]], [px[0]+w,px[1],px[2]], [px[0]+w,px[1]+h,px[2]], [px[0],px[1]+h,px[2]]],
                        1 => [[px[0]+w,px[1],px[2]+d], [px[0],px[1],px[2]+d], [px[0],px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]+d]],
                        2 => [[px[0],px[1]+h,px[2]], [px[0]+w,px[1]+h,px[2]], [px[0]+w,px[1]+h,px[2]+d], [px[0],px[1]+h,px[2]+d]],
                        3 => [[px[0],px[1],px[2]+d], [px[0]+w,px[1],px[2]+d], [px[0]+w,px[1],px[2]], [px[0],px[1],px[2]]],
                        4 => [[px[0],px[1],px[2]+d], [px[0],px[1],px[2]], [px[0],px[1]+h,px[2]], [px[0],px[1]+h,px[2]+d]],
                        5 => [[px[0]+w,px[1],px[2]], [px[0]+w,px[1],px[2]+d], [px[0]+w,px[1]+h,px[2]+d], [px[0]+w,px[1]+h,px[2]]],
                        _ => [[0.0;3];4],
                    };
                    let edge_pts = [corners[0], corners[1], corners[2], corners[3], corners[0]];
                    push_line_segments(&mut verts, &mut idx, &edge_pts, 8.0, edge_color);
                }
            }
        }

        // Apply Y-axis rotation around object center
        if obj.rotation_y.abs() > 0.001 {
            let (sin, cos) = obj.rotation_y.sin_cos();
            let (center_offset_x, center_offset_z) = match &obj.shape {
                Shape::Box { width, depth, .. } => (*width / 2.0, *depth / 2.0),
                Shape::Cylinder { radius, .. } => (*radius, *radius),
                Shape::Sphere { radius, .. } => (*radius, *radius),
                Shape::Line { .. } => (0.0, 0.0),
                Shape::Mesh(ref mesh) => {
                    let (min, max) = mesh.aabb();
                    ((max[0] - min[0]) / 2.0, (max[2] - min[2]) / 2.0)
                }
            };
            let cx = obj.position[0] + center_offset_x;
            let cz = obj.position[2] + center_offset_z;

            for v in &mut verts[start_idx..] {
                let dx = v.position[0] - cx;
                let dz = v.position[2] - cz;
                v.position[0] = cx + dx * cos - dz * sin;
                v.position[2] = cz + dx * sin + dz * cos;
                // Also rotate normals
                let nx = v.normal[0];
                let nz = v.normal[2];
                v.normal[0] = nx * cos - nz * sin;
                v.normal[2] = nx * sin + nz * cos;
            }
        }
    }

    // ── Render the shared free mesh ──────────────────────────────────────────
    {
        let mesh = &scene.free_mesh;
        let mat_color = scene.free_mesh_material.color();
        let face_color = if editing_group_id.is_some() || editing_component_def_id.is_some() {
            [mat_color[0] * 0.3, mat_color[1] * 0.3, mat_color[2] * 0.3, mat_color[3]]
        } else {
            mat_color
        };

        // Render faces
        for (&fid, face) in &mesh.faces {
            let face_verts = mesh.face_vertices(fid);
            if face_verts.len() >= 3 {
                let base = verts.len() as u32;
                for fv in &face_verts {
                    verts.push(Vertex {
                        position: *fv,
                        normal: face.normal,
                        color: face_color,
                    });
                }
                for i in 1..face_verts.len() - 1 {
                    idx.push(base);
                    idx.push(base + i as u32);
                    idx.push(base + (i + 1) as u32);
                }
            }
        }

        // Render edges as thin lines
        for (p1, p2) in mesh.all_edge_segments() {
            push_line_segments(&mut verts, &mut idx, &[p1, p2], 5.0, [0.1, 0.1, 0.1, 1.0]);
        }
    }

    (verts, idx)
}


use eframe::egui;

use crate::app::{
    compute_arc, DrawState, KolibriApp, PullFace, RenderMode, RightTab, ScaleHandle, SelectionMode, Tool,
};
use crate::camera;
use crate::scene::{MaterialKind, Shape};

impl KolibriApp {
    /// 重建空間索引（場景變更時呼叫）
    pub(crate) fn rebuild_spatial_index(&mut self) {
        if self.scene.version == self.spatial_index_version { return; }
        use crate::app::SpatialEntry;
        let entries: Vec<SpatialEntry> = self.scene.objects.values().map(|obj| {
            let p = obj.position;
            let (mn, mx) = match &obj.shape {
                Shape::Box { width, height, depth } => (p, [p[0]+width, p[1]+height, p[2]+depth]),
                Shape::Cylinder { radius, height, .. } => (p, [p[0]+radius*2.0, p[1]+height, p[2]+radius*2.0]),
                Shape::Sphere { radius, .. } => (p, [p[0]+radius*2.0, p[1]+radius*2.0, p[2]+radius*2.0]),
                Shape::Line { points, thickness, .. } => {
                    let mut mx = p;
                    for pt in points { mx[0] = mx[0].max(pt[0]+thickness); mx[1] = mx[1].max(pt[1]+thickness); mx[2] = mx[2].max(pt[2]+thickness); }
                    (p, mx)
                }
                Shape::Mesh(ref mesh) => { let (a,b) = mesh.aabb(); ([p[0]+a[0],p[1]+a[1],p[2]+a[2]], [p[0]+b[0],p[1]+b[1],p[2]+b[2]]) }
                Shape::SteelProfile { params, length, .. } => ([p[0]-params.b/2.0, p[1], p[2]-params.h/2.0], [p[0]+params.b/2.0, p[1]+length, p[2]+params.h/2.0]),
            };
            SpatialEntry { id: obj.id.clone(), min: mn, max: mx }
        }).collect();
        self.spatial_index = Some(rstar::RTree::bulk_load(entries));
        self.spatial_index_version = self.scene.version;
    }

    pub(crate) fn pick(&mut self, mx: f32, my: f32, vw: f32, vh: f32) -> Option<String> {
        self.rebuild_spatial_index();
        let (origin, dir) = self.viewer.camera.screen_ray(mx, my, vw, vh);
        let mut best: Option<(f32, String)> = None;
        let editing_gid = &self.editor.editing_group_id;

        // 使用空間索引：沿射線採樣多個點，查詢附近的候選物件
        let candidates: Vec<String> = if let Some(ref tree) = self.spatial_index {
            use rstar::RTreeObject;
            let mut ids = std::collections::HashSet::new();
            // 沿射線採樣 10 個點（距離 100-50000mm）
            for i in 0..10 {
                let t = 100.0 + (i as f32) * 5000.0;
                let pt = origin + dir * t;
                let query_pt = [pt.x, pt.y, pt.z];
                // 查詢最近的 5 個 AABB
                for entry in tree.nearest_neighbor_iter(&query_pt).take(5) {
                    ids.insert(entry.id.clone());
                }
            }
            ids.into_iter().collect()
        } else {
            self.scene.objects.keys().cloned().collect()
        };

        for id in &candidates {
            let obj = match self.scene.objects.get(id) { Some(o) => o, None => continue };
            if obj.locked { continue; } // 鎖定物件不可選取
            if let Some(gid) = editing_gid {
                if obj.parent_id.as_deref() != Some(gid.as_str()) && &obj.id != gid { continue; }
            }
            let pos = glam::Vec3::from(obj.position);
            let (pick_min, pick_max) = match &obj.shape {
                Shape::Box { width, height, depth } => {
                    let pick_sz = 30.0_f32;
                    let center = pos + glam::Vec3::new(*width, *height, *depth) * 0.5;
                    let half = glam::Vec3::new(width.max(pick_sz), height.max(pick_sz), depth.max(pick_sz)) * 0.5;
                    (center - half, center + half)
                }
                Shape::Cylinder { radius, height, .. } => (pos, pos + glam::Vec3::new(*radius*2.0, *height, *radius*2.0)),
                Shape::Sphere { radius, .. } => (pos, pos + glam::Vec3::splat(*radius*2.0)),
                Shape::Line { points, thickness, .. } => {
                    let mut mx = pos;
                    for pt in points { mx = mx.max(glam::Vec3::from(*pt) + glam::Vec3::splat(*thickness)); }
                    (pos, mx)
                }
                Shape::Mesh(ref mesh) => {
                    let (aabb_min, aabb_max) = mesh.aabb();
                    (pos + glam::Vec3::from(aabb_min), pos + glam::Vec3::from(aabb_max))
                }
                Shape::SteelProfile { params, length, .. } => {
                    (pos + glam::Vec3::new(-params.b / 2.0, 0.0, -params.h / 2.0),
                     pos + glam::Vec3::new(params.b / 2.0, *length, params.h / 2.0))
                }
            };
            if let Some(t) = camera::ray_aabb(origin, dir, pick_min, pick_max) {
                if best.as_ref().map_or(true, |(bt,_)| t < *bt) { best = Some((t, obj.id.clone())); }
            }
        }
        best.map(|(_, id)| id)
    }

    /// B5: After push/pull, move adjacent objects that were touching the pulled face
    /// to maintain contact. Only moves (does not resize) adjacent boxes.
    pub(crate) fn adjust_adjacent_after_pull(scene: &mut crate::scene::Scene, pulled_id: &str, face: PullFace, delta: f32) {
        let pulled = match scene.objects.get(pulled_id).cloned() {
            Some(o) => o,
            None => return,
        };
        let (pw, ph, pd) = match &pulled.shape {
            Shape::Box { width, height, depth } => (*width, *height, *depth),
            _ => return,
        };
        let pp = pulled.position;
        let tol = 5.0; // 5mm tolerance for face adjacency

        let ids: Vec<String> = scene.objects.keys().cloned().collect();
        for id in &ids {
            if id == pulled_id { continue; }
            if let Some(other) = scene.objects.get_mut(id) {
                if let Shape::Box { .. } = &other.shape {
                    match face {
                        PullFace::Right => {
                            // Right face of pulled obj grew rightward; push adjacent objects right
                            // Before pull, right face was at pp[0] + pw - delta
                            if (pp[0] + pw - delta - other.position[0]).abs() < tol {
                                other.position[0] += delta;
                            }
                        }
                        PullFace::Left => {
                            // Left face moved leftward (position decreased)
                            // Before pull, left face was at pp[0] + delta (since position moved by -delta)
                            // Check objects whose right face touched original left face
                            if let Shape::Box { width: ow, .. } = &other.shape {
                                if (other.position[0] + ow - (pp[0] - delta)).abs() < tol {
                                    other.position[0] += delta; // delta is negative for left pull
                                }
                            }
                        }
                        PullFace::Top => {
                            if (pp[1] + ph - delta - other.position[1]).abs() < tol {
                                other.position[1] += delta;
                            }
                        }
                        PullFace::Bottom => {
                            if let Shape::Box { height: oh, .. } = &other.shape {
                                if (other.position[1] + oh - (pp[1] - delta)).abs() < tol {
                                    other.position[1] += delta;
                                }
                            }
                        }
                        PullFace::Back => {
                            if (pp[2] + pd - delta - other.position[2]).abs() < tol {
                                other.position[2] += delta;
                            }
                        }
                        PullFace::Front => {
                            if let Shape::Box { depth: od, .. } = &other.shape {
                                if (other.position[2] + od - (pp[2] - delta)).abs() < tol {
                                    other.position[2] += delta;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Pick which face of which object the ray hits (for Push/Pull)
    pub(crate) fn pick_face(&self, mx: f32, my: f32, vw: f32, vh: f32) -> Option<(String, PullFace)> {
        let (origin, dir) = self.viewer.camera.screen_ray(mx, my, vw, vh);
        let mut best: Option<(f32, String, PullFace)> = None;

        for obj in self.scene.objects.values() {
            let p = glam::Vec3::from(obj.position);
            let (faces, obj_max): (Vec<(PullFace, glam::Vec3, glam::Vec3)>, _) = match &obj.shape {
                Shape::Box { width, height, depth } => {
                    let max = p + glam::Vec3::new(*width, *height, *depth);
                    let faces = vec![
                        (PullFace::Top,    glam::Vec3::new(p.x, max.y, p.z), glam::Vec3::new(max.x, max.y, max.z)),
                        (PullFace::Bottom, glam::Vec3::new(p.x, p.y, p.z),   glam::Vec3::new(max.x, p.y, max.z)),
                        (PullFace::Right,  glam::Vec3::new(max.x, p.y, p.z), glam::Vec3::new(max.x, max.y, max.z)),
                        (PullFace::Left,   glam::Vec3::new(p.x, p.y, p.z),   glam::Vec3::new(p.x, max.y, max.z)),
                        (PullFace::Back,   glam::Vec3::new(p.x, p.y, max.z), glam::Vec3::new(max.x, max.y, max.z)),
                        (PullFace::Front,  glam::Vec3::new(p.x, p.y, p.z),   glam::Vec3::new(max.x, max.y, p.z)),
                    ];
                    (faces, max)
                }
                Shape::Cylinder { radius, height, .. } => {
                    let max = p + glam::Vec3::new(*radius * 2.0, *height, *radius * 2.0);
                    let faces = vec![
                        (PullFace::Top,    glam::Vec3::new(p.x, max.y, p.z), max),
                        (PullFace::Bottom, p, glam::Vec3::new(max.x, p.y, max.z)),
                    ];
                    (faces, max)
                }
                Shape::SteelProfile { params, length, .. } => {
                    let p = p + glam::Vec3::new(-params.b / 2.0, 0.0, -params.h / 2.0);
                    let max = p + glam::Vec3::new(params.b, *length, params.h);
                    let faces = vec![
                        (PullFace::Top,    glam::Vec3::new(p.x, max.y, p.z), glam::Vec3::new(max.x, max.y, max.z)),
                        (PullFace::Bottom, glam::Vec3::new(p.x, p.y, p.z),   glam::Vec3::new(max.x, p.y, max.z)),
                        (PullFace::Right,  glam::Vec3::new(max.x, p.y, p.z), glam::Vec3::new(max.x, max.y, max.z)),
                        (PullFace::Left,   glam::Vec3::new(p.x, p.y, p.z),   glam::Vec3::new(p.x, max.y, max.z)),
                        (PullFace::Back,   glam::Vec3::new(p.x, p.y, max.z), glam::Vec3::new(max.x, max.y, max.z)),
                        (PullFace::Front,  glam::Vec3::new(p.x, p.y, p.z),   glam::Vec3::new(max.x, max.y, p.z)),
                    ];
                    (faces, max)
                }
                _ => continue,
            };

            // Test each face quad as a plane
            for (face, fmin, fmax) in &faces {
                let normal = match face {
                    PullFace::Top    => glam::Vec3::Y,
                    PullFace::Bottom => glam::Vec3::NEG_Y,
                    PullFace::Right  => glam::Vec3::X,
                    PullFace::Left   => glam::Vec3::NEG_X,
                    PullFace::Back   => glam::Vec3::Z,
                    PullFace::Front  => glam::Vec3::NEG_Z,
                };
                let denom = dir.dot(normal);
                if denom.abs() < 1e-6 { continue; } // parallel

                // fmin is a known point on the face plane — correct for all orientations
                let t = (*fmin - origin).dot(normal) / denom;
                if t < 0.0 { continue; }

                let hit = origin + dir * t;

                // Check if hit is within face bounds
                let in_bounds = hit.x >= fmin.x - 1.0 && hit.x <= fmax.x + 1.0
                    && hit.y >= fmin.y - 1.0 && hit.y <= fmax.y + 1.0
                    && hit.z >= fmin.z - 1.0 && hit.z <= fmax.z + 1.0;
                if !in_bounds { continue; }

                if best.as_ref().map_or(true, |(bt, _, _)| t < *bt) {
                    best = Some((t, obj.id.clone(), *face));
                }
            }
            let _ = obj_max;
        }

        best.map(|(_, id, face)| (id, face))
    }

    /// Pick a face on the shared free mesh by ray-casting.
    pub(crate) fn pick_free_mesh_face(&self, mx: f32, my: f32, vw: f32, vh: f32) -> Option<u32> {
        let (origin, dir) = self.viewer.camera.screen_ray(mx, my, vw, vh);
        let mut best: Option<(f32, u32)> = None;

        for (&fid, face) in &self.scene.free_mesh.faces {
            let verts = self.scene.free_mesh.face_vertices(fid);
            if verts.len() < 3 { continue; }

            let normal = glam::Vec3::from(face.normal);
            let v0 = glam::Vec3::from(verts[0]);

            // Ray-plane intersection
            let denom = dir.dot(normal);
            if denom.abs() < 1e-6 { continue; }
            let t = (v0 - origin).dot(normal) / denom;
            if t < 0.0 { continue; }

            let hit = origin + dir * t;

            // Point-in-polygon test using winding number on the projected plane
            let mut inside = false;
            let n = verts.len();
            // Use a simple crossing-number (ray-casting) test projected onto the
            // dominant plane (drop the axis with the largest normal component).
            let (ax1, ax2) = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
                (1usize, 2usize) // drop X, use Y/Z
            } else if normal.y.abs() >= normal.z.abs() {
                (0, 2) // drop Y, use X/Z
            } else {
                (0, 1) // drop Z, use X/Y
            };
            let hx = [hit.x, hit.y, hit.z][ax1];
            let hy = [hit.x, hit.y, hit.z][ax2];
            let mut j = n - 1;
            for i in 0..n {
                let yi = verts[i][ax2]; let yj = verts[j][ax2];
                let xi = verts[i][ax1]; let xj = verts[j][ax1];
                if ((yi > hy) != (yj > hy))
                    && (hx < (xj - xi) * (hy - yi) / (yj - yi) + xi)
                {
                    inside = !inside;
                }
                j = i;
            }
            if !inside { continue; }

            if best.as_ref().map_or(true, |(bt, _)| t < *bt) {
                best = Some((t, fid));
            }
        }

        best.map(|(_, fid)| fid)
    }

}

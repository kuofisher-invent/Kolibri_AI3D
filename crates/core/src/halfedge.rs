//! Half-edge mesh data structure for free-form modeling
//! Supports: vertices, edges, faces with topology queries

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

pub type VId = u32;
pub type EId = u32;
pub type FId = u32;

#[derive(Debug, Clone)]
pub struct HeMesh {
    pub vertices: HashMap<VId, HeVertex>,
    pub edges: HashMap<EId, HeEdge>,
    pub faces: HashMap<FId, HeFace>,
    next_vid: VId,
    next_eid: EId,
    next_fid: FId,
    /// SDK 原始邊線（匯入時填入，優先用於渲染）
    pub sdk_edge_segments: Vec<([f32; 3], [f32; 3])>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeVertex {
    pub pos: [f32; 3],
    pub edge: Option<EId>,  // one outgoing half-edge
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeEdge {
    pub origin: VId,        // vertex this edge starts from
    pub twin: Option<EId>,  // opposite half-edge
    pub next: Option<EId>,  // next edge in face loop
    pub prev: Option<EId>,  // previous edge in face loop
    pub face: Option<FId>,  // face to the left (None = boundary)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeFace {
    pub edge: EId,          // one half-edge on boundary
    pub normal: [f32; 3],   // face normal
    /// 直接頂點索引（快速匯入用，跳過 edge topology）
    #[serde(default)]
    pub vert_ids: Option<Vec<VId>>,
    #[serde(default)]
    pub source_face_label: Option<String>,
}

impl Default for HeMesh {
    fn default() -> Self { Self::new() }
}

#[derive(Serialize, Deserialize)]
struct HeMeshSerde {
    vertices: Vec<(VId, HeVertex)>,
    edges: Vec<(EId, HeEdge)>,
    faces: Vec<(FId, HeFace)>,
    next_vid: VId,
    next_eid: EId,
    next_fid: FId,
    #[serde(default)]
    sdk_edge_segments: Vec<([f32; 3], [f32; 3])>,
}

impl Serialize for HeMesh {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut vertices: Vec<_> = self.vertices.iter().map(|(&id, vertex)| (id, vertex.clone())).collect();
        let mut edges: Vec<_> = self.edges.iter().map(|(&id, edge)| (id, edge.clone())).collect();
        let mut faces: Vec<_> = self.faces.iter().map(|(&id, face)| (id, face.clone())).collect();
        vertices.sort_by_key(|(id, _)| *id);
        edges.sort_by_key(|(id, _)| *id);
        faces.sort_by_key(|(id, _)| *id);

        HeMeshSerde {
            vertices,
            edges,
            faces,
            next_vid: self.next_vid,
            next_eid: self.next_eid,
            next_fid: self.next_fid,
            sdk_edge_segments: self.sdk_edge_segments.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for HeMesh {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = HeMeshSerde::deserialize(deserializer)?;
        Ok(Self {
            vertices: helper.vertices.into_iter().collect(),
            edges: helper.edges.into_iter().collect(),
            faces: helper.faces.into_iter().collect(),
            next_vid: helper.next_vid,
            next_eid: helper.next_eid,
            next_fid: helper.next_fid,
            sdk_edge_segments: helper.sdk_edge_segments,
        })
    }
}

impl HeMesh {
    pub fn new() -> Self {
        Self {
            vertices: HashMap::new(),
            edges: HashMap::new(),
            faces: HashMap::new(),
            next_vid: 1,
            next_eid: 1,
            next_fid: 1,
            sdk_edge_segments: Vec::new(),
        }
    }

    /// Get next face ID and increment counter (for direct face insertion)
    pub fn next_fid(&mut self) -> FId {
        let id = self.next_fid;
        self.next_fid += 1;
        id
    }

    /// Add a vertex, return its ID
    pub fn add_vertex(&mut self, pos: [f32; 3]) -> VId {
        let id = self.next_vid;
        self.next_vid += 1;
        self.vertices.insert(id, HeVertex { pos, edge: None });
        id
    }

    /// Find vertex near position (within tolerance)
    pub fn find_vertex_near(&self, pos: [f32; 3], tol: f32) -> Option<VId> {
        for (&id, v) in &self.vertices {
            let dx = v.pos[0] - pos[0];
            let dy = v.pos[1] - pos[1];
            let dz = v.pos[2] - pos[2];
            if dx*dx + dy*dy + dz*dz < tol*tol {
                return Some(id);
            }
        }
        None
    }

    /// Add an edge between two vertices. Creates a pair of half-edges.
    /// Returns (edge_id, twin_id)
    pub fn add_edge_between(&mut self, v1: VId, v2: VId) -> (EId, EId) {
        // Check if edge already exists
        if let Some(eid) = self.find_edge(v1, v2) {
            let twin = self.edges[&eid].twin.unwrap_or(eid);
            return (eid, twin);
        }

        let e1 = self.next_eid; self.next_eid += 1;
        let e2 = self.next_eid; self.next_eid += 1;

        self.edges.insert(e1, HeEdge {
            origin: v1, twin: Some(e2), next: None, prev: None, face: None,
        });
        self.edges.insert(e2, HeEdge {
            origin: v2, twin: Some(e1), next: None, prev: None, face: None,
        });

        // Set vertex outgoing edges
        if let Some(v) = self.vertices.get_mut(&v1) {
            if v.edge.is_none() { v.edge = Some(e1); }
        }
        if let Some(v) = self.vertices.get_mut(&v2) {
            if v.edge.is_none() { v.edge = Some(e2); }
        }

        (e1, e2)
    }

    /// Add a face from a list of vertex IDs (used for mesh import).
    /// Creates edges between consecutive vertices and assigns them to a face.
    pub fn add_face(&mut self, vertex_ids: &[u32]) {
        self.add_face_with_source(vertex_ids, None);
    }

    pub fn add_face_with_source(&mut self, vertex_ids: &[u32], source_face_label: Option<String>) {
        if vertex_ids.len() < 3 { return; }

        // 計算面法線
        let p0 = self.vertices.get(&vertex_ids[0]).map(|v| v.pos).unwrap_or([0.0; 3]);
        let p1 = self.vertices.get(&vertex_ids[1]).map(|v| v.pos).unwrap_or([0.0; 3]);
        let p2 = self.vertices.get(&vertex_ids[2]).map(|v| v.pos).unwrap_or([0.0; 3]);
        let a = [p1[0]-p0[0], p1[1]-p0[1], p1[2]-p0[2]];
        let b = [p2[0]-p0[0], p2[1]-p0[1], p2[2]-p0[2]];
        let nx = a[1]*b[2] - a[2]*b[1];
        let ny = a[2]*b[0] - a[0]*b[2];
        let nz = a[0]*b[1] - a[1]*b[0];
        let len = (nx*nx + ny*ny + nz*nz).sqrt().max(1e-10);
        let normal = [nx/len, ny/len, nz/len];

        // 建立面
        let fid = self.next_fid;
        self.next_fid += 1;

        // 為每對連續頂點建立 half-edge
        let n = vertex_ids.len();
        let mut edge_ids: Vec<EId> = Vec::with_capacity(n);
        for i in 0..n {
            let v1 = vertex_ids[i];
            let v2 = vertex_ids[(i + 1) % n];
            // 找既有 edge 或建立新的
            let eid = if let Some(e) = self.find_edge(v1, v2) {
                e
            } else {
                let (e, _) = self.add_edge_between(v1, v2);
                e
            };
            edge_ids.push(eid);
        }

        // 設定 face 和 next/prev 鏈結
        for i in 0..edge_ids.len() {
            let eid = edge_ids[i];
            let next_eid = edge_ids[(i + 1) % edge_ids.len()];
            let prev_eid = edge_ids[(i + edge_ids.len() - 1) % edge_ids.len()];
            if let Some(e) = self.edges.get_mut(&eid) {
                e.face = Some(fid);
                e.next = Some(next_eid);
                e.prev = Some(prev_eid);
            }
        }

        self.faces.insert(fid, HeFace {
            edge: edge_ids[0],
            normal,
            vert_ids: Some(vertex_ids.to_vec()),
            source_face_label,
        });
    }

    /// Find half-edge from v1 to v2
    pub fn find_edge(&self, v1: VId, v2: VId) -> Option<EId> {
        for (&id, e) in &self.edges {
            if e.origin == v1 {
                if let Some(twin) = e.twin {
                    if self.edges.get(&twin).map(|t| t.origin) == Some(v2) {
                        return Some(id);
                    }
                }
            }
        }
        None
    }

    /// Try to detect and create faces from closed edge loops.
    /// Called after adding edges. Uses a proper boundary-walk algorithm:
    /// at each vertex with multiple outgoing boundary edges, pick the
    /// rightmost turn (smallest signed angle) to trace the tightest loop.
    pub fn detect_faces(&mut self) {
        // Step 1: Build adjacency map: vertex -> list of outgoing boundary half-edges
        let mut outgoing: std::collections::HashMap<VId, Vec<EId>> = std::collections::HashMap::new();
        for (&eid, edge) in &self.edges {
            if edge.face.is_none() {
                outgoing.entry(edge.origin).or_default().push(eid);
            }
        }

        // Step 2: Walk loops using the adjacency map with rightmost-turn heuristic
        let mut used: std::collections::HashSet<EId> = std::collections::HashSet::new();

        // Collect start candidates up front to avoid borrow issues
        let start_candidates: Vec<EId> = outgoing.values().flatten().copied().collect();

        for start_eid in start_candidates {
            if used.contains(&start_eid) { continue; }
            if self.edges.get(&start_eid).map(|e| e.face.is_some()).unwrap_or(true) { continue; }

            let mut loop_edges: Vec<EId> = Vec::new();
            let mut current = start_eid;
            let mut visited_edges: std::collections::HashSet<EId> = std::collections::HashSet::new();

            let found_loop = loop {
                if visited_edges.contains(&current) {
                    // Check if we completed the loop back to start
                    break current == start_eid && loop_edges.len() >= 3;
                }
                visited_edges.insert(current);
                loop_edges.push(current);

                // Get the endpoint of current edge (= origin of twin)
                let end_vertex = match self.edges.get(&current)
                    .and_then(|e| e.twin)
                    .and_then(|t| self.edges.get(&t))
                    .map(|t| t.origin)
                {
                    Some(v) => v,
                    None => break false,
                };

                // Find all boundary edges leaving end_vertex (not yet used)
                let candidates: Vec<EId> = outgoing.get(&end_vertex)
                    .map(|v| v.iter()
                        .filter(|&&eid| {
                            !used.contains(&eid)
                            && self.edges.get(&eid)
                                .map(|e| e.face.is_none())
                                .unwrap_or(false)
                        })
                        .copied()
                        .collect())
                    .unwrap_or_default();

                if candidates.is_empty() {
                    break false;
                }

                if candidates.len() == 1 {
                    current = candidates[0];
                } else {
                    // Multiple choices: pick the rightmost turn (smallest signed angle)
                    // This traces the tightest/smallest face
                    let prev_dir = self.edge_direction(current);
                    let mut best = candidates[0];
                    let mut best_angle = f32::MAX;

                    for &cand in &candidates {
                        let cand_dir = self.edge_direction(cand);
                        // Signed angle from reversed-incoming (-prev_dir) to outgoing (cand_dir)
                        let cross = -prev_dir[0] * cand_dir[1] + prev_dir[1] * cand_dir[0];
                        let dot = -prev_dir[0] * cand_dir[0] - prev_dir[1] * cand_dir[1];
                        let angle = cross.atan2(dot);
                        if angle < best_angle {
                            best_angle = angle;
                            best = cand;
                        }
                    }
                    current = best;
                }

                // Safety limit
                if loop_edges.len() > 50 {
                    break false;
                }
            };

            // Create face from the loop
            if found_loop {
                self.create_face_from_loop(&loop_edges);
                for &e in &loop_edges {
                    used.insert(e);
                }
            }
        }
    }

    /// Compute the 2D direction (XZ plane) of a half-edge
    fn edge_direction(&self, eid: EId) -> [f32; 2] {
        let e = match self.edges.get(&eid) { Some(e) => e, None => return [1.0, 0.0] };
        let p1 = match self.vertices.get(&e.origin) { Some(v) => v.pos, None => return [1.0, 0.0] };
        let p2 = match e.twin.and_then(|t| self.edges.get(&t)).and_then(|t| self.vertices.get(&t.origin)) {
            Some(v) => v.pos, None => return [1.0, 0.0]
        };
        let dx = p2[0] - p1[0];
        let dz = p2[2] - p1[2];
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1e-6 { return [1.0, 0.0]; }
        [dx / len, dz / len]
    }

    fn create_face_from_loop(&mut self, loop_edges: &[EId]) {
        let fid = self.next_fid;
        self.next_fid += 1;

        // Set next/prev/face for each edge in the loop
        let n = loop_edges.len();
        for i in 0..n {
            let eid = loop_edges[i];
            let next = loop_edges[(i + 1) % n];
            let prev = loop_edges[(i + n - 1) % n];
            if let Some(e) = self.edges.get_mut(&eid) {
                e.next = Some(next);
                e.prev = Some(prev);
                e.face = Some(fid);
            }
        }

        // Compute face normal from first 3 vertices
        let normal = self.compute_face_normal(loop_edges);

        self.faces.insert(fid, HeFace {
            edge: loop_edges[0],
            normal,
            vert_ids: None,
            source_face_label: None,
        });
    }

    fn compute_face_normal(&self, loop_edges: &[EId]) -> [f32; 3] {
        if loop_edges.len() < 3 { return [0.0, 1.0, 0.0]; }

        let get_pos = |eid: EId| -> [f32; 3] {
            let vid = self.edges[&eid].origin;
            self.vertices[&vid].pos
        };

        let p0 = get_pos(loop_edges[0]);
        let p1 = get_pos(loop_edges[1]);
        let p2 = get_pos(loop_edges[2]);

        let u = [p1[0]-p0[0], p1[1]-p0[1], p1[2]-p0[2]];
        let v = [p2[0]-p0[0], p2[1]-p0[1], p2[2]-p0[2]];
        let nx = u[1]*v[2] - u[2]*v[1];
        let ny = u[2]*v[0] - u[0]*v[2];
        let nz = u[0]*v[1] - u[1]*v[0];
        let len = (nx*nx + ny*ny + nz*nz).sqrt();
        if len < 1e-6 { return [0.0, 1.0, 0.0]; }
        [nx/len, ny/len, nz/len]
    }

    /// Get all face vertex positions (for rendering)
    pub fn face_vertices(&self, fid: FId) -> Vec<[f32; 3]> {
        let face = match self.faces.get(&fid) {
            Some(f) => f,
            None => return vec![],
        };
        // 快速路徑：直接頂點索引（無 edge topology 的匯入 mesh）
        if let Some(ref ids) = face.vert_ids {
            return ids.iter()
                .filter_map(|vid| self.vertices.get(vid).map(|v| v.pos))
                .collect();
        }
        // 標準路徑：走 edge loop
        let mut verts = Vec::new();
        let start = face.edge;
        let mut current = start;
        for _ in 0..100 {
            if let Some(e) = self.edges.get(&current) {
                if let Some(v) = self.vertices.get(&e.origin) {
                    verts.push(v.pos);
                }
                match e.next {
                    Some(next) if next != start => current = next,
                    _ => break,
                }
            } else {
                break;
            }
        }
        verts
    }

    /// Get all edge segments for rendering
    pub fn all_edge_segments(&self) -> Vec<([f32; 3], [f32; 3])> {
        // 優先使用 SDK 原始邊線（無三角化產物）
        if !self.sdk_edge_segments.is_empty() {
            return self.sdk_edge_segments.clone();
        }
        // 如果有半邊拓撲，用原來的方式
        if !self.edges.is_empty() {
            let mut segments = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for (&eid, edge) in &self.edges {
                if seen.contains(&eid) { continue; }
                if let Some(twin) = edge.twin { seen.insert(twin); }
                seen.insert(eid);
                let p1 = self.vertices.get(&edge.origin).map(|v| v.pos).unwrap_or([0.0; 3]);
                let p2 = edge.twin
                    .and_then(|t| self.edges.get(&t))
                    .and_then(|t| self.vertices.get(&t.origin))
                    .map(|v| v.pos)
                    .unwrap_or([0.0; 3]);
                segments.push((p1, p2));
            }
            return segments;
        }
        // 快速路徑：從 face.vert_ids 生成邊
        // 記錄每條邊的相鄰面法線，共面邊（三角化對角線）不顯示
        let mut edge_faces: std::collections::HashMap<(u32, u32), Vec<[f32; 3]>> = std::collections::HashMap::new();
        for face in self.faces.values() {
            if let Some(ref ids) = face.vert_ids {
                let n = ids.len();
                for i in 0..n {
                    let a = ids[i];
                    let b = ids[(i + 1) % n];
                    let key = if a < b { (a, b) } else { (b, a) };
                    edge_faces.entry(key).or_default().push(face.normal);
                }
            }
        }
        let mut segments = Vec::new();
        for ((a, b), normals) in &edge_faces {
            // 邊界邊（只有 1 個面）→ 一定顯示
            // 內部邊（2 個面）→ 法線夾角 > 閾值才顯示（非共面）
            let show = if normals.len() == 1 {
                true
            } else if normals.len() >= 2 {
                let n0 = &normals[0];
                let n1 = &normals[1];
                let dot = n0[0]*n1[0] + n0[1]*n1[1] + n0[2]*n1[2];
                dot.abs() < 0.9998 // 法線夾角 > ~1° → 顯示（過濾三角化邊）
            } else {
                true
            };
            if show {
                let p1 = self.vertices.get(a).map(|v| v.pos).unwrap_or([0.0; 3]);
                let p2 = self.vertices.get(b).map(|v| v.pos).unwrap_or([0.0; 3]);
                segments.push((p1, p2));
            }
        }
        segments
    }

    /// Get number of edges (unique, not counting twins)
    pub fn edge_count(&self) -> usize {
        self.edges.len() / 2
    }

    /// Push/Pull a face along its normal by a distance.
    /// Creates new faces for the extruded sides.
    pub fn push_pull_face(&mut self, fid: FId, distance: f32) {
        let face = match self.faces.get(&fid) {
            Some(f) => f.clone(),
            None => return,
        };

        let normal = face.normal;
        let offset = [normal[0] * distance, normal[1] * distance, normal[2] * distance];

        // Get face vertices
        let face_verts = self.face_vertices(fid);
        if face_verts.len() < 3 { return; }

        // Create new vertices (offset copies)
        let mut new_vids = Vec::new();
        for p in &face_verts {
            let new_pos = [p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]];
            let vid = self.add_vertex(new_pos);
            new_vids.push(vid);
        }

        // Get original vertex IDs
        let mut orig_vids = Vec::new();
        let start = face.edge;
        let mut current = start;
        for _ in 0..100 {
            if let Some(e) = self.edges.get(&current) {
                orig_vids.push(e.origin);
                match e.next {
                    Some(next) if next != start => current = next,
                    _ => break,
                }
            } else { break; }
        }

        // Create side faces (quads connecting old and new vertices)
        let n = orig_vids.len().min(new_vids.len());
        for i in 0..n {
            let j = (i + 1) % n;
            // Side quad: orig[i] -> orig[j] -> new[j] -> new[i]
            self.add_edge_between(orig_vids[i], orig_vids[j]);
            self.add_edge_between(orig_vids[j], new_vids[j]);
            self.add_edge_between(new_vids[j], new_vids[i]);
            self.add_edge_between(new_vids[i], orig_vids[i]);
        }

        // Create top face edges
        for i in 0..n {
            let j = (i + 1) % n;
            self.add_edge_between(new_vids[i], new_vids[j]);
        }

        // Detect all new faces
        self.detect_faces();
    }

    /// Create a box as a HeMesh
    pub fn from_box(pos: [f32; 3], w: f32, h: f32, d: f32) -> Self {
        let mut mesh = Self::new();
        let p = pos;

        // 8 vertices
        let v = [
            mesh.add_vertex([p[0],     p[1],     p[2]]),     // 0: FBL
            mesh.add_vertex([p[0]+w,   p[1],     p[2]]),     // 1: FBR
            mesh.add_vertex([p[0]+w,   p[1]+h,   p[2]]),     // 2: FTR
            mesh.add_vertex([p[0],     p[1]+h,   p[2]]),     // 3: FTL
            mesh.add_vertex([p[0],     p[1],     p[2]+d]),   // 4: BBL
            mesh.add_vertex([p[0]+w,   p[1],     p[2]+d]),   // 5: BBR
            mesh.add_vertex([p[0]+w,   p[1]+h,   p[2]+d]),   // 6: BTR
            mesh.add_vertex([p[0],     p[1]+h,   p[2]+d]),   // 7: BTL
        ];

        // 12 edges (unique, creates 24 half-edges)
        // Front face
        mesh.add_edge_between(v[0], v[1]);
        mesh.add_edge_between(v[1], v[2]);
        mesh.add_edge_between(v[2], v[3]);
        mesh.add_edge_between(v[3], v[0]);
        // Back face
        mesh.add_edge_between(v[5], v[4]);
        mesh.add_edge_between(v[4], v[7]);
        mesh.add_edge_between(v[7], v[6]);
        mesh.add_edge_between(v[6], v[5]);
        // Connecting edges
        mesh.add_edge_between(v[0], v[4]);
        mesh.add_edge_between(v[1], v[5]);
        mesh.add_edge_between(v[2], v[6]);
        mesh.add_edge_between(v[3], v[7]);

        // Detect faces
        mesh.detect_faces();

        mesh
    }

    /// Compute axis-aligned bounding box: (min, max)
    pub fn aabb(&self) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for v in self.vertices.values() {
            for i in 0..3 {
                min[i] = min[i].min(v.pos[i]);
                max[i] = max[i].max(v.pos[i]);
            }
        }
        if min[0] > max[0] {
            // No vertices
            return ([0.0; 3], [0.0; 3]);
        }
        (min, max)
    }
}

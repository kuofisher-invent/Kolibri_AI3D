//! Half-edge mesh data structure for free-form modeling
//! Supports: vertices, edges, faces with topology queries

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

pub type VId = u32;
pub type EId = u32;
pub type FId = u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeMesh {
    pub vertices: HashMap<VId, HeVertex>,
    pub edges: HashMap<EId, HeEdge>,
    pub faces: HashMap<FId, HeFace>,
    next_vid: VId,
    next_eid: EId,
    next_fid: FId,
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
}

impl Default for HeMesh {
    fn default() -> Self { Self::new() }
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
        }
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

//! GeometryKernel trait — 幾何核心抽象介面
//! 目前由 HeMesh 實作，未來可替換為 Truck (NURBS/B-Rep)

/// 幾何核心的統一介面
pub trait GeometryKernel: std::fmt::Debug + Clone {
    /// 新增頂點，回傳 ID
    fn add_vertex(&mut self, pos: [f32; 3]) -> u32;

    /// 新增面（頂點 ID 列表），回傳面 ID
    fn add_face(&mut self, vertex_ids: &[u32]) -> Option<u32>;

    /// 新增邊（兩頂點 ID），回傳邊 ID pair
    fn add_edge(&mut self, v1: u32, v2: u32) -> (u32, u32);

    /// 頂點數量
    fn vertex_count(&self) -> usize;

    /// 面數量
    fn face_count(&self) -> usize;

    /// 邊數量
    fn edge_count(&self) -> usize;

    /// 取得頂點座標
    fn vertex_position(&self, vid: u32) -> Option<[f32; 3]>;

    /// 取得面的頂點座標列表
    fn face_vertices(&self, fid: u32) -> Vec<[f32; 3]>;

    /// 取得面法線
    fn face_normal(&self, fid: u32) -> Option<[f32; 3]>;

    /// 計算 AABB
    fn aabb(&self) -> ([f32; 3], [f32; 3]);

    /// 偵測封閉迴圈自動生成面
    fn detect_faces(&mut self);

    /// Push/Pull 面
    fn push_pull_face(&mut self, face_id: u32, distance: f32);

    /// 所有邊的線段（用於渲染）— 回傳 slice 引用避免 clone
    fn all_edge_segments_vec(&self) -> Vec<([f32; 3], [f32; 3])>;
}

/// 為 HeMesh 實作 GeometryKernel
impl GeometryKernel for crate::halfedge::HeMesh {
    fn add_vertex(&mut self, pos: [f32; 3]) -> u32 {
        self.add_vertex(pos)
    }

    fn add_face(&mut self, vertex_ids: &[u32]) -> Option<u32> {
        let before = self.faces.len();
        self.add_face(vertex_ids);
        if self.faces.len() > before {
            self.faces.keys().max().copied()
        } else {
            None
        }
    }

    fn add_edge(&mut self, v1: u32, v2: u32) -> (u32, u32) {
        self.add_edge_between(v1, v2)
    }

    fn vertex_count(&self) -> usize { self.vertices.len() }
    fn face_count(&self) -> usize { self.faces.len() }
    fn edge_count(&self) -> usize { self.edges.len() }

    fn vertex_position(&self, vid: u32) -> Option<[f32; 3]> {
        self.vertices.get(&vid).map(|v| v.pos)
    }

    fn face_vertices(&self, fid: u32) -> Vec<[f32; 3]> {
        self.face_vertices(fid)
    }

    fn face_normal(&self, fid: u32) -> Option<[f32; 3]> {
        self.faces.get(&fid).map(|f| f.normal)
    }

    fn aabb(&self) -> ([f32; 3], [f32; 3]) {
        self.aabb()
    }

    fn detect_faces(&mut self) {
        self.detect_faces()
    }

    fn push_pull_face(&mut self, face_id: u32, distance: f32) {
        self.push_pull_face(face_id, distance)
    }

    fn all_edge_segments_vec(&self) -> Vec<([f32; 3], [f32; 3])> {
        self.all_edge_segments().to_vec()
    }
}

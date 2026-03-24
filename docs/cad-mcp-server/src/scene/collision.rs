// ============================================================
//  collision.rs — 碰撞偵測 + 重量計算
// ============================================================

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use super::geometry::{CadObject, Shape};

// ─── AABB ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Aabb {
    pub fn from_object(obj: &CadObject) -> Self {
        let (lmin, lmax) = obj.shape.bounding_box();
        Self {
            min: [obj.position[0]+lmin[0], obj.position[1]+lmin[1], obj.position[2]+lmin[2]],
            max: [obj.position[0]+lmax[0], obj.position[1]+lmax[1], obj.position[2]+lmax[2]],
        }
    }

    pub fn overlaps(&self, o: &Aabb) -> bool {
        self.min[0] <= o.max[0] && self.max[0] >= o.min[0] &&
        self.min[1] <= o.max[1] && self.max[1] >= o.min[1] &&
        self.min[2] <= o.max[2] && self.max[2] >= o.min[2]
    }

    pub fn expanded(&self, m: f64) -> Self {
        Self {
            min: [self.min[0]-m, self.min[1]-m, self.min[2]-m],
            max: [self.max[0]+m, self.max[1]+m, self.max[2]+m],
        }
    }

    pub fn volume(&self) -> f64 {
        (self.max[0]-self.min[0])*(self.max[1]-self.min[1])*(self.max[2]-self.min[2])
    }

    /// 計算兩個 AABB 之間的最短距離（重疊時回傳 0）
    pub fn distance_to(&self, other: &Aabb) -> f64 {
        let mut dist_sq = 0.0f64;
        for i in 0..3 {
            let gap = (other.min[i] - self.max[i]).max(self.min[i] - other.max[i]).max(0.0);
            dist_sq += gap * gap;
        }
        dist_sq.sqrt()
    }
}

// ─── 碰撞結果 ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollisionPair {
    pub obj_a:      String,
    pub obj_b:      String,
    pub overlap_mm: [f64; 3],
    pub severity:   CollisionSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CollisionSeverity {
    Touch,      // < 1mm
    Overlap,    // 明顯重疊
    Contained,  // 完全包含
}

// ─── 碰撞世界 ─────────────────────────────────────────────────

#[derive(Default)]
pub struct CollisionWorld {
    aabbs: HashMap<String, Aabb>,
}

impl CollisionWorld {
    pub fn new() -> Self { Self::default() }

    pub fn update_object(&mut self, obj: &CadObject) {
        self.aabbs.insert(obj.id.0.clone(), Aabb::from_object(obj));
    }

    pub fn remove_object(&mut self, id: &str) {
        self.aabbs.remove(id);
    }

    /// 寬域過濾：找出可能碰撞的候選清單
    pub fn broad_phase(&self, target_id: &str) -> Vec<String> {
        let Some(aabb) = self.aabbs.get(target_id) else { return vec![] };
        let expanded = aabb.expanded(0.01);
        self.aabbs.iter()
            .filter(|(id, _)| id.as_str() != target_id)
            .filter(|(_, a)| a.overlaps(&expanded))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 精確碰撞計算
    pub fn narrow_phase(&self, a_id: &str, b_id: &str) -> Option<CollisionPair> {
        let a = self.aabbs.get(a_id)?;
        let b = self.aabbs.get(b_id)?;
        if !a.overlaps(b) { return None; }

        let ox = (a.max[0].min(b.max[0]) - a.min[0].max(b.min[0])).max(0.0);
        let oy = (a.max[1].min(b.max[1]) - a.min[1].max(b.min[1])).max(0.0);
        let oz = (a.max[2].min(b.max[2]) - a.min[2].max(b.min[2])).max(0.0);

        let contained = b.min[0]>=a.min[0] && b.max[0]<=a.max[0] &&
                        b.min[1]>=a.min[1] && b.max[1]<=a.max[1] &&
                        b.min[2]>=a.min[2] && b.max[2]<=a.max[2];

        let severity = if contained { CollisionSeverity::Contained }
                       else if ox.max(oy).max(oz) < 1.0 { CollisionSeverity::Touch }
                       else { CollisionSeverity::Overlap };

        Some(CollisionPair {
            obj_a: a_id.into(), obj_b: b_id.into(),
            overlap_mm: [ox, oy, oz], severity,
        })
    }

    /// 掃描整個場景的所有碰撞
    pub fn check_all(&self) -> Vec<CollisionPair> {
        let ids: Vec<&String> = self.aabbs.keys().collect();
        let mut results = Vec::new();
        for i in 0..ids.len() {
            for j in (i+1)..ids.len() {
                if let Some(p) = self.narrow_phase(ids[i], ids[j]) {
                    results.push(p);
                }
            }
        }
        results
    }

    /// 單一物件與場景的碰撞
    pub fn check_object(&self, obj_id: &str) -> Vec<CollisionPair> {
        self.broad_phase(obj_id).iter()
            .filter_map(|other| self.narrow_phase(obj_id, other))
            .collect()
    }

    /// 附近物件查詢（用於智能捕捉）
    pub fn query_nearby(&self, point: [f64; 3], radius: f64) -> Vec<String> {
        let q = Aabb {
            min: [point[0]-radius, point[1]-radius, point[2]-radius],
            max: [point[0]+radius, point[1]+radius, point[2]+radius],
        };
        self.aabbs.iter()
            .filter(|(_, a)| a.overlaps(&q))
            .map(|(id, _)| id.clone())
            .collect()
    }
}

// ─── 體積 / 重量計算 ──────────────────────────────────────────

pub struct VolumeCalc;

impl VolumeCalc {
    pub fn volume_mm3(shape: &Shape) -> f64 {
        match shape {
            Shape::Box { width, height, depth } =>
                width * height * depth,
            Shape::Cylinder { radius, height, .. } =>
                std::f64::consts::PI * radius * radius * height,
            Shape::Sphere { radius, .. } =>
                (4.0/3.0) * std::f64::consts::PI * radius.powi(3),
            Shape::Extrusion { base_shape, distance, .. } => {
                let (min, max) = base_shape.bounding_box();
                let area = (max[0]-min[0]) * (max[2]-min[2]);
                area * distance.abs()
            }
            Shape::Mesh { vertices, indices } =>
                Self::mesh_volume(vertices, indices),
        }
    }

    /// 散度定理計算網格體積
    fn mesh_volume(verts: &[[f32; 3]], idx: &[u32]) -> f64 {
        let mut v = 0.0f64;
        for c in idx.chunks(3) {
            if c.len() < 3 { continue; }
            let a = verts[c[0] as usize];
            let b = verts[c[1] as usize];
            let c = verts[c[2] as usize];
            v += a[0] as f64 * (b[1] as f64 * c[2] as f64 - c[1] as f64 * b[2] as f64);
            v += b[0] as f64 * (c[1] as f64 * a[2] as f64 - a[1] as f64 * c[2] as f64);
            v += c[0] as f64 * (a[1] as f64 * b[2] as f64 - b[1] as f64 * a[2] as f64);
        }
        (v / 6.0).abs()
    }

    /// 重量 kg
    pub fn weight_kg(shape: &Shape, density_kg_m3: f64) -> f64 {
        Self::volume_mm3(shape) * 1e-9 * density_kg_m3
    }

    /// 重量 kN（結構工程常用）
    pub fn weight_kn(shape: &Shape, density_kg_m3: f64) -> f64 {
        Self::weight_kg(shape, density_kg_m3) * 9.81 / 1000.0
    }
}

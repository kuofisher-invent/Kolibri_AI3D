//! Simple CSG (boolean) operations for axis-aligned boxes

use crate::scene::{SceneObject, Shape, MaterialKind};

#[derive(Debug, Clone, Copy)]
pub enum CsgOp {
    Union,
    Subtract,
    Intersect,
}

/// Perform CSG operation on two box objects.
/// Returns the resulting objects (may be 0-3 boxes).
pub fn box_csg(a: &SceneObject, b: &SceneObject, op: CsgOp) -> Vec<SceneObject> {
    let (a_min, a_max) = box_bounds(a);
    let (b_min, b_max) = box_bounds(b);

    // Check overlap
    let overlap = a_min[0] < b_max[0] && a_max[0] > b_min[0]
               && a_min[1] < b_max[1] && a_max[1] > b_min[1]
               && a_min[2] < b_max[2] && a_max[2] > b_min[2];

    match op {
        CsgOp::Union => {
            if !overlap {
                return vec![a.clone(), b.clone()];
            }
            // Overlapping union: compute bounding box
            let min = [
                a_min[0].min(b_min[0]),
                a_min[1].min(b_min[1]),
                a_min[2].min(b_min[2]),
            ];
            let max = [
                a_max[0].max(b_max[0]),
                a_max[1].max(b_max[1]),
                a_max[2].max(b_max[2]),
            ];
            vec![make_box(&a.name, min, max, a.material)]
        }
        CsgOp::Subtract => {
            if !overlap {
                return vec![a.clone()];
            }
            // Compute intersection region
            let i_min = [
                a_min[0].max(b_min[0]),
                a_min[1].max(b_min[1]),
                a_min[2].max(b_min[2]),
            ];
            let i_max = [
                a_max[0].min(b_max[0]),
                a_max[1].min(b_max[1]),
                a_max[2].min(b_max[2]),
            ];

            // Split A into up to 6 boxes (subtract the intersection volume)
            let mut result = Vec::new();

            // Left piece (X < i_min.x)
            if a_min[0] < i_min[0] {
                result.push(make_box(&format!("{}_L", a.name),
                    a_min, [i_min[0], a_max[1], a_max[2]], a.material));
            }
            // Right piece (X > i_max.x)
            if a_max[0] > i_max[0] {
                result.push(make_box(&format!("{}_R", a.name),
                    [i_max[0], a_min[1], a_min[2]], a_max, a.material));
            }
            // Bottom piece (Y < i_min.y, within X range)
            if a_min[1] < i_min[1] {
                result.push(make_box(&format!("{}_B", a.name),
                    [i_min[0], a_min[1], a_min[2]], [i_max[0], i_min[1], a_max[2]], a.material));
            }
            // Top piece (Y > i_max.y, within X range)
            if a_max[1] > i_max[1] {
                result.push(make_box(&format!("{}_T", a.name),
                    [i_min[0], i_max[1], a_min[2]], [i_max[0], a_max[1], a_max[2]], a.material));
            }
            // Front piece (Z < i_min.z, within X and Y range)
            if a_min[2] < i_min[2] {
                result.push(make_box(&format!("{}_F", a.name),
                    [i_min[0], i_min[1], a_min[2]], [i_max[0], i_max[1], i_min[2]], a.material));
            }
            // Back piece (Z > i_max.z, within X and Y range)
            if a_max[2] > i_max[2] {
                result.push(make_box(&format!("{}_K", a.name),
                    [i_min[0], i_min[1], i_max[2]], [i_max[0], i_max[1], a_max[2]], a.material));
            }

            result
        }
        CsgOp::Intersect => {
            if !overlap {
                return vec![];
            }
            let min = [
                a_min[0].max(b_min[0]),
                a_min[1].max(b_min[1]),
                a_min[2].max(b_min[2]),
            ];
            let max = [
                a_max[0].min(b_max[0]),
                a_max[1].min(b_max[1]),
                a_max[2].min(b_max[2]),
            ];
            if max[0] > min[0] && max[1] > min[1] && max[2] > min[2] {
                vec![make_box(&format!("{}_\u{2229}_{}", a.name, b.name), min, max, a.material)]
            } else {
                vec![]
            }
        }
    }
}

fn box_bounds(obj: &SceneObject) -> ([f32; 3], [f32; 3]) {
    let p = obj.position;
    match &obj.shape {
        Shape::Box { width, height, depth } => {
            (p, [p[0] + width, p[1] + height, p[2] + depth])
        }
        _ => (p, p),
    }
}

fn make_box(name: &str, min: [f32; 3], max: [f32; 3], material: MaterialKind) -> SceneObject {
    let w = (max[0] - min[0]).max(1.0);
    let h = (max[1] - min[1]).max(1.0);
    let d = (max[2] - min[2]).max(1.0);
    SceneObject {
        id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
        name: name.to_string(),
        shape: Shape::Box { width: w, height: h, depth: d },
        position: min,
        material,
        rotation_y: 0.0,
        tag: "\u{9810}\u{8a2d}".to_string(),
        visible: true,
        roughness: 0.5,
        metallic: 0.0,
        texture_path: None,
        component_kind: Default::default(),
        parent_id: None,
        component_def_id: None,
        locked: false,
    }
}

/// Generic CSG — converts any shape to AABB box approximation first
pub fn shape_csg(a: &SceneObject, b: &SceneObject, op: CsgOp) -> Vec<SceneObject> {
    let to_box = |obj: &SceneObject| -> SceneObject {
        let p = obj.position;
        let (w, h, d) = match &obj.shape {
            Shape::Box { width, height, depth } => (*width, *height, *depth),
            Shape::Cylinder { radius, height, .. } => (*radius * 2.0, *height, *radius * 2.0),
            Shape::Sphere { radius, .. } => (*radius * 2.0, *radius * 2.0, *radius * 2.0),
            _ => return obj.clone(),
        };
        SceneObject {
            shape: Shape::Box { width: w, height: h, depth: d },
            ..obj.clone()
        }
    };
    let box_a = to_box(a);
    let box_b = to_box(b);
    box_csg(&box_a, &box_b, op)
}

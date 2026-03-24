use serde::{Deserialize, Serialize};
use glam::Vec3;

// ─── Primitive IDs ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub String);

impl ObjectId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string()[..8].to_string())
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Face Reference ───────────────────────────────────────────────────────────
// Format: "obj_id.face.top" | "obj_id.face.bottom" | etc.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceRef {
    pub obj_id:   String,
    pub face:     FaceSide,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FaceSide {
    Top, Bottom, Front, Back, Left, Right,
}

impl FaceRef {
    /// Parse "obj_abc.face.top" → FaceRef
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 || parts[1] != "face" {
            anyhow::bail!("Invalid face ref: '{}', expected format: 'obj_id.face.top'", s);
        }
        let face = match parts[2] {
            "top"    => FaceSide::Top,
            "bottom" => FaceSide::Bottom,
            "front"  => FaceSide::Front,
            "back"   => FaceSide::Back,
            "left"   => FaceSide::Left,
            "right"  => FaceSide::Right,
            other    => anyhow::bail!("Unknown face side: '{}'", other),
        };
        Ok(Self { obj_id: parts[0].to_string(), face })
    }

    pub fn normal(&self) -> Vec3 {
        match self.face {
            FaceSide::Top    => Vec3::Y,
            FaceSide::Bottom => Vec3::NEG_Y,
            FaceSide::Front  => Vec3::NEG_Z,
            FaceSide::Back   => Vec3::Z,
            FaceSide::Right  => Vec3::X,
            FaceSide::Left   => Vec3::NEG_X,
        }
    }
}

// ─── Core 3D Object ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadObject {
    pub id:       ObjectId,
    pub name:     String,
    pub shape:    Shape,
    pub position: [f64; 3],   // origin in mm
    pub material: Material,
    pub visible:  bool,
    pub locked:   bool,
}

impl CadObject {
    pub fn new(name: impl Into<String>, shape: Shape, position: [f64; 3]) -> Self {
        Self {
            id:       ObjectId::new(),
            name:     name.into(),
            shape,
            position,
            material: Material::default(),
            visible:  true,
            locked:   false,
        }
    }
}

// ─── Shapes ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Shape {
    Box {
        width:  f64,   // X mm
        height: f64,   // Y mm
        depth:  f64,   // Z mm
    },
    Cylinder {
        radius: f64,
        height: f64,
        segments: u32,
    },
    Sphere {
        radius:   f64,
        segments: u32,
    },
    Extrusion {
        /// Original face that was push/pulled
        source_face: FaceSide,
        base_shape:  Box<Shape>,
        distance:    f64,
    },
    Mesh {
        /// Raw triangulated mesh (after boolean ops, etc.)
        vertices: Vec<[f32; 3]>,
        indices:  Vec<u32>,
    },
}

impl Shape {
    pub fn type_name(&self) -> &'static str {
        match self {
            Shape::Box { .. }       => "box",
            Shape::Cylinder { .. }  => "cylinder",
            Shape::Sphere { .. }    => "sphere",
            Shape::Extrusion { .. } => "extrusion",
            Shape::Mesh { .. }      => "mesh",
        }
    }

    pub fn bounding_box(&self) -> ([f64; 3], [f64; 3]) {
        match self {
            Shape::Box { width, height, depth } => {
                ([0.0, 0.0, 0.0], [*width, *height, *depth])
            }
            Shape::Cylinder { radius, height, .. } => {
                ([-radius, 0.0, -radius], [*radius, *height, *radius])
            }
            Shape::Sphere { radius, .. } => {
                ([-radius, -radius, -radius], [*radius, *radius, *radius])
            }
            Shape::Extrusion { base_shape, distance, .. } => {
                let (min, mut max) = base_shape.bounding_box();
                max[1] += distance;
                (min, max)
            }
            Shape::Mesh { vertices, .. } => {
                if vertices.is_empty() {
                    return ([0.0; 3], [0.0; 3]);
                }
                let mut min = [f64::MAX; 3];
                let mut max = [f64::MIN; 3];
                for v in vertices {
                    for i in 0..3 {
                        min[i] = min[i].min(v[i] as f64);
                        max[i] = max[i].max(v[i] as f64);
                    }
                }
                (min, max)
            }
        }
    }
}

// ─── Material ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub name:      String,
    pub color:     [f32; 4],   // RGBA 0..1
    pub roughness: f32,
    pub metallic:  f32,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            name:      "Default".into(),
            color:     [0.8, 0.8, 0.8, 1.0],
            roughness: 0.7,
            metallic:  0.0,
        }
    }
}

impl Material {
    pub fn from_name(name: &str) -> Self {
        let color = match name.to_lowercase().as_str() {
            "wood"     => [0.6, 0.4, 0.2, 1.0],
            "concrete" => [0.5, 0.5, 0.5, 1.0],
            "glass"    => [0.8, 0.9, 1.0, 0.3],
            "metal"    => [0.7, 0.7, 0.8, 1.0],
            "brick"    => [0.7, 0.3, 0.2, 1.0],
            "white"    => [1.0, 1.0, 1.0, 1.0],
            "black"    => [0.05, 0.05, 0.05, 1.0],
            _          => [0.8, 0.8, 0.8, 1.0],
        };
        let metallic = if name == "metal" { 0.9 } else { 0.0 };
        Self { name: name.to_string(), color, roughness: 0.5, metallic }
    }
}

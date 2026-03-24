use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use anyhow::{bail, Result};

use super::geometry::*;
use super::operations::*;
use super::collision::CollisionWorld;

// ─── Scene ────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct CadScene {
    pub objects:   HashMap<String, CadObject>,
    pub version:   u64,
    pub metadata:  SceneMetadata,
    pub collision: CollisionWorld,   // ← 自動同步，永遠保持最新
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneMetadata {
    pub name:       String,
    pub unit:       String,   // "mm" | "cm" | "m"
    pub created_at: String,
}

impl Default for SceneMetadata {
    fn default() -> Self {
        Self {
            name:       "Untitled Scene".into(),
            unit:       "mm".into(),
            created_at: chrono_now(),
        }
    }
}

fn chrono_now() -> String {
    // Simple timestamp without chrono dependency
    "2026-01-01T00:00:00Z".to_string()
}

// ─── Scene Summary (for Claude to read) ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneSummary {
    pub object_count: usize,
    pub version:      u64,
    pub unit:         String,
    pub objects:      Vec<ObjectSummary>,
    pub bounding_box: Option<([f64; 3], [f64; 3])>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSummary {
    pub id:          String,
    pub name:        String,
    pub shape_type:  String,
    pub position:    [f64; 3],
    pub dimensions:  [f64; 3],   // width, height, depth
    pub material:    String,
    pub visible:     bool,
    pub faces:       Vec<String>, // Available face refs
}

// ─── Scene Implementation ─────────────────────────────────────────────────────

impl CadScene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bump_version(&mut self) {
        self.version += 1;
    }

    /// Internal: sync collision world after any object change
    fn sync_collision(&mut self, id: &str) {
        if let Some(obj) = self.objects.get(id) {
            self.collision.update_object(obj);
        }
    }

    // ── Create Operations ────────────────────────────────────────────────────

    pub fn create_box(
        &mut self,
        name: Option<String>,
        origin: [f64; 3],
        width: f64, height: f64, depth: f64,
    ) -> Result<String> {
        if width <= 0.0 || height <= 0.0 || depth <= 0.0 {
            bail!("All dimensions must be positive (got {}x{}x{})", width, height, depth);
        }
        let label = name.unwrap_or_else(|| format!("Box_{}", self.objects.len() + 1));
        let obj = CadObject::new(label, Shape::Box { width, height, depth }, origin);
        let id = obj.id.0.clone();
        self.objects.insert(id.clone(), obj);
        self.sync_collision(&id);
        self.bump_version();
        Ok(id)
    }

    pub fn create_cylinder(
        &mut self,
        name: Option<String>,
        origin: [f64; 3],
        radius: f64, height: f64, segments: u32,
    ) -> Result<String> {
        if radius <= 0.0 || height <= 0.0 {
            bail!("radius and height must be positive");
        }
        let label = name.unwrap_or_else(|| format!("Cylinder_{}", self.objects.len() + 1));
        let obj = CadObject::new(label, Shape::Cylinder { radius, height, segments }, origin);
        let id = obj.id.0.clone();
        self.objects.insert(id.clone(), obj);
        self.sync_collision(&id);
        self.bump_version();
        Ok(id)
    }

    pub fn create_sphere(
        &mut self,
        name: Option<String>,
        origin: [f64; 3],
        radius: f64, segments: u32,
    ) -> Result<String> {
        if radius <= 0.0 {
            bail!("radius must be positive");
        }
        let label = name.unwrap_or_else(|| format!("Sphere_{}", self.objects.len() + 1));
        let obj = CadObject::new(label, Shape::Sphere { radius, segments }, origin);
        let id = obj.id.0.clone();
        self.objects.insert(id.clone(), obj);
        self.sync_collision(&id);
        self.bump_version();
        Ok(id)
    }

    // ── Push/Pull ─────────────────────────────────────────────────────────────

    pub fn push_pull(&mut self, face_str: &str, distance: f64) -> Result<String> {
        let face_ref = FaceRef::parse(face_str)?;
        let obj = self.objects.get_mut(&face_ref.obj_id)
            .ok_or_else(|| anyhow::anyhow!("Object '{}' not found", face_ref.obj_id))?;

        if obj.locked {
            bail!("Object '{}' is locked", obj.id);
        }

        // Apply the push/pull to the shape
        match &mut obj.shape {
            Shape::Box { width, height, depth } => {
                match face_ref.face {
                    FaceSide::Top | FaceSide::Bottom => *height = (*height + distance).max(0.1),
                    FaceSide::Left | FaceSide::Right => *width  = (*width  + distance).max(0.1),
                    FaceSide::Front | FaceSide::Back => *depth  = (*depth  + distance).max(0.1),
                }
            }
            Shape::Cylinder { height, .. } => {
                match face_ref.face {
                    FaceSide::Top | FaceSide::Bottom => {
                        *height = (*height + distance).max(0.1);
                    }
                    _ => bail!("Can only push/pull top or bottom face of a cylinder"),
                }
            }
            _ => bail!("Push/Pull not supported for this shape type"),
        }

        self.sync_collision(&face_ref.obj_id);
        self.bump_version();
        Ok(face_ref.obj_id)
    }

    // ── Material ──────────────────────────────────────────────────────────────

    pub fn set_material(&mut self, obj_id: &str, material_name: &str) -> Result<()> {
        let obj = self.objects.get_mut(obj_id)
            .ok_or_else(|| anyhow::anyhow!("Object '{}' not found", obj_id))?;
        obj.material = Material::from_name(material_name);
        self.bump_version();
        Ok(())
    }

    // ── Move ──────────────────────────────────────────────────────────────────

    pub fn move_object(&mut self, obj_id: &str, delta: [f64; 3]) -> Result<()> {
        let obj = self.objects.get_mut(obj_id)
            .ok_or_else(|| anyhow::anyhow!("Object '{}' not found", obj_id))?;
        for i in 0..3 {
            obj.position[i] += delta[i];
        }
        self.sync_collision(obj_id);
        self.bump_version();
        Ok(())
    }

    // ── Delete ────────────────────────────────────────────────────────────────

    pub fn delete_object(&mut self, obj_id: &str) -> Result<()> {
        if self.objects.remove(obj_id).is_none() {
            bail!("Object '{}' not found", obj_id);
        }
        self.collision.remove_object(obj_id);
        self.bump_version();
        Ok(())
    }

    // ── Boolean (simplified - marks relationship) ─────────────────────────────

    pub fn boolean_subtract(&mut self, base_id: &str, tool_id: &str, name: Option<String>) -> Result<String> {
        let base = self.objects.get(base_id)
            .ok_or_else(|| anyhow::anyhow!("Base object '{}' not found", base_id))?.clone();
        let _tool = self.objects.get(tool_id)
            .ok_or_else(|| anyhow::anyhow!("Tool object '{}' not found", tool_id))?;

        // In MVP: create an "opening" notation - full CSG requires geometric kernel
        let label = name.unwrap_or_else(|| format!("{}_cut", base.name));
        let result = CadObject::new(label, base.shape.clone(), base.position);
        let id = result.id.0.clone();

        self.objects.remove(tool_id);   // consume the cutter
        self.objects.remove(base_id);   // consume the base
        self.objects.insert(id.clone(), result);
        self.bump_version();
        Ok(id)
    }

    // ── Clear ─────────────────────────────────────────────────────────────────

    pub fn clear(&mut self) {
        self.objects.clear();
        self.collision = CollisionWorld::new();
        self.bump_version();
    }

    // ── Batch Execute ─────────────────────────────────────────────────────────

    pub fn execute_batch(&mut self, ops: Vec<CadOperation>) -> Vec<OpResult> {
        let mut results = Vec::new();
        for (i, op) in ops.into_iter().enumerate() {
            let result = self.execute_one(i, op);
            results.push(result);
        }
        results
    }

    fn execute_one(&mut self, i: usize, op: CadOperation) -> OpResult {
        match op {
            CadOperation::CreateBox { name, origin, width, height, depth } => {
                match self.create_box(name, origin, width, height, depth) {
                    Ok(id)  => OpResult::ok(i, &id, format!("Created box '{id}'")),
                    Err(e)  => OpResult::err(i, e.to_string()),
                }
            }
            CadOperation::CreateCylinder { name, origin, radius, height, segments } => {
                match self.create_cylinder(name, origin, radius, height, segments) {
                    Ok(id)  => OpResult::ok(i, &id, format!("Created cylinder '{id}'")),
                    Err(e)  => OpResult::err(i, e.to_string()),
                }
            }
            CadOperation::CreateSphere { name, origin, radius, segments } => {
                match self.create_sphere(name, origin, radius, segments) {
                    Ok(id)  => OpResult::ok(i, &id, format!("Created sphere '{id}'")),
                    Err(e)  => OpResult::err(i, e.to_string()),
                }
            }
            CadOperation::PushPull { face, distance } => {
                match self.push_pull(&face, distance) {
                    Ok(id)  => OpResult::ok(i, &id, format!("Push/Pull on {face} by {distance}mm")),
                    Err(e)  => OpResult::err(i, e.to_string()),
                }
            }
            CadOperation::SetMaterial { obj_id, material } => {
                match self.set_material(&obj_id, &material) {
                    Ok(_)  => OpResult::ok(i, &obj_id, format!("Material set to '{material}'")),
                    Err(e) => OpResult::err(i, e.to_string()),
                }
            }
            CadOperation::MoveObject { obj_id, delta } => {
                match self.move_object(&obj_id, delta) {
                    Ok(_)  => OpResult::ok(i, &obj_id, "Moved".into()),
                    Err(e) => OpResult::err(i, e.to_string()),
                }
            }
            CadOperation::DeleteObject { obj_id } => {
                match self.delete_object(&obj_id) {
                    Ok(_)  => OpResult::ok(i, &obj_id, "Deleted".into()),
                    Err(e) => OpResult::err(i, e.to_string()),
                }
            }
            CadOperation::RenameObject { obj_id, name } => {
                if let Some(obj) = self.objects.get_mut(&obj_id) {
                    obj.name = name.clone();
                    self.bump_version();
                    OpResult::ok(i, &obj_id, format!("Renamed to '{name}'"))
                } else {
                    OpResult::err(i, format!("Object '{obj_id}' not found"))
                }
            }
            CadOperation::BooleanSubtract { base_id, tool_id, name } => {
                match self.boolean_subtract(&base_id, &tool_id, name) {
                    Ok(id)  => OpResult::ok(i, &id, format!("Boolean subtract → '{id}'")),
                    Err(e)  => OpResult::err(i, e.to_string()),
                }
            }
            CadOperation::BooleanUnion { base_id, tool_id, name } => {
                // MVP: just keep base, remove tool
                if self.objects.contains_key(&tool_id) {
                    self.objects.remove(&tool_id);
                    if let Some(obj) = self.objects.get_mut(&base_id) {
                        if let Some(n) = name { obj.name = n; }
                    }
                    self.bump_version();
                    OpResult::ok(i, &base_id, "Union applied (MVP)".into())
                } else {
                    OpResult::err(i, format!("Tool object '{tool_id}' not found"))
                }
            }
            CadOperation::ClearScene => {
                self.clear();
                OpResult::ok(i, "", "Scene cleared".into())
            }
        }
    }

    // ── Scene Summary for Claude ──────────────────────────────────────────────

    pub fn summarize(&self) -> SceneSummary {
        let objects: Vec<ObjectSummary> = self.objects.values().map(|obj| {
            let (min, max) = obj.shape.bounding_box();
            let dimensions = [
                max[0] - min[0],
                max[1] - min[1],
                max[2] - min[2],
            ];
            let faces = match &obj.shape {
                Shape::Box { .. } => vec![
                    format!("{}.face.top",    obj.id),
                    format!("{}.face.bottom", obj.id),
                    format!("{}.face.front",  obj.id),
                    format!("{}.face.back",   obj.id),
                    format!("{}.face.left",   obj.id),
                    format!("{}.face.right",  obj.id),
                ],
                Shape::Cylinder { .. } => vec![
                    format!("{}.face.top",    obj.id),
                    format!("{}.face.bottom", obj.id),
                ],
                _ => vec![],
            };
            ObjectSummary {
                id:         obj.id.0.clone(),
                name:       obj.name.clone(),
                shape_type: obj.shape.type_name().to_string(),
                position:   obj.position,
                dimensions,
                material:   obj.material.name.clone(),
                visible:    obj.visible,
                faces,
            }
        }).collect();

        SceneSummary {
            object_count: objects.len(),
            version:      self.version,
            unit:         self.metadata.unit.clone(),
            objects,
            bounding_box: self.total_bounding_box(),
        }
    }

    fn total_bounding_box(&self) -> Option<([f64; 3], [f64; 3])> {
        if self.objects.is_empty() { return None; }
        let mut global_min = [f64::MAX; 3];
        let mut global_max = [f64::MIN; 3];
        for obj in self.objects.values() {
            let (min, max) = obj.shape.bounding_box();
            for i in 0..3 {
                global_min[i] = global_min[i].min(obj.position[i] + min[i]);
                global_max[i] = global_max[i].max(obj.position[i] + max[i]);
            }
        }
        Some((global_min, global_max))
    }
}

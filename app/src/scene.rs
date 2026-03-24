use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Maximum number of undo history entries to keep in memory.
const MAX_HISTORY: usize = 50;

/// A group is a collection of object IDs that move/transform together
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupDef {
    pub id: String,
    pub name: String,
    pub children: Vec<String>,  // child object IDs
    pub position: [f32; 3],     // group origin offset
    pub rotation_y: f32,
}

/// A component definition: a reusable shape template.
/// Editing one instance updates ALL instances of the same component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDef {
    pub id: String,
    pub name: String,
    pub objects: Vec<SceneObject>,  // the geometry inside this component
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub objects: HashMap<String, SceneObject>,
    pub version: u64,
    pub(crate) groups: HashMap<String, GroupDef>,
    /// Component definitions: reusable shape templates
    pub(crate) component_defs: HashMap<String, ComponentDef>,
    /// Undo stack: each entry is a full snapshot of `objects` + `free_mesh` before a change.
    pub(crate) undo_stack: Vec<(HashMap<String, SceneObject>, crate::halfedge::HeMesh)>,
    /// Redo stack: snapshots that were undone and can be re-applied.
    redo_stack: Vec<(HashMap<String, SceneObject>, crate::halfedge::HeMesh)>,
    /// Construction/guide lines (pairs of start, end points). Not saved as objects.
    pub(crate) guide_lines: Vec<([f32; 3], [f32; 3])>,
    /// Shared free-form modeling mesh. Lines drawn by the user become edges here;
    /// closed loops auto-detect as faces that can be push/pulled.
    pub(crate) free_mesh: crate::halfedge::HeMesh,
    pub(crate) free_mesh_material: MaterialKind,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            objects: HashMap::new(),
            version: 0,
            groups: HashMap::new(),
            component_defs: HashMap::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            guide_lines: Vec::new(),
            free_mesh: crate::halfedge::HeMesh::new(),
            free_mesh_material: MaterialKind::White,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneObject {
    pub id: String,
    pub name: String,
    pub shape: Shape,
    pub position: [f32; 3],
    pub material: MaterialKind,
    /// Y-axis rotation in radians
    #[serde(default)]
    pub(crate) rotation_y: f32,
    /// Layer/tag name
    #[serde(default = "default_tag")]
    pub(crate) tag: String,
    /// Layer visibility (controlled by hidden_tags)
    #[serde(default = "default_visible")]
    pub(crate) visible: bool,
    /// PBR roughness: 0.0 = mirror, 1.0 = matte
    #[serde(default = "default_roughness")]
    pub(crate) roughness: f32,
    /// PBR metallic: 0.0 = dielectric, 1.0 = metal
    #[serde(default)]
    pub(crate) metallic: f32,
    /// Optional image texture path (PNG/JPG) for future texture mapping
    #[serde(default)]
    pub(crate) texture_path: Option<String>,
    /// Collision detection component kind (column, beam, plate, etc.)
    #[serde(default)]
    pub(crate) component_kind: crate::collision::ComponentKind,
}

fn default_tag() -> String { "\u{9810}\u{8a2d}".to_string() }  // "預設"
fn default_visible() -> bool { true }
fn default_roughness() -> f32 { 0.5 }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Shape {
    Box { width: f32, height: f32, depth: f32 },
    Cylinder { radius: f32, height: f32, segments: u32 },
    Sphere { radius: f32, segments: u32 },
    Line { points: Vec<[f32; 3]>, thickness: f32 },
    Mesh(crate::halfedge::HeMesh),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialKind {
    // ── 石材/混凝土 ──
    Concrete,        // 混凝土
    ConcreteSmooth,  // 清水混凝土
    Stone,           // 石材
    Marble,          // 大理石
    Granite,         // 花崗岩

    // ── 木材 ──
    Wood,            // 木材
    WoodLight,       // 淺色木
    WoodDark,        // 深色木
    Bamboo,          // 竹
    Plywood,         // 合板

    // ── 金屬 ──
    Metal,           // 金屬
    Steel,           // 鋼
    Aluminum,        // 鋁
    Copper,          // 銅
    Gold,            // 金

    // ── 磚/瓦 ──
    Brick,           // 紅磚
    BrickWhite,      // 白磚
    Tile,            // 磁磚
    TileDark,        // 深色磁磚

    // ── 玻璃 ──
    Glass,           // 透明玻璃
    GlassTinted,     // 有色玻璃
    GlassFrosted,    // 霧面玻璃

    // ── 路面/地面 ──
    Asphalt,         // 柏油路
    Gravel,          // 碎石
    Grass,           // 草地
    Soil,            // 泥土

    // ── 其他 ──
    White,           // 白色
    Black,           // 黑色
    Plaster,         // 灰泥
    Paint(u32),      // 油漆色 (0xRRGGBB)
    Custom([f32; 4]),// 自訂 RGBA
}

impl MaterialKind {
    /// RGBA colour sent to the GPU.
    /// Sentinel alpha values (0.91-0.98) trigger procedural textures in the shader:
    ///   0.91 = brick, 0.92 = wood, 0.93 = metal, 0.94 = concrete,
    ///   0.95 = marble, 0.96 = tile, 0.97 = asphalt, 0.98 = grass
    pub fn color(&self) -> [f32; 4] {
        match self {
            // ── 石材/混凝土 ──
            Self::Concrete       => [0.55, 0.55, 0.55, 0.94],
            Self::ConcreteSmooth => [0.75, 0.73, 0.70, 0.94],
            Self::Stone          => [0.60, 0.58, 0.55, 1.0],
            Self::Marble         => [0.92, 0.90, 0.88, 0.95],
            Self::Granite        => [0.45, 0.43, 0.42, 1.0],

            // ── 木材 ──
            Self::Wood           => [0.60, 0.40, 0.20, 0.92],
            Self::WoodLight      => [0.85, 0.72, 0.52, 0.92],
            Self::WoodDark       => [0.42, 0.28, 0.15, 0.92],
            Self::Bamboo         => [0.80, 0.75, 0.50, 0.92],
            Self::Plywood        => [0.78, 0.65, 0.45, 0.92],

            // ── 金屬 ──
            Self::Metal          => [0.72, 0.72, 0.78, 0.93],
            Self::Steel          => [0.62, 0.63, 0.65, 0.93],
            Self::Aluminum       => [0.80, 0.81, 0.83, 0.93],
            Self::Copper         => [0.72, 0.45, 0.20, 0.93],
            Self::Gold           => [0.83, 0.69, 0.22, 0.93],

            // ── 磚/瓦 ──
            Self::Brick          => [0.72, 0.35, 0.22, 0.91],
            Self::BrickWhite     => [0.88, 0.85, 0.80, 0.91],
            Self::Tile           => [0.85, 0.85, 0.82, 0.96],
            Self::TileDark       => [0.35, 0.35, 0.38, 0.96],

            // ── 玻璃 ──
            Self::Glass          => [0.70, 0.85, 0.95, 0.3],
            Self::GlassTinted    => [0.40, 0.55, 0.65, 0.35],
            Self::GlassFrosted   => [0.80, 0.85, 0.88, 0.6],

            // ── 路面/地面 ──
            Self::Asphalt        => [0.25, 0.25, 0.27, 0.97],
            Self::Gravel         => [0.55, 0.52, 0.48, 1.0],
            Self::Grass          => [0.35, 0.55, 0.25, 0.98],
            Self::Soil           => [0.45, 0.35, 0.22, 1.0],

            // ── 其他 ──
            Self::White          => [0.95, 0.95, 0.95, 1.0],
            Self::Black          => [0.10, 0.10, 0.10, 1.0],
            Self::Plaster        => [0.88, 0.86, 0.82, 1.0],
            Self::Paint(hex) => {
                let r = ((*hex >> 16) & 0xFF) as f32 / 255.0;
                let g = ((*hex >> 8) & 0xFF) as f32 / 255.0;
                let b = (*hex & 0xFF) as f32 / 255.0;
                [r, g, b, 1.0]
            }
            Self::Custom(c)      => *c,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Concrete       => "混凝土",
            Self::ConcreteSmooth => "清水混凝土",
            Self::Stone          => "石材",
            Self::Marble         => "大理石",
            Self::Granite        => "花崗岩",
            Self::Wood           => "木材",
            Self::WoodLight      => "淺色木",
            Self::WoodDark       => "深色木",
            Self::Bamboo         => "竹",
            Self::Plywood        => "合板",
            Self::Metal          => "金屬",
            Self::Steel          => "鋼",
            Self::Aluminum       => "鋁",
            Self::Copper         => "銅",
            Self::Gold           => "金",
            Self::Brick          => "紅磚",
            Self::BrickWhite     => "白磚",
            Self::Tile           => "磁磚",
            Self::TileDark       => "深色磁磚",
            Self::Glass          => "玻璃",
            Self::GlassTinted    => "有色玻璃",
            Self::GlassFrosted   => "霧面玻璃",
            Self::Asphalt        => "柏油路",
            Self::Gravel         => "碎石",
            Self::Grass          => "草地",
            Self::Soil           => "泥土",
            Self::White          => "白色",
            Self::Black          => "黑色",
            Self::Plaster        => "灰泥",
            Self::Paint(_)       => "油漆",
            Self::Custom(_)      => "自訂",
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            Self::Concrete | Self::ConcreteSmooth | Self::Stone
            | Self::Marble | Self::Granite => "石材混凝土",

            Self::Wood | Self::WoodLight | Self::WoodDark
            | Self::Bamboo | Self::Plywood => "木材",

            Self::Metal | Self::Steel | Self::Aluminum
            | Self::Copper | Self::Gold => "金屬",

            Self::Brick | Self::BrickWhite
            | Self::Tile | Self::TileDark => "磚瓦磁磚",

            Self::Glass | Self::GlassTinted
            | Self::GlassFrosted => "玻璃",

            Self::Asphalt | Self::Gravel
            | Self::Grass | Self::Soil => "路面地面",

            _ => "其他",
        }
    }

    /// All non-parameterised presets for the UI picker.
    pub const ALL: &'static [MaterialKind] = &[
        // 石材混凝土
        Self::Concrete, Self::ConcreteSmooth, Self::Stone, Self::Marble, Self::Granite,
        // 木材
        Self::Wood, Self::WoodLight, Self::WoodDark, Self::Bamboo, Self::Plywood,
        // 金屬
        Self::Metal, Self::Steel, Self::Aluminum, Self::Copper, Self::Gold,
        // 磚瓦磁磚
        Self::Brick, Self::BrickWhite, Self::Tile, Self::TileDark,
        // 玻璃
        Self::Glass, Self::GlassTinted, Self::GlassFrosted,
        // 路面地面
        Self::Asphalt, Self::Gravel, Self::Grass, Self::Soil,
        // 其他
        Self::White, Self::Black, Self::Plaster,
    ];
}

impl Scene {
    /// Save the current objects state onto the undo stack.
    /// Call this **before** any modifying operation.
    /// Clears the redo stack (new branch of history).
    pub fn snapshot(&mut self) {
        self.undo_stack.push((self.objects.clone(), self.free_mesh.clone()));
        if self.undo_stack.len() > MAX_HISTORY {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Undo the last change: restores the previous objects + free_mesh state.
    /// Returns `true` if there was something to undo.
    pub fn undo(&mut self) -> bool {
        if let Some((prev_objs, prev_mesh)) = self.undo_stack.pop() {
            self.redo_stack.push((self.objects.clone(), self.free_mesh.clone()));
            self.objects = prev_objs;
            self.free_mesh = prev_mesh;
            self.version += 1;
            true
        } else {
            false
        }
    }

    /// Redo a previously undone change.
    /// Returns `true` if there was something to redo.
    pub fn redo(&mut self) -> bool {
        if let Some((next_objs, next_mesh)) = self.redo_stack.pop() {
            self.undo_stack.push((self.objects.clone(), self.free_mesh.clone()));
            self.objects = next_objs;
            self.free_mesh = next_mesh;
            self.version += 1;
            true
        } else {
            false
        }
    }

    /// Returns how many undo steps are available.
    pub fn undo_count(&self) -> usize { self.undo_stack.len() }

    /// Returns how many redo steps are available.
    pub fn redo_count(&self) -> usize { self.redo_stack.len() }

    pub fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }

    pub fn next_id_pub(&self) -> String { self.next_id() }

    fn next_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    }

    /// Insert a box without calling snapshot() or bumping version.
    /// Used internally by split_box to avoid multiple snapshots.
    pub(crate) fn insert_box_raw(
        &mut self, name: String, pos: [f32; 3],
        w: f32, h: f32, d: f32, mat: MaterialKind,
    ) -> String {
        let id = self.next_id();
        self.objects.insert(id.clone(), SceneObject {
            id: id.clone(), name,
            shape: Shape::Box { width: w, height: h, depth: d },
            position: pos, material: mat,
            rotation_y: 0.0, tag: default_tag(), visible: true,
            roughness: default_roughness(), metallic: 0.0, texture_path: None, component_kind: Default::default(),
        });
        id
    }

    /// Split a box at a given position along an axis (0=X, 1=Y, 2=Z).
    /// Returns the IDs of the two resulting boxes.
    pub fn split_box(&mut self, obj_id: &str, axis: u8, split_pos: f32) -> Option<(String, String)> {
        let obj = self.objects.get(obj_id)?.clone();
        let p = obj.position;

        let (w, h, d) = match &obj.shape {
            Shape::Box { width, height, depth } => (*width, *height, *depth),
            _ => return None,
        };

        self.snapshot();
        self.objects.remove(obj_id);

        let (id1, id2) = match axis {
            0 => { // Split along X
                let split_local = (split_pos - p[0]).clamp(10.0, w - 10.0);
                let a = self.insert_box_raw(format!("{}_A", obj.name), p, split_local, h, d, obj.material);
                let b = self.insert_box_raw(format!("{}_B", obj.name), [p[0] + split_local, p[1], p[2]], w - split_local, h, d, obj.material);
                (a, b)
            }
            2 => { // Split along Z
                let split_local = (split_pos - p[2]).clamp(10.0, d - 10.0);
                let a = self.insert_box_raw(format!("{}_A", obj.name), p, w, h, split_local, obj.material);
                let b = self.insert_box_raw(format!("{}_B", obj.name), [p[0], p[1], p[2] + split_local], w, h, d - split_local, obj.material);
                (a, b)
            }
            1 => { // Split along Y
                let split_local = (split_pos - p[1]).clamp(10.0, h - 10.0);
                let a = self.insert_box_raw(format!("{}_A", obj.name), p, w, split_local, d, obj.material);
                let b = self.insert_box_raw(format!("{}_B", obj.name), [p[0], p[1] + split_local, p[2]], w, h - split_local, d, obj.material);
                (a, b)
            }
            _ => return None,
        };

        self.version += 1;
        Some((id1, id2))
    }

    pub fn add_box(
        &mut self, name: String, pos: [f32; 3],
        w: f32, h: f32, d: f32, mat: MaterialKind,
    ) -> String {
        self.snapshot();
        let id = self.next_id();
        self.objects.insert(id.clone(), SceneObject {
            id: id.clone(), name,
            shape: Shape::Box { width: w, height: h, depth: d },
            position: pos, material: mat,
            rotation_y: 0.0, tag: default_tag(), visible: true,
            roughness: default_roughness(), metallic: 0.0, texture_path: None, component_kind: Default::default(),
        });
        self.version += 1;
        id
    }

    pub fn add_cylinder(
        &mut self, name: String, pos: [f32; 3],
        r: f32, h: f32, seg: u32, mat: MaterialKind,
    ) -> String {
        self.snapshot();
        let id = self.next_id();
        self.objects.insert(id.clone(), SceneObject {
            id: id.clone(), name,
            shape: Shape::Cylinder { radius: r, height: h, segments: seg },
            position: pos, material: mat,
            rotation_y: 0.0, tag: default_tag(), visible: true,
            roughness: default_roughness(), metallic: 0.0, texture_path: None, component_kind: Default::default(),
        });
        self.version += 1;
        id
    }

    pub fn add_sphere(
        &mut self, name: String, pos: [f32; 3],
        r: f32, seg: u32, mat: MaterialKind,
    ) -> String {
        self.snapshot();
        let id = self.next_id();
        self.objects.insert(id.clone(), SceneObject {
            id: id.clone(), name,
            shape: Shape::Sphere { radius: r, segments: seg },
            position: pos, material: mat,
            rotation_y: 0.0, tag: default_tag(), visible: true,
            roughness: default_roughness(), metallic: 0.0, texture_path: None, component_kind: Default::default(),
        });
        self.version += 1;
        id
    }

    pub fn add_line(
        &mut self, name: String, points: Vec<[f32; 3]>,
        thickness: f32, mat: MaterialKind,
    ) -> String {
        self.snapshot();
        let id = self.next_id();
        let pos = points.first().copied().unwrap_or([0.0; 3]);
        self.objects.insert(id.clone(), SceneObject {
            id: id.clone(), name,
            shape: Shape::Line { points, thickness },
            position: pos, material: mat,
            rotation_y: 0.0, tag: default_tag(), visible: true,
            roughness: default_roughness(), metallic: 0.0, texture_path: None, component_kind: Default::default(),
        });
        self.version += 1;
        id
    }

    pub fn delete(&mut self, id: &str) -> bool {
        if !self.objects.contains_key(id) { return false; }
        self.snapshot();
        self.objects.remove(id);
        self.version += 1;
        true
    }

    pub fn clear(&mut self) {
        if self.objects.is_empty() && self.free_mesh.vertices.is_empty() { return; }
        self.snapshot();
        self.objects.clear();
        self.groups.clear();
        self.component_defs.clear();
        self.free_mesh = crate::halfedge::HeMesh::new();
        self.version += 1;
    }

    pub fn create_group(&mut self, name: String, child_ids: Vec<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        self.groups.insert(id.clone(), GroupDef {
            id: id.clone(),
            name,
            children: child_ids,
            position: [0.0; 3],
            rotation_y: 0.0,
        });
        self.version += 1;
        id
    }

    pub fn dissolve_group(&mut self, group_id: &str) {
        self.groups.remove(group_id);
        self.version += 1;
    }

    /// Create a component definition from selected objects.
    /// The original objects are tagged as instances of this component.
    pub fn create_component_def(&mut self, name: String, object_ids: &[String]) -> String {
        let def_id = self.next_id();
        let objects: Vec<SceneObject> = object_ids.iter()
            .filter_map(|id| self.objects.get(id).cloned())
            .collect();

        self.component_defs.insert(def_id.clone(), ComponentDef {
            id: def_id.clone(),
            name,
            objects,
        });

        // Mark original objects as instances of this component
        for id in object_ids {
            if let Some(obj) = self.objects.get_mut(id) {
                obj.tag = format!("元件:{}", def_id);
            }
        }

        self.version += 1;
        def_id
    }

    /// Update all instances of a component when the definition changes.
    /// For single-object components, syncs shape and material across all instances.
    pub fn sync_component_instances(&mut self, def_id: &str) {
        let def = match self.component_defs.get(def_id) {
            Some(d) => d.clone(),
            None => return,
        };

        let tag = format!("元件:{}", def_id);
        let instance_ids: Vec<String> = self.objects.iter()
            .filter(|(_, obj)| obj.tag == tag)
            .map(|(id, _)| id.clone())
            .collect();

        // For each instance, update its shape to match the definition
        // (simplified: only works for single-object components)
        if let Some(def_obj) = def.objects.first() {
            for id in &instance_ids {
                if let Some(obj) = self.objects.get_mut(id) {
                    obj.shape = def_obj.shape.clone();
                    obj.material = def_obj.material.clone();
                    // Keep position and name
                }
            }
        }

        self.version += 1;
    }

    /// Save scene to a JSON file
    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let file_data = SceneFile {
            version: "1.0".into(),
            app: "Kolibri_Ai3D".into(),
            objects: self.objects.values().cloned().collect(),
            groups: self.groups.values().cloned().collect(),
            component_defs: self.component_defs.values().cloned().collect(),
        };
        let json = serde_json::to_string_pretty(&file_data)
            .map_err(|e| format!("序列化失敗: {}", e))?;
        std::fs::write(path, json)
            .map_err(|e| format!("寫入失敗: {}", e))?;
        Ok(())
    }

    /// Load scene from a JSON file
    pub fn load_from_file(&mut self, path: &str) -> Result<usize, String> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| format!("讀取失敗: {}", e))?;
        let file_data: SceneFile = serde_json::from_str(&json)
            .map_err(|e| format!("解析失敗: {}", e))?;
        self.snapshot();
        self.objects.clear();
        self.groups.clear();
        self.component_defs.clear();
        for obj in file_data.objects {
            self.objects.insert(obj.id.clone(), obj);
        }
        for g in file_data.groups {
            self.groups.insert(g.id.clone(), g);
        }
        for cd in file_data.component_defs {
            self.component_defs.insert(cd.id.clone(), cd);
        }
        self.version += 1;
        Ok(self.objects.len())
    }
}

/// Convert all scene objects to collision::Component for collision queries.
pub fn scene_to_collision_components(scene: &Scene) -> Vec<crate::collision::Component> {
    scene.objects.values().map(|obj| {
        let (size, center) = match &obj.shape {
            Shape::Box { width, height, depth } => {
                ([*width, *height, *depth],
                 [obj.position[0] + width / 2.0, obj.position[1] + height / 2.0, obj.position[2] + depth / 2.0])
            }
            Shape::Cylinder { radius, height, .. } => {
                ([radius * 2.0, *height, radius * 2.0],
                 [obj.position[0] + radius, obj.position[1] + height / 2.0, obj.position[2] + radius])
            }
            Shape::Sphere { radius, .. } => {
                ([radius * 2.0, radius * 2.0, radius * 2.0],
                 [obj.position[0] + radius, obj.position[1] + radius, obj.position[2] + radius])
            }
            _ => ([100.0; 3], obj.position),
        };
        crate::collision::Component::new(obj.id.clone(), obj.component_kind, center, size)
    }).collect()
}

/// Compute collision center and size for a single SceneObject.
pub fn obj_collision_center_size(obj: &SceneObject) -> ([f32; 3], [f32; 3]) {
    match &obj.shape {
        Shape::Box { width, height, depth } => {
            ([obj.position[0] + width / 2.0, obj.position[1] + height / 2.0, obj.position[2] + depth / 2.0],
             [*width, *height, *depth])
        }
        Shape::Cylinder { radius, height, .. } => {
            ([obj.position[0] + radius, obj.position[1] + height / 2.0, obj.position[2] + radius],
             [radius * 2.0, *height, radius * 2.0])
        }
        Shape::Sphere { radius, .. } => {
            ([obj.position[0] + radius, obj.position[1] + radius, obj.position[2] + radius],
             [radius * 2.0, radius * 2.0, radius * 2.0])
        }
        _ => (obj.position, [100.0; 3]),
    }
}

#[derive(Serialize, Deserialize)]
struct SceneFile {
    version: String,
    app: String,
    objects: Vec<SceneObject>,
    #[serde(default)]
    groups: Vec<GroupDef>,
    #[serde(default)]
    component_defs: Vec<ComponentDef>,
}

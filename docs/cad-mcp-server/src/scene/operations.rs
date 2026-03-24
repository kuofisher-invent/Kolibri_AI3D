use serde::{Deserialize, Serialize};

// ─── Incoming Operations (from Claude / AI) ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CadOperation {
    CreateBox {
        name:     Option<String>,
        origin:   [f64; 3],
        width:    f64,
        height:   f64,
        depth:    f64,
    },
    CreateCylinder {
        name:     Option<String>,
        origin:   [f64; 3],
        radius:   f64,
        height:   f64,
        #[serde(default = "default_segments")]
        segments: u32,
    },
    CreateSphere {
        name:     Option<String>,
        origin:   [f64; 3],
        radius:   f64,
        #[serde(default = "default_segments")]
        segments: u32,
    },
    PushPull {
        /// Format: "obj_id.face.top"
        face:     String,
        distance: f64,
    },
    SetMaterial {
        obj_id:   String,
        material: String,
    },
    MoveObject {
        obj_id: String,
        delta:  [f64; 3],
    },
    DeleteObject {
        obj_id: String,
    },
    RenameObject {
        obj_id:  String,
        name:    String,
    },
    BooleanUnion {
        base_id: String,
        tool_id: String,
        name:    Option<String>,
    },
    BooleanSubtract {
        base_id: String,
        tool_id: String,
        name:    Option<String>,
    },
    ClearScene,
}

fn default_segments() -> u32 { 32 }

// ─── Operation Results ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpResult {
    pub op_index: usize,
    pub success:  bool,
    pub obj_id:   Option<String>,
    pub message:  String,
}

impl OpResult {
    pub fn ok(index: usize, obj_id: impl Into<String>, msg: impl Into<String>) -> Self {
        Self { op_index: index, success: true, obj_id: Some(obj_id.into()), message: msg.into() }
    }
    pub fn err(index: usize, msg: impl Into<String>) -> Self {
        Self { op_index: index, success: false, obj_id: None, message: msg.into() }
    }
}

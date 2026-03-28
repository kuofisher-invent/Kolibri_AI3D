//! kolibri-skp — SketchUp SDK FFI (動態載入，不需安裝 SketchUp)
//!
//! 使用 SketchUp C API SDK 直接讀取 .skp 檔案。
//! SDK DLL 在 runtime 動態載入：
//!   - 有 DLL → 完整匯入（幾何 + 元件 + 群組 + 材質）
//!   - 沒有 DLL → 回傳 Err，由上層退回 heuristic/bridge
//!
//! SDK 下載：https://extensions.sketchup.com/developer_center/sketchup_sdk

pub mod ffi;
pub mod converter;

/// 檢查 SDK DLL 是否可用
pub fn sdk_available() -> bool {
    ffi::try_load_sdk().is_ok()
}

/// 從 .skp 檔案匯入場景資料
/// 回傳序列化的場景結構，與 UnifiedIR 相容
pub fn import_skp(path: &str) -> Result<SkpScene, SkpError> {
    let sdk = ffi::try_load_sdk()?;
    let model = sdk.open_model(path)?;
    converter::convert_model(&sdk, &model)
}

/// SKP 匯入結果
#[derive(Debug, Clone, serde::Serialize)]
pub struct SkpScene {
    pub meshes: Vec<SkpMesh>,
    pub instances: Vec<SkpInstance>,
    pub groups: Vec<SkpGroup>,
    pub component_defs: Vec<SkpComponentDef>,
    pub materials: Vec<SkpMaterial>,
    pub units: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkpMesh {
    pub id: String,
    pub name: String,
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub material_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkpInstance {
    pub id: String,
    pub mesh_id: String,
    pub component_def_id: Option<String>,
    pub transform: [f32; 16],
    pub name: String,
    pub layer: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkpGroup {
    pub id: String,
    pub name: String,
    pub children: Vec<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkpComponentDef {
    pub id: String,
    pub name: String,
    pub mesh_ids: Vec<String>,
    pub instance_count: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkpMaterial {
    pub id: String,
    pub name: String,
    pub color: [f32; 4],
    pub texture_path: Option<String>,
    pub opacity: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum SkpError {
    #[error("SketchUp SDK DLL not found: {0}")]
    SdkNotFound(String),
    #[error("Failed to open model: {0}")]
    OpenFailed(String),
    #[error("SDK error: {0}")]
    SdkError(String),
    #[error("Conversion error: {0}")]
    ConvertError(String),
}

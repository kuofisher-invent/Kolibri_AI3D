//! kolibri-core — 純邏輯核心（場景、幾何、碰撞）
//! 無 GUI/wgpu 依賴，可獨立測試、headless 使用

pub mod scene;
pub mod halfedge;
pub mod collision;
pub mod command;
pub mod csg;
pub mod measure;

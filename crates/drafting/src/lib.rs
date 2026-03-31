//! Kolibri Drafting Module — 2D 出圖引擎
//!
//! 提供 2D CAD 繪圖實體、圖層管理、幾何運算（trim/offset/fillet）。
//! 透過 Cargo feature flag `drafting` 啟用。

pub mod entities;
pub mod layer;
pub mod geometry;

pub use entities::*;
pub use layer::*;

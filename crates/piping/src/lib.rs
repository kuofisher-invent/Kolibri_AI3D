//! Kolibri Piping Module — 管線繪製外掛
//!
//! 支援 PVC 水管、電管、鐵管（消防）等管線系統的繪製與管理。
//! 透過 Cargo feature flag `piping` 啟用，不啟用時零編譯成本。

pub mod pipe_data;
pub mod catalog;
pub mod geometry;
pub mod tools;

pub use pipe_data::*;
pub use catalog::PipeCatalog;
pub use tools::{PipingTool, PipingState};

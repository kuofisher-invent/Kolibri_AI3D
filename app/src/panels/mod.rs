pub(crate) mod material_swatches;
mod toolbar;
mod tab_properties;
mod tab_scene;
mod tab_help;
#[cfg(feature = "drafting")]
mod ribbon;
#[cfg(feature = "drafting")]
pub(crate) mod draft_canvas;
mod ai_analysis;

pub(crate) use material_swatches::draw_material_swatch;

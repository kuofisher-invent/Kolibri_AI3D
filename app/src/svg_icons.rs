//! SVG Icon Loader — 載入 docs/CAD_icons/ 下的 SVG → egui TextureHandle
//!
//! 使用 resvg 渲染 SVG → RGBA pixel buffer → egui ColorImage → TextureHandle
//! 快取機制：每個 icon 只載入一次

#[cfg(feature = "drafting")]
use eframe::egui;
#[cfg(feature = "drafting")]
use std::collections::HashMap;

/// SVG icon 快取管理器
#[cfg(feature = "drafting")]
pub(crate) struct SvgIconCache {
    textures: HashMap<String, egui::TextureHandle>,
    icon_dir: String,
    size: u32,
}

#[cfg(feature = "drafting")]
impl SvgIconCache {
    pub fn new() -> Self {
        // 嘗試找到 CAD_icons 目錄
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_default();

        let candidates = [
            "docs/CAD_icons".to_string(),
            format!("{}/docs/CAD_icons", exe_dir.display()),
            "../docs/CAD_icons".to_string(),
            "D:/AI_Design/Kolibri_Ai3D/docs/CAD_icons".to_string(),
        ];

        let icon_dir = candidates.iter()
            .find(|p| std::path::Path::new(p).is_dir())
            .cloned()
            .unwrap_or_else(|| "docs/CAD_icons".into());

        Self {
            textures: HashMap::new(),
            icon_dir,
            size: 64, // 64x64 渲染尺寸（高解析度）
        }
    }

    /// 取得 icon texture，如果尚未載入則載入
    pub fn get(&mut self, ctx: &egui::Context, name: &str) -> Option<egui::TextureId> {
        if let Some(tex) = self.textures.get(name) {
            return Some(tex.id());
        }

        // 嘗試載入 SVG
        let path = format!("{}/{}.svg", self.icon_dir, name);
        if let Some(tex) = load_svg_to_texture(ctx, &path, self.size) {
            let id = tex.id();
            self.textures.insert(name.to_string(), tex);
            Some(id)
        } else {
            None
        }
    }

    /// 預載所有 Ribbon 需要的 icon
    pub fn preload_ribbon_icons(&mut self, ctx: &egui::Context) {
        let icons = [
            "line", "arc", "circle", "rectangle", "polyline", "ellipse",
            "move", "rotate", "scale", "offset", "trim", "mirror", "array",
            "dim_linear", "dim_aligned", "dim_angle", "dim_radius", "dim_diameter",
            "text", "leader", "hatch",
            "zoomall", "zoomwin",
            "print", "print_pdf", "export",
            "copy", "delete", "edit",
            "layer", "grid", "snap",
        ];
        for name in &icons {
            let _ = self.get(ctx, name);
        }
    }
}

/// 載入單個 SVG 檔案並轉為 egui TextureHandle
#[cfg(feature = "drafting")]
fn load_svg_to_texture(
    ctx: &egui::Context,
    path: &str,
    size: u32,
) -> Option<egui::TextureHandle> {
    let svg_data = std::fs::read(path).ok()?;

    let opts = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(&svg_data, &opts).ok()?;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;

    // 計算縮放比例讓 SVG 適合目標尺寸
    let svg_size = tree.size();
    let sx = size as f32 / svg_size.width();
    let sy = size as f32 / svg_size.height();
    let scale = sx.min(sy);

    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // tiny_skia 輸出是 premultiplied RGBA，egui 需要 straight RGBA
    let pixels = pixmap.data();
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for chunk in pixels.chunks(4) {
        let a = chunk[3] as f32 / 255.0;
        if a > 0.0 {
            rgba.push((chunk[0] as f32 / a).min(255.0) as u8);
            rgba.push((chunk[1] as f32 / a).min(255.0) as u8);
            rgba.push((chunk[2] as f32 / a).min(255.0) as u8);
            rgba.push(chunk[3]);
        } else {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        }
    }

    let image = egui::ColorImage::from_rgba_unmultiplied([size as _, size as _], &rgba);
    let tex = ctx.load_texture(
        format!("svg_icon_{}", path),
        image,
        egui::TextureOptions::LINEAR,
    );
    Some(tex)
}

/// DraftTool → SVG icon name 對照表
#[cfg(feature = "drafting")]
pub(crate) fn tool_icon_name(tool: crate::editor::Tool) -> Option<&'static str> {
    use crate::editor::Tool;
    match tool {
        // 繪圖
        Tool::DraftLine => Some("line"),
        Tool::DraftArc => Some("arc"),
        Tool::DraftCircle => Some("circle"),
        Tool::DraftRectangle => Some("rectangle"),
        Tool::DraftPolyline => Some("polyline"),
        Tool::DraftEllipse => Some("ellipse"),
        // 修改
        Tool::DraftSelect => Some("edit"),
        Tool::DraftMove => Some("move"),
        Tool::DraftRotate => Some("rotate"),
        Tool::DraftScale => Some("scale"),
        Tool::DraftOffset => Some("offset"),
        Tool::DraftTrim => Some("trim"),
        Tool::DraftMirror => Some("mirror"),
        Tool::DraftArray => Some("array"),
        // 標註
        Tool::DraftDimLinear => Some("dim_linear"),
        Tool::DraftDimAligned => Some("dim_aligned"),
        Tool::DraftDimAngle => Some("dim_angle"),
        Tool::DraftDimRadius => Some("dim_radius"),
        Tool::DraftDimDiameter => Some("dim_diameter"),
        // 文字
        Tool::DraftText => Some("text"),
        Tool::DraftLeader => Some("leader"),
        // 填充
        Tool::DraftHatch => Some("hatch"),
        // 檢視
        Tool::DraftZoomAll => Some("zoomall"),
        Tool::DraftZoomWindow => Some("zoomwin"),
        Tool::DraftPan => Some("move"),
        // 輸出
        Tool::DraftPrint => Some("print"),
        Tool::DraftExportPdf => Some("print_pdf"),
        // Top 10 新增
        Tool::DraftCopy => Some("copy"),
        Tool::DraftFillet => Some("fillet"),
        Tool::DraftChamfer => Some("fillet"),  // 共用 fillet icon
        Tool::DraftExplode => Some("delete"),  // 暫用 delete icon
        Tool::DraftStretch => Some("stretch"),
        Tool::DraftExtend => Some("stretch"),  // 暫用 stretch icon
        Tool::DraftDimContinue => Some("dim_linear"),
        Tool::DraftDimBaseline => Some("dim_linear"),
        Tool::DraftPolygon => Some("polyline"),
        Tool::DraftSpline => Some("spline"),
        Tool::DraftBlock => Some("block_create"),
        Tool::DraftInsert => Some("block_insert"),
        Tool::DraftPoint => Some("point"),
        Tool::DraftXline => Some("line"),
        _ => None,
    }
}

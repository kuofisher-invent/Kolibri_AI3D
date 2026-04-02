//! Phase C: 鋼構施工圖引擎
//! 3D 模型 → 2D 正交投影（正視/側視/上視）
//! 自動標註、螺栓/焊接符號、單件圖/組裝圖/GA 圖

use crate::scene::{Scene, SceneObject, Shape};
use crate::collision::ComponentKind;
use crate::steel_connection::*;
use crate::steel_numbering::NumberingResult;

/// 投影方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionView {
    /// 正視圖（XY 平面，看 -Z 方向）
    Front,
    /// 側視圖（ZY 平面，看 -X 方向）
    Side,
    /// 上視圖（XZ 平面，看 -Y 方向）
    Top,
    /// 等角圖（Isometric）
    Isometric,
}

impl ProjectionView {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Front => "正視圖",
            Self::Side => "側視圖",
            Self::Top => "上視圖",
            Self::Isometric => "等角圖",
        }
    }
}

/// 2D 投影元素
#[derive(Debug, Clone)]
pub enum DrawingElement {
    /// 實線（可見邊）
    Line {
        start: [f32; 2],
        end: [f32; 2],
        thickness: f32,
    },
    /// 虛線（隱藏邊）
    HiddenLine {
        start: [f32; 2],
        end: [f32; 2],
    },
    /// 中心線（點劃線）
    CenterLine {
        start: [f32; 2],
        end: [f32; 2],
    },
    /// 線性標註
    Dimension {
        start: [f32; 2],
        end: [f32; 2],
        value_mm: f32,
        offset: f32, // 標註線偏移量
    },
    /// 文字標註
    Text {
        position: [f32; 2],
        text: String,
        size: f32,
        anchor: TextAnchor,
    },
    /// 螺栓符號（圓 + 十字）
    BoltSymbol {
        center: [f32; 2],
        diameter: f32,
    },
    /// 焊接符號（V 形 + 參考線）
    WeldSymbol {
        position: [f32; 2],
        weld_type: WeldType,
        size_mm: f32,
        length_mm: f32,
    },
    /// 構件編號標籤（氣泡）
    MarkBubble {
        center: [f32; 2],
        mark: String,
    },
    /// 剖面標記
    SectionCut {
        start: [f32; 2],
        end: [f32; 2],
        label: String,
    },
    /// 填充矩形（斷面）
    FilledRect {
        min: [f32; 2],
        max: [f32; 2],
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAnchor {
    Left,
    Center,
    Right,
}

/// 圖面類型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingType {
    /// 單件圖（Part Drawing）— 每個構件的加工圖
    PartDrawing,
    /// 組裝圖（Assembly Drawing）— 組裝件的組合圖
    AssemblyDrawing,
    /// GA 總裝圖（General Arrangement）— 整體結構配置
    GeneralArrangement,
    /// 錨栓佈置圖
    AnchorPlan,
}

impl DrawingType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PartDrawing => "單件圖",
            Self::AssemblyDrawing => "組裝圖",
            Self::GeneralArrangement => "GA 總裝圖",
            Self::AnchorPlan => "錨栓佈置圖",
        }
    }
}

/// 施工圖紙
#[derive(Debug, Clone)]
pub struct SteelDrawing {
    pub drawing_type: DrawingType,
    pub title: String,
    pub scale: f32,           // 比例尺 (e.g. 20.0 = 1:20)
    pub paper_width: f32,     // 紙張寬 mm (A3=420, A1=841)
    pub paper_height: f32,    // 紙張高 mm
    pub views: Vec<DrawingView>,
    /// 圖框標題欄資料
    pub title_block: TitleBlock,
}

/// 單一視圖（含投影方向 + 元素）
#[derive(Debug, Clone)]
pub struct DrawingView {
    pub projection: ProjectionView,
    pub label: String,
    pub elements: Vec<DrawingElement>,
    /// 視圖在圖紙上的位置 (mm)
    pub origin: [f32; 2],
    /// 視圖邊界
    pub bounds: [f32; 4], // [min_x, min_y, max_x, max_y]
}

/// 標題欄
#[derive(Debug, Clone)]
pub struct TitleBlock {
    pub project_name: String,
    pub drawing_number: String,
    pub revision: String,
    pub drawn_by: String,
    pub checked_by: String,
    pub date: String,
    pub scale_text: String,
    pub material: String,
    pub weight_kg: f32,
}

impl Default for TitleBlock {
    fn default() -> Self {
        Self {
            project_name: "Kolibri 鋼構專案".into(),
            drawing_number: "DWG-001".into(),
            revision: "R0".into(),
            drawn_by: "Kolibri Ai3D".into(),
            checked_by: String::new(),
            date: String::new(),
            scale_text: "1:20".into(),
            material: "SS400".into(),
            weight_kg: 0.0,
        }
    }
}

// ─── 投影邏輯 ──────────────────────────────────────────────────────────────────

/// 將 3D 點投影到 2D
pub fn project_point(p: [f32; 3], view: ProjectionView) -> [f32; 2] {
    match view {
        ProjectionView::Front => [p[0], p[1]],           // XY (看 -Z)
        ProjectionView::Side => [p[2], p[1]],            // ZY (看 -X)
        ProjectionView::Top => [p[0], p[2]],             // XZ (看 -Y)
        ProjectionView::Isometric => {
            // 30° 等角投影
            let cos30 = 0.866_f32;
            let sin30 = 0.5_f32;
            [
                cos30 * p[0] - cos30 * p[2],
                sin30 * p[0] + p[1] + sin30 * p[2],
            ]
        }
    }
}

/// 將 Box 投影為 2D 矩形的可見邊
fn project_box(
    pos: [f32; 3], w: f32, h: f32, d: f32,
    view: ProjectionView,
) -> Vec<DrawingElement> {
    let mut elements = Vec::new();

    // Box 的 8 個頂點
    let corners = [
        [pos[0],     pos[1],     pos[2]],
        [pos[0] + w, pos[1],     pos[2]],
        [pos[0] + w, pos[1] + h, pos[2]],
        [pos[0],     pos[1] + h, pos[2]],
        [pos[0],     pos[1],     pos[2] + d],
        [pos[0] + w, pos[1],     pos[2] + d],
        [pos[0] + w, pos[1] + h, pos[2] + d],
        [pos[0],     pos[1] + h, pos[2] + d],
    ];

    // 12 條邊（Box edges）
    let edges = [
        (0,1), (1,2), (2,3), (3,0), // front face
        (4,5), (5,6), (6,7), (7,4), // back face
        (0,4), (1,5), (2,6), (3,7), // connecting edges
    ];

    // 投影所有邊
    let projected: Vec<[f32; 2]> = corners.iter()
        .map(|c| project_point(*c, view))
        .collect();

    // 判斷可見邊 vs 隱藏邊（簡化：正交投影中，前 4 條邊可見，後 4 隱藏）
    let (visible, hidden) = match view {
        ProjectionView::Front => {
            // 正視圖：XY 平面，front face 可見
            let vis: Vec<usize> = vec![0,1,2,3]; // front face edges
            let hid: Vec<usize> = vec![4,5,6,7]; // back face edges
            (vis, hid)
        }
        ProjectionView::Side => {
            // 側視圖：ZY 平面
            let vis: Vec<usize> = vec![0,1,2,3,8,9,10,11];
            let hid: Vec<usize> = vec![4,5,6,7];
            (vis, hid)
        }
        ProjectionView::Top => {
            // 上視圖：XZ 平面
            let vis: Vec<usize> = vec![0,1,4,5,8,9,10,11];
            let hid: Vec<usize> = vec![2,3,6,7];
            (vis, hid)
        }
        _ => {
            let vis: Vec<usize> = (0..12).collect();
            (vis, vec![])
        }
    };

    for &ei in &visible {
        let (a, b) = edges[ei];
        elements.push(DrawingElement::Line {
            start: projected[a],
            end: projected[b],
            thickness: 0.35, // 可見線 0.35mm
        });
    }

    for &ei in &hidden {
        let (a, b) = edges[ei];
        // 投影後若與可見邊重疊則跳過
        let s = projected[edges[ei].0];
        let e = projected[edges[ei].1];
        if (s[0] - e[0]).abs() > 0.1 || (s[1] - e[1]).abs() > 0.1 {
            elements.push(DrawingElement::HiddenLine {
                start: s,
                end: e,
            });
        }
    }

    elements
}

/// 產生構件的單件圖
pub fn generate_part_drawing(
    obj: &SceneObject,
    mark: &str,
    connections: &[SteelConnection],
    numbering: &NumberingResult,
) -> SteelDrawing {
    let mut views = Vec::new();
    let scale = 20.0;

    match &obj.shape {
        Shape::Box { width, height, depth } => {
            // 正視圖
            let front_elems = project_box(obj.position, *width, *height, *depth, ProjectionView::Front);
            let mut front_view = DrawingView {
                projection: ProjectionView::Front,
                label: "正視圖".into(),
                elements: front_elems,
                origin: [50.0, 200.0],
                bounds: compute_bounds_2d(&obj.position, *width, *height, *depth, ProjectionView::Front),
            };
            // 自動標註尺寸
            add_box_dimensions(&mut front_view, obj.position, *width, *height, *depth, ProjectionView::Front);
            views.push(front_view);

            // 側視圖
            let side_elems = project_box(obj.position, *width, *height, *depth, ProjectionView::Side);
            let mut side_view = DrawingView {
                projection: ProjectionView::Side,
                label: "側視圖".into(),
                elements: side_elems,
                origin: [250.0, 200.0],
                bounds: compute_bounds_2d(&obj.position, *width, *height, *depth, ProjectionView::Side),
            };
            add_box_dimensions(&mut side_view, obj.position, *width, *height, *depth, ProjectionView::Side);
            views.push(side_view);

            // 上視圖
            let top_elems = project_box(obj.position, *width, *height, *depth, ProjectionView::Top);
            let mut top_view = DrawingView {
                projection: ProjectionView::Top,
                label: "上視圖".into(),
                elements: top_elems,
                origin: [50.0, 50.0],
                bounds: compute_bounds_2d(&obj.position, *width, *height, *depth, ProjectionView::Top),
            };
            add_box_dimensions(&mut top_view, obj.position, *width, *height, *depth, ProjectionView::Top);
            views.push(top_view);
        }
        _ => {}
    }

    // 加螺栓/焊接符號（如果此構件有相關接頭）
    for conn in connections {
        if conn.member_ids.iter().any(|id| numbering.marks.get(id).map_or(false, |m| m == mark)) {
            add_connection_symbols(&mut views, conn);
        }
    }

    // 構件編號氣泡
    if let Some(view) = views.first_mut() {
        let cx = (view.bounds[0] + view.bounds[2]) / 2.0;
        let cy = view.bounds[3] + 15.0;
        view.elements.push(DrawingElement::MarkBubble {
            center: [cx, cy],
            mark: mark.to_string(),
        });
    }

    // 計算重量
    let volume = match &obj.shape {
        Shape::Box { width, height, depth } => width * height * depth,
        Shape::Cylinder { radius, height, .. } => std::f32::consts::PI * radius * radius * height,
        _ => 0.0,
    };
    let weight = volume * 7.85e-6;

    SteelDrawing {
        drawing_type: DrawingType::PartDrawing,
        title: format!("單件圖 — {}", mark),
        scale,
        paper_width: 420.0,  // A3
        paper_height: 297.0,
        views,
        title_block: TitleBlock {
            drawing_number: format!("P-{}", mark),
            weight_kg: weight,
            scale_text: format!("1:{}", scale as i32),
            ..Default::default()
        },
    }
}

/// 產生 GA 總裝圖
pub fn generate_ga_drawing(
    scene: &Scene,
    numbering: &NumberingResult,
) -> SteelDrawing {
    let mut views = Vec::new();

    // 正視圖：所有構件
    let mut front_elems = Vec::new();
    for (id, obj) in &scene.objects {
        if !obj.visible { continue; }
        match obj.component_kind {
            ComponentKind::Column | ComponentKind::Beam | ComponentKind::Brace
            | ComponentKind::Plate => {}
            _ => continue,
        }
        if let Shape::Box { width, height, depth } = &obj.shape {
            front_elems.extend(project_box(obj.position, *width, *height, *depth, ProjectionView::Front));
        }
        // 編號氣泡
        if let Some(mark) = numbering.marks.get(id) {
            let center = project_point([
                obj.position[0] + match &obj.shape {
                    Shape::Box { width, .. } => width / 2.0,
                    _ => 0.0,
                },
                obj.position[1] + match &obj.shape {
                    Shape::Box { height, .. } => height / 2.0,
                    _ => 0.0,
                },
                obj.position[2],
            ], ProjectionView::Front);
            front_elems.push(DrawingElement::MarkBubble {
                center,
                mark: mark.clone(),
            });
        }
    }

    // 計算整體邊界
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for elem in &front_elems {
        match elem {
            DrawingElement::Line { start, end, .. } | DrawingElement::HiddenLine { start, end } => {
                min_x = min_x.min(start[0]).min(end[0]);
                min_y = min_y.min(start[1]).min(end[1]);
                max_x = max_x.max(start[0]).max(end[0]);
                max_y = max_y.max(start[1]).max(end[1]);
            }
            _ => {}
        }
    }

    views.push(DrawingView {
        projection: ProjectionView::Front,
        label: "正視圖".into(),
        elements: front_elems,
        origin: [50.0, 50.0],
        bounds: [min_x, min_y, max_x, max_y],
    });

    // 上視圖
    let mut top_elems = Vec::new();
    for obj in scene.objects.values() {
        if !obj.visible { continue; }
        match obj.component_kind {
            ComponentKind::Column | ComponentKind::Beam | ComponentKind::Brace => {}
            _ => continue,
        }
        if let Shape::Box { width, height, depth } = &obj.shape {
            top_elems.extend(project_box(obj.position, *width, *height, *depth, ProjectionView::Top));
        }
    }
    views.push(DrawingView {
        projection: ProjectionView::Top,
        label: "上視圖".into(),
        elements: top_elems,
        origin: [50.0, 300.0],
        bounds: [0.0; 4],
    });

    let scale = auto_scale(max_x - min_x, max_y - min_y, 841.0, 594.0); // A1

    SteelDrawing {
        drawing_type: DrawingType::GeneralArrangement,
        title: "GA 總裝圖".into(),
        scale,
        paper_width: 841.0,  // A1
        paper_height: 594.0,
        views,
        title_block: TitleBlock {
            drawing_number: "GA-001".into(),
            scale_text: format!("1:{}", scale as i32),
            ..Default::default()
        },
    }
}

/// 匯出施工圖為 DXF 格式（2D）
pub fn export_drawing_dxf(drawing: &SteelDrawing, path: &str) -> Result<(), String> {
    use std::io::Write;
    let mut f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let scale_inv = 1.0 / drawing.scale;

    // DXF header
    writeln!(f, "0\nSECTION\n2\nHEADER\n0\nENDSEC").map_err(|e| e.to_string())?;

    // Tables section (minimal)
    writeln!(f, "0\nSECTION\n2\nTABLES").map_err(|e| e.to_string())?;
    // Line types
    writeln!(f, "0\nTABLE\n2\nLTYPE\n70\n3").map_err(|e| e.to_string())?;
    writeln!(f, "0\nLTYPE\n2\nCONTINUOUS\n70\n0\n3\nSolid line\n72\n65\n73\n0\n40\n0.0")
        .map_err(|e| e.to_string())?;
    writeln!(f, "0\nLTYPE\n2\nHIDDEN\n70\n0\n3\nHidden\n72\n65\n73\n2\n40\n6.0\n49\n3.0\n49\n-3.0")
        .map_err(|e| e.to_string())?;
    writeln!(f, "0\nLTYPE\n2\nCENTER\n70\n0\n3\nCenter\n72\n65\n73\n4\n40\n20.0\n49\n12.0\n49\n-3.0\n49\n3.0\n49\n-3.0")
        .map_err(|e| e.to_string())?;
    writeln!(f, "0\nENDTAB").map_err(|e| e.to_string())?;
    // Layers
    writeln!(f, "0\nTABLE\n2\nLAYER\n70\n5").map_err(|e| e.to_string())?;
    for (name, color) in [("VISIBLE", 7), ("HIDDEN", 8), ("CENTER", 1), ("DIM", 3), ("TEXT", 7), ("SYMBOL", 5)] {
        writeln!(f, "0\nLAYER\n2\n{}\n70\n0\n62\n{}\n6\nCONTINUOUS", name, color)
            .map_err(|e| e.to_string())?;
    }
    writeln!(f, "0\nENDTAB\n0\nENDSEC").map_err(|e| e.to_string())?;

    // Entities
    writeln!(f, "0\nSECTION\n2\nENTITIES").map_err(|e| e.to_string())?;

    for view in &drawing.views {
        let ox = view.origin[0];
        let oy = view.origin[1];

        for elem in &view.elements {
            match elem {
                DrawingElement::Line { start, end, .. } => {
                    let x1 = ox + start[0] * scale_inv;
                    let y1 = oy + start[1] * scale_inv;
                    let x2 = ox + end[0] * scale_inv;
                    let y2 = oy + end[1] * scale_inv;
                    writeln!(f, "0\nLINE\n8\nVISIBLE\n10\n{:.3}\n20\n{:.3}\n11\n{:.3}\n21\n{:.3}",
                        x1, y1, x2, y2).map_err(|e| e.to_string())?;
                }
                DrawingElement::HiddenLine { start, end } => {
                    let x1 = ox + start[0] * scale_inv;
                    let y1 = oy + start[1] * scale_inv;
                    let x2 = ox + end[0] * scale_inv;
                    let y2 = oy + end[1] * scale_inv;
                    writeln!(f, "0\nLINE\n8\nHIDDEN\n6\nHIDDEN\n10\n{:.3}\n20\n{:.3}\n11\n{:.3}\n21\n{:.3}",
                        x1, y1, x2, y2).map_err(|e| e.to_string())?;
                }
                DrawingElement::Dimension { start, end, value_mm, offset } => {
                    // DIMENSION entity (linear)
                    let x1 = ox + start[0] * scale_inv;
                    let y1 = oy + start[1] * scale_inv;
                    let x2 = ox + end[0] * scale_inv;
                    let y2 = oy + end[1] * scale_inv;
                    let mx = (x1 + x2) / 2.0;
                    let my = (y1 + y2) / 2.0 + offset * scale_inv;
                    writeln!(f, "0\nDIMENSION\n8\nDIM\n70\n0\n10\n{:.3}\n20\n{:.3}\n13\n{:.3}\n23\n{:.3}\n14\n{:.3}\n24\n{:.3}\n1\n{:.0}",
                        mx, my, x1, y1, x2, y2, value_mm).map_err(|e| e.to_string())?;
                }
                DrawingElement::Text { position, text, size, .. } => {
                    let x = ox + position[0] * scale_inv;
                    let y = oy + position[1] * scale_inv;
                    writeln!(f, "0\nTEXT\n8\nTEXT\n10\n{:.3}\n20\n{:.3}\n40\n{:.1}\n1\n{}",
                        x, y, size, text).map_err(|e| e.to_string())?;
                }
                DrawingElement::BoltSymbol { center, diameter } => {
                    let cx = ox + center[0] * scale_inv;
                    let cy = oy + center[1] * scale_inv;
                    let r = diameter * scale_inv / 2.0;
                    writeln!(f, "0\nCIRCLE\n8\nSYMBOL\n10\n{:.3}\n20\n{:.3}\n40\n{:.3}",
                        cx, cy, r).map_err(|e| e.to_string())?;
                    // Cross
                    writeln!(f, "0\nLINE\n8\nSYMBOL\n10\n{:.3}\n20\n{:.3}\n11\n{:.3}\n21\n{:.3}",
                        cx - r, cy, cx + r, cy).map_err(|e| e.to_string())?;
                    writeln!(f, "0\nLINE\n8\nSYMBOL\n10\n{:.3}\n20\n{:.3}\n11\n{:.3}\n21\n{:.3}",
                        cx, cy - r, cx, cy + r).map_err(|e| e.to_string())?;
                }
                DrawingElement::MarkBubble { center, mark } => {
                    let cx = ox + center[0] * scale_inv;
                    let cy = oy + center[1] * scale_inv;
                    writeln!(f, "0\nCIRCLE\n8\nTEXT\n10\n{:.3}\n20\n{:.3}\n40\n5.0", cx, cy)
                        .map_err(|e| e.to_string())?;
                    writeln!(f, "0\nTEXT\n8\nTEXT\n10\n{:.3}\n20\n{:.3}\n40\n3.0\n72\n1\n1\n{}\n11\n{:.3}\n21\n{:.3}",
                        cx, cy - 1.5, mark, cx, cy - 1.5).map_err(|e| e.to_string())?;
                }
                _ => {}
            }
        }
    }

    // Title block
    draw_title_block_dxf(&mut f, &drawing.title_block, drawing.paper_width, drawing.paper_height)?;

    writeln!(f, "0\nENDSEC\n0\nEOF").map_err(|e| e.to_string())?;
    Ok(())
}

// ─── 輔助函式 ──────────────────────────────────────────────────────────────────

fn compute_bounds_2d(pos: &[f32; 3], w: f32, h: f32, d: f32, view: ProjectionView) -> [f32; 4] {
    let p0 = project_point(*pos, view);
    let p1 = project_point([pos[0] + w, pos[1] + h, pos[2] + d], view);
    [p0[0].min(p1[0]), p0[1].min(p1[1]), p0[0].max(p1[0]), p0[1].max(p1[1])]
}

fn add_box_dimensions(view: &mut DrawingView, pos: [f32; 3], w: f32, h: f32, d: f32, proj: ProjectionView) {
    let (dim_h, dim_w) = match proj {
        ProjectionView::Front => (h, w),
        ProjectionView::Side => (h, d),
        ProjectionView::Top => (d, w),
        ProjectionView::Isometric => return,
    };

    let p0 = project_point(pos, proj);

    // 水平標註（寬度）
    if dim_w > 1.0 {
        view.elements.push(DrawingElement::Dimension {
            start: [p0[0], p0[1]],
            end: [p0[0] + dim_w, p0[1]],
            value_mm: dim_w,
            offset: -15.0,
        });
    }
    // 垂直標註（高度）
    if dim_h > 1.0 {
        view.elements.push(DrawingElement::Dimension {
            start: [p0[0], p0[1]],
            end: [p0[0], p0[1] + dim_h],
            value_mm: dim_h,
            offset: -15.0,
        });
    }
}

fn add_connection_symbols(views: &mut [DrawingView], conn: &SteelConnection) {
    let front_view = views.iter_mut().find(|v| v.projection == ProjectionView::Front);
    if let Some(view) = front_view {
        // 螺栓符號
        for bg in &conn.bolts {
            for bp in &bg.positions {
                let p2d = project_point(
                    [conn.position[0] + bp[0], conn.position[1] + bp[1], conn.position[2] + bp[2]],
                    ProjectionView::Front,
                );
                view.elements.push(DrawingElement::BoltSymbol {
                    center: p2d,
                    diameter: bg.bolt_size.hole_diameter(),
                });
            }
        }
        // 焊接符號
        for weld in &conn.welds {
            let p2d = project_point(
                [conn.position[0] + weld.start[0], conn.position[1] + weld.start[1], conn.position[2] + weld.start[2]],
                ProjectionView::Front,
            );
            view.elements.push(DrawingElement::WeldSymbol {
                position: p2d,
                weld_type: weld.weld_type,
                size_mm: weld.size,
                length_mm: weld.length,
            });
        }
    }
}

fn auto_scale(model_w: f32, model_h: f32, paper_w: f32, paper_h: f32) -> f32 {
    let margin = 60.0; // 圖框邊距
    let avail_w = paper_w - 2.0 * margin;
    let avail_h = paper_h - 2.0 * margin;
    let scale_w = model_w / avail_w;
    let scale_h = model_h / avail_h;
    let raw = scale_w.max(scale_h).max(1.0);
    // 取標準比例尺
    let standards = [1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0, 200.0, 500.0];
    *standards.iter().find(|&&s| s >= raw).unwrap_or(&100.0)
}

fn draw_title_block_dxf(
    f: &mut std::fs::File,
    tb: &TitleBlock,
    pw: f32, ph: f32,
) -> Result<(), String> {
    use std::io::Write;
    // 外框
    let m = 10.0;
    writeln!(f, "0\nLINE\n8\nVISIBLE\n10\n{:.1}\n20\n{:.1}\n11\n{:.1}\n21\n{:.1}", m, m, pw-m, m).map_err(|e| e.to_string())?;
    writeln!(f, "0\nLINE\n8\nVISIBLE\n10\n{:.1}\n20\n{:.1}\n11\n{:.1}\n21\n{:.1}", pw-m, m, pw-m, ph-m).map_err(|e| e.to_string())?;
    writeln!(f, "0\nLINE\n8\nVISIBLE\n10\n{:.1}\n20\n{:.1}\n11\n{:.1}\n21\n{:.1}", pw-m, ph-m, m, ph-m).map_err(|e| e.to_string())?;
    writeln!(f, "0\nLINE\n8\nVISIBLE\n10\n{:.1}\n20\n{:.1}\n11\n{:.1}\n21\n{:.1}", m, ph-m, m, m).map_err(|e| e.to_string())?;

    // 標題欄（右下角）
    let tbx = pw - m - 170.0;
    let tby = m;
    let tbw = 170.0;
    let tbh = 50.0;
    writeln!(f, "0\nLINE\n8\nVISIBLE\n10\n{:.1}\n20\n{:.1}\n11\n{:.1}\n21\n{:.1}", tbx, tby, tbx+tbw, tby).map_err(|e| e.to_string())?;
    writeln!(f, "0\nLINE\n8\nVISIBLE\n10\n{:.1}\n20\n{:.1}\n11\n{:.1}\n21\n{:.1}", tbx+tbw, tby, tbx+tbw, tby+tbh).map_err(|e| e.to_string())?;
    writeln!(f, "0\nLINE\n8\nVISIBLE\n10\n{:.1}\n20\n{:.1}\n11\n{:.1}\n21\n{:.1}", tbx+tbw, tby+tbh, tbx, tby+tbh).map_err(|e| e.to_string())?;
    writeln!(f, "0\nLINE\n8\nVISIBLE\n10\n{:.1}\n20\n{:.1}\n11\n{:.1}\n21\n{:.1}", tbx, tby+tbh, tbx, tby).map_err(|e| e.to_string())?;

    // 文字
    writeln!(f, "0\nTEXT\n8\nTEXT\n10\n{:.1}\n20\n{:.1}\n40\n4.0\n1\n{}", tbx+5.0, tby+38.0, tb.project_name).map_err(|e| e.to_string())?;
    writeln!(f, "0\nTEXT\n8\nTEXT\n10\n{:.1}\n20\n{:.1}\n40\n3.0\n1\n{}", tbx+5.0, tby+28.0, tb.drawing_number).map_err(|e| e.to_string())?;
    writeln!(f, "0\nTEXT\n8\nTEXT\n10\n{:.1}\n20\n{:.1}\n40\n2.5\n1\nScale: {}", tbx+5.0, tby+18.0, tb.scale_text).map_err(|e| e.to_string())?;
    writeln!(f, "0\nTEXT\n8\nTEXT\n10\n{:.1}\n20\n{:.1}\n40\n2.5\n1\nMaterial: {}", tbx+5.0, tby+10.0, tb.material).map_err(|e| e.to_string())?;
    writeln!(f, "0\nTEXT\n8\nTEXT\n10\n{:.1}\n20\n{:.1}\n40\n2.5\n1\nWeight: {:.1} kg", tbx+5.0, tby+3.0, tb.weight_kg).map_err(|e| e.to_string())?;
    writeln!(f, "0\nTEXT\n8\nTEXT\n10\n{:.1}\n20\n{:.1}\n40\n2.5\n1\n{} {}", tbx+100.0, tby+18.0, tb.drawn_by, tb.revision).map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::MaterialKind;

    #[test]
    fn test_projection() {
        let p = [100.0, 200.0, 300.0];
        assert_eq!(project_point(p, ProjectionView::Front), [100.0, 200.0]);
        assert_eq!(project_point(p, ProjectionView::Side), [300.0, 200.0]);
        assert_eq!(project_point(p, ProjectionView::Top), [100.0, 300.0]);
    }

    #[test]
    fn test_box_projection_generates_lines() {
        let elems = project_box([0.0; 3], 100.0, 200.0, 50.0, ProjectionView::Front);
        assert!(!elems.is_empty());
        let line_count = elems.iter().filter(|e| matches!(e, DrawingElement::Line { .. })).count();
        assert!(line_count >= 4); // 至少 4 條可見邊
    }

    #[test]
    fn test_part_drawing_generation() {
        let obj = SceneObject {
            id: "test".into(), name: "COL_1".into(),
            shape: Shape::Box { width: 150.0, height: 4200.0, depth: 150.0 },
            position: [0.0; 3], material: MaterialKind::Steel,
            rotation_y: 0.0, rotation_xyz: [0.0; 3], tag: "鋼構".into(), visible: true,
            roughness: 0.5, metallic: 0.0, texture_path: None,
            component_kind: ComponentKind::Column, parent_id: None,
            component_def_id: None, locked: false, obj_version: 0, base_level_idx: None, top_level_idx: None,
        };
        let numbering = crate::steel_numbering::auto_number(&Scene::default());
        let dwg = generate_part_drawing(&obj, "C1", &[], &numbering);
        assert_eq!(dwg.drawing_type, DrawingType::PartDrawing);
        assert!(!dwg.views.is_empty());
        assert!(dwg.views[0].elements.len() > 4); // 有邊線+標註
    }

    #[test]
    fn test_auto_scale() {
        assert_eq!(auto_scale(5000.0, 3000.0, 420.0, 297.0), 20.0);
        assert_eq!(auto_scale(50000.0, 30000.0, 841.0, 594.0), 100.0);
    }
}

//! Phase D: DSTV NC1 格式輸出
//! Deutscher Stahlbau-Verband 標準 — CNC 鑽孔機/切割機對接
//! 參考: DSTV NC 格式規範（DIN 1025 / EN 10365）

use std::io::Write;
use kolibri_core::scene::{Scene, Shape};
use kolibri_core::collision::ComponentKind;
use kolibri_core::steel_connection::*;
use kolibri_core::steel_numbering::NumberingResult;

/// NC 程式碼（一個構件 = 一個 .nc1 檔）
#[derive(Debug, Clone)]
pub struct NcProgram {
    pub mark: String,           // 構件編號
    pub profile: String,        // 截面規格
    pub material: String,       // 材質
    pub length: f32,            // 構件長度 mm
    pub quantity: u32,          // 數量
    pub operations: Vec<NcOperation>,
}

/// NC 加工操作
#[derive(Debug, Clone)]
pub enum NcOperation {
    /// 鑽孔
    Hole {
        face: NcFace,       // 加工面
        x: f32,             // 沿長度方向位置 mm
        y: f32,             // 面上偏移 mm
        diameter: f32,      // 孔徑 mm
    },
    /// 切割線（端部輪廓）
    Cut {
        face: NcFace,
        x: f32,
        angle_deg: f32,     // 切割角度
    },
    /// 標記（劃線）
    Mark {
        face: NcFace,
        x: f32,
        y: f32,
        text: String,
    },
    /// 缺口（Cope/Notch）
    Notch {
        face: NcFace,
        x: f32,
        width: f32,
        depth: f32,
    },
}

/// DSTV 面代號
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NcFace {
    /// v = 上翼板（top flange）
    TopFlange,
    /// u = 下翼板（bottom flange）
    BottomFlange,
    /// o = 腹板上方（web top）
    WebTop,
    /// h = 腹板（web）
    Web,
    /// s = 左翼板側（stiffener side）
    LeftSide,
    /// r = 右翼板側
    RightSide,
}

impl NcFace {
    pub fn dstv_code(&self) -> char {
        match self {
            Self::TopFlange => 'v',
            Self::BottomFlange => 'u',
            Self::WebTop => 'o',
            Self::Web => 'h',
            Self::LeftSide => 's',
            Self::RightSide => 'r',
        }
    }
}

/// 從場景構件產生 NC 程式
pub fn generate_nc_programs(
    scene: &Scene,
    connections: &[SteelConnection],
    numbering: &NumberingResult,
) -> Vec<NcProgram> {
    let mut programs = Vec::new();

    // 對每個有編號的構件產生 NC 程式
    let mut processed_marks = std::collections::HashSet::new();

    for (id, obj) in &scene.objects {
        if !obj.visible { continue; }
        let mark = match numbering.marks.get(id) {
            Some(m) => m.clone(),
            None => continue,
        };

        // 只處理主要結構件
        match obj.component_kind {
            ComponentKind::Column | ComponentKind::Beam | ComponentKind::Brace => {}
            _ => continue,
        }

        // 同一編號只產生一次
        if !processed_marks.insert(mark.clone()) { continue; }

        let (length, profile) = match &obj.shape {
            Shape::Box { width, height, depth } => {
                let l = width.max(*height).max(*depth);
                (l, obj.name.clone())
            }
            _ => continue,
        };

        let quantity = numbering.mark_counts.get(&mark).copied().unwrap_or(1);

        // 收集此構件的孔位（來自相關接頭）
        let mut operations = Vec::new();

        for conn in connections {
            if !conn.member_ids.contains(id) { continue; }

            // 將螺栓位置轉為 NC 鑽孔操作
            for bg in &conn.bolts {
                for bp in &bg.positions {
                    // 判斷孔在哪個面
                    let (face, x, y) = classify_hole_face(bp, obj);
                    operations.push(NcOperation::Hole {
                        face,
                        x,
                        y,
                        diameter: bg.hole_diameter,
                    });
                }
            }
        }

        // 端部切割（90° 直切）
        operations.push(NcOperation::Cut {
            face: NcFace::Web,
            x: 0.0,
            angle_deg: 90.0,
        });
        operations.push(NcOperation::Cut {
            face: NcFace::Web,
            x: length,
            angle_deg: 90.0,
        });

        programs.push(NcProgram {
            mark,
            profile,
            material: "SS400".into(),
            length,
            quantity,
            operations,
        });
    }

    programs
}

/// 判斷孔位屬於哪個面
fn classify_hole_face(bp: &[f32; 3], _obj: &kolibri_core::scene::SceneObject) -> (NcFace, f32, f32) {
    // 簡化邏輯：根據 Y 座標判斷上/下翼板，否則腹板
    if bp[1] > 0.0 {
        (NcFace::TopFlange, bp[0], bp[2])
    } else if bp[1] < 0.0 {
        (NcFace::BottomFlange, bp[0], bp[2])
    } else {
        (NcFace::Web, bp[0], bp[1])
    }
}

/// 匯出單一 NC1 檔案
pub fn export_nc1(program: &NcProgram, path: &str) -> Result<(), String> {
    let mut f = std::fs::File::create(path).map_err(|e| e.to_string())?;

    // DSTV NC1 header
    // ST = Start, 接構件資訊
    writeln!(f, "ST").map_err(|e| e.to_string())?;
    // 訂單號
    writeln!(f, "  Kolibri-NC").map_err(|e| e.to_string())?;
    // 圖號
    writeln!(f, "  {}", program.mark).map_err(|e| e.to_string())?;
    // 備註
    writeln!(f, "  ").map_err(|e| e.to_string())?;
    // 構件編號
    writeln!(f, "  {}", program.mark).map_err(|e| e.to_string())?;
    // 材質
    writeln!(f, "  {}", program.material).map_err(|e| e.to_string())?;
    // 數量
    writeln!(f, "  {}", program.quantity).map_err(|e| e.to_string())?;
    // 截面規格
    writeln!(f, "  {}", program.profile).map_err(|e| e.to_string())?;
    // 長度
    writeln!(f, "  {:.1}", program.length).map_err(|e| e.to_string())?;

    // 加工操作
    for op in &program.operations {
        match op {
            NcOperation::Hole { face, x, y, diameter } => {
                // BO = Boring (鑽孔)
                writeln!(f, "BO").map_err(|e| e.to_string())?;
                writeln!(f, "  {} {:.1} {:.1} {:.1}", face.dstv_code(), x, y, diameter)
                    .map_err(|e| e.to_string())?;
            }
            NcOperation::Cut { face, x, angle_deg } => {
                // SC = Saw Cut (切割)
                writeln!(f, "SC").map_err(|e| e.to_string())?;
                writeln!(f, "  {} {:.1} {:.1}", face.dstv_code(), x, angle_deg)
                    .map_err(|e| e.to_string())?;
            }
            NcOperation::Mark { face, x, y, text } => {
                // SI = Scribing (劃線)
                writeln!(f, "SI").map_err(|e| e.to_string())?;
                writeln!(f, "  {} {:.1} {:.1} {}", face.dstv_code(), x, y, text)
                    .map_err(|e| e.to_string())?;
            }
            NcOperation::Notch { face, x, width, depth } => {
                // AK = Ausklinkung (缺口)
                writeln!(f, "AK").map_err(|e| e.to_string())?;
                writeln!(f, "  {} {:.1} {:.1} {:.1}", face.dstv_code(), x, width, depth)
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    // EN = End
    writeln!(f, "EN").map_err(|e| e.to_string())?;

    Ok(())
}

/// 批次匯出所有 NC 到資料夾
pub fn export_all_nc(
    programs: &[NcProgram],
    output_dir: &str,
) -> Result<usize, String> {
    std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;
    let mut count = 0;
    for prog in programs {
        let path = format!("{}/{}.nc1", output_dir, prog.mark);
        export_nc1(prog, &path)?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kolibri_core::scene::MaterialKind;

    #[test]
    fn test_nc_program_generation() {
        let mut scene = kolibri_core::scene::Scene::default();
        let b1 = scene.insert_box_raw("BM_1".into(), [0.0, 4200.0, 0.0], 6000.0, 300.0, 9.0, MaterialKind::Steel);
        scene.objects.get_mut(&b1).unwrap().component_kind = ComponentKind::Beam;

        let numbering = kolibri_core::steel_numbering::auto_number(&scene);
        let connections = vec![SteelConnection {
            id: "c1".into(),
            conn_type: ConnectionType::EndPlate,
            member_ids: vec![b1.clone()],
            plates: vec![], bolts: vec![BoltGroup {
                bolt_size: BoltSize::M20, bolt_grade: BoltGrade::F10T,
                rows: 2, cols: 2, row_spacing: 70.0, col_spacing: 80.0,
                edge_dist: 34.0, hole_diameter: 22.0,
                positions: vec![[0.0, 100.0, 0.0], [0.0, -100.0, 0.0]],
            }],
            welds: vec![], position: [0.0; 3], group_id: None,
        }];

        let programs = generate_nc_programs(&scene, &connections, &numbering);
        assert!(!programs.is_empty());
        let prog = &programs[0];
        assert!(prog.length > 0.0);
        // 至少有 2 個切割 + 2 個鑽孔
        assert!(prog.operations.len() >= 4);
    }
}

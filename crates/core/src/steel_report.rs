//! Phase B: 鋼構報表系統
//! 完整 BOM + 螺栓表 + 焊接表 + 組裝件清單
//! 輸出 CSV（UTF-8 BOM for Excel 相容）

use std::io::Write;
use std::collections::HashMap;
use crate::collision::ComponentKind;
use crate::scene::{Scene, Shape, MaterialKind};
use crate::steel_connection::*;
use crate::steel_numbering::{auto_number, NumberingResult};

/// 報表類型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportType {
    /// 材料表（截面/長度/數量/單重/小計）
    MaterialList,
    /// 螺栓表（尺寸/等級/數量/位置）
    BoltSchedule,
    /// 焊接表（類型/尺寸/長度）
    WeldSchedule,
    /// 組裝件清單（組裝件編號/構件清單/總重）
    AssemblyList,
    /// 全部合併
    FullReport,
}

/// 單一材料表行
#[derive(Debug, Clone)]
pub struct MaterialRow {
    pub mark: String,         // 構件編號 (C1, B2...)
    pub kind: String,         // 類型 (柱/梁/撐/板)
    pub profile: String,      // 截面規格
    pub length_mm: f32,       // 長度
    pub width_mm: f32,        // 寬度
    pub height_mm: f32,       // 高度/深度
    pub material_grade: String, // 材質等級
    pub quantity: u32,        // 數量
    pub unit_weight_kg: f32,  // 單件重量
    pub total_weight_kg: f32, // 小計重量
}

/// 螺栓表行
#[derive(Debug, Clone)]
pub struct BoltRow {
    pub size: String,         // M20, M24...
    pub grade: String,        // F10T, A325...
    pub quantity: u32,        // 數量
    pub location: String,     // 位置描述
    pub conn_type: String,    // 接頭類型
}

/// 焊接表行
#[derive(Debug, Clone)]
pub struct WeldRow {
    pub weld_type: String,    // 角焊/全滲透/半滲透
    pub size_mm: f32,         // 焊腳尺寸
    pub length_mm: f32,       // 焊接長度
    pub location: String,     // 位置
}

/// 組裝件行
#[derive(Debug, Clone)]
pub struct AssemblyRow {
    pub asm_mark: String,     // A1, A2...
    pub members: Vec<String>, // 構件編號列表
    pub total_weight_kg: f32, // 組裝件總重
}

/// 完整報表資料
#[derive(Debug, Clone)]
pub struct SteelReport {
    pub material_rows: Vec<MaterialRow>,
    pub bolt_rows: Vec<BoltRow>,
    pub weld_rows: Vec<WeldRow>,
    pub assembly_rows: Vec<AssemblyRow>,
    pub numbering: NumberingResult,
    /// 全部總重 (kg)
    pub grand_total_weight: f32,
    /// 全部螺栓總數
    pub total_bolt_count: u32,
    /// 全部焊接總長 (mm)
    pub total_weld_length: f32,
}

/// 從場景產生完整報表
pub fn generate_report(
    scene: &Scene,
    connections: &[SteelConnection],
) -> SteelReport {
    let numbering = auto_number(scene);
    let mut material_rows = Vec::new();
    let mut bolt_rows = Vec::new();
    let mut weld_rows = Vec::new();
    let mut assembly_rows = Vec::new();

    // ── 材料表 ──
    // 按編號分組，計算每個編號的重量
    let mut mark_data: HashMap<String, MaterialRow> = HashMap::new();

    for (id, obj) in &scene.objects {
        if !obj.visible { continue; }
        let mark = numbering.marks.get(id).cloned().unwrap_or_default();
        if mark.is_empty() { continue; }

        let (w, h, d) = match &obj.shape {
            Shape::Box { width, height, depth } => (*width, *height, *depth),
            Shape::Cylinder { radius, height, .. } => (*radius * 2.0, *height, *radius * 2.0),
            _ => continue,
        };

        let kind = match obj.component_kind {
            ComponentKind::Column => "柱",
            ComponentKind::Beam => "梁",
            ComponentKind::Brace => "斜撐",
            ComponentKind::Plate => "鋼板",
            ComponentKind::Bolt => "螺栓",
            ComponentKind::Weld => "焊接",
            _ => "其他",
        };

        // 估算重量（鋼材密度 7850 kg/m³）
        let volume_mm3 = match &obj.shape {
            Shape::Box { width, height, depth } => width * height * depth,
            Shape::Cylinder { radius, height, .. } => {
                std::f32::consts::PI * radius * radius * height
            }
            _ => 0.0,
        };
        let weight_kg = volume_mm3 * 7.85e-6; // mm³ → m³ × 7850 kg/m³

        let material_grade = match obj.material {
            MaterialKind::Steel => "SS400".to_string(),
            MaterialKind::Metal => "SS400".to_string(),
            _ => format!("{:?}", obj.material),
        };

        let entry = mark_data.entry(mark.clone()).or_insert_with(|| MaterialRow {
            mark: mark.clone(),
            kind: kind.to_string(),
            profile: obj.name.clone(),
            length_mm: h.max(w).max(d),
            width_mm: w.min(d),
            height_mm: h,
            material_grade,
            quantity: 0,
            unit_weight_kg: weight_kg,
            total_weight_kg: 0.0,
        });
        entry.quantity += 1;
        entry.total_weight_kg += weight_kg;
    }

    material_rows = mark_data.into_values().collect();
    material_rows.sort_by(|a, b| a.mark.cmp(&b.mark));

    // ── 螺栓表 ──
    let mut bolt_map: HashMap<String, BoltRow> = HashMap::new();
    for conn in connections {
        for bg in &conn.bolts {
            let key = format!("{}_{}", bg.bolt_size.label(), bg.bolt_grade.label());
            let entry = bolt_map.entry(key.clone()).or_insert_with(|| BoltRow {
                size: bg.bolt_size.label().to_string(),
                grade: bg.bolt_grade.label().to_string(),
                quantity: 0,
                location: conn.conn_type.label().to_string(),
                conn_type: conn.conn_type.label().to_string(),
            });
            entry.quantity += bg.positions.len() as u32;
        }
    }
    bolt_rows = bolt_map.into_values().collect();
    bolt_rows.sort_by(|a, b| a.size.cmp(&b.size));

    // ── 焊接表 ──
    let mut weld_map: HashMap<String, WeldRow> = HashMap::new();
    for conn in connections {
        for weld in &conn.welds {
            let key = format!("{}_S{:.0}", weld.weld_type.label(), weld.size);
            let entry = weld_map.entry(key).or_insert_with(|| WeldRow {
                weld_type: weld.weld_type.label().to_string(),
                size_mm: weld.size,
                length_mm: 0.0,
                location: conn.conn_type.label().to_string(),
            });
            entry.length_mm += weld.length;
        }
    }
    weld_rows = weld_map.into_values().collect();

    // ── 組裝件 ──
    for (asm_mark, members) in &numbering.assemblies {
        let total_weight: f32 = members.iter()
            .filter_map(|m| material_rows.iter().find(|r| r.mark == *m))
            .map(|r| r.unit_weight_kg)
            .sum();
        assembly_rows.push(AssemblyRow {
            asm_mark: asm_mark.clone(),
            members: members.clone(),
            total_weight_kg: total_weight,
        });
    }
    assembly_rows.sort_by(|a, b| a.asm_mark.cmp(&b.asm_mark));

    let grand_total_weight = material_rows.iter().map(|r| r.total_weight_kg).sum();
    let total_bolt_count = bolt_rows.iter().map(|r| r.quantity).sum();
    let total_weld_length = weld_rows.iter().map(|r| r.length_mm).sum();

    SteelReport {
        material_rows,
        bolt_rows,
        weld_rows,
        assembly_rows,
        numbering,
        grand_total_weight,
        total_bolt_count,
        total_weld_length,
    }
}

/// 匯出完整報表到 CSV
pub fn export_report_csv(report: &SteelReport, path: &str) -> Result<(), String> {
    let mut f = std::fs::File::create(path).map_err(|e| e.to_string())?;

    // UTF-8 BOM
    write!(f, "\u{FEFF}").map_err(|e| e.to_string())?;

    // ── 材料表 ──
    writeln!(f, "=== 材料表 (Material List) ===").map_err(|e| e.to_string())?;
    writeln!(f, "編號,類型,名稱,長度(mm),寬度(mm),高度(mm),材質,數量,單重(kg),小計(kg)")
        .map_err(|e| e.to_string())?;
    for row in &report.material_rows {
        writeln!(f, "{},{},{},{:.0},{:.0},{:.0},{},{},{:.1},{:.1}",
            row.mark, row.kind, row.profile,
            row.length_mm, row.width_mm, row.height_mm,
            row.material_grade, row.quantity,
            row.unit_weight_kg, row.total_weight_kg,
        ).map_err(|e| e.to_string())?;
    }
    writeln!(f, "總重,,,,,,,,, {:.1} kg", report.grand_total_weight)
        .map_err(|e| e.to_string())?;
    writeln!(f).map_err(|e| e.to_string())?;

    // ── 螺栓表 ──
    writeln!(f, "=== 螺栓表 (Bolt Schedule) ===").map_err(|e| e.to_string())?;
    writeln!(f, "尺寸,等級,數量,接頭類型").map_err(|e| e.to_string())?;
    for row in &report.bolt_rows {
        writeln!(f, "{},{},{},{}", row.size, row.grade, row.quantity, row.conn_type)
            .map_err(|e| e.to_string())?;
    }
    writeln!(f, "螺栓總數,,,{}", report.total_bolt_count).map_err(|e| e.to_string())?;
    writeln!(f).map_err(|e| e.to_string())?;

    // ── 焊接表 ──
    writeln!(f, "=== 焊接表 (Weld Schedule) ===").map_err(|e| e.to_string())?;
    writeln!(f, "焊接類型,焊腳(mm),總長度(mm),位置").map_err(|e| e.to_string())?;
    for row in &report.weld_rows {
        writeln!(f, "{},{:.0},{:.0},{}", row.weld_type, row.size_mm, row.length_mm, row.location)
            .map_err(|e| e.to_string())?;
    }
    writeln!(f, "焊接總長,,{:.0} mm,", report.total_weld_length).map_err(|e| e.to_string())?;
    writeln!(f).map_err(|e| e.to_string())?;

    // ── 組裝件清單 ──
    writeln!(f, "=== 組裝件清單 (Assembly List) ===").map_err(|e| e.to_string())?;
    writeln!(f, "組裝件編號,構件編號,總重(kg)").map_err(|e| e.to_string())?;
    for row in &report.assembly_rows {
        writeln!(f, "{},{},{:.1}", row.asm_mark, row.members.join("+"), row.total_weight_kg)
            .map_err(|e| e.to_string())?;
    }

    // ── 統計摘要 ──
    writeln!(f).map_err(|e| e.to_string())?;
    writeln!(f, "=== 統計摘要 ===").map_err(|e| e.to_string())?;
    let s = &report.numbering.stats;
    writeln!(f, "柱數,{}", s.total_columns).map_err(|e| e.to_string())?;
    writeln!(f, "梁數,{}", s.total_beams).map_err(|e| e.to_string())?;
    writeln!(f, "斜撐,{}", s.total_braces).map_err(|e| e.to_string())?;
    writeln!(f, "鋼板,{}", s.total_plates).map_err(|e| e.to_string())?;
    writeln!(f, "不同編號,{}", s.unique_marks).map_err(|e| e.to_string())?;
    writeln!(f, "組裝件,{}", s.assembly_count).map_err(|e| e.to_string())?;
    writeln!(f, "螺栓總數,{}", report.total_bolt_count).map_err(|e| e.to_string())?;
    writeln!(f, "焊接總長,{:.0} mm", report.total_weld_length).map_err(|e| e.to_string())?;
    writeln!(f, "鋼材總重,{:.1} kg", report.grand_total_weight).map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_report() {
        let mut scene = Scene::default();
        let c1 = scene.insert_box_raw("COL_1".into(), [0.0; 3], 150.0, 4200.0, 9.0, MaterialKind::Steel);
        scene.objects.get_mut(&c1).unwrap().component_kind = ComponentKind::Column;
        let b1 = scene.insert_box_raw("BM_1".into(), [0.0, 4200.0, 0.0], 6000.0, 300.0, 9.0, MaterialKind::Steel);
        scene.objects.get_mut(&b1).unwrap().component_kind = ComponentKind::Beam;

        let connections = vec![
            SteelConnection {
                id: "test_conn".into(),
                conn_type: ConnectionType::EndPlate,
                member_ids: vec![c1.clone(), b1.clone()],
                plates: vec![],
                bolts: vec![BoltGroup {
                    bolt_size: BoltSize::M20,
                    bolt_grade: BoltGrade::F10T,
                    rows: 2, cols: 2,
                    row_spacing: 70.0, col_spacing: 80.0,
                    edge_dist: 34.0, hole_diameter: 22.0,
                    positions: vec![[0.0;3]; 4],
                }],
                welds: vec![WeldLine {
                    weld_type: WeldType::Fillet,
                    size: 8.0, length: 300.0,
                    start: [0.0;3], end: [300.0, 0.0, 0.0],
                }],
                position: [0.0; 3],
                group_id: None,
            },
        ];

        let report = generate_report(&scene, &connections);
        assert!(!report.material_rows.is_empty());
        assert_eq!(report.total_bolt_count, 4);
        assert!(report.total_weld_length > 0.0);
        assert!(report.grand_total_weight > 0.0);
    }
}

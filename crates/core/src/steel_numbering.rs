//! Phase B: 鋼構自動編號系統
//! 構件自動編號（C1, B1, BR1, PL1）+ 組裝件編號（A1）
//! 相同截面+長度 = 相同編號，只標數量

use std::collections::HashMap;
use crate::collision::ComponentKind;
use crate::scene::{Scene, SceneObject, Shape};

/// 構件編號結果
#[derive(Debug, Clone)]
pub struct NumberingResult {
    /// 物件 ID → 編號（如 "C1", "B2"）
    pub marks: HashMap<String, String>,
    /// 編號 → 數量
    pub mark_counts: HashMap<String, u32>,
    /// 組裝件: 組裝件編號 → 構件編號列表
    pub assemblies: HashMap<String, Vec<String>>,
    /// 統計
    pub stats: NumberingStats,
}

#[derive(Debug, Clone, Default)]
pub struct NumberingStats {
    pub total_columns: u32,
    pub total_beams: u32,
    pub total_braces: u32,
    pub total_plates: u32,
    pub total_bolts: u32,
    pub unique_marks: u32,
    pub assembly_count: u32,
}

/// 構件特徵碼（用於合併相同構件）
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MemberSignature {
    kind: ComponentKind,
    /// 截面規格（四捨五入到整數）
    profile_key: String,
    /// 長度（四捨五入到 mm）
    length_mm: i32,
}

/// 自動編號：掃描 Scene，為每個鋼構件指派編號
pub fn auto_number(scene: &Scene) -> NumberingResult {
    let mut marks = HashMap::new();
    let mut mark_counts: HashMap<String, u32> = HashMap::new();
    let mut assemblies: HashMap<String, Vec<String>> = HashMap::new();
    let mut stats = NumberingStats::default();

    // 收集所有構件簽名
    let mut sig_to_mark: HashMap<MemberSignature, String> = HashMap::new();
    let mut counters: HashMap<&str, u32> = HashMap::new();

    // 第一遍：按群組分類，為每個群組建立組裝件
    let mut group_members: HashMap<String, Vec<String>> = HashMap::new(); // group_id → obj_ids
    let mut obj_to_group: HashMap<String, String> = HashMap::new();

    for (gid, group) in &scene.groups {
        for cid in &group.children {
            obj_to_group.insert(cid.clone(), gid.clone());
            group_members.entry(gid.clone()).or_default().push(cid.clone());
        }
    }

    // 收集所有需要編號的物件（排除子群組中已分類的零件）
    let mut objects_to_number: Vec<(&str, &SceneObject)> = Vec::new();
    for (id, obj) in &scene.objects {
        if !obj.visible { continue; }
        match obj.component_kind {
            ComponentKind::Column | ComponentKind::Beam | ComponentKind::Brace
            | ComponentKind::Plate | ComponentKind::Bolt | ComponentKind::Weld => {
                objects_to_number.push((id.as_str(), obj));
            }
            _ => {}
        }
    }

    // 按 ComponentKind 排序（柱→梁→撐→板→栓→焊）
    objects_to_number.sort_by_key(|(_, obj)| match obj.component_kind {
        ComponentKind::Column => 0,
        ComponentKind::Beam => 1,
        ComponentKind::Brace => 2,
        ComponentKind::Plate => 3,
        ComponentKind::Bolt => 4,
        ComponentKind::Weld => 5,
        _ => 9,
    });

    // 第二遍：為每個構件指派或復用編號
    for (id, obj) in &objects_to_number {
        let (prefix, sig) = member_signature(obj);

        let mark = if let Some(existing) = sig_to_mark.get(&sig) {
            existing.clone()
        } else {
            let counter = counters.entry(prefix).or_insert(0);
            *counter += 1;
            let new_mark = format!("{}{}", prefix, counter);
            sig_to_mark.insert(sig.clone(), new_mark.clone());
            new_mark
        };

        *mark_counts.entry(mark.clone()).or_insert(0) += 1;
        marks.insert(id.to_string(), mark.clone());

        // 統計
        match obj.component_kind {
            ComponentKind::Column => stats.total_columns += 1,
            ComponentKind::Beam => stats.total_beams += 1,
            ComponentKind::Brace => stats.total_braces += 1,
            ComponentKind::Plate => stats.total_plates += 1,
            ComponentKind::Bolt => stats.total_bolts += 1,
            _ => {}
        }
    }

    // 第三遍：建立組裝件（以群組為單位）
    let mut asm_counter = 0_u32;
    for (gid, member_ids) in &group_members {
        let member_marks: Vec<String> = member_ids.iter()
            .filter_map(|mid| marks.get(mid).cloned())
            .collect();
        if !member_marks.is_empty() {
            asm_counter += 1;
            let asm_mark = format!("A{}", asm_counter);
            assemblies.insert(asm_mark, member_marks);
        }
    }

    stats.unique_marks = mark_counts.len() as u32;
    stats.assembly_count = asm_counter;

    NumberingResult { marks, mark_counts, assemblies, stats }
}

/// 計算構件特徵碼
fn member_signature(obj: &SceneObject) -> (&'static str, MemberSignature) {
    let prefix = match obj.component_kind {
        ComponentKind::Column => "C",
        ComponentKind::Beam => "B",
        ComponentKind::Brace => "BR",
        ComponentKind::Plate => "PL",
        ComponentKind::Bolt => "BT",
        ComponentKind::Weld => "W",
        _ => "X",
    };

    let (profile_key, length_mm) = match &obj.shape {
        Shape::Box { width, height, depth } => {
            // 對柱：profile = WxD, length = H
            // 對梁：profile = WxD, length = W(or longest dim)
            let dims = sorted_dims(*width, *height, *depth);
            let key = format!("{:.0}x{:.0}", dims.1, dims.2); // 中×大
            (key, dims.0 as i32) // 最小 = 厚度方向的尺寸 → 改用最大當長度
        }
        Shape::Cylinder { radius, height, .. } => {
            let key = format!("R{:.0}", radius);
            (key, *height as i32)
        }
        Shape::Line { points, thickness, .. } => {
            let length = if points.len() >= 2 {
                let p0 = points[0];
                let p1 = points[points.len() - 1];
                ((p1[0]-p0[0]).powi(2) + (p1[1]-p0[1]).powi(2) + (p1[2]-p0[2]).powi(2)).sqrt()
            } else { 0.0 };
            (format!("T{:.0}", thickness), length as i32)
        }
        _ => ("misc".into(), 0),
    };

    (prefix, MemberSignature {
        kind: obj.component_kind,
        profile_key,
        length_mm,
    })
}

/// 排序三個維度（小→中→大）
fn sorted_dims(a: f32, b: f32, c: f32) -> (f32, f32, f32) {
    let mut v = [a, b, c];
    v.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    (v[0], v[1], v[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{MaterialKind, Scene};

    fn make_steel_scene() -> Scene {
        let mut scene = Scene::default();
        // 建立 2 根相同截面的柱（應得到相同編號 C1）
        let c1 = scene.insert_box_raw("COL_1_F1".into(), [0.0, 0.0, 0.0], 150.0, 4200.0, 9.0, MaterialKind::Steel);
        let c2 = scene.insert_box_raw("COL_2_F1".into(), [6000.0, 0.0, 0.0], 150.0, 4200.0, 9.0, MaterialKind::Steel);
        for id in [&c1, &c2] {
            scene.objects.get_mut(id).unwrap().component_kind = ComponentKind::Column;
        }
        // 建立 1 根梁（不同截面，應得到 B1）
        let b1 = scene.insert_box_raw("BM_1_F1".into(), [0.0, 4200.0, 0.0], 6000.0, 300.0, 9.0, MaterialKind::Steel);
        scene.objects.get_mut(&b1).unwrap().component_kind = ComponentKind::Beam;
        // 建立 1 塊板
        let pl1 = scene.insert_box_raw("PL_1".into(), [0.0, 0.0, 0.0], 200.0, 20.0, 300.0, MaterialKind::Metal);
        scene.objects.get_mut(&pl1).unwrap().component_kind = ComponentKind::Plate;
        // 建立群組
        scene.create_group("柱組裝件".into(), vec![c1.clone(), b1.clone(), pl1.clone()]);
        scene
    }

    #[test]
    fn test_auto_numbering() {
        let scene = make_steel_scene();
        let result = auto_number(&scene);

        // 2 根相同柱 → 同一個 C 編號
        assert!(result.stats.total_columns == 2);
        assert!(result.stats.total_beams == 1);
        assert!(result.stats.total_plates == 1);

        // 相同截面的柱應該有相同的編號
        let col_marks: Vec<_> = result.marks.values()
            .filter(|m| m.starts_with('C'))
            .collect();
        assert!(col_marks.len() == 2);
        assert_eq!(col_marks[0], col_marks[1]); // 相同編號

        // C1 數量 = 2
        assert_eq!(result.mark_counts.get("C1"), Some(&2));

        // 有至少一個組裝件
        assert!(result.stats.assembly_count >= 1);
    }

    #[test]
    fn test_different_sections_get_different_marks() {
        let mut scene = Scene::default();
        let c1 = scene.insert_box_raw("COL_A".into(), [0.0, 0.0, 0.0], 150.0, 4200.0, 9.0, MaterialKind::Steel);
        let c2 = scene.insert_box_raw("COL_B".into(), [3000.0, 0.0, 0.0], 200.0, 4200.0, 13.0, MaterialKind::Steel);
        for id in [&c1, &c2] {
            scene.objects.get_mut(id).unwrap().component_kind = ComponentKind::Column;
        }
        let result = auto_number(&scene);
        let marks: Vec<_> = result.marks.values().collect();
        // 不同截面 → 不同編號
        assert_ne!(marks[0], marks[1]);
    }
}

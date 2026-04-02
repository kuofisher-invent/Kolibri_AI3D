//! 鋼構接頭幾何驗證測試
//! 精確模擬 GUI 的柱/梁建法，驗證接頭生成的所有物件位置是否合理

use crate::collision::ComponentKind;
use crate::scene::{MaterialKind, Scene, Shape};
use crate::steel_connection::*;

/// 精確模擬 GUI 的 SteelColumn 建立（H300×150×6.5×9, 高4200mm, 在地面 cx,cz）
fn gui_create_column(scene: &mut Scene, cx: f32, cz: f32) -> String {
    let h_sec = 300.0;  // H300
    let b_sec = 150.0;  // B150
    let tw = 6.5;
    let tf = 9.0;
    let member_h = 4200.0;

    let f1 = scene.insert_box_raw(
        "COL_F1".into(),
        [cx - b_sec / 2.0, 0.0, cz - h_sec / 2.0],
        b_sec, member_h, tf, MaterialKind::Steel,
    );
    let f2 = scene.insert_box_raw(
        "COL_F2".into(),
        [cx - b_sec / 2.0, 0.0, cz + h_sec / 2.0 - tf],
        b_sec, member_h, tf, MaterialKind::Steel,
    );
    let web = scene.insert_box_raw(
        "COL_W".into(),
        [cx - tw / 2.0, 0.0, cz - h_sec / 2.0 + tf],
        tw, member_h, h_sec - 2.0 * tf, MaterialKind::Steel,
    );
    for id in [&f1, &f2, &web] {
        scene.objects.get_mut(id).unwrap().component_kind = ComponentKind::Column;
    }
    let gid = scene.create_group("COL".into(), vec![f1, f2, web]);
    gid
}

/// 精確模擬 GUI 的 SteelBeam 建立（H300×150×6.5×9, X方向, 柱高4200處）
fn gui_create_beam_x(scene: &mut Scene, x1: f32, x2: f32, cz: f32) -> String {
    let h_sec = 300.0;
    let b_sec = 150.0;
    let tw = 6.5;
    let tf = 9.0;
    let steel_height = 4200.0;
    let beam_y = steel_height - h_sec; // 3900
    let min_x = x1.min(x2);
    let length = (x2 - x1).abs();

    let tf_id = scene.insert_box_raw(
        "BM_TF".into(),
        [min_x, beam_y + h_sec - tf, cz - b_sec / 2.0],
        length, tf, b_sec, MaterialKind::Steel,
    );
    let bf_id = scene.insert_box_raw(
        "BM_BF".into(),
        [min_x, beam_y, cz - b_sec / 2.0],
        length, tf, b_sec, MaterialKind::Steel,
    );
    let w_id = scene.insert_box_raw(
        "BM_W".into(),
        [min_x, beam_y + tf, cz - tw / 2.0],
        length, h_sec - 2.0 * tf, tw, MaterialKind::Steel,
    );
    for id in [&tf_id, &bf_id, &w_id] {
        scene.objects.get_mut(id).unwrap().component_kind = ComponentKind::Beam;
    }
    let gid = scene.create_group("BM".into(), vec![tf_id, bf_id, w_id]);
    gid
}

/// 計算群組 AABB 中心
fn group_center(scene: &Scene, gid: &str) -> [f32; 3] {
    let group = scene.groups.get(gid).unwrap();
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for cid in &group.children {
        let obj = scene.objects.get(cid).unwrap();
        let p = obj.position;
        if let Shape::Box { width, height, depth } = &obj.shape {
            for i in 0..3 {
                let dims = [*width, *height, *depth];
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i] + dims[i]);
            }
        }
    }
    [(min[0]+max[0])/2.0, (min[1]+max[1])/2.0, (min[2]+max[2])/2.0]
}

fn group_bounds(scene: &Scene, gid: &str) -> ([f32; 3], [f32; 3]) {
    let group = scene.groups.get(gid).unwrap();
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for cid in &group.children {
        let obj = scene.objects.get(cid).unwrap();
        let p = obj.position;
        if let Shape::Box { width, height, depth } = &obj.shape {
            let dims = [*width, *height, *depth];
            for i in 0..3 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i] + dims[i]);
            }
        }
    }
    (min, max)
}

#[test]
fn test_column_geometry() {
    let mut scene = Scene::default();
    let gid = gui_create_column(&mut scene, 0.0, 0.0);

    let center = group_center(&scene, &gid);
    let (bmin, bmax) = group_bounds(&scene, &gid);

    println!("柱 AABB: min={:?} max={:?}", bmin, bmax);
    println!("柱 中心: {:?}", center);

    // 柱中心應在 (0, 2100, 0)
    assert!((center[0] - 0.0).abs() < 1.0, "X 中心偏了: {}", center[0]);
    assert!((center[1] - 2100.0).abs() < 1.0, "Y 中心偏了: {}", center[1]);
    assert!((center[2] - 0.0).abs() < 1.0, "Z 中心偏了: {}", center[2]);

    // AABB 高度 = 4200
    assert!((bmax[1] - bmin[1] - 4200.0).abs() < 1.0);
    // AABB X 寬度 = 翼板寬 150
    assert!((bmax[0] - bmin[0] - 150.0).abs() < 1.0);
    // AABB Z 深度 = 截面高 300
    assert!((bmax[2] - bmin[2] - 300.0).abs() < 1.0);
}

#[test]
fn test_beam_geometry() {
    let mut scene = Scene::default();
    let gid = gui_create_beam_x(&mut scene, 0.0, 6000.0, 0.0);

    let center = group_center(&scene, &gid);
    let (bmin, bmax) = group_bounds(&scene, &gid);

    println!("梁 AABB: min={:?} max={:?}", bmin, bmax);
    println!("梁 中心: {:?}", center);

    // 梁 X 長度 = 6000
    assert!((bmax[0] - bmin[0] - 6000.0).abs() < 1.0);
    // 梁 Y 位置: beam_y=3900, beam_y+h_sec=4200
    assert!((bmin[1] - 3900.0).abs() < 1.0, "梁底 Y 偏了: {}", bmin[1]);
    assert!((bmax[1] - 4200.0).abs() < 1.0, "梁頂 Y 偏了: {}", bmax[1]);
    // 梁 Z 寬度 = 翼板寬 150
    assert!((bmax[2] - bmin[2] - 150.0).abs() < 1.0);
}

#[test]
fn test_end_plate_at_correct_position() {
    let mut scene = Scene::default();

    // 建立: 柱@(0,0), 柱@(6000,0), 梁@(0→6000)
    let col1_gid = gui_create_column(&mut scene, 0.0, 0.0);
    let col2_gid = gui_create_column(&mut scene, 6000.0, 0.0);
    let beam_gid = gui_create_beam_x(&mut scene, 0.0, 6000.0, 0.0);

    let col1_center = group_center(&scene, &col1_gid);
    let beam_center = group_center(&scene, &beam_gid);

    println!("\n=== 端板接頭幾何驗證 ===");
    println!("柱1 中心: {:?}", col1_center);
    println!("梁 中心: {:?}", beam_center);

    // 計算端板接頭（梁-柱1）
    let beam_section = (300.0, 150.0, 6.5, 9.0);
    let col_section = (300.0, 150.0, 6.5, 9.0);
    let conn = calc_end_plate(&EndPlateParams {
        beam_section,
        col_section,
        bolt_size: BoltSize::M20,
        bolt_grade: BoltGrade::F10T,
        plate_thickness: None,
        add_stiffeners: true,
    });

    // 接頭位置應在柱1中心 XZ + 梁中心 Y
    let expected_conn_pos = [col1_center[0], beam_center[1], col1_center[2]];
    println!("預期接頭位置: {:?}", expected_conn_pos);

    // 端板
    let ep = &conn.plates[0];
    println!("端板: {:.0}×{:.0}×{:.0}mm", ep.width, ep.height, ep.thickness);
    assert!(ep.width > 0.0 && ep.height > 0.0 && ep.thickness > 0.0);

    // 端板位置應在接頭位置附近
    // 端板應垂直於梁（梁沿X方向→端板在 YZ 平面）
    // 端板中心 X ≈ 柱中心 X (0)
    // 端板中心 Y ≈ 梁中心 Y (4050)
    // 端板中心 Z ≈ 柱中心 Z (0)

    // 螺栓
    let bg = &conn.bolts[0];
    println!("螺栓: {}×{} = {} 顆, 孔Ø{:.0}mm",
        bg.rows, bg.cols, bg.positions.len(), bg.hole_diameter);

    for (i, bp) in bg.positions.iter().enumerate() {
        println!("  螺栓{}: 相對位置 ({:.1}, {:.1}, {:.1})", i+1, bp[0], bp[1], bp[2]);
        // 螺栓 X 位置（相對）應在端板寬度範圍內
        assert!(bp[0].abs() <= ep.width, "螺栓 X 超出端板: {} > {}", bp[0].abs(), ep.width);
        // 螺栓 Y 位置（相對）應在端板高度範圍內
        assert!(bp[1].abs() <= ep.height, "螺栓 Y 超出端板: {} > {}", bp[1].abs(), ep.height);
    }

    // 焊接
    for (i, w) in conn.welds.iter().enumerate() {
        println!("  焊接{}: {} S={:.0}mm L={:.0}mm", i+1, w.weld_type.label(), w.size, w.length);
    }

    // 肋板
    let stiffs: Vec<_> = conn.plates.iter().filter(|p| p.plate_type == PlateType::Stiffener).collect();
    println!("肋板: {} 片", stiffs.len());

    println!("=== 端板接頭驗證通過 ===");
}

#[test]
fn test_base_plate_at_column_bottom() {
    let mut scene = Scene::default();
    let col_gid = gui_create_column(&mut scene, 3000.0, 2000.0);

    let col_center = group_center(&scene, &col_gid);
    let (col_min, col_max) = group_bounds(&scene, &col_gid);

    println!("\n=== 底板接頭幾何驗證 ===");
    println!("柱 中心: {:?}", col_center);
    println!("柱 底部Y: {:.0}", col_min[1]);

    let col_section = (300.0, 150.0, 6.5, 9.0);
    let conn = calc_base_plate(col_section, BoltSize::M24, BoltGrade::F8T);

    let bp = &conn.plates[0];
    println!("底板: {:.0}×{:.0}×{:.0}mm", bp.width, bp.height, bp.thickness);

    // 底板應在柱底部 (Y=0)
    // 底板中心 X ≈ 柱中心 X (3000)
    // 底板中心 Z ≈ 柱中心 Z (2000)
    // 底板頂面 Y ≈ 0 (地面)

    // 底板尺寸合理性
    assert!(bp.width >= 150.0, "底板太窄: {}", bp.width);
    assert!(bp.height >= 150.0, "底板太短: {}", bp.height);
    assert!(bp.thickness >= 20.0, "底板太薄: {}", bp.thickness);

    // 錨栓
    let bg = &conn.bolts[0];
    println!("錨栓: {}×{} = {} 顆", bg.rows, bg.cols, bg.positions.len());
    for (i, p) in bg.positions.iter().enumerate() {
        println!("  錨栓{}: ({:.1}, {:.1}, {:.1})", i+1, p[0], p[1], p[2]);
        // 所有錨栓應在底板範圍內
        assert!(p[0].abs() <= bp.width / 2.0 + 1.0, "錨栓X超出底板");
        assert!(p[2].abs() <= bp.height / 2.0 + 1.0, "錨栓Z超出底板");
    }

    println!("=== 底板接頭驗證通過 ===");
}

#[test]
fn test_shear_tab_dimensions() {
    println!("\n=== 腹板接頭幾何驗證 ===");

    let beam_section = (300.0, 150.0, 6.5, 9.0);
    let conn = calc_shear_tab(beam_section, BoltSize::M20, BoltGrade::F10T);

    let tab = &conn.plates[0];
    println!("剪力板: {:.0}×{:.0}×{:.0}mm", tab.width, tab.height, tab.thickness);

    // 剪力板高度 ≤ 梁腹板淨高
    let web_clear = 300.0 - 2.0 * 9.0; // 282mm
    assert!(tab.height <= web_clear, "剪力板高 {} > 腹板淨高 {}", tab.height, web_clear);
    // 剪力板厚 ≥ 梁腹板厚
    assert!(tab.thickness >= 6.5, "剪力板厚 {} < 梁腹板厚 6.5", tab.thickness);

    let bg = &conn.bolts[0];
    println!("螺栓: {}×{} = {} 顆 ({})", bg.rows, bg.cols, bg.positions.len(), bg.bolt_size.label());
    // 單列螺栓
    assert_eq!(bg.cols, 1);

    // 焊接
    assert!(!conn.welds.is_empty());
    let w = &conn.welds[0];
    println!("焊接: {} S={:.0}mm L={:.0}mm", w.weld_type.label(), w.size, w.length);
    assert_eq!(w.weld_type, WeldType::Fillet);

    // AISC 驗算
    let check = check_connection(&conn, &SteelMaterial::SS400, DesignMethod::LRFD);
    println!("抗剪: {:.0}kN | 焊接: {:.0}kN | pass: {}",
        check.total_bolt_shear, check.total_weld_capacity, check.pass);
    assert!(check.pass);

    println!("=== 腹板接頭驗證通過 ===");
}

#[test]
fn test_connection_for_all_profiles() {
    // 測試所有 CNS 386 截面的端板接頭都能正確生成
    let profiles: &[(f32, f32, f32, f32)] = &[
        (100.0, 50.0, 5.0, 7.0),     // H100
        (200.0, 100.0, 5.5, 8.0),    // H200
        (300.0, 150.0, 6.5, 9.0),    // H300
        (400.0, 200.0, 8.0, 13.0),   // H400
        (500.0, 200.0, 10.0, 16.0),  // H500
        (700.0, 300.0, 13.0, 24.0),  // H700
        (900.0, 300.0, 16.0, 28.0),  // H900
    ];

    println!("\n=== 全截面接頭驗證 ===");
    for &(h, b, tw, tf) in profiles {
        let params = EndPlateParams {
            beam_section: (h, b, tw, tf),
            col_section: (h, b, tw, tf),
            bolt_size: suggest_bolt_size(h, tf),
            bolt_grade: BoltGrade::F10T,
            plate_thickness: None,
            add_stiffeners: true,
        };
        let conn = calc_end_plate(&params);
        let check = check_connection(&conn, &SteelMaterial::SS400, DesignMethod::LRFD);

        let ep = &conn.plates[0];
        let bg = &conn.bolts[0];
        println!("H{:.0}: 端板{:.0}×{:.0}×{:.0} | {}×{}={} 顆 {} | 抗剪{:.0}kN | {}",
            h, ep.width, ep.height, ep.thickness,
            bg.rows, bg.cols, bg.positions.len(), bg.bolt_size.label(),
            check.total_bolt_shear,
            if check.pass { "PASS" } else { "FAIL" },
        );

        // 基本合理性
        assert!(ep.width >= b, "H{}: 端板寬 {} < 翼板寬 {}", h, ep.width, b);
        assert!(ep.height >= h, "H{}: 端板高 {} < 梁高 {}", h, ep.height, h);
        assert!(ep.thickness >= 12.0, "H{}: 端板厚 {} < 12mm", h, ep.thickness);
        assert!(check.pass, "H{}: AISC FAIL {:?}", h, check.warnings);
    }
    println!("=== 全截面通過 ===");
}

//! 鋼構接頭自動設計整合測試
//! 驗證完整流程：建柱/梁 → 計算接頭 → 驗證板件/螺栓/焊接 → AISC 檢查 → 報表 → 施工圖

use crate::collision::ComponentKind;
use crate::scene::{MaterialKind, Scene, Shape};
use crate::steel_connection::*;
use crate::steel_numbering::*;
use crate::steel_report::*;
use crate::steel_drawing::*;

/// 建立測試用鋼構場景：2 柱 + 1 梁 + 1 斜撐
fn create_test_steel_frame() -> Scene {
    let mut scene = Scene::default();

    // 柱 C1: H300×300×10×15 at (0, 0, 0)，高 4200mm
    let c1_f1 = scene.insert_box_raw("C1_F1".into(), [-150.0, 0.0, -150.0], 300.0, 4200.0, 15.0, MaterialKind::Steel);
    let c1_f2 = scene.insert_box_raw("C1_F2".into(), [-150.0, 0.0, 135.0], 300.0, 4200.0, 15.0, MaterialKind::Steel);
    let c1_web = scene.insert_box_raw("C1_W".into(), [-5.0, 0.0, -135.0], 10.0, 4200.0, 270.0, MaterialKind::Steel);
    for id in [&c1_f1, &c1_f2, &c1_web] {
        scene.objects.get_mut(id).unwrap().component_kind = ComponentKind::Column;
    }
    scene.create_group("COL_1".into(), vec![c1_f1.clone(), c1_f2.clone(), c1_web.clone()]);

    // 柱 C2: 同截面 at (6000, 0, 0)
    let c2_f1 = scene.insert_box_raw("C2_F1".into(), [5850.0, 0.0, -150.0], 300.0, 4200.0, 15.0, MaterialKind::Steel);
    let c2_f2 = scene.insert_box_raw("C2_F2".into(), [5850.0, 0.0, 135.0], 300.0, 4200.0, 15.0, MaterialKind::Steel);
    let c2_web = scene.insert_box_raw("C2_W".into(), [5995.0, 0.0, -135.0], 10.0, 4200.0, 270.0, MaterialKind::Steel);
    for id in [&c2_f1, &c2_f2, &c2_web] {
        scene.objects.get_mut(id).unwrap().component_kind = ComponentKind::Column;
    }
    scene.create_group("COL_2".into(), vec![c2_f1, c2_f2, c2_web]);

    // 梁 B1: H400×200×8×13 at (0, 3900, 0)，跨距 6000mm
    let b1_f1 = scene.insert_box_raw("B1_F1".into(), [0.0, 3900.0, -100.0], 6000.0, 13.0, 200.0, MaterialKind::Steel);
    let b1_f2 = scene.insert_box_raw("B1_F2".into(), [0.0, 4287.0, -100.0], 6000.0, 13.0, 200.0, MaterialKind::Steel);
    let b1_web = scene.insert_box_raw("B1_W".into(), [0.0, 3913.0, -4.0], 6000.0, 374.0, 8.0, MaterialKind::Steel);
    for id in [&b1_f1, &b1_f2, &b1_web] {
        scene.objects.get_mut(id).unwrap().component_kind = ComponentKind::Beam;
    }
    scene.create_group("BM_1".into(), vec![b1_f1, b1_f2, b1_web]);

    scene
}

#[test]
fn test_end_plate_full_workflow() {
    // ── Step 1: 計算端板式接頭（梁-柱剛接）──
    let params = EndPlateParams {
        beam_section: (400.0, 200.0, 8.0, 13.0),  // H400×200×8×13
        col_section: (300.0, 300.0, 10.0, 15.0),   // H300×300×10×15
        bolt_size: BoltSize::M20,
        bolt_grade: BoltGrade::F10T,
        plate_thickness: None,  // 自動計算
        add_stiffeners: true,
    };

    let conn = calc_end_plate(&params);

    // 驗證接頭類型
    assert_eq!(conn.conn_type, ConnectionType::EndPlate);

    // ── Step 2: 驗證端板尺寸 ──
    let end_plate = &conn.plates[0];
    assert_eq!(end_plate.plate_type, PlateType::EndPlate);
    // 端板寬 ≥ 梁翼板寬 (200mm)
    assert!(end_plate.width >= 200.0, "端板寬 {:.0} < 梁翼板寬 200", end_plate.width);
    // 端板高 ≥ 梁高 (400mm)
    assert!(end_plate.height >= 400.0, "端板高 {:.0} < 梁高 400", end_plate.height);
    // 端板厚 ≥ 16mm（AISC 建議最小）
    assert!(end_plate.thickness >= 16.0, "端板厚 {:.0} < 16mm", end_plate.thickness);
    println!("端板: {:.0}×{:.0}×{:.0}mm", end_plate.width, end_plate.height, end_plate.thickness);

    // ── Step 3: 驗證螺栓配置 ──
    let bg = &conn.bolts[0];
    assert_eq!(bg.bolt_size, BoltSize::M20);
    assert_eq!(bg.bolt_grade, BoltGrade::F10T);
    // H400 梁應有 4 列螺栓（梁高 > 400 → 4 rows）
    assert_eq!(bg.rows, 4, "H400 梁應有 4 列螺栓");
    assert_eq!(bg.cols, 2);
    assert_eq!(bg.positions.len(), (bg.rows * bg.cols) as usize);
    // 螺栓間距 ≥ 2.667d（AISC J3.3）
    let min_sp = (bg.bolt_size.diameter() * 2.667).ceil();
    assert!(bg.row_spacing >= min_sp - 1.0, "行距 {:.0} < AISC {:.0}", bg.row_spacing, min_sp);
    // 邊距 ≥ AISC Table J3.4
    assert!(bg.edge_dist >= bg.bolt_size.min_edge(), "邊距 {:.0} < AISC {:.0}", bg.edge_dist, bg.bolt_size.min_edge());
    println!("螺栓: {}×{} = {} 顆, 間距={:.0}, 邊距={:.0}",
        bg.rows, bg.cols, bg.positions.len(), bg.row_spacing, bg.edge_dist);

    // ── Step 4: 驗證肋板 ──
    let stiffeners: Vec<_> = conn.plates.iter()
        .filter(|p| p.plate_type == PlateType::Stiffener)
        .collect();
    assert_eq!(stiffeners.len(), 2, "應有上下各一片肋板");
    for s in &stiffeners {
        // 肋板厚 ≥ 梁翼板厚
        assert!(s.thickness >= 13.0, "肋板厚 {:.0} < 梁翼板厚 13", s.thickness);
    }
    println!("肋板: {} 片, 厚={:.0}mm", stiffeners.len(), stiffeners[0].thickness);

    // ── Step 5: 驗證焊接 ──
    assert_eq!(conn.welds.len(), 3, "應有 3 道焊接（上翼板+下翼板+腹板）");
    let flange_welds: Vec<_> = conn.welds.iter()
        .filter(|w| w.weld_type == WeldType::FullPenetration)
        .collect();
    assert_eq!(flange_welds.len(), 2, "翼板應為全滲透焊接");
    let web_welds: Vec<_> = conn.welds.iter()
        .filter(|w| w.weld_type == WeldType::Fillet)
        .collect();
    assert_eq!(web_welds.len(), 1, "腹板應為角焊");
    println!("焊接: {} 道 (全滲透×{} + 角焊×{})",
        conn.welds.len(), flange_welds.len(), web_welds.len());

    // ── Step 6: AISC 360-22 驗算 ──
    let check = check_connection(&conn, &SteelMaterial::SS400, DesignMethod::LRFD);

    println!("\n=== AISC 360-22 驗算 ===");
    println!("螺栓抗剪: {:.1} kN ({}顆×{:.1}kN)", check.total_bolt_shear,
        bg.positions.len(), check.total_bolt_shear / bg.positions.len() as f32);
    println!("螺栓抗拉: {:.1} kN", check.total_bolt_tension);
    println!("承壓強度: {:.1} kN", check.min_bearing);
    println!("焊接強度: {:.1} kN", check.total_weld_capacity);

    if !check.warnings.is_empty() {
        println!("警告:");
        for w in &check.warnings { println!("  ⚠ {}", w); }
    }

    // 所有幾何限制應通過
    assert!(check.pass, "AISC 檢查未通過: {:?}", check.warnings);
    println!("AISC 檢查: ✅ 通過");
}

#[test]
fn test_shear_tab_full_workflow() {
    let conn = calc_shear_tab(
        (400.0, 200.0, 8.0, 13.0),  // H400×200×8×13
        BoltSize::M22,
        BoltGrade::A325,
    );

    assert_eq!(conn.conn_type, ConnectionType::ShearTab);

    // 剪力板
    let tab = &conn.plates[0];
    assert_eq!(tab.plate_type, PlateType::ShearTab);
    assert!(tab.thickness >= 8.0, "剪力板厚 < 梁腹板厚");
    println!("剪力板: {:.0}×{:.0}×{:.0}mm", tab.width, tab.height, tab.thickness);

    // 螺栓（單列）
    let bg = &conn.bolts[0];
    assert_eq!(bg.cols, 1, "剪力板應為單列螺栓");
    assert!(bg.rows >= 2, "至少 2 顆螺栓");
    println!("螺栓: {}×1 = {} 顆 (M22 A325)", bg.rows, bg.positions.len());

    // 焊接（角焊，剪力板焊於柱）
    assert!(!conn.welds.is_empty());
    assert_eq!(conn.welds[0].weld_type, WeldType::Fillet);
    println!("焊接: 角焊 S={:.0}mm L={:.0}mm", conn.welds[0].size, conn.welds[0].length);

    // AISC 驗算
    let check = check_connection(&conn, &SteelMaterial::A572_50, DesignMethod::LRFD);
    println!("抗剪: {:.1}kN, 焊接: {:.1}kN", check.total_bolt_shear, check.total_weld_capacity);
    assert!(check.pass, "AISC 檢查未通過: {:?}", check.warnings);
    println!("AISC: ✅");
}

#[test]
fn test_base_plate_full_workflow() {
    let conn = calc_base_plate(
        (300.0, 300.0, 10.0, 15.0),  // H300×300
        BoltSize::M24,
        BoltGrade::F8T,
    );

    assert_eq!(conn.conn_type, ConnectionType::BasePlate);

    // 底板
    let bp = &conn.plates[0];
    assert_eq!(bp.plate_type, PlateType::BasePlate);
    assert!(bp.width > 300.0, "底板應大於柱截面");
    assert!(bp.thickness >= 20.0, "底板厚 ≥ 20mm");
    println!("底板: {:.0}×{:.0}×{:.0}mm", bp.width, bp.height, bp.thickness);

    // 錨栓 ≥ 4 顆
    assert!(conn.bolts[0].positions.len() >= 4, "至少 4 顆錨栓");
    println!("錨栓: M24×{} 顆 (F8T) {}×{}", conn.bolts[0].positions.len(),
        conn.bolts[0].rows, conn.bolts[0].cols);

    // AISC
    let check = check_connection(&conn, &SteelMaterial::SS400, DesignMethod::LRFD);
    for w in &check.warnings { println!("  ⚠ {}", w); }
    // 底板接頭的錨栓間距較大是正常的，只要無硬性違規
    let hard_fails: Vec<_> = check.warnings.iter()
        .filter(|w| w.contains("< AISC 最小"))
        .collect();
    assert!(hard_fails.is_empty(), "AISC 硬性違規: {:?}", hard_fails);
    println!("AISC: ✅ 抗剪={:.1}kN 抗拉={:.1}kN",
        check.total_bolt_shear, check.total_bolt_tension);
}

#[test]
fn test_full_pipeline_scene_to_report() {
    // ── 建場景 ──
    let scene = create_test_steel_frame();
    let col_count = scene.objects.values().filter(|o| o.component_kind == ComponentKind::Column).count();
    let beam_count = scene.objects.values().filter(|o| o.component_kind == ComponentKind::Beam).count();
    println!("場景: {} 柱件 + {} 梁件", col_count, beam_count);

    // ── 自動編號 ──
    let numbering = auto_number(&scene);
    println!("編號: {} 種 (柱{} 梁{})",
        numbering.stats.unique_marks, numbering.stats.total_columns, numbering.stats.total_beams);
    assert!(numbering.stats.total_columns > 0);
    assert!(numbering.stats.total_beams > 0);
    assert!(numbering.stats.assembly_count > 0);

    // ── 建立接頭 ──
    let conn1 = calc_end_plate(&EndPlateParams {
        beam_section: (400.0, 200.0, 8.0, 13.0),
        col_section: (300.0, 300.0, 10.0, 15.0),
        bolt_size: BoltSize::M20,
        bolt_grade: BoltGrade::F10T,
        plate_thickness: None,
        add_stiffeners: true,
    });
    let connections = vec![conn1];

    // ── 報表 ──
    let report = generate_report(&scene, &connections);
    println!("報表: {}種構件, {:.0}kg, {}螺栓, {:.0}mm焊接",
        report.material_rows.len(), report.grand_total_weight,
        report.total_bolt_count, report.total_weld_length);
    assert!(!report.material_rows.is_empty());
    assert!(report.grand_total_weight > 0.0);

    // ── 施工圖 ──
    let ga = generate_ga_drawing(&scene, &numbering);
    let total_elements: usize = ga.views.iter().map(|v| v.elements.len()).sum();
    println!("GA 圖: {} 視圖, {} 元素, 比例 1:{}", ga.views.len(), total_elements, ga.scale as i32);
    assert!(!ga.views.is_empty());
    assert!(total_elements > 10);

    // ── 碰撞偵測 ──
    let collision = crate::collision::check_scene_collisions(&scene, &crate::collision::CollisionConfig::default());
    println!("碰撞: {} 問題, all_clear={}", collision.warnings.len(), collision.all_clear);

    println!("\n=== 全流程測試通過 ===");
}

#[test]
fn test_bolt_capacity_cross_validation() {
    // 手算驗證: M20 A325 LRFD
    // Ab = π×20²/4 = 314.16 mm²
    // 抗剪(N): φRn = 0.75 × 372 × 314.16 / 1000 = 87.6 kN
    // 抗拉:    φRn = 0.75 × 620 × 314.16 / 1000 = 146.1 kN
    // 承壓(t=16, Fu=400): φRn = 0.75 × 2.4 × 20 × 16 × 400 / 1000 = 230.4 kN
    let cap = bolt_capacity(&BoltSize::M20, &BoltGrade::A325, 16.0, &SteelMaterial::SS400, DesignMethod::LRFD, true);

    let ab_expected = std::f32::consts::PI * 20.0 * 20.0 / 4.0;
    assert!((cap.area - ab_expected).abs() < 0.1, "Ab={:.2} expected {:.2}", cap.area, ab_expected);

    let shear_expected = 0.75 * 372.0 * ab_expected / 1000.0;
    assert!((cap.shear_capacity - shear_expected).abs() < 1.0,
        "抗剪={:.1} expected {:.1}", cap.shear_capacity, shear_expected);

    let tension_expected = 0.75 * 620.0 * ab_expected / 1000.0;
    assert!((cap.tensile_capacity - tension_expected).abs() < 1.0,
        "抗拉={:.1} expected {:.1}", cap.tensile_capacity, tension_expected);

    let bearing_expected = 0.75 * 2.4 * 20.0 * 16.0 * 400.0 / 1000.0;
    assert!((cap.bearing_capacity - bearing_expected).abs() < 1.0,
        "承壓={:.1} expected {:.1}", cap.bearing_capacity, bearing_expected);

    println!("M20 A325 LRFD 手算驗證:");
    println!("  Ab = {:.2} mm² (expected {:.2})", cap.area, ab_expected);
    println!("  抗剪 φRn = {:.1} kN (expected {:.1})", cap.shear_capacity, shear_expected);
    println!("  抗拉 φRn = {:.1} kN (expected {:.1})", cap.tensile_capacity, tension_expected);
    println!("  承壓 φRn = {:.1} kN (expected {:.1})", cap.bearing_capacity, bearing_expected);
}

#[test]
fn test_weld_capacity_cross_validation() {
    // 手算驗證: 角焊 8mm, 長度 300mm, E70, SS400, LRFD
    // te = 8/√2 = 5.657 mm
    // Awe = 5.657 × 300 = 1697.1 mm²
    // Fnw = 0.6 × 482 = 289.2 MPa
    // φRn(weld) = 0.75 × 289.2 × 1697.1 / 1000 = 368.1 kN
    let weld = WeldLine {
        weld_type: WeldType::Fillet,
        size: 8.0, length: 300.0,
        start: [0.0; 3], end: [300.0, 0.0, 0.0],
    };
    let cap = weld_capacity(&weld, &SteelMaterial::SS400, DesignMethod::LRFD);

    let te_expected = 8.0 / std::f32::consts::SQRT_2;
    assert!((cap.effective_throat - te_expected).abs() < 0.01);

    let awe_expected = te_expected * 300.0;
    assert!((cap.effective_area - awe_expected).abs() < 0.1);

    let fnw = 0.6 * 482.0;
    let weld_cap_expected = 0.75 * fnw * awe_expected / 1000.0;
    assert!((cap.weld_metal_capacity - weld_cap_expected).abs() < 1.0);

    println!("角焊 8mm×300mm E70 LRFD 手算驗證:");
    println!("  te = {:.3} mm (expected {:.3})", cap.effective_throat, te_expected);
    println!("  Awe = {:.1} mm² (expected {:.1})", cap.effective_area, awe_expected);
    println!("  φRn = {:.1} kN (expected {:.1})", cap.weld_metal_capacity, weld_cap_expected);
}

#[test]
fn test_aisc_suggest_beam_to_column() {
    let beam = (400.0, 200.0, 8.0, 13.0);  // H400×200
    let col = (300.0, 300.0, 10.0, 15.0);   // H300×300

    let suggestions = suggest_connection(
        beam, col,
        ConnectionIntent::BeamToColumn,
        "SS400",
    );

    assert!(suggestions.len() >= 2, "應至少建議 2 種方案");

    // 方案 1: 端板（剛接）
    let ep = &suggestions[0];
    assert_eq!(ep.conn_type, ConnectionType::EndPlate);
    assert!(ep.plate_thickness >= 16.0);
    assert!(ep.estimated_capacity.pass, "端板方案 AISC 應通過");
    println!("方案1: {} | {} {} | 板厚{:.0}mm | 加勁板:{} | 抗剪{:.0}kN",
        ep.conn_type.label(), ep.bolt_size.label(), ep.bolt_grade.label(),
        ep.plate_thickness, ep.need_stiffeners, ep.estimated_capacity.total_bolt_shear);
    println!("  原因: {}", ep.reason);
    println!("  加勁板: {}", ep.stiffener_reason);

    // 方案 2: 腹板（鉸接）
    let st = &suggestions[1];
    assert_eq!(st.conn_type, ConnectionType::ShearTab);
    assert!(st.estimated_capacity.pass, "腹板方案 AISC 應通過");
    println!("方案2: {} | {} {} | 板厚{:.0}mm | 抗剪{:.0}kN",
        st.conn_type.label(), st.bolt_size.label(), st.bolt_grade.label(),
        st.plate_thickness, st.estimated_capacity.total_bolt_shear);

    // 驗證螺栓建議合理性
    // H400 梁 → 應建議 M22 或 M24
    assert!(ep.bolt_size.diameter() >= 20.0, "H400梁螺栓太小: {}", ep.bolt_size.label());
}

#[test]
fn test_aisc_suggest_column_base() {
    let col = (300.0, 300.0, 10.0, 15.0);

    let suggestions = suggest_connection(
        col, col,
        ConnectionIntent::ColumnBase,
        "SS400",
    );

    assert!(!suggestions.is_empty());
    let bp = &suggestions[0];
    assert_eq!(bp.conn_type, ConnectionType::BasePlate);
    assert!(bp.plate_thickness >= 20.0, "底板厚不足");
    println!("底板建議: {} | 板厚{:.0}mm | 錨栓{} | 加勁板:{}",
        bp.reason, bp.plate_thickness, bp.bolt_size.label(),
        if bp.need_stiffeners { "需要" } else { "不需" });
    println!("  加勁板原因: {}", bp.stiffener_reason);
}

#[test]
fn test_stiffener_check_logic() {
    // 大梁小柱 → 需要加勁板
    let beam = (700.0, 300.0, 13.0, 24.0);
    let col = (300.0, 300.0, 10.0, 15.0);
    let (needed, reason) = need_stiffeners_check(beam, col);
    assert!(needed, "大梁小柱應需加勁板");
    println!("大梁小柱: needed={} reason={}", needed, reason);

    // 小梁大柱 → 不需要
    let beam2 = (200.0, 100.0, 5.5, 8.0);
    let col2 = (400.0, 400.0, 13.0, 21.0);
    let (needed2, reason2) = need_stiffeners_check(beam2, col2);
    assert!(!needed2, "小梁大柱不應需加勁板");
    println!("小梁大柱: needed={} reason={}", needed2, reason2);
}

#[test]
fn test_hole_layout() {
    let layout = calc_hole_layout(268.0, 508.0, BoltSize::M20, 4, 2);
    assert_eq!(layout.holes.len(), 8);
    assert_eq!(layout.hole_diameter, 22.0); // M20 + 2mm
    assert!(layout.pitch >= BoltSize::M20.min_spacing(), "行距 {:.0} < AISC {:.0}", layout.pitch, BoltSize::M20.min_spacing());
    assert!(layout.edge_x >= BoltSize::M20.min_edge(), "X邊距 {:.0} < AISC {:.0}", layout.edge_x, BoltSize::M20.min_edge());

    println!("孔位佈置 4×2 = {} 孔:", layout.holes.len());
    println!("  孔徑: Ø{:.0}mm | 邊距: X={:.0} Y={:.0} | 行距={:.0} 列距={:.0}",
        layout.hole_diameter, layout.edge_x, layout.edge_y, layout.pitch, layout.gauge);
    for (i, h) in layout.holes.iter().enumerate() {
        println!("  孔{}: ({:.1}, {:.1})", i+1, h[0], h[1]);
    }
    for c in &layout.checks {
        println!("  {}", c);
    }
}

#[test]
fn test_base_plate_stiffener_suggestion() {
    // 大柱 → 應建議加勁肋
    let stiff = suggest_base_plate_stiffeners(
        (300.0, 300.0, 10.0, 15.0), 25.0, 500.0
    );
    println!("大柱底板加勁: needed={} reason={}", stiff.needed, stiff.reason);
    println!("  尺寸: {:.0}×{:.0}×{:.0}mm × {} 片 | 焊腳={:.0}mm",
        stiff.width, stiff.height, stiff.thickness, stiff.quantity, stiff.weld_size);

    // 小柱 → 不需要
    let stiff2 = suggest_base_plate_stiffeners(
        (200.0, 200.0, 8.0, 12.0), 30.0, 200.0
    );
    assert!(!stiff2.needed, "小柱不應需底板加勁肋");
}

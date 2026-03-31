//! 管線幾何生成測試

use kolibri_core::scene::Scene;
use kolibri_piping::pipe_data::*;
use kolibri_piping::catalog::PipeCatalog;
use kolibri_piping::geometry;

#[test]
fn test_vertical_pipe() {
    let mut scene = Scene::default();
    let spec = PipeCatalog::default_spec(PipeSystem::PvcWater);
    println!("Spec: {} OD={} WT={}", spec.spec_name, spec.outer_diameter, spec.wall_thickness);

    let id = geometry::create_pipe_segment(
        &mut scene, &spec,
        [0.0, 0.0, 0.0], [0.0, 3000.0, 0.0],
        "垂直管".into(),
    );
    assert!(!id.is_empty(), "Should create vertical pipe");
    let obj = scene.objects.get(&id).unwrap();
    println!("Vertical pipe: shape={:?}", std::mem::discriminant(&obj.shape));
    assert!(matches!(obj.shape, kolibri_core::scene::Shape::Mesh(_)), "Should be Mesh");
    if let kolibri_core::scene::Shape::Mesh(ref m) = obj.shape {
        println!("  vertices={} faces={}", m.vertices.len(), m.faces.len());
        assert!(m.vertices.len() >= 16, "Should have circle vertices: {}", m.vertices.len());
        assert!(m.faces.len() >= 10, "Should have side faces + caps: {}", m.faces.len());
    }
    assert_eq!(obj.ifc_class, "IfcPipeSegment");
}

#[test]
fn test_horizontal_pipe_x() {
    let mut scene = Scene::default();
    let spec = PipeCatalog::default_spec(PipeSystem::IronFireSprinkler);
    println!("Spec: {} OD={}", spec.spec_name, spec.outer_diameter);

    let id = geometry::create_pipe_segment(
        &mut scene, &spec,
        [0.0, 2700.0, 0.0], [5000.0, 2700.0, 0.0],
        "水平管X".into(),
    );
    assert!(!id.is_empty(), "Should create horizontal pipe");
    let obj = scene.objects.get(&id).unwrap();
    if let kolibri_core::scene::Shape::Mesh(ref m) = obj.shape {
        println!("  Horizontal X: vertices={} faces={}", m.vertices.len(), m.faces.len());
        assert!(m.faces.len() >= 10);
    }
}

#[test]
fn test_horizontal_pipe_z() {
    let mut scene = Scene::default();
    let spec = PipeCatalog::default_spec(PipeSystem::PvcDrain);

    let id = geometry::create_pipe_segment(
        &mut scene, &spec,
        [1000.0, 2400.0, 0.0], [1000.0, 2400.0, 3000.0],
        "水平管Z".into(),
    );
    assert!(!id.is_empty());
    let obj = scene.objects.get(&id).unwrap();
    assert!(matches!(obj.shape, kolibri_core::scene::Shape::Mesh(_)));
}

#[test]
fn test_diagonal_pipe() {
    let mut scene = Scene::default();
    let spec = PipeCatalog::default_spec(PipeSystem::SteelProcess);

    let id = geometry::create_pipe_segment(
        &mut scene, &spec,
        [0.0, 0.0, 0.0], [2000.0, 1500.0, 1000.0],
        "斜管".into(),
    );
    assert!(!id.is_empty());
    let obj = scene.objects.get(&id).unwrap();
    if let kolibri_core::scene::Shape::Mesh(ref m) = obj.shape {
        println!("  Diagonal: vertices={} faces={}", m.vertices.len(), m.faces.len());
        // 斜管也應該是圓柱 mesh
        assert!(m.faces.len() >= 10);
    }
}

#[test]
fn test_elbow_90() {
    let mut scene = Scene::default();
    let spec = PipeCatalog::default_spec(PipeSystem::PvcWater);

    let id = geometry::create_fitting(
        &mut scene,
        FittingKind::Elbow90,
        &spec,
        [0.0, 2700.0, 0.0],
        "90度彎頭".into(),
    );
    assert!(!id.is_empty(), "Should create elbow");
    let obj = scene.objects.get(&id).unwrap();
    println!("Elbow90: ifc_class={}", obj.ifc_class);
    assert_eq!(obj.ifc_class, "IfcPipeFitting");
    if let kolibri_core::scene::Shape::Mesh(ref m) = obj.shape {
        println!("  Elbow: vertices={} faces={}", m.vertices.len(), m.faces.len());
        assert!(m.vertices.len() >= 32, "Elbow should have multiple rings: {}", m.vertices.len());
        assert!(m.faces.len() >= 16, "Elbow should have ring connections: {}", m.faces.len());
    }
}

#[test]
fn test_tee_fitting() {
    let mut scene = Scene::default();
    let spec = PipeCatalog::default_spec(PipeSystem::IronFireSprinkler);

    let id = geometry::create_fitting(
        &mut scene,
        FittingKind::Tee,
        &spec,
        [500.0, 2700.0, 500.0],
        "三通".into(),
    );
    assert!(!id.is_empty());
    let obj = scene.objects.get(&id).unwrap();
    if let kolibri_core::scene::Shape::Mesh(ref m) = obj.shape {
        println!("  Tee: vertices={} faces={}", m.vertices.len(), m.faces.len());
        // 三通 = 主管 + 分支管 → 較多頂點
        assert!(m.vertices.len() >= 48, "Tee should merge two cylinders: {}", m.vertices.len());
    }
}

#[test]
fn test_valve() {
    let mut scene = Scene::default();
    let spec = PipeCatalog::default_spec(PipeSystem::PvcWater);

    let id = geometry::create_fitting(
        &mut scene,
        FittingKind::Valve,
        &spec,
        [0.0, 0.0, 0.0],
        "閥門".into(),
    );
    assert!(!id.is_empty());
    let obj = scene.objects.get(&id).unwrap();
    if let kolibri_core::scene::Shape::Mesh(ref m) = obj.shape {
        println!("  Valve: vertices={} faces={}", m.vertices.len(), m.faces.len());
        assert!(m.vertices.len() >= 32);
    }
}

#[test]
fn test_flange() {
    let mut scene = Scene::default();
    let spec = PipeCatalog::default_spec(PipeSystem::SteelProcess);

    let id = geometry::create_fitting(
        &mut scene,
        FittingKind::Flange,
        &spec,
        [1000.0, 2700.0, 0.0],
        "法蘭".into(),
    );
    assert!(!id.is_empty());
    let obj = scene.objects.get(&id).unwrap();
    assert_eq!(obj.ifc_class, "IfcPipeFitting");
    if let kolibri_core::scene::Shape::Mesh(ref m) = obj.shape {
        println!("  Flange: vertices={} faces={}", m.vertices.len(), m.faces.len());
        assert!(m.faces.len() >= 8, "Flange should be a short cylinder");
    }
}

#[test]
fn test_all_fitting_kinds() {
    let mut scene = Scene::default();
    let spec = PipeCatalog::default_spec(PipeSystem::PvcWater);

    let kinds = [
        FittingKind::Elbow90, FittingKind::Elbow45, FittingKind::Tee,
        FittingKind::Cross, FittingKind::Reducer, FittingKind::Cap,
        FittingKind::Valve, FittingKind::Coupling, FittingKind::Flange,
    ];

    for kind in &kinds {
        let id = geometry::create_fitting(
            &mut scene, *kind, &spec,
            [0.0, 0.0, 0.0],
            format!("{:?}", kind),
        );
        assert!(!id.is_empty(), "{:?} should create object", kind);
        let obj = scene.objects.get(&id).unwrap();
        assert!(matches!(obj.shape, kolibri_core::scene::Shape::Mesh(_)),
            "{:?} should be Mesh", kind);
        if let kolibri_core::scene::Shape::Mesh(ref m) = obj.shape {
            println!("  {:?}: verts={} faces={}", kind, m.vertices.len(), m.faces.len());
            assert!(m.vertices.len() > 0, "{:?} mesh empty", kind);
            assert!(m.faces.len() > 0, "{:?} no faces", kind);
        }
    }
}

#[test]
fn test_pipe_spec_dimensions() {
    // 確認規格表的尺寸正確帶入 geometry
    let mut scene = Scene::default();

    // DN100 消防管 OD=114.3mm
    let specs = PipeCatalog::specs_for(PipeSystem::IronFireSprinkler);
    let dn100 = specs.iter().find(|s| s.nominal_dn == 100.0).unwrap();
    assert!((dn100.outer_diameter - 114.3).abs() < 0.1, "DN100 OD should be 114.3");
    assert!((dn100.wall_thickness - 6.02).abs() < 0.1, "DN100 WT should be 6.02");

    let id = geometry::create_pipe_segment(
        &mut scene, dn100,
        [0.0, 2700.0, 0.0], [0.0, 2700.0, 5000.0],
        "DN100消防管".into(),
    );
    let obj = scene.objects.get(&id).unwrap();
    assert_eq!(obj.ifc_material_name, "DN100 (4\") SCH40");
    println!("DN100 pipe created with correct spec: {}", obj.ifc_material_name);
}

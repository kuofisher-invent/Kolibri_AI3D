//! Phase D: IFC 2x3 匯出
//! 輸出 STEP Part 21 格式 (.ifc)
//! 支援: IfcColumn, IfcBeam, IfcPlate, IfcMechanicalFastener, IfcWeld

use std::io::Write;
use kolibri_core::scene::{Scene, SceneObject, Shape, MaterialKind};
use kolibri_core::collision::ComponentKind;
use kolibri_core::steel_connection::*;
use kolibri_core::steel_numbering::NumberingResult;

/// IFC 實體 ID 計數器
struct IfcWriter {
    next_id: u64,
    lines: Vec<String>,
}

impl IfcWriter {
    fn new() -> Self { Self { next_id: 100, lines: Vec::new() } }

    fn next(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn add(&mut self, id: u64, content: String) {
        self.lines.push(format!("#{}={};", id, content));
    }
}

/// 匯出場景為 IFC 2x3 格式
pub fn export_ifc(
    scene: &Scene,
    connections: &[SteelConnection],
    numbering: &NumberingResult,
    path: &str,
) -> Result<usize, String> {
    let mut w = IfcWriter::new();
    let mut entity_count = 0;

    // ── Header entities ──
    let org_id = w.next();
    w.add(org_id, "IFCORGANIZATION($,'Kolibri Ai3D','CAD/BIM Software',$,$)".into());

    let app_id = w.next();
    w.add(app_id, format!("IFCAPPLICATION(#{},'0.1.0','Kolibri Ai3D','KolibriAi3D')", org_id));

    let person_id = w.next();
    w.add(person_id, "IFCPERSON($,'User','Kolibri',$,$,$,$,$)".into());

    let person_org_id = w.next();
    w.add(person_org_id, format!("IFCPERSONANDORGANIZATION(#{},#{},$)", person_id, org_id));

    let owner_id = w.next();
    w.add(owner_id, format!("IFCOWNERHISTORY(#{},#{},$,.READWRITE.,$,$,$,0)", person_org_id, app_id));

    // Geometric context
    let origin_id = w.next();
    w.add(origin_id, "IFCCARTESIANPOINT((0.,0.,0.))".into());

    let dir_z_id = w.next();
    w.add(dir_z_id, "IFCDIRECTION((0.,0.,1.))".into());
    let dir_x_id = w.next();
    w.add(dir_x_id, "IFCDIRECTION((1.,0.,0.))".into());

    let axis_id = w.next();
    w.add(axis_id, format!("IFCAXIS2PLACEMENT3D(#{},#{},#{})", origin_id, dir_z_id, dir_x_id));

    let context_id = w.next();
    w.add(context_id, format!(
        "IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.0E-5,#{},$)", axis_id
    ));

    let units_id = write_units(&mut w);

    // Project
    let project_id = w.next();
    w.add(project_id, format!(
        "IFCPROJECT('{}',#{},'Kolibri Steel Project',$,$,$,$,(#{}),#{})",
        new_guid(), owner_id, context_id, units_id
    ));

    // Site → Building → Storey
    let site_place_id = w.next();
    w.add(site_place_id, format!("IFCLOCALPLACEMENT($,#{})", axis_id));
    let site_id = w.next();
    w.add(site_id, format!(
        "IFCSITE('{}',#{},'Site',$,$,#{},$,$,.ELEMENT.,$,$,$,$,$)",
        new_guid(), owner_id, site_place_id
    ));

    let bldg_id = w.next();
    w.add(bldg_id, format!(
        "IFCBUILDING('{}',#{},'Building',$,$,#{},$,$,.ELEMENT.,$,$,$)",
        new_guid(), owner_id, site_place_id
    ));

    let storey_id = w.next();
    w.add(storey_id, format!(
        "IFCBUILDINGSTOREY('{}',#{},'Level 1',$,$,#{},$,$,.ELEMENT.,0.)",
        new_guid(), owner_id, site_place_id
    ));

    // Aggregation relationships
    let rel_site = w.next();
    w.add(rel_site, format!(
        "IFCRELAGGREGATES('{}',#{},$,$,#{},({}))",
        new_guid(), owner_id, project_id, format!("#{}", site_id)
    ));
    let rel_bldg = w.next();
    w.add(rel_bldg, format!(
        "IFCRELAGGREGATES('{}',#{},$,$,#{},({}))",
        new_guid(), owner_id, site_id, format!("#{}", bldg_id)
    ));
    let rel_storey = w.next();
    w.add(rel_storey, format!(
        "IFCRELAGGREGATES('{}',#{},$,$,#{},({}))",
        new_guid(), owner_id, bldg_id, format!("#{}", storey_id)
    ));

    // ── 構件實體 ──
    let mut storey_members = Vec::new();

    for (id, obj) in &scene.objects {
        if !obj.visible { continue; }
        let mark = numbering.marks.get(id).cloned().unwrap_or_default();

        match obj.component_kind {
            ComponentKind::Column => {
                let eid = write_column(&mut w, obj, &mark, owner_id, context_id, site_place_id);
                storey_members.push(format!("#{}", eid));
                entity_count += 1;
            }
            ComponentKind::Beam => {
                let eid = write_beam(&mut w, obj, &mark, owner_id, context_id, site_place_id);
                storey_members.push(format!("#{}", eid));
                entity_count += 1;
            }
            ComponentKind::Plate => {
                let eid = write_plate(&mut w, obj, &mark, owner_id, context_id, site_place_id);
                storey_members.push(format!("#{}", eid));
                entity_count += 1;
            }
            ComponentKind::Bolt => {
                let eid = write_fastener(&mut w, obj, &mark, owner_id, context_id, site_place_id);
                storey_members.push(format!("#{}", eid));
                entity_count += 1;
            }
            _ => {}
        }
    }

    // IfcRelContainedInSpatialStructure
    if !storey_members.is_empty() {
        let rel_id = w.next();
        w.add(rel_id, format!(
            "IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',#{},$,$,({}),#{})",
            new_guid(), owner_id, storey_members.join(","), storey_id
        ));
    }

    // ── 輸出檔案 ──
    let mut f = std::fs::File::create(path).map_err(|e| e.to_string())?;

    // STEP header
    writeln!(f, "ISO-10303-21;").map_err(|e| e.to_string())?;
    writeln!(f, "HEADER;").map_err(|e| e.to_string())?;
    writeln!(f, "FILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');").map_err(|e| e.to_string())?;
    writeln!(f, "FILE_NAME('{}','2026-04-02',('Kolibri'),('Kolibri Ai3D'),'Kolibri IFC Exporter','Kolibri Ai3D 0.1','');", path).map_err(|e| e.to_string())?;
    writeln!(f, "FILE_SCHEMA(('IFC2X3'));").map_err(|e| e.to_string())?;
    writeln!(f, "ENDSEC;").map_err(|e| e.to_string())?;
    writeln!(f, "DATA;").map_err(|e| e.to_string())?;

    for line in &w.lines {
        writeln!(f, "{}", line).map_err(|e| e.to_string())?;
    }

    writeln!(f, "ENDSEC;").map_err(|e| e.to_string())?;
    writeln!(f, "END-ISO-10303-21;").map_err(|e| e.to_string())?;

    Ok(entity_count)
}

// ─── IFC Entity Writers ────────────────────────────────────────────────────────

fn write_units(w: &mut IfcWriter) -> u64 {
    let mm_id = w.next();
    w.add(mm_id, "IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.)".into());
    let rad_id = w.next();
    w.add(rad_id, "IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.)".into());
    let kg_id = w.next();
    w.add(kg_id, "IFCSIUNIT(*,.MASSUNIT.,.KILO.,.GRAM.)".into());

    let units_id = w.next();
    w.add(units_id, format!("IFCUNITASSIGNMENT((#{},#{},#{}))", mm_id, rad_id, kg_id));
    units_id
}

fn write_placement(w: &mut IfcWriter, pos: [f32; 3], parent_placement: u64) -> u64 {
    let pt_id = w.next();
    w.add(pt_id, format!("IFCCARTESIANPOINT(({:.3},{:.3},{:.3}))", pos[0], pos[1], pos[2]));

    let dir_z = w.next();
    w.add(dir_z, "IFCDIRECTION((0.,0.,1.))".into());
    let dir_x = w.next();
    w.add(dir_x, "IFCDIRECTION((1.,0.,0.))".into());

    let axis = w.next();
    w.add(axis, format!("IFCAXIS2PLACEMENT3D(#{},#{},#{})", pt_id, dir_z, dir_x));

    let lp = w.next();
    w.add(lp, format!("IFCLOCALPLACEMENT(#{},#{})", parent_placement, axis));
    lp
}

fn write_extruded_box(w: &mut IfcWriter, width: f32, height: f32, depth: f32, context: u64) -> u64 {
    // Rectangle profile
    let pt = w.next();
    w.add(pt, "IFCCARTESIANPOINT((0.,0.))".into());
    let dir = w.next();
    w.add(dir, "IFCDIRECTION((1.,0.))".into());
    let axis2d = w.next();
    w.add(axis2d, format!("IFCAXIS2PLACEMENT2D(#{},#{})", pt, dir));

    let profile = w.next();
    w.add(profile, format!(
        "IFCRECTANGLEPROFILEDEF(.AREA.,$,#{},{:.3},{:.3})", axis2d, width, depth
    ));

    // Extrusion direction (Y up)
    let ext_dir = w.next();
    w.add(ext_dir, "IFCDIRECTION((0.,0.,1.))".into());

    // Placement for extrusion
    let ext_origin = w.next();
    w.add(ext_origin, "IFCCARTESIANPOINT((0.,0.,0.))".into());
    let ext_axis_z = w.next();
    w.add(ext_axis_z, "IFCDIRECTION((0.,1.,0.))".into());
    let ext_axis_x = w.next();
    w.add(ext_axis_x, "IFCDIRECTION((1.,0.,0.))".into());
    let ext_placement = w.next();
    w.add(ext_placement, format!("IFCAXIS2PLACEMENT3D(#{},#{},#{})", ext_origin, ext_axis_z, ext_axis_x));

    let solid = w.next();
    w.add(solid, format!(
        "IFCEXTRUDEDAREASOLID(#{},#{},#{},{:.3})", profile, ext_placement, ext_dir, height
    ));

    let shape_rep = w.next();
    w.add(shape_rep, format!(
        "IFCSHAPEREPRESENTATION(#{},'Body','SweptSolid',(#{}))", context, solid
    ));

    let prod_shape = w.next();
    w.add(prod_shape, format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{}))", shape_rep));
    prod_shape
}

fn write_column(w: &mut IfcWriter, obj: &SceneObject, mark: &str, owner: u64, ctx: u64, parent_lp: u64) -> u64 {
    let (width, height, depth) = match &obj.shape {
        Shape::Box { width, height, depth } => (*width, *height, *depth),
        _ => (150.0, 3000.0, 150.0),
    };

    let lp = write_placement(w, obj.position, parent_lp);
    let shape = write_extruded_box(w, width, height, depth, ctx);

    let col_id = w.next();
    w.add(col_id, format!(
        "IFCCOLUMN('{}',#{},'{}','Column {}',$,#{},#{},$)",
        new_guid(), owner, mark, obj.name, lp, shape
    ));
    col_id
}

fn write_beam(w: &mut IfcWriter, obj: &SceneObject, mark: &str, owner: u64, ctx: u64, parent_lp: u64) -> u64 {
    let (width, height, depth) = match &obj.shape {
        Shape::Box { width, height, depth } => (*width, *height, *depth),
        _ => (6000.0, 300.0, 150.0),
    };

    let lp = write_placement(w, obj.position, parent_lp);
    let shape = write_extruded_box(w, width, height, depth, ctx);

    let beam_id = w.next();
    w.add(beam_id, format!(
        "IFCBEAM('{}',#{},'{}','Beam {}',$,#{},#{},$)",
        new_guid(), owner, mark, obj.name, lp, shape
    ));
    beam_id
}

fn write_plate(w: &mut IfcWriter, obj: &SceneObject, mark: &str, owner: u64, ctx: u64, parent_lp: u64) -> u64 {
    let (width, height, depth) = match &obj.shape {
        Shape::Box { width, height, depth } => (*width, *height, *depth),
        _ => (200.0, 16.0, 300.0),
    };

    let lp = write_placement(w, obj.position, parent_lp);
    let shape = write_extruded_box(w, width, height, depth, ctx);

    let plate_id = w.next();
    w.add(plate_id, format!(
        "IFCPLATE('{}',#{},'{}','Plate {}',$,#{},#{},$)",
        new_guid(), owner, mark, obj.name, lp, shape
    ));
    plate_id
}

fn write_fastener(w: &mut IfcWriter, obj: &SceneObject, mark: &str, owner: u64, ctx: u64, parent_lp: u64) -> u64 {
    let (r, h) = match &obj.shape {
        Shape::Cylinder { radius, height, .. } => (*radius, *height),
        _ => (10.0, 50.0),
    };

    let lp = write_placement(w, obj.position, parent_lp);

    // Simplified: use extruded circle
    let pt = w.next();
    w.add(pt, "IFCCARTESIANPOINT((0.,0.))".into());
    let dir2d = w.next();
    w.add(dir2d, "IFCDIRECTION((1.,0.))".into());
    let axis2d = w.next();
    w.add(axis2d, format!("IFCAXIS2PLACEMENT2D(#{},#{})", pt, dir2d));
    let profile = w.next();
    w.add(profile, format!("IFCCIRCLEPROFILEDEF(.AREA.,$,#{},{:.3})", axis2d, r));

    let ext_dir = w.next();
    w.add(ext_dir, "IFCDIRECTION((0.,0.,1.))".into());
    let ext_origin = w.next();
    w.add(ext_origin, "IFCCARTESIANPOINT((0.,0.,0.))".into());
    let ext_z = w.next();
    w.add(ext_z, "IFCDIRECTION((0.,1.,0.))".into());
    let ext_x = w.next();
    w.add(ext_x, "IFCDIRECTION((1.,0.,0.))".into());
    let ext_p = w.next();
    w.add(ext_p, format!("IFCAXIS2PLACEMENT3D(#{},#{},#{})", ext_origin, ext_z, ext_x));
    let solid = w.next();
    w.add(solid, format!("IFCEXTRUDEDAREASOLID(#{},#{},#{},{:.3})", profile, ext_p, ext_dir, h));

    let shape_rep = w.next();
    w.add(shape_rep, format!("IFCSHAPEREPRESENTATION(#{},'Body','SweptSolid',(#{}))", ctx, solid));
    let prod_shape = w.next();
    w.add(prod_shape, format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{}))", shape_rep));

    let fastener_id = w.next();
    w.add(fastener_id, format!(
        "IFCMECHANICALFASTENER('{}',#{},'{}','Bolt {}',$,#{},#{},$,$,$)",
        new_guid(), owner, mark, obj.name, lp, prod_shape
    ));
    fastener_id
}

// ─── IFC4 匯出 ──────────────────────────────────────────────────────────────

/// IFC 版本
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IfcVersion {
    Ifc2x3,
    Ifc4,
}

impl IfcVersion {
    pub fn schema_id(&self) -> &'static str {
        match self {
            Self::Ifc2x3 => "IFC2X3",
            Self::Ifc4 => "IFC4",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ifc2x3 => "IFC 2x3",
            Self::Ifc4 => "IFC4",
        }
    }
}

/// 匯出場景為 IFC4 格式（含材質關聯 + IfcPipeSegment）
pub fn export_ifc4(
    scene: &Scene,
    connections: &[SteelConnection],
    numbering: &NumberingResult,
    path: &str,
) -> Result<usize, String> {
    let mut w = IfcWriter::new();
    let mut entity_count = 0;

    // ── Header entities ──
    let org_id = w.next();
    w.add(org_id, "IFCORGANIZATION($,'Kolibri Ai3D','CAD/BIM Software',$,$)".into());

    let app_id = w.next();
    w.add(app_id, format!("IFCAPPLICATION(#{},'0.1.0','Kolibri Ai3D','KolibriAi3D')", org_id));

    let person_id = w.next();
    w.add(person_id, "IFCPERSON($,'User','Kolibri',$,$,$,$,$)".into());

    let person_org_id = w.next();
    w.add(person_org_id, format!("IFCPERSONANDORGANIZATION(#{},#{},$)", person_id, org_id));

    let owner_id = w.next();
    w.add(owner_id, format!("IFCOWNERHISTORY(#{},#{},$,.READWRITE.,$,$,$,0)", person_org_id, app_id));

    // Geometric context
    let origin_id = w.next();
    w.add(origin_id, "IFCCARTESIANPOINT((0.,0.,0.))".into());
    let dir_z_id = w.next();
    w.add(dir_z_id, "IFCDIRECTION((0.,0.,1.))".into());
    let dir_x_id = w.next();
    w.add(dir_x_id, "IFCDIRECTION((1.,0.,0.))".into());
    let axis_id = w.next();
    w.add(axis_id, format!("IFCAXIS2PLACEMENT3D(#{},#{},#{})", origin_id, dir_z_id, dir_x_id));
    let context_id = w.next();
    w.add(context_id, format!(
        "IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.0E-5,#{},$)", axis_id
    ));

    let units_id = write_units(&mut w);

    // Project
    let project_id = w.next();
    w.add(project_id, format!(
        "IFCPROJECT('{}',#{},'Kolibri Steel Project',$,$,$,$,(#{}),#{})",
        new_guid(), owner_id, context_id, units_id
    ));

    // Site → Building → Storey
    let site_place_id = w.next();
    w.add(site_place_id, format!("IFCLOCALPLACEMENT($,#{})", axis_id));
    let site_id = w.next();
    w.add(site_id, format!(
        "IFCSITE('{}',#{},'Site',$,$,#{},$,$,.ELEMENT.,$,$,$,$,$)",
        new_guid(), owner_id, site_place_id
    ));
    let bldg_id = w.next();
    w.add(bldg_id, format!(
        "IFCBUILDING('{}',#{},'Building',$,$,#{},$,$,.ELEMENT.,$,$,$)",
        new_guid(), owner_id, site_place_id
    ));
    let storey_id = w.next();
    w.add(storey_id, format!(
        "IFCBUILDINGSTOREY('{}',#{},'Level 1',$,$,#{},$,$,.ELEMENT.,0.)",
        new_guid(), owner_id, site_place_id
    ));

    // Aggregation
    let rel_site = w.next();
    w.add(rel_site, format!(
        "IFCRELAGGREGATES('{}',#{},$,$,#{},({}))",
        new_guid(), owner_id, project_id, format!("#{}", site_id)
    ));
    let rel_bldg = w.next();
    w.add(rel_bldg, format!(
        "IFCRELAGGREGATES('{}',#{},$,$,#{},({}))",
        new_guid(), owner_id, site_id, format!("#{}", bldg_id)
    ));
    let rel_storey = w.next();
    w.add(rel_storey, format!(
        "IFCRELAGGREGATES('{}',#{},$,$,#{},({}))",
        new_guid(), owner_id, bldg_id, format!("#{}", storey_id)
    ));

    // ── 材質定義（IFC4 新增 IfcMaterial + IfcRelAssociatesMaterial）──
    let steel_mat_id = w.next();
    w.add(steel_mat_id, "IFCMATERIAL('Steel','Steel','$')".into());
    let concrete_mat_id = w.next();
    w.add(concrete_mat_id, "IFCMATERIAL('Concrete','Concrete','$')".into());

    // ── 構件實體 ──
    let mut storey_members = Vec::new();
    let mut steel_member_ids = Vec::new();

    for (id, obj) in &scene.objects {
        if !obj.visible { continue; }
        let mark = numbering.marks.get(id).cloned().unwrap_or_default();

        let eid = match obj.component_kind {
            ComponentKind::Column => {
                let eid = write_column(&mut w, obj, &mark, owner_id, context_id, site_place_id);
                steel_member_ids.push(format!("#{}", eid));
                entity_count += 1;
                Some(eid)
            }
            ComponentKind::Beam => {
                let eid = write_beam(&mut w, obj, &mark, owner_id, context_id, site_place_id);
                steel_member_ids.push(format!("#{}", eid));
                entity_count += 1;
                Some(eid)
            }
            ComponentKind::Plate => {
                let eid = write_plate(&mut w, obj, &mark, owner_id, context_id, site_place_id);
                steel_member_ids.push(format!("#{}", eid));
                entity_count += 1;
                Some(eid)
            }
            ComponentKind::Bolt => {
                let eid = write_fastener(&mut w, obj, &mark, owner_id, context_id, site_place_id);
                entity_count += 1;
                Some(eid)
            }
            _ => {
                // IFC4: 管件用 IfcPipeSegment，其他用 IfcBuildingElementProxy
                if obj.name.starts_with("PIPE") || obj.name.contains("pipe") {
                    let eid = write_pipe_segment(&mut w, obj, &mark, owner_id, context_id, site_place_id);
                    entity_count += 1;
                    Some(eid)
                } else if obj.component_kind != ComponentKind::Generic {
                    let eid = write_proxy(&mut w, obj, &mark, owner_id, context_id, site_place_id);
                    entity_count += 1;
                    Some(eid)
                } else {
                    None
                }
            }
        };
        if let Some(eid) = eid {
            storey_members.push(format!("#{}", eid));
        }
    }

    // IfcRelAssociatesMaterial（鋼構件 → Steel 材質）
    if !steel_member_ids.is_empty() {
        let rel_mat_id = w.next();
        w.add(rel_mat_id, format!(
            "IFCRELASSOCIATESMATERIAL('{}',#{},$,$,({}),#{})",
            new_guid(), owner_id, steel_member_ids.join(","), steel_mat_id
        ));
    }

    // IfcRelContainedInSpatialStructure
    if !storey_members.is_empty() {
        let rel_id = w.next();
        w.add(rel_id, format!(
            "IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',#{},$,$,({}),#{})",
            new_guid(), owner_id, storey_members.join(","), storey_id
        ));
    }

    // ── 輸出檔案 ──
    let mut f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    writeln!(f, "ISO-10303-21;").map_err(|e| e.to_string())?;
    writeln!(f, "HEADER;").map_err(|e| e.to_string())?;
    writeln!(f, "FILE_DESCRIPTION(('ViewDefinition [ReferenceView_V1.2]'),'2;1');").map_err(|e| e.to_string())?;
    writeln!(f, "FILE_NAME('{}','2026-04-04',('Kolibri'),('Kolibri Ai3D'),'Kolibri IFC4 Exporter','Kolibri Ai3D 0.1','');", path).map_err(|e| e.to_string())?;
    writeln!(f, "FILE_SCHEMA(('IFC4'));").map_err(|e| e.to_string())?;
    writeln!(f, "ENDSEC;").map_err(|e| e.to_string())?;
    writeln!(f, "DATA;").map_err(|e| e.to_string())?;
    for line in &w.lines {
        writeln!(f, "{}", line).map_err(|e| e.to_string())?;
    }
    writeln!(f, "ENDSEC;").map_err(|e| e.to_string())?;
    writeln!(f, "END-ISO-10303-21;").map_err(|e| e.to_string())?;

    Ok(entity_count)
}

/// IFC4 IfcPipeSegment
fn write_pipe_segment(w: &mut IfcWriter, obj: &SceneObject, mark: &str, owner: u64, ctx: u64, parent_lp: u64) -> u64 {
    let (width, height, depth) = match &obj.shape {
        Shape::Box { width, height, depth } => (*width, *height, *depth),
        Shape::Cylinder { radius, height, .. } => (*radius * 2.0, *height, *radius * 2.0),
        _ => (100.0, 1000.0, 100.0),
    };
    let lp = write_placement(w, obj.position, parent_lp);
    let shape = write_extruded_box(w, width, height, depth, ctx);
    let pipe_id = w.next();
    w.add(pipe_id, format!(
        "IFCPIPESEGMENT('{}',#{},'{}','Pipe {}',$,#{},#{},$,.RIGIDSEGMENT.)",
        new_guid(), owner, mark, obj.name, lp, shape
    ));
    pipe_id
}

/// IFC4 IfcBuildingElementProxy（通用元素）
fn write_proxy(w: &mut IfcWriter, obj: &SceneObject, mark: &str, owner: u64, ctx: u64, parent_lp: u64) -> u64 {
    let (width, height, depth) = match &obj.shape {
        Shape::Box { width, height, depth } => (*width, *height, *depth),
        Shape::Cylinder { radius, height, .. } => (*radius * 2.0, *height, *radius * 2.0),
        _ => (100.0, 100.0, 100.0),
    };
    let lp = write_placement(w, obj.position, parent_lp);
    let shape = write_extruded_box(w, width, height, depth, ctx);
    let proxy_id = w.next();
    w.add(proxy_id, format!(
        "IFCBUILDINGELEMENTPROXY('{}',#{},'{}','Proxy {}',$,#{},#{},$,.NOTDEFINED.)",
        new_guid(), owner, mark, obj.name, lp, shape
    ));
    proxy_id
}

/// 產生簡易 GUID（22 字元 base64）
fn new_guid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let n = t.as_nanos();
    // 簡化版 IFC GUID: 22 chars from base64 of timestamp + counter
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let c = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let raw = format!("{:016X}{:06X}", n, c);
    // IFC GUID 使用 A-Z, a-z, 0-9, _, $ (64 chars)
    let charset = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";
    let bytes: Vec<u8> = raw.bytes().take(22).map(|b| charset[(b as usize) % 64]).collect();
    String::from_utf8(bytes).unwrap_or_else(|_| "0000000000000000000000".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ifc_guid_format() {
        let g1 = new_guid();
        let g2 = new_guid();
        assert_eq!(g1.len(), 22);
        assert_ne!(g1, g2); // 每次不同
    }

    #[test]
    fn test_ifc_export_basic() {
        let mut scene = Scene::default();
        let c1 = scene.insert_box_raw("COL_1".into(), [0.0; 3], 150.0, 4200.0, 150.0, MaterialKind::Steel);
        scene.objects.get_mut(&c1).unwrap().component_kind = ComponentKind::Column;
        let b1 = scene.insert_box_raw("BM_1".into(), [0.0, 4200.0, 0.0], 6000.0, 300.0, 150.0, MaterialKind::Steel);
        scene.objects.get_mut(&b1).unwrap().component_kind = ComponentKind::Beam;

        let numbering = kolibri_core::steel_numbering::auto_number(&scene);

        let tmp = std::env::temp_dir().join("kolibri_test.ifc");
        let path = tmp.to_string_lossy().to_string();
        let count = export_ifc(&scene, &[], &numbering, &path).unwrap();
        assert_eq!(count, 2); // 1 column + 1 beam

        // 驗證檔案存在且有內容
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("IFCCOLUMN"));
        assert!(content.contains("IFCBEAM"));
        assert!(content.contains("IFC2X3"));
        std::fs::remove_file(&path).ok();
    }
}

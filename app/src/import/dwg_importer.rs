//! DWG/DXF importer — bridges to existing cad_import module

use super::unified_ir::*;

pub fn import_dxf_to_unified_ir(path: &str) -> Result<UnifiedIR, String> {
    // Use existing cad_import pipeline
    let drawing_ir = crate::cad_import::import_dxf_to_ir(path)?;

    let mut ir = UnifiedIR {
        source_format: "dxf".into(),
        source_file: path.into(),
        units: drawing_ir.units.clone(),
        grids: Some(drawing_ir.grids.clone()),
        levels: drawing_ir.levels.clone(),
        ..Default::default()
    };

    // Convert columns to members
    for col in &drawing_ir.columns {
        ir.members.push(IrMember {
            id: col.id.clone(),
            member_type: MemberType::Column,
            start: [col.position[0], col.position[1], col.base_level],
            end: [col.position[0], col.position[1], col.top_level],
            profile: col.profile.clone(),
            material: None,
        });
    }

    // Convert beams to members
    for beam in &drawing_ir.beams {
        ir.members.push(IrMember {
            id: beam.id.clone(),
            member_type: MemberType::Beam,
            start: [beam.start_pos[0], beam.start_pos[1], beam.elevation],
            end: [beam.end_pos[0], beam.end_pos[1], beam.elevation],
            profile: beam.profile.clone(),
            material: None,
        });
    }

    ir.stats.member_count = ir.members.len();

    Ok(ir)
}

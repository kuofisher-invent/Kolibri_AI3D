//! Steel Parser — identifies columns, beams, and base plates from DXF geometry

use super::geometry_parser::RawGeometry;
use super::ir::*;

pub fn parse_steel_elements(geom: &RawGeometry, grids: &GridSystem, levels: &[LevelDef]) -> (Vec<ColumnDef>, Vec<BeamDef>, Vec<BasePlateDef>) {
    let mut columns = Vec::new();
    let mut beams = Vec::new();
    let mut base_plates = Vec::new();

    let base_level = levels.iter().map(|l| l.elevation).fold(f64::MAX, f64::min).max(0.0);
    let top_level = levels.iter().map(|l| l.elevation).fold(0.0_f64, f64::max).max(base_level + 3000.0);

    // Detect columns at grid intersections
    for xg in &grids.x_grids {
        for yg in &grids.y_grids {
            let col_id = format!("COL_{}_{}", xg.name, yg.name);

            // Check if there's a block/symbol near this grid intersection (wider 1000mm radius)
            let has_symbol = geom.blocks.iter().any(|b| {
                (b.insert_point[0] - xg.position).abs() < 1000.0 &&
                (b.insert_point[1] - yg.position).abs() < 1000.0
            });

            // Check if there's a closed polyline (column outline) near intersection
            let has_outline = geom.polylines.iter().any(|p| {
                if !p.closed || p.points.len() < 4 { return false; }
                let cx: f64 = p.points.iter().map(|pt| pt[0]).sum::<f64>() / p.points.len() as f64;
                let cy: f64 = p.points.iter().map(|pt| pt[1]).sum::<f64>() / p.points.len() as f64;
                (cx - xg.position).abs() < 300.0 && (cy - yg.position).abs() < 300.0
            });

            if has_symbol || has_outline || (grids.x_grids.len() > 1 && grids.y_grids.len() > 1) {
                columns.push(ColumnDef {
                    id: col_id,
                    grid_x: xg.name.clone(),
                    grid_y: yg.name.clone(),
                    position: [xg.position, yg.position],
                    base_level,
                    top_level,
                    profile: None,
                });
            }
        }
    }

    // Detect beams between adjacent columns (X direction)
    for i in 0..grids.x_grids.len().saturating_sub(1) {
        for yg in &grids.y_grids {
            let from = &grids.x_grids[i];
            let to = &grids.x_grids[i + 1];
            beams.push(BeamDef {
                id: format!("BM_{}_{}_{}", from.name, to.name, yg.name),
                from_grid: format!("{}{}", from.name, yg.name),
                to_grid: format!("{}{}", to.name, yg.name),
                elevation: top_level,
                start_pos: [from.position, yg.position],
                end_pos: [to.position, yg.position],
                profile: None,
            });
        }
    }
    // Y-direction beams
    for xg in &grids.x_grids {
        for j in 0..grids.y_grids.len().saturating_sub(1) {
            let from = &grids.y_grids[j];
            let to = &grids.y_grids[j + 1];
            beams.push(BeamDef {
                id: format!("BM_{}_{}{}", xg.name, from.name, to.name),
                from_grid: format!("{}{}", xg.name, from.name),
                to_grid: format!("{}{}", xg.name, to.name),
                elevation: top_level,
                start_pos: [xg.position, from.position],
                end_pos: [xg.position, to.position],
                profile: None,
            });
        }
    }

    // Detect base plates
    for col in &columns {
        base_plates.push(BasePlateDef {
            id: format!("BP_{}", col.id),
            position: col.position,
            width: 500.0,
            depth: 500.0,
            height: 30.0,
        });
    }

    (columns, beams, base_plates)
}

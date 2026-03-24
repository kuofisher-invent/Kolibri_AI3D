//! Steel Builder — converts DrawingIR into Kolibri Scene objects

use crate::cad_import::ir::DrawingIR;
use crate::scene::{Scene, MaterialKind};

pub struct BuildResult {
    pub columns_created: usize,
    pub beams_created: usize,
    pub plates_created: usize,
    pub ids: Vec<String>,
}

/// Build 3D scene from IR data
pub fn build_from_ir(scene: &mut Scene, ir: &DrawingIR) -> BuildResult {
    let mut result = BuildResult {
        columns_created: 0, beams_created: 0, plates_created: 0, ids: Vec::new(),
    };

    let col_mat = MaterialKind::Steel;
    let beam_mat = MaterialKind::Steel;
    let plate_mat = MaterialKind::Metal;

    // H-section column dimensions (H300x300x10x15 typical)
    let h_d = 300.0_f32;   // overall depth
    let h_bf = 300.0_f32;  // flange width
    let h_tf = 15.0_f32;   // flange thickness
    let h_tw = 10.0_f32;   // web thickness

    // Build columns as H-sections (2 flanges + 1 web)
    for col in &ir.columns {
        let height = (col.top_level - col.base_level) as f32;
        if height < 10.0 { continue; }

        let cx = col.position[0] as f32;
        let cy = col.base_level as f32;
        let cz = col.position[1] as f32;

        // Bottom flange
        let id_bf = scene.add_box(
            format!("{}_BF", col.id),
            [cx - h_bf / 2.0, cy, cz - h_d / 2.0],
            h_bf, height, h_tf,
            col_mat,
        );
        if let Some(obj) = scene.objects.get_mut(&id_bf) {
            obj.component_kind = crate::collision::ComponentKind::Column;
        }
        result.ids.push(id_bf);

        // Top flange
        let id_tf = scene.add_box(
            format!("{}_TF", col.id),
            [cx - h_bf / 2.0, cy, cz + h_d / 2.0 - h_tf],
            h_bf, height, h_tf,
            col_mat,
        );
        if let Some(obj) = scene.objects.get_mut(&id_tf) {
            obj.component_kind = crate::collision::ComponentKind::Column;
        }
        result.ids.push(id_tf);

        // Web
        let web_h = h_d - 2.0 * h_tf;
        let id_w = scene.add_box(
            format!("{}_W", col.id),
            [cx - h_tw / 2.0, cy, cz - web_h / 2.0],
            h_tw, height, web_h,
            col_mat,
        );
        if let Some(obj) = scene.objects.get_mut(&id_w) {
            obj.component_kind = crate::collision::ComponentKind::Column;
        }
        result.ids.push(id_w);

        result.columns_created += 1;
    }

    // Build beams as H-sections
    let bm_d = 400.0_f32;   // beam depth
    let bm_bf = 200.0_f32;  // beam flange width
    let bm_tf = 13.0_f32;   // beam flange thickness
    let bm_tw = 8.0_f32;    // beam web thickness

    for beam in &ir.beams {
        let x1 = beam.start_pos[0] as f32;
        let z1 = beam.start_pos[1] as f32;
        let x2 = beam.end_pos[0] as f32;
        let z2 = beam.end_pos[1] as f32;
        let y_top = beam.elevation as f32;
        let y_bot = y_top - bm_d;

        let dx = x2 - x1;
        let dz = z2 - z1;
        let length = (dx * dx + dz * dz).sqrt();
        if length < 10.0 { continue; }

        if dx.abs() > dz.abs() {
            // X-direction beam (flanges in Z, web in Y)
            let min_x = x1.min(x2);

            // Bottom flange
            let id1 = scene.add_box(
                format!("{}_BF", beam.id),
                [min_x, y_bot, z1 - bm_bf / 2.0],
                length, bm_tf, bm_bf,
                beam_mat,
            );
            if let Some(obj) = scene.objects.get_mut(&id1) {
                obj.component_kind = crate::collision::ComponentKind::Beam;
            }
            result.ids.push(id1);

            // Top flange
            let id2 = scene.add_box(
                format!("{}_TF", beam.id),
                [min_x, y_top - bm_tf, z1 - bm_bf / 2.0],
                length, bm_tf, bm_bf,
                beam_mat,
            );
            if let Some(obj) = scene.objects.get_mut(&id2) {
                obj.component_kind = crate::collision::ComponentKind::Beam;
            }
            result.ids.push(id2);

            // Web
            let web_h = bm_d - 2.0 * bm_tf;
            let id3 = scene.add_box(
                format!("{}_W", beam.id),
                [min_x, y_bot + bm_tf, z1 - bm_tw / 2.0],
                length, web_h, bm_tw,
                beam_mat,
            );
            if let Some(obj) = scene.objects.get_mut(&id3) {
                obj.component_kind = crate::collision::ComponentKind::Beam;
            }
            result.ids.push(id3);
        } else {
            // Z-direction beam (flanges in X, web in Y)
            let min_z = z1.min(z2);

            // Bottom flange
            let id1 = scene.add_box(
                format!("{}_BF", beam.id),
                [x1 - bm_bf / 2.0, y_bot, min_z],
                bm_bf, bm_tf, length,
                beam_mat,
            );
            if let Some(obj) = scene.objects.get_mut(&id1) {
                obj.component_kind = crate::collision::ComponentKind::Beam;
            }
            result.ids.push(id1);

            // Top flange
            let id2 = scene.add_box(
                format!("{}_TF", beam.id),
                [x1 - bm_bf / 2.0, y_top - bm_tf, min_z],
                bm_bf, bm_tf, length,
                beam_mat,
            );
            if let Some(obj) = scene.objects.get_mut(&id2) {
                obj.component_kind = crate::collision::ComponentKind::Beam;
            }
            result.ids.push(id2);

            // Web
            let web_h = bm_d - 2.0 * bm_tf;
            let id3 = scene.add_box(
                format!("{}_W", beam.id),
                [x1 - bm_tw / 2.0, y_bot + bm_tf, min_z],
                bm_tw, web_h, length,
                beam_mat,
            );
            if let Some(obj) = scene.objects.get_mut(&id3) {
                obj.component_kind = crate::collision::ComponentKind::Beam;
            }
            result.ids.push(id3);
        }
        result.beams_created += 1;
    }

    // Build base plates
    for bp in &ir.base_plates {
        let x = bp.position[0] as f32 - bp.width as f32 / 2.0;
        let z = bp.position[1] as f32 - bp.depth as f32 / 2.0;
        let y = -bp.height as f32;

        let id = scene.add_box(
            bp.id.clone(),
            [x, y, z],
            bp.width as f32, bp.height as f32, bp.depth as f32,
            plate_mat,
        );
        if let Some(obj) = scene.objects.get_mut(&id) {
            obj.component_kind = crate::collision::ComponentKind::Plate;
        }
        result.ids.push(id);
        result.plates_created += 1;
    }

    result
}

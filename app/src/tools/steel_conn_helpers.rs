//! 鋼構接頭輔助函式 — 群組操作、截面辨識、位置計算、螺栓生成
//! 從 steel_connections.rs 拆分，減少單檔行數

use crate::app::KolibriApp;
use crate::scene::{MaterialKind, SceneObject, Shape};
use kolibri_core::collision::ComponentKind;
use kolibri_core::steel_connection::*;

/// 計算物件 AABB 邊界 (min, max)
pub(crate) fn obj_bounds(obj: &SceneObject) -> ([f32; 3], [f32; 3]) {
    let p = obj.position;
    match &obj.shape {
        Shape::Box { width, height, depth } => {
            (p, [p[0] + width, p[1] + height, p[2] + depth])
        }
        Shape::Cylinder { radius, height, .. } => {
            ([p[0] - radius, p[1], p[2] - radius],
             [p[0] + radius, p[1] + height, p[2] + radius])
        }
        _ => (p, p),
    }
}

/// 計算物件幾何中心
pub(crate) fn obj_center(obj: &SceneObject) -> [f32; 3] {
    let (min, max) = obj_bounds(obj);
    [(min[0]+max[0])/2.0, (min[1]+max[1])/2.0, (min[2]+max[2])/2.0]
}

impl KolibriApp {
    /// 辨識選取物件中的梁和柱
    pub(crate) fn identify_beam_column(&self, ids: &[String]) -> Option<(String, String)> {
        let mut beam_id = None;
        let mut col_id = None;

        for id in ids {
            // 先找 group 的子物件
            let check_ids = self.get_group_member_ids(id);
            for cid in &check_ids {
                if let Some(obj) = self.scene.objects.get(cid) {
                    match obj.component_kind {
                        ComponentKind::Beam => { beam_id = Some(id.clone()); }
                        ComponentKind::Column => { col_id = Some(id.clone()); }
                        _ => {}
                    }
                }
            }
            // 也檢查物件本身
            if let Some(obj) = self.scene.objects.get(id) {
                match obj.component_kind {
                    ComponentKind::Beam => { beam_id = Some(id.clone()); }
                    ComponentKind::Column => { col_id = Some(id.clone()); }
                    _ => {}
                }
            }
        }

        // 如果只有兩個物件且無法辨識，用名稱推斷
        if beam_id.is_none() || col_id.is_none() {
            if ids.len() >= 2 {
                for id in ids {
                    let name = self.scene.objects.get(id).map_or(String::new(), |o| o.name.to_uppercase());
                    if name.contains("BM") || name.contains("BEAM") {
                        beam_id = Some(id.clone());
                    } else if name.contains("COL") || name.contains("COLUMN") {
                        col_id = Some(id.clone());
                    }
                }
            }
        }

        // 最後嘗試：依重力路徑推斷
        // 柱 = 垂直構件（Y 跨度 >> XZ 跨度，且底部接近地面）
        // 梁 = 水平構件（XZ 跨度 >> Y 跨度）
        if beam_id.is_none() || col_id.is_none() {
            if ids.len() >= 2 {
                for id in ids {
                    let (bmin, bmax) = self.get_group_bounds(id);
                    let span_y = bmax[1] - bmin[1];
                    let span_xz = ((bmax[0]-bmin[0]).max(bmax[2]-bmin[2]));
                    let aspect = span_y / span_xz.max(1.0);

                    if aspect > 3.0 {
                        // Y 跨度遠大於 XZ → 垂直構件 = 柱
                        if col_id.is_none() { col_id = Some(id.clone()); }
                    } else if aspect < 0.5 {
                        // XZ 跨度遠大於 Y → 水平構件 = 梁
                        if beam_id.is_none() { beam_id = Some(id.clone()); }
                    }
                }
                // 如果還無法判斷，用 Y 跨度比較
                if beam_id.is_none() || col_id.is_none() {
                    let h0 = self.get_member_height(&ids[0]);
                    let h1 = self.get_member_height(&ids[1]);
                    if h0 > h1 {
                        if col_id.is_none() { col_id = Some(ids[0].clone()); }
                        if beam_id.is_none() { beam_id = Some(ids[1].clone()); }
                    } else {
                        if col_id.is_none() { col_id = Some(ids[1].clone()); }
                        if beam_id.is_none() { beam_id = Some(ids[0].clone()); }
                    }
                }
            }
        }

        match (beam_id, col_id) {
            (Some(b), Some(c)) => Some((b, c)),
            _ => None,
        }
    }

    /// 取得構件的 H 截面參數 (H, B, tw, tf)
    pub(crate) fn get_member_section(&self, id: &str) -> (f32, f32, f32, f32) {
        // 查找群組子物件來推斷截面（H 型鋼 = 2 翼板 + 1 腹板）
        let child_ids = self.get_group_member_ids(id);
        if child_ids.len() >= 3 {
            // 收集所有子物件的 shape 尺寸
            let mut shapes: Vec<(f32, f32, f32)> = Vec::new();
            for cid in &child_ids {
                if let Some(obj) = self.scene.objects.get(cid) {
                    if let crate::scene::Shape::Box { width, height, depth } = &obj.shape {
                        shapes.push((*width, *height, *depth));
                    }
                }
            }
            if shapes.len() >= 3 {
                // 找構件長度方向：取所有子物件中最大尺寸的軸
                // 柱：height 最大（Y 方向）
                // 梁：width 或 depth 最大（X 或 Z 方向）
                let max_val = shapes.iter()
                    .flat_map(|(w,h,d)| [*w, *h, *d])
                    .fold(0.0_f32, f32::max);

                // 收集各子物件的截面尺寸（去掉長度方向）
                let mut flanges: Vec<(f32, f32)> = Vec::new(); // (截面寬, 截面厚)
                let mut web: Option<(f32, f32)> = None;        // (截面高, 截面厚)

                for (w, h, d) in &shapes {
                    // 找出哪個軸是長度方向（≈ max_val）
                    let cross = if (*w - max_val).abs() < 1.0 {
                        (*h, *d) // 長度在 X → 截面在 YZ
                    } else if (*h - max_val).abs() < 1.0 {
                        (*w, *d) // 長度在 Y → 截面在 XZ（柱）
                    } else {
                        (*w, *h) // 長度在 Z → 截面在 XY
                    };
                    let (a, b) = (cross.0.max(cross.1), cross.0.min(cross.1));
                    // a=較大截面尺寸, b=較小截面尺寸
                    // 翼板：a > b*3 且 b < 30（薄且寬）
                    // 腹板：a > b*3 且 a > 50（高且薄）
                    if b < 30.0 && a > b * 3.0 {
                        if flanges.len() < 2 {
                            flanges.push((a, b)); // (翼板寬, 翼板厚)
                        } else {
                            web = Some((a, b)); // 多的當腹板
                        }
                    } else {
                        web = Some((a, b)); // (腹板高, 腹板厚)
                    }
                }

                if let (Some((wh, wt)), true) = (web, !flanges.is_empty()) {
                    let (fb, ft) = flanges[0];
                    let h_sec = wh + 2.0 * ft; // 截面高 = 腹板高 + 2×翼板厚
                    return (h_sec, fb, wt, ft);
                }
            }
        }

        // 嘗試直接從物件 shape 推斷
        if let Some(obj) = self.scene.objects.get(id) {
            if let crate::scene::Shape::Box { width, height, depth } = &obj.shape {
                return (*height, width.max(*depth), 8.0, 12.0);
            }
        }

        // 預設 H300x150x6x9
        (300.0, 150.0, 6.0, 9.0)
    }

    /// 取得構件高度（Y 方向）
    pub(crate) fn get_member_height(&self, id: &str) -> f32 {
        let child_ids = self.get_group_member_ids(id);
        let mut max_h = 0.0_f32;
        for cid in &child_ids {
            if let Some(obj) = self.scene.objects.get(cid) {
                if let crate::scene::Shape::Box { height, .. } = &obj.shape {
                    max_h = max_h.max(*height);
                }
            }
        }
        if max_h == 0.0 {
            if let Some(obj) = self.scene.objects.get(id) {
                if let crate::scene::Shape::Box { height, .. } = &obj.shape {
                    return *height;
                }
            }
        }
        max_h
    }

    /// 取得群組的子物件 ID（如果是群組的話）
    pub(crate) fn get_group_member_ids(&self, id: &str) -> Vec<String> {
        // 檢查是否為群組
        if let Some(group) = self.scene.groups.get(id) {
            return group.children.clone();
        }
        // 檢查物件是否屬於某群組
        if let Some(obj) = self.scene.objects.get(id) {
            if let Some(ref pid) = obj.parent_id {
                if let Some(group) = self.scene.groups.get(pid) {
                    return group.children.clone();
                }
            }
        }
        // 不是群組，回傳自身
        vec![id.to_string()]
    }

    /// 取得群組或物件的 AABB 幾何中心（正確處理 Box position=左下角）
    pub(crate) fn get_group_center(&self, id: &str) -> [f32; 3] {
        let child_ids = self.get_group_member_ids(id);
        if child_ids.is_empty() {
            if let Some(obj) = self.scene.objects.get(id) {
                return obj_center(obj);
            }
            return [0.0; 3];
        }

        // 計算所有子物件的 AABB → 取中心
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for cid in &child_ids {
            if let Some(obj) = self.scene.objects.get(cid) {
                let (obj_min, obj_max) = obj_bounds(obj);
                for i in 0..3 {
                    min[i] = min[i].min(obj_min[i]);
                    max[i] = max[i].max(obj_max[i]);
                }
            }
        }
        [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0, (min[2] + max[2]) / 2.0]
    }

    /// 取得群組或物件的 AABB 邊界 (min, max)
    pub(crate) fn get_group_bounds(&self, id: &str) -> ([f32; 3], [f32; 3]) {
        let child_ids = self.get_group_member_ids(id);
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        let ids = if child_ids.is_empty() { vec![id.to_string()] } else { child_ids };
        for cid in &ids {
            if let Some(obj) = self.scene.objects.get(cid) {
                let (obj_min, obj_max) = obj_bounds(obj);
                for i in 0..3 {
                    min[i] = min[i].min(obj_min[i]);
                    max[i] = max[i].max(obj_max[i]);
                }
            }
        }
        (min, max)
    }

    /// 計算接頭位置（梁端靠近柱的那一端）
    pub(crate) fn calc_connection_position(&self, beam_id: &str, col_id: &str) -> [f32; 3] {
        let col_center = self.get_group_center(col_id);
        let (beam_min, beam_max) = self.get_group_bounds(beam_id);
        let beam_center = self.get_group_center(beam_id);

        // 判斷梁沿哪個軸延伸
        let span_x = beam_max[0] - beam_min[0];
        let span_z = beam_max[2] - beam_min[2];

        if span_x > span_z {
            // 梁沿 X 方向 — 找最近的 X 端
            let beam_end_x = if (beam_min[0] - col_center[0]).abs() < (beam_max[0] - col_center[0]).abs() {
                beam_min[0]
            } else {
                beam_max[0]
            };
            // 接頭 X = 梁端 X，Y = 梁中心 Y，Z = 梁中心 Z（與柱對齊）
            [beam_end_x, beam_center[1], beam_center[2]]
        } else {
            // 梁沿 Z 方向
            let beam_end_z = if (beam_min[2] - col_center[2]).abs() < (beam_max[2] - col_center[2]).abs() {
                beam_min[2]
            } else {
                beam_max[2]
            };
            [beam_center[0], beam_center[1], beam_end_z]
        }
    }

    /// 判斷梁相對於柱的方向（回傳 true=X方向, false=Z方向）和方向符號
    pub(crate) fn beam_direction(&self, beam_id: &str, col_id: &str) -> (bool, f32) {
        let col_center = self.get_group_center(col_id);
        let beam_center = self.get_group_center(beam_id);
        let dx = beam_center[0] - col_center[0];
        let dz = beam_center[2] - col_center[2];
        if dx.abs() > dz.abs() {
            (true, if dx > 0.0 { 1.0 } else { -1.0 })
        } else {
            (false, if dz > 0.0 { 1.0 } else { -1.0 })
        }
    }

    /// 把接頭本地座標 (local_x, local_y, local_z) 轉換為世界座標
    /// 本地座標系：X=板件水平, Y=板件垂直(高度), Z=板件法線(沿梁方向)
    pub(crate) fn conn_local_to_world(
        &self, conn_pos: [f32; 3], local: [f32; 3],
        is_x_dir: bool, sign: f32,
    ) -> [f32; 3] {
        if is_x_dir {
            // 梁沿 X → 端板面在 YZ 平面 → local_x→Z, local_y→Y, local_z→X
            [
                conn_pos[0] + local[2] * sign,
                conn_pos[1] + local[1],
                conn_pos[2] + local[0],
            ]
        } else {
            // 梁沿 Z → 端板面在 XY 平面 → local_x→X, local_y→Y, local_z→Z
            [
                conn_pos[0] + local[0],
                conn_pos[1] + local[1],
                conn_pos[2] + local[2] * sign,
            ]
        }
    }

    /// 計算板件在世界座標的位置（Box 左下角）
    pub(crate) fn calc_plate_world_pos(
        &self, conn_pos: [f32; 3], plate: &ConnectionPlate,
        is_x_dir: bool, sign: f32,
    ) -> ([f32; 3], f32, f32, f32) {
        // 板件中心在本地座標
        let center_local = plate.position; // [local_x, local_y, local_z]
        let center_world = self.conn_local_to_world(conn_pos, center_local, is_x_dir, sign);

        // Box 尺寸：width=板寬, height=板高, depth=板厚
        // 在世界座標中，根據方向分配 w/h/d
        if is_x_dir {
            // 端板在 YZ 平面：Box(厚=X, 高=Y, 寬=Z)
            let bw = plate.thickness; // X 方向
            let bh = plate.height;    // Y 方向
            let bd = plate.width;     // Z 方向
            let pos = [
                center_world[0] - bw / 2.0,
                center_world[1] - bh / 2.0,
                center_world[2] - bd / 2.0,
            ];
            (pos, bw, bh, bd)
        } else {
            // 端板在 XY 平面：Box(寬=X, 高=Y, 厚=Z)
            let bw = plate.width;
            let bh = plate.height;
            let bd = plate.thickness;
            let pos = [
                center_world[0] - bw / 2.0,
                center_world[1] - bh / 2.0,
                center_world[2] - bd / 2.0,
            ];
            (pos, bw, bh, bd)
        }
    }

    /// 生成螺栓群組的 3D mesh（含桿身+頭+墊圈+螺帽+孔徑標記）
    pub(crate) fn create_bolt_group_meshes(
        &mut self, bg: &BoltGroup, conn_pos: [f32; 3],
        _beam_id: &str, _col_id: &str,
    ) -> Vec<String> {
        let mut ids = Vec::new();
        let bolt_r = bg.bolt_size.diameter() / 2.0;
        let hole_r = bg.hole_diameter / 2.0;       // 孔徑半徑
        let head_r = bg.bolt_size.head_across_flats() / 2.0;
        let head_t = bg.bolt_size.head_thickness();
        let washer_r = head_r + 2.0;               // 墊圈比螺栓頭大 2mm
        let washer_t = 3.0;                         // 墊圈厚 3mm
        let nut_t = bg.bolt_size.diameter() * 0.8;  // 螺帽厚 ≈ 0.8d
        let grip = 50.0;                             // 夾持長度（板厚總和）

        // 輸出孔位資訊到 Console
        self.console_push("BOLT", format!(
            "螺栓組 {} {} | {}×{} = {} 顆 | 孔Ø{:.0}mm | 邊距{:.0}mm | 間距{:.0}mm",
            bg.bolt_size.label(), bg.bolt_grade.label(),
            bg.rows, bg.cols, bg.positions.len(),
            bg.hole_diameter, bg.edge_dist, bg.row_spacing,
        ));

        for (i, bp) in bg.positions.iter().enumerate() {
            let bolt_name = format!("{}_{}", bg.bolt_size.label(), i + 1);
            let bolt_pos = [
                conn_pos[0] + bp[0],
                conn_pos[1] + bp[1],
                conn_pos[2] + bp[2],
            ];

            // 1. 螺栓孔標記（透明圓柱，比螺栓大，代表孔徑）
            let hole_id = self.scene.insert_cylinder_raw(
                format!("{}_hole", bolt_name),
                [bolt_pos[0], bolt_pos[1] - 1.0, bolt_pos[2]],
                hole_r, grip + 2.0, 12,
                MaterialKind::Custom([0.2, 0.2, 0.2, 0.3]), // 深灰半透明
            );
            if let Some(obj) = self.scene.objects.get_mut(&hole_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(hole_id);

            // 2. 螺栓桿身（實心）
            let shank_id = self.scene.insert_cylinder_raw(
                format!("{}_shank", bolt_name),
                bolt_pos,
                bolt_r, grip + head_t + nut_t, 8, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&shank_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(shank_id);

            // 3. 螺栓頭（上方）
            let head_pos = [bolt_pos[0], bolt_pos[1] + grip, bolt_pos[2]];
            let head_id = self.scene.insert_cylinder_raw(
                format!("{}_head", bolt_name),
                head_pos,
                head_r, head_t, 6, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&head_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(head_id);

            // 4. 墊圈（螺栓頭下方）
            let washer_pos = [bolt_pos[0], bolt_pos[1] + grip - washer_t, bolt_pos[2]];
            let washer_id = self.scene.insert_cylinder_raw(
                format!("{}_washer", bolt_name),
                washer_pos,
                washer_r, washer_t, 12, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&washer_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(washer_id);

            // 5. 螺帽（底部）
            let nut_pos = [bolt_pos[0], bolt_pos[1] - nut_t, bolt_pos[2]];
            let nut_id = self.scene.insert_cylinder_raw(
                format!("{}_nut", bolt_name),
                nut_pos,
                head_r, nut_t, 6, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&nut_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(nut_id);
        }

        ids
    }

    /// 生成螺栓群組（使用本地→世界座標轉換）
    /// bolt positions 是相對於接頭中心的本地座標 [local_x, local_y, 0]
    pub(crate) fn create_bolt_group_world(
        &mut self, bg: &BoltGroup, conn_pos: [f32; 3],
        is_x_dir: bool, sign: f32,
    ) -> Vec<String> {
        let mut ids = Vec::new();
        let bolt_r = bg.bolt_size.diameter() / 2.0;
        let hole_r = bg.hole_diameter / 2.0;
        let head_r = bg.bolt_size.head_across_flats() / 2.0;
        let head_t = bg.bolt_size.head_thickness();
        let washer_r = head_r + 2.0;
        let washer_t = 3.0;
        let nut_t = bg.bolt_size.diameter() * 0.8;
        let grip = 50.0;

        self.console_push("BOLT", format!(
            "螺栓 {} {} | {}×{} = {} 顆 | 孔Ø{:.0} | 邊距{:.0} | 間距{:.0}",
            bg.bolt_size.label(), bg.bolt_grade.label(),
            bg.rows, bg.cols, bg.positions.len(),
            bg.hole_diameter, bg.edge_dist, bg.row_spacing,
        ));

        for (i, bp) in bg.positions.iter().enumerate() {
            let bolt_name = format!("{}_{}", bg.bolt_size.label(), i + 1);

            // bp = [local_x, local_y, 0] → 轉世界座標
            // 螺栓軸向沿板件法線（local_z）
            let bolt_center = self.conn_local_to_world(conn_pos, *bp, is_x_dir, sign);

            // 螺栓沿 local_z 方向延伸 — 在世界座標中是哪個軸？
            let bolt_axis_offset = if is_x_dir {
                // local_z → world X (× sign)
                [sign, 0.0, 0.0]
            } else {
                // local_z → world Z (× sign)
                [0.0, 0.0, sign]
            };

            // 孔標記（圓柱沿 Y 軸，穿透板件）
            let hole_id = self.scene.insert_cylinder_raw(
                format!("{}_hole", bolt_name),
                [bolt_center[0], bolt_center[1] - grip / 2.0, bolt_center[2]],
                hole_r, grip, 12,
                MaterialKind::Custom([0.2, 0.2, 0.2, 0.3]),
            );
            if let Some(obj) = self.scene.objects.get_mut(&hole_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(hole_id);

            // 螺栓桿身（沿板法線方向 = 用 bolt_axis_offset）
            // 簡化：螺栓都沿 Y 軸放（垂直），因為端板也是垂直的
            // 實際上螺栓穿透端板方向是水平的，但 Cylinder 只能沿 Y 軸
            // 所以用多個小 Box 代替或直接用 Y 軸 cylinder
            let shank_id = self.scene.insert_cylinder_raw(
                format!("{}_shank", bolt_name),
                [bolt_center[0] - bolt_axis_offset[0] * grip / 2.0,
                 bolt_center[1],
                 bolt_center[2] - bolt_axis_offset[2] * grip / 2.0],
                bolt_r, grip, 8, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&shank_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(shank_id);

            // 螺栓頭（板外側）
            let head_pos = [
                bolt_center[0] + bolt_axis_offset[0] * (grip / 2.0 + head_t / 2.0),
                bolt_center[1],
                bolt_center[2] + bolt_axis_offset[2] * (grip / 2.0 + head_t / 2.0),
            ];
            let head_id = self.scene.insert_cylinder_raw(
                format!("{}_head", bolt_name),
                head_pos,
                head_r, head_t, 6, MaterialKind::Metal,
            );
            if let Some(obj) = self.scene.objects.get_mut(&head_id) {
                obj.component_kind = ComponentKind::Bolt;
            }
            ids.push(head_id);
        }

        ids
    }
}

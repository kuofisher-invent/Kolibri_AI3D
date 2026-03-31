//! 台灣 CNS / ASTM 標準管材規格目錄
//!
//! 參考標準：
//! - CNS 4055（PVC 給水管）
//! - CNS 1298（PVC 排水管）
//! - CNS 6445 / ANSI C80.3（EMT 電線導管）
//! - CNS 4626 / ASTM A795 SCH40（消防鍍鋅鐵管）
//! - CNS 6331 / ASTM A106 SCH40（碳鋼管）
//! - CNS 6259 / ASTM A312 SCH10S（不鏽鋼管）
//! - CNS 2433 / ASTM B88 Type L（銅管）

use crate::pipe_data::{PipeSpec, PipeSystem};

/// 管材規格目錄
pub struct PipeCatalog;

impl PipeCatalog {
    /// 取得指定系統的所有標準規格
    pub fn specs_for(system: PipeSystem) -> Vec<PipeSpec> {
        match system {
            PipeSystem::PvcWater => Self::pvc_water(),
            PipeSystem::PvcDrain => Self::pvc_drain(),
            PipeSystem::ElectricalConduit => Self::emt_conduit(),
            PipeSystem::IronFireSprinkler => Self::iron_fire(),
            PipeSystem::SteelProcess => Self::steel_process(),
            PipeSystem::StainlessSteel => Self::stainless(),
            PipeSystem::Copper => Self::copper(),
        }
    }

    /// 取得預設規格（最接近 DN25 的）
    pub fn default_spec(system: PipeSystem) -> PipeSpec {
        Self::specs_for(system).into_iter()
            .find(|s| s.nominal_dn >= 25.0)
            .unwrap_or_else(|| Self::specs_for(system).remove(0))
    }

    // ── PVC 給水管（CNS 4055 / SCH40 相當）──
    // 外徑/壁厚依台灣市場常用規格
    fn pvc_water() -> Vec<PipeSpec> {
        vec![
            spec(PipeSystem::PvcWater, 15.0,  21.3,  2.3, "DN15 (1/2\") PVC"),
            spec(PipeSystem::PvcWater, 20.0,  26.7,  2.5, "DN20 (3/4\") PVC"),
            spec(PipeSystem::PvcWater, 25.0,  33.4,  2.8, "DN25 (1\") PVC"),
            spec(PipeSystem::PvcWater, 32.0,  42.2,  3.0, "DN32 (1-1/4\") PVC"),
            spec(PipeSystem::PvcWater, 40.0,  48.3,  3.2, "DN40 (1-1/2\") PVC"),
            spec(PipeSystem::PvcWater, 50.0,  60.3,  3.5, "DN50 (2\") PVC"),
            spec(PipeSystem::PvcWater, 65.0,  76.2,  4.0, "DN65 (2-1/2\") PVC"),
            spec(PipeSystem::PvcWater, 80.0,  88.9,  4.5, "DN80 (3\") PVC"),
            spec(PipeSystem::PvcWater, 100.0, 114.3, 5.5, "DN100 (4\") PVC"),
            spec(PipeSystem::PvcWater, 150.0, 168.3, 7.0, "DN150 (6\") PVC"),
            spec(PipeSystem::PvcWater, 200.0, 219.1, 8.5, "DN200 (8\") PVC"),
        ]
    }

    // ── PVC 排水管（CNS 1298）──
    // 台灣常見灰色排水管（薄壁）
    fn pvc_drain() -> Vec<PipeSpec> {
        vec![
            spec(PipeSystem::PvcDrain, 40.0,  48.0,  1.8, "DN40 (1-1/2\") 排水"),
            spec(PipeSystem::PvcDrain, 50.0,  60.0,  2.0, "DN50 (2\") 排水"),
            spec(PipeSystem::PvcDrain, 65.0,  76.0,  2.2, "DN65 (2-1/2\") 排水"),
            spec(PipeSystem::PvcDrain, 80.0,  89.0,  2.8, "DN80 (3\") 排水"),
            spec(PipeSystem::PvcDrain, 100.0, 114.0, 3.0, "DN100 (4\") 排水"),
            spec(PipeSystem::PvcDrain, 125.0, 140.0, 3.5, "DN125 (5\") 排水"),
            spec(PipeSystem::PvcDrain, 150.0, 165.0, 4.0, "DN150 (6\") 排水"),
            spec(PipeSystem::PvcDrain, 200.0, 216.0, 5.0, "DN200 (8\") 排水"),
            spec(PipeSystem::PvcDrain, 250.0, 267.0, 6.0, "DN250 (10\") 排水"),
            spec(PipeSystem::PvcDrain, 300.0, 318.0, 7.0, "DN300 (12\") 排水"),
        ]
    }

    // ── EMT 電線導管（CNS 6445 / ANSI C80.3）──
    // 台灣電氣工程常用
    fn emt_conduit() -> Vec<PipeSpec> {
        vec![
            spec(PipeSystem::ElectricalConduit, 16.0,  17.1, 1.2, "E16 (1/2\") EMT"),
            spec(PipeSystem::ElectricalConduit, 22.0,  22.2, 1.2, "E22 (3/4\") EMT"),
            spec(PipeSystem::ElectricalConduit, 28.0,  28.6, 1.4, "E28 (1\") EMT"),
            spec(PipeSystem::ElectricalConduit, 36.0,  35.1, 1.4, "E36 (1-1/4\") EMT"),
            spec(PipeSystem::ElectricalConduit, 42.0,  41.2, 1.4, "E42 (1-1/2\") EMT"),
            spec(PipeSystem::ElectricalConduit, 54.0,  53.4, 1.65, "E54 (2\") EMT"),
            spec(PipeSystem::ElectricalConduit, 70.0,  73.0, 1.8, "E70 (2-1/2\") EMT"),
            spec(PipeSystem::ElectricalConduit, 82.0,  88.9, 1.8, "E82 (3\") EMT"),
            spec(PipeSystem::ElectricalConduit, 105.0, 114.3, 2.0, "E105 (4\") EMT"),
        ]
    }

    // ── 消防灑水鍍鋅鐵管（CNS 4626 / ASTM A795 SCH40）──
    // 依內政部消防署規定，消防管路使用 SCH40 鍍鋅鐵管
    fn iron_fire() -> Vec<PipeSpec> {
        vec![
            spec(PipeSystem::IronFireSprinkler, 25.0,  33.4,  3.38, "DN25 (1\") SCH40 鍍鋅"),
            spec(PipeSystem::IronFireSprinkler, 32.0,  42.2,  3.56, "DN32 (1-1/4\") SCH40"),
            spec(PipeSystem::IronFireSprinkler, 40.0,  48.3,  3.68, "DN40 (1-1/2\") SCH40"),
            spec(PipeSystem::IronFireSprinkler, 50.0,  60.3,  3.91, "DN50 (2\") SCH40"),
            spec(PipeSystem::IronFireSprinkler, 65.0,  73.0,  5.16, "DN65 (2-1/2\") SCH40"),
            spec(PipeSystem::IronFireSprinkler, 80.0,  88.9,  5.49, "DN80 (3\") SCH40"),
            spec(PipeSystem::IronFireSprinkler, 100.0, 114.3, 6.02, "DN100 (4\") SCH40"),
            spec(PipeSystem::IronFireSprinkler, 125.0, 141.3, 6.55, "DN125 (5\") SCH40"),
            spec(PipeSystem::IronFireSprinkler, 150.0, 168.3, 7.11, "DN150 (6\") SCH40"),
            spec(PipeSystem::IronFireSprinkler, 200.0, 219.1, 8.18, "DN200 (8\") SCH40"),
        ]
    }

    // ── 碳鋼管（CNS 6331 / ASTM A106 SCH40）──
    fn steel_process() -> Vec<PipeSpec> {
        vec![
            spec(PipeSystem::SteelProcess, 15.0,  21.3,  2.77, "DN15 (1/2\") SCH40"),
            spec(PipeSystem::SteelProcess, 20.0,  26.7,  2.87, "DN20 (3/4\") SCH40"),
            spec(PipeSystem::SteelProcess, 25.0,  33.4,  3.38, "DN25 (1\") SCH40"),
            spec(PipeSystem::SteelProcess, 32.0,  42.2,  3.56, "DN32 (1-1/4\") SCH40"),
            spec(PipeSystem::SteelProcess, 40.0,  48.3,  3.68, "DN40 (1-1/2\") SCH40"),
            spec(PipeSystem::SteelProcess, 50.0,  60.3,  3.91, "DN50 (2\") SCH40"),
            spec(PipeSystem::SteelProcess, 65.0,  73.0,  5.16, "DN65 (2-1/2\") SCH40"),
            spec(PipeSystem::SteelProcess, 80.0,  88.9,  5.49, "DN80 (3\") SCH40"),
            spec(PipeSystem::SteelProcess, 100.0, 114.3, 6.02, "DN100 (4\") SCH40"),
            spec(PipeSystem::SteelProcess, 150.0, 168.3, 7.11, "DN150 (6\") SCH40"),
            spec(PipeSystem::SteelProcess, 200.0, 219.1, 8.18, "DN200 (8\") SCH40"),
            spec(PipeSystem::SteelProcess, 250.0, 273.1, 9.27, "DN250 (10\") SCH40"),
            spec(PipeSystem::SteelProcess, 300.0, 323.8, 10.31, "DN300 (12\") SCH40"),
        ]
    }

    // ── 不鏽鋼管（CNS 6259 / ASTM A312 SCH10S）──
    fn stainless() -> Vec<PipeSpec> {
        vec![
            spec(PipeSystem::StainlessSteel, 15.0,  21.3,  2.11, "DN15 (1/2\") SCH10S SUS304"),
            spec(PipeSystem::StainlessSteel, 20.0,  26.7,  2.11, "DN20 (3/4\") SCH10S"),
            spec(PipeSystem::StainlessSteel, 25.0,  33.4,  2.77, "DN25 (1\") SCH10S"),
            spec(PipeSystem::StainlessSteel, 32.0,  42.2,  2.77, "DN32 (1-1/4\") SCH10S"),
            spec(PipeSystem::StainlessSteel, 40.0,  48.3,  2.77, "DN40 (1-1/2\") SCH10S"),
            spec(PipeSystem::StainlessSteel, 50.0,  60.3,  2.77, "DN50 (2\") SCH10S"),
            spec(PipeSystem::StainlessSteel, 65.0,  73.0,  3.05, "DN65 (2-1/2\") SCH10S"),
            spec(PipeSystem::StainlessSteel, 80.0,  88.9,  3.05, "DN80 (3\") SCH10S"),
            spec(PipeSystem::StainlessSteel, 100.0, 114.3, 3.05, "DN100 (4\") SCH10S"),
            spec(PipeSystem::StainlessSteel, 150.0, 168.3, 3.40, "DN150 (6\") SCH10S"),
        ]
    }

    // ── 銅管（CNS 2433 / ASTM B88 Type L）──
    // 台灣冷媒/瓦斯常用
    fn copper() -> Vec<PipeSpec> {
        vec![
            spec(PipeSystem::Copper, 6.35,   6.35,  0.76, "1/4\" Type L 銅管"),
            spec(PipeSystem::Copper, 9.53,   9.53,  0.89, "3/8\" Type L 銅管"),
            spec(PipeSystem::Copper, 12.7,  12.70,  0.89, "1/2\" Type L 銅管"),
            spec(PipeSystem::Copper, 15.88, 15.88,  1.02, "5/8\" Type L 銅管"),
            spec(PipeSystem::Copper, 19.05, 19.05,  1.07, "3/4\" Type L 銅管"),
            spec(PipeSystem::Copper, 22.22, 22.22,  1.14, "7/8\" Type L 銅管"),
            spec(PipeSystem::Copper, 25.4,  28.58,  1.27, "1\" Type L 銅管"),
            spec(PipeSystem::Copper, 31.75, 34.92,  1.40, "1-1/4\" Type L 銅管"),
            spec(PipeSystem::Copper, 38.1,  41.28,  1.52, "1-1/2\" Type L 銅管"),
            spec(PipeSystem::Copper, 50.8,  53.98,  1.78, "2\" Type L 銅管"),
        ]
    }
}

fn spec(system: PipeSystem, dn: f32, od: f32, wt: f32, name: &str) -> PipeSpec {
    PipeSpec {
        system,
        nominal_dn: dn,
        outer_diameter: od,
        wall_thickness: wt,
        spec_name: name.to_string(),
    }
}

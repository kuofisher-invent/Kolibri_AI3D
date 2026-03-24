// ============================================================
//  material_properties.rs
//  工程材質庫 — 視覺 + 物理 + 結構分析預口
// ============================================================

use serde::{Deserialize, Serialize};

// ─── 完整材質定義 ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialProperties {
    // ── 識別 ────────────────────────────────────────────────
    pub id:       String,
    pub name:     String,
    pub category: MaterialCategory,

    // ── 視覺屬性（渲染用）───────────────────────────────────
    pub visual: VisualProps,

    // ── 物理屬性（碰撞 / 重量計算）─────────────────────────
    pub physical: PhysicalProps,

    // ── 結構分析預口（FEA 未來用）──────────────────────────
    pub structural: Option<StructuralProps>,

    // ── 熱學屬性預口 ────────────────────────────────────────
    pub thermal: Option<ThermalProps>,
}

// ─── 材質分類 ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MaterialCategory {
    Metal,
    Concrete,
    Wood,
    Glass,
    Plastic,
    Stone,
    Composite,
    Custom,
}

// ─── 視覺屬性 ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualProps {
    pub color:        [f32; 4],   // RGBA 0..1
    pub roughness:    f32,        // 0=鏡面 1=粗糙
    pub metallic:     f32,        // 0=非金屬 1=金屬
    pub opacity:      f32,        // 1=不透明 0=透明
    pub texture_hint: Option<String>,
}

// ─── 物理屬性 ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalProps {
    /// 密度 kg/m³
    /// 用於計算物件重量：weight = volume * density
    pub density: f64,

    /// 摩擦係數（靜摩擦）
    pub friction_static:  f64,

    /// 摩擦係數（動摩擦）
    pub friction_dynamic: f64,

    /// 恢復係數（碰撞彈性 0=完全非彈性 1=完全彈性）
    pub restitution: f64,
}

// ─── 結構分析屬性（FEA 預口）─────────────────────────────────
// 現在不實作 FEA，但資料欄位先留好
// 未來接 FEA 引擎時直接用，不需要修改資料結構

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralProps {
    /// 楊氏模數 E (Pa) — 材料抵抗變形的能力
    /// 鋼: 200 GPa | 混凝土: 30 GPa | 木材: 12 GPa
    pub youngs_modulus: f64,

    /// 泊松比 ν — 橫向應變 / 縱向應變
    /// 鋼: 0.3 | 混凝土: 0.2 | 橡膠: 0.5
    pub poissons_ratio: f64,

    /// 降伏強度 (Pa) — 開始永久變形的應力
    pub yield_strength: f64,

    /// 極限抗拉強度 (Pa)
    pub tensile_strength: f64,

    /// 抗壓強度 (Pa)
    pub compressive_strength: f64,

    /// 剪切模數 G (Pa)
    pub shear_modulus: f64,

    /// 斷裂韌性 (MPa·√m)
    pub fracture_toughness: Option<f64>,

    /// 疲勞強度 (Pa) — 預口，未來動態分析用
    pub fatigue_strength: Option<f64>,
}

// ─── 熱學屬性（預口）─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalProps {
    /// 熱膨脹係數 (1/°C)
    pub thermal_expansion: f64,

    /// 熱導係數 (W/m·K)
    pub thermal_conductivity: f64,

    /// 比熱容 (J/kg·K)
    pub specific_heat: f64,

    /// 熔點 (°C) — 選填
    pub melting_point: Option<f64>,
}

// ─── 重量計算 ─────────────────────────────────────────────────

impl MaterialProperties {
    /// 根據體積計算重量 (kg)
    /// volume 單位: mm³  → 轉換為 m³ 再乘密度
    pub fn weight_kg(&self, volume_mm3: f64) -> f64 {
        let volume_m3 = volume_mm3 * 1e-9;
        volume_m3 * self.physical.density
    }

    /// 重量 (kN) — 結構工程常用單位
    pub fn weight_kn(&self, volume_mm3: f64) -> f64 {
        self.weight_kg(volume_mm3) * 9.81 / 1000.0
    }
}

// ─── 內建材質庫 ───────────────────────────────────────────────

pub struct MaterialLibrary;

impl MaterialLibrary {
    pub fn get(name: &str) -> MaterialProperties {
        match name.to_lowercase().as_str() {
            "steel" | "鋼" | "鋼材"     => Self::steel(),
            "concrete" | "混凝土"        => Self::concrete(),
            "wood" | "木材" | "木"       => Self::wood(),
            "glass" | "玻璃"             => Self::glass(),
            "aluminum" | "鋁"            => Self::aluminum(),
            "brick" | "磚"               => Self::brick(),
            _                            => Self::default_material(),
        }
    }

    pub fn list() -> Vec<&'static str> {
        vec!["steel", "concrete", "wood", "glass", "aluminum", "brick"]
    }

    // ── 鋼材 ──────────────────────────────────────────────────
    fn steel() -> MaterialProperties {
        MaterialProperties {
            id: "steel".into(), name: "鋼材 (Steel)".into(),
            category: MaterialCategory::Metal,
            visual: VisualProps {
                color: [0.7, 0.7, 0.75, 1.0],
                roughness: 0.3, metallic: 0.95, opacity: 1.0,
                texture_hint: Some("metal_brushed".into()),
            },
            physical: PhysicalProps {
                density: 7850.0,           // kg/m³
                friction_static:  0.74,
                friction_dynamic: 0.57,
                restitution: 0.6,
            },
            structural: Some(StructuralProps {
                youngs_modulus:       200e9,   // 200 GPa
                poissons_ratio:       0.3,
                yield_strength:       250e6,   // 250 MPa (A36)
                tensile_strength:     400e6,   // 400 MPa
                compressive_strength: 250e6,
                shear_modulus:        77e9,
                fracture_toughness:   Some(50.0),
                fatigue_strength:     Some(120e6),
            }),
            thermal: Some(ThermalProps {
                thermal_expansion:    12e-6,
                thermal_conductivity: 50.0,
                specific_heat:        490.0,
                melting_point:        Some(1370.0),
            }),
        }
    }

    // ── 混凝土 ────────────────────────────────────────────────
    fn concrete() -> MaterialProperties {
        MaterialProperties {
            id: "concrete".into(), name: "混凝土 (Concrete)".into(),
            category: MaterialCategory::Concrete,
            visual: VisualProps {
                color: [0.5, 0.5, 0.5, 1.0],
                roughness: 0.9, metallic: 0.0, opacity: 1.0,
                texture_hint: Some("concrete_rough".into()),
            },
            physical: PhysicalProps {
                density: 2400.0,
                friction_static:  0.6,
                friction_dynamic: 0.4,
                restitution: 0.1,
            },
            structural: Some(StructuralProps {
                youngs_modulus:       30e9,    // 30 GPa
                poissons_ratio:       0.2,
                yield_strength:       30e6,    // fc' = 30 MPa
                tensile_strength:     3e6,     // 約為抗壓的 1/10
                compressive_strength: 30e6,
                shear_modulus:        12.5e9,
                fracture_toughness:   Some(1.0),
                fatigue_strength:     None,
            }),
            thermal: Some(ThermalProps {
                thermal_expansion:    10e-6,
                thermal_conductivity: 1.7,
                specific_heat:        880.0,
                melting_point:        None,
            }),
        }
    }

    // ── 木材 ──────────────────────────────────────────────────
    fn wood() -> MaterialProperties {
        MaterialProperties {
            id: "wood".into(), name: "木材 (Wood)".into(),
            category: MaterialCategory::Wood,
            visual: VisualProps {
                color: [0.55, 0.35, 0.15, 1.0],
                roughness: 0.8, metallic: 0.0, opacity: 1.0,
                texture_hint: Some("wood_grain".into()),
            },
            physical: PhysicalProps {
                density: 600.0,            // 中等硬木
                friction_static:  0.5,
                friction_dynamic: 0.35,
                restitution: 0.3,
            },
            structural: Some(StructuralProps {
                youngs_modulus:       12e9,
                poissons_ratio:       0.35,
                yield_strength:       40e6,
                tensile_strength:     80e6,
                compressive_strength: 40e6,
                shear_modulus:        0.75e9,
                fracture_toughness:   Some(10.0),
                fatigue_strength:     None,
            }),
            thermal: Some(ThermalProps {
                thermal_expansion:    5e-6,
                thermal_conductivity: 0.15,
                specific_heat:        1700.0,
                melting_point:        None,
            }),
        }
    }

    // ── 玻璃 ──────────────────────────────────────────────────
    fn glass() -> MaterialProperties {
        MaterialProperties {
            id: "glass".into(), name: "玻璃 (Glass)".into(),
            category: MaterialCategory::Glass,
            visual: VisualProps {
                color: [0.85, 0.92, 1.0, 0.25],
                roughness: 0.05, metallic: 0.0, opacity: 0.25,
                texture_hint: None,
            },
            physical: PhysicalProps {
                density: 2500.0,
                friction_static:  0.9,
                friction_dynamic: 0.4,
                restitution: 0.5,
            },
            structural: Some(StructuralProps {
                youngs_modulus:       70e9,
                poissons_ratio:       0.22,
                yield_strength:       50e6,
                tensile_strength:     50e6,
                compressive_strength: 1000e6,
                shear_modulus:        28e9,
                fracture_toughness:   Some(0.75),
                fatigue_strength:     None,
            }),
            thermal: Some(ThermalProps {
                thermal_expansion:    9e-6,
                thermal_conductivity: 1.0,
                specific_heat:        840.0,
                melting_point:        Some(1400.0),
            }),
        }
    }

    // ── 鋁 ────────────────────────────────────────────────────
    fn aluminum() -> MaterialProperties {
        MaterialProperties {
            id: "aluminum".into(), name: "鋁 (Aluminum)".into(),
            category: MaterialCategory::Metal,
            visual: VisualProps {
                color: [0.82, 0.82, 0.85, 1.0],
                roughness: 0.2, metallic: 1.0, opacity: 1.0,
                texture_hint: Some("metal_aluminum".into()),
            },
            physical: PhysicalProps {
                density: 2700.0,
                friction_static:  0.61,
                friction_dynamic: 0.47,
                restitution: 0.5,
            },
            structural: Some(StructuralProps {
                youngs_modulus:       69e9,
                poissons_ratio:       0.33,
                yield_strength:       276e6,   // 6061-T6
                tensile_strength:     310e6,
                compressive_strength: 276e6,
                shear_modulus:        26e9,
                fracture_toughness:   Some(29.0),
                fatigue_strength:     Some(97e6),
            }),
            thermal: Some(ThermalProps {
                thermal_expansion:    23e-6,
                thermal_conductivity: 167.0,
                specific_heat:        896.0,
                melting_point:        Some(660.0),
            }),
        }
    }

    // ── 磚 ────────────────────────────────────────────────────
    fn brick() -> MaterialProperties {
        MaterialProperties {
            id: "brick".into(), name: "磚 (Brick)".into(),
            category: MaterialCategory::Stone,
            visual: VisualProps {
                color: [0.7, 0.3, 0.2, 1.0],
                roughness: 0.95, metallic: 0.0, opacity: 1.0,
                texture_hint: Some("brick_wall".into()),
            },
            physical: PhysicalProps {
                density: 1900.0,
                friction_static:  0.7,
                friction_dynamic: 0.5,
                restitution: 0.1,
            },
            structural: Some(StructuralProps {
                youngs_modulus:       15e9,
                poissons_ratio:       0.15,
                yield_strength:       10e6,
                tensile_strength:     2e6,
                compressive_strength: 20e6,
                shear_modulus:        6e9,
                fracture_toughness:   None,
                fatigue_strength:     None,
            }),
            thermal: Some(ThermalProps {
                thermal_expansion:    6e-6,
                thermal_conductivity: 0.8,
                specific_heat:        840.0,
                melting_point:        None,
            }),
        }
    }

    fn default_material() -> MaterialProperties {
        MaterialProperties {
            id: "default".into(), name: "預設材質".into(),
            category: MaterialCategory::Custom,
            visual: VisualProps {
                color: [0.8, 0.8, 0.8, 1.0],
                roughness: 0.7, metallic: 0.0, opacity: 1.0,
                texture_hint: None,
            },
            physical: PhysicalProps {
                density: 1000.0,
                friction_static: 0.5, friction_dynamic: 0.4,
                restitution: 0.3,
            },
            structural: None,
            thermal: None,
        }
    }
}

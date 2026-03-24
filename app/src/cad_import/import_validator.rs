// version: v0.1.0
// changelog: Initial skeleton for import validation, bbox checks, and suspicious import diagnostics.
// DEPENDENCY: std
// REQUIRED_METHODS: validate_import, analyze_bbox, classify_scale, has_main_geometry_signal
// SIDE_EFFECTS: None. Pure validation / diagnostics only.

// (dead_code allowed at module level via mod.rs)

use std::collections::HashMap;

/// CHANGELOG: v0.1.0 - Shared 3D point alias.
pub type Vec3 = [f32; 3];

/// CHANGELOG: v0.1.0 - Basic bounding box model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bbox3 {
    pub min: Vec3,
    pub max: Vec3,
}

impl Bbox3 {
    /// CHANGELOG: v0.1.0 - Added constructor.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// CHANGELOG: v0.1.0 - Added empty bbox factory.
    pub fn empty() -> Self {
        Self {
            min: [f32::INFINITY, f32::INFINITY, f32::INFINITY],
            max: [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY],
        }
    }

    /// CHANGELOG: v0.1.0 - Added point expansion.
    pub fn include_point(&mut self, p: Vec3) {
        self.min[0] = self.min[0].min(p[0]);
        self.min[1] = self.min[1].min(p[1]);
        self.min[2] = self.min[2].min(p[2]);

        self.max[0] = self.max[0].max(p[0]);
        self.max[1] = self.max[1].max(p[1]);
        self.max[2] = self.max[2].max(p[2]);
    }

    /// CHANGELOG: v0.1.0 - Added validity check.
    pub fn is_valid(&self) -> bool {
        self.min[0].is_finite()
            && self.min[1].is_finite()
            && self.min[2].is_finite()
            && self.max[0].is_finite()
            && self.max[1].is_finite()
            && self.max[2].is_finite()
            && self.min[0] <= self.max[0]
            && self.min[1] <= self.max[1]
            && self.min[2] <= self.max[2]
    }

    /// CHANGELOG: v0.1.0 - Added size getter.
    pub fn size(&self) -> Vec3 {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }

    /// CHANGELOG: v0.1.0 - Added center getter.
    pub fn center(&self) -> Vec3 {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }
}

/// CHANGELOG: v0.1.0 - Minimal import summary input for validator.
#[derive(Debug, Clone)]
pub struct ImportSnapshot {
    pub source_name: String,
    pub units: String,

    pub curve_count: usize,
    pub text_count: usize,
    pub dimension_count: usize,
    pub block_count: usize,
    pub insert_count: usize,
    pub mesh_count: usize,
    pub object_count: usize,

    pub bbox: Option<Bbox3>,
    pub points: Vec<Vec3>,

    /// Optional metadata from importer, builder, or parser.
    pub metadata: HashMap<String, String>,
}

/// CHANGELOG: v0.1.0 - Validation configuration with engineering-friendly thresholds.
#[derive(Debug, Clone)]
pub struct ImportValidationConfig {
    pub min_reasonable_extent_mm: f32,
    pub max_reasonable_extent_mm: f32,
    pub min_points_for_main_geometry: usize,
    pub max_origin_distance_mm: f32,
    pub allow_single_object_if_meshes_exist: bool,
}

impl Default for ImportValidationConfig {
    fn default() -> Self {
        Self {
            min_reasonable_extent_mm: 1000.0,
            max_reasonable_extent_mm: 100_000.0,
            min_points_for_main_geometry: 4,
            max_origin_distance_mm: 200_000.0,
            allow_single_object_if_meshes_exist: true,
        }
    }
}

/// CHANGELOG: v0.1.0 - Validation severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
}

/// CHANGELOG: v0.1.0 - Validation issue model.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub code: &'static str,
    pub severity: ValidationSeverity,
    pub message: String,
    pub suggestion: Option<String>,
}

/// CHANGELOG: v0.1.0 - Overall import health classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportHealth {
    Healthy,
    Suspicious,
    Broken,
}

/// CHANGELOG: v0.1.0 - Scale classification result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleClass {
    TooSmall,
    Reasonable,
    TooLarge,
    Invalid,
}

/// CHANGELOG: v0.1.0 - Full validation report.
#[derive(Debug, Clone)]
pub struct ImportValidationReport {
    pub health: ImportHealth,
    pub bbox: Option<Bbox3>,
    pub bbox_size: Option<Vec3>,
    pub bbox_center: Option<Vec3>,
    pub scale_class_x: Option<ScaleClass>,
    pub scale_class_y: Option<ScaleClass>,
    pub scale_class_z: Option<ScaleClass>,
    pub issues: Vec<ValidationIssue>,
}

impl ImportValidationReport {
    /// CHANGELOG: v0.1.0 - Added constructor.
    pub fn new(bbox: Option<Bbox3>) -> Self {
        let bbox_size = bbox.map(|b| b.size());
        let bbox_center = bbox.map(|b| b.center());

        Self {
            health: ImportHealth::Healthy,
            bbox,
            bbox_size,
            bbox_center,
            scale_class_x: None,
            scale_class_y: None,
            scale_class_z: None,
            issues: Vec::new(),
        }
    }

    /// CHANGELOG: v0.1.0 - Added issue insertion helper with automatic health escalation.
    pub fn push_issue(&mut self, issue: ValidationIssue) {
        match issue.severity {
            ValidationSeverity::Info => {}
            ValidationSeverity::Warning => {
                if self.health == ImportHealth::Healthy {
                    self.health = ImportHealth::Suspicious;
                }
            }
            ValidationSeverity::Error => {
                self.health = ImportHealth::Broken;
            }
        }
        self.issues.push(issue);
    }
}

/// CHANGELOG: v0.1.0 - Public validator entry point.
pub fn validate_import(
    snapshot: &ImportSnapshot,
    config: &ImportValidationConfig,
) -> ImportValidationReport {
    let bbox = snapshot.bbox.or_else(|| analyze_bbox(&snapshot.points));
    let mut report = ImportValidationReport::new(bbox);

    if !has_main_geometry_signal(snapshot, config) {
        report.push_issue(ValidationIssue {
            code: "LOW_GEOMETRY_SIGNAL",
            severity: ValidationSeverity::Error,
            message: "Import contains too little geometry to build a reliable model.".to_string(),
            suggestion: Some(
                "Check whether DXF/DWG parsing failed, or whether the file was filtered too aggressively.".to_string(),
            ),
        });
    }

    if let Some(bbox) = bbox {
        let size = bbox.size();

        let sx = classify_scale(size[0], config);
        let sy = classify_scale(size[1], config);
        let sz = classify_scale(size[2], config);

        report.scale_class_x = Some(sx);
        report.scale_class_y = Some(sy);
        report.scale_class_z = Some(sz);

        push_scale_issue_if_needed("X", size[0], sx, &mut report, config);
        push_scale_issue_if_needed("Y", size[1], sy, &mut report, config);
        push_scale_issue_if_needed("Z", size[2], sz, &mut report, config);

        let center = bbox.center();
        let origin_distance = distance_xy(center);

        if origin_distance > config.max_origin_distance_mm {
            report.push_issue(ValidationIssue {
                code: "FAR_FROM_ORIGIN",
                severity: ValidationSeverity::Warning,
                message: format!(
                    "Main geometry center is far from world origin ({origin_distance:.1} mm)."
                ),
                suggestion: Some(
                    "Normalize the main geometry cluster to a local working origin before building the scene."
                        .to_string(),
                ),
            });
        }
    } else {
        report.push_issue(ValidationIssue {
            code: "NO_BBOX",
            severity: ValidationSeverity::Error,
            message: "Unable to compute a bounding box from imported data.".to_string(),
            suggestion: Some(
                "Ensure the importer produced points, curves, or meshes before scene construction.".to_string(),
            ),
        });
    }

    if snapshot.object_count == 1
        && snapshot.mesh_count <= 1
        && snapshot.curve_count > 20
        && !config.allow_single_object_if_meshes_exist
    {
        report.push_issue(ValidationIssue {
            code: "SINGLE_BOX_BUILD",
            severity: ValidationSeverity::Warning,
            message: "Import appears to have collapsed rich drawing data into a single generic object."
                .to_string(),
            suggestion: Some(
                "Split importer parsing from builder logic. Build from IR instead of forcing a fallback box."
                    .to_string(),
            ),
        });
    }

    if snapshot.dimension_count == 0 && snapshot.text_count == 0 && snapshot.curve_count > 0 {
        report.push_issue(ValidationIssue {
            code: "NO_ANNOTATION_SIGNAL",
            severity: ValidationSeverity::Info,
            message: "No text or dimensions were detected; semantic reconstruction may be limited."
                .to_string(),
            suggestion: Some(
                "Proceed with geometry import, but expect weaker grid / elevation / steel semantic detection."
                    .to_string(),
            ),
        });
    }

    report
}

/// CHANGELOG: v0.1.0 - Compute bbox from a list of points.
pub fn analyze_bbox(points: &[Vec3]) -> Option<Bbox3> {
    if points.is_empty() {
        return None;
    }

    let mut bbox = Bbox3::empty();
    for &point in points {
        bbox.include_point(point);
    }

    bbox.is_valid().then_some(bbox)
}

/// CHANGELOG: v0.1.0 - Determine if import contains enough geometric signal to proceed.
pub fn has_main_geometry_signal(
    snapshot: &ImportSnapshot,
    config: &ImportValidationConfig,
) -> bool {
    if snapshot.points.len() >= config.min_points_for_main_geometry {
        return true;
    }

    if snapshot.curve_count > 0 || snapshot.mesh_count > 0 || snapshot.insert_count > 0 {
        return true;
    }

    false
}

/// CHANGELOG: v0.1.0 - Classify a single axis extent.
pub fn classify_scale(extent_mm: f32, config: &ImportValidationConfig) -> ScaleClass {
    if !extent_mm.is_finite() || extent_mm < 0.0 {
        return ScaleClass::Invalid;
    }

    if extent_mm < config.min_reasonable_extent_mm {
        ScaleClass::TooSmall
    } else if extent_mm > config.max_reasonable_extent_mm {
        ScaleClass::TooLarge
    } else {
        ScaleClass::Reasonable
    }
}

/// CHANGELOG: v0.1.0 - Push scale-related issue into report.
fn push_scale_issue_if_needed(
    axis_name: &str,
    extent_mm: f32,
    class: ScaleClass,
    report: &mut ImportValidationReport,
    config: &ImportValidationConfig,
) {
    match class {
        ScaleClass::Reasonable => {}
        ScaleClass::TooSmall => report.push_issue(ValidationIssue {
            code: "BBOX_TOO_SMALL",
            severity: ValidationSeverity::Warning,
            message: format!(
                "Bounding box extent on {axis_name} axis is suspiciously small: {extent_mm:.3} mm."
            ),
            suggestion: Some(format!(
                "Check whether valid geometry was filtered out. Current minimum reasonable extent is {} mm.",
                config.min_reasonable_extent_mm
            )),
        }),
        ScaleClass::TooLarge => report.push_issue(ValidationIssue {
            code: "BBOX_TOO_LARGE",
            severity: ValidationSeverity::Warning,
            message: format!(
                "Bounding box extent on {axis_name} axis is suspiciously large: {extent_mm:.3} mm."
            ),
            suggestion: Some(format!(
                "Possible causes: outlier coordinates, wrong units, or DWG binary misread. Current maximum reasonable extent is {} mm.",
                config.max_reasonable_extent_mm
            )),
        }),
        ScaleClass::Invalid => report.push_issue(ValidationIssue {
            code: "BBOX_INVALID",
            severity: ValidationSeverity::Error,
            message: format!("Bounding box extent on {axis_name} axis is invalid."),
            suggestion: Some(
                "Inspect parsed point data and ensure numeric group-code values were read correctly."
                    .to_string(),
            ),
        }),
    }
}

/// CHANGELOG: v0.1.0 - Approximate horizontal origin distance for validator warnings.
fn distance_xy(center: Vec3) -> f32 {
    (center[0] * center[0] + center[2] * center[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbox_from_points_is_computed() {
        let bbox = analyze_bbox(&[[0.0, 0.0, 0.0], [1000.0, 2000.0, 3000.0]]).unwrap();
        assert_eq!(bbox.min, [0.0, 0.0, 0.0]);
        assert_eq!(bbox.max, [1000.0, 2000.0, 3000.0]);
    }

    #[test]
    fn suspicious_large_bbox_is_flagged() {
        let snapshot = ImportSnapshot {
            source_name: "test".to_string(),
            units: "mm".to_string(),
            curve_count: 10,
            text_count: 1,
            dimension_count: 1,
            block_count: 0,
            insert_count: 0,
            mesh_count: 0,
            object_count: 0,
            bbox: Some(Bbox3::new([0.0, 0.0, 0.0], [500_000.0, 5_000.0, 10_000.0])),
            points: vec![],
            metadata: HashMap::new(),
        };

        let report = validate_import(&snapshot, &ImportValidationConfig::default());
        assert_ne!(report.health, ImportHealth::Healthy);
        assert!(!report.issues.is_empty());
    }
}

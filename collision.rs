// version: v0.1.0
// changelog: Initial skeleton for Kolibri Ai3D collision detection module.
// DEPENDENCY: std
// REQUIRED_METHODS: aabb, intersects_with_epsilon, classify_collision, can_place_component, can_move_component
// SIDE_EFFECTS: None. Pure collision queries only.

//! Kolibri Ai3D collision module skeleton
//!
//! Design goals:
//! - Fast broad-phase checks with AABB
//! - Support CAD-style "allowed touch" vs "illegal penetration"
//! - Keep business rules separate from geometry math
//! - Be easy to extend later to OBB / mesh / B-Rep checks

use std::collections::HashSet;

/// Global tolerance for CAD / modeling queries.
///
/// In mm-based scenes, tiny floating point overlap should usually not count as penetration.
pub const DEFAULT_EPSILON_MM: f32 = 0.1;

/// 3D vector / point type used by this module.
pub type Vec3 = [f32; 3];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// CHANGELOG: v0.1.0 - Added constructor with auto-normalization.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self {
            min: [min[0].min(max[0]), min[1].min(max[1]), min[2].min(max[2])],
            max: [min[0].max(max[0]), min[1].max(max[1]), min[2].max(max[2])],
        }
    }

    /// CHANGELOG: v0.1.0 - Added helper constructor from center + size.
    pub fn from_center_size(center: Vec3, size: Vec3) -> Self {
        let half = [size[0] * 0.5, size[1] * 0.5, size[2] * 0.5];
        Self {
            min: [center[0] - half[0], center[1] - half[1], center[2] - half[2]],
            max: [center[0] + half[0], center[1] + half[1], center[2] + half[2]],
        }
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

    /// CHANGELOG: v0.1.0 - Added broad-phase overlap test with tolerance.
    pub fn intersects_with_epsilon(&self, other: &Aabb, epsilon: f32) -> bool {
        self.min[0] <= other.max[0] - epsilon
            && self.max[0] >= other.min[0] + epsilon
            && self.min[1] <= other.max[1] - epsilon
            && self.max[1] >= other.min[1] + epsilon
            && self.min[2] <= other.max[2] - epsilon
            && self.max[2] >= other.min[2] + epsilon
    }

    /// CHANGELOG: v0.1.0 - Added raw overlap depth computation.
    ///
    /// Positive values mean overlapping on that axis.
    /// Zero or negative values mean separated or just touching.
    pub fn overlap_depth(&self, other: &Aabb) -> Vec3 {
        [
            self.max[0].min(other.max[0]) - self.min[0].max(other.min[0]),
            self.max[1].min(other.max[1]) - self.min[1].max(other.min[1]),
            self.max[2].min(other.max[2]) - self.min[2].max(other.min[2]),
        ]
    }

    /// CHANGELOG: v0.1.0 - Added translation helper.
    pub fn translated(&self, delta: Vec3) -> Self {
        Self {
            min: [self.min[0] + delta[0], self.min[1] + delta[1], self.min[2] + delta[2]],
            max: [self.max[0] + delta[0], self.max[1] + delta[1], self.max[2] + delta[2]],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentKind {
    Generic,
    Beam,
    Column,
    Plate,
    Bolt,
    Weld,
    Foundation,
    Equipment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionKind {
    /// No overlap and no contact of interest.
    None,
    /// Touching within tolerance. Often legal in CAD / steel workflows.
    Touching,
    /// Geometric penetration / overlap beyond tolerance.
    Penetrating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionDecision {
    Allow,
    Block,
    Warn,
}

#[derive(Debug, Clone)]
pub struct Component {
    pub id: String,
    pub kind: ComponentKind,
    pub name: Option<String>,

    /// Logical transform state. For now, collision uses AABB only.
    pub position: Vec3,
    pub size: Vec3,

    /// Optional tags for business rules, e.g. "temporary", "structural", "ignore_collision".
    pub tags: HashSet<String>,
}

impl Component {
    /// CHANGELOG: v0.1.0 - Added constructor for quick scene queries.
    pub fn new(id: impl Into<String>, kind: ComponentKind, position: Vec3, size: Vec3) -> Self {
        Self {
            id: id.into(),
            kind,
            name: None,
            position,
            size,
            tags: HashSet::new(),
        }
    }

    /// CHANGELOG: v0.1.0 - Added default AABB generator.
    pub fn aabb(&self) -> Aabb {
        Aabb::from_center_size(self.position, self.size)
    }

    /// CHANGELOG: v0.1.0 - Added predicted AABB for move previews.
    pub fn predicted_aabb(&self, new_position: Vec3) -> Aabb {
        Aabb::from_center_size(new_position, self.size)
    }

    /// CHANGELOG: v0.1.0 - Added opt-out helper for rules.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }
}

#[derive(Debug, Clone)]
pub struct CollisionPair {
    pub moving_id: String,
    pub other_id: String,
    pub moving_kind: ComponentKind,
    pub other_kind: ComponentKind,
    pub collision_kind: CollisionKind,
    pub decision: CollisionDecision,
    pub overlap_depth: Vec3,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CollisionReport {
    pub is_allowed: bool,
    pub blocking_pairs: Vec<CollisionPair>,
    pub warning_pairs: Vec<CollisionPair>,
    pub allowed_pairs: Vec<CollisionPair>,
}

impl CollisionReport {
    /// CHANGELOG: v0.1.0 - Added convenience constructor.
    pub fn empty() -> Self {
        Self {
            is_allowed: true,
            blocking_pairs: Vec::new(),
            warning_pairs: Vec::new(),
            allowed_pairs: Vec::new(),
        }
    }

    /// CHANGELOG: v0.1.0 - Added insert helper.
    pub fn push(&mut self, pair: CollisionPair) {
        match pair.decision {
            CollisionDecision::Allow => self.allowed_pairs.push(pair),
            CollisionDecision::Warn => self.warning_pairs.push(pair),
            CollisionDecision::Block => {
                self.is_allowed = false;
                self.blocking_pairs.push(pair);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CollisionConfig {
    pub epsilon_mm: f32,

    /// Whether touching is allowed by default if no explicit rule exists.
    pub allow_touching_by_default: bool,

    /// Whether same-kind penetration should warn instead of block in early prototype mode.
    pub soft_fail_same_kind: bool,
}

impl Default for CollisionConfig {
    fn default() -> Self {
        Self {
            epsilon_mm: DEFAULT_EPSILON_MM,
            allow_touching_by_default: true,
            soft_fail_same_kind: false,
        }
    }
}

/// CHANGELOG: v0.1.0 - Added public entry point for classifying AABB relation.
pub fn classify_collision(a: &Aabb, b: &Aabb, epsilon: f32) -> CollisionKind {
    let overlap = a.overlap_depth(b);

    let positive_axes = overlap.iter().filter(|&&v| v > epsilon).count();
    let touching_axes = overlap
        .iter()
        .filter(|&&v| v >= -epsilon && v <= epsilon)
        .count();

    if positive_axes == 3 {
        CollisionKind::Penetrating
    } else if positive_axes >= 2 && touching_axes >= 1 {
        CollisionKind::Touching
    } else {
        CollisionKind::None
    }
}

/// CHANGELOG: v0.1.0 - Added business-rule gateway for legal / illegal contacts.
pub fn decide_collision(
    moving: &Component,
    other: &Component,
    collision_kind: CollisionKind,
    config: &CollisionConfig,
) -> CollisionDecision {
    if collision_kind == CollisionKind::None {
        return CollisionDecision::Allow;
    }

    if moving.has_tag("ignore_collision") || other.has_tag("ignore_collision") {
        return CollisionDecision::Allow;
    }

    match collision_kind {
        CollisionKind::Touching => {
            if is_touching_allowed(moving.kind, other.kind, config.allow_touching_by_default) {
                CollisionDecision::Allow
            } else {
                CollisionDecision::Warn
            }
        }
        CollisionKind::Penetrating => {
            if is_penetration_allowed(moving.kind, other.kind) {
                CollisionDecision::Warn
            } else if config.soft_fail_same_kind && moving.kind == other.kind {
                CollisionDecision::Warn
            } else {
                CollisionDecision::Block
            }
        }
        CollisionKind::None => CollisionDecision::Allow,
    }
}

/// CHANGELOG: v0.1.0 - Added default touching rule table.
pub fn is_touching_allowed(
    a: ComponentKind,
    b: ComponentKind,
    allow_touching_by_default: bool,
) -> bool {
    use ComponentKind::*;

    match (a, b) {
        (Beam, Column) | (Column, Beam) => true,
        (Plate, Beam) | (Beam, Plate) => true,
        (Plate, Column) | (Column, Plate) => true,
        (Foundation, Column) | (Column, Foundation) => true,
        (Bolt, Plate) | (Plate, Bolt) => true,
        (Bolt, Beam) | (Beam, Bolt) => true,
        (Weld, Plate) | (Plate, Weld) => true,
        (Weld, Beam) | (Beam, Weld) => true,
        _ => allow_touching_by_default,
    }
}

/// CHANGELOG: v0.1.0 - Added default penetration rule table.
pub fn is_penetration_allowed(a: ComponentKind, b: ComponentKind) -> bool {
    use ComponentKind::*;

    matches!(
        (a, b),
        // Bolts and welds often "penetrate" host geometry by design in simplified previews.
        (Bolt, Plate)
            | (Plate, Bolt)
            | (Bolt, Beam)
            | (Beam, Bolt)
            | (Weld, Plate)
            | (Plate, Weld)
            | (Weld, Beam)
            | (Beam, Weld)
    )
}

/// CHANGELOG: v0.1.0 - Added placement query for newly created components.
pub fn can_place_component(
    new_component: &Component,
    others: &[Component],
    config: &CollisionConfig,
) -> CollisionReport {
    let candidate_aabb = new_component.aabb();
    collect_collision_report(new_component, &candidate_aabb, others, config)
}

/// CHANGELOG: v0.1.0 - Added move query for interactive dragging / snapping.
pub fn can_move_component(
    moving: &Component,
    new_position: Vec3,
    others: &[Component],
    config: &CollisionConfig,
) -> CollisionReport {
    let candidate_aabb = moving.predicted_aabb(new_position);
    collect_collision_report(moving, &candidate_aabb, others, config)
}

/// CHANGELOG: v0.1.0 - Added shared implementation for place / move validation.
fn collect_collision_report(
    moving: &Component,
    candidate_aabb: &Aabb,
    others: &[Component],
    config: &CollisionConfig,
) -> CollisionReport {
    let mut report = CollisionReport::empty();

    for other in others {
        if other.id == moving.id {
            continue;
        }

        // Broad phase: skip clearly separate AABBs.
        if !candidate_aabb.intersects_with_epsilon(&other.aabb(), -config.epsilon_mm) {
            continue;
        }

        let collision_kind = classify_collision(candidate_aabb, &other.aabb(), config.epsilon_mm);
        if collision_kind == CollisionKind::None {
            continue;
        }

        let decision = decide_collision(moving, other, collision_kind, config);

        let pair = CollisionPair {
            moving_id: moving.id.clone(),
            other_id: other.id.clone(),
            moving_kind: moving.kind,
            other_kind: other.kind,
            collision_kind,
            decision,
            overlap_depth: candidate_aabb.overlap_depth(&other.aabb()),
            note: default_collision_note(moving.kind, other.kind, collision_kind, decision),
        };

        report.push(pair);
    }

    report
}

/// CHANGELOG: v0.1.0 - Added default user-facing note generator.
fn default_collision_note(
    moving_kind: ComponentKind,
    other_kind: ComponentKind,
    collision_kind: CollisionKind,
    decision: CollisionDecision,
) -> Option<String> {
    let note = match (moving_kind, other_kind, collision_kind, decision) {
        (_, _, CollisionKind::Touching, CollisionDecision::Allow) => "Legal touching contact".to_string(),
        (_, _, CollisionKind::Touching, CollisionDecision::Warn) => "Touching detected; confirm if intentional".to_string(),
        (_, _, CollisionKind::Penetrating, CollisionDecision::Warn) => "Penetration allowed by rule, but should be reviewed".to_string(),
        (_, _, CollisionKind::Penetrating, CollisionDecision::Block) => "Illegal geometric penetration".to_string(),
        _ => return None,
    };

    Some(note)
}

/// CHANGELOG: v0.1.0 - Added helper to move a component only if collision rules allow it.
pub fn move_component_if_safe(
    moving: &mut Component,
    new_position: Vec3,
    others: &[Component],
    config: &CollisionConfig,
) -> CollisionReport {
    let report = can_move_component(moving, new_position, others, config);
    if report.is_allowed {
        moving.position = new_position;
    }
    report
}

/// CHANGELOG: v0.1.0 - Added simple "find nearest safe position" helper.
///
/// This is intentionally basic. In production, you may replace it with:
/// - axis-constrained search
/// - snap-aware resolution
/// - sweep tests
/// - BVH / spatial hash acceleration
pub fn find_nearest_safe_position_along_axis(
    moving: &Component,
    desired_position: Vec3,
    axis: usize,
    search_step_mm: f32,
    max_steps: usize,
    others: &[Component],
    config: &CollisionConfig,
) -> Option<Vec3> {
    debug_assert!(axis < 3, "axis must be 0, 1, or 2");

    // First try the requested position.
    let direct = can_move_component(moving, desired_position, others, config);
    if direct.is_allowed {
        return Some(desired_position);
    }

    for step in 1..=max_steps {
        let delta = search_step_mm * step as f32;

        let mut positive = desired_position;
        positive[axis] += delta;
        if can_move_component(moving, positive, others, config).is_allowed {
            return Some(positive);
        }

        let mut negative = desired_position;
        negative[axis] -= delta;
        if can_move_component(moving, negative, others, config).is_allowed {
            return Some(negative);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_penetration_is_detected() {
        let a = Aabb::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        let b = Aabb::new([5.0, 5.0, 5.0], [15.0, 15.0, 15.0]);
        assert_eq!(classify_collision(&a, &b, 0.1), CollisionKind::Penetrating);
    }

    #[test]
    fn aabb_touching_is_detected() {
        let a = Aabb::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        let b = Aabb::new([10.0, 2.0, 2.0], [20.0, 8.0, 8.0]);
        assert_eq!(classify_collision(&a, &b, 0.1), CollisionKind::Touching);
    }

    #[test]
    fn beam_column_touching_is_allowed() {
        let beam = Component::new("beam-1", ComponentKind::Beam, [5.0, 0.0, 0.0], [10.0, 2.0, 2.0]);
        let col = Component::new("col-1", ComponentKind::Column, [11.0, 0.0, 0.0], [2.0, 10.0, 2.0]);

        let report = can_place_component(&beam, &[col], &CollisionConfig::default());
        assert!(report.is_allowed);
    }

    #[test]
    fn generic_penetration_is_blocked() {
        let a = Component::new("a", ComponentKind::Generic, [0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        let b = Component::new("b", ComponentKind::Generic, [5.0, 5.0, 5.0], [10.0, 10.0, 10.0]);

        let report = can_place_component(&a, &[b], &CollisionConfig::default());
        assert!(!report.is_allowed);
        assert_eq!(report.blocking_pairs.len(), 1);
    }
}

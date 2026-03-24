//! Inference 2.0 — Context-aware snap scoring system
//! Replaces simple "nearest point" with "highest score" snap selection.
//!
//! Three-layer architecture:
//!   Layer 1: Geometry — endpoint, midpoint, axis (existing snap.rs)
//!   Layer 2: Context  — working plane, last direction, current tool
//!   Layer 3: Intent   — "user is drawing a wall" -> boost relevant snaps

use crate::app::{SnapType, Tool};
use crate::scene::{Scene, Shape};

/// Interaction context for inference scoring
#[derive(Debug, Clone)]
pub struct InferenceContext {
    pub current_tool: Tool,
    pub last_direction: Option<[f32; 2]>, // XZ direction of last drawn edge
    pub working_plane: WorkingPlane,
    pub consecutive_lines: u32,           // how many lines drawn in sequence
    pub last_created_type: Option<String>, // "wall", "column", etc.
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WorkingPlane {
    Ground,       // Y=0 (default)
    FaceXY(f32),  // on a face at Z=value
    FaceXZ(f32),  // on a face at Y=value
    FaceYZ(f32),  // on a face at X=value
}

impl Default for InferenceContext {
    fn default() -> Self {
        Self {
            current_tool: Tool::Select,
            last_direction: None,
            working_plane: WorkingPlane::Ground,
            consecutive_lines: 0,
            last_created_type: None,
        }
    }
}

/// A snap candidate with a score
#[derive(Debug, Clone)]
pub struct InferenceCandidate {
    pub position: [f32; 3],
    pub snap_type: SnapType,
    pub score: f32,
    pub label: String,
    pub source: InferenceSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InferenceSource {
    Geometry, // Layer 1: basic geometric snap
    Context,  // Layer 2: context-aware
    Intent,   // Layer 3: AI intent
}

/// Score and rank all snap candidates
pub fn rank_candidates(
    candidates: &mut Vec<InferenceCandidate>,
    ctx: &InferenceContext,
    _cursor_pos: [f32; 3],
    from_point: Option<[f32; 3]>,
    scene: &Scene,
) {
    // Detect intent once for all candidates
    let intent = detect_intent(ctx, scene);

    for candidate in candidates.iter_mut() {
        let mut score = candidate.score;

        // ── Layer 2: Context scoring ──

        // Boost snaps on the current working plane
        match ctx.working_plane {
            WorkingPlane::Ground => {
                if candidate.position[1].abs() < 10.0 {
                    score += 30.0; // on ground plane
                }
            }
            WorkingPlane::FaceXY(z) => {
                if (candidate.position[2] - z).abs() < 10.0 {
                    score += 50.0;
                }
            }
            WorkingPlane::FaceXZ(y) => {
                if (candidate.position[1] - y).abs() < 10.0 {
                    score += 50.0;
                }
            }
            WorkingPlane::FaceYZ(x) => {
                if (candidate.position[0] - x).abs() < 10.0 {
                    score += 50.0;
                }
            }
        }

        // Boost if continuing same direction (extend behavior)
        if let (Some(last_dir), Some(from)) = (&ctx.last_direction, from_point) {
            let dx = candidate.position[0] - from[0];
            let dz = candidate.position[2] - from[2];
            let len = (dx * dx + dz * dz).sqrt();
            if len > 50.0 {
                let dir = [dx / len, dz / len];
                let dot = dir[0] * last_dir[0] + dir[1] * last_dir[1];

                // Parallel to last direction (extend)
                if dot.abs() > 0.95 {
                    score += 40.0;
                    if candidate.source == InferenceSource::Geometry {
                        candidate.label = "\u{5ef6}\u{7e8c}\u{65b9}\u{5411}".to_string(); // 延續方向
                        candidate.source = InferenceSource::Context;
                    }
                }
                // Perpendicular to last direction
                if dot.abs() < 0.05 {
                    score += 25.0;
                    if candidate.source == InferenceSource::Geometry {
                        candidate.label = "\u{5782}\u{76f4}\u{65b9}\u{5411}".to_string(); // 垂直方向
                        candidate.source = InferenceSource::Context;
                    }
                }
            }
        }

        // Boost consecutive line drawing (user is drawing a shape)
        if ctx.consecutive_lines > 1 {
            if matches!(
                candidate.snap_type,
                SnapType::AxisX
                    | SnapType::AxisZ
                    | SnapType::Parallel
                    | SnapType::Perpendicular
            ) {
                score += 20.0 * ctx.consecutive_lines as f32;
            }
        }

        // ── Layer 3: Intent scoring ──

        match intent {
            UserIntent::DrawWall => {
                // Boost horizontal axis snaps
                if matches!(
                    candidate.snap_type,
                    SnapType::AxisX | SnapType::AxisZ
                ) {
                    score += 60.0;
                }
                // Boost snaps aligned with existing walls
                for obj in scene.objects.values() {
                    if let Shape::Box { depth, .. } = &obj.shape {
                        if *depth < 300.0 {
                            // thin box = wall
                            let wall_z = obj.position[2];
                            if (candidate.position[2] - wall_z).abs() < 50.0 {
                                score += 45.0;
                                candidate.label =
                                    "\u{7246}\u{5c0d}\u{9f4a}".to_string(); // 牆對齊
                                candidate.source = InferenceSource::Intent;
                            }
                        }
                    }
                }
            }
            UserIntent::DrawColumn => {
                // Boost endpoint snaps near existing columns
                for obj in scene.objects.values() {
                    if matches!(&obj.shape, Shape::Cylinder { .. }) {
                        let d = ((candidate.position[0] - obj.position[0]).powi(2)
                            + (candidate.position[2] - obj.position[2]).powi(2))
                        .sqrt();
                        if d < 100.0 {
                            score += 50.0;
                            candidate.label =
                                "\u{67f1}\u{5c0d}\u{9f4a}".to_string(); // 柱對齊
                            candidate.source = InferenceSource::Intent;
                        }
                    }
                }
            }
            UserIntent::ExtendLine => {
                // Already handled by direction continuation above
                score += 15.0;
            }
            UserIntent::Orthogonal => {
                // Boost 90-degree snaps
                if matches!(candidate.snap_type, SnapType::Perpendicular) {
                    score += 50.0;
                }
            }
            UserIntent::General => {}
        }

        candidate.score = score;
    }

    // Sort by score (highest first)
    candidates
        .sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
}

#[derive(Debug, Clone, PartialEq)]
enum UserIntent {
    DrawWall,
    DrawColumn,
    ExtendLine,
    Orthogonal,
    General,
}

fn detect_intent(ctx: &InferenceContext, scene: &Scene) -> UserIntent {
    // Rule-based intent detection (no AI model needed)

    match ctx.current_tool {
        Tool::Line | Tool::Rectangle => {
            // If drawing lines and last direction was mostly horizontal
            if let Some(dir) = &ctx.last_direction {
                let is_horizontal = dir[1].abs() < 0.2; // mostly in X or Z
                if is_horizontal && ctx.consecutive_lines >= 2 {
                    return UserIntent::DrawWall;
                }
            }
            // If angle to last edge is ~90 degrees
            if ctx.consecutive_lines >= 1 {
                return UserIntent::Orthogonal;
            }
            if ctx.consecutive_lines > 0 {
                return UserIntent::ExtendLine;
            }
        }
        Tool::CreateCylinder => {
            return UserIntent::DrawColumn;
        }
        Tool::CreateBox => {
            // Check if nearby objects are walls -> user is probably adding another wall
            let has_nearby_walls = scene.objects.values().any(|obj| {
                if let Shape::Box { depth, .. } = &obj.shape {
                    *depth < 300.0
                } else {
                    false
                }
            });
            if has_nearby_walls {
                return UserIntent::DrawWall;
            }
        }
        _ => {}
    }

    UserIntent::General
}

/// Update context after a line is drawn
pub fn update_context_after_line(ctx: &mut InferenceContext, p1: [f32; 3], p2: [f32; 3]) {
    let dx = p2[0] - p1[0];
    let dz = p2[2] - p1[2];
    let len = (dx * dx + dz * dz).sqrt();
    if len > 10.0 {
        ctx.last_direction = Some([dx / len, dz / len]);
    }
    ctx.consecutive_lines += 1;
}

/// Update working plane when user draws on a face
pub fn update_working_plane(
    ctx: &mut InferenceContext,
    snap_type: SnapType,
    pos: [f32; 3],
) {
    if snap_type == SnapType::OnFace {
        // Determine which plane the face is on
        // Simple heuristic: if Y != 0, we're on a vertical face
        if pos[1].abs() > 10.0 {
            ctx.working_plane = WorkingPlane::FaceXZ(pos[1]);
        }
    }
}

/// Reset context (e.g., when tool changes or ESC pressed)
pub fn reset_context(ctx: &mut InferenceContext) {
    ctx.consecutive_lines = 0;
    ctx.last_direction = None;
    ctx.working_plane = WorkingPlane::Ground;
}

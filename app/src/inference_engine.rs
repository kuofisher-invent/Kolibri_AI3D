//! Inference Engine 2.0 — formal scoring pipeline
//!
//! Architecture:
//!   Candidates → ScoreRules → ScoreBreakdown → Resolver → UI
//!
//! Four score layers:
//!   1. Geometry: distance, alignment, feature type
//!   2. Context: tool preference, working plane, mode
//!   3. Semantic: engineering meaning (beam axis, column center, grid)
//!   4. Intent: user behavior pattern (continuation, repetition)

// ═══════════════════════════════════════════════════════════════
//  Core Types
// ═══════════════════════════════════════════════════════════════

/// A candidate point/feature that the inference engine evaluates
#[derive(Debug, Clone)]
pub struct InferenceCandidate {
    pub id: String,
    pub inference_type: InferenceType,
    pub position: [f32; 3],
    pub source_object_id: Option<String>,
    pub raw_distance: f32, // screen-space distance to cursor (pixels)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceType {
    Endpoint,
    Midpoint,
    Intersection,
    OnEdge,
    OnFace,
    AxisLockX,
    AxisLockY,
    AxisLockZ,
    Parallel,
    Perpendicular,
    GridLine,
    BeamAxis,
    ColumnCenter,
    Origin,
    Grid,
    Custom,
}

/// 4-layer score breakdown with reasons
#[derive(Debug, Clone, Default)]
pub struct ScoreBreakdown {
    pub geometry: f32,
    pub context: f32,
    pub semantic: f32,
    pub intent: f32,
    pub total: f32,
    pub reasons: Vec<String>,
}

impl ScoreBreakdown {
    pub fn compute_total(&mut self) {
        self.total = self.geometry + self.context + self.semantic + self.intent;
    }
}

/// Scored candidate = candidate + breakdown
#[derive(Debug, Clone)]
pub struct ScoredCandidate {
    pub candidate: InferenceCandidate,
    pub breakdown: ScoreBreakdown,
}

/// Interaction context for scoring decisions
#[derive(Debug, Clone)]
pub struct InferenceContext {
    pub current_tool: ToolKind,
    pub current_mode: AppMode,
    pub selected_ids: Vec<String>,
    pub hover_id: Option<String>,
    pub last_direction: Option<[f32; 2]>, // XZ direction of last drawn line
    pub last_action: String,
    pub working_plane_y: f32,
    pub locked_axis: Option<u8>, // 0=X, 1=Y, 2=Z
    pub is_drawing: bool,
    pub consecutive_same_tool: u32, // how many actions with same tool
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Select,
    Move,
    Rotate,
    Scale,
    Line,
    Arc,
    Rectangle,
    Circle,
    Box,
    Cylinder,
    Sphere,
    PushPull,
    Offset,
    FollowMe,
    TapeMeasure,
    PaintBucket,
    Orbit,
    Pan,
    SteelColumn,
    SteelBeam,
    SteelGrid,
    Eraser,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Modeling,
    Steel,
    Layout,
}

// ═══════════════════════════════════════════════════════════════
//  ScoreRule Trait (pluggable)
// ═══════════════════════════════════════════════════════════════

/// Pluggable scoring rule — each rule contributes to one score layer
pub trait ScoreRule: Send + Sync {
    fn name(&self) -> &'static str;
    fn layer(&self) -> ScoreLayer;
    fn score(&self, candidate: &InferenceCandidate, ctx: &InferenceContext) -> f32;
    fn reason(&self, candidate: &InferenceCandidate, ctx: &InferenceContext) -> Option<String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreLayer {
    Geometry,
    Context,
    Semantic,
    Intent,
}

// ═══════════════════════════════════════════════════════════════
//  Built-in Score Rules
// ═══════════════════════════════════════════════════════════════

/// Rule 1: Distance-based geometry score (closer = higher)
pub struct GeometryDistanceRule;
impl ScoreRule for GeometryDistanceRule {
    fn name(&self) -> &'static str {
        "距離"
    }
    fn layer(&self) -> ScoreLayer {
        ScoreLayer::Geometry
    }
    fn score(&self, c: &InferenceCandidate, _ctx: &InferenceContext) -> f32 {
        // 0px = 40 points, 15px = 20 points, 30px = 0 points
        (40.0 - c.raw_distance * 1.33).max(0.0)
    }
    fn reason(&self, c: &InferenceCandidate, _ctx: &InferenceContext) -> Option<String> {
        if c.raw_distance < 10.0 {
            Some(format!("距離 {:.0}px (很近)", c.raw_distance))
        } else {
            None
        }
    }
}

/// Rule 2: Feature type bonus (endpoints/midpoints worth more than edge-on)
pub struct FeatureTypeRule;
impl ScoreRule for FeatureTypeRule {
    fn name(&self) -> &'static str {
        "特徵類型"
    }
    fn layer(&self) -> ScoreLayer {
        ScoreLayer::Geometry
    }
    fn score(&self, c: &InferenceCandidate, _ctx: &InferenceContext) -> f32 {
        match c.inference_type {
            InferenceType::Endpoint => 25.0,
            InferenceType::Midpoint => 20.0,
            InferenceType::Origin => 20.0,
            InferenceType::Intersection => 22.0,
            InferenceType::OnFace => 10.0,
            InferenceType::OnEdge => 12.0,
            InferenceType::AxisLockX
            | InferenceType::AxisLockY
            | InferenceType::AxisLockZ => 18.0,
            InferenceType::Parallel | InferenceType::Perpendicular => 15.0,
            InferenceType::GridLine => 20.0,
            _ => 5.0,
        }
    }
    fn reason(&self, c: &InferenceCandidate, _ctx: &InferenceContext) -> Option<String> {
        let label = match c.inference_type {
            InferenceType::Endpoint => "端點",
            InferenceType::Midpoint => "中點",
            InferenceType::Origin => "原點",
            InferenceType::Intersection => "交點",
            InferenceType::OnFace => "面上",
            InferenceType::OnEdge => "邊上",
            InferenceType::GridLine => "軸線",
            InferenceType::Parallel => "平行",
            InferenceType::Perpendicular => "垂直",
            _ => return None,
        };
        Some(label.into())
    }
}

/// Rule 3: Tool preference (different tools prefer different snap types)
pub struct ToolPreferenceRule;
impl ScoreRule for ToolPreferenceRule {
    fn name(&self) -> &'static str {
        "工具偏好"
    }
    fn layer(&self) -> ScoreLayer {
        ScoreLayer::Context
    }
    fn score(&self, c: &InferenceCandidate, ctx: &InferenceContext) -> f32 {
        match (ctx.current_tool, c.inference_type) {
            // Line tool: prefer endpoints and axis
            (ToolKind::Line, InferenceType::Endpoint) => 30.0,
            (ToolKind::Line, InferenceType::Midpoint) => 20.0,
            (ToolKind::Line, InferenceType::AxisLockX | InferenceType::AxisLockZ) => 25.0,
            (ToolKind::Line, InferenceType::Perpendicular) => 20.0,

            // Move tool: prefer endpoints and grid
            (ToolKind::Move, InferenceType::Endpoint) => 25.0,
            (ToolKind::Move, InferenceType::GridLine) => 20.0,

            // Steel tools: prefer grid points
            (ToolKind::SteelColumn, InferenceType::GridLine) => 35.0,
            (ToolKind::SteelColumn, InferenceType::Intersection) => 30.0,
            (ToolKind::SteelBeam, InferenceType::ColumnCenter) => 30.0,
            (ToolKind::SteelBeam, InferenceType::GridLine) => 25.0,

            // TapeMeasure: prefer exact points
            (ToolKind::TapeMeasure, InferenceType::Endpoint) => 35.0,
            (ToolKind::TapeMeasure, InferenceType::Midpoint) => 30.0,

            _ => 0.0,
        }
    }
    fn reason(&self, c: &InferenceCandidate, ctx: &InferenceContext) -> Option<String> {
        let s = self.score(c, ctx);
        if s > 20.0 {
            Some(format!("工具偏好 +{:.0}", s))
        } else {
            None
        }
    }
}

/// Rule 4: Axis lock / sticky axis bonus
pub struct StickyAxisRule;
impl ScoreRule for StickyAxisRule {
    fn name(&self) -> &'static str {
        "軸鎖定"
    }
    fn layer(&self) -> ScoreLayer {
        ScoreLayer::Context
    }
    fn score(&self, c: &InferenceCandidate, ctx: &InferenceContext) -> f32 {
        if let Some(locked) = ctx.locked_axis {
            match (locked, c.inference_type) {
                (0, InferenceType::AxisLockX) => 40.0,
                (1, InferenceType::AxisLockY) => 40.0,
                (2, InferenceType::AxisLockZ) => 40.0,
                _ => -10.0, // penalize non-locked axis candidates
            }
        } else {
            0.0
        }
    }
    fn reason(&self, c: &InferenceCandidate, ctx: &InferenceContext) -> Option<String> {
        if ctx.locked_axis.is_some() && self.score(c, ctx) > 30.0 {
            Some("軸鎖定中".into())
        } else {
            None
        }
    }
}

/// Rule 5: Direction continuation (when drawing, prefer same direction)
pub struct IntentContinuationRule;
impl ScoreRule for IntentContinuationRule {
    fn name(&self) -> &'static str {
        "延續方向"
    }
    fn layer(&self) -> ScoreLayer {
        ScoreLayer::Intent
    }
    fn score(&self, c: &InferenceCandidate, ctx: &InferenceContext) -> f32 {
        if !ctx.is_drawing {
            return 0.0;
        }
        if let Some(dir) = ctx.last_direction {
            match c.inference_type {
                InferenceType::Parallel => 20.0,
                InferenceType::AxisLockX if dir[0].abs() > dir[1].abs() => 15.0,
                InferenceType::AxisLockZ if dir[1].abs() > dir[0].abs() => 15.0,
                _ => 0.0,
            }
        } else {
            0.0
        }
    }
    fn reason(&self, _c: &InferenceCandidate, ctx: &InferenceContext) -> Option<String> {
        if ctx.is_drawing && ctx.last_direction.is_some() {
            Some("延續上一步方向".into())
        } else {
            None
        }
    }
}

/// Rule 6: Repetition detection (same tool used many times → prefer consistency)
pub struct RepetitionRule;
impl ScoreRule for RepetitionRule {
    fn name(&self) -> &'static str {
        "重複操作"
    }
    fn layer(&self) -> ScoreLayer {
        ScoreLayer::Intent
    }
    fn score(&self, c: &InferenceCandidate, ctx: &InferenceContext) -> f32 {
        if ctx.consecutive_same_tool >= 3 {
            match c.inference_type {
                InferenceType::Endpoint | InferenceType::GridLine => 10.0,
                _ => 0.0,
            }
        } else {
            0.0
        }
    }
    fn reason(&self, _c: &InferenceCandidate, ctx: &InferenceContext) -> Option<String> {
        if ctx.consecutive_same_tool >= 3 {
            Some(format!("連續 {} 次相同操作", ctx.consecutive_same_tool))
        } else {
            None
        }
    }
}

/// Rule 7: Semantic alignment (near grid/column/beam)
pub struct SemanticAlignmentRule;
impl ScoreRule for SemanticAlignmentRule {
    fn name(&self) -> &'static str {
        "工程語意"
    }
    fn layer(&self) -> ScoreLayer {
        ScoreLayer::Semantic
    }
    fn score(&self, c: &InferenceCandidate, ctx: &InferenceContext) -> f32 {
        match (ctx.current_mode, c.inference_type) {
            (AppMode::Steel, InferenceType::GridLine) => 25.0,
            (AppMode::Steel, InferenceType::ColumnCenter) => 30.0,
            (AppMode::Steel, InferenceType::BeamAxis) => 25.0,
            _ => 0.0,
        }
    }
    fn reason(&self, c: &InferenceCandidate, ctx: &InferenceContext) -> Option<String> {
        if ctx.current_mode == AppMode::Steel {
            match c.inference_type {
                InferenceType::GridLine => Some("軸線對齊".into()),
                InferenceType::ColumnCenter => Some("柱中心".into()),
                InferenceType::BeamAxis => Some("梁軸線".into()),
                _ => None,
            }
        } else {
            None
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  Inference Engine
// ═══════════════════════════════════════════════════════════════

/// The main inference engine — scores candidates and resolves results
pub struct InferenceEngine {
    rules: Vec<Box<dyn ScoreRule>>,
}

impl InferenceEngine {
    /// Create with all built-in rules
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(GeometryDistanceRule),
                Box::new(FeatureTypeRule),
                Box::new(ToolPreferenceRule),
                Box::new(StickyAxisRule),
                Box::new(IntentContinuationRule),
                Box::new(RepetitionRule),
                Box::new(SemanticAlignmentRule),
            ],
        }
    }

    /// Add a custom rule
    #[allow(dead_code)]
    pub fn add_rule(&mut self, rule: Box<dyn ScoreRule>) {
        self.rules.push(rule);
    }

    /// Score all candidates and return sorted results
    pub fn score_candidates(
        &self,
        candidates: &[InferenceCandidate],
        ctx: &InferenceContext,
    ) -> Vec<ScoredCandidate> {
        let mut scored: Vec<ScoredCandidate> = candidates
            .iter()
            .map(|c| {
                let mut breakdown = ScoreBreakdown::default();

                for rule in &self.rules {
                    let s = rule.score(c, ctx);
                    if s.abs() < 0.01 {
                        continue;
                    }

                    match rule.layer() {
                        ScoreLayer::Geometry => breakdown.geometry += s,
                        ScoreLayer::Context => breakdown.context += s,
                        ScoreLayer::Semantic => breakdown.semantic += s,
                        ScoreLayer::Intent => breakdown.intent += s,
                    }

                    if let Some(reason) = rule.reason(c, ctx) {
                        breakdown.reasons.push(reason);
                    }
                }

                breakdown.compute_total();

                ScoredCandidate {
                    candidate: c.clone(),
                    breakdown,
                }
            })
            .collect();

        // Sort by total score descending
        scored.sort_by(|a, b| {
            b.breakdown
                .total
                .partial_cmp(&a.breakdown.total)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored
    }

    /// Resolve: get the top candidate for cursor hint
    #[allow(dead_code)]
    pub fn resolve_primary<'a>(
        &self,
        scored: &'a [ScoredCandidate],
        config: &ResolveConfig,
    ) -> Option<&'a ScoredCandidate> {
        scored.iter().find(|s| s.breakdown.total >= config.min_score)
    }

    /// Resolve: get top-N candidates for semantic review
    #[allow(dead_code)]
    pub fn resolve_semantic<'a>(
        &self,
        scored: &'a [ScoredCandidate],
        config: &ResolveConfig,
    ) -> Vec<&'a ScoredCandidate> {
        scored
            .iter()
            .filter(|s| s.breakdown.total >= config.min_score)
            .take(config.max_results)
            .collect()
    }

    /// Generate a debug report for the console
    #[allow(dead_code)]
    pub fn debug_report(&self, scored: &[ScoredCandidate]) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push("[Inference Engine 2.0]".into());
        lines.push(format!("  Candidates: {}", scored.len()));
        lines.push(format!(
            "  Rules: {}",
            self.rules
                .iter()
                .map(|r| r.name())
                .collect::<Vec<_>>()
                .join(", ")
        ));

        for (i, s) in scored.iter().take(5).enumerate() {
            lines.push(format!(
                "  #{} {:?} score={:.0} (G:{:.0} C:{:.0} S:{:.0} I:{:.0})",
                i + 1,
                s.candidate.inference_type,
                s.breakdown.total,
                s.breakdown.geometry,
                s.breakdown.context,
                s.breakdown.semantic,
                s.breakdown.intent,
            ));
            for reason in &s.breakdown.reasons {
                lines.push(format!("      → {}", reason));
            }
        }

        lines
    }
}

/// Configuration for the resolver
#[derive(Debug, Clone)]
pub struct ResolveConfig {
    pub min_score: f32,
    pub max_results: usize,
    #[allow(dead_code)]
    pub prefer_stable: bool, // prefer same candidate as last frame
}

impl Default for ResolveConfig {
    fn default() -> Self {
        Self {
            min_score: 20.0,
            max_results: 10,
            prefer_stable: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  Conversion helpers (from app types to engine types)
// ═══════════════════════════════════════════════════════════════

/// Convert app Tool to engine ToolKind
pub fn tool_to_kind(tool: crate::app::Tool) -> ToolKind {
    use crate::app::Tool;
    match tool {
        Tool::Select => ToolKind::Select,
        Tool::Move => ToolKind::Move,
        Tool::Rotate => ToolKind::Rotate,
        Tool::Scale => ToolKind::Scale,
        Tool::Line => ToolKind::Line,
        Tool::Arc => ToolKind::Arc,
        Tool::Rectangle => ToolKind::Rectangle,
        Tool::Circle => ToolKind::Circle,
        Tool::CreateBox => ToolKind::Box,
        Tool::CreateCylinder => ToolKind::Cylinder,
        Tool::CreateSphere => ToolKind::Sphere,
        Tool::PushPull => ToolKind::PushPull,
        Tool::Offset => ToolKind::Offset,
        Tool::FollowMe => ToolKind::FollowMe,
        Tool::TapeMeasure | Tool::Dimension => ToolKind::TapeMeasure,
        Tool::PaintBucket => ToolKind::PaintBucket,
        Tool::Orbit => ToolKind::Orbit,
        Tool::Pan | Tool::ZoomExtents => ToolKind::Pan,
        Tool::SteelColumn => ToolKind::SteelColumn,
        Tool::SteelBeam => ToolKind::SteelBeam,
        Tool::SteelGrid => ToolKind::SteelGrid,
        Tool::Eraser => ToolKind::Eraser,
        Tool::Text | Tool::Group | Tool::Component
        | Tool::SteelBrace | Tool::SteelPlate | Tool::SteelConnection
        | Tool::Arc3Point | Tool::Pie => ToolKind::Other,
    }
}

/// Convert app SnapType to engine InferenceType
pub fn snap_type_to_inference_type(snap: &crate::app::SnapType) -> InferenceType {
    use crate::app::SnapType;
    match snap {
        SnapType::Endpoint => InferenceType::Endpoint,
        SnapType::Midpoint => InferenceType::Midpoint,
        SnapType::Origin => InferenceType::Origin,
        SnapType::AxisX => InferenceType::AxisLockX,
        SnapType::AxisY => InferenceType::AxisLockY,
        SnapType::AxisZ => InferenceType::AxisLockZ,
        SnapType::Parallel => InferenceType::Parallel,
        SnapType::Perpendicular => InferenceType::Perpendicular,
        SnapType::Intersection => InferenceType::Intersection,
        SnapType::OnEdge => InferenceType::OnEdge,
        SnapType::OnFace | SnapType::FaceCenter => InferenceType::OnFace,
        SnapType::Grid => InferenceType::Grid,
        SnapType::None => InferenceType::Custom,
    }
}

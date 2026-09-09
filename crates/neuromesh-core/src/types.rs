use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(pub String);

impl ProjectId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn random() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub Arc<str>);

impl NodeId {
    pub fn new(id: impl AsRef<str>) -> Self {
        Self(Arc::from(id.as_ref()))
    }

    pub fn from_file_path(path: &str) -> Self {
        let normalized = path.replace('\\', "/");
        Self(Arc::from(format!("file:{normalized}")))
    }

    pub fn from_symbol(file_path: &str, symbol: &str) -> Self {
        Self::from_symbol_parts(file_path, symbol, None)
    }

    /// Qualify by enclosing type so `TypeAdapter.write` and
    /// `NullSafeTypeAdapter.write` are distinct nodes in the same file.
    pub fn from_symbol_parts(file_path: &str, symbol: &str, parent: Option<&str>) -> Self {
        let normalized = file_path.replace('\\', "/");
        match parent.map(str::trim).filter(|p| !p.is_empty()) {
            Some(parent) => Self(Arc::from(format!("sym:{normalized}:{parent}.{symbol}"))),
            None => Self(Arc::from(format!("sym:{normalized}:{symbol}"))),
        }
    }

    pub fn random() -> Self {
        Self(Arc::from(Uuid::new_v4().to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EdgeId(pub Arc<str>);

impl EdgeId {
    pub fn new(source: &NodeId, target: &NodeId, edge_type: &EdgeType) -> Self {
        Self(Arc::from(format!(
            "{}->{}::{:?}",
            source.as_str(),
            target.as_str(),
            edge_type
        )))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EdgeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Project,
    Directory,
    File,
    Component,
    Class,
    Function,
    Symbol,
    Import,
    Dependency,
    Api,
    DbModel,
    Test,
    Config,
    Doc,
    Task,
    Decision,
    Memory,
    StyleToken,
    // --- Artifact layer: ML / data-pipeline vocabulary ---------------------
    /// A dataset definition — a `Dataset` subclass, a loader, a data module.
    Dataset,
    /// A preprocessing or augmentation step applied to samples.
    Transform,
    /// A trainable model — an `nn.Module` subclass, a `from_pretrained` wrapper.
    Model,
    /// A named component *inside* a model: a backbone, a head, a block.
    Layer,
    /// The training loop that consumes a dataset and updates a model.
    TrainLoop,
    /// The evaluation / validation loop that scores a model.
    EvalLoop,
    /// A saved or loaded set of weights.
    Checkpoint,
    /// A quantity a loop reports: loss, accuracy, mAP.
    Metric,
    /// One configured run: a sweep entry, an experiment name, a tracked run.
    Experiment,
    /// A tunable value that parameterizes a model, loop, or transform.
    Hyperparameter,
    /// A notebook file as a whole.
    Notebook,
    /// One ordered cell inside a notebook.
    NotebookCell,
}

impl NodeType {
    /// Nodes that exist because a framework overlay recognised them, rather
    /// than because the language parser found a symbol.
    pub fn is_artifact(&self) -> bool {
        matches!(
            self,
            NodeType::Dataset
                | NodeType::Transform
                | NodeType::Model
                | NodeType::Layer
                | NodeType::TrainLoop
                | NodeType::EvalLoop
                | NodeType::Checkpoint
                | NodeType::Metric
                | NodeType::Experiment
                | NodeType::Hyperparameter
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    Imports,
    Calls,
    References,
    Contains,
    DependsOn,
    ModifiedWith,
    TestedBy,
    RelatedTo,
    UsedBy,
    PreviouslySuccessfulWith,
    // --- Artifact layer: how ML artifacts relate --------------------------
    /// A training loop trains a model.
    Trains,
    /// An evaluation loop scores a model.
    Evaluates,
    /// A loop or function emits an artifact: a checkpoint, a metric value.
    Produces,
    /// A loop or model reads an artifact: a dataset, a checkpoint.
    Consumes,
    /// A transform is applied to a dataset or a sample.
    Transforms,
    /// A checkpoint holds the weights of a model.
    CheckpointOf,
    /// A model's quality is reported by a metric.
    MeasuredBy,
    /// A hyperparameter configures a model, loop, or transform.
    Parameterizes,
    /// Ordering between notebook cells: an earlier cell precedes a later one.
    Precedes,
}

impl EdgeType {
    /// Base propagation attenuation factor for spreading activation
    pub fn attenuation(&self) -> f32 {
        match self {
            EdgeType::Imports => 1.0,
            EdgeType::DependsOn => 0.95,
            EdgeType::Calls => 0.85,
            EdgeType::PreviouslySuccessfulWith => 0.90,
            EdgeType::Contains => 0.80,
            EdgeType::References => 0.75,
            EdgeType::ModifiedWith => 0.70,
            EdgeType::UsedBy => 0.65,
            EdgeType::TestedBy => 0.50,
            EdgeType::RelatedTo => 0.40,
            // Artifact edges carry the "why did this metric move?" chain, and a
            // metric sits six hops from the dataset that produced it, so they
            // have to survive the walk the way Imports and DependsOn do.
            EdgeType::Trains => 0.95,
            EdgeType::Evaluates => 0.95,
            EdgeType::Produces => 0.92,
            EdgeType::Consumes => 0.92,
            EdgeType::CheckpointOf => 0.90,
            EdgeType::MeasuredBy => 0.90,
            EdgeType::Transforms => 0.88,
            EdgeType::Parameterizes => 0.85,
            // Cell adjacency is real but weak: being the next cell says little.
            EdgeType::Precedes => 0.60,
        }
    }

    /// Edges introduced by the artifact layer, as opposed to language-level
    /// structure recovered from the AST.
    pub fn is_artifact(&self) -> bool {
        matches!(
            self,
            EdgeType::Trains
                | EdgeType::Evaluates
                | EdgeType::Produces
                | EdgeType::Consumes
                | EdgeType::Transforms
                | EdgeType::CheckpointOf
                | EdgeType::MeasuredBy
                | EdgeType::Parameterizes
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextNode {
    pub id: NodeId,
    pub project_id: ProjectId,
    pub file_path: PathBuf,
    pub node_type: NodeType,
    pub name: String,
    pub signature: Option<String>,
    #[serde(default)]
    pub doc_summary: Option<String>,
    pub line_range: Option<Range<usize>>,
    pub token_cost: usize,
    pub content: Option<String>,
    pub content_hash: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub base_relevance: f32,
    pub access_count: u64,
    pub last_accessed: DateTime<Utc>,
}

impl ContextNode {
    /// Clone metadata only — skip source bodies so the galaxy UI is not a 2MB JSON dump.
    pub fn without_content(&self) -> Self {
        Self {
            id: self.id.clone(),
            project_id: self.project_id.clone(),
            file_path: self.file_path.clone(),
            node_type: self.node_type,
            name: self.name.clone(),
            signature: self.signature.clone(),
            doc_summary: self.doc_summary.clone(),
            line_range: self.line_range.clone(),
            token_cost: self.token_cost,
            content: None,
            content_hash: self.content_hash.clone(),
            parent: self.parent.clone(),
            base_relevance: self.base_relevance,
            access_count: self.access_count,
            last_accessed: self.last_accessed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEdge {
    pub id: EdgeId,
    pub project_id: ProjectId,
    pub source: NodeId,
    pub target: NodeId,
    pub edge_type: EdgeType,
    pub pheromone_weight: f32,
    pub reinforcement_count: u64,
    pub failure_count: u64,
    pub last_reinforced: DateTime<Utc>,
    #[serde(default)]
    pub confidence: EdgeConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EdgeConfidence {
    Proven,
    #[default]
    Likely,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedRef {
    pub name: String,
    pub from: String,
    pub from_file: PathBuf,
    pub reason: String,
    pub relationship: EdgeType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedResolution {
    pub query: String,
    pub resolved_id: Option<NodeId>,
    pub confidence: f32,
    /// How this seed was resolved: `L1_exact`, `L2_pattern`, `L3_semantic_recovery`, `embedding_primary`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_tier: Option<String>,
    /// Cosine similarity when resolved via embedding (L3 / embedding engine).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketGap {
    pub kind: String,
    pub path: String,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedFile {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralEvidence {
    pub symbol: String,
    pub path: String,
    pub line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_line: Option<String>,
    pub callers_count: usize,
    pub is_dead: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub who_reads: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageReport {
    pub seeds_hit: Vec<String>,
    pub seeds_missed: Vec<String>,
    pub claim: String,
    /// Files the task likely needs but that are not in the packet.
    #[serde(default)]
    pub packet_gaps: Vec<PacketGap>,
    /// Near-miss files worth expanding before Grep.
    #[serde(default)]
    pub unsure: Vec<String>,
    /// Files included in this packet.
    #[serde(default)]
    pub covered: Vec<String>,
    /// Files intentionally excluded (noise filter, budget, parse partial).
    #[serde(default)]
    pub skipped: Vec<SkippedFile>,
    /// For style tasks: share of packet files under styles/ (0.0–1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_coverage: Option<f32>,
    /// Connector / physarum fill files included for context but not task anchors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sidecar_files: Vec<String>,
}

impl CoverageReport {
    /// `no_recorded_gap` only when every attempted seed resolved, packet gaps are empty,
    /// no sidecar connector files, and the packet was not truncated by budget.
    /// `bounded` when seeds resolved but optional connector/sidecar fill or budget cut applied.
    pub fn from_seeds(seeds: &[SeedResolution]) -> Self {
        Self::from_seeds_with_gaps(
            seeds,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_seeds_with_gaps(
        seeds: &[SeedResolution],
        packet_gaps: Vec<PacketGap>,
        unsure: Vec<String>,
        covered: Vec<String>,
        skipped: Vec<SkippedFile>,
        semantic_coverage: Option<f32>,
        sidecar_files: Vec<String>,
        budget_truncated: bool,
    ) -> Self {
        let seeds_hit: Vec<String> = seeds
            .iter()
            .filter(|s| s.resolved_id.is_some())
            .map(|s| s.query.clone())
            .collect();
        let seeds_missed: Vec<String> = seeds
            .iter()
            .filter(|s| s.resolved_id.is_none())
            .map(|s| s.query.clone())
            .collect();
        let claim = if seeds_hit.is_empty() {
            "no_seed_resolved".to_string()
        } else if !seeds_missed.is_empty() || !packet_gaps.is_empty() {
            "partial".to_string()
        } else if !sidecar_files.is_empty() || budget_truncated {
            "bounded".to_string()
        } else {
            "no_recorded_gap".to_string()
        };
        Self {
            seeds_hit,
            seeds_missed,
            claim,
            packet_gaps,
            unsure,
            covered,
            skipped,
            semantic_coverage,
            sidecar_files,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextAction {
    pub tool: String,
    pub query: String,
    pub why: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMeta {
    pub generation: u64,
    pub file_count: usize,
    pub indexed_at: DateTime<Utc>,
    pub stale_files: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextStatus {
    Active,
    Inactive,
    Expanded,
    Bypass,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InactiveContextDescriptor {
    pub id: NodeId,
    pub file_path: PathBuf,
    pub line_range: Option<Range<usize>>,
    pub content_hash: String,
    pub version: u64,
    pub token_cost: usize,
    pub relevance: f32,
    pub confidence: f32,
    pub activation_score: f32,
    pub parent_node: Option<NodeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivatedNodeView {
    pub node: ContextNode,
    pub activation_score: f32,
    pub status: ContextStatus,
    pub expansion_reason: Option<String>,
    #[serde(default)]
    pub sidecar: bool,
    #[serde(default)]
    pub folded_symbols: Vec<String>,
}

/// Runtime retrieval estimate — a decision signal, not evaluation ground truth.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetrievalMetadata {
    pub retrieval_level: String,
    pub sufficiency_score: f32,
    pub confidence: f32,
    pub claim: String,
    #[serde(default)]
    pub levels_attempted: Vec<String>,
    #[serde(default)]
    pub latency_ms: std::collections::HashMap<String, u64>,
    /// Hard invariant: must always be false (no full-workspace fallback).
    #[serde(default)]
    pub full_workspace_fallback: bool,
    #[serde(default)]
    pub critical_gaps: Vec<String>,
    #[serde(default)]
    pub non_critical_gaps: Vec<String>,
    #[serde(default)]
    pub eligible_for_early_exit: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
    #[serde(default)]
    pub suggested_keywords: Option<Vec<String>>,
    #[serde(default)]
    pub embedding_used: bool,
    /// Dominant resolution tier for this packet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_tier: Option<String>,
    /// Best embedding cosine among resolved seeds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_embedding_score: Option<f32>,
    /// True when ONNX embedder singleton is loaded (session retained until MCP restart).
    #[serde(default, skip_serializing_if = "is_false")]
    pub ort_session_active: bool,
    /// True when MCP returned a semantically cached packet (near-duplicate prompt).
    #[serde(default, skip_serializing_if = "is_false")]
    pub cache_hit: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextView {
    pub project_id: ProjectId,
    pub active_nodes: Vec<ActivatedNodeView>,
    pub inactive_descriptors: Vec<InactiveContextDescriptor>,
    pub total_raw_tokens: usize,
    pub active_tokens: usize,
    pub reduction_percentage: f32,
    pub confidence_score: f32,
    pub bypass_applied: bool,
    #[serde(default)]
    pub seeds: Vec<SeedResolution>,
    #[serde(default)]
    pub unresolved: Vec<UnresolvedRef>,
    #[serde(default)]
    pub coverage: Option<CoverageReport>,
    #[serde(default)]
    pub next_actions: Vec<NextAction>,
    #[serde(default)]
    pub budget_used: usize,
    #[serde(default)]
    pub budget_cap: usize,
    #[serde(default)]
    pub budget_mode: String,
    /// Tokens that must ship (resolved seed files/symbols after skeletonize).
    #[serde(default)]
    pub budget_seed_tokens: usize,
    /// Extra connector tokens actually filled.
    #[serde(default)]
    pub budget_fill_used: usize,
    /// Extra connector allowance for this mode (0 / 5000 / 16000 on top of seeds).
    #[serde(default)]
    pub budget_fill_cap: usize,
    /// True when fill exceeded fill_cap (should not happen) or seeds alone are huge.
    #[serde(default)]
    pub over_budget: bool,
    #[serde(default)]
    pub fold_ids: Vec<String>,
    /// Fraction of seed outbound Calls whose target file is in the packet.
    #[serde(default)]
    pub seed_call_coverage: f32,
    /// Indexed workspace token count (for honest reduction vs dump-all).
    #[serde(default)]
    pub workspace_tokens: usize,
    /// True when neighborhood Physarum actually built tubes for this packet.
    #[serde(default)]
    pub physarum_used: bool,
    /// Wall time of the Physarum tube solve in milliseconds (0 if skipped).
    #[serde(default)]
    pub physarum_ms: u64,
    /// `seed_then_fill` or `physarum_seed_fill`.
    #[serde(default)]
    pub selection_method: String,
    /// `brownfield` when symbol seeds resolved; `greenfield` when scaffold entry points were used.
    #[serde(default = "default_task_scenario")]
    pub task_scenario: String,
    /// Ranked file candidates (selected + runners-up) for before/after feedback comparison.
    #[serde(default)]
    pub rank_candidates: Vec<RankCandidateView>,
    /// Caller counts / dead-code hints for seeded symbols.
    #[serde(default)]
    pub structural_evidence: Vec<StructuralEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_resolution_telemetry: Option<crate::SeedResolutionTelemetry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet_header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval: Option<RetrievalMetadata>,
    #[serde(default)]
    pub embedding_used: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmissionDropStage {
    #[default]
    None,
    StyleFilter,
    FocusTighten,
    HmvcFilter,
    SchemaFilter,
    PenalizedSuppress,
    LearningRerank,
    FillCap,
    PacketCap,
    NoiseFilter,
    SemanticDuplicate,
    NotSelected,
}

impl EmissionDropStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::StyleFilter => "style_filter",
            Self::FocusTighten => "focus_tighten",
            Self::HmvcFilter => "hmvc_filter",
            Self::SchemaFilter => "schema_filter",
            Self::PenalizedSuppress => "penalized_suppress",
            Self::LearningRerank => "learning_rerank",
            Self::FillCap => "fill_cap",
            Self::PacketCap => "packet_cap",
            Self::NoiseFilter => "noise_filter",
            Self::SemanticDuplicate => "semantic_duplicate",
            Self::NotSelected => "not_selected",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextScoreBreakdown {
    pub utility_score: f32,
    pub semantic_score: f32,
    pub graph_score: f32,
    pub learned_score: f32,
    pub pheromone_score: f32,
    pub negative_penalty: f32,
    pub final_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankCandidateView {
    pub path: String,
    pub score: f32,
    pub learning_bonus: f32,
    pub reason: String,
    pub selected: bool,
    #[serde(default)]
    pub emitted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drop_stage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_breakdown: Option<ContextScoreBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDiff {
    pub base_hash: String,
    pub new_hash: String,
    pub file_path: PathBuf,
    pub added_lines: Vec<(usize, String)>,
    pub removed_lines: Vec<(usize, String)>,
    pub net_token_change: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationMetadata {
    pub request_id: String,
    pub task_id: Option<String>,
    pub project_id: ProjectId,
    pub mode: String,
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub token_reduction_pct: f32,
    pub nodes_before: usize,
    pub nodes_after: usize,
    pub expansions_count: usize,
    pub cache_hit: bool,
    pub provider: String,
    pub model: String,
    pub latency_ms: u64,
    pub success: bool,
    pub timestamp: DateTime<Utc>,
    /// Canonical workspace root when known (MCP / CLI / monitor).
    #[serde(default)]
    pub workspace_path: Option<String>,
    /// Origin surface: `mcp`, `cli`, `monitor`, `openai`.
    #[serde(default = "default_telemetry_surface")]
    pub surface: String,
    /// MCP client name from initialize (Cursor, VS Code, …).
    #[serde(default)]
    pub client_id: Option<String>,
    /// Tool or CLI subcommand (`get_context`, `index`, `optimize`, …).
    #[serde(default)]
    pub command: Option<String>,
}

fn default_task_scenario() -> String {
    "brownfield".into()
}

fn default_telemetry_surface() -> String {
    "mcp".into()
}

#[cfg(test)]
mod tests {
    use super::NodeId;

    #[test]
    fn symbol_node_id_includes_enclosing_type() {
        let outer =
            NodeId::from_symbol_parts("gson/TypeAdapter.java", "write", Some("TypeAdapter"));
        let inner = NodeId::from_symbol_parts(
            "gson/TypeAdapter.java",
            "write",
            Some("NullSafeTypeAdapter"),
        );
        assert_ne!(outer, inner);
        assert_eq!(
            outer.as_str(),
            "sym:gson/TypeAdapter.java:TypeAdapter.write"
        );
        assert_eq!(
            inner.as_str(),
            "sym:gson/TypeAdapter.java:NullSafeTypeAdapter.write"
        );
        let bare = NodeId::from_symbol("gson/TypeAdapter.java", "write");
        assert_eq!(bare.as_str(), "sym:gson/TypeAdapter.java:write");
    }

    #[test]
    fn coverage_claim_bounded_when_sidecars_present() {
        use super::{CoverageReport, SeedResolution};
        let seeds = vec![SeedResolution {
            query: "CheckoutView".into(),
            resolved_id: Some(NodeId::new("file:checkout")),
            confidence: 1.0,
            resolution_tier: None,
            embedding_score: None,
        }];
        let report = CoverageReport::from_seeds_with_gaps(
            &seeds,
            Vec::new(),
            Vec::new(),
            vec!["src/CheckoutView.vue".into()],
            Vec::new(),
            None,
            vec!["src/App.vue".into()],
            false,
        );
        assert_eq!(report.claim, "bounded");
    }
}

#[cfg(test)]
mod artifact_ir_tests {
    use super::*;

    #[test]
    fn artifact_edges_survive_the_metric_to_dataset_walk() {
        // The killer query walks Metric -> EvalLoop -> Model -> Checkpoint ->
        // Transform -> Dataset. Five hops of pure attenuation must not bury the
        // dataset below the weakest language edge a single hop away.
        let chain = [
            EdgeType::MeasuredBy,
            EdgeType::Evaluates,
            EdgeType::CheckpointOf,
            EdgeType::Consumes,
            EdgeType::Transforms,
        ];
        let survived: f32 = chain.iter().map(|e| e.attenuation()).product();
        assert!(
            survived > EdgeType::RelatedTo.attenuation(),
            "artifact chain attenuates to {survived}, below a single RelatedTo hop"
        );
    }

    #[test]
    fn artifact_predicates_agree_with_the_variant_lists() {
        assert!(NodeType::Model.is_artifact());
        assert!(NodeType::Metric.is_artifact());
        // A notebook is a file on disk, not a recognised ML artifact.
        assert!(!NodeType::Notebook.is_artifact());
        assert!(!NodeType::NotebookCell.is_artifact());
        assert!(!NodeType::Function.is_artifact());

        assert!(EdgeType::Trains.is_artifact());
        assert!(EdgeType::Parameterizes.is_artifact());
        // Cell ordering is structural, not an artifact relation.
        assert!(!EdgeType::Precedes.is_artifact());
        assert!(!EdgeType::Calls.is_artifact());
    }

    #[test]
    fn artifact_variants_serialize_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&NodeType::TrainLoop).unwrap(),
            "\"train_loop\""
        );
        assert_eq!(
            serde_json::to_string(&EdgeType::CheckpointOf).unwrap(),
            "\"checkpoint_of\""
        );
    }
}

pub mod activation;
mod chunk_rank;
pub use chunk_rank::{ChunkOpts, RankedDefinition};
pub mod concept_index;
pub mod edge;
pub mod embeddings;
pub mod file_rank;
pub mod graph;
mod intern;
pub mod manifest;
pub mod node;
pub mod physarum;
pub mod query;
pub mod synapse;

#[cfg(test)]
mod isolation_tests;
#[cfg(test)]
mod quality_tests;
#[cfg(test)]
mod repo_quality_tests;

pub use activation::{SpreadingActivation, SpreadingActivationConfig};
pub use concept_index::{ConceptId, ConceptIndex};
pub use edge::{PheromoneConfig, PheromoneEngine};
#[cfg(feature = "embeddings")]
pub use embeddings::{
    coarse_candidate_indices, concept_stem_patterns, ensure_file_tier_sidecar, graph_digest,
    lazy_embed_symbols_for_files, maybe_rebuild_embeddings, rebuild_embeddings,
    rebuild_embeddings_for_workspace, refresh_embeddings_after_index, rerank_file_hits,
    sidecar_tier_stats, stem_union_file_hits,
};
pub use embeddings::{load_sidecar, EmbeddingIndex, EmbeddingSidecar};
pub use file_rank::RankedFile;
pub use graph::{
    node_learning_bonus, path_echoes_symbol, GraphStats, IndexState, NeuralProjectGraph,
    NodeLearningProfile, ProjectIdReconciliation, GRAPH_PARSER_EPOCH,
};
pub use node::NodeFactory;
pub use physarum::{PhysarumConfig, PhysarumResult, PhysarumSolver};
pub use query::{
    ArchitecturePackage, ArchitectureSummary, ImpactResult, NeighborView, SearchHit,
    TraceDirection, TraceHop, TraceResult,
};
pub use synapse::{NeuralSpike, StdpConfig, SynapticPlasticityEngine};

/// `NM_TIMING=1`: stage timings on stderr (index stages, rank index build).
pub fn timing_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("NM_TIMING").is_ok_and(|v| v == "1"))
}

/// Print `[timing] label elapsed` when [`timing_enabled`].
pub fn timing(label: &str, since: std::time::Instant) {
    if timing_enabled() {
        eprintln!("[timing] {label} {:?}", since.elapsed());
    }
}

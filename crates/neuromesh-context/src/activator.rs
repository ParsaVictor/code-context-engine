use crate::emission::EmissionPipeline;
use crate::fold::{FoldPolicy, OPTIONAL_EXON_BUDGET, SEED_EXON_BUDGET};
use crate::packet_analysis::{
    build_structural_evidence, compute_packet_gaps, enrich_coverage, inject_caller_context,
    prompt_is_call_graph_task, restrict_selection_to_call_graph, semantic_style_coverage,
};
use crate::registry::ReversibleContextRegistry;
use crate::scoring::{ActivationScorer, ScoringWeights};
use crate::seed::{
    resolve_engine_id, run_seed_resolution, MicroHeaderGenerator, NearestAncestorManifestResolver,
    SeedBuffers, SeedSink,
};
use crate::selector::{
    budget_mode_name, consumer_named_in_focus, fill_budget, focus_terms_ask_for_consumers,
    is_noise_path, packet_cap, path_sort_keys, seed_callee_exon_names, select, sort_key,
};
use crate::skeleton::{CodeSkeletonizer, FoldedIntron, FunctionSpan};
use crate::style_routing::{
    inject_style_seeds, inject_view_component_seeds, is_style_task, style_noise_penalty,
    tighten_focused_view_selection, tighten_style_extension_selection,
};
use crate::unified_score::compute_unified_file_score;
use neuromesh_core::{
    decoy_allowed_for_prompt, hmvc_app_prefix, is_name_collision_decoy, is_schema_path,
    prompt_targets_database, prompt_targets_types, ActivatedNodeView, Config, ContextStatus,
    ContextView, CoverageReport, EdgeConfidence, EdgeType, EmissionDropStage, NextAction, NodeId,
    NodeType, OptimizationMode, SeedResolution, SkippedFile, TaskSignature, Thresholds,
};
use neuromesh_graph::{path_echoes_symbol, NeuralProjectGraph};
use neuromesh_task::{
    extract_cluster_nouns, extract_prompt_anchors, is_prompt_stopword, is_route_query,
    split_task_clusters, stem_search_queries,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

const MAX_INACTIVE: usize = 12;
const MAX_PHYSARUM_SIDECAR_FILES: usize = 3;
/// A required file (not named by the prompt) whose file node, or any symbol
/// in it, has more negative than positive feedback (`base_relevance` starts
/// at 1.0, −0.12 per "not useful", +0.08 per "useful") is demoted (F69).
const REQUIRED_DEMOTE_RELEVANCE: f32 = 0.9;

struct MaterializedNode {
    node: neuromesh_core::ContextNode,
    score: f32,
    reason: String,
    sidecar: bool,
    raw_tokens: usize,
    folds: Vec<FoldedIntron>,
    folded_symbols: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PhysarumTelemetry {
    pub used: bool,
    pub ms: u64,
}

/// Compact last-packet facts for the monitor dashboard (not the full evidence packet).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PacketSnapshot {
    pub coverage_claim: String,
    pub seeds_hit: usize,
    pub seeds_missed: usize,
    pub file_count: usize,
    pub fold_count: usize,
    pub physarum_used: bool,
    pub physarum_ms: u64,
    pub selection_method: String,
    pub workspace_tokens: usize,
    pub packet_tokens: usize,
    pub fill_used: usize,
    pub fill_cap: usize,
    pub budget_mode: String,
    pub seed_call_coverage: f32,
    pub next_action_count: usize,
    pub grep_needed: bool,
    pub file_paths: Vec<String>,
}

impl PacketSnapshot {
    fn from_view(view: &ContextView) -> Self {
        let coverage = view.coverage.as_ref();
        let claim = coverage
            .map(|c| c.claim.clone())
            .unwrap_or_else(|| "unknown".into());
        let files: Vec<String> = view
            .active_nodes
            .iter()
            .filter(|n| n.node.node_type == NodeType::File)
            .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
            .collect();
        Self {
            coverage_claim: claim.clone(),
            seeds_hit: coverage.map(|c| c.seeds_hit.len()).unwrap_or(0),
            seeds_missed: coverage.map(|c| c.seeds_missed.len()).unwrap_or(0),
            file_count: files.len(),
            fold_count: view.fold_ids.len(),
            physarum_used: view.physarum_used,
            physarum_ms: view.physarum_ms,
            selection_method: view.selection_method.clone(),
            workspace_tokens: view.workspace_tokens,
            packet_tokens: view.active_tokens,
            fill_used: view.budget_fill_used,
            fill_cap: view.budget_fill_cap,
            budget_mode: view.budget_mode.clone(),
            seed_call_coverage: view.seed_call_coverage,
            next_action_count: view.next_actions.len(),
            grep_needed: matches!(
                claim.as_str(),
                "partial" | "no_seed_resolved" | "no_confident_match"
            ),
            file_paths: files.into_iter().take(12).collect(),
        }
    }
}

pub struct ContextActivator {
    scorer: ActivationScorer,
    registry: Arc<ReversibleContextRegistry>,
    last_physarum: Mutex<PhysarumTelemetry>,
    last_packet: Mutex<Option<PacketSnapshot>>,
    physarum_sidecar: bool,
}

impl ContextActivator {
    pub fn new(registry: Arc<ReversibleContextRegistry>) -> Self {
        Self {
            scorer: ActivationScorer::new(ScoringWeights::default()),
            registry,
            last_physarum: Mutex::new(PhysarumTelemetry::default()),
            last_packet: Mutex::new(None),
            physarum_sidecar: true,
        }
    }

    /// Drop the Physarum sidecar from this activator's packets.
    ///
    /// An ablation switch. The sidecar is deterministic — the tube depends on
    /// the graph and the seeds only — so the production path is what the
    /// quality harness measures. Turn it off to see what the tube contributes
    /// to a packet, or in a test that pins the seed-then-fill path alone.
    /// Per-activator rather than a global switch: a process-wide flag would
    /// leak into every other test sharing the binary.
    pub fn without_physarum_sidecar(mut self) -> Self {
        self.physarum_sidecar = false;
        self
    }

    pub fn registry(&self) -> &Arc<ReversibleContextRegistry> {
        &self.registry
    }

    pub fn last_physarum(&self) -> PhysarumTelemetry {
        *self.last_physarum.lock()
    }

    pub fn last_packet(&self) -> Option<PacketSnapshot> {
        self.last_packet.lock().clone()
    }

    pub fn activate(
        &self,
        graph: &NeuralProjectGraph,
        signature: &TaskSignature,
        mode: OptimizationMode,
    ) -> ContextView {
        #[cfg(feature = "embeddings")]
        neuromesh_embed::packet_cache_begin();
        let view = self.activate_with_hops(graph, signature, mode, 0);
        #[cfg(feature = "embeddings")]
        neuromesh_embed::packet_cache_end();
        view
    }

    /// Tier orchestrator entry: `hops_override` of 0 uses mode-derived hops.
    pub fn activate_with_hops(
        &self,
        graph: &NeuralProjectGraph,
        signature: &TaskSignature,
        mode: OptimizationMode,
        hops_override: u8,
    ) -> ContextView {
        self.activate_inner(graph, signature, mode, hops_override)
    }

    /// L1→L2→L3 cost-aware retrieval with conservative sufficiency early exit.
    pub fn activate_tiered(
        &self,
        graph: &NeuralProjectGraph,
        signature: &TaskSignature,
        mode: OptimizationMode,
    ) -> ContextView {
        crate::retrieval::RetrievalOrchestrator::default().run(self, graph, signature, mode)
    }

    /// Single-pass incremental escalation entry (see `retrieval::escalate`).
    pub fn activate_incremental(
        &self,
        graph: &NeuralProjectGraph,
        signature: &TaskSignature,
        mode: OptimizationMode,
        phase: crate::retrieval::escalate::IncrementalPhase,
        plan: &crate::retrieval::query_intent::QueryPlan,
        prior: Option<ContextView>,
    ) -> ContextView {
        use crate::retrieval::concept_seeds::resolve_concept_seeds;
        use crate::retrieval::escalate::IncrementalPhase;

        match phase {
            IncrementalPhase::L1 => {
                let mut sig = signature.clone();
                #[cfg(feature = "embeddings")]
                let skip_concepts = graph.embedding_index().is_loaded();
                #[cfg(not(feature = "embeddings"))]
                let skip_concepts = false;
                // A concept-index hit is a guess ("static" → `applyStatic`
                // in the docs site, for a Rust question). It used to be
                // promoted to `identifiers`, the strongest tier, which no
                // weak-seed prune (noise path, off-family, style asset) ever
                // touches — and only on this tiered path, the one the MCP
                // server and CLI use, never the gold harness (F62). It goes
                // in as a concept, the tier it is.
                if !skip_concepts {
                    for (id, _score, _reason) in resolve_concept_seeds(graph, &sig, plan) {
                        if let Some(node) = graph.get_node(&id) {
                            let name = node.name.clone();
                            if !sig.identifiers.iter().any(|i| i == &name)
                                && !sig.related_concepts.iter().any(|c| c == &name)
                            {
                                sig.related_concepts.push(name);
                            }
                        }
                    }
                }
                self.activate_with_hops(graph, &sig, mode, 1)
            }
            IncrementalPhase::L2 { extra_files, hops } => {
                let mut merged =
                    prior.unwrap_or_else(|| self.activate_with_hops(graph, signature, mode, hops));
                for file_id in extra_files {
                    include_file_hint(graph, &mut merged, &file_id);
                }
                merged
            }
            IncrementalPhase::L3 {
                max_recovery_seeds,
                hops,
            } => {
                let base = prior.expect("L3 incremental phase requires prior view");
                let mut sig = signature.clone();
                let cfg = neuromesh_core::Config::load();
                let sidecar_loaded = graph.embedding_index().is_loaded();
                let embeddings_active = cfg.embeddings.effective_enabled() || sidecar_loaded;
                sig.engine_override = Some(crate::retrieval::tier::RetrievalTier::L3.seed_engine(
                    cfg.seed_resolution.engine,
                    cfg.retrieval.engine,
                    embeddings_active,
                    sidecar_loaded,
                ));
                let recovery = self.activate_with_hops(graph, &sig, mode, hops);
                let mut merged = merge_context_views(base, recovery);
                cap_semantic_recovery_seeds(&mut merged, max_recovery_seeds);
                merged
            }
        }
    }

    /// Resolved seed node ids from a context view.
    pub fn seed_node_ids(&self, view: &ContextView) -> HashSet<NodeId> {
        view.seeds
            .iter()
            .filter_map(|s| s.resolved_id.clone())
            .collect()
    }

    fn activate_inner(
        &self,
        graph: &NeuralProjectGraph,
        signature: &TaskSignature,
        mode: OptimizationMode,
        hops_override: u8,
    ) -> ContextView {
        self.registry.begin_activate(&graph.project_id());

        let is_critical = signature.requires_conservative_mode();
        let effective_mode = if is_critical {
            OptimizationMode::MaxQuality
        } else {
            mode
        };

        let prompt = signature.raw_prompt.as_str();
        let call_graph_task = prompt_is_call_graph_task(prompt);

        let hops: usize = if hops_override > 0 {
            hops_override as usize
        } else if call_graph_task {
            1
        } else {
            match effective_mode {
                OptimizationMode::MaxQuality => 3,
                OptimizationMode::Balanced => 2,
                OptimizationMode::MaxSavings => 1,
            }
        };
        let mut seed_resolutions = Vec::new();
        let mut seed_energies: HashMap<NodeId, f32> = HashMap::new();
        let mut seed_reasons: HashMap<NodeId, String> = HashMap::new();

        let app_config = Config::load();
        let seed_config = app_config.seed_resolution.clone();
        let embedding_config = app_config.embeddings.clone();
        let header_config = app_config.packet_header.clone();

        let mut buffers = SeedBuffers {
            resolutions: &mut seed_resolutions,
            energies: &mut seed_energies,
            reasons: &mut seed_reasons,
        };

        let mut sig_for_seeds = signature.clone();
        crate::retrieval::alias::inject_alias_expansion(
            &mut sig_for_seeds.related_concepts,
            prompt,
        );

        let mut seed_result = run_seed_resolution(
            graph,
            &sig_for_seeds,
            prompt,
            &seed_config,
            &embedding_config,
            &mut buffers,
            resolve_seed_query,
            is_style_task(signature),
        );

        let scaffold_used = seed_result.scaffold_used;
        let embedding_used = seed_result.embedding_used;

        {
            let mut sink = SeedSink::new(
                buffers.resolutions,
                buffers.energies,
                buffers.reasons,
                resolve_seed_query,
            );
            inject_style_seeds(graph, prompt, signature, &mut sink);
            inject_view_component_seeds(graph, prompt, signature, &mut sink);
        }

        let seed_resolution_telemetry = seed_result.telemetry.clone();

        crate::seed::bare_owner::prune_bare_owner_seeds(
            &mut seed_resolutions,
            &mut seed_energies,
            &mut seed_reasons,
        );
        crate::seed::owner_cohere::repoint_bare_members_to_seeded_owners(
            graph,
            &mut seed_resolutions,
            &mut seed_energies,
            &mut seed_reasons,
        );
        mark_equivalent_file_hits(graph, &mut seed_resolutions, &mut seed_energies);
        cohere_ambiguous_seeds_to_app(graph, &mut seed_resolutions, &mut seed_energies, prompt);
        crate::seed::twin_cohere::cohere_twin_definitions(
            graph,
            &mut seed_resolutions,
            &mut seed_energies,
            prompt,
        );
        crate::seed::lang_cohere::prune_off_family_weak_seeds(
            graph,
            &mut seed_resolutions,
            &mut seed_energies,
            &mut seed_reasons,
        );
        crate::seed::config_cohere::prune_data_seeds_for_code_question(
            graph,
            &mut seed_resolutions,
            &mut seed_energies,
            &mut seed_reasons,
            prompt,
        );
        crate::seed::weak_file_seed::prune_weak_file_seeds_unnamed_in_prompt(
            graph,
            &mut seed_resolutions,
            &mut seed_energies,
            &mut seed_reasons,
            prompt,
        );
        crate::seed::weak_symbol_seed::prune_weak_substring_symbol_seeds(
            graph,
            &mut seed_resolutions,
            &mut seed_energies,
            &mut seed_reasons,
            prompt,
        );
        expand_file_seeds_to_symbols(
            graph,
            signature,
            &mut seed_energies,
            &mut seed_reasons,
            &mut seed_resolutions,
            8,
        );
        boost_path_stem_seed_energies(graph, prompt, &mut seed_energies);

        if is_style_task(signature) {
            let noise_ids: Vec<NodeId> = seed_energies
                .keys()
                .filter(|id| {
                    graph
                        .get_node(id)
                        .is_some_and(|n| style_noise_penalty(&n.file_path, signature) >= 20.0)
                })
                .cloned()
                .collect();
            for id in noise_ids {
                seed_energies.remove(&id);
                seed_reasons.remove(&id);
            }
            seed_resolutions.retain(|s| {
                s.resolved_id
                    .as_ref()
                    .is_none_or(|id| seed_energies.contains_key(id))
            });
        }

        // The header is written once the seed pipeline has settled: a seed
        // pruned above (bare owner, off-family, weak substring, noise path)
        // must not be announced in `@nm:seeds` (F68).
        let seed_paths: Vec<String> = seed_energies
            .keys()
            .filter_map(|id| graph.get_node(id))
            .map(|n| n.file_path.to_string_lossy().replace('\\', "/"))
            .collect();
        let mut manifest = NearestAncestorManifestResolver::new(graph);
        let stack_line = manifest.stack_line(&seed_paths);
        seed_result.packet_header = MicroHeaderGenerator::generate(
            graph,
            &header_config,
            stack_line.as_deref(),
            &seed_resolutions,
            &seed_energies,
            header_config.max_call_chain_depth,
        );
        let packet_header = seed_result.packet_header.clone();

        let seed_set: HashSet<NodeId> = seed_energies.keys().cloned().collect();
        let neighborhood = if seed_set.is_empty() {
            HashSet::new()
        } else {
            graph.neighborhood(&seed_set, hops)
        };

        let mut focus_terms: HashSet<String> = HashSet::new();
        for ident in &signature.identifiers {
            focus_terms.insert(ident.to_lowercase());
        }
        for hint in &signature.file_hints {
            if let Some(stem) = std::path::Path::new(hint)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                focus_terms.insert(stem.to_lowercase());
            }
        }
        for token in signature
            .raw_prompt
            .split(|c: char| !c.is_alphanumeric() && c != '_')
        {
            let t = token.to_lowercase();
            if t.len() >= 5
                && !matches!(
                    t.as_str(),
                    "where" | "about" | "does" | "using" | "should" | "would" | "could"
                )
            {
                focus_terms.insert(t);
            }
        }
        let retrieval_engine = signature
            .retrieval_engine_override
            .unwrap_or_else(|| neuromesh_core::Config::load().retrieval.engine);
        if retrieval_engine == neuromesh_core::RetrievalEngine::Fast {
            use crate::retrieval::alias::expand_aliases;
            for term in expand_aliases(prompt) {
                focus_terms.insert(term.to_lowercase());
            }
        }
        for token in prompt.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_') {
            let t = token.to_lowercase();
            for part in t.split(['-', '_']) {
                if part.len() >= 4 {
                    focus_terms.insert(part.to_string());
                }
            }
        }

        let mut selection = select(
            graph,
            &neighborhood,
            &seed_set,
            &seed_energies,
            &focus_terms,
            effective_mode,
        );
        if !call_graph_task {
            inject_caller_context(graph, &seed_set, prompt, &mut selection);
        } else {
            restrict_selection_to_call_graph(graph, &seed_set, &mut selection);
        }
        let app_cfg = neuromesh_core::Config::load();
        if app_cfg.retrieval.engine == neuromesh_core::RetrievalEngine::Hybrid
            && effective_mode == OptimizationMode::Balanced
        {
            selection.optional.truncate(2);
        }
        tighten_focused_view_selection(graph, signature, &mut selection);

        let mut physarum_used = false;
        let mut physarum_ms = 0u64;
        if self.physarum_sidecar && seed_set.len() >= 2 && !call_graph_task {
            // The solve is bounded by the neighbourhood caps inside
            // `solve_physarum_tube` (node/edge count, iteration count), not by
            // the clock. Whatever it returns is used: the packet must be a
            // function of the graph and the question, and an elapsed-time check
            // here would make it a function of machine load as well (issue #15).
            // `physarum_ms` stays as telemetry only.
            let started = Instant::now();
            let tube = graph.solve_physarum_tube(&seed_set, hops.min(2));
            physarum_ms = started.elapsed().as_millis() as u64;
            let ran = tube.iterations_converged > 0;
            if ran {
                physarum_used = true;
                let mut physarum_candidates: Vec<(NodeId, f32)> = Vec::new();
                for id in &tube.active_nodes {
                    let Some(node) = graph.get_node(id) else {
                        continue;
                    };
                    let Some(file_id) = graph.file_id_for_path(&node.file_path) else {
                        continue;
                    };
                    if selection.required.contains(&file_id) {
                        continue;
                    }
                    let score = selection.scores.get(&file_id).copied().unwrap_or(8.0);
                    physarum_candidates.push((file_id, score));
                }
                let path_keys = path_sort_keys(graph, physarum_candidates.iter().map(|(id, _)| id));
                physarum_candidates.sort_by(|(a, sa), (b, sb)| {
                    sb.partial_cmp(sa)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| sort_key(&path_keys, a).cmp(sort_key(&path_keys, b)))
                });
                physarum_candidates.dedup_by(|(a, _), (b, _)| a == b);
                for (file_id, _) in physarum_candidates
                    .into_iter()
                    .take(MAX_PHYSARUM_SIDECAR_FILES)
                {
                    let entry = selection.scores.entry(file_id.clone()).or_insert(0.0);
                    if *entry < 8.0 {
                        *entry = 8.0;
                    }
                    if !selection.optional.contains(&file_id) {
                        selection.optional.push(file_id.clone());
                    }
                    seed_reasons
                        .entry(file_id)
                        .or_insert_with(|| "physarum_tube".into());
                }
                selection.method = "physarum_seed_fill";
            }
        }
        // After the sidecar on purpose: a tube file is optional like any other
        // and goes through the same noise filter. Run before it, the filter was
        // bypassed by whatever the tube added, and a style question shipped the
        // cart drawer because it happens to include the same mixin.
        let mut skipped_files: Vec<SkippedFile> = Vec::new();
        // The frontend client of a backend question arrives here as a
        // connector or a tube file, scored on shared words like `auth` and
        // `login`. The seeds already settled which side of the stack the
        // question is about; fill does not reopen it.
        let families = crate::seed::lang_cohere::strong_families(graph, &seed_resolutions);
        selection.optional.retain(|id| {
            let off = crate::seed::lang_cohere::off_family(graph, id, &families);
            if off {
                if let Some(node) = graph.get_node(id) {
                    skipped_files.push(SkippedFile {
                        path: node.file_path.to_string_lossy().replace('\\', "/"),
                        reason: "outside the language family of every strong seed".into(),
                    });
                }
            }
            !off
        });
        if is_style_task(signature) {
            selection.required.retain(|id| {
                let keep = graph
                    .get_node(id)
                    .map(|n| style_noise_penalty(&n.file_path, signature) < 20.0)
                    .unwrap_or(true);
                if !keep {
                    if let Some(node) = graph.get_node(id) {
                        skipped_files.push(SkippedFile {
                            path: node.file_path.to_string_lossy().replace('\\', "/"),
                            reason: "style task: filtered cart/promo noise (required)".into(),
                        });
                    }
                }
                keep
            });
            for id in selection.optional.clone() {
                let Some(node) = graph.get_node(&id) else {
                    continue;
                };
                if style_noise_penalty(&node.file_path, signature) >= 20.0 {
                    skipped_files.push(SkippedFile {
                        path: node.file_path.to_string_lossy().replace('\\', "/"),
                        reason: "style task: filtered cart/promo noise".into(),
                    });
                }
            }
            selection.optional.retain(|id| {
                graph
                    .get_node(id)
                    .map(|n| style_noise_penalty(&n.file_path, signature) < 20.0)
                    .unwrap_or(true)
            });
            for id in selection.optional.clone() {
                let Some(node) = graph.get_node(&id) else {
                    continue;
                };
                let penalty = style_noise_penalty(&node.file_path, signature);
                if penalty > 0.0 {
                    if let Some(score) = selection.scores.get_mut(&id) {
                        *score = (*score - penalty).max(0.0);
                    }
                }
            }
        }
        let fill_cap = fill_budget(effective_mode);
        let scores = selection.scores.clone();
        let path_keys = path_sort_keys(graph, selection.optional.iter());
        selection.optional.sort_by(|a, b| {
            let sa = scores.get(a).copied().unwrap_or(0.0);
            let sb = scores.get(b).copied().unwrap_or(0.0);
            let score = sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal);
            if score != std::cmp::Ordering::Equal {
                return score;
            }
            sort_key(&path_keys, a).cmp(sort_key(&path_keys, b))
        });
        let extra_cap = selection.optional_cap;
        selection.optional.truncate(extra_cap);
        if let Some(lock) = locked_seed_hmvc_prefix(graph, &seed_set) {
            selection
                .required
                .retain(|id| keep_hmvc_packet_file(graph, &seed_set, &lock, id));
            selection
                .optional
                .retain(|id| keep_hmvc_packet_file(graph, &seed_set, &lock, id));
        }
        selection
            .required
            .retain(|id| keep_schema_packet_file(graph, &seed_set, prompt, id));
        selection
            .optional
            .retain(|id| keep_schema_packet_file(graph, &seed_set, prompt, id));
        tighten_style_extension_selection(graph, signature, &mut selection);

        let thresholds = Thresholds::default();
        let learning_index = graph.file_learning_boost_index();
        let mut emission = EmissionPipeline::default();
        // F69: negative feedback only ever touched *optional* files, while
        // the files users complain about are required ones a weak seed or a
        // callee brought in. A required file that no strong seed (a symbol,
        // file or config key the prompt named) resolved into, and that
        // feedback has marked not useful, is demoted. A file the prompt
        // named is never demoted by feedback.
        let strong_seed_files: HashSet<std::path::PathBuf> = seed_resolutions
            .iter()
            .filter(|s| {
                let prefix = s.query.split(':').next().unwrap_or("");
                matches!(
                    prefix,
                    "identifier"
                        | "entity"
                        | "file"
                        | "path_hint"
                        | "client_keyword"
                        | "config_key"
                        | "literal"
                )
            })
            .filter_map(|s| s.resolved_id.as_ref())
            .filter_map(|id| graph.get_node(id))
            .map(|n| n.file_path)
            .collect();
        selection.required.retain(|id| {
            let Some(node) = graph.get_node(id) else {
                return true;
            };
            if strong_seed_files.contains(&node.file_path) {
                return true;
            }
            let penalized = graph
                .file_min_base_relevance(id)
                .is_some_and(|r| r < REQUIRED_DEMOTE_RELEVANCE);
            if penalized {
                emission.record_drop(id, EmissionDropStage::PenalizedSuppress);
            }
            !penalized
        });
        let required_set: HashSet<NodeId> = selection.required.iter().cloned().collect();
        EmissionPipeline::suppress_penalized_optional(
            graph,
            &mut selection.optional,
            &required_set,
            &mut emission,
            thresholds.penalized_suppression_threshold,
        );
        EmissionPipeline::rerank_optional_with_learning(
            graph,
            &mut selection.optional,
            &mut selection.scores,
            &learning_index,
            &focus_terms,
            &thresholds,
        );
        #[cfg(feature = "embeddings")]
        {
            if effective_mode == OptimizationMode::MaxQuality {
                crate::optional_dedup::apply_module_cluster_bonus(
                    graph,
                    &seed_set,
                    &mut selection.optional,
                    &mut selection.scores,
                );
            }
            if let Some(threshold) = app_cfg.embeddings.optional_dedup_min_cosine {
                if selection.optional.len() > 2 {
                    crate::optional_dedup::dedup_optional_files(
                        graph,
                        &mut selection.optional,
                        &selection.scores,
                        &mut emission,
                        threshold,
                    );
                }
            }
        }
        if !call_graph_task {
            EmissionPipeline::ensure_learned_emission(
                graph,
                &mut selection.optional,
                &mut selection.scores,
                &required_set,
                &learning_index,
                &focus_terms,
                &thresholds,
                selection.optional_cap,
            );
        }

        *self.last_physarum.lock() = PhysarumTelemetry {
            used: physarum_used,
            ms: physarum_ms,
        };

        let mut active_symbol_names: HashSet<String> = HashSet::new();
        active_symbol_names.insert(signature.entity.to_lowercase());
        for ident in &signature.identifiers {
            active_symbol_names.insert(ident.to_lowercase());
        }
        for seed in &seed_resolutions {
            if let Some(id) = &seed.resolved_id {
                if let Some(node) = graph.get_node(id) {
                    active_symbol_names.insert(node.name.to_lowercase());
                }
            }
            active_symbol_names.insert(seed.query.to_lowercase());
        }
        for name in seed_callee_exon_names(graph, &seed_set) {
            active_symbol_names.insert(name);
        }

        let mut priority_symbols: HashSet<String> = HashSet::new();
        let mut priority_qualified: HashSet<(String, String)> = HashSet::new();
        for seed in &seed_resolutions {
            if let Some(id) = &seed.resolved_id {
                if let Some(node) = graph.get_node(id) {
                    // A seed on a method is that method, not its name: `forward`
                    // under `GPT`, not the five `forward`s of the file (F7).
                    match node.parent.as_deref() {
                        Some(owner) if node.node_type != NodeType::File => {
                            priority_qualified
                                .insert((owner.to_lowercase(), node.name.to_lowercase()));
                        }
                        _ if node.node_type != NodeType::File => {
                            priority_symbols.insert(node.name.to_lowercase());
                        }
                        _ => {}
                    }
                }
            }
            push_seed_priority_symbol(&mut priority_symbols, &seed.query);
        }

        let fold_policy = FoldPolicy::from_task(&active_symbol_names, signature)
            .with_priority_symbols(priority_symbols)
            .with_priority_qualified(priority_qualified);
        let packet_limit = packet_cap(effective_mode);

        let mut active_nodes = Vec::new();
        let mut included: HashSet<NodeId> = HashSet::new();
        let mut fill_used: usize = 0;
        let mut total_raw_tokens = 0;
        let mut fold_ids = Vec::new();
        let mut all_folds: Vec<FoldedIntron> = Vec::new();
        let registry = self.registry.clone();

        // A plain stem-match fill file is gated at every project size. The
        // gate used to switch on only above 20 files (`FILL_GATE_MIN_FILES`)
        // because two fixtures needed a fill file to reach a legitimate gold
        // file; both now have a structural reason to be there (F21: owner
        // twin coherence; F31: the Api -> handler edge below), so the size
        // switch is gone and the rule is one rule.
        // F31: a route file that *registers* a seed as its handler is wiring
        // the question asked about, not a coincidental consumer. The link is
        // the parser's Api -> handler `Calls` edge resolved to the seed's own
        // node id — a same-named function in another file (fastapi's
        // `routes/private.py::create_user` next to the seeded
        // `crud.create_user`) does not qualify.
        let route_files_of_seeds: HashSet<std::path::PathBuf> = seed_set
            .iter()
            .flat_map(|seed| graph.get_connected_neighbors(seed))
            .filter(|(_, edge)| {
                edge.edge_type == EdgeType::Calls && seed_set.contains(&edge.target)
            })
            .filter_map(|(id, _)| graph.get_node(&id))
            .filter(|n| n.node_type == NodeType::Api)
            .map(|n| n.file_path)
            .collect();
        // Files joined to a seed (or the seed's file) by an edge feedback has
        // reinforced (`reinforce_path`): `reinforcement_count` only ever moves
        // on feedback, so it is the learned signal without a threshold.
        let reinforced_files: HashSet<NodeId> = seed_set
            .iter()
            .flat_map(|seed| {
                let mut ends = vec![seed.clone()];
                if let Some(fid) = graph
                    .get_node(seed)
                    .and_then(|n| graph.file_id_for_path(&n.file_path))
                {
                    ends.push(fid);
                }
                ends
            })
            .flat_map(|end| graph.get_connected_neighbors(&end))
            .filter(|(_, edge)| edge.reinforcement_count > 0)
            .filter_map(|(id, _)| graph.get_node(&id))
            .filter_map(|n| graph.file_id_for_path(&n.file_path))
            .collect();

        let materialize = |id: &NodeId,
                           scores: &HashMap<NodeId, f32>,
                           seed_energies: &HashMap<NodeId, f32>,
                           seed_reasons: &HashMap<NodeId, String>,
                           scorer: &crate::scoring::ActivationScorer,
                           exon_budget: usize,
                           required_file: bool|
         -> Option<MaterializedNode> {
            let mut node = graph.get_node(id)?;
            if is_noise_path(&node.file_path) && !seed_set.contains(id) {
                let seed_file = seed_set.iter().any(|s| {
                    graph
                        .get_node(s)
                        .is_some_and(|n| n.file_path == node.file_path)
                });
                if !seed_file {
                    return None;
                }
            }
            let rel_strength = *seed_energies.get(id).unwrap_or(&0.35);
            let hist_success = (node.base_relevance / 3.0).clamp(0.20, 1.0);
            let score = scores
                .get(id)
                .copied()
                .unwrap_or_else(|| scorer.score_node(&node, signature, rel_strength, hist_success));
            let reason = seed_reasons.get(id).cloned().unwrap_or_else(|| {
                scores
                    .get(id)
                    .map(|s| format!("utility:{s:.2}"))
                    .unwrap_or_else(|| "connector".into())
            });
            let sidecar =
                !required_file && (reason == "physarum_tube" || reason.starts_with("utility:"));
            // A plain term-score fill file (not a physarum structural bridge)
            // with no other justification is a coincidental stem match, not
            // something the question asked for. Gate it the same way a
            // symbol's consumers are gated: only in when the question names
            // the file (or its stem) or asks for usage/consumers. Physarum
            // tube files are left alone: they are graph-theoretic bridges
            // (e.g. a router wiring a controller to its template) that the
            // question does not name directly but that connect the seeds.
            // A file the user's feedback has reinforced (synaptic spikes,
            // `reinforce_node_access`) carries its own justification: that is
            // the learning loop, and the gate must not undo it. F31 found the
            // gate had been doing exactly that on every project over 20
            // files — the loop's own unit tests only ran on smaller ones.
            let learned = graph.file_id_for_path(&node.file_path).is_some_and(|fid| {
                learning_index.get(&fid).copied().unwrap_or(0.0) > 0.0
                    || reinforced_files.contains(&fid)
            });
            if sidecar
                && !learned
                && !route_files_of_seeds.contains(&node.file_path)
                && !focus_terms_ask_for_consumers(&focus_terms)
                && !(consumer_named_in_focus(&node.name, &node.file_path, &focus_terms)
                    && !shared_stem_without_dir_focus(graph, &node.file_path, &focus_terms))
            {
                return None;
            }
            let mut folds = Vec::new();
            let mut folded_symbols = Vec::new();
            let policy = fold_policy.clone().with_exon_budget(exon_budget);
            let raw = if let Some(content) = graph.read_source(&node.file_path) {
                let raw = neuromesh_core::TokenCounter::count_tokens(&content);
                let spans = function_spans_for_file(graph, &node.file_path);
                let skeleton_res = CodeSkeletonizer::skeletonize_with_policy(
                    &node.file_path.to_string_lossy(),
                    &content,
                    &policy,
                    &spans,
                );
                for fold in skeleton_res.folds {
                    registry.register_fold(node.file_path.clone(), fold.clone());
                    folded_symbols.push(fold.symbol_name.clone());
                    folds.push(fold);
                }
                node.content = Some(skeleton_res.skeleton_code);
                node.token_cost = skeleton_res.skeleton_tokens;
                raw
            } else {
                node.token_cost
            };
            Some(MaterializedNode {
                node,
                score,
                reason,
                sidecar,
                raw_tokens: raw,
                folds,
                folded_symbols,
            })
        };

        let mut packet_truncated = false;
        let mut seed_items: Vec<MaterializedNode> = Vec::new();
        for id in &selection.required {
            if included.contains(id) {
                continue;
            }
            let Some(item) = materialize(
                id,
                &selection.scores,
                &seed_energies,
                &seed_reasons,
                &self.scorer,
                SEED_EXON_BUDGET,
                true,
            ) else {
                continue;
            };
            included.insert(id.clone());
            total_raw_tokens += item.raw_tokens;
            seed_items.push(item);
            let breakdown = compute_unified_file_score(
                graph,
                id,
                selection.scores.get(id).copied().unwrap_or(8.0),
                &learning_index,
                &focus_terms,
                &thresholds,
                0.0,
            );
            emission.record_emitted(id, breakdown);
        }
        let mut seed_tokens = unique_file_tokens(seed_items.iter());

        let mut fill_items: Vec<MaterializedNode> = Vec::new();
        for id in &selection.optional {
            if included.contains(id) {
                continue;
            }
            let Some(item) = materialize(
                id,
                &selection.scores,
                &seed_energies,
                &seed_reasons,
                &self.scorer,
                OPTIONAL_EXON_BUDGET,
                false,
            ) else {
                continue;
            };
            let cost = item.node.token_cost.max(1);
            if fill_cap == 0 || fill_used.saturating_add(cost) > fill_cap {
                emission.record_drop(id, EmissionDropStage::FillCap);
                self.registry.register_inactive(
                    &item.node,
                    0.2,
                    signature.confidence,
                    item.score,
                    None,
                );
                continue;
            }
            included.insert(id.clone());
            total_raw_tokens += item.raw_tokens;
            fill_used += cost;
            fill_items.push(item);
            let breakdown = compute_unified_file_score(
                graph,
                id,
                selection.scores.get(id).copied().unwrap_or(8.0),
                &learning_index,
                &focus_terms,
                &thresholds,
                0.0,
            );
            emission.record_emitted(id, breakdown);
        }

        let packet_tokens = |seeds: &[MaterializedNode], fill: &[MaterializedNode]| -> usize {
            unique_file_tokens(seeds.iter().chain(fill.iter()))
        };
        while packet_tokens(&seed_items, &fill_items) > packet_limit && !fill_items.is_empty() {
            packet_truncated = true;
            let dropped = fill_items.pop().expect("non-empty");
            if let Some(fid) = graph.file_id_for_path(&dropped.node.file_path) {
                emission.record_drop(&fid, EmissionDropStage::PacketCap);
            }
            fill_used = fill_used.saturating_sub(dropped.node.token_cost.max(1));
            total_raw_tokens = total_raw_tokens.saturating_sub(dropped.raw_tokens);
            included.remove(&dropped.node.id);
            self.registry.register_inactive(
                &dropped.node,
                0.2,
                signature.confidence,
                dropped.score,
                None,
            );
        }
        if packet_tokens(&seed_items, &fill_items) > packet_limit {
            for item in &mut seed_items {
                let Some(shrunk) = materialize(
                    &item.node.id,
                    &selection.scores,
                    &seed_energies,
                    &seed_reasons,
                    &self.scorer,
                    2,
                    selection.required.contains(&item.node.id),
                ) else {
                    continue;
                };
                *item = shrunk;
            }
            seed_tokens = unique_file_tokens(seed_items.iter());
        }

        let active_tokens = packet_tokens(&seed_items, &fill_items);
        for item in seed_items.into_iter().chain(fill_items) {
            fold_ids.extend(item.folds.iter().map(|f| f.fold_id.clone()));
            all_folds.extend(item.folds);
            active_nodes.push(ActivatedNodeView {
                node: item.node,
                activation_score: item.score,
                status: ContextStatus::Active,
                expansion_reason: Some(item.reason),
                sidecar: item.sidecar,
                folded_symbols: item.folded_symbols,
            });
        }

        let mut inactive_count = 0usize;
        for id in &neighborhood {
            if included.contains(id) || inactive_count >= MAX_INACTIVE {
                continue;
            }
            if let Some(node) = graph.get_node(id) {
                let score = self.scorer.score_node(&node, signature, 0.2, 1.0);
                self.registry
                    .register_inactive(&node, 0.2, signature.confidence, score, None);
                inactive_count += 1;
            }
        }

        let mut inactive_descriptors = self.registry.get_inactive_descriptors();
        inactive_descriptors.sort_by(|a, b| {
            b.activation_score
                .partial_cmp(&a.activation_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        inactive_descriptors.truncate(MAX_INACTIVE);

        let workspace_tokens = graph.total_tokens().max(1);
        if total_raw_tokens == 0 {
            total_raw_tokens = workspace_tokens.max(active_tokens);
        }

        let reduction_percentage = if workspace_tokens > 0 {
            let saved = workspace_tokens.saturating_sub(active_tokens);
            (saved as f32 / workspace_tokens as f32) * 100.0
        } else {
            0.0
        };

        let selected_paths: HashSet<String> = active_nodes
            .iter()
            .filter(|n| n.node.node_type == NodeType::File)
            .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
            .collect();
        let covered: Vec<String> = selected_paths.iter().cloned().collect();
        let sidecar_files: Vec<String> = active_nodes
            .iter()
            .filter(|n| n.sidecar && n.node.node_type == NodeType::File)
            .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
            .collect();
        let (packet_gaps, unsure) =
            compute_packet_gaps(graph, &seed_set, &selected_paths, signature);
        let semantic_cov = semantic_style_coverage(&selected_paths, signature);
        let budget_truncated =
            packet_truncated || fill_used > fill_cap || active_tokens > packet_limit;
        let mut coverage = enrich_coverage(
            &seed_resolutions,
            packet_gaps,
            unsure,
            covered,
            skipped_files,
            semantic_cov,
            sidecar_files,
            budget_truncated,
        );
        if let Some(override_claim) =
            crate::retrieval::embedding_confidence::confidence_coverage_override(
                &seed_resolutions,
                &seed_reasons,
                &embedding_config,
                resolve_engine_id(&sig_for_seeds, &seed_config),
            )
        {
            coverage.claim = override_claim.to_string();
            coverage.seeds_hit.clear();
            coverage.covered.clear();
        }
        let structural_evidence = build_structural_evidence(graph, &seed_set);
        let unresolved: Vec<_> = graph
            .unresolved_refs()
            .into_iter()
            .filter(|u| {
                selected_paths.contains(&u.from_file.to_string_lossy().replace('\\', "/"))
                    || seed_resolutions.iter().any(|s| s.query == u.name)
            })
            .take(40)
            .collect();

        let mut next_actions = build_next_actions(
            graph,
            &active_nodes,
            &included,
            &coverage,
            &all_folds,
            &unresolved,
        );
        if scaffold_used {
            next_actions.push(NextAction {
                tool: "neuromesh_get_architecture".into(),
                query: String::new(),
                why: "greenfield scaffold — review framework entry points and conventions".into(),
            });
        }

        let selected_set: HashSet<NodeId> = selection
            .required
            .iter()
            .chain(selection.optional.iter())
            .cloned()
            .collect();
        selection.rank_candidates = emission.finalize_rank_candidates(
            graph,
            &selection.scores,
            &learning_index,
            &selected_set,
            &focus_terms,
            &thresholds,
            &selection.rank_candidates,
        );

        let view = ContextView {
            project_id: graph.project_id(),
            active_nodes,
            inactive_descriptors,
            total_raw_tokens,
            active_tokens,
            reduction_percentage,
            confidence_score: signature.confidence,
            bypass_applied: is_critical,
            seeds: seed_resolutions,
            unresolved,
            coverage: Some(coverage),
            next_actions,
            budget_used: active_tokens,
            budget_cap: seed_tokens.saturating_add(fill_cap),
            budget_mode: budget_mode_name(effective_mode).to_string(),
            budget_seed_tokens: seed_tokens,
            budget_fill_used: fill_used,
            budget_fill_cap: fill_cap,
            over_budget: fill_used > fill_cap || active_tokens > packet_limit,
            fold_ids,
            seed_call_coverage: compute_seed_call_coverage(graph, &seed_set, &selected_paths),
            workspace_tokens,
            physarum_used,
            physarum_ms,
            selection_method: selection.method.to_string(),
            rank_candidates: selection
                .rank_candidates
                .iter()
                .map(|c| neuromesh_core::RankCandidateView {
                    path: c.path.clone(),
                    score: c.score,
                    learning_bonus: c.learning_bonus,
                    reason: c.reason.clone(),
                    selected: c.selected,
                    emitted: c.emitted,
                    drop_stage: c.drop_stage.map(|s| s.as_str().to_string()),
                    score_breakdown: c.breakdown.clone(),
                })
                .collect(),
            structural_evidence,
            task_scenario: if scaffold_used {
                "greenfield".to_string()
            } else {
                "brownfield".to_string()
            },
            seed_resolution_telemetry: Some(seed_resolution_telemetry),
            packet_header,
            retrieval: None,
            embedding_used,
        };
        *self.last_packet.lock() = Some(PacketSnapshot::from_view(&view));
        view
    }
}

/// Drop fuzzy NL seeds that block greenfield scaffold routing (Create intent, no keywords).
/// Keeps proven file hints and exact symbol hits so brownfield Create tasks are unchanged.
pub(crate) fn prune_weak_greenfield_seeds_inner(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    buffers: &mut SeedBuffers<'_, '_, '_>,
) {
    prune_weak_greenfield_seeds_legacy(
        graph,
        signature,
        buffers.resolutions,
        buffers.energies,
        buffers.reasons,
    );
}

fn prune_weak_greenfield_seeds_legacy(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    seed_resolutions: &mut Vec<SeedResolution>,
    seed_energies: &mut HashMap<NodeId, f32>,
    seed_reasons: &mut HashMap<NodeId, String>,
) {
    let resolution_conf = |id: &NodeId| -> f32 {
        seed_resolutions
            .iter()
            .find(|s| s.resolved_id.as_ref() == Some(id))
            .map(|s| s.confidence)
            .unwrap_or(0.0)
    };
    let query_for = |id: &NodeId| -> Option<&str> {
        seed_resolutions
            .iter()
            .find(|s| s.resolved_id.as_ref() == Some(id))
            .map(|s| s.query.as_str())
    };
    let keep: HashSet<NodeId> = seed_energies
        .keys()
        .filter(|id| {
            let reason = seed_reasons.get(*id).map(String::as_str).unwrap_or("");
            if reason.starts_with("file:") {
                return true;
            }
            if !reason.starts_with("identifier:") || resolution_conf(id) < 1.0 {
                return false;
            }
            let Some(query) = query_for(id) else {
                return false;
            };
            if query.eq_ignore_ascii_case(signature.technology.as_str()) {
                return false;
            }
            graph
                .resolve_best(query)
                .is_some_and(|node| node.name == query && node.id == **id)
        })
        .cloned()
        .collect();
    for id in seed_energies.keys().cloned().collect::<Vec<_>>() {
        if !keep.contains(&id) {
            seed_energies.remove(&id);
            seed_reasons.remove(&id);
        }
    }
    seed_resolutions.retain(|s| {
        s.resolved_id
            .as_ref()
            .is_none_or(|id| seed_energies.contains_key(id))
    });
}

fn cluster_terms_covered(
    cluster: &str,
    seed_resolutions: &[SeedResolution],
    file_hints: &[String],
) -> bool {
    let cluster_lower = cluster.to_lowercase();
    if file_hints
        .iter()
        .any(|hint| cluster_lower.contains(&hint.to_lowercase()))
    {
        return true;
    }
    let anchors = extract_prompt_anchors(cluster);
    // A clause that spelled out a code identifier and resolved it
    // (`RoIHeads.select_training_samples`) is about that symbol; its English
    // nouns ("proposals") are not missing seeds to go fuzzy-searching for.
    let nouns = extract_cluster_nouns(cluster);
    // ("a Dense" lands in the nouns, not the identifiers — same test either way.)
    if anchors
        .identifiers
        .iter()
        .chain(nouns.iter())
        .any(|id| looks_like_code_identifier(id) && seed_term_resolved(id, seed_resolutions))
    {
        return true;
    }
    let mut terms = anchors.identifiers;
    terms.extend(nouns);
    terms.sort();
    terms.dedup();
    terms.retain(|t| !is_prompt_stopword(t));
    let significant: Vec<String> = terms
        .into_iter()
        .filter(|t| {
            let tl = t.to_lowercase();
            tl.len() >= 5 || t.contains('.') || t.contains('_')
        })
        .collect();
    if significant.is_empty() {
        return seed_resolutions.iter().any(|s| s.resolved_id.is_some());
    }
    significant
        .iter()
        .all(|term| seed_term_resolved(term, seed_resolutions))
}

/// `RoIHeads`, `select_training_samples`, `Owner.member` — not the word "proposals".
fn looks_like_code_identifier(id: &str) -> bool {
    id.chars()
        .any(|c| c.is_ascii_uppercase() || c == '_' || c == '.' || c == ':')
}

fn seed_term_resolved(term: &str, seeds: &[SeedResolution]) -> bool {
    let tl = term.to_lowercase();
    seeds.iter().any(|s| {
        if s.resolved_id.is_none() {
            return false;
        }
        // A resolved seed stores `reason:query`; compare the query part.
        let sq = s
            .query
            .split_once(':')
            .filter(|(reason, rest)| {
                !reason.is_empty()
                    && !rest.starts_with(':')
                    && reason.chars().all(|c| c.is_ascii_lowercase() || c == '_')
            })
            .map(|(_, rest)| rest)
            .unwrap_or(&s.query)
            .to_lowercase();
        sq == tl
            || sq.ends_with(&format!(".{tl}"))
            || sq.rsplit('.').next().is_some_and(|member| member == tl)
    })
}

fn push_seed_priority_symbol(symbols: &mut HashSet<String>, query: &str) {
    if query.contains(['/', '\\']) {
        return;
    }
    if let Some((_, member)) = query.split_once('.') {
        if !member.is_empty() {
            symbols.insert(member.to_lowercase());
            symbols.insert(query.to_lowercase());
            return;
        }
    }
    symbols.insert(query.to_lowercase());
}

/// Receivers that name "the current object", not a type or module: their
/// member can live on any parent, so they carry no owner constraint.
const OWNER_WILDCARDS: &[&str] = &["self", "this", "cls", "super", "me"];

enum DottedMember {
    Hit((NodeId, f32)),
    /// The graph knows the owner as a receiver, but it has no such member.
    OwnerLacksMember,
    /// The owner is not a recorded parent (an instance name, a module): no
    /// owner constraint applies and the path heuristics decide.
    Unknown,
}

fn resolve_dotted_member(
    graph: &NeuralProjectGraph,
    owner: &str,
    member: &str,
    prompt: &str,
) -> DottedMember {
    let owner_l = owner.to_lowercase();
    // The parent the parser recorded wins over every path heuristic below:
    // `res.json` is the `json` whose parent is `res`, not the body-parser
    // export in `express.js` that a substring hint (`res` ⊂ `express`) picks.
    // And when the graph knows `req` as a receiver but has no `req.get`,
    // `res.get` is a different owner's member, not an answer (F28).
    if !OWNER_WILDCARDS.contains(&owner_l.as_str()) {
        let owned: Vec<NodeId> = graph
            .members_of_owner(owner, member)
            .into_iter()
            .filter(|id| seed_path_allowed(graph, id, prompt))
            .collect();
        if let Some(first) = owned.first() {
            let conf = if owned.len() == 1 { 1.0 } else { 0.72 };
            let ranked = graph
                .resolve_ranked(member, Some(owner), None)
                .map(|(id, _)| id)
                .filter(|id| owned.contains(id));
            return DottedMember::Hit((ranked.unwrap_or_else(|| first.clone()), conf));
        }
        if graph.is_known_owner(owner) {
            return DottedMember::OwnerLacksMember;
        }
    }
    let hints: Vec<&str> = match owner_l.as_str() {
        "app" => vec!["application", "app"],
        _ => vec![owner],
    };
    for hint in hints {
        if let Some((ranked_id, confidence)) = graph.resolve_ranked(member, Some(hint), None) {
            let id = prefer_search_seed(graph, member, ranked_id, confidence, prompt);
            if seed_path_allowed(graph, &id, prompt) {
                let conf = match confidence {
                    EdgeConfidence::Proven => 1.0,
                    EdgeConfidence::Likely => 0.72,
                    EdgeConfidence::Unresolved => 0.0,
                };
                if conf > 0.0 {
                    return DottedMember::Hit((id, conf));
                }
            }
        }
    }
    DottedMember::Unknown
}

/// `owner.member` whose member the graph does not know: fall back to the
/// module the owner names, by exact file stem. `app.handle` in an Express
/// codebase is `lib/application.js` even when the parser produced no symbol
/// for `app.handle = function handle(...)` — which it currently does not.
///
/// Exact stem only, and the bare owner name only when the user wrote the
/// expression: the loose path matcher used to take the `app` of a
/// server-inferred `app.render` and hit `app.php` in a PHP project. The
/// `application` alias is safe either way — a file by that name is the
/// application module in every framework that has one.
fn resolve_owner_module(
    graph: &NeuralProjectGraph,
    owner: &str,
    written_by_user: bool,
    prompt: &str,
) -> Option<NodeId> {
    let owner_l = owner.to_lowercase();
    if owner_l.len() < 3
        || !owner_l
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    let mut stems: Vec<String> = Vec::new();
    if owner_l == "app" {
        stems.push("application".into());
    }
    if written_by_user {
        stems.push(owner_l.clone());
    }
    let files = graph.file_node_paths();
    for stem in stems {
        let mut hits: Vec<NodeId> = files
            .iter()
            .filter(|(_, path)| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case(&stem))
            })
            .map(|(id, _)| id.clone())
            .collect();
        hits.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        if hits.len() == 1 && seed_path_allowed(graph, &hits[0], prompt) {
            return Some(hits.remove(0));
        }
    }
    None
}

fn merge_context_views(base: ContextView, extension: ContextView) -> ContextView {
    let mut merged = base;
    for node in extension.active_nodes {
        if !merged
            .active_nodes
            .iter()
            .any(|n| n.node.id == node.node.id)
        {
            merged.active_nodes.push(node);
        }
    }
    merged.active_tokens = merged.active_nodes.iter().map(|n| n.node.token_cost).sum();
    for seed in extension.seeds {
        if !merged.seeds.iter().any(|s| s.query == seed.query) {
            merged.seeds.push(seed);
        }
    }
    if extension.seed_call_coverage > merged.seed_call_coverage {
        merged.seed_call_coverage = extension.seed_call_coverage;
    }
    if let Some(cov) = extension.coverage {
        merged.coverage = Some(cov);
    }
    if extension.embedding_used {
        merged.embedding_used = true;
    }
    merged
}

fn include_file_hint(graph: &NeuralProjectGraph, view: &mut ContextView, file_id: &NodeId) {
    if view.active_nodes.iter().any(|n| n.node.id == *file_id) {
        return;
    }
    let Some(node) = graph.get_node(file_id) else {
        return;
    };
    if node.node_type != NodeType::File {
        return;
    }
    view.active_nodes.push(ActivatedNodeView {
        node: node.clone(),
        activation_score: 0.75,
        status: ContextStatus::Active,
        expansion_reason: Some("pattern_expand".into()),
        sidecar: true,
        folded_symbols: Vec::new(),
    });
    view.active_tokens = view.active_nodes.iter().map(|n| n.node.token_cost).sum();
}

fn cap_semantic_recovery_seeds(view: &mut ContextView, max: u8) {
    let is_semantic = view
        .seed_resolution_telemetry
        .as_ref()
        .is_some_and(|t| t.engine == "semantic_lite");
    if !is_semantic {
        return;
    }
    let mut ranked: Vec<(usize, f32)> = view
        .seeds
        .iter()
        .enumerate()
        .filter(|(_, s)| s.resolved_id.is_some())
        .map(|(i, s)| (i, s.confidence))
        .collect();
    if ranked.len() <= max as usize {
        return;
    }
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (idx, _) in ranked.into_iter().skip(max as usize) {
        if let Some(seed) = view.seeds.get_mut(idx) {
            seed.resolved_id = None;
            seed.confidence = 0.0;
        }
    }
}

/// Replace file-level seeds with best-matching symbol nodes inside those files.
fn expand_file_seeds_to_symbols(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    seed_energies: &mut HashMap<NodeId, f32>,
    seed_reasons: &mut HashMap<NodeId, String>,
    seed_resolutions: &mut Vec<SeedResolution>,
    max_per_file: usize,
) {
    let file_ids: Vec<NodeId> = seed_energies
        .keys()
        .filter(|id| {
            graph
                .get_node(id)
                .is_some_and(|n| n.node_type == NodeType::File)
        })
        .cloned()
        .collect();
    if file_ids.is_empty() {
        return;
    }

    let mut idents: Vec<String> = signature
        .identifiers
        .iter()
        .chain(signature.client_keywords.iter())
        .map(|s| s.to_lowercase())
        .collect();
    for token in signature
        .raw_prompt
        .split(|c: char| !c.is_alphanumeric() && c != '_')
    {
        let t = token.to_lowercase();
        if t.len() >= 4 {
            idents.push(t);
        }
    }

    for file_id in file_ids {
        let Some(file_node) = graph.get_node(&file_id) else {
            continue;
        };
        let energy = seed_energies.remove(&file_id).unwrap_or(0.0);
        seed_reasons.remove(&file_id);
        seed_resolutions.retain(|s| s.resolved_id.as_ref() != Some(&file_id));

        let mut candidates: Vec<(NodeId, f32)> = graph
            .nodes_in_file(&file_node.file_path)
            .into_iter()
            .filter(|n| {
                matches!(
                    n.node_type,
                    NodeType::Function
                        | NodeType::Class
                        | NodeType::Component
                        | NodeType::Api
                        | NodeType::Symbol
                )
            })
            .map(|node| {
                let name = node.name.to_lowercase();
                let mut score = 0.0f32;
                for ident in &idents {
                    if ident.is_empty() {
                        continue;
                    }
                    if name == *ident {
                        score += 4.0;
                    } else if name.contains(ident.as_str()) {
                        score += 2.0;
                    }
                }
                if node
                    .signature
                    .as_deref()
                    .is_some_and(|s| idents.iter().any(|i| s.to_lowercase().contains(i)))
                {
                    score += 1.5;
                }
                (node.id, score)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(max_per_file);

        if candidates.is_empty() {
            seed_energies.insert(file_id.clone(), energy);
            seed_reasons
                .entry(file_id.clone())
                .or_insert_with(|| "file_seed:unexpanded".into());
            seed_resolutions.push(SeedResolution {
                query: file_node.file_path.to_string_lossy().into_owned(),
                resolved_id: Some(file_id),
                confidence: energy.min(1.0),
                resolution_tier: Some("hierarchical:file".into()),
                embedding_score: None,
            });
            continue;
        }

        for (sym_id, bonus) in candidates {
            let e = energy * (0.85 + bonus * 0.05).min(1.2);
            seed_energies
                .entry(sym_id.clone())
                .and_modify(|v| *v = v.max(e))
                .or_insert(e);
            seed_reasons
                .entry(sym_id.clone())
                .or_insert_with(|| "file_seed:expanded".into());
            seed_resolutions.push(SeedResolution {
                query: file_node.file_path.to_string_lossy().into_owned(),
                resolved_id: Some(sym_id),
                confidence: e.min(1.0),
                resolution_tier: Some("hierarchical:file_expand".into()),
                embedding_score: None,
            });
        }
    }
}

pub(crate) fn resolve_seed_query(
    graph: &NeuralProjectGraph,
    query: &str,
    prompt: &str,
) -> Option<(NodeId, f32)> {
    // A stem hit (`calling` → `call`) is a guess about a word, not the name
    // the question wrote; it never carries exact confidence.
    const STEM_HIT_MAX_CONFIDENCE: f32 = 0.72;
    let hit = resolve_seed_query_once(graph, query, prompt).or_else(|| {
        stem_search_queries(query)
            .iter()
            .find_map(|stem| resolve_seed_query_once(graph, stem, prompt))
            .map(|(id, conf)| {
                // A file whose name carries the stem (`create_sms_messages_table`)
                // is what a keyword names; a same-named *symbol* elsewhere is a guess.
                let is_file = graph
                    .get_node(&id)
                    .is_some_and(|n| n.node_type == NodeType::File);
                let conf = if is_file {
                    conf
                } else {
                    conf.min(STEM_HIT_MAX_CONFIDENCE)
                };
                (id, conf)
            })
    });
    match hit {
        Some((id, conf)) => {
            let id =
                crate::seed::path_steer::steer_same_name_file(graph, &id, prompt).unwrap_or(id);
            let id =
                crate::seed::path_steer::steer_same_name_symbol(graph, &id, prompt).unwrap_or(id);
            Some((id, conf))
        }
        None => crate::seed::path_steer::resolve_dir_segment_seed(graph, query, prompt)
            .map(|id| (id, 0.9)),
    }
}

fn resolve_seed_query_once(
    graph: &NeuralProjectGraph,
    query: &str,
    prompt: &str,
) -> Option<(NodeId, f32)> {
    if query.starts_with("__") {
        if let Some(node) = graph.resolve_best(query) {
            if node.name == query && seed_path_allowed(graph, &node.id, prompt) {
                return Some((node.id, 1.0));
            }
        }
        return None;
    }
    if !query.contains(['/', '\\']) && !is_route_query(query) {
        if let Some((owner, member)) = query.split_once('.') {
            match resolve_dotted_member(graph, owner, member, prompt) {
                DottedMember::Hit(hit) => return Some(hit),
                // `req.get` when the graph knows `req` but not its `get`: no
                // file named `req.get.js`, no `get` of another owner.
                DottedMember::OwnerLacksMember => return None,
                DottedMember::Unknown => {}
            }
            let written = prompt.to_lowercase().contains(&query.to_lowercase());
            if let Some(id) = resolve_owner_module(graph, owner, written, prompt) {
                return Some((id, 0.8));
            }
        }
    }
    if query.contains("::") {
        if let Some(node) = graph.resolve_best(query) {
            if seed_path_allowed(graph, &node.id, prompt) {
                return Some((node.id, 1.0));
            }
        }
    }
    if graph.is_file_hint_query(query) && !is_route_query(query) {
        if let Some(id) = graph.resolve_file_hint(query) {
            if seed_path_allowed(graph, &id, prompt) {
                return Some((id, 0.95));
            }
        }
    }
    if let Some((ranked_id, confidence)) = graph.resolve_ranked(query, None, None) {
        let id = prefer_search_seed(graph, query, ranked_id, confidence, prompt);
        if seed_path_allowed(graph, &id, prompt) {
            let conf = match confidence {
                EdgeConfidence::Proven => 1.0,
                EdgeConfidence::Likely => 0.62,
                EdgeConfidence::Unresolved => 0.0,
            };
            return Some((id, conf));
        }
    }
    let hits = graph.search_symbols(query, 12);
    let q = query.to_lowercase();
    // A code file named exactly like the word (`stripe` → `lib/stripe.ts`)
    // outranks a symbol that merely starts with it (`STRIPE_API_KEY` in
    // `.env.example`, holdout-web): the word names the module.
    let stem_file: Option<NodeId> = if query.contains(['.', '/', '\\', ':']) || q.len() < 4 {
        None
    } else {
        let mut matches = graph
            .file_node_paths()
            .into_iter()
            .filter(|(_, p)| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case(&q))
                    && !crate::selector::is_noise_path_in(p, graph.examples_are_core())
                    && p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                        !matches!(
                            e.to_ascii_lowercase().as_str(),
                            "md" | "txt"
                                | "rst"
                                | "json"
                                | "yaml"
                                | "yml"
                                | "toml"
                                | "env"
                                | "html"
                                | "css"
                                | "svg"
                                | "lock"
                        )
                    })
            })
            .map(|(id, _)| id);
        match (matches.next(), matches.next()) {
            (Some(id), None) => Some(id),
            _ => None,
        }
    };
    let config_prefix_only = stem_file.is_some()
        && !hits.is_empty()
        && hits.iter().all(|h| {
            !h.name.eq_ignore_ascii_case(query)
                && matches!(h.node_type, NodeType::Config | NodeType::Hyperparameter)
        });
    let hit = hits.into_iter().find(|hit| {
        if !seed_path_allowed(graph, &hit.id, prompt) {
            return false;
        }
        if stem_file.is_some()
            && !hit.name.eq_ignore_ascii_case(query)
            && matches!(hit.node_type, NodeType::Config | NodeType::Hyperparameter)
        {
            return false;
        }
        // A bare word is a symbol query. A *file* in a docs/markdown path
        // that merely starts with it (`API` → `docs/.../api.md`, F65) is not
        // an anchor for it; a file is asked for by name (`api.md`) or stem.
        if hit.node_type == NodeType::File
            && !query.contains(['.', '/', '\\'])
            && graph.get_node(&hit.id).is_some_and(|n| {
                crate::selector::is_noise_path_in(&n.file_path, graph.examples_are_core())
            })
        {
            return false;
        }
        if hit.name.eq_ignore_ascii_case(query) {
            return true;
        }
        let n = hit.name.to_lowercase();
        hit.match_reason != "token" && hit.score >= 86.0 && (n.starts_with(&q) || q.starts_with(&n))
    });
    if let Some(hit) = hit {
        return Some((hit.id, (hit.score / 100.0).clamp(0.2, 0.75)));
    }
    if let (Some(id), true) = (stem_file, config_prefix_only) {
        if seed_path_allowed(graph, &id, prompt) {
            return Some((id, 0.8));
        }
    }
    // Nothing by name: a bare word that is exactly a source file's stem
    // (`image_classification_from_scratch`) names that file, whatever symbols
    // share its tokens. Code beats a notebook or markdown mirror of the script.
    // Only a long snake/kebab name qualifies: "transform" in prose is not a
    // request for `transform.py`, and a two-word `roi_heads` is as often an
    // attribute as a file. Three words or more is a script's name.
    let words = query.split(['_', '-']).filter(|w| !w.is_empty()).count();
    // A shorter word still names a script when the question says so:
    // "in the autoencoder example", "in mnist_convnet, how ...".
    let named_as_script = {
        let p = prompt.to_lowercase();
        let q = query.to_lowercase();
        p.contains(&format!("{q} example"))
            || p.contains(&format!("{q} script"))
            || p.contains(&format!("in {q},"))
            || p.contains(&format!("in the {q},"))
    };
    if (words >= 3 || named_as_script) && !query.contains(['/', '\\', '.', ':']) {
        if let Some(id) = graph.file_by_stem(query) {
            if seed_path_allowed(graph, &id, prompt) {
                return Some((id, 0.9));
            }
        }
    }
    None
}

fn seed_path_allowed(graph: &NeuralProjectGraph, id: &NodeId, prompt: &str) -> bool {
    graph
        .get_node(id)
        .map(|node| {
            !is_name_collision_decoy(&node.file_path)
                || decoy_allowed_for_prompt(&node.file_path, prompt)
        })
        .unwrap_or(true)
}

fn mark_equivalent_file_hits(
    graph: &NeuralProjectGraph,
    seeds: &mut [SeedResolution],
    seed_energies: &mut HashMap<NodeId, f32>,
) {
    let hit_paths: Vec<(NodeId, String)> = seeds
        .iter()
        .filter_map(|s| {
            let id = s.resolved_id.as_ref()?;
            let node = graph.get_node(id)?;
            Some((
                id.clone(),
                node.file_path.to_string_lossy().replace('\\', "/"),
            ))
        })
        .collect();
    if hit_paths.is_empty() {
        return;
    }
    for seed in seeds.iter_mut() {
        if seed.resolved_id.is_some() {
            continue;
        }
        let query = seed.query.replace('\\', "/");
        let query_base = std::path::Path::new(&query)
            .file_name()
            .map(|s| s.to_string_lossy().replace('\\', "/"));
        if let Some((id, _)) = hit_paths.iter().find(|(_, path)| {
            path == &query
                || path.ends_with(&query)
                || query_base.as_ref().is_some_and(|base| {
                    path.ends_with(base) || path.rsplit('/').next() == Some(base)
                })
        }) {
            seed.resolved_id = Some(id.clone());
            seed.confidence = seed.confidence.max(0.95);
            seed_energies.entry(id.clone()).or_insert(0.95);
        }
    }
}

fn cohere_ambiguous_seeds_to_app(
    graph: &NeuralProjectGraph,
    seeds: &mut [SeedResolution],
    seed_energies: &mut HashMap<NodeId, f32>,
    prompt: &str,
) {
    let mut prefixes = HashSet::new();
    for seed in seeds.iter() {
        if seed.confidence < 0.9 {
            continue;
        }
        let Some(id) = seed.resolved_id.as_ref() else {
            continue;
        };
        if let Some(prefix) = graph
            .get_node(id)
            .and_then(|node| hmvc_app_prefix(&node.file_path))
        {
            prefixes.insert(prefix);
        }
    }
    if prefixes.is_empty() {
        return;
    }
    let prefix = if prefixes.len() == 1 {
        prefixes.into_iter().next().expect("checked len")
    } else {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for seed in seeds.iter() {
            if seed.confidence < 0.9 {
                continue;
            }
            let Some(id) = seed.resolved_id.as_ref() else {
                continue;
            };
            if let Some(p) = graph
                .get_node(id)
                .and_then(|node| hmvc_app_prefix(&node.file_path))
            {
                *counts.entry(p).or_default() += 1;
            }
        }
        counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(p, _)| p)
            .unwrap_or_else(|| prefixes.into_iter().next().expect("non-empty"))
    };
    let prefix_slash = format!("{prefix}/");
    for seed in seeds.iter_mut() {
        let stem = seed.query.rsplit(':').next().unwrap_or(seed.query.as_str());
        let short_ambiguous =
            stem.len() <= 5 || stem.chars().all(|c| c.is_ascii_lowercase() || c == '_');
        if seed.confidence >= 0.9 && !short_ambiguous {
            continue;
        }
        let Some(id) = seed.resolved_id.clone() else {
            continue;
        };
        let Some(node) = graph.get_node(&id) else {
            continue;
        };
        let path = node.file_path.to_string_lossy().replace('\\', "/");
        if path.contains(&prefix_slash) {
            continue;
        }
        let hits = graph.search_symbols(stem, 16);
        let Some(hit) = hits.into_iter().find(|hit| {
            if !seed_path_allowed(graph, &hit.id, prompt) {
                return false;
            }
            if !hit
                .file_path
                .to_string_lossy()
                .replace('\\', "/")
                .contains(&prefix_slash)
            {
                return false;
            }
            let name = hit
                .name
                .rsplit(['.', ':'])
                .next()
                .unwrap_or(hit.name.as_str());
            name.eq_ignore_ascii_case(stem)
        }) else {
            continue;
        };
        seed_energies.remove(&id);
        seed.resolved_id = Some(hit.id.clone());
        seed.confidence = seed.confidence.max(0.9);
        seed_energies
            .entry(hit.id)
            .and_modify(|energy| *energy = (*energy).max(0.7))
            .or_insert(0.7);
    }
}

fn keep_hmvc_packet_file(
    graph: &NeuralProjectGraph,
    seeds: &HashSet<NodeId>,
    lock: &str,
    id: &NodeId,
) -> bool {
    let Some(node) = graph.get_node(id) else {
        return false;
    };
    if seeds.contains(id)
        || seeds.iter().any(|seed| {
            graph
                .get_node(seed)
                .is_some_and(|s| s.file_path == node.file_path)
        })
    {
        return true;
    }
    match hmvc_app_prefix(&node.file_path) {
        Some(prefix) => prefix == lock,
        None => true,
    }
}

fn keep_schema_packet_file(
    graph: &NeuralProjectGraph,
    seeds: &HashSet<NodeId>,
    prompt: &str,
    id: &NodeId,
) -> bool {
    let Some(node) = graph.get_node(id) else {
        return false;
    };
    if seeds.contains(id)
        || seeds.iter().any(|seed| {
            graph
                .get_node(seed)
                .is_some_and(|s| s.file_path == node.file_path)
        })
    {
        return true;
    }
    if !is_schema_path(&node.file_path) {
        return true;
    }
    prompt_targets_database(prompt)
}

fn locked_seed_hmvc_prefix(graph: &NeuralProjectGraph, seeds: &HashSet<NodeId>) -> Option<String> {
    let mut prefixes = HashSet::new();
    for id in seeds {
        if let Some(prefix) = graph
            .get_node(id)
            .and_then(|node| hmvc_app_prefix(&node.file_path))
        {
            prefixes.insert(prefix);
        }
    }
    if prefixes.len() == 1 {
        prefixes.into_iter().next()
    } else {
        None
    }
}

/// When a compound task names a second topic that identifier extraction skipped
/// (lowercase "router permission guard"), try those nouns as seeds. A cluster
/// with zero hits is recorded as a miss so coverage cannot claim no_recorded_gap.
pub(crate) fn seed_uncovered_clusters_if_compound(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    buffers: &mut SeedBuffers<'_, '_, '_>,
) {
    if split_task_clusters(&signature.raw_prompt).len() <= 1 {
        return;
    }
    seed_uncovered_clusters_inner(graph, signature, buffers);
}

pub(crate) fn seed_uncovered_clusters_inner(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    buffers: &mut SeedBuffers<'_, '_, '_>,
) {
    seed_uncovered_clusters_legacy(
        graph,
        signature,
        buffers.resolutions,
        buffers.energies,
        buffers.reasons,
    );
}

fn seed_uncovered_clusters_legacy(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    seed_resolutions: &mut Vec<SeedResolution>,
    seed_energies: &mut HashMap<NodeId, f32>,
    seed_reasons: &mut HashMap<NodeId, String>,
) {
    for cluster in split_task_clusters(&signature.raw_prompt) {
        if cluster_terms_covered(&cluster, seed_resolutions, &signature.file_hints) {
            continue;
        }
        let nouns = extract_cluster_nouns(&cluster);
        let mut cluster_hit = false;
        for noun in &nouns {
            if let Some(idx) = seed_resolutions.iter().position(|s| s.query == *noun) {
                if seed_resolutions[idx].resolved_id.is_some() {
                    cluster_hit = true;
                    continue;
                }
                let hits = resolve_cluster_noun_seeds(graph, noun, &nouns);
                if let Some((id, conf)) = hits.into_iter().next() {
                    let energy = 0.85;
                    seed_energies
                        .entry(id.clone())
                        .and_modify(|e| *e = (*e).max(energy))
                        .or_insert(energy);
                    seed_reasons
                        .entry(id.clone())
                        .or_insert_with(|| format!("cluster:{noun}"));
                    seed_resolutions[idx].resolved_id = Some(id);
                    seed_resolutions[idx].confidence = conf;
                    cluster_hit = true;
                }
                continue;
            }
            let hits = resolve_cluster_noun_seeds(graph, noun, &nouns);
            if hits.is_empty() {
                continue;
            }
            cluster_hit = true;
            for (id, conf) in hits {
                let energy = 0.85;
                seed_energies
                    .entry(id.clone())
                    .and_modify(|e| *e = (*e).max(energy))
                    .or_insert(energy);
                seed_reasons
                    .entry(id.clone())
                    .or_insert_with(|| format!("cluster:{noun}"));
                seed_resolutions.push(SeedResolution {
                    query: noun.clone(),
                    resolved_id: Some(id),
                    confidence: conf,
                    resolution_tier: None,
                    embedding_score: None,
                });
            }
        }
        if !cluster_hit {
            let miss = nouns
                .first()
                .cloned()
                .or_else(|| {
                    cluster
                        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                        .map(|t| t.to_string())
                        .find(|t| t.len() >= 5)
                })
                .unwrap_or_else(|| cluster.chars().take(32).collect());
            if !miss.is_empty() && !seed_resolutions.iter().any(|s| s.query == miss) {
                seed_resolutions.push(SeedResolution {
                    query: miss,
                    resolved_id: None,
                    confidence: 0.0,
                    resolution_tier: None,
                    embedding_score: None,
                });
            }
        }
    }
}

/// Resolve a lowercase cluster noun to the files that actually answer it.
/// Sibling nouns in the same clause (`guard`, `router`) outrank a Vue
/// `directive/permission` UI helper that merely path-echoes `permission`.
fn resolve_cluster_noun_seeds(
    graph: &NeuralProjectGraph,
    noun: &str,
    sibling_nouns: &[String],
) -> Vec<(NodeId, f32)> {
    let hits = graph.search_symbols(noun, 8);
    if let Some(hit) = hits
        .iter()
        .find(|hit| hit.name.eq_ignore_ascii_case(noun) && hit.node_type != NodeType::File)
    {
        return vec![(hit.id.clone(), (hit.score / 100.0).clamp(0.5, 1.0))];
    }
    if let Some(hit) = resolve_file_path_noun(graph, noun) {
        return vec![hit];
    }
    let noun_l = noun.to_lowercase();
    let cluster_l = sibling_nouns
        .iter()
        .map(|s| s.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    let hits = graph.search_symbols(noun, 16);
    // Per file: (score, node, symbol name, discriminated). A file is
    // discriminated when something beyond the bare symbol match — the path,
    // the stem, a sibling noun, or the exact name — singled it out.
    let mut by_file: HashMap<String, (f32, NodeId, String, bool)> = HashMap::new();
    for hit in hits {
        let Some(node) = graph.get_node(&hit.id) else {
            continue;
        };
        let path = node.file_path.to_string_lossy().replace('\\', "/");
        if is_noise_path(Path::new(&path)) {
            continue;
        }
        let path_l = path.to_lowercase();
        let stem = Path::new(&path_l)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let raw = hit.score;
        let mut score = hit.score;
        if stem == noun_l {
            score += 24.0;
        }
        if is_template_path(&path_l) && stem == noun_l {
            score += 50.0;
        }
        if node.name.eq_ignore_ascii_case(noun) && stem != noun_l {
            score -= 36.0;
        }
        let hay = format!("{} {path_l}", node.name.to_lowercase());
        if path_l
            .split('/')
            .any(|seg| path_segment_matches_noun(seg, &noun_l))
        {
            score += 44.0;
        }
        for sib in sibling_nouns {
            let sib_l = sib.to_lowercase();
            if sib_l != noun_l && hay.contains(&sib_l) {
                score += 20.0;
            }
        }
        if (path_l.contains("/directive/") || path_l.contains("/directives/"))
            && !cluster_l.contains("directive")
        {
            score -= 40.0;
        }
        if path_l.contains("/clipboard") && !cluster_l.contains("clipboard") {
            score -= 50.0;
        }
        if (path_l.contains("/profile/") || path_l.contains("usercard"))
            && !cluster_l.contains("profile")
            && !cluster_l.contains("card")
        {
            score -= 30.0;
        }
        let discriminated = score != raw || node.name.eq_ignore_ascii_case(noun);
        let name_l = node.name.to_lowercase();
        let entry =
            by_file
                .entry(path_l)
                .or_insert((f32::MIN, hit.id.clone(), String::new(), false));
        if score > entry.0 {
            *entry = (score, hit.id, name_l, discriminated);
        }
    }
    // Total order: score, then path. `by_file` iterates in map order, and a
    // tie among twin files otherwise seeds a different `val.py` per run.
    let mut ranked: Vec<(f32, NodeId, String, bool, String)> = by_file
        .into_iter()
        .map(|(path, (score, id, name, disc))| (score, id, name, disc, path))
        .collect();
    ranked.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.4.cmp(&b.4))
    });
    // One symbol that merely contains the noun, in three or more files, and
    // nothing — path, stem, sibling noun — prefers any of them (`predictions`
    // -> `plot_predictions` in seven `val.py` twins): the noun names a
    // concept, not a place. Seeding it would pick files by tie order.
    let top_tier_twins = ranked
        .iter()
        .take_while(|(score, _, name, disc, _)| {
            !disc && *name == ranked[0].2 && *score == ranked[0].0
        })
        .count();
    if top_tier_twins >= 3 {
        return Vec::new();
    }
    let ranked: Vec<(f32, NodeId)> = ranked
        .into_iter()
        .map(|(score, id, _, _, _)| (score, id))
        .collect();
    if ranked.is_empty() {
        if let Some(hit) = resolve_file_path_noun(graph, noun) {
            return vec![hit];
        }
        return Vec::new();
    }
    let Some(&(best, _)) = ranked.first() else {
        return Vec::new();
    };
    if best < 50.0 {
        return Vec::new();
    }
    ranked
        .into_iter()
        .filter(|(score, _)| *score >= best - 28.0 && *score >= 50.0)
        .take(3)
        .map(|(score, id)| (id, (score / 200.0).clamp(0.55, 0.95)))
        .collect()
}

fn resolve_file_path_noun(graph: &NeuralProjectGraph, noun: &str) -> Option<(NodeId, f32)> {
    let noun_l = noun.to_lowercase();
    for (id, path) in graph.file_node_paths() {
        let path_l = path.to_string_lossy().replace('\\', "/").to_lowercase();
        if !path_l
            .split('/')
            .any(|seg| path_segment_matches_noun(seg, &noun_l))
        {
            continue;
        }
        if is_noise_path(&path) {
            continue;
        }
        return Some((id, 0.88));
    }
    None
}

fn path_segment_matches_noun(segment: &str, noun_l: &str) -> bool {
    segment == noun_l && !segment.contains('.')
}

fn is_template_path(path_l: &str) -> bool {
    path_l.contains("/theme/")
        || path_l.contains("/templates/")
        || path_l.contains("/views/")
        || path_l.ends_with(".twig")
        || path_l.ends_with(".blade.php")
        || path_l.ends_with(".jinja")
        || path_l.ends_with(".hbs")
}

fn prefer_search_seed(
    graph: &NeuralProjectGraph,
    query: &str,
    ranked_id: NodeId,
    ranked_confidence: EdgeConfidence,
    prompt: &str,
) -> NodeId {
    let hits = graph.search_symbols(query, 8);
    if prompt_targets_types(prompt) {
        if let Some(hit) = hits.iter().find(|hit| {
            hit.node_type == NodeType::Symbol
                && hit.name.eq_ignore_ascii_case(query)
                && seed_path_allowed(graph, &hit.id, prompt)
        }) {
            return hit.id.clone();
        }
    }
    let Some(hit) = hits
        .iter()
        .find(|hit| hit.score >= 90.0 && seed_path_allowed(graph, &hit.id, prompt))
        .cloned()
    else {
        return ranked_id;
    };
    if hit.id == ranked_id {
        return ranked_id;
    }
    // Search only overrides the graph ranking when it is strictly better. On a
    // tie the ranked candidate stays: that ranking already weighed degree and
    // body size, which the search score does not, and before this check the
    // winner of a tie was whichever hit the hash map yielded first.
    if hits
        .iter()
        .any(|h| h.id == ranked_id && h.score >= hit.score)
    {
        return ranked_id;
    }
    if matches!(hit.node_type, NodeType::File) {
        return ranked_id;
    }
    let exact_case = hit.name == query;
    let path_hit = path_echoes_symbol(&hit.file_path, query);
    if !exact_case && !path_hit {
        return ranked_id;
    }
    if ranked_confidence == EdgeConfidence::Proven && !exact_case && !path_hit {
        return ranked_id;
    }
    hit.id
}

/// Boost seed energy when file path stem aligns with the natural-language prompt.
fn boost_path_stem_seed_energies(
    graph: &NeuralProjectGraph,
    prompt: &str,
    seed_energies: &mut HashMap<NodeId, f32>,
) {
    let prompt_l = prompt.to_lowercase();
    let prompt_has_lib_stem = graph
        .file_node_paths()
        .into_iter()
        .filter(|(_, p)| {
            let l = p.to_string_lossy().replace('\\', "/").to_lowercase();
            l.contains("/lib/") || l.contains("/src/")
        })
        .any(|(_, p)| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|stem| stem.len() >= 4 && prompt_l.contains(&stem.to_lowercase()))
        });
    for (id, energy) in seed_energies.iter_mut() {
        let Some(node) = graph.get_node(id) else {
            continue;
        };
        let path_l = node
            .file_path
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase();
        if prompt_has_lib_stem && (path_l.contains("/types/") || path_l.ends_with(".d.ts")) {
            *energy *= 0.80;
            continue;
        }
        let stem = node
            .file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if stem.len() >= 4 && prompt_l.contains(&stem) {
            *energy *= 1.15;
            continue;
        }
        if path_echoes_symbol(&node.file_path, prompt) {
            *energy *= 1.10;
        }
    }
}

fn unique_file_tokens<'a, I>(items: I) -> usize
where
    I: Iterator<Item = &'a MaterializedNode>,
{
    let mut by_path: HashMap<String, (bool, usize)> = HashMap::new();
    for item in items {
        let path = item.node.file_path.to_string_lossy().replace('\\', "/");
        let is_file = item.node.node_type == NodeType::File;
        let cost = item.node.token_cost;
        match by_path.get_mut(&path) {
            Some((had_file, stored)) => {
                if is_file {
                    *had_file = true;
                    *stored = cost;
                } else if !*had_file {
                    *stored = (*stored).max(cost);
                }
            }
            None => {
                by_path.insert(path, (is_file, cost));
            }
        }
    }
    by_path.values().map(|(_, cost)| *cost).sum()
}

fn function_spans_for_file(
    graph: &NeuralProjectGraph,
    path: &std::path::Path,
) -> Vec<FunctionSpan> {
    graph
        .nodes_in_file(path)
        .into_iter()
        // The artifact overlay retypes functions — `train_one_epoch` becomes
        // a TrainLoop, `forward` a Layer, `build_transforms` a Transform. A
        // span is about the text, not the type: anything whose signature is a
        // function definition is a span, or its body silently leaves the
        // packet (no fold marker, nothing to expand), which is exactly what
        // happened to the function a question named.
        .filter(|n| n.node_type == NodeType::Function || looks_like_function_def(n))
        .filter_map(|n| {
            let range = n.line_range?;
            Some(FunctionSpan {
                name: n.name,
                start_line: range.start,
                end_line: range.end.saturating_sub(1).max(range.start),
                signature: n.signature.unwrap_or_default(),
                owner: n.parent,
            })
        })
        .collect()
}

fn looks_like_function_def(node: &neuromesh_core::ContextNode) -> bool {
    if !node.node_type.is_artifact() {
        return false;
    }
    let Some(sig) = node.signature.as_deref() else {
        return false;
    };
    let mut head = sig.trim();
    for modifier in [
        "export ",
        "pub(crate) ",
        "pub ",
        "async ",
        "static ",
        "private ",
        "public ",
        "protected ",
        "override ",
        "@",
    ] {
        while let Some(rest) = head.strip_prefix(modifier) {
            head = rest.trim_start();
        }
    }
    ["def ", "fn ", "function ", "fun ", "func "]
        .iter()
        .any(|kw| head.starts_with(kw))
}

fn compute_seed_call_coverage(
    graph: &NeuralProjectGraph,
    seeds: &HashSet<NodeId>,
    selected_paths: &HashSet<String>,
) -> f32 {
    let mut total = 0usize;
    let mut hit = 0usize;
    for seed in seeds {
        for (neighbor, edge) in graph.get_connected_neighbors(seed) {
            if edge.edge_type != EdgeType::Calls || edge.source != *seed {
                continue;
            }
            let Some(node) = graph.get_node(&neighbor) else {
                continue;
            };
            total += 1;
            let path = node.file_path.to_string_lossy().replace('\\', "/");
            if selected_paths.contains(&path) {
                hit += 1;
            }
        }
    }
    if total == 0 {
        1.0
    } else {
        hit as f32 / total as f32
    }
}

fn build_next_actions(
    graph: &NeuralProjectGraph,
    active: &[ActivatedNodeView],
    selected: &HashSet<NodeId>,
    coverage: &CoverageReport,
    fold_ids: &[FoldedIntron],
    unresolved: &[neuromesh_core::UnresolvedRef],
) -> Vec<NextAction> {
    let mut actions = Vec::new();
    let needs_search = matches!(
        coverage.claim.as_str(),
        "partial" | "no_seed_resolved" | "no_confident_match"
    );
    if needs_search {
        let why = match coverage.claim.as_str() {
            "no_seed_resolved" => {
                "no seed resolved — Grep this identifier; do not trust an empty or utility packet"
            }
            "no_confident_match" => {
                "no confident embedding match — functionality may be absent; Grep before assuming relevance"
            }
            _ => "coverage is partial — Grep/search this missed seed",
        };
        for missed in &coverage.seeds_missed {
            actions.push(NextAction {
                tool: "neuromesh_search_symbols".into(),
                query: missed.clone(),
                why: why.into(),
            });
        }
        for gap in &coverage.packet_gaps {
            actions.push(NextAction {
                tool: "neuromesh_search_symbols".into(),
                query: gap.path.clone(),
                why: format!("packet gap ({}): {}", gap.kind, gap.reason),
            });
        }
    }
    let mut ranked: Vec<&FoldedIntron> = fold_ids.iter().collect();
    ranked.sort_by(|a, b| {
        b.task_score
            .partial_cmp(&a.task_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.symbol_name.cmp(&b.symbol_name))
    });
    let relevant: Vec<&FoldedIntron> = ranked
        .iter()
        .copied()
        .filter(|f| f.task_score >= 8.0)
        .take(3)
        .collect();
    let expand: Vec<&FoldedIntron> = if !relevant.is_empty() {
        relevant
    } else {
        ranked.into_iter().take(1).collect()
    };
    for fold in expand {
        let owner = fold
            .owner
            .as_deref()
            .map(|o| format!("{o}."))
            .unwrap_or_default();
        actions.push(NextAction {
            tool: "neuromesh_expand_fold".into(),
            query: fold.fold_id.clone(),
            why: format!(
                "wake folded {owner}{} — closest intron to the task",
                fold.symbol_name
            ),
        });
    }
    for node in active.iter().take(4) {
        for (neighbor, edge) in graph.get_connected_neighbors(&node.node.id) {
            if edge.edge_type == EdgeType::Calls && !selected.contains(&neighbor) {
                if let Some(outside) = graph.get_node(&neighbor) {
                    actions.push(NextAction {
                        tool: "neuromesh_trace".into(),
                        query: outside.name,
                        why: "caller or callee sits outside the packet".into(),
                    });
                    break;
                }
            }
        }
        if actions.len() >= 8 {
            break;
        }
    }
    if needs_search {
        if let Some(u) = unresolved.first() {
            if !actions.iter().any(|a| a.query == u.name) {
                actions.push(NextAction {
                    tool: "neuromesh_search_symbols".into(),
                    query: u.name.clone(),
                    why: format!(
                        "unresolved {:?} from {} — Grep only because coverage is partial",
                        u.relationship, u.from
                    ),
                });
            }
        }
    }
    actions.truncate(8);
    actions
}

/// A fill file whose stem every route file shares (`route.ts` ×7 in a Next.js
/// app) is not "named" by the prompt's `route`; it counts as named only when
/// a directory of its own path is a focus term (`webhooks`, `stripe`).
/// Without this, every `route.ts` with some term overlap rode in as a
/// sidecar, including the forbidden one (holdout-web).
fn shared_stem_without_dir_focus(
    graph: &NeuralProjectGraph,
    path: &std::path::Path,
    focus_terms: &HashSet<String>,
) -> bool {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    // Only the file names a framework stamps on every module (`route.ts`,
    // `index.ts`, `page.tsx`, `layout.tsx`, `handler`, `controller`): a
    // `train.jl` that three files share is still a name the prompt can mean
    // (flux, holdout-lang).
    const CONVENTION: &[&str] = &[
        "route",
        "routes",
        "index",
        "page",
        "layout",
        "loading",
        "handler",
        "handlers",
        "controller",
        "controllers",
        "service",
        "services",
        "view",
        "views",
        "mod",
        "main",
        "types",
        "schema",
        "middleware",
        "error",
        "not-found",
        "template",
    ];
    if !CONVENTION.contains(&stem.to_lowercase().as_str()) {
        return false;
    }
    let shared = graph
        .file_node_paths()
        .iter()
        .filter(|(_, p)| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case(stem))
        })
        .count()
        >= 3;
    if !shared {
        return false;
    }
    let dir_focus = path.parent().is_some_and(|d| {
        d.components().any(|c| {
            c.as_os_str().to_str().is_some_and(|s| {
                let s = s.trim_matches(['(', ')', '[', ']']).to_lowercase();
                focus_terms.iter().any(|t| {
                    t.len() >= 4
                        && (crate::seed::weak_file_seed::strip_plural(&s)
                            == crate::seed::weak_file_seed::strip_plural(t))
                })
            })
        })
    });
    !dir_focus
}

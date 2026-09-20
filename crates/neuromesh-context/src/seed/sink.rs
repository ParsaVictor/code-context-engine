use neuromesh_core::{NodeId, SeedResolution};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::HashMap;

fn tier_for_reason(reason: &str) -> Option<&'static str> {
    use crate::retrieval::embedding_confidence::{TIER_EMBEDDING_PRIMARY, TIER_L1_EXACT};
    match reason {
        "identifier" | "entity" | "file" | "client_keyword" | "alias_code" => Some(TIER_L1_EXACT),
        "client_expansion" | "inferred_keyword" | "path_hint" | "entity_type" | "token"
        | "style_hint" | "style_component" | "style_partial" | "style_mixin" | "style_token"
        | "view_component" => Some(TIER_L1_EXACT),
        r if r.starts_with("fallback:") => Some(TIER_L1_EXACT),
        r if r.starts_with("semantic_embed") => Some(TIER_EMBEDDING_PRIMARY),
        _ => None,
    }
}

pub struct SeedBuffers<'res, 'eng, 'rsn> {
    pub resolutions: &'res mut Vec<SeedResolution>,
    pub energies: &'eng mut HashMap<NodeId, f32>,
    pub reasons: &'rsn mut HashMap<NodeId, String>,
}

pub type ResolveSeedFn = fn(&NeuralProjectGraph, &str, &str) -> Option<(NodeId, f32)>;

pub struct SeedSink<'res, 'eng, 'rsn> {
    resolutions: &'res mut Vec<SeedResolution>,
    energies: &'eng mut HashMap<NodeId, f32>,
    reasons: &'rsn mut HashMap<NodeId, String>,
    resolve: ResolveSeedFn,
}

impl<'res, 'eng, 'rsn> SeedSink<'res, 'eng, 'rsn> {
    pub fn new(
        resolutions: &'res mut Vec<SeedResolution>,
        energies: &'eng mut HashMap<NodeId, f32>,
        reasons: &'rsn mut HashMap<NodeId, String>,
        resolve: ResolveSeedFn,
    ) -> Self {
        Self {
            resolutions,
            energies,
            reasons,
            resolve,
        }
    }

    pub fn resolutions(&self) -> &[SeedResolution] {
        self.resolutions
    }

    /// Every node a seed has resolved to so far, in resolution order.
    pub fn resolved_ids(&self) -> Vec<NodeId> {
        self.resolutions
            .iter()
            .filter_map(|s| s.resolved_id.clone())
            .collect()
    }

    pub fn resolved_count(&self) -> usize {
        self.resolutions
            .iter()
            .filter(|s| s.resolved_id.is_some())
            .count()
    }

    pub fn push(
        &mut self,
        graph: &NeuralProjectGraph,
        prompt: &str,
        query: String,
        energy: f32,
        reason: &str,
    ) {
        if self.resolutions.iter().any(|s| {
            s.resolved_id.is_some()
                && (s.query == query || s.query == format!("identifier:{query}"))
        }) {
            return;
        }
        if self.resolutions.iter().any(|s| s.query == query) {
            return;
        }
        if let Some((mut id, conf)) = (self.resolve)(graph, &query, prompt) {
            // A prompt word matched against every symbol in the graph (the
            // token fallbacks) is a guess; a guess that lands in docs, tests
            // or fixtures is noise unless the question is about those
            // (F75-B: `token:learning` → docs/index.html, `token:packet` →
            // tests/learning_loop.rs while the real code sat in src/).
            // An acronym (`CLI`, `MCP`, `API`) names exactly that: a symbol
            // or file called that, case aside. A prefix hit on a longer name
            // (`CLI` → `cli_request_id`) is the F61 acronym-fragment noise
            // again, on the resolver's side (F75-C).
            // A type may carry the acronym as its first CamelCase segment
            // (`SmsStore`, `McpServer`, `HttpClient`): that is how domain
            // acronyms are spelled in type names, so it stays.
            let acronym_fragment = is_acronym(&query)
                && graph.get_node(&id).is_some_and(|n| {
                    let typed_prefix =
                        matches!(
                            n.node_type,
                            neuromesh_core::NodeType::Class
                                | neuromesh_core::NodeType::Model
                                | neuromesh_core::NodeType::Component
                        ) && n.name.to_lowercase().starts_with(&query.to_lowercase());
                    !typed_prefix
                        && !n.name.eq_ignore_ascii_case(&query)
                        && !n
                            .file_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .is_some_and(|s| s.eq_ignore_ascii_case(&query))
                });
            if acronym_fragment
                || (is_guess_reason(reason)
                    && !prompt_names_low_priority(prompt)
                    && graph
                        .get_node(&id)
                        .is_some_and(|n| crate::selector::is_noise_path(&n.file_path)))
            {
                self.resolutions.push(SeedResolution {
                    query,
                    resolved_id: None,
                    confidence: 0.0,
                    resolution_tier: None,
                    embedding_score: None,
                });
                return;
            }
            if graph
                .get_node(&id)
                .is_some_and(|n| n.node_type == neuromesh_core::NodeType::File)
            {
                if let Some(hit) = graph.search_symbols(&query, 6).into_iter().find(|hit| {
                    hit.name.eq_ignore_ascii_case(&query)
                        && hit.node_type != neuromesh_core::NodeType::File
                }) {
                    id = hit.id;
                }
            }
            self.insert(
                id,
                energy.max(conf),
                format!("{reason}:{query}"),
                tier_for_reason(reason),
                None,
            );
        } else {
            self.resolutions.push(SeedResolution {
                query,
                resolved_id: None,
                confidence: 0.0,
                resolution_tier: None,
                embedding_score: None,
            });
        }
    }

    pub fn insert(
        &mut self,
        id: NodeId,
        energy: f32,
        reason: String,
        resolution_tier: Option<&str>,
        embedding_score: Option<f32>,
    ) {
        self.energies
            .entry(id.clone())
            .and_modify(|e| *e = (*e).max(energy))
            .or_insert(energy);
        self.reasons.entry(id.clone()).or_insert(reason.clone());
        if !self
            .resolutions
            .iter()
            .any(|s| s.resolved_id.as_ref() == Some(&id))
        {
            self.resolutions.push(SeedResolution {
                query: reason,
                resolved_id: Some(id),
                confidence: energy,
                resolution_tier: resolution_tier.map(str::to_string),
                embedding_score,
            });
        }
    }

    pub fn buffers_mut(&mut self) -> SeedBuffers<'_, '_, '_> {
        SeedBuffers {
            resolutions: self.resolutions,
            energies: self.energies,
            reasons: self.reasons,
        }
    }
}

/// Reasons that mean "a prompt word, matched to whatever symbol it fits",
/// as opposed to something the prompt named (identifier, file, config key).
fn is_guess_reason(reason: &str) -> bool {
    matches!(
        reason,
        "token"
            | "fallback:token"
            | "fallback:lexical"
            | "concept"
            | "alias_gap_fill"
            | "client_expansion"
            | "inferred_keyword"
    )
}

/// The question is about tests, docs, examples or fixtures, so those paths
/// are its subject and not noise.
fn prompt_names_low_priority(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    [
        "test",
        "spec",
        "fixture",
        "harness",
        "benchmark",
        "docs",
        "documentation",
        "example",
        "readme",
        "tutorial",
    ]
    .iter()
    .any(|w| lower.contains(w))
}

/// `CLI`, `MCP`, `API`, `RPN`: two to four upper-case letters.
fn is_acronym(query: &str) -> bool {
    (2..=4).contains(&query.len()) && query.chars().all(|c| c.is_ascii_uppercase())
}

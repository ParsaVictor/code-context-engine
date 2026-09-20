//! Seed pipeline helpers shared by the strategy engines.

use crate::activator::prune_weak_greenfield_seeds_inner;
use crate::seed::ranker::{signal_weight, SignalKind};
use crate::seed::sink::SeedSink;
use neuromesh_core::{SeedResolutionConfig, TaskIntent, TaskSignature};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::{is_prompt_stopword, normalize_prompt_tokens};

pub(crate) fn push_anchor_queries(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    for ident in &signature.identifiers {
        if ident.eq_ignore_ascii_case(signature.technology.as_str()) {
            continue;
        }
        sink.push(graph, prompt, ident.clone(), 1.0, "identifier");
    }
    if !signature.entity.is_empty()
        && signature.entity != "Workspace"
        && !signature
            .identifiers
            .iter()
            .any(|id| id == &signature.entity)
        && !matches!(signature.intent, TaskIntent::Create)
    {
        sink.push(graph, prompt, signature.entity.clone(), 1.0, "entity");
    }
    for hint in &signature.file_hints {
        sink.push(graph, prompt, hint.clone(), 0.95, "file");
    }
    // "the compile key", "the masks flag", "--epochs": the word before
    // `key`/`flag`/`option`/`setting` (or after `--`) is a configuration
    // name even when it is an English word the extractor would never take
    // as an identifier. It resolves against Config/Hyperparameter nodes only,
    // so the config file that defines it enters the packet; energy never
    // flows reader → key on its own (F55).
    for word in config_key_mentions(prompt) {
        // No dedupe by word here: `identifier:compile` (the bare member of
        // `torch.compile`) may already sit on the *wrong* config file and is
        // pruned later as a bare member anyway. `insert` dedupes by node, so
        // the same file is never seeded twice.
        // Among several files defining the key, the one whose values name
        // an already-seeded reader's class/module wins (Hydra `_target_`);
        // then the plain resolver; then the first candidate by id, so a
        // key in two files still seeds instead of resolving to nothing.
        let readers: Vec<neuromesh_core::NodeId> = sink
            .resolutions()
            .iter()
            .filter_map(|s| s.resolved_id.clone())
            .collect();
        let resolved = readers
            .iter()
            .find_map(|r| graph.resolve_config_key_for_reader(&word, None, r))
            .or_else(|| graph.resolve_config_key(&word, None))
            .or_else(|| {
                graph
                    .config_key_candidates(&word)
                    .into_iter()
                    .min_by(|a, b| a.as_str().cmp(b.as_str()))
                    .map(|id| (id, neuromesh_core::EdgeConfidence::Likely))
            });
        if let Some((id, conf)) = resolved {
            let energy = match conf {
                neuromesh_core::EdgeConfidence::Proven => 0.95,
                _ => 0.85,
            };
            let reason = format!("config_key:{word}");
            // The extractor may already have this node as `identifier:compile`
            // (the bare member of `torch.compile`), which `bare_owner` prunes
            // later as a fragment of the dotted seed. The prompt said "the
            // compile key": retag it so the config anchor survives.
            let retagged = {
                let buffers = sink.buffers_mut();
                let mut hit = false;
                for s in buffers.resolutions.iter_mut() {
                    if s.resolved_id.as_ref() == Some(&id) && s.query.starts_with("identifier:") {
                        s.query = reason.clone();
                        hit = true;
                    }
                }
                if hit {
                    buffers.reasons.insert(id.clone(), reason.clone());
                }
                hit
            };
            if retagged {
                continue;
            }
            sink.insert(
                id,
                energy,
                reason,
                Some(crate::retrieval::embedding_confidence::TIER_L1_EXACT),
                None,
            );
        }
    }
    for concept in &signature.related_concepts {
        let is_code = concept.contains('.') || concept.contains('(');
        if !is_code && concept.len() < 4 {
            continue;
        }
        if signature
            .identifiers
            .iter()
            .any(|id| id.eq_ignore_ascii_case(concept))
        {
            continue;
        }
        let lower = concept.to_lowercase();
        if lower == "layout" || lower == "breakpoints" || lower == "state" {
            continue;
        }
        if concept.eq_ignore_ascii_case(signature.technology.as_str()) {
            continue;
        }
        sink.push(
            graph,
            prompt,
            concept.clone(),
            if is_code { 0.88 } else { 0.82 },
            if is_code { "alias_code" } else { "concept" },
        );
    }
    for term in crate::retrieval::alias::alias_seed_queries(prompt) {
        if signature
            .identifiers
            .iter()
            .any(|id| id.eq_ignore_ascii_case(&term))
        {
            continue;
        }
        sink.push(graph, prompt, term, 0.92, "alias_code");
    }
    if sink.resolved_count() == 0 && sink.resolutions().is_empty() {
        for token in signature.raw_prompt.split_whitespace().take(8) {
            let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
            if clean.len() < 5 || is_prompt_stopword(clean) {
                continue;
            }
            sink.push(graph, prompt, clean.to_string(), 0.55, "token");
        }
    }
}

pub(crate) fn push_alias_lexical_gap_fill(
    graph: &NeuralProjectGraph,
    prompt: &str,
    config: &SeedResolutionConfig,
    embedding_config: &neuromesh_core::EmbeddingConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    let max_embed = sink
        .resolutions()
        .iter()
        .filter_map(|s| s.embedding_score)
        .fold(0.0f32, f32::max);
    let weak_embed = max_embed < embedding_config.min_cosine;
    if !weak_embed && sink.resolved_count() > 0 {
        return;
    }
    let mut added = 0usize;
    for seed in crate::retrieval::alias::alias_code_seeds_for_prompt(prompt) {
        if added >= config.max_resolved_seeds {
            break;
        }
        if sink.resolutions().iter().any(|s| s.query == seed) {
            continue;
        }
        let before = sink.resolved_count();
        let energy = signal_weight(config, SignalKind::Keyword, added);
        sink.push(graph, prompt, seed, energy * 0.85, "alias_gap_fill");
        if sink.resolved_count() > before {
            added += 1;
        }
    }
}

pub(crate) fn push_client_keywords(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    if signature.client_keywords.is_empty() {
        return;
    }
    let mut resolved = 0usize;
    for (pos, kw) in signature
        .client_keywords
        .iter()
        .take(config.max_keywords)
        .enumerate()
    {
        if resolved >= config.max_resolved_seeds {
            break;
        }
        if sink.resolutions().iter().any(|s| s.query == *kw) {
            continue;
        }
        let energy = signal_weight(config, SignalKind::Keyword, pos);
        let before = sink.resolved_count();
        // A keyword the server inferred from the prompt is a guess, not a
        // client anchor: it seeds at the weak tier so the noise-path,
        // off-family and style-asset pruning apply to it (F62: "style asset"
        // → `stat` → `applyStatic` in the docs site for a Rust question).
        let reason = if signature.client_keywords_inferred {
            "inferred_keyword"
        } else {
            "client_keyword"
        };
        sink.push(graph, prompt, kw.clone(), energy, reason);
        if sink.resolved_count() > before {
            resolved += 1;
        }
    }
}

pub(crate) fn push_client_expansion(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    for (pos, term) in signature
        .client_expansion
        .iter()
        .take(config.max_expansion)
        .enumerate()
    {
        if sink.resolutions().iter().any(|s| s.query == *term) {
            continue;
        }
        let energy = signal_weight(config, SignalKind::Expansion, pos);
        sink.push(graph, prompt, term.clone(), energy, "client_expansion");
    }
}

pub(crate) fn push_path_hint_seeds(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    for (pos, hint) in signature.client_path_hints.iter().enumerate() {
        if graph.resolve_file_hint(hint).is_some() {
            let energy = signal_weight(config, SignalKind::PathHint, pos);
            sink.push(graph, prompt, hint.clone(), energy, "path_hint");
        }
    }
    for (pos, et) in signature.client_entity_types.iter().enumerate() {
        let energy = signal_weight(config, SignalKind::EntityType, pos);
        sink.push(graph, prompt, et.clone(), energy, "entity_type");
    }
}

pub(crate) fn token_fallback_seeds(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    prompt: &str,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    let tokens: Vec<String> =
        if signature.client_keywords.is_empty() && signature.client_expansion.is_empty() {
            signature
                .raw_prompt
                .split_whitespace()
                .take(8)
                .map(|t| {
                    t.trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
                        .to_string()
                })
                .collect()
        } else {
            normalize_prompt_tokens(signature.raw_prompt.as_str())
                .into_iter()
                .take(8)
                .collect()
        };
    for token in tokens {
        if token.len() < 5 || is_prompt_stopword(&token) {
            continue;
        }
        sink.push(graph, prompt, token, 0.55, "token");
    }
}

pub(crate) fn prune_weak_greenfield_seeds(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    // Brownfield-safe: only prune fuzzy NL seeds on greenfield Create tasks.
    if !matches!(signature.intent, TaskIntent::Create) {
        return;
    }
    if !signature.client_keywords.is_empty() || !signature.client_expansion.is_empty() {
        return;
    }
    prune_weak_greenfield_seeds_inner(graph, signature, &mut sink.buffers_mut());
}

/// Words the prompt marks as configuration names: `X key`, `X flag`,
/// `X option`, `X setting`, `X parameter`, or `--X`. Lower-cased, deduped,
/// in prompt order.
pub(crate) fn config_key_mentions(prompt: &str) -> Vec<String> {
    const MARKERS: &[&str] = &["key", "flag", "option", "setting", "parameter", "argument"];
    let words: Vec<&str> = prompt.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    let clean = |w: &str| {
        w.trim_matches(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
            .to_lowercase()
    };
    for (i, w) in words.iter().enumerate() {
        let word = clean(w);
        if let Some(flag) = word.strip_prefix("--") {
            let flag = flag.replace('-', "_");
            if flag.len() >= 2 && !out.contains(&flag) {
                out.push(flag);
            }
            continue;
        }
        let Some(next) = words.get(i + 1) else {
            continue;
        };
        let marker = clean(next);
        let marker = marker.trim_end_matches('s');
        if !MARKERS.contains(&marker) {
            continue;
        }
        if word.len() < 3
            || is_prompt_stopword(&word)
            || matches!(
                word.as_str(),
                "the" | "this" | "that" | "config" | "same" | "each"
            )
        {
            continue;
        }
        if !out.contains(&word) {
            out.push(word);
        }
    }
    out
}

#[cfg(test)]
mod config_key_tests {
    use super::config_key_mentions;

    #[test]
    fn marks_key_flag_and_dashed_words() {
        assert_eq!(
            config_key_mentions(
                "How does the compile key in the model config make MNISTLitModule.setup call torch.compile?"
            ),
            vec!["compile"]
        );
        assert_eq!(
            config_key_mentions(
                "How does the masks flag change return_masks, and what does --num_queries set?"
            ),
            vec!["masks", "num_queries"]
        );
        // "primary key" is a database term, not a config name — a known false
        // positive; harmless unless a Config node named `primary` exists.
        assert_eq!(
            config_key_mentions("Where is the primary key of the User model defined?"),
            vec!["primary"]
        );
        assert!(config_key_mentions("How does the parser handle keys?").is_empty());
    }
}

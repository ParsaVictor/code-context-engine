//! Seed pipeline helpers shared by the strategy engines.

use crate::activator::prune_weak_greenfield_seeds_inner;
use crate::seed::ranker::{signal_weight, SignalKind};
use crate::seed::sink::SeedSink;
use neuromesh_core::{NodeId, SeedResolutionConfig, TaskIntent, TaskSignature};
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
        let before = sink.resolved_count();
        sink.push(graph, prompt, ident.clone(), 1.0, "identifier");
        if sink.resolved_count() > before {
            continue;
        }
        // The bare half of a dotted identifier (`segmentsFromString` next to
        // `PathChunk.segmentsFromString`) is answered by the dotted one; as
        // a literal it only finds the test that quotes the name (holdout-lang).
        if signature.identifiers.iter().any(|other| {
            other != ident
                && other
                    .rsplit(['.', ':'])
                    .next()
                    .is_some_and(|last| last.eq_ignore_ascii_case(ident))
        }) {
            continue;
        }
        // No symbol is called that, but a file spells it in quotes: a tool
        // name (`neuromesh_record_feedback`), a route, an event, an env var.
        // Up to eight files — a literal in more places is a shared constant,
        // not the place the question means (F75-A).
        // A route is quoted with its slash (`"/update-password"`); the prompt
        // says it without.
        let mut all = graph.files_with_literal(ident);
        if all.is_empty() && !ident.starts_with('/') {
            all = graph.files_with_literal(&format!("/{ident}"));
        }
        let is_noise = |id: &neuromesh_core::NodeId| {
            graph
                .get_node(id)
                .is_some_and(|n| crate::selector::is_noise_path(&n.file_path))
        };
        // Product code first; but an exact string that only a test or a
        // script spells (`NM_EXPLAIN` in the harness) still names that file.
        let mut files: Vec<neuromesh_core::NodeId> = if all.iter().any(|id| !is_noise(id)) {
            all.iter().filter(|id| !is_noise(id)).cloned().collect()
        } else {
            all.iter()
                .filter(|id| {
                    graph.get_node(id).is_some_and(|n| {
                        let p = n.file_path.to_string_lossy().to_lowercase();
                        !(p.ends_with(".md") || p.ends_with(".txt") || p.ends_with(".rst"))
                    })
                })
                .cloned()
                .collect()
        };
        if files.is_empty() || files.len() > 8 {
            continue;
        }
        // The seed cap is small, so the files are ranked by how much of the
        // prompt they echo in their path and symbol names ("MCP server ...
        // tool call" → `mcp/src/tools.rs` first), then by path.
        let prompt_words: Vec<String> = prompt
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() >= 4)
            .map(|w| w.to_lowercase())
            .filter(|w| !is_prompt_stopword(w))
            .collect();
        let echo = |id: &neuromesh_core::NodeId| -> (usize, String) {
            let Some(node) = graph.get_node(id) else {
                return (0, String::new());
            };
            let path = node.file_path.to_string_lossy().replace('\\', "/");
            let mut hay = path.to_lowercase();
            for sym in graph.nodes_in_file(&node.file_path) {
                hay.push(' ');
                hay.push_str(&sym.name.to_lowercase());
            }
            let hits = prompt_words
                .iter()
                .filter(|w| hay.contains(w.as_str()))
                .count();
            (hits, path)
        };
        files.sort_by(|a, b| {
            let (ha, pa) = echo(a);
            let (hb, pb) = echo(b);
            hb.cmp(&ha).then_with(|| pa.cmp(&pb))
        });
        files.truncate(4);
        // The identifier is answered by the file: it is no longer a miss.
        sink.buffers_mut()
            .resolutions
            .retain(|s| !(s.resolved_id.is_none() && s.query == *ident));
        for (pos, file_id) in files.iter().enumerate() {
            if graph.get_node(file_id).is_none() {
                continue;
            };
            sink.insert(
                file_id.clone(),
                if pos == 0 { 0.9 } else { 0.75 },
                format!("literal:{ident}"),
                Some(crate::retrieval::embedding_confidence::TIER_L1_EXACT),
                None,
            );
        }
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
    push_compound_stem_seeds(graph, prompt, config, sink);
    push_dir_convention_seeds(graph, prompt, config, sink);
    push_word_stem_seeds(graph, prompt, config, sink);
    push_path_word_seeds(graph, prompt, config, sink);
    push_body_word_seeds(graph, prompt, config, sink);
}

/// Route-shaped trees (Next.js app router, `feature/handler.ts`) name a
/// file by its directory words, not its stem: "the Stripe webhook" is
/// `app/api/webhooks/stripe/route.ts`, "the dashboard layout" is
/// `app/(dashboard)/dashboard/layout.tsx`. A file whose path spells two or
/// more distinct prompt words (plural-tolerant), and is the unique best (or
/// one of two tied), is seeded (holdout-web).
pub(crate) fn push_path_word_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    const GENERIC: &[&str] = &[
        "src",
        "app",
        "lib",
        "api",
        "index",
        "main",
        "page",
        "pages",
        "component",
        "components",
        "util",
        "utils",
        "test",
        "tests",
        "route",
        "routes",
        "handler",
        "handlers",
        "file",
        "files",
        "config",
        "core",
        "common",
        "internal",
        "pkg",
        "cmd",
    ];
    let words: std::collections::HashSet<String> = prompt
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 4)
        .map(|w| crate::seed::weak_file_seed::strip_plural(&w.to_lowercase()).to_string())
        .filter(|w| !is_prompt_stopword(w) && !GENERIC.contains(&w.as_str()))
        .collect();
    // Words the files already seeded cover: a candidate must add a word of
    // its own. `src/unix/loop-watcher.c` covers {unix, loop} — both already
    // in `src/unix/loop.c`, the seed — so it is the same place said twice,
    // not a second file (holdout-c). `webhooks/stripe/route.ts` adds
    // "webhook" to a `lib/stripe.ts` seed and is a second file.
    let seeded_words: std::collections::HashSet<String> = sink
        .resolutions()
        .iter()
        .filter_map(|s| s.resolved_id.as_ref())
        .filter_map(|id| graph.get_node(id))
        .flat_map(|n| {
            n.file_path
                .to_string_lossy()
                .replace('\\', "/")
                .split(|c: char| !c.is_alphanumeric())
                .filter(|t| t.len() >= 4)
                .map(|t| crate::seed::weak_file_seed::strip_plural(&t.to_lowercase()).to_string())
                .collect::<Vec<_>>()
        })
        .collect();
    // Only when nothing was named: with a symbol, file or key resolved, path
    // words cost precision on every other repository (libuv, hydra: −0.08),
    // and the novelty check above was not enough to stop that.
    let anchored = sink.resolutions().iter().any(|s| {
        s.resolved_id.is_some()
            && crate::seed::weak_file_seed::STRONG
                .contains(&s.query.split_once(':').map(|(p, _)| p).unwrap_or(""))
    });
    if anchored || words.len() < 2 {
        return;
    }
    let paths: Vec<(String, std::collections::HashSet<String>)> = graph
        .file_node_paths()
        .into_iter()
        .filter(|(_, p)| !crate::selector::is_noise_path(p))
        .map(|(_, p)| {
            let path = p.to_string_lossy().replace('\\', "/");
            let toks: std::collections::HashSet<String> = path
                .split(|c: char| !c.is_alphanumeric())
                .filter(|t| t.len() >= 4)
                .map(|t| crate::seed::weak_file_seed::strip_plural(&t.to_lowercase()).to_string())
                .collect();
            (path, toks)
        })
        .collect();
    // How many files carry each prompt word in their path: a word shared by
    // a whole subtree (`unix` in libuv, `models` in torchvision) names a
    // place, not a file. Only a word few files carry can single one out.
    const RARE: usize = 4;
    let spread = |w: &String| paths.iter().filter(|(_, toks)| toks.contains(w)).count();
    let mut scored: Vec<(usize, String)> = paths
        .iter()
        .filter_map(|(path, toks)| {
            let covered: Vec<&String> = words.iter().filter(|w| toks.contains(*w)).collect();
            (covered.len() >= 2
                && covered.iter().any(|w| spread(w) <= RARE)
                && covered.iter().any(|w| !seeded_words.contains(*w)))
            .then_some((covered.len(), path.clone()))
        })
        .collect();
    if scored.is_empty() {
        return;
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let best = scored[0].0;
    let top: Vec<&String> = scored
        .iter()
        .filter(|(n, _)| *n == best)
        .map(|(_, p)| p)
        .collect();
    if top.len() > 2 {
        return;
    }
    for (pos, path) in top.into_iter().enumerate() {
        let energy = signal_weight(config, SignalKind::PathHint, pos + 1);
        sink.push(graph, prompt, path.clone(), energy, "stem");
    }
}

/// "the billing page", "the tasks route", "the settings page": a prompt word
/// that is a directory, next to a word that is what a framework calls the
/// file inside it (`page`, `route`, `layout`, `index`, `handler`). The file
/// `<dir>/<convention>.*` is the address (G4, holdout-web). Only when that
/// file is unique for the pair; a `page.tsx` under several `settings/`
/// directories is not one place.
pub(crate) fn push_dir_convention_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    const CONVENTION: &[(&str, &[&str])] = &[
        ("page", &["page"]),
        ("pages", &["page"]),
        ("route", &["route", "index", "routes"]),
        ("routes", &["route", "index", "routes"]),
        ("endpoint", &["route", "index"]),
        ("layout", &["layout"]),
        ("handler", &["handler", "index"]),
        ("controller", &["controller", "index"]),
        ("plugin", &["index", "plugin"]),
        ("loading", &["loading"]),
    ];
    let strip = |w: &str| crate::seed::weak_file_seed::strip_plural(w).to_string();
    let words: Vec<String> = prompt
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 4)
        .map(|w| w.to_lowercase())
        .collect();
    // Only the word right before the convention word ("the billing page",
    // "the tasks route"): a noun elsewhere in the sentence ("store the
    // user" next to "the /login route") names no directory.
    let pairs: Vec<(String, &[&str])> = words
        .windows(2)
        .filter_map(|w| {
            CONVENTION
                .iter()
                .find(|(k, _)| *k == w[1])
                .map(|(_, s)| (w[0].clone(), *s))
        })
        .collect();
    if pairs.is_empty() {
        return;
    }
    let files = graph.file_node_paths();
    let mut pushed = 0usize;
    for (word, stems) in &pairs {
        if CONVENTION.iter().any(|(k, _)| k == word) || is_prompt_stopword(word) {
            continue;
        }
        let w = strip(word);
        let hits: Vec<String> = files
            .iter()
            .filter(|(_, p)| !crate::selector::is_noise_path(p))
            .filter(|(_, p)| {
                let stem = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                let parent = p
                    .parent()
                    .and_then(|d| d.file_name())
                    .and_then(|s| s.to_str())
                    .map(|s| strip(&s.trim_matches(['(', ')', '[', ']']).to_lowercase()))
                    .unwrap_or_default();
                parent == w && stems.contains(&stem.as_str())
            })
            .map(|(_, p)| p.to_string_lossy().replace('\\', "/"))
            .collect();
        if hits.len() != 1 {
            continue;
        }
        let energy = signal_weight(config, SignalKind::PathHint, pushed + 1);
        sink.push(graph, prompt, hits[0].clone(), energy, "file");
        pushed += 1;
    }
}

/// A prose word that no symbol matched but that *starts with* a file's stem
/// names that file: "the skeletonizer" is `skeleton.rs`, "the tokenizer" is
/// `token.rs`. Only for words the pipeline already tried and missed, only
/// when exactly one non-noise file qualifies, stem ≥ 5 letters (F75-D).
pub(crate) fn push_word_stem_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    let missed: Vec<String> = sink
        .resolutions()
        .iter()
        .filter(|s| s.resolved_id.is_none())
        .map(|s| s.query.rsplit(':').next().unwrap_or("").to_lowercase())
        .filter(|w| w.len() >= 7 && w.chars().all(|c| c.is_ascii_alphabetic()))
        .collect();
    if missed.is_empty() {
        return;
    }
    let stems: Vec<(String, String)> = graph
        .file_node_paths()
        .into_iter()
        .filter(|(_, p)| !crate::selector::is_noise_path(p))
        .filter_map(|(_, p)| {
            let stem = p.file_stem()?.to_str()?.to_lowercase();
            (stem.len() >= 5 && stem.chars().all(|c| c.is_ascii_alphabetic()))
                .then(|| (stem, p.to_string_lossy().replace('\\', "/")))
        })
        .collect();
    let mut pushed = 0usize;
    for word in missed {
        let mut hits = stems
            .iter()
            .filter(|(s, _)| word.starts_with(s.as_str()) && word.len() - s.len() <= 4);
        let (Some((_, path)), None) = (hits.next(), hits.next()) else {
            continue;
        };
        let energy = signal_weight(config, SignalKind::PathHint, pushed + 1);
        sink.push(graph, prompt, path.clone(), energy, "stem");
        pushed += 1;
        if pushed >= 2 {
            break;
        }
    }
}

/// Two adjacent prompt words that spell a file's stem name that file: "the
/// index cache" is `index_cache.rs`, "rate limiter" is `rate-limiter.ts`.
/// The identifier extractor only sees the single word `index`, which lands
/// on whatever symbol is called `index` (F74: a PHP fixture's controller
/// action). The stem must be unique among non-noise files, else the pair is
/// ambiguous and stays out.
pub(crate) fn push_compound_stem_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    // Two *separate* prose words. An identifier the prompt already wrote as
    // one token (`roi_heads`, `rate-limiter`) is one word here, not a pair:
    // it is the extractor's identifier and resolves on its own.
    let words: Vec<String> = prompt
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|w| w.len() >= 3)
        .map(|w| {
            if w.chars().all(|c| c.is_alphanumeric()) {
                w.to_lowercase()
            } else {
                String::new() // keeps its place so its neighbours do not pair up
            }
        })
        .collect();
    if words.len() < 2 {
        return;
    }
    let stems: Vec<(String, String)> = graph
        .file_node_paths()
        .into_iter()
        .filter_map(|(_, p)| {
            let stem = p.file_stem()?.to_str()?.to_lowercase();
            let joined: String = stem.chars().filter(|c| c.is_alphanumeric()).collect();
            (stem.contains(['_', '-']) && joined.len() >= 6)
                .then(|| (joined, p.to_string_lossy().replace('\\', "/")))
        })
        .collect();
    let mut pushed = 0usize;
    // A kebab token the prompt wrote (`benchmark-holdout`, `phase-c-run`)
    // that is exactly a file's stem names that file — the identifier
    // extractor keeps only its first half (`benchmark`). Kebab only: a
    // snake token (`roi_heads`) is an identifier the symbol path already
    // owns, and as a file name it turned a listed attribute into a required
    // module (holdout-2 −0.002 the first time).
    for token in prompt
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|t| t.len() >= 6 && t.contains('-') && !t.contains('_'))
    {
        let joined: String = token
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        let mut hits = stems.iter().filter(|(s, _)| *s == joined);
        if let (Some((_, path)), None) = (hits.next(), hits.next()) {
            let energy = signal_weight(config, SignalKind::PathHint, pushed);
            sink.push(graph, prompt, path.clone(), energy, "file");
            pushed += 1;
            continue;
        }
        // Not a file: a kebab token is often a route (`update-password` →
        // `'/update-password'` in the routes file).
        let lower = token.to_lowercase();
        let mut files = graph.files_with_literal(&format!("/{lower}"));
        if files.is_empty() {
            files = graph.files_with_literal(&lower);
        }
        files.retain(|id| {
            graph
                .get_node(id)
                .is_some_and(|n| !crate::selector::is_noise_path(&n.file_path))
        });
        if files.is_empty() || files.len() > 3 {
            continue;
        }
        for id in files {
            sink.insert(
                id,
                0.85,
                format!("literal:{lower}"),
                Some(crate::retrieval::embedding_confidence::TIER_L1_EXACT),
                None,
            );
        }
        pushed += 1;
    }
    for pair in words.windows(2) {
        if pair[0].is_empty()
            || pair[1].is_empty()
            || is_prompt_stopword(&pair[0])
            || is_prompt_stopword(&pair[1])
        {
            continue;
        }
        let joined = format!("{}{}", pair[0], pair[1]);
        let mut hits = stems.iter().filter(|(s, _)| *s == joined);
        let Some((_, path)) = hits.next() else {
            continue;
        };
        if hits.next().is_some() {
            continue;
        }
        let energy = signal_weight(config, SignalKind::PathHint, pushed);
        let before = sink.resolved_count();
        sink.push(graph, prompt, path.clone(), energy, "file");
        if sink.resolved_count() == before {
            continue;
        }
        // The halves of the pair are not symbols of their own once the pair
        // named a file (the bare_owner rule for `owner.member`): the
        // `identifier:index` that reached some unrelated `index()` goes.
        let halves = [
            format!("identifier:{}", pair[0]),
            format!("identifier:{}", pair[1]),
        ];
        let buffers = sink.buffers_mut();
        let dropped: Vec<neuromesh_core::NodeId> = buffers
            .resolutions
            .iter()
            .filter(|s| halves.iter().any(|h| s.query.eq_ignore_ascii_case(h)))
            .filter_map(|s| s.resolved_id.clone())
            .collect();
        buffers
            .resolutions
            .retain(|s| !halves.iter().any(|h| s.query.eq_ignore_ascii_case(h)));
        for id in dropped {
            let still_used = buffers
                .resolutions
                .iter()
                .any(|s| s.resolved_id.as_ref() == Some(&id));
            if !still_used {
                buffers.energies.remove(&id);
                buffers.reasons.remove(&id);
            }
        }
        pushed += 1;
        if pushed >= 2 {
            break;
        }
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

/// Fallback only: two adjacent prompt words that spell a snake_case symbol
/// (`packet cap` → `packet_cap`) seed it, when that name has exactly one
/// definition outside noise paths. Runs after the token guesses, never
/// instead of them.
pub(crate) fn push_compound_symbol_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    let words: Vec<String> = prompt
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|w| w.len() >= 3)
        .map(|w| {
            if w.chars().all(|c| c.is_alphanumeric()) {
                w.to_lowercase()
            } else {
                String::new()
            }
        })
        .collect();
    let mut pushed = 0usize;
    for pair in words.windows(2) {
        if pair[0].is_empty()
            || pair[1].is_empty()
            || is_prompt_stopword(&pair[0])
            || is_prompt_stopword(&pair[1])
        {
            continue;
        }
        let name = format!("{}_{}", pair[0], pair[1]);
        let defs: Vec<_> = graph
            .nodes_named(&name)
            .into_iter()
            .filter(|n| {
                n.node_type != neuromesh_core::NodeType::File
                    && !crate::selector::is_noise_path(&n.file_path)
            })
            .collect();
        if defs.len() != 1 {
            continue;
        }
        sink.push(graph, prompt, name, 0.6, "fallback:compound");
        pushed += 1;
        if pushed >= 2 {
            break;
        }
    }
}
/// The lexical fallback (W3): when nothing strong was named, the file whose
/// *body* spells the most prompt words — what a grep would find — is the
/// seed. A question about a threshold constant in a test names no symbol;
/// the test that spells its words is the answer.
/// Competes only with guess seeds (token/concept/fallback tiers), never
/// with a symbol, file or key the prompt named: a guess that landed
/// elsewhere on one prefix-matched word (`token:pheromone` →
/// `PheromoneConfig` in edge.rs) is dropped when the body hit spells two
/// or more words and the guess's file spells fewer.
pub(crate) fn push_body_word_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    config: &SeedResolutionConfig,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    use crate::seed::weak_file_seed::{prefix, STRONG, WEAK};
    // A named symbol anchors the question — unless it resolved into a test
    // or fixture the prompt never asked about (`identifier:packet` → the
    // `packet()` helper of a test): that is a prose word that happened to
    // be a function name, not an anchor.
    let names_low = crate::seed::sink::prompt_names_low_priority(prompt);
    // A file inferred from prose ("python parser" → `python_lang.rs`, tagged
    // `stem`) is a guess of the same kind and competes like one.
    let anchored = sink.resolutions().iter().any(|s| {
        STRONG.contains(&prefix(&s.query))
            && prefix(&s.query) != "stem"
            && s.resolved_id.as_ref().is_some_and(|id| {
                names_low
                    || graph
                        .get_node(id)
                        .is_some_and(|n| !crate::selector::is_noise_path(&n.file_path))
            })
    });
    if anchored {
        return;
    }
    let words: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        prompt
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .flat_map(|tok| {
                // `add_argument` counts whole and by its parts.
                let mut out = vec![tok.to_string()];
                if tok.contains('_') {
                    out.extend(tok.split('_').map(str::to_string));
                }
                out
            })
            .filter(|w| w.len() >= 4)
            .map(|w| w.to_lowercase())
            .filter(|w| !is_prompt_stopword(w) && !BODY_GENERIC.contains(&w.as_str()))
            .filter(|w| seen.insert(w.clone()))
            .collect()
    };
    if words.len() < 2 {
        return;
    }
    // Product code first; a test or script only when it spells strictly
    // more of the prompt than any product file ("the threshold for the
    // large gold set" is a test's constant).
    let all = graph.body_word_ranking(&words, 12);
    let is_noise = |id: &NodeId| {
        graph
            .get_node(id)
            .is_some_and(|n| crate::selector::is_noise_path(&n.file_path))
    };
    // Prose (README, docs, planning notes) spells every word of every
    // question and answers none of them.
    let is_prose = |id: &NodeId| {
        graph.get_node(id).is_some_and(|n| {
            let p = n
                .file_path
                .to_string_lossy()
                .replace('\\', "/")
                .to_lowercase();
            p.ends_with(".md")
                || p.ends_with(".txt")
                || p.ends_with(".rst")
                || p.contains("/docs/")
                || p.starts_with("docs/")
        })
    };
    let all: Vec<(NodeId, usize, f32)> =
        all.into_iter().filter(|(id, _, _)| !is_prose(id)).collect();
    let best_product = all
        .iter()
        .filter(|(id, _, _)| !is_noise(id))
        .map(|(_, c, _)| *c)
        .max()
        .unwrap_or(0);
    let ranked: Vec<(NodeId, usize, f32)> = all
        .into_iter()
        .filter(|(id, c, _)| !is_noise(id) || (*c > best_product && !names_low) || names_low)
        .collect();
    let Some((_, covered, _)) = ranked.first().cloned() else {
        return;
    };
    if covered < 2 {
        return;
    }
    // The files that spell the most: one, or two on a tie (both ship —
    // recall over precision here, as with path-word seeds). Three or more
    // tied is a word every module uses, not an address.
    // Among files spelling the same number of words, the one whose *path*
    // spells more of the prompt is the address ("large gold set" →
    // `third_party_large_gold.rs`, not `third_party_holdout_gold.rs`); then
    // a much denser file (BM25 score well above the rest) stands alone.
    let best_score = ranked.first().map(|(_, _, s)| *s).unwrap_or(0.0);
    let path_hits = |id: &NodeId| -> usize {
        graph
            .get_node(id)
            .map(|n| {
                let p = n
                    .file_path
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_lowercase();
                words.iter().filter(|w| p.contains(w.as_str())).count()
            })
            .unwrap_or(0)
    };
    // One word of coverage is not a lead: a 12k-token file that spells one
    // more prompt word than a 400-token file at a third of its density is
    // the bigger haystack, not the answer. Files within one word of the
    // best compete on density, then the path breaks what is left.
    let wide: Vec<(NodeId, usize, f32)> = ranked
        .iter()
        .filter(|(_, c, _)| *c + 1 >= covered)
        .cloned()
        .collect();
    let max_path = wide
        .iter()
        .map(|(id, _, _)| path_hits(id))
        .max()
        .unwrap_or(0);
    let mut tied: Vec<(NodeId, usize, f32)> = if max_path > 0 {
        // A file whose path spells the prompt ("seed sink" → `seed/sink.rs`)
        // beats a haystack that spells one more word.
        wide.into_iter()
            .filter(|(id, _, _)| path_hits(id) == max_path)
            .collect()
    } else {
        wide.into_iter().filter(|(_, c, _)| *c == covered).collect()
    };
    let best_score = tied.iter().map(|(_, _, s)| *s).fold(best_score, f32::max);
    tied.retain(|(_, _, s)| *s >= best_score * 0.6);
    let covered = tied.iter().map(|(_, c, _)| *c).min().unwrap_or(covered);
    let top: Vec<(NodeId, usize)> = tied.into_iter().map(|(id, c, _)| (id, c)).collect();
    if top.len() > 2 {
        return;
    }
    let top_paths: Vec<std::path::PathBuf> = top
        .iter()
        .filter_map(|(id, _)| graph.get_node(id).map(|n| n.file_path))
        .collect();
    // Guess seeds that spelled fewer words than the body hit give way.
    let weaker: Vec<NodeId> = sink
        .resolutions()
        .iter()
        .filter(|s| WEAK.contains(&prefix(&s.query)) || prefix(&s.query) == "stem")
        .filter_map(|s| s.resolved_id.clone())
        .filter(|id| {
            graph.get_node(id).is_some_and(|n| {
                !top_paths.contains(&n.file_path)
                    && graph.body_word_hits(&n.file_path, &words) < covered
            })
        })
        .collect();
    if !weaker.is_empty() {
        let buffers = sink.buffers_mut();
        for s in buffers.resolutions.iter_mut() {
            if s.resolved_id.as_ref().is_some_and(|id| weaker.contains(id)) {
                s.resolved_id = None;
                s.confidence = 0.0;
                s.resolution_tier = None;
            }
        }
        for id in &weaker {
            buffers.energies.remove(id);
            buffers.reasons.remove(id);
        }
    }
    for (pos, path) in top_paths.iter().enumerate() {
        let energy = signal_weight(config, SignalKind::PathHint, pos + 1);
        let rel = path.to_string_lossy().replace('\\', "/");
        sink.push(graph, prompt, rel, energy, "body");
    }
}

/// Prompt words that spell nothing about *which* file: every file has them.
const BODY_GENERIC: &[&str] = &[
    "does",
    "where",
    "when",
    "which",
    "what",
    "with",
    "from",
    "into",
    "this",
    "that",
    "than",
    "then",
    "them",
    "they",
    "have",
    "been",
    "being",
    "after",
    "before",
    "about",
    "also",
    "only",
    "each",
    "some",
    "such",
    "used",
    "uses",
    "using",
    "make",
    "makes",
    "made",
    "code",
    "file",
    "files",
    "function",
    "functions",
    "method",
    "methods",
    "class",
    "value",
    "values",
    "call",
    "calls",
    "called",
    "return",
    "returns",
    "true",
    "false",
    "none",
    "null",
    "self",
    "read",
    "reads",
    "write",
    "writes",
    "handle",
    "handles",
    "check",
    "checks",
    "set",
    "sets",
    "get",
    "gets",
    "define",
    "defined",
    "defines",
    "pick",
    "picks",
    "decide",
    "decides",
    "apply",
    "applies",
    "applied",
];
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

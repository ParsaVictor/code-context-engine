//! `neuromesh eval --tasks`: the task-success harness.
//!
//! Reads `tests/tasks/*.toml` (or `--tasks-file <path>`), indexes each case's
//! repository, builds the packet through the production signature, and
//! scores it with an executor:
//!
//! - `--executor oracle` (default; what CI runs): symbol-level sufficiency,
//!   no model. See `neuromesh_context::task_harness`.
//! - `--executor model [--provider anthropic|openai|openrouter|google]
//!   [--model <id>] [--context packet|whole-gold-files|grep]`: asks a model
//!   for a patch, applies it to a scratch copy of the repository and runs
//!   the case's `verify` command. `--context` picks what the model sees:
//!   `packet` (default, this project's retrieval), `whole-gold-files` (the
//!   full, unfolded content of every file the case's `needs` name — the
//!   "just open the right files" ceiling), or `grep` (a naive literal
//!   keyword search over the prompt — the "grep is enough" baseline). All
//!   three report `task_success` and `success_per_1k_tokens` the same way,
//!   so they compare directly. Needs the provider's API key in the
//!   environment (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, ...); `--provider
//!   mock --mock-reply <file>` replays a canned reply with no key, for
//!   exercising patch-apply + `verify` in isolation. A case with no `verify`
//!   (an explanation question, not a fix-it task) instead goes through a
//!   QA-judge path: the model answers the question from the context, then a
//!   second call grades whether the answer specifically and correctly
//!   explains the case's `needs` — see `run_qa_judge_case` for the real
//!   limitation (no gold reference answers exist yet, only symbol names).
//!
//! `--json` prints the outcomes and summary as one JSON document after the
//! table, for the benchmark workflow to keep.

use neuromesh_context::gold::production_signature;
use neuromesh_context::task_harness::{
    load_task_dir, oracle_outcome, parse_task_toml, render_packet_text, summarize, TaskCase,
    TaskOutcome, TaskSuiteSummary,
};
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{
    NeuroMeshError, OptimizationMode, ProjectId, ProviderConfig, ProviderType, Result, TokenCounter,
};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use neuromesh_provider::{ChatMessage, Provider, ProviderFactory, ProviderRequest};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DEFAULT_ANTHROPIC_MODEL: &str = "claude-opus-5";
const DEFAULT_OPENAI_MODEL: &str = "gpt-4.1";
/// The grader must not be the answerer: a model grading its own prose is the
/// one judge that never notices its own mistakes. Default judge models differ
/// from the default answer models within each provider.
const DEFAULT_ANTHROPIC_JUDGE_MODEL: &str = "claude-sonnet-5";
const DEFAULT_OPENAI_JUDGE_MODEL: &str = "gpt-4.1-mini";
/// Ceiling on what a model may send back; a patch is small, a runaway is not.
const MAX_PATCH_BYTES: usize = 200_000;
const VERIFY_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone)]
enum Executor {
    Oracle,
    Model {
        provider: ProviderType,
        model: String,
    },
}

/// Phase 6: what context the model executor sends, so task-success and
/// success-per-1k-tokens can be compared against two baselines that need no
/// retrieval engine at all. `Packet` (default) is this project's own
/// retrieval. `WholeGoldFiles` sends the full, unfolded content of every
/// file the case's `needs` name — what a developer gets by opening exactly
/// the right files, with no token budget. `Grep` sends whatever a literal
/// keyword search over the prompt's significant words turns up — what an
/// agent gets with no semantic retrieval at all, a common baseline claim
/// ("grep is enough").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextMode {
    Packet,
    WholeGoldFiles,
    Grep,
}

impl ContextMode {
    fn label(self) -> &'static str {
        match self {
            Self::Packet => "packet",
            Self::WholeGoldFiles => "whole-gold-files",
            Self::Grep => "grep",
        }
    }
}

fn parse_context_mode(args: &[String]) -> Result<ContextMode> {
    match flag_value(args, "--context").unwrap_or("packet") {
        "packet" => Ok(ContextMode::Packet),
        "whole-gold-files" | "whole-gold" => Ok(ContextMode::WholeGoldFiles),
        "grep" => Ok(ContextMode::Grep),
        other => Err(NeuroMeshError::Config(format!(
            "unknown --context {other}; use packet, whole-gold-files or grep"
        ))),
    }
}

/// Baseline 1: every file named in `needs`, in full, unfolded. No retrieval,
/// no budget — the ceiling on how much context "just open the right files"
/// costs. Dedups by path; a case naming the same file for two symbols only
/// pays for it once.
fn whole_gold_files_context(case: &TaskCase, repo: &Path) -> String {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = String::new();
    for need in &case.needs {
        if !seen.insert(need.file.clone()) {
            continue;
        }
        let path = repo.join(&need.file);
        match std::fs::read_to_string(&path) {
            Ok(body) => {
                out.push_str(&format!("=== {} ===\n{body}\n\n", need.file));
            }
            Err(e) => {
                out.push_str(&format!("=== {} ===\n(could not read: {e})\n\n", need.file));
            }
        }
    }
    out
}

/// Words from the prompt worth grepping for: alphanumeric runs of 4+ chars,
/// lowercased, minus a short English stopword list. Short/common words would
/// turn the grep into "match every file", defeating the point of a baseline
/// meant to be cheap and naive.
fn grep_keywords(prompt: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "does", "what", "when", "where", "which", "with", "from", "this", "that", "have", "into",
        "over", "then", "than", "each", "call", "calls", "using", "used", "about", "would",
        "could", "should",
    ];
    let mut words: Vec<String> = prompt
        .split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 4 && !STOPWORDS.contains(&w.as_str()))
        .collect();
    words.sort();
    words.dedup();
    words
}

const GREP_SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", ".venv", "dist", "build"];
/// Cap on the grep baseline's total output — a naive grep over a real repo
/// can otherwise return megabytes; the point is to measure what a *bounded*
/// naive search costs, not to let it grow without limit.
const GREP_MAX_BYTES: usize = 60_000;
/// Cap on one file's contribution: without this, a single large file that
/// happens to match a common keyword on many lines (e.g. a huge `_test.go`
/// matching a generic word like "path" on every other line) can consume the
/// entire `GREP_MAX_BYTES` budget by itself, silently crowding out every
/// other match — including the one file the task actually needs. Found via
/// `holdout-gin`/grep scoring 0.400: `context_test.go` (287 matching lines)
/// filled the whole cap alphabetically ahead of `tree.go`.
const GREP_MAX_BYTES_PER_FILE: usize = 6_000;

fn grep_context(case: &TaskCase, repo: &Path) -> String {
    let keywords = grep_keywords(&case.prompt);
    if keywords.is_empty() {
        return String::new();
    }
    // (distinct keywords matched, relative path, snippet) — collected for
    // every matching file first, then emitted most-relevant-first, so which
    // file the filesystem happens to list first can't decide what survives
    // the byte cap. Distinct-keyword count is still pure text matching (no
    // semantic ranking), just no longer order-dependent.
    let mut candidates: Vec<(usize, String, String)> = Vec::new();
    let mut stack = vec![repo.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if !GREP_SKIP_DIRS.contains(&name.as_ref()) {
                    stack.push(path);
                }
                continue;
            }
            let Ok(body) = std::fs::read_to_string(&path) else {
                continue;
            };
            let lower = body.to_lowercase();
            let distinct = keywords
                .iter()
                .filter(|k| lower.contains(k.as_str()))
                .count();
            if distinct == 0 {
                continue;
            }
            let rel = path
                .strip_prefix(repo)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let lines: Vec<&str> = body.lines().collect();
            let mut snippet = String::new();
            for (i, line) in lines.iter().enumerate() {
                if snippet.len() >= GREP_MAX_BYTES_PER_FILE {
                    snippet.push_str("...(file truncated at per-file cap)\n");
                    break;
                }
                if keywords
                    .iter()
                    .any(|k| line.to_lowercase().contains(k.as_str()))
                {
                    let start = i.saturating_sub(2);
                    let end = (i + 3).min(lines.len());
                    for l in &lines[start..end] {
                        snippet.push_str(l);
                        snippet.push('\n');
                    }
                    snippet.push_str("...\n");
                }
            }
            if snippet.is_empty() {
                continue;
            }
            candidates.push((distinct, rel, snippet));
        }
    }
    // Most distinct keywords matched first; ties broken by path for
    // determinism (no dependence on filesystem listing order).
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    let mut out = String::new();
    for (_, rel, snippet) in candidates {
        let block = format!("=== {rel} (grep match) ===\n{snippet}\n");
        if out.len() + block.len() > GREP_MAX_BYTES {
            out.push_str("\n...(grep baseline truncated at cap)\n");
            break;
        }
        out.push_str(&block);
    }
    out
}

pub fn wants_tasks(args: &[String]) -> bool {
    args.iter().any(|a| a == "--tasks")
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn parse_executor(args: &[String]) -> Result<Executor> {
    match flag_value(args, "--executor").unwrap_or("oracle") {
        "oracle" => Ok(Executor::Oracle),
        "model" => {
            let provider = match flag_value(args, "--provider").unwrap_or("anthropic") {
                "anthropic" => ProviderType::Anthropic,
                "openai" => ProviderType::OpenAI,
                "openrouter" => ProviderType::OpenRouter,
                "google" => ProviderType::Google,
                // Replays a canned reply from `--mock-reply <file>`: exercises
                // patch application and `verify` without a model or a key.
                "mock" => ProviderType::Mock,
                other => {
                    return Err(NeuroMeshError::Config(format!(
                    "unknown provider {other}; use anthropic, openai, openrouter, google or mock"
                )))
                }
            };
            let model = flag_value(args, "--model")
                .map(str::to_string)
                .unwrap_or_else(|| match provider {
                    ProviderType::Anthropic => DEFAULT_ANTHROPIC_MODEL.into(),
                    ProviderType::Mock => "mock".into(),
                    _ => DEFAULT_OPENAI_MODEL.into(),
                });
            Ok(Executor::Model { provider, model })
        }
        other => Err(NeuroMeshError::Config(format!(
            "unknown executor {other}; use oracle or model"
        ))),
    }
}

fn api_key_for(provider: ProviderType) -> Option<String> {
    let var = match provider {
        ProviderType::Anthropic => "ANTHROPIC_API_KEY",
        ProviderType::OpenAI => "OPENAI_API_KEY",
        ProviderType::OpenRouter => "OPENROUTER_API_KEY",
        ProviderType::Google => "GOOGLE_API_KEY",
        _ => return None,
    };
    std::env::var(var).ok().filter(|k| !k.trim().is_empty())
}

fn load_cases(workspace: &Path, args: &[String]) -> Vec<TaskCase> {
    if let Some(file) = flag_value(args, "--tasks-file") {
        let path = workspace.join(file);
        return std::fs::read_to_string(&path)
            .map(|raw| parse_task_toml(&raw))
            .unwrap_or_default();
    }
    load_task_dir(&workspace.join("tests").join("tasks"))
}

pub async fn execute(args: &[String]) -> Result<()> {
    let executor = parse_executor(args)?;
    let context_mode = parse_context_mode(args)?;
    let json_out = args.iter().any(|a| a == "--json");
    let workspace = neuromesh_index::assert_safe_workspace(&std::env::current_dir()?)?;
    let cases = load_cases(&workspace, args);
    if cases.is_empty() {
        println!("No task cases under tests/tasks/ (or --tasks-file). Nothing to run.");
        return Ok(());
    }
    let only: Option<Vec<&str>> = flag_value(args, "--only").map(|s| s.split(',').collect());

    let provider: Option<Arc<dyn Provider>> = match &executor {
        Executor::Oracle => None,
        Executor::Model {
            provider: ProviderType::Mock,
            ..
        } => {
            let Some(file) = flag_value(args, "--mock-reply") else {
                return Err(NeuroMeshError::Config(
                    "provider=mock needs --mock-reply <file> with the canned reply".into(),
                ));
            };
            let reply = std::fs::read_to_string(workspace.join(file)).map_err(|e| {
                NeuroMeshError::Config(format!("cannot read --mock-reply {file}: {e}"))
            })?;
            Some(Arc::new(neuromesh_provider::mock::MockProvider::new(reply)))
        }
        Executor::Model { provider, model } => {
            let Some(api_key) = api_key_for(*provider) else {
                return Err(NeuroMeshError::Config(format!(
                    "executor=model needs an API key in the environment for {provider:?}"
                )));
            };
            let cfg = ProviderConfig {
                provider_type: *provider,
                api_key: Some(api_key),
                base_url: None,
                default_model: model.clone(),
                timeout_seconds: 600,
            };
            Some(ProviderFactory::create(&cfg))
        }
    };

    // `--judge-provider`/`--judge-model` pick the grader for QA-judge cases.
    // Default: the answerer's provider with a different model; a judge equal
    // to the answerer is refused unless `--allow-self-judge` is passed.
    let judge: Option<(Arc<dyn Provider>, String)> = match (&executor, &provider) {
        (Executor::Model { provider: p, model }, Some(answerer)) if *p != ProviderType::Mock => {
            let judge_provider = match flag_value(args, "--judge-provider") {
                None => *p,
                Some("anthropic") => ProviderType::Anthropic,
                Some("openai") => ProviderType::OpenAI,
                Some("openrouter") => ProviderType::OpenRouter,
                Some("google") => ProviderType::Google,
                Some(other) => {
                    return Err(NeuroMeshError::Config(format!(
                    "unknown --judge-provider {other}; use anthropic, openai, openrouter or google"
                )))
                }
            };
            let judge_model = flag_value(args, "--judge-model")
                .map(str::to_string)
                .unwrap_or_else(|| match judge_provider {
                    ProviderType::Anthropic => DEFAULT_ANTHROPIC_JUDGE_MODEL.into(),
                    _ => DEFAULT_OPENAI_JUDGE_MODEL.into(),
                });
            if judge_provider == *p
                && judge_model == *model
                && !args.iter().any(|a| a == "--allow-self-judge")
            {
                return Err(NeuroMeshError::Config(format!(
                    "judge {judge_model} is the answerer; pick --judge-model/--judge-provider or pass --allow-self-judge"
                )));
            }
            let judge_arc: Arc<dyn Provider> = if judge_provider == *p {
                Arc::clone(answerer)
            } else {
                let Some(api_key) = api_key_for(judge_provider) else {
                    return Err(NeuroMeshError::Config(format!(
                        "--judge-provider {judge_provider:?} needs its API key in the environment"
                    )));
                };
                ProviderFactory::create(&ProviderConfig {
                    provider_type: judge_provider,
                    api_key: Some(api_key),
                    base_url: None,
                    default_model: judge_model.clone(),
                    timeout_seconds: 600,
                })
            };
            Some((judge_arc, judge_model))
        }
        (Executor::Model { model, .. }, Some(answerer)) => {
            Some((Arc::clone(answerer), model.clone()))
        }
        _ => None,
    };

    println!(
        "\nNeuroMesh task harness — executor={} context={}{}",
        match &executor {
            Executor::Oracle => "oracle".to_string(),
            Executor::Model { provider, model } => format!("model ({provider:?} {model})"),
        },
        context_mode.label(),
        judge
            .as_ref()
            .map(|(p, m)| format!(" judge={}:{m}", p.name()))
            .unwrap_or_default()
    );
    println!(
        "{:<28} {:<32} {:>7} {:>7} {:>8} {:>8} {:>6}  Needs",
        "Task", "Repo", "Result", "Strict", "Packet", "Effect.", "ms"
    );
    println!("{}", "-".repeat(120));

    let mut outcomes: Vec<TaskOutcome> = Vec::new();
    let mut graphs: std::collections::BTreeMap<String, Arc<NeuralProjectGraph>> =
        std::collections::BTreeMap::new();
    for case in &cases {
        if let Some(only) = &only {
            if !only.contains(&case.id.as_str()) {
                continue;
            }
        }
        let repo = workspace.join(&case.repo);
        if !repo.is_dir() {
            println!("{:<28} {:<32} missing repository", case.id, case.repo);
            continue;
        }
        let graph = match graphs.get(&case.repo) {
            Some(g) => g.clone(),
            None => {
                let pid = ProjectId::new(&case.repo);
                let graph = Arc::new(NeuralProjectGraph::new(pid.clone()));
                graph.set_workspace(&repo);
                let scanned = ProjectWalker::new(repo.clone(), pid).scan()?;
                graph.ingest_workspace(&scanned);
                graphs.insert(case.repo.clone(), graph.clone());
                graph
            }
        };
        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry.clone());
        let signature = production_signature(&case.prompt);
        let started = Instant::now();
        let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);
        let latency_ms = started.elapsed().as_millis() as u64;
        let mut outcome = oracle_outcome(case, &view, &registry, latency_ms);

        if let (Some(provider), Executor::Model { model, .. }) = (&provider, &executor) {
            let context_text = match context_mode {
                ContextMode::Packet => render_packet_text(&view),
                ContextMode::WholeGoldFiles => whole_gold_files_context(case, &repo),
                ContextMode::Grep => grep_context(case, &repo),
            };
            outcome = if case.verify.is_some() {
                run_model_case(case, &repo, context_text, provider.as_ref(), model, outcome).await
            } else {
                let (judge_provider, judge_model) =
                    judge.as_ref().expect("judge set with model executor");
                run_qa_judge_case(
                    case,
                    context_text,
                    provider.as_ref(),
                    model,
                    judge_provider.as_ref(),
                    judge_model,
                    outcome,
                )
                .await
            };
        }

        let needs: Vec<String> = outcome
            .needs
            .iter()
            .map(|n| format!("{}={:?}", n.need, n.presence))
            .collect();
        println!(
            "{:<28} {:<32} {:>7} {:>7} {:>8} {:>8} {:>6}  {}{}",
            case.id,
            truncate(&case.repo, 32),
            if outcome.success { "ok" } else { "FAIL" },
            if outcome.strict_success { "ok" } else { "-" },
            outcome.packet_tokens,
            outcome.effective_tokens,
            outcome.latency_ms,
            needs.join(", "),
            if outcome.forbidden_hit.is_empty() {
                String::new()
            } else {
                format!("  FORBIDDEN={:?}", outcome.forbidden_hit)
            }
        );
        if let Some(note) = &outcome.note {
            println!("{:<28} note: {note}", "");
        }
        outcomes.push(outcome);
    }

    let summary: TaskSuiteSummary = summarize(&outcomes);
    println!(
        "\nTask success ({}): {}/{} = {:.3} (strict {:.3}) · mean effective tokens {:.0} · success per 1k tokens {:.3} · forbidden hits {}",
        summary.executor,
        summary.succeeded,
        summary.cases,
        summary.success_rate,
        summary.strict_success_rate,
        summary.mean_effective_tokens,
        summary.success_per_1k_tokens,
        summary.forbidden_hits
    );
    if json_out {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "summary": summary,
                "outcomes": outcomes,
            }))?
        );
    }
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let cut: String = s.chars().take(n - 1).collect();
        format!("{cut}…")
    }
}

const QA_ANSWER_SYSTEM_PROMPT: &str = "You are answering a question about a codebase you cannot \
browse directly. You see only the context below: a set of files, some with function bodies folded \
to markers (expand a marker only by reasoning about the visible signature and surrounding code — \
you cannot actually call an expand tool here). Answer the question specifically and concretely: \
name the exact functions/methods/files involved and describe what each one does and how they \
connect. Do not pad with generic statements that would apply to any codebase. If the context is not \
enough to answer confidently, say so plainly instead of guessing.";

/// The grader gets the gold `needs` (file::symbol identifiers) but no gold
/// prose, because none exists — task gold is authored as file/symbol
/// identifiers, not reference answers (see docs/planning/stage5-findings.fa.md
/// and the third-party gold sets). The judge is asked to use its own
/// knowledge of what a symbol with that name/location/signature-shape would
/// plausibly do, the same way a human reviewer with domain knowledge but no
/// repository access would grade an answer. This is a real limitation, not a
/// design nicety: a judge with no ground-truth description can be fooled by a
/// fluent but wrong answer that happens to name the right symbols. Treat a
/// QA-judge task_success number as directional, not as strong as the
/// verify-command path, until gold answers are authored.
const QA_JUDGE_SYSTEM_PROMPT: &str = "You are grading whether a candidate answer correctly and \
specifically explains a set of named code elements, based only on the question, the list of \
elements, and your own knowledge of typical code at those names/locations. A vague answer that \
merely repeats the element names without describing what they actually do must fail. An answer \
that gets the mechanism wrong must fail. An answer that plainly says the context was insufficient \
must fail (it did not complete the task). Reply with exactly one line: PASS or FAIL, followed by a \
dash and a one-sentence reason — nothing else.";

async fn run_qa_judge_case(
    case: &TaskCase,
    context_text: String,
    provider: &dyn Provider,
    model: &str,
    judge_provider: &dyn Provider,
    judge_model: &str,
    mut outcome: TaskOutcome,
) -> TaskOutcome {
    outcome.executor = format!(
        "{}:{model} (qa-judge {}:{judge_model})",
        provider.name(),
        judge_provider.name()
    );
    let context_tokens = TokenCounter::count_tokens(&context_text);
    let answer_request = ProviderRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: QA_ANSWER_SYSTEM_PROMPT.into(),
                name: None,
            },
            ChatMessage {
                role: "user".into(),
                content: format!("## Question\n{}\n\n## Context\n{context_text}", case.prompt),
                name: None,
            },
        ],
        temperature: None,
        max_tokens: Some(4_000),
        stream: false,
        api_key: None,
    };
    let answer_response = match provider.send(&answer_request).await {
        Ok(r) => r,
        Err(e) => {
            outcome.success = false;
            outcome.strict_success = false;
            outcome.note = Some(format!("provider error (answer): {e}"));
            return outcome;
        }
    };
    let answer = answer_response.content.clone();

    let needs_list = case
        .needs
        .iter()
        .map(|n| format!("- {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let judge_user = format!(
        "## Question asked of the candidate\n{}\n\n## Code elements the answer must correctly explain\n{needs_list}\n\n## Candidate answer\n{answer}",
        case.prompt
    );
    let judge_request = ProviderRequest {
        model: judge_model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: QA_JUDGE_SYSTEM_PROMPT.into(),
                name: None,
            },
            ChatMessage {
                role: "user".into(),
                content: judge_user.clone(),
                name: None,
            },
        ],
        temperature: None,
        max_tokens: Some(4_000),
        stream: false,
        api_key: None,
    };
    let judge_response = match judge_provider.send(&judge_request).await {
        Ok(r) => r,
        Err(e) => {
            outcome.success = false;
            outcome.strict_success = false;
            outcome.note = Some(format!("provider error (judge): {e}"));
            return outcome;
        }
    };
    let verdict = judge_response.content.trim();
    let passed = verdict
        .split(['-', '—', ':'])
        .next()
        .unwrap_or(verdict)
        .trim()
        .eq_ignore_ascii_case("PASS");

    outcome.success = passed;
    outcome.strict_success = passed && outcome.strict_success;
    let judge_prompt_tokens = TokenCounter::count_tokens(&judge_user);
    outcome.effective_tokens = context_tokens
        + answer_response.usage.completion_tokens
        + judge_prompt_tokens
        + judge_response.usage.completion_tokens;
    outcome.note = Some(format!(
        "qa-judge: {verdict} · context {context_tokens} tok, answer {} tok, judge {} tok",
        answer_response.usage.completion_tokens, judge_response.usage.completion_tokens
    ));
    outcome
}

const PATCH_SYSTEM_PROMPT: &str = "You are completing a coding task in a repository you cannot browse. \
You see only the context packet below: a set of files, some with function bodies folded to markers. \
Reply with exactly one unified diff (git format, `diff --git a/<path> b/<path>` headers, paths relative \
to the repository root) that completes the task, and nothing else — no prose, no code fences. \
Only touch files shown in the packet or create new files. If the packet is not enough to complete \
the task safely, reply with the single line `INSUFFICIENT: <what is missing>`.";

async fn run_model_case(
    case: &TaskCase,
    repo: &Path,
    context_text: String,
    provider: &dyn Provider,
    model: &str,
    mut outcome: TaskOutcome,
) -> TaskOutcome {
    outcome.executor = format!("{}:{model}", provider.name());
    let context_tokens = TokenCounter::count_tokens(&context_text);
    let request = ProviderRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: PATCH_SYSTEM_PROMPT.into(),
                name: None,
            },
            ChatMessage {
                role: "user".into(),
                content: format!(
                    "## Task\n{}\n\n## Context packet\n{context_text}",
                    case.prompt
                ),
                name: None,
            },
        ],
        temperature: None,
        max_tokens: Some(16_000),
        stream: false,
        api_key: None,
    };
    let response = match provider.send(&request).await {
        Ok(r) => r,
        Err(e) => {
            outcome.success = false;
            outcome.strict_success = false;
            outcome.note = Some(format!("provider error: {e}"));
            return outcome;
        }
    };
    let reply = response.content.clone();
    if reply.len() > MAX_PATCH_BYTES {
        outcome.success = false;
        outcome.strict_success = false;
        outcome.note = Some(format!("reply too large ({} bytes)", reply.len()));
        return outcome;
    }
    if let Some(rest) = reply.trim_start().strip_prefix("INSUFFICIENT:") {
        outcome.success = false;
        outcome.strict_success = false;
        outcome.note = Some(format!("model declined: {}", rest.trim()));
        return outcome;
    }
    // `NM_TASK_REPLY_DIR`: keep every raw reply — the only way to see why a
    // patch did not apply.
    if let Ok(dir) = std::env::var("NM_TASK_REPLY_DIR") {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            Path::new(&dir).join(format!("{}.reply.txt", case.id)),
            &reply,
        );
    }
    let patch = number_hunk_headers(&strip_fences(&reply), repo);

    let scratch = std::env::temp_dir().join(format!(
        "nm-task-{}-{}",
        std::process::id(),
        sanitize(&case.id)
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    if let Err(e) = copy_tree(repo, &scratch) {
        outcome.success = false;
        outcome.strict_success = false;
        outcome.note = Some(format!("scratch copy failed: {e}"));
        return outcome;
    }
    let patch_path = scratch.join(".nm-task.patch");
    if std::fs::write(&patch_path, patch.as_bytes()).is_err() {
        outcome.note = Some("could not write patch".into());
        outcome.success = false;
        outcome.strict_success = false;
        return outcome;
    }
    let mut apply = git_apply(&scratch).await;
    // The packet the model read is a skeleton: blank lines between blocks are
    // not rendered, so a correct diff can still miss its context by a blank
    // line. Second try on a scratch copy with the blank lines of the touched
    // files removed — the same program to every interpreter here, and the
    // scratch is thrown away after `verify`.
    if matches!(&apply, Ok(out) if !out.status.success()) {
        let touched = patched_paths(&patch);
        let mut normalized = 0;
        for rel in &touched {
            let path = scratch.join(rel);
            if let Ok(text) = std::fs::read_to_string(&path) {
                let kept: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
                if std::fs::write(
                    &path,
                    kept.join(
                        "
",
                    ) + "
",
                )
                .is_ok()
                {
                    normalized += 1;
                }
            }
        }
        if normalized > 0 {
            apply = git_apply(&scratch).await;
            if matches!(&apply, Ok(out) if out.status.success()) {
                outcome.note = Some("applied after blank-line normalization".into());
            }
        }
    }
    match apply {
        Ok(out) if out.status.success() => {}
        Ok(out) => {
            outcome.success = false;
            outcome.strict_success = false;
            outcome.note = Some(format!(
                "patch did not apply: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
            let _ = std::fs::remove_dir_all(&scratch);
            return outcome;
        }
        Err(e) => {
            outcome.success = false;
            outcome.strict_success = false;
            outcome.note = Some(format!("git apply failed to start: {e}"));
            let _ = std::fs::remove_dir_all(&scratch);
            return outcome;
        }
    }
    let _ = std::fs::remove_file(&patch_path);

    let verify = case.verify.clone().unwrap_or_default();
    // `verify` is written for a POSIX shell (quoting, `&&`). On Windows use
    // the `sh` that ships with Git when it is on PATH; `cmd` only as a last
    // resort, where quoting will differ.
    let (program, flag) = if !cfg!(windows) || sh_available() {
        ("sh", "-c")
    } else {
        ("cmd", "/C")
    };
    let run = tokio::time::timeout(
        VERIFY_TIMEOUT,
        tokio::process::Command::new(program)
            .arg(flag)
            .arg(&verify)
            .current_dir(&scratch)
            .output(),
    )
    .await;
    let (exit, tail) = match run {
        Ok(Ok(out)) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            (out.status.code().unwrap_or(-1), last_lines(&combined, 3))
        }
        Ok(Err(e)) => (-1, format!("verify failed to start: {e}")),
        Err(_) => (-1, "verify timed out".into()),
    };
    let _ = std::fs::remove_dir_all(&scratch);

    outcome.verify_exit = Some(exit);
    outcome.success = exit == 0;
    outcome.strict_success = exit == 0 && outcome.strict_success;
    // Charged against the context actually sent, not the packet's own token
    // count — for the whole-gold-files/grep baselines those two can differ
    // a lot, and that difference is the entire point of the comparison.
    outcome.effective_tokens = context_tokens + response.usage.completion_tokens;
    outcome.note = Some(format!(
        "verify exit {exit}; context {context_tokens} tok; model in/out tokens {}/{}; {}",
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
        tail.replace('\n', " | ")
    ));
    outcome
}

/// Unwrap a fenced reply without disturbing the patch itself: a unified
/// diff's context lines can be a single space, so nothing here trims
/// whitespace off the ends of lines — only fence lines and blank tails go.
async fn git_apply(scratch: &Path) -> std::io::Result<std::process::Output> {
    // `git apply` refuses absolute paths and `..` components on its own; the
    // scratch directory is the only thing it can touch. `--ignore-whitespace`:
    // a CRLF checkout and a model that writes LF is still the same diff.
    tokio::process::Command::new("git")
        .arg("apply")
        .arg("--whitespace=nowarn")
        .arg("--ignore-whitespace")
        // Models miscount hunk lengths; `--recount` trusts the lines, not the header.
        .arg("--recount")
        .arg(".nm-task.patch")
        .current_dir(scratch)
        .output()
        .await
}

/// `+++ b/src/x.js` lines of a unified diff → `src/x.js`.
/// A hunk header the model left as `@@ ... @@` (or any header without
/// numbers) is "garbage" to `git apply`, even with `--recount`. The lines
/// under it are usually right, so the header is rebuilt: the first old-side
/// line of the hunk is looked up in the file under `repo` to find the start
/// line; `--recount` fixes the counts. Numbered headers are left alone (F76).
fn number_hunk_headers(patch: &str, repo: &Path) -> String {
    let mut out: Vec<String> = Vec::new();
    let lines: Vec<&str> = patch.lines().collect();
    let mut file: Option<Vec<String>> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(rest) = line.strip_prefix("+++ ") {
            let rel = rest.trim().trim_start_matches("b/");
            file = std::fs::read_to_string(repo.join(rel))
                .ok()
                .map(|t| t.lines().map(|l| l.to_string()).collect());
            out.push(line.to_string());
            i += 1;
            continue;
        }
        let numbered = line.split_whitespace().nth(1).is_some_and(|s| {
            s.starts_with('-') && s[1..].starts_with(|c: char| c.is_ascii_digit())
        });
        let placeholder = line.starts_with("@@") && !numbered;
        if !placeholder {
            out.push(line.to_string());
            i += 1;
            continue;
        }
        // Old-side lines of this hunk: context and `-`, until the next header/file.
        let body: Vec<&str> = lines[i + 1..]
            .iter()
            .take_while(|l| {
                !l.starts_with("@@") && !l.starts_with("diff ") && !l.starts_with("--- ")
            })
            .copied()
            .collect();
        let old: Vec<&str> = body
            .iter()
            .filter(|l| !l.starts_with('+'))
            .map(|l| l.get(1..).unwrap_or(""))
            .collect();
        let start = file
            .as_ref()
            .and_then(|f| {
                let first = old.iter().find(|l| !l.trim().is_empty())?;
                let idx_in_old = old.iter().position(|l| l == first)?;
                f.iter()
                    .position(|l| l.trim_end() == first.trim_end())
                    .map(|p| p.saturating_sub(idx_in_old) + 1)
            })
            .unwrap_or(1);
        let new_len = body.iter().filter(|l| !l.starts_with('-')).count();
        out.push(format!("@@ -{start},{} +{start},{new_len} @@", old.len()));
        i += 1;
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

fn patched_paths(patch: &str) -> Vec<String> {
    patch
        .lines()
        .filter_map(|l| l.strip_prefix("+++ "))
        .map(|p| p.trim().trim_start_matches("b/").to_string())
        .filter(|p| !p.is_empty() && p != "/dev/null" && !p.contains(".."))
        .collect()
}

fn strip_fences(reply: &str) -> String {
    // A model that talks before the diff ("Here is the patch:") still sent a
    // diff: start at the first line that is one. Text after the closing fence
    // is dropped with it.
    let start = reply
        .lines()
        .position(|l| {
            l.starts_with("diff --git ") || l.starts_with("--- a/") || l.starts_with("--- ")
        })
        .unwrap_or(0);
    let mut lines: Vec<&str> = reply.lines().skip(start).collect();
    if let Some(end) = lines.iter().position(|l| l.trim() == "```") {
        lines.truncate(end);
    }
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    if lines
        .first()
        .is_some_and(|l| l.trim_start().starts_with("```"))
    {
        lines.remove(0);
    }
    if lines.last().is_some_and(|l| l.trim() == "```") {
        lines.pop();
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn last_lines(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    lines
        .iter()
        .rev()
        .take(n)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" || name == "target" || name == "node_modules" || name == ".venv" {
            continue;
        }
        let to: PathBuf = dst.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

fn sh_available() -> bool {
    std::process::Command::new("sh")
        .arg("-c")
        .arg("exit 0")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod hunk_header_tests {
    use super::number_hunk_headers;

    #[test]
    fn placeholder_header_gets_numbers_from_the_file() {
        let dir = std::env::temp_dir().join(format!("nm-hunk-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/pricing.js"),
            "function lineTotal(a, b) {\n    return a * b;\n}\n\n// BUG: tax\nfunction totalFor(subtotal, taxRate) {\n    return subtotal;\n}\n",
        )
        .unwrap();
        let patch = "diff --git a/src/pricing.js b/src/pricing.js\n--- a/src/pricing.js\n+++ b/src/pricing.js\n@@ ... @@ function lineTotal(a, b) {\n \n // BUG: tax\n function totalFor(subtotal, taxRate) {\n-    return subtotal;\n+    return subtotal * (1 + taxRate);\n }\n";
        let fixed = number_hunk_headers(patch, &dir);
        assert!(fixed.contains("@@ -4,5 +4,5 @@"), "{fixed}");
        // A numbered header is left alone.
        let numbered = "--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n-a\n+b\n c\n";
        assert_eq!(
            number_hunk_headers(numbered, &dir).trim_end(),
            numbered.trim_end()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

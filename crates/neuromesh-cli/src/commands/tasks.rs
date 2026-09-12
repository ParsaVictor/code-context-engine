//! `neuromesh eval --tasks`: the task-success harness.
//!
//! Reads `tests/tasks/*.toml` (or `--tasks-file <path>`), indexes each case's
//! repository, builds the packet through the production signature, and
//! scores it with an executor:
//!
//! - `--executor oracle` (default; what CI runs): symbol-level sufficiency,
//!   no model. See `neuromesh_context::task_harness`.
//! - `--executor model [--provider anthropic|openai|openrouter|google]
//!   [--model <id>]`: asks a model for a patch from the packet alone, applies
//!   it to a scratch copy of the repository and runs the case's `verify`
//!   command. Needs the provider's API key in the environment
//!   (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, ...). Cases without `verify` are
//!   scored by the oracle and reported as such.
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
    NeuroMeshError, OptimizationMode, ProjectId, ProviderConfig, ProviderType, Result,
};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use neuromesh_provider::{ChatMessage, Provider, ProviderFactory, ProviderRequest};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DEFAULT_ANTHROPIC_MODEL: &str = "claude-opus-5";
const DEFAULT_OPENAI_MODEL: &str = "gpt-4.1";
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

    println!(
        "\nNeuroMesh task harness — executor={}",
        match &executor {
            Executor::Oracle => "oracle".to_string(),
            Executor::Model { provider, model } => format!("model ({provider:?} {model})"),
        }
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
            if case.verify.is_some() {
                outcome =
                    run_model_case(case, &repo, &view, provider.as_ref(), model, outcome).await;
            } else {
                outcome.note = Some("no verify command; scored by oracle".into());
            }
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

const PATCH_SYSTEM_PROMPT: &str = "You are completing a coding task in a repository you cannot browse. \
You see only the context packet below: a set of files, some with function bodies folded to markers. \
Reply with exactly one unified diff (git format, `diff --git a/<path> b/<path>` headers, paths relative \
to the repository root) that completes the task, and nothing else — no prose, no code fences. \
Only touch files shown in the packet or create new files. If the packet is not enough to complete \
the task safely, reply with the single line `INSUFFICIENT: <what is missing>`.";

async fn run_model_case(
    case: &TaskCase,
    repo: &Path,
    view: &neuromesh_core::ContextView,
    provider: &dyn Provider,
    model: &str,
    mut outcome: TaskOutcome,
) -> TaskOutcome {
    outcome.executor = format!("{}:{model}", provider.name());
    let packet = render_packet_text(view);
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
                content: format!("## Task\n{}\n\n## Context packet\n{packet}", case.prompt),
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
    let patch = strip_fences(&reply);

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
    // `git apply` refuses absolute paths and `..` components on its own; the
    // scratch directory is the only thing it can touch.
    let apply = tokio::process::Command::new("git")
        .arg("apply")
        .arg("--whitespace=nowarn")
        .arg(".nm-task.patch")
        .current_dir(&scratch)
        .output()
        .await;
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
    outcome.effective_tokens = outcome.packet_tokens + response.usage.completion_tokens;
    outcome.note = Some(format!(
        "verify exit {exit}; model in/out tokens {}/{}; {}",
        response.usage.prompt_tokens,
        response.usage.completion_tokens,
        tail.replace('\n', " | ")
    ));
    outcome
}

/// Unwrap a fenced reply without disturbing the patch itself: a unified
/// diff's context lines can be a single space, so nothing here trims
/// whitespace off the ends of lines — only fence lines and blank tails go.
fn strip_fences(reply: &str) -> String {
    let mut lines: Vec<&str> = reply.lines().collect();
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

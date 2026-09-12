//! Task-success harness: does a packet let a model finish the task?
//!
//! Recall and precision score files. A task is finished with symbols: the
//! body of `SmsStore.save`, not the file it lives in. A packet can contain
//! the right file with the one needed function folded to a marker, and file
//! recall calls that a hit. This harness scores what the model would have
//! had to work with.
//!
//! A task case names the evidence the task needs (`file::symbol`, or a bare
//! file), what must not be shipped, and optionally a command that proves the
//! task done. Two executors read the same cases:
//!
//! - **oracle** (offline, runs in CI): success when every needed symbol is
//!   in the packet, unfolded or reachable by expanding a fold. The cost of
//!   the expansions is charged to the packet, because an agent pays it. This
//!   is a *sufficiency* oracle, not a model run; the report labels it so.
//! - **model** (`neuromesh eval --tasks --executor <provider>`): renders the
//!   packet, asks a model for a patch, applies it to a scratch copy of the
//!   repository and runs `verify`. Success is the exit code. Needs an API
//!   key and is never run by CI.
//!
//! Cases live in `tests/tasks/*.toml`, one `[[task]]` per case:
//!
//! ```toml
//! [[task]]
//! id = "sms_receive"
//! repo = "tests/fixtures/mini-python"
//! prompt = "How does on_receive use SmsStore.save?"
//! needs = ["src/receiver.py::on_receive", "src/sms_store.py::SmsStore.save"]
//! forbidden = ["src/inbox_store.py"]
//! verify = "python -m pytest -q"
//! ```

use crate::registry::ReversibleContextRegistry;
use neuromesh_core::{ContextView, NodeType, TokenCounter};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One piece of evidence a task needs: a file, or a symbol inside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Need {
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

impl Need {
    pub fn parse(raw: &str) -> Self {
        match raw.split_once("::") {
            Some((file, symbol)) if !symbol.trim().is_empty() => Self {
                file: file.trim().replace('\\', "/"),
                symbol: Some(symbol.trim().to_string()),
            },
            _ => Self {
                file: raw.trim().replace('\\', "/"),
                symbol: None,
            },
        }
    }
}

impl std::fmt::Display for Need {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.symbol {
            Some(symbol) => write!(f, "{}::{symbol}", self.file),
            None => write!(f, "{}", self.file),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskCase {
    pub id: String,
    /// Repository root, relative to the workspace the harness runs in.
    pub repo: String,
    pub prompt: String,
    pub needs: Vec<Need>,
    #[serde(default)]
    pub forbidden: Vec<String>,
    /// Shell command run in a scratch copy after the model's patch; exit 0 = done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify: Option<String>,
}

/// How a needed item showed up in the packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    /// Body in the packet as shipped.
    Unfolded,
    /// Only the marker shipped; one `expand_fold` call away.
    Folded,
    /// The file is there but the symbol is not in it (or not indexed).
    SymbolMissing,
    /// Not in the packet at all.
    FileMissing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeedReport {
    pub need: String,
    pub presence: Presence,
    /// Tokens an agent would spend expanding this fold, when `Folded`.
    pub expansion_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutcome {
    pub id: String,
    /// `oracle`, or the provider/model that produced the patch.
    pub executor: String,
    /// Every need reachable (unfolded or folded) and nothing forbidden shipped.
    pub success: bool,
    /// Every need in the packet as shipped, no expansion required.
    pub strict_success: bool,
    pub needs: Vec<NeedReport>,
    pub forbidden_hit: Vec<String>,
    pub packet_tokens: usize,
    /// Packet plus the expansions the needs required.
    pub effective_tokens: usize,
    pub latency_ms: u64,
    /// Model executor only: what `verify` said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_exit: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Which files and folds the packet holds — the shape both executors read.
struct PacketIndex {
    files: Vec<(String, Vec<String>)>,
}

impl PacketIndex {
    fn of(view: &ContextView) -> Self {
        let files = view
            .active_nodes
            .iter()
            .filter(|n| n.node.node_type == NodeType::File)
            .map(|n| {
                (
                    n.node.file_path.to_string_lossy().replace('\\', "/"),
                    n.folded_symbols.clone(),
                )
            })
            .collect();
        Self { files }
    }

    fn file(&self, wanted: &str) -> Option<&(String, Vec<String>)> {
        self.files
            .iter()
            .find(|(path, _)| path == wanted || path.ends_with(&format!("/{wanted}")))
    }
}

fn symbol_matches(folded: &str, wanted: &str) -> bool {
    folded == wanted
        || folded.rsplit(['.', ':']).next() == wanted.rsplit(['.', ':']).next()
            && (folded.ends_with(wanted) || wanted.ends_with(folded))
}

/// Does the shipped code for a file contain a *definition* of `symbol`?
///
/// Language-agnostic on purpose, and deliberately not fooled by a call site:
/// `train_one_epoch(model, loader)` inside `main` is not a definition of
/// `train_one_epoch`. A line counts when the symbol's last path segment
/// appears as a whole word and either a definition keyword precedes it on
/// that line (`def`, `fn`, `class`, `function`, `const`, `public void`, ...)
/// or the name opens the line and the line opens a body (`{`, `:`, `=>`),
/// which is how method shorthand and Kotlin/Java-style members look. The
/// fold list is the authority for "shipped but folded"; this only decides
/// "present at all".
fn code_defines_symbol(code: &str, symbol: &str) -> bool {
    const DEF_WORDS: &[&str] = &[
        "def",
        "fn",
        "function",
        "fun",
        "func",
        "class",
        "struct",
        "enum",
        "trait",
        "interface",
        "impl",
        "type",
        "const",
        "let",
        "var",
        "val",
        "static",
        "public",
        "private",
        "protected",
        "export",
        "async",
        "override",
        "pub",
        "sub",
        "proc",
        "method",
        "object",
        "record",
    ];
    let leaf = symbol.rsplit(['.', ':']).next().unwrap_or(symbol);
    if leaf.is_empty() {
        return false;
    }
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    for line in code.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*") {
            continue;
        }
        let mut search_from = 0usize;
        while let Some(rel) = trimmed[search_from..].find(leaf) {
            let at = search_from + rel;
            let end = at + leaf.len();
            search_from = end;
            let before_ok = at == 0 || !trimmed[..at].ends_with(is_ident);
            let after_ok = end >= trimmed.len() || !trimmed[end..].starts_with(is_ident);
            if !before_ok || !after_ok {
                continue;
            }
            let before = trimmed[..at].trim_end();
            if before.ends_with('.') || before.ends_with("->") || before.ends_with("::") {
                continue;
            }
            let after = trimmed[end..].trim_start();
            let opens = after.starts_with('(')
                || after.starts_with('<')
                || after.starts_with(':')
                || after.starts_with('=')
                || after.starts_with('{');
            if !opens && !after.is_empty() {
                continue;
            }
            let keyword_before = before
                .split(|c: char| !is_ident(c))
                .any(|w| DEF_WORDS.contains(&w));
            let body_opener =
                trimmed.ends_with('{') || trimmed.ends_with(':') || trimmed.ends_with("=>");
            if keyword_before || (before.is_empty() && body_opener) {
                return true;
            }
        }
    }
    false
}

/// Score a packet against a case without running a model.
pub fn oracle_outcome(
    case: &TaskCase,
    view: &ContextView,
    registry: &ReversibleContextRegistry,
    latency_ms: u64,
) -> TaskOutcome {
    let index = PacketIndex::of(view);
    let mut needs = Vec::with_capacity(case.needs.len());
    let mut expansion_total = 0usize;
    for need in &case.needs {
        let Some((path, folded)) = index.file(&need.file) else {
            needs.push(NeedReport {
                need: need.to_string(),
                presence: Presence::FileMissing,
                expansion_tokens: 0,
            });
            continue;
        };
        let Some(symbol) = &need.symbol else {
            needs.push(NeedReport {
                need: need.to_string(),
                presence: Presence::Unfolded,
                expansion_tokens: 0,
            });
            continue;
        };
        let folded_here = if symbol.contains(['.', ':']) {
            find_fold(registry, view, path, symbol).is_some()
        } else {
            folded.iter().any(|f| symbol_matches(f, symbol))
        };
        if folded_here {
            let expansion_tokens = fold_expansion_tokens(registry, view, path, symbol);
            expansion_total += expansion_tokens;
            needs.push(NeedReport {
                need: need.to_string(),
                presence: Presence::Folded,
                expansion_tokens,
            });
            continue;
        }
        let code = view
            .active_nodes
            .iter()
            .find(|n| {
                n.node.node_type == NodeType::File
                    && n.node.file_path.to_string_lossy().replace('\\', "/") == *path
            })
            .and_then(|n| n.node.content.clone())
            .unwrap_or_default();
        let presence = if code_defines_symbol(&code, symbol) {
            Presence::Unfolded
        } else {
            Presence::SymbolMissing
        };
        needs.push(NeedReport {
            need: need.to_string(),
            presence,
            expansion_tokens: 0,
        });
    }
    let forbidden_hit: Vec<String> = case
        .forbidden
        .iter()
        .filter(|f| index.file(&f.replace('\\', "/")).is_some())
        .cloned()
        .collect();
    let reachable = needs
        .iter()
        .all(|n| matches!(n.presence, Presence::Unfolded | Presence::Folded));
    let strict = needs.iter().all(|n| n.presence == Presence::Unfolded);
    TaskOutcome {
        id: case.id.clone(),
        executor: "oracle".into(),
        success: reachable && forbidden_hit.is_empty() && !case.needs.is_empty(),
        strict_success: strict && forbidden_hit.is_empty() && !case.needs.is_empty(),
        needs,
        forbidden_hit,
        packet_tokens: view.active_tokens,
        effective_tokens: view.active_tokens + expansion_total,
        latency_ms,
        verify_exit: None,
        note: None,
    }
}

/// The fold registered for `symbol` in `path`, if the packet folded it.
///
/// The view's bare `folded_symbols` cannot tell `GPT.forward` from
/// `CausalSelfAttention.forward`; the registry's folds carry the owner, so
/// a qualified need is matched on owner and leaf, an unqualified one on the
/// leaf alone.
fn find_fold(
    registry: &ReversibleContextRegistry,
    view: &ContextView,
    path: &str,
    symbol: &str,
) -> Option<crate::registry::StoredFold> {
    let (wanted_owner, leaf) = match symbol.rsplit_once(['.', ':']) {
        Some((owner, leaf)) => (Some(owner.trim_end_matches(':').to_lowercase()), leaf),
        None => (None, symbol),
    };
    view.fold_ids
        .iter()
        .filter_map(|id| registry.get_fold(id))
        .filter(|stored| stored.file_path.to_string_lossy().replace('\\', "/") == path)
        .filter(|stored| {
            let fold_leaf = stored
                .fold
                .symbol_name
                .rsplit(['.', ':'])
                .next()
                .unwrap_or(&stored.fold.symbol_name);
            if fold_leaf != leaf {
                return false;
            }
            match (&wanted_owner, &stored.fold.owner) {
                (Some(want), Some(have)) => have.to_lowercase() == *want,
                (Some(_), None) => false,
                (None, _) => true,
            }
        })
        .max_by_key(|stored| TokenCounter::count_tokens(&stored.fold.original_body))
}

fn fold_expansion_tokens(
    registry: &ReversibleContextRegistry,
    view: &ContextView,
    path: &str,
    symbol: &str,
) -> usize {
    find_fold(registry, view, path, symbol)
        .map(|stored| TokenCounter::count_tokens(&stored.fold.original_body))
        .unwrap_or(0)
}

/// The packet as a model would read it: one block per file, folded code as
/// shipped. Sidecar files are marked so the model knows they are context, not
/// the target.
pub fn render_packet_text(view: &ContextView) -> String {
    let mut out = String::new();
    for node in view
        .active_nodes
        .iter()
        .filter(|n| n.node.node_type == NodeType::File)
    {
        let path = node.node.file_path.to_string_lossy().replace('\\', "/");
        let tag = if node.sidecar { " (related)" } else { "" };
        out.push_str(&format!("### {path}{tag}\n```\n"));
        out.push_str(node.node.content.as_deref().unwrap_or(""));
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("```\n\n");
    }
    out
}

/// Aggregate over a run: the numbers the release gate reads.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskSuiteSummary {
    pub executor: String,
    pub cases: usize,
    pub succeeded: usize,
    pub strict_succeeded: usize,
    pub success_rate: f32,
    pub strict_success_rate: f32,
    pub mean_effective_tokens: f32,
    /// Successes per thousand effective tokens — the project's headline metric.
    pub success_per_1k_tokens: f32,
    pub forbidden_hits: usize,
}

pub fn summarize(outcomes: &[TaskOutcome]) -> TaskSuiteSummary {
    let cases = outcomes.len();
    let succeeded = outcomes.iter().filter(|o| o.success).count();
    let strict_succeeded = outcomes.iter().filter(|o| o.strict_success).count();
    let tokens: usize = outcomes.iter().map(|o| o.effective_tokens).sum();
    let n = cases.max(1) as f32;
    let success_rate = succeeded as f32 / n;
    let mean_effective_tokens = tokens as f32 / n;
    let success_per_1k_tokens = if tokens > 0 {
        succeeded as f32 / (tokens as f32 / 1000.0)
    } else {
        0.0
    };
    TaskSuiteSummary {
        executor: outcomes
            .first()
            .map(|o| o.executor.clone())
            .unwrap_or_default(),
        cases,
        succeeded,
        strict_succeeded,
        success_rate,
        strict_success_rate: strict_succeeded as f32 / n,
        mean_effective_tokens,
        success_per_1k_tokens,
        forbidden_hits: outcomes
            .iter()
            .filter(|o| !o.forbidden_hit.is_empty())
            .count(),
    }
}

/// Every `*.toml` under `dir`, in name order.
pub fn load_task_dir(dir: &Path) -> Vec<TaskCase> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .flat_map(|raw| parse_task_toml(&raw))
        .collect()
}

/// The subset of TOML these files use: `[[task]]` tables, string values,
/// and string arrays that may span lines. Same dialect as `gold_tasks.toml`.
pub fn parse_task_toml(raw: &str) -> Vec<TaskCase> {
    let mut cases = Vec::new();
    let mut current: Option<TaskCase> = None;
    let mut array_buf: Option<(String, String)> = None;
    let finish_array = |case: &mut TaskCase, key: &str, buf: &str| {
        let items = crate::gold::parse_string_array(buf);
        match key {
            "needs" => case.needs = items.iter().map(|s| Need::parse(s)).collect(),
            "forbidden" => case.forbidden = items,
            _ => {}
        }
    };
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, buf)) = array_buf.as_mut() {
            buf.push(' ');
            buf.push_str(line);
            if line.contains(']') {
                if let Some(case) = current.as_mut() {
                    finish_array(case, key, buf);
                }
                array_buf = None;
            }
            continue;
        }
        if line == "[[task]]" {
            if let Some(case) = current.take() {
                cases.push(case);
            }
            current = Some(TaskCase::default());
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let Some(case) = current.as_mut() else {
            continue;
        };
        if value.starts_with('[') {
            if value.contains(']') {
                finish_array(case, key, value);
            } else {
                array_buf = Some((key.to_string(), value.to_string()));
            }
            continue;
        }
        let value = crate::gold::unquote(value);
        match key {
            "id" => case.id = value,
            "repo" => case.repo = value,
            "prompt" => case.prompt = value,
            "verify" => case.verify = Some(value),
            _ => {}
        }
    }
    if let Some(case) = current.take() {
        cases.push(case);
    }
    cases.retain(|c| !c.id.is_empty() && !c.prompt.is_empty());
    cases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_needs_with_symbols_and_multiline_arrays() {
        let raw = r#"
[[task]]
id = "a"
repo = "tests/fixtures/mini-python"
prompt = "How does on_receive use SmsStore.save?"
needs = [
  "src/receiver.py::on_receive",
  "src/sms_store.py::SmsStore.save",
]
forbidden = ["src/inbox_store.py"]
verify = "python -m pytest -q"

[[task]]
id = "b"
repo = "x"
prompt = "p"
needs = ["lib/a.js"]
"#;
        let cases = parse_task_toml(raw);
        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0].needs.len(), 2);
        assert_eq!(cases[0].needs[1].symbol.as_deref(), Some("SmsStore.save"));
        assert_eq!(cases[0].forbidden, vec!["src/inbox_store.py"]);
        assert_eq!(cases[0].verify.as_deref(), Some("python -m pytest -q"));
        assert_eq!(cases[1].needs[0].symbol, None);
    }

    #[test]
    fn symbol_match_accepts_owner_prefix_either_way() {
        assert!(symbol_matches("SmsStore.save", "save"));
        assert!(symbol_matches("save", "SmsStore.save"));
        assert!(symbol_matches("SmsStore.save", "SmsStore.save"));
        assert!(!symbol_matches("InboxStore.load", "SmsStore.save"));
    }

    #[test]
    fn code_definition_is_not_a_call_site() {
        assert!(code_defines_symbol(
            "def on_receive(x):\n    pass",
            "on_receive"
        ));
        assert!(!code_defines_symbol("def on_receive_all(x):", "on_receive"));
        assert!(code_defines_symbol(
            "class SmsStore:\n  def save(self)",
            "SmsStore.save"
        ));
        // A call inside another body is not a definition.
        assert!(!code_defines_symbol(
            "def main():\n    train_one_epoch(model, loader, optimizer)\n",
            "train_one_epoch"
        ));
        assert!(!code_defines_symbol(
            "    loss = detection_loss(p, t)",
            "detection_loss"
        ));
        assert!(code_defines_symbol(
            "pub fn solve_physarum_tube(&self) {",
            "solve_physarum_tube"
        ));
        assert!(code_defines_symbol(
            "    setQty(productId, qty) {",
            "setQty"
        ));
        assert!(code_defines_symbol(
            "export const StatCard: FC<{ label: string }> = ({ label }) => {",
            "StatCard"
        ));
        assert!(code_defines_symbol(
            "app.handle = function handle(req, res) {",
            "handle"
        ));
        assert!(code_defines_symbol(
            "    fun onReceive(body: String) {",
            "onReceive"
        ));
    }
}

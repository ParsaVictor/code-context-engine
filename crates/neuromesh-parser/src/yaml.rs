//! YAML as configuration: every mapping key becomes a `Config` node, the way
//! `json.rs` treats a JSON object, so a question that names a key
//! (`learning_rate`, `lr0`, `batch_size`) has a node to land on and the code
//! that reads it (`config_reads.rs`) has a target for its `Parameterizes` edge.
//!
//! Indentation-based on purpose — no YAML dependency, no anchors/aliases, no
//! multi-document files. Lists are skipped (a dataset's 80 class names are not
//! configuration keys), nesting stops at depth 2, and CI workflows under
//! `.github/` are left alone: `jobs`/`steps`/`with` are not project knobs.

use crate::types::{AstAnalysisResult, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use std::path::Path;

/// `cfg/default.yaml` in ultralytics has ~120 keys; a cap of 64 (JSON's)
/// would cut `lr0` off. Two hundred is still one file's worth of nodes.
const MAX_SYMBOLS: usize = 200;
const MAX_DEPTH: usize = 2;

pub struct YamlParser;

impl YamlParser {
    pub fn parse(file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        if is_workflow_path(file_path) {
            return result;
        }
        // (indent, key) of the mapping keys that enclose the current line.
        let mut stack: Vec<(usize, String)> = Vec::new();
        for (idx, raw) in content.lines().enumerate() {
            if result.symbols.len() >= MAX_SYMBOLS {
                break;
            }
            let line = raw.trim_end();
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("---") {
                continue;
            }
            let indent = line.len() - trimmed.len();
            // A list item is a value, not a key, even when it carries `k: v`.
            if trimmed.starts_with("- ") || trimmed == "-" {
                // Hydra composition: an item under a top-level `defaults:`
                // names another config this file layers over (`- early_stopping`
                // → `early_stopping.yaml` beside it; `- model: mnist` →
                // `model/mnist.yaml`). That is an import, and the linker turns
                // it into the edge that tells a composer from its base.
                if stack.len() == 1 && stack[0].1 == "defaults" {
                    if let Some(target) = hydra_default_target(&trimmed[1..]) {
                        let dir = file_path
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|s| s.to_str())
                            .filter(|s| !s.is_empty());
                        let hint = match dir {
                            Some(dir) => format!("{dir}/{target}.yaml"),
                            None => format!("{target}.yaml"),
                        };
                        result.relationships.push(ParsedRelationship {
                            source_symbol: file_path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("config")
                                .to_string(),
                            target_symbol: target.rsplit('/').next().unwrap_or(&target).to_string(),
                            relationship: EdgeType::Imports,
                            target_file_hint: Some(hint),
                            receiver_hint: None,
                        });
                    }
                }
                continue;
            }
            let Some((key, value)) = split_key(trimmed) else {
                continue;
            };
            while stack.last().is_some_and(|(i, _)| *i >= indent) {
                stack.pop();
            }
            let depth = stack.len();
            if depth > MAX_DEPTH {
                continue;
            }
            if keep_key(key) && !result.symbols.iter().any(|s| s.name == key) {
                let signature = if value.is_empty() {
                    key.to_string()
                } else {
                    format!("{key}: {}", value.chars().take(60).collect::<String>())
                };
                result.symbols.push(ParsedSymbol::new(
                    key,
                    NodeType::Config,
                    Some(signature),
                    (idx + 1)..(idx + 2),
                    true,
                ));
            }
            if value.is_empty() {
                stack.push((indent, key.to_string()));
            }
        }
        result
    }
}

/// `key: value` → (`key`, `value` without a trailing comment). A line that is
/// not a plain scalar key (quoted keys, `? ` complex keys, URLs) is skipped.
fn split_key(line: &str) -> Option<(&str, &str)> {
    let colon = line.find(':')?;
    let key = line[..colon].trim();
    let rest = &line[colon + 1..];
    if !(rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')) {
        return None;
    }
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        || !key
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        return None;
    }
    let value = rest.trim();
    let value = match value.find(" #") {
        Some(i) => value[..i].trim(),
        None => value,
    };
    Some((key, value))
}

fn keep_key(key: &str) -> bool {
    key.len() >= 2
        && key.len() <= 60
        && !matches!(
            key,
            "name" | "version" | "description" | "type" | "id" | "kind" | "on" | "env"
        )
}

/// The config a Hydra `defaults:` item names, relative to the composing file's
/// directory: `early_stopping` → `early_stopping`, `model: mnist` →
/// `model/mnist`, `override /model: mnist` → `model/mnist`. `_self_`,
/// `null`, and package-qualified (`@`) or interpolated items are skipped.
fn hydra_default_target(item: &str) -> Option<String> {
    let item = item.trim().trim_start_matches("override").trim();
    if item.is_empty()
        || item.starts_with('_')
        || item.contains('$')
        || item.contains('@')
        || item.contains('[')
    {
        return None;
    }
    let (group, name) = match item.split_once(':') {
        Some((g, n)) => (g.trim().trim_start_matches('/'), n.trim()),
        None => ("", item),
    };
    let name = name.trim_matches(|c| c == '"' || c == '\'');
    if name.is_empty() || name == "null" || name.starts_with('_') {
        return None;
    }
    let ok = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '/' || c == '.')
    };
    if !ok(group) || !ok(name) {
        return None;
    }
    let name = name.strip_suffix(".yaml").unwrap_or(name);
    Some(if group.is_empty() {
        name.to_string()
    } else {
        format!("{group}/{name}")
    })
}

fn is_workflow_path(path: &Path) -> bool {
    path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s == ".github" || s == ".gitlab-ci" || s == ".circleci"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_at_three_levels_lists_skipped() {
        let src = "# YOLO defaults\ntask: detect # (str) task\nlr0: 0.01 # (float) initial learning rate\nmodel:\n  backbone:\n    depth: 3\n    layers:\n      - [1, 2]\n      - name: conv\nnames:\n  0: person\n";
        let ast = YamlParser::parse(Path::new("cfg/default.yaml"), src);
        let names: Vec<&str> = ast.symbols.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["task", "lr0", "model", "backbone", "depth", "layers", "names"]
        );
        let lr0 = ast.symbols.iter().find(|s| s.name == "lr0").unwrap();
        assert_eq!(lr0.signature.as_deref(), Some("lr0: 0.01"));
        assert_eq!(lr0.symbol_type, NodeType::Config);
        assert_eq!(lr0.line_range, 3..4);
    }

    #[test]
    fn workflows_and_urls_are_not_config() {
        let src = "name: CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu\n";
        assert!(
            YamlParser::parse(Path::new(".github/workflows/ci.yml"), src)
                .symbols
                .is_empty()
        );
        let ast = YamlParser::parse(Path::new("mkdocs.yml"), "site_url: https://x.y\nrepo: a\n");
        let names: Vec<&str> = ast.symbols.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["site_url", "repo"]);
    }

    #[test]
    fn hydra_defaults_compose_sibling_configs() {
        let src = "defaults:\n  - model_checkpoint\n  - early_stopping\n  - override /model: mnist\n  - _self_\n\nearly_stopping:\n  patience: 100\n";
        let ast = YamlParser::parse(Path::new("configs/callbacks/default.yaml"), src);
        let hints: Vec<&str> = ast
            .relationships
            .iter()
            .map(|r| r.target_file_hint.as_deref().unwrap())
            .collect();
        assert_eq!(
            hints,
            vec![
                "callbacks/model_checkpoint.yaml",
                "callbacks/early_stopping.yaml",
                "callbacks/model/mnist.yaml"
            ]
        );
        assert!(ast
            .relationships
            .iter()
            .all(|r| r.relationship == EdgeType::Imports && r.source_symbol == "default"));
        let names: Vec<&str> = ast.symbols.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["defaults", "early_stopping", "patience"]);
    }
}

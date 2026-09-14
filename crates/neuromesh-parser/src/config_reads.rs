//! Config→Code: where Python reads a configuration key.
//!
//! `self.args.lr0`, `cfg.optimizer.lr`, `config["batch_size"]`,
//! `cfg.get("seed")`, `hparams.dropout` — each is a def that is parameterized
//! by a key that lives in a YAML/JSON file. The graph already has
//! `NodeType::Config` for the keys (`json.rs`, `yaml.rs`) and
//! `EdgeType::Parameterizes` for the relation; nothing produced the edge.
//! This overlay does, as a pending relationship the linker (`graph.rs`,
//! `Parameterizes` arm) binds to the key node — in the file the module names
//! (`load("configs/train.yaml")`, Hydra `config_name="train"`) when it names
//! one, otherwise to the dominant config file of the project.
//!
//! argparse is its own config file: `parser.add_argument("--lr")` *defines*
//! the key, so it becomes a `Hyperparameter` node in that file, and `args.lr`
//! anywhere else links to it through the same arm.
//!
//! Plain Python only; not gated on a framework. A key is read as a bare
//! attribute or subscript of a well-known config holder — not any attribute
//! of any object, which would make every `self.x` a config read.

use crate::ml_overlay::{enclosing_def, line_of, push_relationship, push_symbol, python_defs};
use crate::types::AstAnalysisResult;
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

/// The objects a config key is read from: `args.lr`, `self.args.lr0`,
/// `cfg.model.depth`, `hparams.dropout`, `opt.epochs`.
const HOLDERS: &str =
    r"(?:self\.)?(?:args|cfg|config|configs|conf|hparams|hyp|opt|opts|params|settings|options)";

/// Names that are methods of the holder or of a dict, never keys.
const NOT_KEYS: &[&str] = &[
    "get",
    "items",
    "keys",
    "values",
    "update",
    "copy",
    "pop",
    "setdefault",
    "parse_args",
    "add_argument",
    "dict",
    "to_dict",
    "save",
    "load",
    "merge",
    "freeze",
    "defrost",
    "clone",
    "from_dict",
    "as_dict",
    "len",
    "str",
    "int",
    "float",
    "bool",
    "list",
];

pub fn config_reads_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    static ATTR_RE: OnceLock<Regex> = OnceLock::new();
    static SUBSCRIPT_RE: OnceLock<Regex> = OnceLock::new();
    static GET_RE: OnceLock<Regex> = OnceLock::new();
    static ARGPARSE_RE: OnceLock<Regex> = OnceLock::new();
    static FILE_HINT_RE: OnceLock<Regex> = OnceLock::new();
    static HYDRA_RE: OnceLock<Regex> = OnceLock::new();
    let attr_re = ATTR_RE
        .get_or_init(|| Regex::new(&format!(r"\b{HOLDERS}((?:\.[A-Za-z_]\w*)+)(\s*\()?")).unwrap());
    let subscript_re = SUBSCRIPT_RE.get_or_init(|| {
        Regex::new(&format!(r#"\b{HOLDERS}\[\s*["']([A-Za-z_]\w*)["']\s*\]"#)).unwrap()
    });
    // `cfg.get("seed")`, `cfg.extras.get("print_config")`: Hydra/OmegaConf
    // code reads optional keys through `.get`, so the name is the argument.
    let get_re = GET_RE.get_or_init(|| {
        Regex::new(&format!(
            r#"\b{HOLDERS}(?:\.[A-Za-z_]\w*)*\.get\(\s*["']([A-Za-z_]\w*)["']"#
        ))
        .unwrap()
    });
    let argparse_re = ARGPARSE_RE
        .get_or_init(|| Regex::new(r#"add_argument\(\s*["']--([A-Za-z][\w-]*)["']"#).unwrap());
    let file_hint_re = FILE_HINT_RE
        .get_or_init(|| Regex::new(r#"["']([\w./-]+\.(?:ya?ml|json|toml))["']"#).unwrap());
    let hydra_re =
        HYDRA_RE.get_or_init(|| Regex::new(r#"config_name\s*=\s*["']([\w./-]+)["']"#).unwrap());

    // The config file this module names, if any: the hint the linker uses to
    // pick the right `lr0` among several config files.
    let hint: Option<String> = hydra_re
        .captures(content)
        .map(|c| {
            let name = c[1].to_string();
            if name.ends_with(".yaml") || name.ends_with(".yml") {
                name
            } else {
                format!("{name}.yaml")
            }
        })
        .or_else(|| file_hint_re.captures(content).map(|c| c[1].to_string()))
        .filter(|h| !h.contains("://"));

    let defs = python_defs(content);
    let module_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module")
        .to_string();
    let source_at = |byte: usize| -> String {
        enclosing_def(&defs, byte)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| module_name.clone())
    };
    let push = |ast: &mut AstAnalysisResult, byte: usize, key: &str| {
        let key = key.replace('-', "_");
        if key.len() < 2 || NOT_KEYS.contains(&key.as_str()) || key.starts_with('_') {
            return;
        }
        push_relationship(
            ast,
            &source_at(byte),
            &key,
            EdgeType::Parameterizes,
            hint.clone(),
        );
    };

    for cap in attr_re.captures_iter(content) {
        let byte = cap.get(0).unwrap().start();
        let mut segs: Vec<&str> = cap[1].split('.').filter(|s| !s.is_empty()).collect();
        // `args.get(...)`, `cfg.extras.get(...)`: the last segment is a method
        // call, not a key; the sections before it still are.
        if cap.get(2).is_some() {
            segs.pop();
        }
        // `cfg.model.depth`: both `model` (a section) and `depth` (the key).
        for seg in segs {
            push(ast, byte, seg);
        }
    }
    for cap in subscript_re.captures_iter(content) {
        push(ast, cap.get(0).unwrap().start(), &cap[1]);
    }
    for cap in get_re.captures_iter(content) {
        push(ast, cap.get(0).unwrap().start(), &cap[1]);
    }
    for cap in argparse_re.captures_iter(content) {
        let key = cap[1].replace('-', "_");
        if key.len() < 2 || NOT_KEYS.contains(&key.as_str()) {
            continue;
        }
        let start = cap.get(0).unwrap().start();
        let line_end = content[start..]
            .find('\n')
            .map(|i| start + i)
            .unwrap_or(content.len());
        push_symbol(
            ast,
            &key,
            NodeType::Hyperparameter,
            Some(content[start..line_end].trim().chars().take(100).collect()),
            line_of(content, start),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rels(src: &str) -> Vec<(String, String, Option<String>)> {
        let mut ast = AstAnalysisResult::default();
        config_reads_overlay(Path::new("engine/trainer.py"), src, &mut ast);
        ast.relationships
            .iter()
            .filter(|r| r.relationship == EdgeType::Parameterizes)
            .map(|r| {
                (
                    r.source_symbol.clone(),
                    r.target_symbol.clone(),
                    r.target_file_hint.clone(),
                )
            })
            .collect()
    }

    #[test]
    fn attribute_subscript_and_argparse_reads() {
        let src = r#"import argparse

def build_optimizer(self):
    lr = self.args.lr0 * self.args.get("warmup")
    depth = cfg.model.depth
    bs = config["batch_size"]

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--num-epochs", type=int)
    args = p.parse_args()
"#;
        let r = rels(src);
        let pairs: Vec<(&str, &str)> = r.iter().map(|(s, t, _)| (s.as_str(), t.as_str())).collect();
        assert!(pairs.contains(&("build_optimizer", "lr0")), "{pairs:?}");
        assert!(pairs.contains(&("build_optimizer", "warmup")), "{pairs:?}");
        assert!(!pairs.iter().any(|(_, t)| *t == "get"), "{pairs:?}");
        assert!(pairs.contains(&("build_optimizer", "model")));
        assert!(pairs.contains(&("build_optimizer", "depth")));
        assert!(pairs.contains(&("build_optimizer", "batch_size")));
        assert!(!pairs.iter().any(|(_, t)| *t == "parse_args"));
        assert!(!pairs.iter().any(|(_, t)| *t == "num_epochs"), "{pairs:?}");
        assert!(r.iter().all(|(_, _, h)| h.is_none()));

        let mut ast = AstAnalysisResult::default();
        config_reads_overlay(Path::new("main.py"), src, &mut ast);
        let arg = ast
            .symbols
            .iter()
            .find(|s| s.name == "num_epochs")
            .expect("argparse key is a node");
        assert_eq!(arg.symbol_type, NodeType::Hyperparameter);
        assert_eq!(arg.line_range.start, 10);
    }

    #[test]
    fn get_reads_are_config_reads() {
        let src = "def apply_extras(cfg):\n    if cfg.get(\"seed\"):\n        pass\n    if cfg.extras.get(\"print_config\"):\n        pass\n";
        let r = rels(src);
        let keys: Vec<&str> = r.iter().map(|(_, t, _)| t.as_str()).collect();
        assert!(keys.contains(&"seed"), "{keys:?}");
        assert!(keys.contains(&"extras"), "{keys:?}");
        assert!(keys.contains(&"print_config"), "{keys:?}");
    }

    #[test]
    fn module_level_read_and_file_hint() {
        let src = "cfg = OmegaConf.load(\"configs/train.yaml\")\nlr = cfg.optimizer.lr\n";
        let r = rels(src);
        assert_eq!(
            r,
            vec![
                (
                    "trainer".into(),
                    "optimizer".into(),
                    Some("configs/train.yaml".into())
                ),
                (
                    "trainer".into(),
                    "lr".into(),
                    Some("configs/train.yaml".into())
                ),
            ]
        );
        let hydra = "@hydra.main(config_path=\"../configs\", config_name=\"train\")\ndef main(cfg):\n    x = cfg.seed\n";
        assert_eq!(rels(hydra)[0].2.as_deref(), Some("train.yaml"));
    }
}

//! PyTorch overlay: recognise the artifact layer of a training codebase.
//!
//! The language parser sees `class Detector(nn.Module)` as a `Class` and
//! `train_one_epoch` as a `Function`. That is correct and useless: it gives no
//! way to answer "why did mAP drop?", because nothing in the graph knows which
//! symbol is the model, which loop produced the checkpoint, or which dataset
//! the loop consumed.
//!
//! This module retypes those symbols into the artifact vocabulary and links
//! them. It is regex- and indentation-based, like every other overlay here:
//! unknown shapes are a soft miss, never an index failure.
//!
//! The one rule that keeps precision up: an identifier is treated as a model
//! reference only when it receives a method that exists on `nn.Module` and
//! almost nowhere else (`parameters`, `state_dict`, `forward`, `train`,
//! `eval`, `zero_grad`, `to`). Guessing from a variable called `model` would
//! fire on any web codebase that has one.

use crate::types::{AstAnalysisResult, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Base classes that make a class a model.
const MODEL_BASES: &[&str] = &[
    "Module",
    "LightningModule",
    "PreTrainedModel",
    "Sequential",
    "ModuleList",
];

/// Base classes that make a class a dataset.
const DATASET_BASES: &[&str] = &[
    "Dataset",
    "IterableDataset",
    "VisionDataset",
    "LightningDataModule",
    "DataModule",
];

/// Methods that only a `nn.Module` has. Receiving one of these is what marks an
/// identifier as a model, rather than the identifier being named `model`.
const MODULE_METHODS: &[&str] = &[
    "parameters",
    "state_dict",
    "load_state_dict",
    "forward",
    "zero_grad",
    "train",
    "eval",
    "named_parameters",
    "modules",
];

pub fn pytorch_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    if !has_torch_evidence(content) {
        return;
    }

    let mut defs = python_defs(content);
    let classes = python_classes(content);
    let bindings = local_bindings(content);

    // Research scripts put the training loop at module scope, under
    // `while True:` or `for epoch in ...`, with no enclosing def at all —
    // nanoGPT is the canonical example. Without a synthetic owner the loop is
    // invisible and every checkpoint and metric in the file is unattributed.
    if let Some(module_loop) = module_level_loop(path, content, &defs, ast) {
        defs.push(module_loop);
    }

    let modules = module_classes(&classes);
    let (model_classes, layer_classes) = split_models_and_layers(content, &classes, &modules);
    for name in &model_classes {
        retype_symbol(ast, name, NodeType::Model);
    }
    for name in &layer_classes {
        retype_symbol(ast, name, NodeType::Layer);
    }
    let dataset_classes = retype_classes(&classes, DATASET_BASES, NodeType::Dataset, ast);

    push_layers(content, &classes, &modules, ast);
    retype_defs(content, &defs, &classes, &modules, ast);

    link_artifacts(
        content,
        &defs,
        &bindings,
        &model_classes,
        &dataset_classes,
        ast,
    );
    push_checkpoints(content, &defs, &bindings, &modules, ast);
    push_metrics(content, &defs, ast);
}

fn has_torch_evidence(content: &str) -> bool {
    content.contains("import torch")
        || content.contains("from torch")
        || content.contains("nn.Module")
        || content.contains("torch.nn")
        || content.contains("pytorch_lightning")
        || content.contains("lightning.pytorch")
}

// ---------------------------------------------------------------------------
// Python structure, by indentation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct PyBlock {
    name: String,
    /// Base classes, empty for a `def`.
    bases: Vec<String>,
    line: usize,
    /// Byte range of the block body, header included.
    start: usize,
    end: usize,
    indent: usize,
}

impl PyBlock {
    fn body<'a>(&self, content: &'a str) -> &'a str {
        content.get(self.start..self.end).unwrap_or("")
    }

    fn contains(&self, other: &PyBlock) -> bool {
        other.start > self.start && other.end <= self.end
    }
}

fn python_defs(content: &str) -> Vec<PyBlock> {
    static DEF_RE: OnceLock<Regex> = OnceLock::new();
    let re = DEF_RE.get_or_init(|| {
        Regex::new(r"(?m)^([ \t]*)(?:async[ \t]+)?def[ \t]+([A-Za-z_]\w*)").unwrap()
    });
    collect_blocks(content, re, false)
}

fn python_classes(content: &str) -> Vec<PyBlock> {
    static CLASS_RE: OnceLock<Regex> = OnceLock::new();
    let re = CLASS_RE.get_or_init(|| {
        Regex::new(r"(?m)^([ \t]*)class[ \t]+([A-Za-z_]\w*)[ \t]*(\(([^)]*)\))?").unwrap()
    });
    collect_blocks(content, re, true)
}

/// A block runs from its header to the first later line whose indentation is at
/// most the header's and which is not blank or a comment. That is how Python
/// scoping actually works and it costs no parser.
fn collect_blocks(content: &str, re: &Regex, with_bases: bool) -> Vec<PyBlock> {
    let mut blocks = Vec::new();
    for cap in re.captures_iter(content) {
        let whole = match cap.get(0) {
            Some(m) => m,
            None => continue,
        };
        let indent = cap.get(1).map(|m| m.as_str().len()).unwrap_or(0);
        let name = match cap.get(2) {
            Some(m) => m.as_str().to_string(),
            None => continue,
        };
        let bases = if with_bases {
            cap.get(4)
                .map(|m| {
                    m.as_str()
                        .split(',')
                        .map(|b| b.trim().to_string())
                        .filter(|b| !b.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let start = whole.start();
        blocks.push(PyBlock {
            name,
            bases,
            line: line_of(content, start),
            start,
            end: block_end(content, start, indent),
            indent,
        });
    }
    blocks
}

fn block_end(content: &str, header_start: usize, indent: usize) -> usize {
    let rest_start = match content[header_start..].find('\n') {
        Some(nl) => header_start + nl + 1,
        None => return content.len(),
    };
    let mut cursor = rest_start;
    for line in content[rest_start..].split_inclusive('\n') {
        let trimmed = line.trim_start();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            let line_indent = line.len() - line.trim_start_matches([' ', '\t']).len();
            if line_indent <= indent {
                return cursor;
            }
        }
        cursor += line.len();
    }
    content.len()
}

fn line_of(content: &str, byte: usize) -> usize {
    content
        .get(..byte)
        .map(|head| head.bytes().filter(|b| *b == b'\n').count() + 1)
        .unwrap_or(1)
}

/// `model = Detector(...)` / `self.head = nn.Linear(...)` -> variable to the
/// last segment of the constructor it was assigned from.
fn local_bindings(content: &str) -> HashMap<String, String> {
    static ASSIGN_RE: OnceLock<Regex> = OnceLock::new();
    let re = ASSIGN_RE.get_or_init(|| {
        Regex::new(r"(?m)^[ \t]*(?:self\.)?([A-Za-z_]\w*)[ \t]*=[ \t]*([A-Za-z_][\w.]*)[ \t]*\(")
            .unwrap()
    });
    let mut map = HashMap::new();
    for cap in re.captures_iter(content) {
        let (var, ctor) = match (cap.get(1), cap.get(2)) {
            (Some(v), Some(c)) => (v.as_str(), c.as_str()),
            _ => continue,
        };
        let ctor = ctor.rsplit('.').next().unwrap_or(ctor);
        // First assignment wins: a later rebind is usually a transformation
        // (`model = model.to(device)`), not a different artifact.
        map.entry(var.to_string())
            .or_insert_with(|| ctor.to_string());
    }
    map
}

// ---------------------------------------------------------------------------
// Typing
// ---------------------------------------------------------------------------

/// Retype every class whose bases match, and return the names that matched.
fn retype_classes(
    classes: &[PyBlock],
    bases: &[&str],
    node_type: NodeType,
    ast: &mut AstAnalysisResult,
) -> Vec<String> {
    let mut matched = Vec::new();
    for class in classes {
        if !class.bases.iter().any(|b| {
            let short = b.rsplit('.').next().unwrap_or(b);
            bases.contains(&short)
        }) {
            continue;
        }
        matched.push(class.name.clone());
        retype_symbol(ast, &class.name, node_type);
    }
    matched
}

fn retype_symbol(ast: &mut AstAnalysisResult, name: &str, node_type: NodeType) -> bool {
    if let Some(sym) = ast
        .symbols
        .iter_mut()
        .find(|s| s.name == name && s.parent.is_none())
    {
        sym.symbol_type = node_type;
        return true;
    }
    if let Some(sym) = ast.symbols.iter_mut().find(|s| s.name == name) {
        sym.symbol_type = node_type;
        return true;
    }
    false
}

/// Every `nn.Module` subclass in this file, model or layer alike.
fn module_classes(classes: &[PyBlock]) -> Vec<String> {
    classes
        .iter()
        .filter(|class| {
            class.bases.iter().any(|b| {
                let short = b.rsplit('.').next().unwrap_or(b);
                MODEL_BASES.contains(&short)
            })
        })
        .map(|class| class.name.clone())
        .collect()
}

/// Decide which `nn.Module` subclasses are *models* and which are *layers*.
///
/// Typing every `nn.Module` subclass as a Model is what the first version did,
/// and on a real library it is useless: vit-pytorch has 86 Python files and
/// produced 388 "models". A block, a head, an attention module and the network
/// they compose all became the same thing, so `Model` stopped narrowing
/// anything.
///
/// The distinction that holds up in real code is composition: a module that
/// another module builds inside itself — `self.attn = Attention(...)`,
/// `ModuleList([Block(cfg) for _ in ...])` — is a part. Whatever nothing else
/// builds is the whole. On nanoGPT that leaves `GPT` a Model and demotes
/// `Block`, `MLP`, `CausalSelfAttention` and `LayerNorm` to Layers, which is
/// exactly how a person reads that file.
fn split_models_and_layers(
    content: &str,
    classes: &[PyBlock],
    modules: &[String],
) -> (Vec<String>, Vec<String>) {
    let mut models = Vec::new();
    let mut layers = Vec::new();
    for name in modules {
        let composed_by_another = classes.iter().any(|owner| {
            owner.name != *name
                && modules.contains(&owner.name)
                && constructs(owner.body(content), name)
        });
        if composed_by_another {
            layers.push(name.clone());
        } else {
            models.push(name.clone());
        }
    }
    (models, layers)
}

/// Whether this body calls `Name(...)` as a constructor.
fn constructs(body: &str, name: &str) -> bool {
    let mut from = 0usize;
    while let Some(found) = body[from..].find(name) {
        let at = from + found;
        let before_ok = at == 0
            || !body[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.');
        let after = body[at + name.len()..].trim_start();
        if before_ok && after.starts_with('(') {
            return true;
        }
        from = at + name.len();
    }
    false
}

/// `self.backbone = nn.Sequential(...)` inside a module is a named part of it,
/// and "which backbone?" is a question people actually ask.
fn push_layers(
    content: &str,
    classes: &[PyBlock],
    model_classes: &[String],
    ast: &mut AstAnalysisResult,
) {
    static LAYER_RE: OnceLock<Regex> = OnceLock::new();
    let re = LAYER_RE.get_or_init(|| {
        Regex::new(
            r"(?m)^[ \t]*self\.([A-Za-z_]\w*)[ \t]*=[ \t]*((?:nn|torch\.nn|torchvision\.models|models|timm|F)\.[\w.]+)[ \t]*\(",
        )
        .unwrap()
    });
    for class in classes {
        if !model_classes.contains(&class.name) {
            continue;
        }
        for cap in re.captures_iter(class.body(content)) {
            let (attr, ctor) = match (cap.get(1), cap.get(2)) {
                (Some(a), Some(c)) => (a.as_str(), c.as_str()),
                _ => continue,
            };
            let name = format!("{}.{}", class.name, attr);
            if ast.symbols.iter().any(|s| s.name == name) {
                continue;
            }
            let line =
                class.line + line_of(class.body(content), cap.get(0).map_or(0, |m| m.start())) - 1;
            let mut symbol = ParsedSymbol::new(
                name.clone(),
                NodeType::Layer,
                Some(format!("self.{attr} = {ctor}(...)")),
                line..(line + 1),
                false,
            );
            symbol.parent = Some(class.name.clone());
            ast.symbols.push(symbol);
            push_relationship(ast, &class.name, &name, EdgeType::Contains, None);
        }
    }
}

/// Classify a `def` by what its body does, not by what it is called.
fn retype_defs(
    content: &str,
    defs: &[PyBlock],
    classes: &[PyBlock],
    model_classes: &[String],
    ast: &mut AstAnalysisResult,
) {
    for def in defs {
        let body = def.body(content);
        let node_type = if def.name == "forward" {
            // `forward` outside a model is just a function.
            let in_model = classes
                .iter()
                .any(|c| c.contains(def) && model_classes.contains(&c.name));
            if !in_model {
                continue;
            }
            NodeType::Layer
        } else if is_train_loop(body) {
            NodeType::TrainLoop
        } else if is_eval_loop(body) {
            NodeType::EvalLoop
        } else if is_transform_builder(body) {
            NodeType::Transform
        } else {
            continue;
        };
        // A method is stored under its own name with a parent, so match on the
        // plain name and let `retype_symbol` prefer the top-level entry.
        retype_symbol(ast, &def.name, node_type);
    }
}

fn is_train_loop(body: &str) -> bool {
    (body.contains(".backward()") || body.contains(".backward("))
        || (body.contains(".step()") && body.contains("zero_grad"))
}

/// Gradients being off is not enough on its own. `GPT.from_pretrained` copies
/// weights under `torch.no_grad()` and `generate` samples tokens under it — both
/// were coming out as EvalLoops. An evaluation loop also has to *look at data*
/// or *report a number*; a weight-copying constructor does neither.
fn is_eval_loop(body: &str) -> bool {
    let grads_off =
        body.contains("no_grad") || body.contains("inference_mode") || body.contains(".eval()");
    grads_off && (iterates_data(body) || reports_quality(body))
}

fn iterates_data(body: &str) -> bool {
    body.contains("for ")
        && ["loader", "dataset", "batch", "val_", "valid", "test_"]
            .iter()
            .any(|k| body.contains(k))
}

fn reports_quality(body: &str) -> bool {
    [
        "loss", "acc", "metric", "correct", "map", "perplex", "score", "f1",
    ]
    .iter()
    .any(|k| body.contains(k))
}

/// Only a `Compose(` counts. Matching on `transforms.` or `augment` looked
/// tempting and turned every `main(..., augment=True)` into a Transform.
fn is_transform_builder(body: &str) -> bool {
    body.contains("Compose(")
}

// ---------------------------------------------------------------------------
// Linking
// ---------------------------------------------------------------------------

fn link_artifacts(
    content: &str,
    defs: &[PyBlock],
    bindings: &HashMap<String, String>,
    model_classes: &[String],
    dataset_classes: &[String],
    ast: &mut AstAnalysisResult,
) {
    for def in defs {
        let body = def.body(content);
        let train = is_train_loop(body);
        let eval = !train && is_eval_loop(body);

        // Only a loop trains or evaluates a model.
        if train || eval {
            let edge = if train {
                EdgeType::Trains
            } else {
                EdgeType::Evaluates
            };
            for model in model_references(body, bindings, model_classes) {
                push_relationship(ast, &def.name, &model, edge, None);
            }
        }

        // But whoever builds the DataLoader consumes the dataset, and in every
        // real script that is `main`, not the loop it hands the loader to.
        // Restricting this to loops left the dataset unreachable from anything.
        for dataset in dataset_references(body, bindings, dataset_classes) {
            push_relationship(ast, &def.name, &dataset, EdgeType::Consumes, None);
        }
    }

    link_transforms(content, bindings, ast);
}

/// `CocoDetection(root, transforms=pipeline)` states, in one keyword argument,
/// which transform pipeline shapes which dataset — the `Transform -> Dataset`
/// link the mAP question needs, and the only place it is written down.
fn link_transforms(content: &str, bindings: &HashMap<String, String>, ast: &mut AstAnalysisResult) {
    static KWARG_RE: OnceLock<Regex> = OnceLock::new();
    let re = KWARG_RE.get_or_init(|| {
        Regex::new(
            r"\b([A-Za-z_]\w*)[ \t]*\((?:[^()]|\([^()]*\))*?\btransforms?[ \t]*=[ \t]*([A-Za-z_]\w*)",
        )
        .unwrap()
    });
    for cap in re.captures_iter(content) {
        let (ctor, value) = match (cap.get(1), cap.get(2)) {
            (Some(c), Some(v)) => (c.as_str(), v.as_str()),
            _ => continue,
        };
        if value == "None" {
            continue;
        }
        let dataset = bindings.get(ctor).cloned().unwrap_or_else(|| ctor.into());
        let transform = bindings.get(value).cloned().unwrap_or_else(|| value.into());
        push_relationship(ast, &transform, &dataset, EdgeType::Transforms, None);
    }
}

/// Identifiers in this body that receive an `nn.Module`-only method, mapped
/// back to the class they were constructed from where that is visible.
fn model_references(
    body: &str,
    bindings: &HashMap<String, String>,
    model_classes: &[String],
) -> Vec<String> {
    static CALL_RE: OnceLock<Regex> = OnceLock::new();
    let re =
        CALL_RE.get_or_init(|| Regex::new(r"\b([A-Za-z_]\w*)\.([A-Za-z_]\w*)[ \t]*\(").unwrap());
    let mut found = Vec::new();
    for cap in re.captures_iter(body) {
        let (receiver, method) = match (cap.get(1), cap.get(2)) {
            (Some(r), Some(m)) => (r.as_str(), m.as_str()),
            _ => continue,
        };
        if !MODULE_METHODS.contains(&method) {
            continue;
        }
        // `self.eval()` inside the model itself is not a reference to another
        // artifact, and `optimizer.zero_grad()` is not a model.
        if receiver == "self" || receiver == "optimizer" || receiver == "optim" {
            continue;
        }
        let resolved = bindings
            .get(receiver)
            .filter(|ctor| model_classes.is_empty() || model_classes.contains(ctor))
            .cloned()
            .unwrap_or_else(|| receiver.to_string());
        if !found.contains(&resolved) {
            found.push(resolved);
        }
    }
    found
}

/// A dataset reference is the first argument of a `DataLoader(...)`, or a
/// variable constructed from a known dataset class.
fn dataset_references(
    body: &str,
    bindings: &HashMap<String, String>,
    dataset_classes: &[String],
) -> Vec<String> {
    static LOADER_RE: OnceLock<Regex> = OnceLock::new();
    let re = LOADER_RE
        .get_or_init(|| Regex::new(r"\bDataLoader[ \t]*\([ \t]*([A-Za-z_][\w.]*)").unwrap());
    let mut found = Vec::new();
    for cap in re.captures_iter(body) {
        let arg = match cap.get(1) {
            Some(m) => m.as_str(),
            None => continue,
        };
        let base = arg.split('.').next().unwrap_or(arg);
        let resolved = bindings
            .get(base)
            .cloned()
            .unwrap_or_else(|| base.to_string());
        if !found.contains(&resolved) {
            found.push(resolved);
        }
    }
    for class in dataset_classes {
        if body.contains(&format!("{class}(")) && !found.contains(class) {
            found.push(class.clone());
        }
    }
    found
}

/// `torch.save(...)` / `torch.load(...)` / `load_state_dict(torch.load(...))`.
fn push_checkpoints(
    content: &str,
    defs: &[PyBlock],
    bindings: &HashMap<String, String>,
    modules: &[String],
    ast: &mut AstAnalysisResult,
) {
    static SAVE_RE: OnceLock<Regex> = OnceLock::new();
    static LOAD_RE: OnceLock<Regex> = OnceLock::new();
    // `torch.save(model.state_dict(), "runs/last.pt")` nests one call inside the
    // argument list, so a flat `[^)]*` would stop before reaching the path.
    let save_re = SAVE_RE
        .get_or_init(|| Regex::new(r"torch\.save[ \t]*\(((?:[^()]|\([^()]*\))*)\)").unwrap());
    let load_re = LOAD_RE.get_or_init(|| Regex::new(r"torch\.load[ \t]*\(([^),]*)").unwrap());

    for (re, produces) in [(save_re, true), (load_re, false)] {
        for cap in re.captures_iter(content) {
            let whole = match cap.get(0) {
                Some(m) => m,
                None => continue,
            };
            let args = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let name = checkpoint_name(args);
            let line = line_of(content, whole.start());
            push_symbol(
                ast,
                &name,
                NodeType::Checkpoint,
                Some(whole.as_str().trim().to_string()),
                line,
            );
            if let Some(owner) = enclosing_def(defs, whole.start()) {
                let edge = if produces {
                    EdgeType::Produces
                } else {
                    EdgeType::Consumes
                };
                push_relationship(ast, &owner.name, &name, edge, None);
            }
            // Whose weights these are, when the call actually says so.
            if produces {
                if let Some(model) = saved_model(args, content, bindings, modules) {
                    push_relationship(ast, &name, &model, EdgeType::CheckpointOf, None);
                }
            }
        }
    }
}

/// The model a `torch.save` is saving, when the call states it.
///
/// Taking the first argument's leading identifier unconditionally produced
/// nonsense on real code: `torch.save({"model": _get_full_model_state_dict(m)})`
/// attributed the checkpoint to a *function*. Two shapes actually carry the
/// claim, and nothing else does:
///
///   - `torch.save(x.state_dict(), path)` — only an `nn.Module` has a
///     `state_dict`, so `x` is a model by construction;
///   - `torch.save(x, path)` where `x` is a bare identifier that is either
///     built from a module class in this file, or is treated as a module
///     somewhere in it (`x.eval()`, `x.parameters()`).
///
/// A dict literal, a call, or an unrecognised name yields nothing rather than a
/// guess.
fn saved_model(
    args: &str,
    content: &str,
    bindings: &HashMap<String, String>,
    modules: &[String],
) -> Option<String> {
    let first = args.split(',').next()?.trim();
    let ident = first.strip_suffix(".state_dict()").map(str::trim);
    if let Some(ident) = ident {
        let base = ident.split('.').next()?;
        if base.is_empty() || base == "self" {
            return None;
        }
        return Some(bindings.get(base).cloned().unwrap_or_else(|| base.into()));
    }
    let bare = first
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_')
        .then_some(first)
        .filter(|s| !s.is_empty() && *s != "self")?;
    let built_from_module = bindings.get(bare).is_some_and(|c| modules.contains(c));
    if built_from_module || used_as_module(content, bare) {
        return Some(bindings.get(bare).cloned().unwrap_or_else(|| bare.into()));
    }
    None
}

/// Whether this identifier ever receives an `nn.Module`-only method.
fn used_as_module(content: &str, ident: &str) -> bool {
    MODULE_METHODS
        .iter()
        .any(|m| content.contains(&format!("{ident}.{m}(")))
}

/// Prefer the path literal — `runs/last.pt` is what a person searches for.
fn checkpoint_name(args: &str) -> String {
    static PATH_RE: OnceLock<Regex> = OnceLock::new();
    let re = PATH_RE.get_or_init(|| {
        Regex::new(r#"["']([^"']*\.(?:pt|pth|ckpt|bin|safetensors))["']"#).unwrap()
    });
    if let Some(path) = re.captures(args).and_then(|c| c.get(1)) {
        return path.as_str().to_string();
    }
    // No literal: `torch.load(ckpt_path, map_location=device)`. The variable
    // holding the path is still a better name than "checkpoint" — it is what
    // the file calls this artifact, and it keeps two different checkpoints in
    // one module from collapsing onto one node.
    static PATH_VAR_RE: OnceLock<Regex> = OnceLock::new();
    let var_re = PATH_VAR_RE.get_or_init(|| {
        Regex::new(r"\b([A-Za-z_]\w*(?:_(?:path|file|dir|ckpt))|ckpt\w*|checkpoint\w*)\b").unwrap()
    });
    args.split(',')
        .find_map(|arg| var_re.captures(arg).and_then(|c| c.get(1)))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "checkpoint".to_string())
}

/// Metrics are read off the places a run actually reports them, not off
/// variable names: `self.log("val/mAP", ...)`, `wandb.log({"mAP": ...})`,
/// `metrics["mAP"] = ...`, and torchmetrics constructors.
fn push_metrics(content: &str, defs: &[PyBlock], ast: &mut AstAnalysisResult) {
    static LOG_RE: OnceLock<Regex> = OnceLock::new();
    static DICT_RE: OnceLock<Regex> = OnceLock::new();
    static TM_RE: OnceLock<Regex> = OnceLock::new();
    let log_re = LOG_RE
        .get_or_init(|| Regex::new(r#"\.log(?:_dict)?[ \t]*\([ \t]*["']([^"']+)["']"#).unwrap());
    let dict_re = DICT_RE.get_or_init(|| {
        Regex::new(r#"(?:log[ \t]*\(|metrics)[ \t]*[\{\[][ \t]*["']([^"']+)["']"#).unwrap()
    });
    let tm_re = TM_RE.get_or_init(|| {
        Regex::new(r"\b(MeanAveragePrecision|Accuracy|F1Score|Precision|Recall|AveragePrecision|IoU|JaccardIndex)[ \t]*\(")
            .unwrap()
    });

    for re in [log_re, dict_re, tm_re] {
        for cap in re.captures_iter(content) {
            let (whole, key) = match (cap.get(0), cap.get(1)) {
                (Some(w), Some(k)) => (w, k.as_str()),
                _ => continue,
            };
            if key.is_empty() || key.len() > 64 {
                continue;
            }
            let line = line_of(content, whole.start());
            push_symbol(
                ast,
                key,
                NodeType::Metric,
                Some(format!("metric {key}")),
                line,
            );
            if let Some(owner) = enclosing_def(defs, whole.start()) {
                push_relationship(ast, &owner.name, key, EdgeType::Produces, None);
            }
        }
    }
}

/// The innermost block containing this byte offset — smallest span wins, so a
/// nested def beats its parent and both beat the synthetic module-level block.
fn enclosing_def(defs: &[PyBlock], byte: usize) -> Option<&PyBlock> {
    defs.iter()
        .filter(|d| byte >= d.start && byte < d.end)
        .min_by_key(|d| d.end - d.start)
}

/// A training loop written at module scope, with no enclosing `def`.
///
/// Research scripts routinely do this — nanoGPT's whole loop lives under a bare
/// `while True:` at the top level of `train.py`. With no owner to attach to,
/// the loop was invisible and so was every checkpoint and metric in the file:
/// the audit found nanoGPT had *zero* TrainLoops.
///
/// The synthetic block spans the file and is named after the module, so
/// `train.py` contributes a `train` TrainLoop. `enclosing_def` prefers the
/// smallest containing span, so real defs still win wherever they exist.
fn module_level_loop(
    path: &Path,
    content: &str,
    defs: &[PyBlock],
    ast: &mut AstAnalysisResult,
) -> Option<PyBlock> {
    let at = backward_calls(content).find(|at| enclosing_def(defs, *at).is_none())?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");
    let name = if ast.symbols.iter().any(|s| s.name == stem) {
        format!("{stem}:train")
    } else {
        stem.to_string()
    };
    let line = line_of(content, at);
    push_symbol(
        ast,
        &name,
        NodeType::TrainLoop,
        Some(format!("module-level training loop in {stem}")),
        line,
    );
    Some(PyBlock {
        name,
        bases: Vec::new(),
        line,
        start: 0,
        end: content.len(),
        indent: 0,
    })
}

fn backward_calls(content: &str) -> impl Iterator<Item = usize> + '_ {
    content.match_indices(".backward(").map(|(at, _)| at)
}

fn push_symbol(
    ast: &mut AstAnalysisResult,
    name: &str,
    node_type: NodeType,
    signature: Option<String>,
    line: usize,
) {
    if ast.symbols.iter().any(|s| s.name == name) {
        return;
    }
    let line = line.max(1);
    ast.symbols.push(ParsedSymbol::new(
        name,
        node_type,
        signature,
        line..(line + 1),
        true,
    ));
}

fn push_relationship(
    ast: &mut AstAnalysisResult,
    source: &str,
    target: &str,
    relationship: EdgeType,
    target_file_hint: Option<String>,
) {
    if source.is_empty() || target.is_empty() || source == target {
        return;
    }
    if ast.relationships.iter().any(|r| {
        r.source_symbol == source && r.target_symbol == target && r.relationship == relationship
    }) {
        return;
    }
    ast.relationships.push(ParsedRelationship {
        source_symbol: source.to_string(),
        target_symbol: target.to_string(),
        relationship,
        target_file_hint,
        receiver_hint: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRAIN: &str = r#"
import torch
from torch import nn
from torch.utils.data import DataLoader

from data.dataset import CocoDetection, build_transforms
from models.detector import Detector


def train_one_epoch(model, loader, optimizer):
    model.train()
    for images, targets in loader:
        optimizer.zero_grad()
        loss = model(images, targets)
        loss.backward()
        optimizer.step()
    return loss


def main():
    transforms = build_transforms(image_size=640)
    dataset = CocoDetection("data/coco", transforms=transforms)
    loader = DataLoader(dataset, batch_size=16)
    model = Detector(num_classes=80)
    optimizer = torch.optim.SGD(model.parameters(), lr=0.01)
    train_one_epoch(model, loader, optimizer)
    torch.save(model.state_dict(), "runs/last.pt")
"#;

    const MODEL: &str = r#"
import torch.nn as nn


class Detector(nn.Module):
    def __init__(self, num_classes=80):
        super().__init__()
        self.backbone = nn.Sequential(nn.Conv2d(3, 64, 3))
        self.head = nn.Linear(64, num_classes)

    def forward(self, images, targets=None):
        features = self.backbone(images)
        return self.head(features)
"#;

    const EVAL: &str = r#"
import torch
from torchmetrics.detection import MeanAveragePrecision

from models.detector import Detector


def evaluate(model, loader):
    model.eval()
    metric = MeanAveragePrecision()
    with torch.no_grad():
        for images, targets in loader:
            metric.update(model(images), targets)
    result = metric.compute()
    logger.log("val/mAP", result["map"])
    return result
"#;

    fn run(source: &str, declared: &[(&str, NodeType)]) -> AstAnalysisResult {
        run_at("train.py", source, declared)
    }

    fn run_at(file: &str, source: &str, declared: &[(&str, NodeType)]) -> AstAnalysisResult {
        let mut ast = AstAnalysisResult::default();
        for (name, ty) in declared {
            ast.symbols
                .push(ParsedSymbol::new(*name, *ty, None, 1..2, true));
        }
        pytorch_overlay(Path::new(file), source, &mut ast);
        ast
    }

    fn type_of(ast: &AstAnalysisResult, name: &str) -> Option<NodeType> {
        ast.symbols
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.symbol_type)
    }

    fn has_edge(ast: &AstAnalysisResult, source: &str, target: &str, edge: EdgeType) -> bool {
        ast.relationships.iter().any(|r| {
            r.source_symbol == source && r.target_symbol == target && r.relationship == edge
        })
    }

    #[test]
    fn a_module_subclass_becomes_a_model_and_its_attributes_become_layers() {
        let ast = run(
            MODEL,
            &[
                ("Detector", NodeType::Class),
                ("forward", NodeType::Function),
            ],
        );
        assert_eq!(type_of(&ast, "Detector"), Some(NodeType::Model));
        assert_eq!(type_of(&ast, "Detector.backbone"), Some(NodeType::Layer));
        assert_eq!(type_of(&ast, "Detector.head"), Some(NodeType::Layer));
        assert_eq!(type_of(&ast, "forward"), Some(NodeType::Layer));
        assert!(has_edge(
            &ast,
            "Detector",
            "Detector.backbone",
            EdgeType::Contains
        ));
    }

    #[test]
    fn a_loop_is_classified_by_what_it_does_not_by_its_name() {
        let ast = run(
            TRAIN,
            &[
                ("train_one_epoch", NodeType::Function),
                ("main", NodeType::Function),
            ],
        );
        assert_eq!(type_of(&ast, "train_one_epoch"), Some(NodeType::TrainLoop));

        let ast = run(EVAL, &[("evaluate", NodeType::Function)]);
        assert_eq!(type_of(&ast, "evaluate"), Some(NodeType::EvalLoop));
    }

    #[test]
    fn the_training_loop_links_to_the_model_it_trains() {
        let ast = run(TRAIN, &[("train_one_epoch", NodeType::Function)]);
        // `model.train()` is an nn.Module-only method, which is what identifies
        // the parameter as a model — the name `model` proves nothing. The
        // file-level binding then carries it back to the class it was built
        // from, so the edge names `Detector` rather than a local variable.
        assert!(has_edge(
            &ast,
            "train_one_epoch",
            "Detector",
            EdgeType::Trains
        ));
    }

    #[test]
    fn a_checkpoint_is_named_by_its_path_and_knows_whose_weights_it_holds() {
        let ast = run(TRAIN, &[("main", NodeType::Function)]);
        assert_eq!(type_of(&ast, "runs/last.pt"), Some(NodeType::Checkpoint));
        assert!(has_edge(&ast, "main", "runs/last.pt", EdgeType::Produces));
        assert!(has_edge(
            &ast,
            "runs/last.pt",
            "Detector",
            EdgeType::CheckpointOf
        ));
    }

    #[test]
    fn the_dataloader_argument_is_the_dataset_the_loop_consumes() {
        let ast = run(TRAIN, &[("main", NodeType::Function)]);
        // `main` is not a loop, so the edge belongs to nothing here; the point
        // is that the binding resolves `loader`'s argument back to its class.
        let bindings = local_bindings(TRAIN);
        assert_eq!(
            bindings.get("dataset").map(String::as_str),
            Some("CocoDetection")
        );
        assert_eq!(bindings.get("model").map(String::as_str), Some("Detector"));
        assert!(ast.symbols.iter().any(|s| s.name == "runs/last.pt"));
    }

    #[test]
    fn metrics_come_from_where_the_run_reports_them() {
        let ast = run(EVAL, &[("evaluate", NodeType::Function)]);
        assert_eq!(type_of(&ast, "val/mAP"), Some(NodeType::Metric));
        assert_eq!(
            type_of(&ast, "MeanAveragePrecision"),
            Some(NodeType::Metric)
        );
        assert!(has_edge(&ast, "evaluate", "val/mAP", EdgeType::Produces));
    }

    const NANOGPT_SHAPED: &str = r#"
import torch
import torch.nn as nn

class MLP(nn.Module):
    def __init__(self):
        super().__init__()
        self.c_fc = nn.Linear(4, 4)

    def forward(self, x):
        return self.c_fc(x)


class Block(nn.Module):
    def __init__(self):
        super().__init__()
        self.mlp = MLP()


class GPT(nn.Module):
    def __init__(self):
        super().__init__()
        self.h = nn.ModuleList([Block() for _ in range(4)])


model = GPT()
while True:
    loss = model(x)
    loss.backward()
    torch.save(model.state_dict(), "out/ckpt.pt")
"#;

    #[test]
    fn only_the_module_nothing_else_builds_is_a_model() {
        let ast = run_at(
            "train.py",
            NANOGPT_SHAPED,
            &[
                ("MLP", NodeType::Class),
                ("Block", NodeType::Class),
                ("GPT", NodeType::Class),
            ],
        );
        // `MLP` is built by `Block`, `Block` by `GPT`. Nothing builds `GPT`.
        assert_eq!(type_of(&ast, "GPT"), Some(NodeType::Model));
        assert_eq!(type_of(&ast, "Block"), Some(NodeType::Layer));
        assert_eq!(type_of(&ast, "MLP"), Some(NodeType::Layer));
    }

    #[test]
    fn a_training_loop_at_module_scope_still_gets_an_owner() {
        let ast = run_at("train.py", NANOGPT_SHAPED, &[("GPT", NodeType::Class)]);
        // The loop is under a bare `while True:` with no enclosing def, so the
        // file itself becomes the owner and is named after the module.
        assert_eq!(type_of(&ast, "train"), Some(NodeType::TrainLoop));
        assert!(has_edge(&ast, "train", "GPT", EdgeType::Trains));
        assert!(has_edge(&ast, "train", "out/ckpt.pt", EdgeType::Produces));
    }

    #[test]
    fn gradients_being_off_is_not_enough_to_be_an_eval_loop() {
        let source = r#"
import torch

def from_pretrained(cls, model_type):
    sd = {}
    with torch.no_grad():
        for k in sd_keys:
            sd[k].copy_(sd_hf[k])
    return sd

def estimate_loss(model, loader):
    model.eval()
    with torch.no_grad():
        for batch in loader:
            loss = model(batch)
    return loss
"#;
        let ast = run(
            source,
            &[
                ("from_pretrained", NodeType::Function),
                ("estimate_loss", NodeType::Function),
            ],
        );
        // Copying weights under no_grad looks at no data and reports no number.
        assert_eq!(type_of(&ast, "from_pretrained"), Some(NodeType::Function));
        assert_eq!(type_of(&ast, "estimate_loss"), Some(NodeType::EvalLoop));
    }

    #[test]
    fn a_checkpoint_only_claims_a_model_when_the_call_says_so() {
        let source = r#"
import torch

def save_all(state, path):
    torch.save({"model": _get_full_model_state_dict(m)}, "a.pt")
    torch.save(model.state_dict(), "b.pt")
"#;
        let ast = run(source, &[("save_all", NodeType::Function)]);
        // A dict built from a function call names no model; `.state_dict()` does.
        assert!(!ast
            .relationships
            .iter()
            .any(|r| r.relationship == EdgeType::CheckpointOf
                && r.target_symbol == "_get_full_model_state_dict"));
        assert!(has_edge(&ast, "b.pt", "model", EdgeType::CheckpointOf));
    }

    #[test]
    fn a_checkpoint_with_no_path_literal_is_named_after_its_variable() {
        let source = r#"
import torch

def resume(ckpt_path, device):
    return torch.load(ckpt_path, map_location=device)
"#;
        let ast = run(source, &[("resume", NodeType::Function)]);
        assert_eq!(type_of(&ast, "ckpt_path"), Some(NodeType::Checkpoint));
    }

    #[test]
    fn a_python_file_with_no_torch_in_it_is_left_alone() {
        let source = r#"
class Detector:
    def forward(self, images):
        return images


def train_one_epoch(model, loader):
    loss = 0.0
    loss.backward()
    return loss
"#;
        let ast = run(
            source,
            &[
                ("Detector", NodeType::Class),
                ("train_one_epoch", NodeType::Function),
            ],
        );
        assert_eq!(type_of(&ast, "Detector"), Some(NodeType::Class));
        assert_eq!(type_of(&ast, "train_one_epoch"), Some(NodeType::Function));
    }

    #[test]
    fn block_scoping_follows_indentation() {
        let defs = python_defs(MODEL);
        let forward = defs.iter().find(|d| d.name == "forward").unwrap();
        assert!(forward.body(MODEL).contains("self.head(features)"));
        assert!(!forward
            .body(MODEL)
            .contains("self.backbone = nn.Sequential"));

        let classes = python_classes(MODEL);
        let detector = classes.iter().find(|c| c.name == "Detector").unwrap();
        assert!(detector.contains(forward));
        assert_eq!(detector.bases, vec!["nn.Module".to_string()]);
    }
}

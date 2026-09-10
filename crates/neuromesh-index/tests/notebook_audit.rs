//! Run the notebook normalizer over a real checkout and report numbers.
//!
//! A fixture we wrote passes because we wrote it. This is the counterweight:
//! point it at somebody else's repository and see what actually happens —
//! how many notebooks normalize, how much of the file survives, and whether
//! the result parses as Python at all.
//!
//! ```text
//! NM_AUDIT_PATH=/path/to/checkout \
//!   cargo test -p neuromesh-index --test notebook_audit -- --ignored --nocapture
//!
//! NM_NOTEBOOK_PATH=/path/to/one.ipynb \
//!   cargo test -p neuromesh-index --test notebook_audit dump -- --ignored --nocapture
//! ```
//!
//! Repositories worth keeping in rotation, each breaking a different
//! assumption:
//!
//! * `fastai/fastbook` — notebooks *are* the codebase; heavy markdown, magics.
//! * `huggingface/notebooks` — generated, output-heavy, many megabytes each.
//! * `jakevdp/PythonDataScienceHandbook` — prose-first, v4, few definitions.
//! * `googlecolab/colabtools` — Colab dialect, `!` shell escapes everywhere.

use neuromesh_index::notebook::{self, CellKind};
use std::path::{Path, PathBuf};

fn notebooks_under(root: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }
        if path.is_dir() {
            notebooks_under(&path, found);
        } else if notebook::is_notebook(&path) {
            found.push(path);
        }
    }
}

#[test]
#[ignore = "needs NM_AUDIT_PATH pointed at a checkout"]
fn audit_a_real_checkout() {
    let Ok(root) = std::env::var("NM_AUDIT_PATH") else {
        panic!("set NM_AUDIT_PATH to a directory holding .ipynb files");
    };
    let root = PathBuf::from(root);
    let mut found = Vec::new();
    notebooks_under(&root, &mut found);
    found.sort();
    assert!(!found.is_empty(), "no .ipynb under {}", root.display());

    let mut raw_total = 0usize;
    let mut view_total = 0usize;
    let mut refused: Vec<String> = Vec::new();
    let mut empty: Vec<String> = Vec::new();
    let mut cells_total = 0usize;

    println!(
        "\n{:>9}  {:>9}  {:>6}  {:>5}  file",
        "raw", "view", "kept", "cells"
    );
    for path in &found {
        let display = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(raw) = std::fs::read_to_string(path) else {
            refused.push(format!("{display} (unreadable / not utf-8)"));
            continue;
        };
        let Some(view) = notebook::source_view(&raw) else {
            refused.push(format!("{display} (not a notebook)"));
            continue;
        };
        let cells = notebook::cell_markers(&view)
            .iter()
            .filter(|m| m.kind == CellKind::Code)
            .count();
        if cells == 0 {
            empty.push(display.clone());
        }
        raw_total += raw.len();
        view_total += view.len();
        cells_total += cells;
        println!(
            "{:>9}  {:>9}  {:>5.1}%  {:>5}  {display}",
            raw.len(),
            view.len(),
            100.0 * view.len() as f64 / raw.len().max(1) as f64,
            cells
        );
    }

    println!(
        "\n--- {} notebooks under {} ---",
        found.len(),
        root.display()
    );
    println!("raw bytes   {raw_total}");
    println!("view bytes  {view_total}");
    println!(
        "kept        {:.1}%  (lower is better; the rest was output and prose)",
        100.0 * view_total as f64 / raw_total.max(1) as f64
    );
    println!("code cells  {cells_total}");
    if !empty.is_empty() {
        println!("\nno code cells found in {} notebook(s):", empty.len());
        for name in &empty {
            println!("  {name}");
        }
    }
    if !refused.is_empty() {
        println!("\nrefused {}:", refused.len());
        for name in &refused {
            println!("  {name}");
        }
    }

    // A checkout of notebooks that normalizes to nothing means the transform is
    // broken, not that the repository is unusual.
    assert!(
        cells_total > 0,
        "not one code cell was recovered from {} notebooks",
        found.len()
    );
}

#[test]
#[ignore = "needs NM_NOTEBOOK_PATH pointed at one .ipynb"]
fn dump_one_notebook() {
    let Ok(path) = std::env::var("NM_NOTEBOOK_PATH") else {
        panic!("set NM_NOTEBOOK_PATH to a .ipynb file");
    };
    let path = PathBuf::from(path);
    let raw = std::fs::read_to_string(&path).expect("read the notebook");
    let view = notebook::source_view(&raw).expect("normalize the notebook");
    println!("--- {} ---", path.display());
    println!("{view}");
    println!(
        "--- {} raw bytes -> {} view bytes ({:.1}% kept), {} markers ---",
        raw.len(),
        view.len(),
        100.0 * view.len() as f64 / raw.len().max(1) as f64,
        notebook::cell_markers(&view).len()
    );
}

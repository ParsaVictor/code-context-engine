//! Ordered cells as graph nodes.
//!
//! The Python grammar already reads a notebook's source view as one module,
//! which is what makes a `def` in cell 2 and its call in cell 9 resolve. What
//! it cannot express is that the module came in *pieces that ran in order*.
//! Two questions need that:
//!
//! * "What has to run before the training cell?" — an ordering, not a call
//!   graph. `Precedes` answers it directly.
//! * "Show me the cell that builds the loader" — a 60-cell notebook is one
//!   file node, and emitting the whole file to answer costs as much as opening
//!   it. A cell node carries the line span of one cell, so the packet can
//!   quote eleven lines instead of six hundred.
//!
//! Only code cells become nodes. A markdown cell survives in the view as two
//! or three comment lines; a node for it would cost more than it carries. Its
//! heading is not wasted though — it becomes the title of the code cell it
//! introduces, which is usually exactly what the prose was describing.

use crate::types::{AstAnalysisResult, ParsedRelationship, ParsedSymbol};
use neuromesh_core::{EdgeType, NodeType};
use neuromesh_index::notebook::{self, CellKind};
use std::path::Path;

/// Longest cell title kept. A title is untrusted text from the notebook, so it
/// is also stripped of anything that could end a line.
const MAX_TITLE: usize = 80;

pub fn notebook_overlay(path: &Path, content: &str, ast: &mut AstAnalysisResult) {
    let markers = notebook::cell_markers(content);
    if markers.is_empty() {
        return;
    }
    let lines: Vec<&str> = content.lines().collect();
    let file_hint = path.to_string_lossy().replace('\\', "/");

    let mut pending_title: Option<String> = None;
    let mut previous_cell: Option<String> = None;

    for (index, marker) in markers.iter().enumerate() {
        // A cell runs from its marker to the line before the next one.
        let end_line = markers
            .get(index + 1)
            .map(|next| next.line)
            .unwrap_or(lines.len() + 1);
        let body = lines
            .get(marker.line..end_line.saturating_sub(1))
            .unwrap_or(&[]);

        match marker.kind {
            CellKind::Markdown | CellKind::Raw => {
                // Carry the heading forward to whichever code cell it precedes.
                pending_title = body
                    .iter()
                    .find_map(|line| line.strip_prefix("# "))
                    .map(|text| title_of(text.trim_start_matches('#').trim()));
            }
            CellKind::Code => {
                let title = pending_title.take().or_else(|| {
                    body.iter()
                        .map(|line| line.trim())
                        .find(|line| !line.is_empty() && !line.starts_with('#'))
                        .map(title_of)
                });
                let name = format!("cell{}", marker.number);
                let signature = match &title {
                    Some(title) => format!("cell {} — {title}", marker.number),
                    None => format!("cell {}", marker.number),
                };
                ast.symbols.push(ParsedSymbol::new(
                    name.clone(),
                    NodeType::NotebookCell,
                    Some(signature),
                    marker.line..end_line,
                    false,
                ));
                if let Some(previous) = previous_cell.replace(name.clone()) {
                    ast.relationships.push(ParsedRelationship {
                        source_symbol: previous,
                        target_symbol: name,
                        relationship: EdgeType::Precedes,
                        target_file_hint: Some(file_hint.clone()),
                        receiver_hint: None,
                    });
                }
            }
        }
    }
}

/// One line of untrusted notebook text, safe to store in a signature.
fn title_of(text: &str) -> String {
    let mut out = String::with_capacity(text.len().min(MAX_TITLE));
    for ch in text.chars() {
        if out.len() >= MAX_TITLE {
            break;
        }
        if ch.is_control() || matches!(ch, '\u{85}' | '\u{2028}' | '\u{2029}') {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::notebook_overlay;
    use crate::types::AstAnalysisResult;
    use neuromesh_core::{EdgeType, NodeType};
    use std::path::Path;

    const VIEW: &str = "\
# %% notebook: nbformat 4.5, kernel python3 (python)

# %% [markdown] cell 1
# # Build the loader

# %% cell 2
from torch.utils.data import DataLoader
loader = DataLoader(train_set)

# %% cell 3
for batch in loader:
    step(batch)
";

    #[test]
    fn code_cells_become_ordered_nodes() {
        let mut ast = AstAnalysisResult::default();
        notebook_overlay(Path::new("nb/train.ipynb"), VIEW, &mut ast);

        let cells: Vec<_> = ast
            .symbols
            .iter()
            .filter(|s| s.symbol_type == NodeType::NotebookCell)
            .collect();
        assert_eq!(
            cells.len(),
            2,
            "only code cells get nodes, got {:?}",
            ast.symbols
        );
        assert_eq!(cells[0].name, "cell2");
        assert_eq!(cells[1].name, "cell3");

        // The markdown heading above cell 2 titles it.
        assert_eq!(
            cells[0].signature.as_deref(),
            Some("cell 2 — Build the loader")
        );
        // With no heading, the first real line of code does.
        assert_eq!(
            cells[1].signature.as_deref(),
            Some("cell 3 — for batch in loader:")
        );

        // Cell 2 spans its marker through the line before cell 3's marker.
        let marker_line = VIEW
            .lines()
            .position(|l| l == "# %% cell 2")
            .expect("marker")
            + 1;
        assert_eq!(cells[0].line_range.start, marker_line);
        assert!(cells[0].line_range.end > cells[0].line_range.start + 1);

        let precedes: Vec<_> = ast
            .relationships
            .iter()
            .filter(|r| r.relationship == EdgeType::Precedes)
            .collect();
        assert_eq!(precedes.len(), 1);
        assert_eq!(precedes[0].source_symbol, "cell2");
        assert_eq!(precedes[0].target_symbol, "cell3");
        assert_eq!(
            precedes[0].target_file_hint.as_deref(),
            Some("nb/train.ipynb"),
            "the hint keeps two notebooks' cell3 apart"
        );
    }

    #[test]
    fn a_plain_python_file_gets_no_cells() {
        let mut ast = AstAnalysisResult::default();
        notebook_overlay(
            Path::new("src/train.py"),
            "def main():\n    pass\n",
            &mut ast,
        );
        assert!(ast.symbols.is_empty());
        assert!(ast.relationships.is_empty());
    }

    #[test]
    fn a_title_cannot_carry_a_line_break() {
        let mut ast = AstAnalysisResult::default();
        let view = "# %% cell 1\nx = 1  # \u{2028}def forged(): pass\n";
        notebook_overlay(Path::new("nb/a.ipynb"), view, &mut ast);
        let signature = ast.symbols[0].signature.clone().unwrap();
        assert!(
            !signature.contains('\u{2028}'),
            "line separators must not survive into a signature: {signature:?}"
        );
    }
}

//! `.ipynb` → a deterministic Python source view.
//!
//! A notebook is a JSON document whose bulk is *output*: base64 PNGs, HTML
//! reprs, stack traces. Handing that JSON to the rest of the engine would be
//! wrong twice over — the parser sees no code, and the packet spends its whole
//! token budget on an image. So a notebook is converted, once, at every point
//! where the engine reads a file, into the source view below:
//!
//! ```text
//! # %% notebook: nbformat 4.5, kernel python3 (python)
//!
//! # %% [markdown] cell 1
//! # # Fine-tuning a detector
//!
//! # %% cell 2
//! import torch
//! from torch.utils.data import DataLoader
//!
//! # %% cell 3
//! model = Detector()
//! ```
//!
//! The `# %%` marker is the jupytext "percent" format, which VS Code, PyCharm
//! and Spyder already understand — the view is a real `.py` a human can read.
//!
//! Two consequences make this worth more than a parser of its own:
//!
//! * **Cross-cell DEF-USE comes for free.** Cells concatenated in document
//!   order *are* the module the notebook denotes, so a `def` in cell 2 and its
//!   call in cell 9 land in one file for the existing Python grammar, and the
//!   PyTorch overlay types the artifacts on top without knowing about
//!   notebooks at all.
//! * **The same bytes reach every consumer** — hashing, token counting, the
//!   skeletonizer, and `read_source` for the packet — so line ranges recorded
//!   at index time still address the right lines at emission time.
//!
//! # Trust boundary
//!
//! Every byte here comes from a third-party repository, so this module treats
//! the file as hostile input:
//!
//! * Nothing is executed, at any point. This is a text transform.
//! * Retained memory is bounded independently of the input: cells, per-cell
//!   source, and the whole view each have a cap, and outputs are skipped
//!   during deserialization rather than parsed and dropped.
//! * Text that is *not* code — markdown, raw cells, exception messages — is
//!   emitted as comments with every Unicode line terminator replaced, so no
//!   prose can break out of its `#` and forge a symbol. `\u{2028}` is the
//!   interesting one: a JSON string may carry it literally, several lexers
//!   treat it as a line break, and `split('\n')` does not.
//! * A malformed, truncated, or oversized notebook yields `None`/`Err` and the
//!   caller falls back to treating the file as ordinary text. It is never a
//!   panic and never an index failure.

use serde::de::{self, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

/// Largest `.ipynb` read at all. Outputs make notebooks far bigger than the
/// 2 MB text cap the walker applies to source files, but the file still has to
/// fit in memory to be parsed, so the ceiling is explicit rather than absent.
pub const MAX_NOTEBOOK_BYTES: u64 = 32 * 1024 * 1024;

/// Cells retained. Later cells are drained without being kept.
const MAX_CELLS: usize = 4_096;

/// nbformat v3 worksheets retained. Only the first non-empty one is read.
const MAX_WORKSHEETS: usize = 64;

/// Ceiling on the rendered view, so the retained string cannot be amplified by
/// a notebook made of many small cells.
const MAX_VIEW_BYTES: usize = 2 * 1024 * 1024;

/// Ceiling on one cell's `source`, applied while deserializing.
const MAX_CELL_SOURCE_BYTES: usize = 256 * 1024;

/// Markdown contributes headings and one prose line — enough for retrieval to
/// see that a cell is titled "Evaluate mAP", without paying for the essay.
const MAX_MARKDOWN_BYTES: usize = 400;

/// Ceiling on `ename: evalue` echoed from an error output.
const MAX_ERROR_BYTES: usize = 160;

/// Characters some lexer, somewhere, treats as ending a line. Replaced with a
/// space whenever untrusted text is emitted inside a comment.
///
/// This is Unicode's mandatory-break set plus the three information separators
/// that `str::splitlines` honours in Python. No tokenizer in this pipeline
/// splits on the separators today — `str::lines` is LF-only — but the guarantee
/// this list backs is "prose cannot end its own comment", and that is worth
/// stating over the full set a reader might reasonably expect rather than the
/// subset that happens to matter to the current consumers.
const LINE_TERMINATORS: [char; 10] = [
    '\n', '\r', '\u{0b}', '\u{0c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}', '\u{2029}',
];

/// The literal that opens every marker this module writes.
///
/// It is load-bearing: [`cell_markers`] reads it back to recover cell spans, so
/// a marker has to mean "the renderer put this here" and nothing else. Cells
/// are free to contain the same text — a notebook about jupytext certainly
/// will — so any emitted line that would start with this prefix is shifted one
/// space right, which leaves it a valid comment and no longer a marker.
pub const MARKER_PREFIX: &str = "# %% ";

/// True when `path` names a Jupyter notebook.
pub fn is_notebook(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("ipynb"))
}

/// Read `path` as text, converting a notebook to its source view.
///
/// Non-notebooks are read unchanged, so this is a drop-in for
/// `fs::read_to_string` at every point the engine loads workspace source.
pub fn read_source_text(path: &Path) -> io::Result<String> {
    if !is_notebook(path) {
        return fs::read_to_string(path);
    }

    // Bounded at the reader, not by a prior `metadata()` call: the size a
    // stat reports is not the size the next `read` returns, and a file that
    // grows between the two is exactly the shape an attacker would choose.
    // One byte over the cap is read on purpose, so "too big" is detectable
    // without trusting anything outside this function.
    let mut raw = String::new();
    File::open(path)?
        .take(MAX_NOTEBOOK_BYTES + 1)
        .read_to_string(&mut raw)?;
    if raw.len() as u64 > MAX_NOTEBOOK_BYTES {
        tracing::warn!(
            path = %path.display(),
            limit = MAX_NOTEBOOK_BYTES,
            "notebook skipped: larger than the notebook size limit"
        );
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("notebook is over the {MAX_NOTEBOOK_BYTES} byte limit"),
        ));
    }

    source_view(&raw).ok_or_else(|| {
        tracing::warn!(
            path = %path.display(),
            "file has a .ipynb extension but did not parse as a notebook"
        );
        io::Error::new(
            io::ErrorKind::InvalidData,
            "file has a .ipynb extension but is not a readable notebook",
        )
    })
}

/// Render a notebook's JSON as Python. `None` when `raw` is not a notebook.
///
/// Pure and deterministic: the same bytes in give the same bytes out, which is
/// what lets the content hash drive incremental reindexing.
pub fn source_view(raw: &str) -> Option<String> {
    let notebook: NotebookFile = serde_json::from_str(raw).ok()?;
    let cells = notebook.cells();
    if cells.is_empty() && notebook.nbformat.is_none() {
        // Valid JSON, but nothing that identifies it as a notebook.
        return None;
    }
    Some(render(&notebook, cells))
}

/// What a cell contributes to the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Code,
    Markdown,
    Raw,
}

/// One cell located in a rendered source view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellMarker {
    /// The notebook's own 1-based cell index, so a graph node can say "cell 12"
    /// and mean what Jupyter shows as cell 12.
    pub number: usize,
    pub kind: CellKind,
    /// 1-based line of the marker inside the view.
    pub line: usize,
}

/// Recover the cell spans of a view produced by [`source_view`].
///
/// The parser needs the boundaries to give each cell a node, and reading them
/// back out of the rendered text keeps the format described in exactly one
/// place. Numbers must strictly increase — the renderer emits them in order,
/// so anything else is not ours and is skipped.
pub fn cell_markers(view: &str) -> Vec<CellMarker> {
    let mut markers: Vec<CellMarker> = Vec::new();
    for (index, line) in view.lines().enumerate() {
        let Some(rest) = line.strip_prefix(MARKER_PREFIX) else {
            continue;
        };
        let (kind, rest) = if let Some(rest) = rest.strip_prefix("[markdown] ") {
            (CellKind::Markdown, rest)
        } else if let Some(rest) = rest.strip_prefix("[raw] ") {
            (CellKind::Raw, rest)
        } else {
            (CellKind::Code, rest)
        };
        let Some(number) = rest
            .strip_prefix("cell ")
            .and_then(|n| n.trim().parse::<usize>().ok())
        else {
            continue;
        };
        if markers.last().is_some_and(|last| last.number >= number) {
            continue;
        }
        markers.push(CellMarker {
            number,
            kind,
            line: index + 1,
        });
    }
    markers
}

fn render(notebook: &NotebookFile, cells: &[Cell]) -> String {
    let mut out = String::with_capacity(1024);
    out.push_str("# %% notebook: nbformat ");
    match (notebook.nbformat, notebook.nbformat_minor) {
        (Some(major), Some(minor)) => out.push_str(&format!("{major}.{minor}")),
        (Some(major), None) => out.push_str(&major.to_string()),
        _ => out.push_str("unknown"),
    }
    if let Some(kernel) = notebook.kernel_name() {
        out.push_str(", kernel ");
        push_sanitized(&mut out, kernel, 64);
    }
    if let Some(language) = notebook.language() {
        out.push_str(" (");
        push_sanitized(&mut out, language, 32);
        out.push(')');
    }
    out.push('\n');

    for (index, cell) in cells.iter().enumerate() {
        if out.len() >= MAX_VIEW_BYTES {
            out.push_str("\n# %% … notebook truncated at the source-view limit\n");
            break;
        }
        let number = index + 1;
        match cell.kind() {
            CellKind::Code => {
                out.push_str(&format!("\n# %% cell {number}\n"));
                push_code(&mut out, cell.body());
                if let Some(error) = cell.outputs.error() {
                    out.push_str("# %% error: ");
                    push_sanitized(&mut out, error, MAX_ERROR_BYTES);
                    out.push('\n');
                }
            }
            CellKind::Markdown => {
                let digest = markdown_digest(cell.body());
                if !digest.is_empty() {
                    out.push_str(&format!("\n# %% [markdown] cell {number}\n"));
                    out.push_str(&digest);
                }
            }
            CellKind::Raw => {
                let digest = markdown_digest(cell.body());
                if !digest.is_empty() {
                    out.push_str(&format!("\n# %% [raw] cell {number}\n"));
                    out.push_str(&digest);
                }
            }
        }
    }
    out
}

/// Emit a code cell, commenting out the parts that are not Python.
///
/// IPython accepts three things a Python grammar does not: line magics
/// (`%matplotlib inline`), shell escapes (`!pip install torch`), and help
/// (`?DataLoader`). Leaving them in costs the whole cell — tree-sitter gives up
/// at the syntax error and the fallback regex parser sees far less. A cell that
/// *opens* with a cell magic (`%%bash`) is not Python at all, so all of it
/// becomes comments.
fn push_code(out: &mut String, source: &str) {
    let normalized = normalize_newlines(source);
    let whole_cell_is_magic = normalized
        .lines()
        .find(|line| !line.trim().is_empty())
        .is_some_and(is_cell_magic);

    for line in normalized.lines() {
        if out.len() >= MAX_VIEW_BYTES {
            return;
        }
        if whole_cell_is_magic || is_line_magic(line) {
            push_comment(out, line);
        } else {
            // Code is emitted verbatim: there is no comment to break out of,
            // and rewriting it would move the columns a diagnostic points at.
            // The one exception is a line that would read as a cell marker.
            if line.starts_with("# %%") {
                out.push(' ');
            }
            out.push_str(line);
            out.push('\n');
        }
    }
}

/// True when the line opens an IPython *cell* magic, making the rest of the
/// cell some other language.
fn is_cell_magic(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("%%") && starts_identifier(&trimmed[2..])
}

/// True when the line is IPython syntax rather than Python.
///
/// The `%` case deliberately requires the name to touch the sign: a
/// continuation line like `     % divisor)` inside brackets is modulo, not
/// `%divisor`, and commenting it would corrupt working code. `!` is safe to
/// claim at line start because no Python logical line may begin with it —
/// except `!=`, which can legally continue an expression inside brackets.
fn is_line_magic(line: &str) -> bool {
    let trimmed = line.trim_start();
    let mut chars = trimmed.chars();
    match chars.next() {
        Some('%') => {
            let rest = trimmed.trim_start_matches('%');
            let signs = trimmed.len() - rest.len();
            (1..=2).contains(&signs) && starts_identifier(rest)
        }
        Some('!') => !trimmed.starts_with("!="),
        // `?name` / `??name` is help syntax. A trailing `name?` is too, but it
        // is indistinguishable from prose inside a triple-quoted string, so
        // only the unambiguous leading form is claimed.
        Some('?') => true,
        _ => false,
    }
}

fn starts_identifier(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
}

/// Headings plus the first prose line, as comments. Retrieval wants to know a
/// cell is called "## Evaluate mAP"; it does not want to pay for the paragraph
/// underneath.
fn markdown_digest(source: &str) -> String {
    let normalized = normalize_newlines(source);
    let mut out = String::new();
    let mut prose_taken = false;
    for line in normalized.lines() {
        if out.len() >= MAX_MARKDOWN_BYTES {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let heading = trimmed.starts_with('#');
        if heading {
            push_comment(&mut out, trimmed);
        } else if !prose_taken {
            prose_taken = true;
            push_comment(&mut out, trimmed);
        }
    }
    out
}

/// Emit untrusted text as exactly one comment line.
///
/// Every Unicode line terminator becomes a space, so the `#` cannot be escaped
/// no matter what the cell contains.
fn push_comment(out: &mut String, text: &str) {
    // `# ` + `%%…` would read back as a marker, so prose that opens with `%%`
    // gets one more space. It stays a comment either way.
    if text.starts_with("%%") {
        out.push_str("#  ");
    } else {
        out.push_str("# ");
    }
    for ch in text.chars() {
        if LINE_TERMINATORS.contains(&ch) {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out.push('\n');
}

/// Append at most `limit` bytes of untrusted text, on one line, no newline.
fn push_sanitized(out: &mut String, text: &str, limit: usize) {
    let clipped = &text[..floor_boundary(text, limit)];
    for ch in clipped.chars() {
        if LINE_TERMINATORS.contains(&ch) {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
}

/// `\r\n` and lone `\r` become `\n`, so cell text has one line per line and
/// the view's line numbers address what the reader sees.
fn normalize_newlines(source: &str) -> String {
    if !source.contains('\r') {
        return source.to_string();
    }
    source.replace("\r\n", "\n").replace('\r', "\n")
}

/// Largest index `<= limit` that is a char boundary.
fn floor_boundary(s: &str, limit: usize) -> usize {
    if limit >= s.len() {
        return s.len();
    }
    let mut end = limit;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

// ---------------------------------------------------------------------------
// Deserialization
//
// Only the fields below are retained. Everything else — `outputs[].data`,
// `metadata`, `attachments`, `execution_count` — is skipped by serde without
// being materialized, which is what keeps a notebook full of base64 PNGs from
// being parsed into memory a second time. serde_json's own 128-level recursion
// limit stops a deeply nested value from growing the stack.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct NotebookFile {
    #[serde(default)]
    cells: Cells,
    /// nbformat v3 kept cells inside worksheets. Capped like `cells` is:
    /// a `Vec` grown straight from the file is a hole in the memory bound,
    /// and `{"cells":[]}` is thirteen bytes.
    #[serde(default)]
    worksheets: Worksheets,
    #[serde(default)]
    metadata: NotebookMetadata,
    #[serde(default)]
    nbformat: Option<i64>,
    #[serde(default)]
    nbformat_minor: Option<i64>,
}

impl NotebookFile {
    fn cells(&self) -> &[Cell] {
        if !self.cells.0.is_empty() {
            return &self.cells.0;
        }
        self.worksheets
            .0
            .iter()
            .find(|w| !w.cells.0.is_empty())
            .map(|w| w.cells.0.as_slice())
            .unwrap_or(&[])
    }

    fn kernel_name(&self) -> Option<&str> {
        self.metadata
            .kernelspec
            .as_ref()
            .and_then(|k| k.name.as_deref())
    }

    fn language(&self) -> Option<&str> {
        self.metadata
            .language_info
            .as_ref()
            .and_then(|l| l.name.as_deref())
            .or_else(|| {
                self.metadata
                    .kernelspec
                    .as_ref()
                    .and_then(|k| k.language.as_deref())
            })
    }
}

#[derive(Deserialize, Default)]
struct Worksheet {
    #[serde(default)]
    cells: Cells,
}

/// The worksheet list, capped while reading. Only the first worksheet with any
/// cells is ever used, so the cap is small.
#[derive(Default)]
struct Worksheets(Vec<Worksheet>);

impl<'de> Deserialize<'de> for Worksheets {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(WorksheetsVisitor)
    }
}

struct WorksheetsVisitor;

impl<'de> Visitor<'de> for WorksheetsVisitor {
    type Value = Worksheets;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an array of worksheets")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Worksheets, A::Error> {
        let mut sheets: Vec<Worksheet> = Vec::new();
        loop {
            if sheets.len() < MAX_WORKSHEETS {
                match seq.next_element::<Worksheet>()? {
                    Some(sheet) => sheets.push(sheet),
                    None => break,
                }
            } else if seq.next_element::<IgnoredAny>()?.is_none() {
                break;
            }
        }
        Ok(Worksheets(sheets))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Worksheets, E> {
        Ok(Worksheets::default())
    }

    fn visit_none<E: de::Error>(self) -> Result<Worksheets, E> {
        Ok(Worksheets::default())
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Worksheets, D::Error> {
        d.deserialize_any(WorksheetsVisitor)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Worksheets, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(Worksheets::default())
    }
}

#[derive(Deserialize, Default)]
struct NotebookMetadata {
    #[serde(default)]
    kernelspec: Option<KernelSpec>,
    #[serde(default)]
    language_info: Option<LanguageInfo>,
}

#[derive(Deserialize)]
struct KernelSpec {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    language: Option<String>,
}

#[derive(Deserialize)]
struct LanguageInfo {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct Cell {
    #[serde(default)]
    cell_type: Option<String>,
    #[serde(default)]
    source: Source,
    /// nbformat v3 named a code cell's body `input`.
    #[serde(default)]
    input: Source,
    #[serde(default)]
    outputs: Outputs,
}

impl Cell {
    fn kind(&self) -> CellKind {
        match self.cell_type.as_deref() {
            Some("markdown") | Some("heading") => CellKind::Markdown,
            Some("raw") => CellKind::Raw,
            // An absent or unknown `cell_type` is treated as code: guessing
            // "code" costs a possible syntax error the parser recovers from,
            // guessing "markdown" silently drops real source.
            _ => CellKind::Code,
        }
    }

    /// A cell's text, wherever the format of the day put it.
    fn body(&self) -> &str {
        if self.source.0.is_empty() {
            return &self.input.0;
        }
        &self.source.0
    }
}

/// A cell's `source`: a string in some notebooks, an array of lines in others,
/// occasionally `null` or something else entirely.
///
/// Written as a visitor rather than `#[serde(untagged)]` because untagged
/// buffers the whole value before choosing a variant — the opposite of what a
/// multi-megabyte cell needs — and because the cap has to be applied while
/// reading, not after.
#[derive(Default)]
struct Source(String);

impl Source {
    fn text(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Source {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(SourceVisitor)
    }
}

struct SourceVisitor;

impl<'de> Visitor<'de> for SourceVisitor {
    type Value = Source;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string or an array of strings")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Source, E> {
        Ok(Source(
            v[..floor_boundary(v, MAX_CELL_SOURCE_BYTES)].to_string(),
        ))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Source, A::Error> {
        let mut text = String::new();
        while let Some(Line(part)) = seq.next_element::<Line>()? {
            if text.len() < MAX_CELL_SOURCE_BYTES {
                let room = MAX_CELL_SOURCE_BYTES - text.len();
                text.push_str(&part[..floor_boundary(&part, room)]);
            }
            // The rest of the array is still drained, so the value is consumed
            // and the parse stays in step.
        }
        Ok(Source(text))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Source, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(Source::default())
    }

    fn visit_unit<E: de::Error>(self) -> Result<Source, E> {
        Ok(Source::default())
    }

    fn visit_none<E: de::Error>(self) -> Result<Source, E> {
        Ok(Source::default())
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Source, D::Error> {
        d.deserialize_any(SourceVisitor)
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Source, E> {
        Ok(Source::default())
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Source, E> {
        Ok(Source::default())
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Source, E> {
        Ok(Source::default())
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Source, E> {
        Ok(Source::default())
    }
}

/// One element of a `source` array. Non-strings contribute nothing but are
/// still consumed.
struct Line(String);

impl<'de> Deserialize<'de> for Line {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(LineVisitor)
    }
}

struct LineVisitor;

impl<'de> Visitor<'de> for LineVisitor {
    type Value = Line;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a source line")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Line, E> {
        Ok(Line(
            v[..floor_boundary(v, MAX_CELL_SOURCE_BYTES)].to_string(),
        ))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Line, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(Line(String::new()))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Line, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(Line(String::new()))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Line, E> {
        Ok(Line(String::new()))
    }

    fn visit_none<E: de::Error>(self) -> Result<Line, E> {
        Ok(Line(String::new()))
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Line, D::Error> {
        d.deserialize_any(LineVisitor)
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Line, E> {
        Ok(Line(String::new()))
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Line, E> {
        Ok(Line(String::new()))
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Line, E> {
        Ok(Line(String::new()))
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Line, E> {
        Ok(Line(String::new()))
    }
}

/// The cell list, capped while reading.
///
/// A `Vec<Cell>` grown straight from the file lets a notebook of a million
/// two-field cells cost far more in structs than it did in bytes. Past
/// `MAX_CELLS` the elements are drained and dropped.
#[derive(Default)]
struct Cells(Vec<Cell>);

impl<'de> Deserialize<'de> for Cells {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(CellsVisitor)
    }
}

struct CellsVisitor;

impl<'de> Visitor<'de> for CellsVisitor {
    type Value = Cells;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an array of cells")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Cells, A::Error> {
        let mut cells = Vec::new();
        loop {
            if cells.len() < MAX_CELLS {
                match seq.next_element::<Cell>()? {
                    Some(cell) => cells.push(cell),
                    None => break,
                }
            } else if seq.next_element::<IgnoredAny>()?.is_none() {
                break;
            }
        }
        Ok(Cells(cells))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Cells, E> {
        Ok(Cells::default())
    }

    fn visit_none<E: de::Error>(self) -> Result<Cells, E> {
        Ok(Cells::default())
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Cells, D::Error> {
        d.deserialize_any(CellsVisitor)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Cells, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(Cells::default())
    }
}

/// A cell's outputs, reduced to "did it raise, and what".
///
/// The rest — image/png, text/html, stdout — is skipped without being
/// materialized. `ename: evalue` is kept because a cell that ended in
/// `CUDA out of memory` is exactly what a question about the notebook is
/// usually about.
#[derive(Default)]
struct Outputs(Option<String>);

impl Outputs {
    fn error(&self) -> Option<&str> {
        self.0.as_deref()
    }
}

impl<'de> Deserialize<'de> for Outputs {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(OutputsVisitor)
    }
}

struct OutputsVisitor;

impl<'de> Visitor<'de> for OutputsVisitor {
    type Value = Outputs;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an array of outputs")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Outputs, A::Error> {
        let mut first_error: Option<String> = None;
        while let Some(output) = seq.next_element::<Output>()? {
            if first_error.is_some() {
                continue;
            }
            let name = output.ename.text();
            let value = output.evalue.text();
            if name.is_empty() && value.is_empty() {
                continue;
            }
            let joined = if value.is_empty() {
                name.to_string()
            } else {
                format!("{name}: {value}")
            };
            first_error = Some(joined[..floor_boundary(&joined, MAX_ERROR_BYTES)].to_string());
        }
        Ok(Outputs(first_error))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Outputs, E> {
        Ok(Outputs::default())
    }

    fn visit_none<E: de::Error>(self) -> Result<Outputs, E> {
        Ok(Outputs::default())
    }

    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Outputs, D::Error> {
        d.deserialize_any(OutputsVisitor)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Outputs, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(Outputs::default())
    }
}

#[derive(Deserialize)]
struct Output {
    #[serde(default)]
    ename: Source,
    #[serde(default)]
    evalue: Source,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notebook(cells: &str) -> String {
        format!(
            r#"{{"cells":[{cells}],"metadata":{{"kernelspec":{{"name":"python3","language":"python"}},"language_info":{{"name":"python"}}}},"nbformat":4,"nbformat_minor":5}}"#
        )
    }

    fn code(source: &str) -> String {
        format!(
            r#"{{"cell_type":"code","execution_count":1,"metadata":{{}},"outputs":[],"source":{}}}"#,
            serde_json::to_string(source).unwrap()
        )
    }

    fn view_of(cells: &str) -> String {
        source_view(&notebook(cells)).expect("a notebook")
    }

    // -- the shape of the view ---------------------------------------------

    #[test]
    fn cells_concatenate_into_one_python_module() {
        let raw = notebook(&format!(
            "{},{},{}",
            r##"{"cell_type":"markdown","source":["# Fine-tune a detector\n"]}"##,
            code("from torch.utils.data import DataLoader\nimport torch\n"),
            code("def build_loader(root):\n    return DataLoader(root)\n\nloader = build_loader('data')\n")
        ));
        let view = source_view(&raw).expect("a notebook");

        assert!(view.starts_with("# %% notebook: nbformat 4.5, kernel python3 (python)"));
        assert!(view.contains("# %% [markdown] cell 1"));
        assert!(view.contains("# # Fine-tune a detector"));
        assert!(view.contains("# %% cell 2"));
        assert!(view.contains("import torch"));
        assert!(view.contains("# %% cell 3"));
        // The def and the call that uses it are in one file, which is the
        // whole point: cross-cell DEF-USE is now ordinary name resolution.
        assert!(view.contains("def build_loader(root):"));
        assert!(view.contains("loader = build_loader('data')"));
    }

    #[test]
    fn source_may_be_a_string_or_an_array_of_lines() {
        let as_array = view_of(r#"{"cell_type":"code","source":["import os\n","import sys\n"]}"#);
        let as_string = view_of(r#"{"cell_type":"code","source":"import os\nimport sys\n"}"#);
        assert!(as_array.contains("import os\nimport sys"));
        assert_eq!(as_array, as_string, "both spellings must render the same");
    }

    #[test]
    fn nbformat_v3_keeps_its_cells_in_worksheets() {
        let raw = r#"{"worksheets":[{"cells":[
            {"cell_type":"code","input":["import numpy as np\n"],"language":"python"}
        ]}],"metadata":{},"nbformat":3,"nbformat_minor":0}"#;
        let view = source_view(raw).expect("a v3 notebook");
        assert!(view.contains("# %% cell 1"), "{view}");
        assert!(view.contains("import numpy as np"), "{view}");
    }

    #[test]
    fn an_unknown_cell_type_is_treated_as_code() {
        // Losing real source is worse than a syntax error the parser recovers
        // from, so anything unlabelled is read as code.
        let view = view_of(r#"{"source":"model = Detector()\n"}"#);
        assert!(view.contains("model = Detector()"), "{view}");
    }

    #[test]
    fn crlf_and_lone_cr_become_one_line_each() {
        let view = view_of(&code("a = 1\r\nb = 2\rc = 3\n"));
        assert!(view.contains("a = 1\nb = 2\nc = 3"), "{view:?}");
        assert!(!view.contains('\r'), "no carriage returns survive");
    }

    #[test]
    fn the_view_is_deterministic() {
        let raw = notebook(&code("x = 1\n"));
        assert_eq!(source_view(&raw), source_view(&raw));
    }

    // -- IPython syntax that is not Python ---------------------------------

    #[test]
    fn line_magics_and_shell_escapes_are_commented_out() {
        let view = view_of(&code(
            "%matplotlib inline\n!pip install torch\n?DataLoader\nimport torch\n",
        ));
        assert!(view.contains("# %matplotlib inline"), "{view}");
        assert!(view.contains("# !pip install torch"), "{view}");
        assert!(view.contains("# ?DataLoader"), "{view}");
        assert!(
            view.contains("\nimport torch"),
            "real code stays untouched: {view}"
        );
    }

    #[test]
    fn a_cell_magic_comments_the_whole_cell() {
        let view = view_of(&code("%%bash\nrm -rf build\nmake all\n"));
        for line in view
            .lines()
            .skip_while(|l| !l.starts_with("# %% cell"))
            .skip(1)
        {
            assert!(
                line.is_empty() || line.starts_with('#'),
                "a %%bash cell is shell, not Python: {line:?}"
            );
        }
        assert!(view.contains("%%bash"), "{view}");
        assert!(
            view.contains("# rm -rf build") && view.contains("# make all"),
            "{view}"
        );
    }

    #[test]
    fn a_modulo_continuation_is_not_mistaken_for_a_magic() {
        // `% divisor)` continues an expression inside brackets. Commenting it
        // would silently break working code, so a magic must have its name
        // touching the sign.
        let view = view_of(&code("remainder = (numerator\n    % divisor)\n"));
        assert!(
            view.contains("    % divisor)"),
            "modulo must survive verbatim: {view}"
        );
        assert!(!view.contains("# % divisor"), "{view}");
    }

    #[test]
    fn a_not_equal_continuation_is_not_mistaken_for_a_shell_escape() {
        let view = view_of(&code("ok = (left\n    != right)\n"));
        assert!(view.contains("    != right)"), "{view}");
    }

    // -- markdown pays for headings, not essays ----------------------------

    #[test]
    fn markdown_contributes_headings_and_one_prose_line() {
        let essay =
            "## Evaluate mAP\nWe sweep the score threshold.\nThen we plot it.\nAnd again.\n";
        let view = view_of(&format!(
            r#"{{"cell_type":"markdown","source":{}}}"#,
            serde_json::to_string(essay).unwrap()
        ));
        assert!(view.contains("# ## Evaluate mAP"), "{view}");
        assert!(view.contains("# We sweep the score threshold."), "{view}");
        assert!(
            !view.contains("And again."),
            "the rest of the prose is not worth its tokens: {view}"
        );
    }

    #[test]
    fn an_empty_markdown_cell_contributes_nothing() {
        let view = view_of(r#"{"cell_type":"markdown","source":"   \n\n"}"#);
        assert!(!view.contains("[markdown]"), "{view}");
    }

    // -- outputs ------------------------------------------------------------

    #[test]
    fn outputs_never_reach_the_view() {
        let blob = "iVBORw0KGgoAAAANS".repeat(4_000);
        let raw = notebook(&format!(
            r#"{{"cell_type":"code","source":"plot()\n","outputs":[
                {{"output_type":"display_data","data":{{"image/png":"{blob}"}},"metadata":{{}}}}
            ]}}"#
        ));
        let view = source_view(&raw).expect("a notebook");
        assert!(
            !view.contains("iVBORw0KGgo"),
            "a base64 image must not be indexed as source"
        );
        assert!(
            view.len() < 200,
            "the view is code-sized, got {}",
            view.len()
        );
    }

    #[test]
    fn an_error_output_is_summarised_on_one_line() {
        let raw = notebook(
            r#"{"cell_type":"code","source":"train()\n","outputs":[
                {"output_type":"error","ename":"RuntimeError",
                 "evalue":"CUDA out of memory",
                 "traceback":["a very long traceback","and another frame"]}
            ]}"#,
        );
        let view = source_view(&raw).expect("a notebook");
        assert!(
            view.contains("# %% error: RuntimeError: CUDA out of memory"),
            "{view}"
        );
        assert!(
            !view.contains("traceback"),
            "the frames are not worth their tokens: {view}"
        );
    }

    #[test]
    fn an_error_message_cannot_carry_a_line_break() {
        let raw = notebook(
            r#"{"cell_type":"code","source":"train()\n","outputs":[
                {"output_type":"error","ename":"E","evalue":"boom def forged(): pass"}
            ]}"#,
        );
        let view = source_view(raw.as_str()).expect("a notebook");
        let error_line = view
            .lines()
            .find(|l| l.starts_with("# %% error:"))
            .expect("an error line");
        assert!(error_line.contains("def forged(): pass"));
        assert!(
            !error_line.contains('\u{2028}'),
            "the separator must be gone: {error_line:?}"
        );
    }

    // -- security: untrusted text cannot become code -----------------------

    #[test]
    fn a_unicode_line_separator_cannot_escape_a_comment() {
        // U+2028 is a line break to several lexers and is *not* one to
        // `split('\n')`. A markdown cell carrying it must not be able to end
        // its own comment and start a line of Python.
        let prose = "intro\u{2028}def forged(): pass\u{2029}x = 1\u{0085}y = 2\u{1e}z = 3";
        let view = view_of(&format!(
            r#"{{"cell_type":"markdown","source":{}}}"#,
            serde_json::to_string(prose).unwrap()
        ));
        for line in view.lines() {
            assert!(
                line.is_empty() || line.starts_with('#'),
                "every markdown line must stay a comment: {line:?}"
            );
        }
        // The text survives — it is simply flattened onto the comment line it
        // was already on, where no parser will read it as a definition.
        assert!(view.contains("def forged(): pass"));
        for terminator in LINE_TERMINATORS {
            if terminator == '\n' {
                continue;
            }
            assert!(
                !view.contains(terminator),
                "{terminator:?} survived into the view"
            );
        }
    }

    #[test]
    fn a_cell_cannot_forge_a_cell_marker() {
        // A notebook about jupytext contains `# %% cell 99` as ordinary text.
        // It must not read back as a real cell boundary.
        let view = view_of(&format!(
            "{},{}",
            code("# %% cell 99\nx = 1\n"),
            code("y = 2\n")
        ));
        let markers = cell_markers(&view);
        assert_eq!(
            markers.iter().map(|m| m.number).collect::<Vec<_>>(),
            vec![1, 2],
            "only the renderer's own markers count: {view}"
        );
        assert!(
            view.contains(" # %% cell 99"),
            "the lookalike stays, shifted one space: {view}"
        );
    }

    #[test]
    fn markdown_starting_with_percent_percent_cannot_forge_a_marker() {
        let view = view_of(&format!(
            "{},{}",
            r#"{"cell_type":"markdown","source":"%% cell 42"}"#,
            code("x = 1\n")
        ));
        let markers = cell_markers(&view);
        // Cell 1 is the markdown cell and gets a real marker of its own; what
        // must not appear is the 42 the prose asked for.
        assert_eq!(
            markers.iter().map(|m| m.number).collect::<Vec<_>>(),
            vec![1, 2],
            "{view}"
        );
        assert!(view.contains("%% cell 42"), "the prose is still there");
    }

    #[test]
    fn markers_must_increase_to_be_believed() {
        let view = "# %% cell 5\nx = 1\n# %% cell 2\ny = 2\n# %% cell 9\nz = 3\n";
        assert_eq!(
            cell_markers(view)
                .iter()
                .map(|m| m.number)
                .collect::<Vec<_>>(),
            vec![5, 9]
        );
    }

    #[test]
    fn a_nesting_bomb_is_consumed_without_overflowing_the_stack() {
        // Twenty thousand levels of nesting is well past any stack. It is safe
        // because every field this module does not keep is skipped by
        // serde_json's *iterative* value-skipper, and the two visitors that do
        // recurse — a `source` array holding arrays — bottom out after one
        // level into that same skipper. The assertion is therefore not "this
        // errors"; it is "this returns, cheaply, whatever it returns".
        let depth = 20_000;

        // Nesting under `source`, the one field with a hand-written visitor.
        let in_source = format!(
            r#"{{"cells":[{{"cell_type":"code","source":{}{}}}],"nbformat":4}}"#,
            "[".repeat(depth),
            "]".repeat(depth)
        );
        let view = source_view(&in_source).expect("structurally a notebook");
        assert!(
            view.len() < 256,
            "a bomb must not become a big node: {view:?}"
        );

        // Nesting under an ignored field.
        let in_metadata = format!(
            r#"{{"cells":[],"nbformat":4,"metadata":{}null{}}}"#,
            r#"{"a":"#.repeat(depth),
            "}".repeat(depth)
        );
        assert!(source_view(&in_metadata).is_some());

        // Nesting where a cell should be: not a notebook, and still no crash.
        let in_cells = format!(
            r#"{{"cells":[{}{}],"nbformat":4}}"#,
            "[".repeat(depth),
            "]".repeat(depth)
        );
        assert_eq!(source_view(&in_cells), None);

        // Unterminated nesting: malformed, refused.
        assert_eq!(source_view(&"[".repeat(depth)), None);
    }

    #[test]
    fn a_flood_of_worksheets_stops_at_the_worksheet_cap() {
        // v3's `worksheets` was the one list without a ceiling: thirteen bytes
        // of JSON bought an unbounded `Vec` entry.
        let sheets = vec![r#"{"cells":[]}"#; MAX_WORKSHEETS + 5_000].join(",");
        let raw = format!(r#"{{"worksheets":[{sheets}],"nbformat":3}}"#);
        let view = source_view(&raw).expect("a v3 notebook");
        assert!(cell_markers(&view).is_empty());

        // The cap must not cost a real notebook its cells: a populated sheet
        // inside the cap is still found.
        let mut sheets: Vec<&str> = vec![r#"{"cells":[]}"#; MAX_WORKSHEETS - 1];
        sheets.push(r#"{"cells":[{"cell_type":"code","input":"import numpy\n"}]}"#);
        let raw = format!(r#"{{"worksheets":[{}],"nbformat":3}}"#, sheets.join(","));
        let view = source_view(&raw).expect("a v3 notebook");
        assert!(view.contains("import numpy"), "{view}");
    }

    #[test]
    fn a_notebook_of_many_cells_stops_at_the_cell_cap() {
        let cells: Vec<String> = (0..MAX_CELLS + 500)
            .map(|i| code(&format!("x{i} = 1\n")))
            .collect();
        let view = source_view(&notebook(&cells.join(","))).expect("a notebook");
        let markers = cell_markers(&view);
        assert_eq!(markers.len(), MAX_CELLS);
        assert!(view.contains(&format!("x{} = 1", MAX_CELLS - 1)));
        assert!(!view.contains(&format!("x{} = 1", MAX_CELLS)));
    }

    #[test]
    fn one_enormous_cell_is_truncated_at_the_cell_cap() {
        let huge = "z = 1\n".repeat(MAX_CELL_SOURCE_BYTES); // ~1.5 MB of source
        let view = view_of(&code(&huge));
        assert!(
            view.len() <= MAX_CELL_SOURCE_BYTES + 512,
            "a single cell must not exceed its cap, got {}",
            view.len()
        );
    }

    #[test]
    fn the_whole_view_stops_at_the_view_cap() {
        // Many cells that are each under the per-cell cap still have to add up
        // to something bounded.
        let big = "q = 0\n".repeat(40_000); // ~240 KB per cell
        let cells: Vec<String> = (0..40).map(|_| code(&big)).collect();
        let view = source_view(&notebook(&cells.join(","))).expect("a notebook");
        assert!(
            view.len() <= MAX_VIEW_BYTES + MAX_CELL_SOURCE_BYTES,
            "view grew to {}",
            view.len()
        );
        assert!(view.contains("notebook truncated at the source-view limit"));
    }

    #[test]
    fn a_source_that_is_not_text_is_read_as_empty_rather_than_failing() {
        for odd in [
            r#"{"cell_type":"code","source":null}"#,
            r#"{"cell_type":"code","source":42}"#,
            r#"{"cell_type":"code","source":{"nested":"object"}}"#,
            r#"{"cell_type":"code","source":[1,2,{"a":"b"}]}"#,
            r#"{"cell_type":"code","source":true}"#,
        ] {
            let view = source_view(&notebook(odd))
                .unwrap_or_else(|| panic!("{odd} should still parse as a notebook"));
            assert!(view.contains("# %% cell 1"), "{odd} -> {view}");
        }
    }

    // -- what is not a notebook --------------------------------------------

    #[test]
    fn malformed_or_unrelated_json_is_not_a_notebook() {
        assert_eq!(source_view("not json at all"), None);
        assert_eq!(source_view("{"), None);
        assert_eq!(
            source_view(r#"{"name":"my-package","version":"1.0.0"}"#),
            None
        );
        assert_eq!(source_view("[]"), None);
        // Zero-byte `.ipynb` files are real — a sweep of 142 notebooks on one
        // machine turned up two. They must be skipped, not indexed as an
        // empty notebook with a header.
        assert_eq!(source_view(""), None);
        assert_eq!(source_view("   \n\n"), None);
    }

    #[test]
    fn an_empty_notebook_still_renders_a_header() {
        let view = source_view(r#"{"cells":[],"metadata":{},"nbformat":4,"nbformat_minor":5}"#)
            .expect("nbformat marks it a notebook");
        assert!(view.starts_with("# %% notebook: nbformat 4.5"));
        assert!(cell_markers(&view).is_empty());
    }

    // -- the disk entry point ----------------------------------------------

    #[test]
    fn is_notebook_matches_the_extension_case_insensitively() {
        assert!(is_notebook(Path::new("nb/Train.IPYNB")));
        assert!(is_notebook(Path::new("train.ipynb")));
        assert!(!is_notebook(Path::new("train.py")));
        assert!(!is_notebook(Path::new("ipynb")));
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nm-nb-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn read_source_text_passes_ordinary_files_through_unchanged() {
        let dir = temp_dir("plain");
        let path = dir.join("train.py");
        fs::write(&path, "def main():\n    pass\n").unwrap();
        assert_eq!(
            read_source_text(&path).unwrap(),
            "def main():\n    pass\n",
            "a .py file is not rewritten"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_source_text_returns_the_view_for_a_notebook() {
        let dir = temp_dir("view");
        let path = dir.join("train.ipynb");
        fs::write(&path, notebook(&code("import torch\n"))).unwrap();
        let text = read_source_text(&path).unwrap();
        assert!(text.contains("# %% cell 1"));
        assert!(text.contains("import torch"));
        assert!(
            !text.contains("\"cell_type\""),
            "no raw JSON reaches the engine"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_named_ipynb_that_is_not_a_notebook_is_refused() {
        let dir = temp_dir("bogus");
        let path = dir.join("notes.ipynb");
        fs::write(&path, "just some text\n").unwrap();
        let err = read_source_text(&path).expect_err("not a notebook");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_oversized_notebook_is_refused_at_the_reader() {
        let dir = temp_dir("huge");
        let path = dir.join("huge.ipynb");
        let filler = "x".repeat(1024 * 1024);
        let mut raw = String::from("{\"cells\":[],\"nbformat\":4,\"pad\":\"");
        for _ in 0..(MAX_NOTEBOOK_BYTES / (1024 * 1024) + 1) {
            raw.push_str(&filler);
        }
        raw.push_str("\"}");
        fs::write(&path, &raw).unwrap();
        let err = read_source_text(&path).expect_err("over the limit");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("over the"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_size_limit_holds_even_if_the_file_lies_about_its_length() {
        // The bound has to live at the reader. Checking `metadata().len()` and
        // then reading is a race: the size that comes back from the stat is not
        // the size the read returns. Simulated here by making the *cap* the
        // thing under test — the read stops at the cap no matter what any
        // earlier measurement said.
        let dir = temp_dir("bound");
        let path = dir.join("grown.ipynb");
        let mut raw = String::from("{\"cells\":[],\"nbformat\":4,\"pad\":\"");
        raw.push_str(&"y".repeat(MAX_NOTEBOOK_BYTES as usize));
        raw.push_str("\"}");
        fs::write(&path, &raw).unwrap();

        let before = std::time::Instant::now();
        let err = read_source_text(&path).expect_err("over the limit");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        // And it stopped early rather than reading the whole thing twice.
        assert!(before.elapsed() < std::time::Duration::from_secs(30));
        let _ = fs::remove_dir_all(&dir);
    }
}

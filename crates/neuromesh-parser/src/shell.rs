//! Shell and PowerShell scripts as code: a repository's `scripts/*.sh` are
//! where "how does the benchmark script pick the test" is answered, and until
//! this existed they were not indexed at all (F78). Functions only —
//! `name() {`, `function name`, PowerShell `function Name {` — plus the
//! literal index the graph builds from the source (routes, env vars).

use crate::types::{AstAnalysisResult, ParsedSymbol};
use neuromesh_core::NodeType;
use std::path::Path;

pub struct ShellParser;

impl ShellParser {
    pub fn parse(_file_path: &Path, content: &str) -> AstAnalysisResult {
        let mut result = AstAnalysisResult::default();
        for (idx, raw) in content.lines().enumerate() {
            let line = raw.trim();
            if line.starts_with('#') {
                continue;
            }
            let name = if let Some(rest) = line.strip_prefix("function ") {
                rest.split(|c: char| c == '(' || c == '{' || c.is_whitespace())
                    .next()
                    .unwrap_or("")
            } else if let Some(paren) = line.find("()") {
                let head = line[..paren].trim();
                if head.is_empty() || !line[paren + 2..].trim_start().starts_with('{') {
                    continue;
                }
                head
            } else {
                continue;
            };
            let ok = !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == ':');
            if !ok || result.symbols.iter().any(|s| s.name == name) {
                continue;
            }
            let end = content
                .lines()
                .enumerate()
                .skip(idx + 1)
                .find(|(_, l)| l.trim_end() == "}")
                .map(|(i, _)| i + 2)
                .unwrap_or(idx + 2);
            result.symbols.push(ParsedSymbol::new(
                name,
                NodeType::Function,
                Some(line.chars().take(80).collect()),
                (idx + 1)..end,
                true,
            ));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_and_powershell_functions() {
        let src = "#!/usr/bin/env bash\nclean() { [ -s \"$1\" ]; }\nrun() {\n  echo run\n}\nfunction Get-Thing {\n  param($x)\n}\n";
        let ast = ShellParser::parse(Path::new("scripts/x.sh"), src);
        let names: Vec<&str> = ast.symbols.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["clean", "run", "Get-Thing"]);
        assert_eq!(ast.symbols[1].line_range, 3..6);
    }
}

//! Unicode normalization for natural-language prompts and client-supplied keywords.
//!
//! Works for any non-English script (Persian, Arabic, CJK, Cyrillic, etc.):
//! NFKC, common control/format chars, and optional Persian-specific fixes.

const ZWNJ: char = '\u{200c}';
const ZWJ: char = '\u{200d}';
const LRM: char = '\u{200e}';
const RLM: char = '\u{200f}';
const BOM: char = '\u{feff}';

/// Words from which a prompt reads as a report (an issue, a bug template)
/// rather than a question; below it nothing is stripped.
pub const REPORT_WORDS: usize = 60;

/// The part of a bug report that is about this bug.
///
/// Issue trackers wrap every report in the same template: HTML comments with
/// instructions, checklists ("- [X] I searched the issues and found no
/// similar issues", "I agree to follow the Code of Conduct"), section
/// headings ("### Expected Behaviour"), and links. Read as words, that
/// scaffolding is the same in every report of a project and ranked
/// `CODE_OF_CONDUCT.md` and the CLI module first on SWE-bench dev
/// (sqlfluff). Short prompts are returned unchanged.
pub fn strip_issue_boilerplate(prompt: &str) -> String {
    use std::sync::OnceLock;
    static COMMENT: OnceLock<regex::Regex> = OnceLock::new();
    static URL: OnceLock<regex::Regex> = OnceLock::new();
    if prompt.split_whitespace().count() < REPORT_WORDS {
        return prompt.to_string();
    }
    let comment = COMMENT.get_or_init(|| regex::Regex::new(r"(?s)<!--.*?-->").unwrap());
    let url = URL.get_or_init(|| regex::Regex::new(r"https?://\S+").unwrap());
    let text = comment.replace_all(prompt, " ");
    let text = url.replace_all(&text, " ");
    // Inside a fenced block a `# ...` line is a code comment, not a heading.
    let mut fenced = false;
    let mut kept = Vec::new();
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("```") {
            fenced = !fenced;
            kept.push(line);
            continue;
        }
        let checklist = ["- [", "* [", "+ ["]
            .iter()
            .any(|p| t.starts_with(p) && t.get(4..5) == Some("]"));
        let heading = !fenced && t.starts_with('#') && t.trim_start_matches('#').starts_with(' ');
        if !checklist && !heading {
            kept.push(line);
        }
    }
    kept.join("\n")
}

/// Normalize a client keyword: trim, NFKC, strip control/format chars, collapse space.
pub fn normalize_keyword(raw: &str) -> String {
    let s = normalize_unicode(raw);
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Normalize full prompt text before token splitting (any language).
pub fn normalize_unicode(raw: &str) -> String {
    let mut s: String = unicode_normalization::UnicodeNormalization::nfkc(raw.chars()).collect();
    s = s.replace(ZWNJ, " ").replace([ZWJ, LRM, RLM, BOM], "");
    apply_optional_script_fixes(&mut s);
    s.trim().to_string()
}

/// Split normalized prompt into whitespace-delimited tokens for seed fallback.
pub fn normalize_prompt_tokens(raw: &str) -> Vec<String> {
    normalize_unicode(raw)
        .split_whitespace()
        .map(|t| {
            t.trim_matches(|c: char| {
                !c.is_alphanumeric() && c != '_' && c != '.' && c != '/' && c != ':'
            })
            .to_string()
        })
        .filter(|t| !t.is_empty())
        .collect()
}

fn apply_optional_script_fixes(s: &mut String) {
    // Persian/Arabic letter variants — helpful but not required for other scripts.
    *s = s
        .replace('\u{0643}', "\u{06a9}") // Arabic kaf → Persian ke
        .replace('\u{064a}', "\u{06cc}"); // Arabic yaa → Persian ye
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_template_scaffolding_is_dropped_code_comments_kept() {
        let filler = "word ".repeat(REPORT_WORDS);
        let report = format!(
            "L031 triggers on a query without a join\n\
             ### Search before asking\n\
             - [X] I searched the [issues](https://github.com/o/r/issues) and found no similar issues.\n\
             <!-- describe the bug -->\n\
             ```python\n# keep this comment\nx = 1\n```\n{filler}"
        );
        let out = strip_issue_boilerplate(&report);
        assert!(out.contains("L031 triggers"));
        assert!(out.contains("# keep this comment"));
        assert!(!out.contains("Search before asking"));
        assert!(!out.contains("similar issues"));
        assert!(!out.contains("describe the bug"));
        assert_eq!(
            strip_issue_boilerplate("### short question"),
            "### short question"
        );
    }

    #[test]
    fn nfkc_and_zwnj() {
        let raw = "مدل\u{200c}محصول";
        let norm = normalize_unicode(raw);
        assert!(norm.contains(' ') || !norm.contains(ZWNJ));
    }

    #[test]
    fn keyword_trims_and_collapses() {
        assert_eq!(normalize_keyword("  UserController  "), "UserController");
    }

    #[test]
    fn cjk_prompt_tokens_do_not_panic() {
        let tokens = normalize_prompt_tokens("设计用户认证模块");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn arabic_yeh_ke_optional_fix() {
        let s = normalize_unicode("كتاب");
        assert!(s.contains('\u{06a9}') || s.contains('ك'));
    }
}

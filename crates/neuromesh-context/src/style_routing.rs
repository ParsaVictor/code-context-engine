use crate::seed::sink::SeedSink;
use neuromesh_core::TaskSignature;
use neuromesh_graph::NeuralProjectGraph;
use std::collections::HashSet;

const STYLE_KEYWORDS: &[&str] = &[
    "scss",
    "sass",
    "stylesheet",
    "style sheet",
    "design token",
    "tokens and mixins",
    "tokens/mixins",
    "mixins",
    "mixin",
    "_tokens.",
    "_mixins.",
];

pub fn is_style_task(signature: &TaskSignature) -> bool {
    if signature.style.is_some() {
        return true;
    }
    let lower = signature.raw_prompt.to_lowercase();
    STYLE_KEYWORDS.iter().any(|k| lower.contains(k))
}

pub fn style_path_matches_task(path: &str, style: Option<&str>) -> bool {
    let p = path.replace('\\', "/").to_ascii_lowercase();
    match style {
        Some("scss") | Some("sass") => p.ends_with(".scss") || p.ends_with(".sass"),
        Some("less") => p.ends_with(".less"),
        Some("css") => p.ends_with(".css"),
        _ => true,
    }
}

pub fn is_style_path(path: &std::path::Path) -> bool {
    let p = path.to_string_lossy().replace('\\', "/").to_lowercase();
    p.contains("/styles/")
        || p.ends_with(".scss")
        || p.ends_with(".sass")
        || p.ends_with(".less")
        || p.contains("_tokens.")
        || p.contains("_mixins.")
        || p.contains("tokens.scss")
        || p.contains("mixins.scss")
}

pub(crate) fn inject_style_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    signature: &TaskSignature,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    if !is_style_task(signature) {
        return;
    }

    let style_ext = signature.style.as_deref().map(|s| s.to_ascii_lowercase());
    for hint in style_file_hints(&style_ext) {
        let Some(id) = graph.resolve_file_hint(hint) else {
            continue;
        };
        let path = graph
            .get_node(&id)
            .map(|n| n.file_path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if !style_path_matches_task(&path, style_ext.as_deref()) {
            continue;
        }
        sink.push(graph, prompt, hint.to_string(), 0.95, "style_hint");
    }

    // A partial whose stem the prompt names (`price-card` ~ `_priceCard.scss`)
    // is the target; every hyphenated word (`hover-lift`, `price-card`) is
    // looked up among style tokens and mixins. A component the prompt names
    // is already an identifier seed. Nothing here knows any project's names.
    for path in prompt_named_style_partials(graph, &signature.raw_prompt) {
        sink.push(graph, prompt, path, 0.88, "style_partial");
    }
    for token in style_token_queries(signature) {
        for hit in graph.search_symbols(&token, 4) {
            if hit.node_type == neuromesh_core::NodeType::StyleToken
                || is_style_path(&hit.file_path)
            {
                sink.push(graph, prompt, hit.name.clone(), 0.78, "style_token");
            }
        }
    }
}

/// Every hyphenated word of the prompt (`hover-lift`, `price-card`): the
/// shape of a CSS class, a token or a mixin name.
fn style_token_queries(signature: &TaskSignature) -> Vec<String> {
    let lower = signature.raw_prompt.to_lowercase();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for word in lower.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_') {
        let word = word.trim_matches('-');
        if word.contains('-')
            && word.len() >= 5
            && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && seen.insert(word)
        {
            out.push(word.to_string());
        }
    }
    out
}

fn squash(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Stylesheets whose stem the prompt names: `price-card` ~ `_priceCard.scss`
/// (leading underscore of a partial, hyphens and case ignored).
fn prompt_named_style_partials(graph: &NeuralProjectGraph, prompt: &str) -> Vec<String> {
    let lower = prompt.to_lowercase();
    let words: Vec<String> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| w.len() >= 5 && w.contains(['-', '_']))
        .map(squash)
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = graph
        .file_node_paths()
        .into_iter()
        .filter(|(_, path)| is_style_path(path))
        .filter(|(_, path)| {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .trim_start_matches('_');
            let stem = squash(stem);
            stem.len() >= 5 && words.contains(&stem)
        })
        .map(|(_, path)| path.to_string_lossy().replace('\\', "/"))
        .collect();
    out.sort();
    out
}

const SCSS_STYLE_HINTS: &[&str] = &[
    "src/styles/_tokens.scss",
    "src/styles/tokens.scss",
    "src/styles/_mixins.scss",
    "src/styles/mixins.scss",
    "styles/_tokens.scss",
    "styles/tokens.scss",
    "styles/_mixins.scss",
    "styles/mixins.scss",
];

fn style_file_hints(style: &Option<String>) -> Vec<&'static str> {
    match style.as_deref() {
        Some("scss") | Some("sass") => SCSS_STYLE_HINTS.to_vec(),
        Some("less") => vec!["styles/tokens.less", "src/styles/tokens.less"],
        Some("css") => vec!["styles/tokens.css", "src/styles/tokens.css"],
        _ => SCSS_STYLE_HINTS.to_vec(),
    }
}

pub(crate) fn inject_view_component_seeds(
    graph: &NeuralProjectGraph,
    prompt: &str,
    signature: &TaskSignature,
    sink: &mut SeedSink<'_, '_, '_>,
) {
    // A prompt word that names a view of the project ("checkout" when
    // `CheckoutView` exists) seeds that view. "cart store" names the store,
    // not `CartView`: a word followed by store/state/module is the state
    // module the question is about.
    let lower = signature.raw_prompt.to_lowercase();
    let mut candidates: HashSet<String> = HashSet::new();
    for ident in &signature.identifiers {
        if ident.ends_with("View") || ident.ends_with("Component") {
            candidates.insert(ident.clone());
        }
    }
    for word in prompt_view_words(&lower) {
        let view = format!("{}View", pascal_case(&word));
        if graph
            .nodes_named(&view)
            .iter()
            .any(|n| is_component_or_script_path(&n.file_path))
        {
            candidates.insert(view);
        }
    }
    let mut candidates: Vec<String> = candidates.into_iter().collect();
    candidates.sort();
    for name in candidates {
        if name.len() < 5 {
            continue;
        }
        sink.push(graph, prompt, name, 0.82, "view_component");
    }
}

const STATE_MODULE_WORDS: &[&str] = &["store", "state", "module", "slice", "reducer"];

/// Prompt words that could name a view, minus those the prompt uses as a
/// state module ("cart store").
fn prompt_view_words(lower: &str) -> Vec<String> {
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    // Deduplicate with a set: a hostile multi-megabyte prompt has a hundred
    // thousand distinct words, and a linear "seen" scan made this quadratic.
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        if w.len() < 4 {
            continue;
        }
        let names_state = words
            .get(i + 1)
            .is_some_and(|next| STATE_MODULE_WORDS.contains(next));
        if names_state || !seen.insert(w) {
            continue;
        }
        out.push((*w).to_string());
    }
    out
}

/// The state modules the prompt names as "<word> store" (or state/module/…):
/// the stems to keep when the selection is tightened around a named view.
fn prompt_state_module_words(lower: &str) -> Vec<String> {
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    words
        .windows(2)
        .filter(|pair| pair[0].len() >= 3 && STATE_MODULE_WORDS.contains(&pair[1]))
        .map(|pair| pair[0].to_string())
        .collect()
}

/// In a style task, a component/script file the prompt never names is
/// noise: "following ProductCard styling patterns" wants `ProductCard.vue`
/// and the stylesheets, not every other component that happens to share a
/// mixin or a token. Stylesheets are never penalised; a code file is kept
/// only when the prompt names its stem (`ProductCard`, or both `product`
/// and `card` as separate words).
pub fn style_noise_penalty(path: &std::path::Path, signature: &TaskSignature) -> f32 {
    if !is_style_task(signature) {
        return 0.0;
    }
    if is_style_path(path) {
        return 0.0;
    }
    if !is_component_or_script_path(path) {
        return 0.0;
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if stem_named_in_prompt(stem, &signature.raw_prompt) {
        return 0.0;
    }
    28.0
}

fn is_component_or_script_path(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "vue" | "svelte" | "jsx" | "tsx" | "js" | "ts" | "mjs" | "cjs" | "astro"
        )
    })
}

fn stem_named_in_prompt(stem: &str, prompt: &str) -> bool {
    let words: std::collections::HashSet<String> = prompt
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase().replace(['_', '-'], ""))
        .collect();
    let joined = stem.to_lowercase().replace(['_', '-'], "");
    if words.contains(&joined) {
        return true;
    }
    let parts: Vec<String> = neuromesh_parser::tokenize_ident(stem)
        .into_iter()
        .map(|t| t.to_lowercase())
        .filter(|t| t.len() >= 3)
        .collect();
    !parts.is_empty() && parts.iter().all(|t| words.contains(t))
}

/// Keep only files whose extension matches an explicit stylesheet kind (CSS/Less/SCSS).
pub(crate) fn tighten_style_extension_selection(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    selection: &mut crate::selector::Selection,
) {
    if !is_style_task(signature) {
        return;
    }
    let style_ext = signature
        .style
        .as_deref()
        .map(|s| s.to_ascii_lowercase())
        .filter(|s| matches!(s.as_str(), "css" | "less" | "scss" | "sass"));
    let Some(style_ext) = style_ext else {
        return;
    };
    let keep = |path: &str| {
        if !is_style_path(std::path::Path::new(path)) {
            return true;
        }
        style_path_matches_task(path, Some(style_ext.as_str()))
    };
    selection.optional.retain(|id| {
        graph
            .get_node(id)
            .map(|n| keep(&n.file_path.to_string_lossy()))
            .unwrap_or(false)
    });
    selection.required.retain(|id| {
        graph
            .get_node(id)
            .map(|n| keep(&n.file_path.to_string_lossy()))
            .unwrap_or(true)
    });
}

/// A prompt that names both a view of the project ("checkout" with a
/// `CheckoutView`) and a state module ("cart store") is anchored: the
/// answer is that view and that store, and optional connector fill around
/// them is noise. Nothing is dropped unless both are named.
pub(crate) fn tighten_focused_view_selection(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    selection: &mut crate::selector::Selection,
) {
    let lower = signature.raw_prompt.to_lowercase();
    let views: Vec<String> = prompt_view_words(&lower)
        .into_iter()
        .map(|w| format!("{}View", pascal_case(&w)))
        .filter(|view| {
            graph
                .nodes_named(view)
                .iter()
                .any(|n| is_component_or_script_path(&n.file_path))
        })
        .map(|v| v.to_lowercase())
        .collect();
    let stores = prompt_state_module_words(&lower);
    if views.is_empty() || stores.is_empty() {
        return;
    }
    let keep = move |path: &str| {
        let p = path.replace('\\', "/").to_lowercase();
        let stem = std::path::Path::new(&p)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        views.contains(&stem)
            || stores
                .iter()
                .any(|s| stem == *s && (p.contains("/store") || p.contains("/state")))
    };
    selection.optional.retain(|id| {
        graph
            .get_node(id)
            .map(|n| keep(&n.file_path.to_string_lossy()))
            .unwrap_or(false)
    });
    selection.required.retain(|id| {
        graph
            .get_node(id)
            .map(|n| keep(&n.file_path.to_string_lossy()))
            .unwrap_or(true)
    });
}

fn pascal_case(raw: &str) -> String {
    let clean: String = raw.chars().filter(|c| c.is_alphanumeric()).collect();
    if clean.is_empty() {
        return String::new();
    }
    let mut chars = clean.chars();
    let first = chars.next().unwrap().to_ascii_uppercase();
    let rest: String = chars.flat_map(|c| c.to_lowercase()).collect();
    format!("{first}{rest}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_style_task_from_tokens_keyword() {
        let sig = TaskSignature {
            id: "t".into(),
            intent: neuromesh_core::TaskIntent::Modify,
            domain: "frontend".into(),
            technology: "Vue".into(),
            style: None,
            entity: "ProductCard".into(),
            goal: "style".into(),
            risk: neuromesh_core::TaskRisk::Low,
            related_concepts: vec![],
            identifiers: vec!["ProductCard".into()],
            file_hints: vec![],
            client_keywords: vec![],
            client_keywords_inferred: false,
            client_expansion: vec![],
            client_path_hints: vec![],
            client_entity_types: vec![],
            client_intent: None,
            retrieval_engine_override: None,
            engine_override: None,
            embed_min_cosine_override: None,
            confidence: 0.9,
            raw_prompt: "Apply hover-lift using SCSS tokens and mixins on ProductCard".into(),
        };
        assert!(is_style_task(&sig));
    }

    #[test]
    fn pascal_case_handles_checkout() {
        assert_eq!(pascal_case("checkout"), "Checkout");
    }

    #[test]
    fn hyphenated_prompt_words_are_style_token_queries() {
        let sig = style_sig("Apply hover-lift and the price-card mixin; keep focus-within");
        let mut q = style_token_queries(&sig);
        q.sort();
        assert_eq!(q, vec!["focus-within", "hover-lift", "price-card"]);
        assert!(style_token_queries(&style_sig("plain words only")).is_empty());
    }

    #[test]
    fn a_word_before_store_names_the_store_not_a_view() {
        let lower = "add quantity stepper in checkout list using setqty from cart store";
        let views = prompt_view_words(lower);
        assert!(views.iter().any(|w| w == "checkout"), "{views:?}");
        assert!(!views.iter().any(|w| w == "cart"), "{views:?}");
        assert_eq!(prompt_state_module_words(lower), vec!["cart"]);
        assert!(prompt_view_words("restyle the cart view")
            .iter()
            .any(|w| w == "cart"));
    }

    #[test]
    fn squash_ignores_case_hyphens_and_underscores() {
        assert_eq!(squash("price-card"), "pricecard");
        assert_eq!(squash("_priceCard"), "pricecard");
        assert_eq!(squash("Price_Card"), "pricecard");
    }

    #[test]
    fn css_style_path_filter_rejects_less() {
        assert!(style_path_matches_task("styles/sms.css", Some("css")));
        assert!(!style_path_matches_task("styles/sms.less", Some("css")));
        assert!(style_path_matches_task("styles/sms.less", Some("less")));
    }

    #[test]
    fn default_style_hints_include_non_underscore_paths() {
        let hints = style_file_hints(&None);
        assert!(hints.contains(&"src/styles/tokens.scss"));
        assert!(hints.contains(&"src/styles/mixins.scss"));
    }

    fn style_sig(prompt: &str) -> TaskSignature {
        TaskSignature {
            id: "t".into(),
            intent: neuromesh_core::TaskIntent::Modify,
            domain: "frontend".into(),
            technology: "Vue".into(),
            style: Some("scss".into()),
            entity: String::new(),
            goal: "style".into(),
            risk: neuromesh_core::TaskRisk::Low,
            related_concepts: vec![],
            identifiers: vec![],
            file_hints: vec![],
            client_keywords: vec![],
            client_keywords_inferred: false,
            client_expansion: vec![],
            client_path_hints: vec![],
            client_entity_types: vec![],
            client_intent: None,
            retrieval_engine_override: None,
            engine_override: None,
            embed_min_cosine_override: None,
            confidence: 0.9,
            raw_prompt: prompt.into(),
        }
    }

    #[test]
    fn style_noise_penalises_only_unnamed_code_files() {
        use std::path::Path;
        let sig = style_sig(
            "Create a price-card SCSS partial reusing tokens and mixins, following ProductCard styling",
        );
        // Named in the prompt: kept.
        assert_eq!(
            style_noise_penalty(Path::new("src/components/ProductCard.vue"), &sig),
            0.0
        );
        // Stylesheets are never noise.
        assert_eq!(
            style_noise_penalty(Path::new("src/styles/tokens.scss"), &sig),
            0.0
        );
        // A component the prompt never mentions is noise, whatever it is called.
        assert!(style_noise_penalty(Path::new("src/components/CartDrawer.vue"), &sig) >= 20.0);
        assert!(style_noise_penalty(Path::new("src/components/Sidebar.vue"), &sig) >= 20.0);
        assert!(style_noise_penalty(Path::new("src/stores/cart.js"), &sig) >= 20.0);
        // Two-word stem named as two words.
        let sig2 = style_sig("restyle the cart drawer with SCSS tokens");
        assert_eq!(
            style_noise_penalty(Path::new("src/components/CartDrawer.vue"), &sig2),
            0.0
        );
    }

    #[test]
    fn style_noise_is_zero_outside_style_tasks() {
        let mut sig = style_sig("How does the cart store compute totals?");
        sig.style = None;
        assert!(!is_style_task(&sig));
        assert_eq!(
            style_noise_penalty(std::path::Path::new("src/components/CartDrawer.vue"), &sig),
            0.0
        );
    }
}

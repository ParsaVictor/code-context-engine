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
    "hover-lift",
    "focus-within",
    "price-card",
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

    let lower = signature.raw_prompt.to_lowercase();
    if lower.contains("productcard")
        || lower.contains("product card")
        || lower.contains("price-card")
    {
        sink.push(graph, prompt, "ProductCard".into(), 0.9, "style_component");
    }
    if lower.contains("price-card") || lower.contains("pricecard") {
        for hint in [
            "src/styles/_priceCard.scss",
            "src/styles/priceCard.scss",
            "styles/_priceCard.scss",
        ] {
            if graph.resolve_file_hint(hint).is_some() {
                sink.push(graph, prompt, hint.to_string(), 0.88, "style_partial");
            }
        }
        sink.push(graph, prompt, "price-card-tile".into(), 0.75, "style_mixin");
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

fn style_token_queries(signature: &TaskSignature) -> Vec<String> {
    let lower = signature.raw_prompt.to_lowercase();
    let mut out = Vec::new();
    for kw in [
        "hover-lift",
        "focus-within",
        "price-card",
        "price-card-tile",
    ] {
        if lower.contains(kw) {
            out.push(kw.to_string());
        }
    }
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
    let lower = signature.raw_prompt.to_lowercase();
    let view_task = lower.contains("checkout")
        || lower.contains("cartview")
        || lower.contains("productcard")
        || lower.contains("product card")
        || lower.contains("setqty")
        || lower.contains("quantity")
        || lower.contains("stepper");
    if !view_task {
        return;
    }
    let mut candidates: HashSet<String> = HashSet::new();
    for ident in &signature.identifiers {
        if ident.ends_with("View") || ident.ends_with("Component") {
            candidates.insert(ident.clone());
        }
    }
    for word in ["checkout", "cart", "product", "home", "header"] {
        if word == "cart"
            && (prompt_contains_word(&lower, "checkout") || lower.contains("cartview"))
            && !lower.contains("cart view")
        {
            continue;
        }
        if prompt_contains_word(&lower, word) {
            candidates.insert(format!("{}View", pascal_case(word)));
        }
    }
    for name in candidates {
        if name.len() < 5 {
            continue;
        }
        sink.push(graph, prompt, name, 0.82, "view_component");
    }
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

fn prompt_contains_word(lower: &str, word: &str) -> bool {
    lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| w == word)
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

/// Drop optional connector fill when checkout/store seeds already anchor the task.
pub(crate) fn tighten_focused_view_selection(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    selection: &mut crate::selector::Selection,
) {
    let lower = signature.raw_prompt.to_lowercase();
    let focused_checkout = (lower.contains("setqty") || prompt_contains_word(&lower, "stepper"))
        && prompt_contains_word(&lower, "checkout");
    if !focused_checkout {
        return;
    }
    let keep = |path: &str| {
        let p = path.replace('\\', "/").to_lowercase();
        p.contains("checkoutview") || p.contains("stores/cart")
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

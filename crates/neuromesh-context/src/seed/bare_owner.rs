//! The two halves of an `owner.member` identifier are not symbols of their
//! own when the question wrote the pair.
//!
//! "How does req.get read a request header?" yields the identifiers `req`,
//! `req.get` and `get`. The bare `req` has no exact symbol, so the resolver
//! falls back to prefix search and lands on `View.resolve`; the bare `get`
//! resolves to the only `get` in the graph — `res.get`, a different
//! receiver's member. The dotted seed already carries everything both halves
//! meant, and its own resolution respects the owner (`resolve_dotted_member`
//! is a superset of resolving the bare member), so:
//!
//! - a bare owner of at most three characters (`res`, `req`, `app`) that is
//!   the receiver of a dotted seed in the same question is dropped;
//! - a bare member that is the member of a dotted seed in the same question
//!   is dropped.
//!
//! Both are dropped resolved or not: an unresolved bare half would otherwise
//! count as a missed seed for a question that missed nothing.

use neuromesh_core::{NodeId, SeedResolution};
use std::collections::HashMap;

const MAX_BARE_OWNER_LEN: usize = 3;

/// A resolved seed stores its query as `reason:query`; an unresolved one
/// stores the raw query. Only identifier seeds take part here.
fn identifier_body(query: &str) -> Option<&str> {
    match query.split_once(':') {
        Some(("identifier", body)) => Some(body),
        Some(_) => None,
        None => Some(query),
    }
}

fn dotted(body: &str) -> Option<(&str, &str)> {
    if body.contains(['/', '\\']) {
        return None;
    }
    let (owner, member) = body.split_once('.')?;
    (!owner.is_empty() && !member.is_empty() && !member.contains('.')).then_some((owner, member))
}

pub(crate) fn prune_bare_owner_seeds(
    seeds: &mut Vec<SeedResolution>,
    seed_energies: &mut HashMap<NodeId, f32>,
    seed_reasons: &mut HashMap<NodeId, String>,
) {
    let mut owners: Vec<String> = Vec::new();
    let mut members: Vec<String> = Vec::new();
    for seed in seeds.iter() {
        let Some((owner, member)) = identifier_body(&seed.query).and_then(dotted) else {
            continue;
        };
        if owner.len() <= MAX_BARE_OWNER_LEN {
            owners.push(owner.to_ascii_lowercase());
        }
        members.push(member.to_ascii_lowercase());
    }
    // F61: a bare acronym the question wrote next to the identifier it is
    // part of. "How does CsrfViewMiddleware check the CSRF token" yields
    // `CsrfViewMiddleware` and `CSRF`; the acronym resolved to an unrelated
    // module-level `csrf()` in another package. A bare identifier that is a
    // token of a longer *resolved* identifier seed is a fragment of that
    // anchor, not a second one.
    let mut fragments: std::collections::HashSet<String> = std::collections::HashSet::new();
    for seed in seeds.iter() {
        let Some(body) = identifier_body(&seed.query) else {
            continue;
        };
        if seed.resolved_id.is_none() || body.contains(['.', '/', '\\']) {
            continue;
        }
        let tokens = neuromesh_parser::tokenize_ident(body);
        if tokens.len() < 2 {
            continue;
        }
        fragments.extend(tokens.into_iter().map(|t| t.to_ascii_lowercase()));
    }
    if owners.is_empty() && members.is_empty() && fragments.is_empty() {
        return;
    }
    let mut dropped: Vec<NodeId> = Vec::new();
    seeds.retain(|seed| {
        let Some(body) = identifier_body(&seed.query) else {
            return true;
        };
        if body.contains(['.', '/', '\\']) {
            return true;
        }
        let bare = body.to_ascii_lowercase();
        let acronym_fragment = body.len() >= 3
            && body
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && fragments.contains(&bare);
        if !owners.contains(&bare) && !members.contains(&bare) && !acronym_fragment {
            return true;
        }
        if let Some(id) = seed.resolved_id.as_ref() {
            dropped.push(id.clone());
        }
        false
    });
    for id in dropped {
        if seeds.iter().any(|s| s.resolved_id.as_ref() == Some(&id)) {
            continue;
        }
        seed_energies.remove(&id);
        seed_reasons.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(query: &str, id: Option<&str>) -> SeedResolution {
        SeedResolution {
            query: query.into(),
            resolved_id: id.map(NodeId::new),
            confidence: 1.0,
            resolution_tier: None,
            embedding_score: None,
        }
    }

    fn run(seeds: &mut Vec<SeedResolution>) -> Vec<String> {
        let mut e: HashMap<NodeId, f32> = seeds
            .iter()
            .filter_map(|s| s.resolved_id.clone().map(|id| (id, 1.0)))
            .collect();
        let mut r = HashMap::new();
        prune_bare_owner_seeds(seeds, &mut e, &mut r);
        let mut left: Vec<String> = e.keys().map(|id| id.as_str().to_string()).collect();
        left.sort();
        left
    }

    #[test]
    fn bare_owner_of_a_dotted_seed_is_dropped() {
        let mut seeds = vec![
            seed("identifier:res", Some("sym:lib/view.js:View.resolve")),
            seed("identifier:res.json", Some("sym:lib/response.js:res.json")),
        ];
        let left = run(&mut seeds);
        assert_eq!(seeds.len(), 1);
        assert_eq!(seeds[0].query, "identifier:res.json");
        assert_eq!(left, vec!["sym:lib/response.js:res.json"]);
    }

    #[test]
    fn bare_member_of_a_dotted_seed_is_dropped_even_when_dotted_is_unresolved() {
        let mut seeds = vec![
            seed("req.get", None),
            seed("identifier:get", Some("sym:lib/response.js:res.get")),
            seed("req", None),
            seed("identifier:req.is", Some("sym:lib/request.js:req.is")),
        ];
        let left = run(&mut seeds);
        let queries: Vec<&str> = seeds.iter().map(|s| s.query.as_str()).collect();
        assert_eq!(queries, vec!["req.get", "identifier:req.is"]);
        assert_eq!(left, vec!["sym:lib/request.js:req.is"]);
    }

    #[test]
    fn long_owner_and_unrelated_short_identifier_are_kept() {
        let mut seeds = vec![
            seed("identifier:model.forward", Some("sym:model.py:GPT.forward")),
            seed("identifier:model", Some("file:model.py")),
            seed("identifier:gpt", Some("sym:model.py:GPT")),
        ];
        run(&mut seeds);
        assert_eq!(seeds.len(), 3);
    }

    #[test]
    fn acronym_that_is_a_token_of_a_resolved_identifier_is_dropped() {
        let mut seeds = vec![
            seed(
                "identifier:CsrfViewMiddleware",
                Some("sym:django/middleware/csrf.py:CsrfViewMiddleware"),
            ),
            seed(
                "identifier:CSRF",
                Some("sym:django/template/context_processors.py:csrf"),
            ),
            // lower-case word, not an acronym: left to the weak-seed rules
            seed("identifier:view", Some("sym:django/views/base.py:View")),
        ];
        let left = run(&mut seeds);
        let queries: Vec<&str> = seeds.iter().map(|s| s.query.as_str()).collect();
        assert_eq!(
            queries,
            vec!["identifier:CsrfViewMiddleware", "identifier:view"]
        );
        assert_eq!(left.len(), 2);
    }

    #[test]
    fn non_identifier_seeds_and_paths_are_untouched() {
        let mut seeds = vec![
            seed("identifier:app.use", Some("sym:lib/application.js:app.use")),
            seed("cluster:use", Some("sym:lib/router.js:use")),
            seed("concept:app", Some("file:lib/app.js")),
            seed("file:lib/app.js", Some("file:lib/app.js")),
        ];
        run(&mut seeds);
        assert_eq!(seeds.len(), 4);
    }

    #[test]
    fn energy_survives_when_another_seed_shares_the_node() {
        let mut seeds = vec![
            seed("identifier:app", Some("file:lib/application.js")),
            seed("identifier:app.use", Some("file:lib/application.js")),
        ];
        let left = run(&mut seeds);
        assert_eq!(seeds.len(), 1);
        assert_eq!(left, vec!["file:lib/application.js"]);
    }
}

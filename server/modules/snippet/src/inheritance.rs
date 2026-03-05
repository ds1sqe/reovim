//! Filetype inheritance for snippet resolution (#529).
//!
//! Defines parent relationships between filetypes so that e.g.
//! TypeScript inherits JavaScript snippets. The resolution chain
//! always ends with `"global"`.

/// Maximum chain depth to prevent infinite loops.
const MAX_DEPTH: usize = 8;

/// Get parent filetypes for a given filetype.
///
/// Returns an empty slice for filetypes with no parents.
#[must_use]
pub fn parents(filetype: &str) -> &'static [&'static str] {
    match filetype {
        "typescriptreact" | "tsx" => &["typescript", "javascript"],
        "javascriptreact" | "jsx" | "typescript" => &["javascript"],
        "scss" | "sass" | "less" => &["css"],
        _ => &[],
    }
}

/// Build the full resolution chain for snippet lookup.
///
/// The chain includes:
/// 1. The filetype itself
/// 2. All inherited parents (recursively, breadth-first)
/// 3. `"global"` as the final fallback
///
/// Duplicates are removed. Depth is bounded to prevent cycles.
#[must_use]
pub fn resolution_chain(filetype: &str) -> Vec<&str> {
    resolution_chain_bounded(filetype, MAX_DEPTH, parents)
}

/// Build a resolution chain with a custom depth limit and parent lookup.
///
/// Extracted for testability of the depth guard and duplicate-parent paths.
fn resolution_chain_bounded<F>(filetype: &str, max_depth: usize, parent_fn: F) -> Vec<&str>
where
    F: Fn(&str) -> &'static [&'static str],
{
    use std::collections::VecDeque;

    let mut chain = Vec::with_capacity(4);
    let mut queue = VecDeque::from([filetype]);
    let mut depth = 0;

    while let Some(ft) = queue.pop_front() {
        if chain.contains(&ft) {
            continue;
        }
        chain.push(ft);

        depth += 1;
        if depth >= max_depth {
            break;
        }

        for &parent in parent_fn(ft) {
            if !chain.contains(&parent) {
                queue.push_back(parent);
            }
        }
    }

    // Always end with "global" (if not already present)
    if !chain.contains(&"global") {
        chain.push("global");
    }

    chain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_has_no_parents() {
        assert!(parents("rust").is_empty());
    }

    #[test]
    fn test_typescript_inherits_javascript() {
        assert_eq!(parents("typescript"), &["javascript"]);
    }

    #[test]
    fn test_typescriptreact_inherits_ts_js() {
        assert_eq!(parents("typescriptreact"), &["typescript", "javascript"]);
    }

    #[test]
    fn test_scss_inherits_css() {
        assert_eq!(parents("scss"), &["css"]);
    }

    #[test]
    fn test_jsx_inherits_javascript() {
        assert_eq!(parents("jsx"), &["javascript"]);
        assert_eq!(parents("javascriptreact"), &["javascript"]);
    }

    #[test]
    fn test_tsx_inherits_ts_js() {
        assert_eq!(parents("tsx"), &["typescript", "javascript"]);
    }

    #[test]
    fn test_rust_chain_is_rust_global() {
        let chain = resolution_chain("rust");
        assert_eq!(chain, vec!["rust", "global"]);
    }

    #[test]
    fn test_typescript_chain() {
        let chain = resolution_chain("typescript");
        assert_eq!(chain, vec!["typescript", "javascript", "global"]);
    }

    #[test]
    fn test_typescriptreact_chain() {
        let chain = resolution_chain("typescriptreact");
        assert_eq!(chain, vec!["typescriptreact", "typescript", "javascript", "global"]);
    }

    #[test]
    fn test_chain_always_ends_with_global() {
        for ft in &[
            "rust",
            "python",
            "typescript",
            "typescriptreact",
            "scss",
            "unknown",
            "global",
        ] {
            let chain = resolution_chain(ft);
            assert_eq!(
                chain.last().copied(),
                Some("global"),
                "chain for {ft} must end with global"
            );
        }
    }

    #[test]
    fn test_global_chain_is_just_global() {
        let chain = resolution_chain("global");
        assert_eq!(chain, vec!["global"]);
    }

    #[test]
    fn test_chain_no_duplicates() {
        // typescriptreact → [typescript, javascript], typescript → [javascript]
        // javascript should appear only once
        let chain = resolution_chain("typescriptreact");
        let mut seen = std::collections::HashSet::new();
        for ft in &chain {
            assert!(seen.insert(ft), "duplicate in chain: {ft}");
        }
    }

    #[test]
    fn test_chain_depth_limit() {
        // With our static table there are no cycles, but verify the limit works
        // by checking a deep chain doesn't exceed MAX_DEPTH + 1 (global)
        let chain = resolution_chain("typescriptreact");
        assert!(chain.len() <= MAX_DEPTH + 1);
    }

    #[test]
    fn test_sass_inherits_css() {
        assert_eq!(parents("sass"), &["css"]);
    }

    #[test]
    fn test_less_inherits_css() {
        assert_eq!(parents("less"), &["css"]);
    }

    #[test]
    fn test_sass_chain() {
        let chain = resolution_chain("sass");
        assert_eq!(chain, vec!["sass", "css", "global"]);
    }

    #[test]
    fn test_less_chain() {
        let chain = resolution_chain("less");
        assert_eq!(chain, vec!["less", "css", "global"]);
    }

    #[test]
    fn test_depth_limit_triggers_break() {
        // With max_depth=2, typescriptreact chain stops early
        let chain = resolution_chain_bounded("typescriptreact", 2, parents);
        // Only 2 items processed + global: [typescriptreact, typescript, global]
        // (javascript never reached because depth limit hit after processing typescript)
        assert_eq!(chain, vec!["typescriptreact", "typescript", "global"]);
    }

    #[test]
    fn test_depth_limit_one_stops_at_first() {
        let chain = resolution_chain_bounded("typescriptreact", 1, parents);
        // Only 1 item processed + global
        assert_eq!(chain, vec!["typescriptreact", "global"]);
    }

    #[test]
    fn test_duplicate_parent_already_in_chain() {
        let chain = resolution_chain_bounded("tsx", 4, parents);
        assert_eq!(chain, vec!["tsx", "typescript", "javascript", "global"]);
    }

    #[test]
    fn test_parent_already_in_chain_skips_queue() {
        // Custom parent table with a cycle: a → [b, c], c → [b]
        // When processing c, parent b is already in chain → line 58 false
        fn cyclic_parents(ft: &str) -> &'static [&'static str] {
            match ft {
                "a" => &["b", "c"],
                "c" => &["b"],
                _ => &[],
            }
        }

        let chain = resolution_chain_bounded("a", 8, cyclic_parents);
        // a → [b, c]. Process a, queue b and c.
        // Process b: no parents. Process c: parent b already in chain → skip.
        assert_eq!(chain, vec!["a", "b", "c", "global"]);
    }
}

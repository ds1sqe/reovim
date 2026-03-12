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
#[path = "inheritance_tests.rs"]
mod tests;

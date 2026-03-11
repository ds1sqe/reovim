//! Kahn's algorithm for topological sort over dependency graphs.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    hash::Hash,
};

/// Input entry for dependency resolution.
///
/// Each entry declares its key, required dependencies, and optional dependencies.
/// Required dependencies must exist in the entry set; missing ones produce an error.
/// Optional dependencies are ordered (if present) but silently skipped if absent.
#[derive(Debug, Clone)]
pub struct DepEntry<K> {
    /// Unique identifier for this entry.
    pub key: K,
    /// Required dependencies — must exist in the entry set.
    pub required: Vec<K>,
    /// Optional dependencies — ordered if present, skipped if absent.
    pub optional: Vec<K>,
}

/// Result of successful dependency resolution.
#[derive(Debug, Clone)]
pub struct DependencyOrder<K> {
    /// Keys in topologically sorted order (dependencies first).
    pub order: Vec<K>,
    /// Reverse dependency map: key -> set of keys that depend on it.
    /// Useful for reverse-order shutdown (exit dependents before their deps).
    pub dependents: HashMap<K, HashSet<K>>,
}

/// Error during dependency resolution.
#[derive(Debug, Clone)]
pub enum DepgraphError<K> {
    /// Entry depends on itself.
    SelfReferential(K),
    /// Circular dependency detected. Contains the keys involved in cycles.
    Cycle(Vec<K>),
    /// Required dependencies are missing. Each pair is `(dependent, missing_dep)`.
    Missing(Vec<(K, K)>),
}

impl<K: fmt::Debug> fmt::Display for DepgraphError<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SelfReferential(key) => write!(f, "entry {key:?} depends on itself"),
            Self::Cycle(keys) => write!(f, "circular dependency detected: {keys:?}"),
            Self::Missing(pairs) => {
                write!(f, "missing dependencies: ")?;
                for (i, (dep, missing)) in pairs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{dep:?} requires {missing:?}")?;
                }
                Ok(())
            }
        }
    }
}

impl<K: fmt::Debug> std::error::Error for DepgraphError<K> {}

/// Resolve dependencies using Kahn's algorithm (topological sort).
///
/// # Arguments
///
/// * `entries` — slice of dependency entries to resolve
///
/// # Returns
///
/// * `Ok(DependencyOrder)` — sorted order and reverse dependency map
/// * `Err(DepgraphError)` — if self-referential, circular, or missing dependencies detected
///
/// # Errors
///
/// Returns [`DepgraphError::SelfReferential`] if an entry lists itself as a dependency.
/// Returns [`DepgraphError::Missing`] if a required dependency is not in the entry set.
/// Returns [`DepgraphError::Cycle`] if a circular dependency is detected.
///
/// # Algorithm
///
/// 1. Check for self-referential dependencies (clearer error than cycle detection)
/// 2. Build adjacency list and in-degree map from required + optional deps
/// 3. Collect missing required deps (optional missing deps are silently skipped)
/// 4. Run Kahn's BFS: start from in-degree 0, process iteratively
/// 5. Detect cycles (any remaining nodes with in-degree > 0)
/// 6. Build reverse dependency map for shutdown ordering
pub fn resolve_dependencies<K>(
    entries: &[DepEntry<K>],
) -> Result<DependencyOrder<K>, DepgraphError<K>>
where
    K: Eq + Hash + Clone + fmt::Debug,
{
    if entries.is_empty() {
        return Ok(DependencyOrder {
            order: Vec::new(),
            dependents: HashMap::new(),
        });
    }

    // Build key set for O(1) lookup
    let key_set: HashSet<&K> = entries.iter().map(|e| &e.key).collect();

    // Check for self-referential dependencies (clearer error message)
    for entry in entries {
        if entry.required.contains(&entry.key) || entry.optional.contains(&entry.key) {
            return Err(DepgraphError::SelfReferential(entry.key.clone()));
        }
    }

    // Check for missing required dependencies
    let mut missing = Vec::new();
    for entry in entries {
        for dep in &entry.required {
            if !key_set.contains(dep) {
                missing.push((entry.key.clone(), dep.clone()));
            }
        }
    }
    if !missing.is_empty() {
        return Err(DepgraphError::Missing(missing));
    }

    // Build in-degree map and adjacency list
    let mut in_degree: HashMap<&K, usize> = HashMap::new();
    let mut adjacency: HashMap<&K, Vec<&K>> = HashMap::new();

    for entry in entries {
        in_degree.entry(&entry.key).or_insert(0);
        adjacency.entry(&entry.key).or_default();
    }

    for entry in entries {
        // Required dependencies: always count
        for dep in &entry.required {
            *in_degree.entry(&entry.key).or_insert(0) += 1;
            adjacency.entry(dep).or_default().push(&entry.key);
        }

        // Optional dependencies: only count if present in the entry set
        for dep in &entry.optional {
            if key_set.contains(dep) {
                *in_degree.entry(&entry.key).or_insert(0) += 1;
                adjacency.entry(dep).or_default().push(&entry.key);
            }
        }
    }

    // Kahn's algorithm: BFS from nodes with in-degree 0
    let mut queue: VecDeque<&K> = in_degree
        .iter()
        .filter(|&(_, &degree)| degree == 0)
        .map(|(&key, _)| key)
        .collect();

    let mut order = Vec::with_capacity(entries.len());

    while let Some(key) = queue.pop_front() {
        order.push(key.clone());

        if let Some(deps) = adjacency.get(key) {
            for dep in deps {
                if let Some(degree) = in_degree.get_mut(dep) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(dep);
                    }
                }
            }
        }
    }

    // Detect cycles: any node with remaining in-degree > 0
    if order.len() != entries.len() {
        let in_cycle: Vec<K> = in_degree
            .iter()
            .filter(|&(_, &d)| d > 0)
            .map(|(&key, _)| key.clone())
            .collect();
        return Err(DepgraphError::Cycle(in_cycle));
    }

    // Build reverse dependency map (owned)
    let mut dependents: HashMap<K, HashSet<K>> = HashMap::new();
    for entry in entries {
        for dep in &entry.required {
            dependents
                .entry(dep.clone())
                .or_default()
                .insert(entry.key.clone());
        }
        for dep in &entry.optional {
            if key_set.contains(dep) {
                dependents
                    .entry(dep.clone())
                    .or_default()
                    .insert(entry.key.clone());
            }
        }
    }

    Ok(DependencyOrder { order, dependents })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &'static str) -> DepEntry<&'static str> {
        DepEntry {
            key,
            required: vec![],
            optional: vec![],
        }
    }

    fn entry_with_deps(key: &'static str, deps: &[&'static str]) -> DepEntry<&'static str> {
        DepEntry {
            key,
            required: deps.to_vec(),
            optional: vec![],
        }
    }

    fn entry_with_opt(
        key: &'static str,
        deps: &[&'static str],
        opt: &[&'static str],
    ) -> DepEntry<&'static str> {
        DepEntry {
            key,
            required: deps.to_vec(),
            optional: opt.to_vec(),
        }
    }

    fn pos(order: &[&str], key: &str) -> usize {
        order.iter().position(|k| *k == key).unwrap()
    }

    // =========================================================================
    // Basic cases
    // =========================================================================

    #[test]
    fn test_empty_input() {
        let result = resolve_dependencies::<&str>(&[]).unwrap();
        assert!(result.order.is_empty());
        assert!(result.dependents.is_empty());
    }

    #[test]
    fn test_single_entry() {
        let result = resolve_dependencies(&[entry("a")]).unwrap();
        assert_eq!(result.order, vec!["a"]);
    }

    #[test]
    fn test_no_dependencies() {
        let result = resolve_dependencies(&[entry("a"), entry("b"), entry("c")]).unwrap();
        assert_eq!(result.order.len(), 3);
    }

    // =========================================================================
    // Ordering
    // =========================================================================

    #[test]
    fn test_simple_chain() {
        let entries = vec![
            entry("a"),
            entry_with_deps("b", &["a"]),
            entry_with_deps("c", &["b"]),
        ];
        let result = resolve_dependencies(&entries).unwrap();

        assert!(pos(&result.order, "a") < pos(&result.order, "b"));
        assert!(pos(&result.order, "b") < pos(&result.order, "c"));
    }

    #[test]
    fn test_diamond_dependency() {
        let entries = vec![
            entry("d"),
            entry_with_deps("b", &["d"]),
            entry_with_deps("c", &["d"]),
            entry_with_deps("a", &["b", "c"]),
        ];
        let result = resolve_dependencies(&entries).unwrap();

        assert!(pos(&result.order, "d") < pos(&result.order, "b"));
        assert!(pos(&result.order, "d") < pos(&result.order, "c"));
        assert!(pos(&result.order, "b") < pos(&result.order, "a"));
        assert!(pos(&result.order, "c") < pos(&result.order, "a"));
    }

    #[test]
    fn test_optional_ordering() {
        let entries = vec![entry("b"), entry_with_opt("a", &[], &["b"])];
        let result = resolve_dependencies(&entries).unwrap();

        assert!(pos(&result.order, "b") < pos(&result.order, "a"));
    }

    #[test]
    fn test_disconnected_components() {
        let entries = vec![
            entry("a"),
            entry_with_deps("b", &["a"]),
            entry("c"),
            entry_with_deps("d", &["c"]),
        ];
        let result = resolve_dependencies(&entries).unwrap();

        assert!(pos(&result.order, "a") < pos(&result.order, "b"));
        assert!(pos(&result.order, "c") < pos(&result.order, "d"));
        assert_eq!(result.order.len(), 4);
    }

    #[test]
    fn test_complex_diamond_with_optional() {
        let entries = vec![
            entry("base"),
            entry_with_deps("left", &["base"]),
            entry_with_opt("right", &["base"], &["left"]),
            entry_with_deps("top", &["left", "right"]),
        ];
        let result = resolve_dependencies(&entries).unwrap();

        assert!(pos(&result.order, "base") < pos(&result.order, "left"));
        assert!(pos(&result.order, "base") < pos(&result.order, "right"));
        // right has optional dep on left, so left should come before right
        assert!(pos(&result.order, "left") < pos(&result.order, "right"));
        assert!(pos(&result.order, "right") < pos(&result.order, "top"));
    }

    // =========================================================================
    // Error cases
    // =========================================================================

    #[test]
    fn test_self_referential() {
        let entries = vec![DepEntry {
            key: "a",
            required: vec!["a"],
            optional: vec![],
        }];
        let err = resolve_dependencies(&entries).unwrap_err();
        assert!(matches!(err, DepgraphError::SelfReferential("a")));
    }

    #[test]
    fn test_self_referential_optional() {
        let entries = vec![DepEntry {
            key: "a",
            required: vec![],
            optional: vec!["a"],
        }];
        let err = resolve_dependencies(&entries).unwrap_err();
        assert!(matches!(err, DepgraphError::SelfReferential("a")));
    }

    #[test]
    fn test_circular_two() {
        let entries = vec![entry_with_deps("a", &["b"]), entry_with_deps("b", &["a"])];
        let err = resolve_dependencies(&entries).unwrap_err();
        assert!(matches!(err, DepgraphError::Cycle(_)));
    }

    #[test]
    fn test_circular_three() {
        let entries = vec![
            entry_with_deps("a", &["c"]),
            entry_with_deps("b", &["a"]),
            entry_with_deps("c", &["b"]),
        ];
        let err = resolve_dependencies(&entries).unwrap_err();
        match err {
            DepgraphError::Cycle(keys) => {
                assert_eq!(keys.len(), 3);
            }
            other => panic!("Expected Cycle, got {other:?}"),
        }
    }

    #[test]
    fn test_missing_required() {
        let entries = vec![entry_with_deps("a", &["nonexistent"])];
        let err = resolve_dependencies(&entries).unwrap_err();
        match err {
            DepgraphError::Missing(pairs) => {
                assert_eq!(pairs.len(), 1);
                assert_eq!(pairs[0], ("a", "nonexistent"));
            }
            other => panic!("Expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn test_missing_multiple_required() {
        let entries = vec![
            entry_with_deps("a", &["x"]),
            entry_with_deps("b", &["y", "z"]),
        ];
        let err = resolve_dependencies(&entries).unwrap_err();
        match err {
            DepgraphError::Missing(pairs) => {
                assert_eq!(pairs.len(), 3);
            }
            other => panic!("Expected Missing, got {other:?}"),
        }
    }

    #[test]
    fn test_missing_optional_ok() {
        let entries = vec![entry_with_opt("a", &[], &["nonexistent"])];
        let result = resolve_dependencies(&entries).unwrap();
        assert_eq!(result.order, vec!["a"]);
    }

    // =========================================================================
    // Reverse dependency map
    // =========================================================================

    #[test]
    fn test_dependents_map() {
        let entries = vec![
            entry("a"),
            entry_with_deps("b", &["a"]),
            entry_with_deps("c", &["a"]),
        ];
        let result = resolve_dependencies(&entries).unwrap();

        let a_deps = result.dependents.get("a").unwrap();
        assert!(a_deps.contains("b"));
        assert!(a_deps.contains("c"));
        assert_eq!(a_deps.len(), 2);
    }

    #[test]
    fn test_dependents_includes_optional() {
        let entries = vec![entry("a"), entry_with_opt("b", &[], &["a"])];
        let result = resolve_dependencies(&entries).unwrap();

        let a_deps = result.dependents.get("a").unwrap();
        assert!(a_deps.contains("b"));
    }

    // =========================================================================
    // Display and Error trait
    // =========================================================================

    #[test]
    fn test_error_display_self_referential() {
        let err = DepgraphError::SelfReferential("foo");
        let msg = format!("{err}");
        assert!(msg.contains("foo"));
        assert!(msg.contains("depends on itself"));
    }

    #[test]
    fn test_error_display_cycle() {
        let err = DepgraphError::Cycle(vec!["a", "b"]);
        let msg = format!("{err}");
        assert!(msg.contains("circular dependency"));
    }

    #[test]
    fn test_error_display_missing() {
        let err = DepgraphError::Missing(vec![("a", "b"), ("c", "d")]);
        let msg = format!("{err}");
        assert!(msg.contains("missing dependencies"));
        assert!(msg.contains("requires"));
    }

    #[test]
    fn test_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(DepgraphError::SelfReferential("test"));
        assert!(!err.to_string().is_empty());
    }

    // =========================================================================
    // Clone and genericity
    // =========================================================================

    #[test]
    fn test_dep_entry_clone() {
        let e = entry_with_deps("a", &["b"]);
        let cloned = e.clone();
        assert_eq!(cloned.key, "a");
        assert_eq!(cloned.required, vec!["b"]);
    }

    #[test]
    fn test_dependency_order_clone() {
        let result = resolve_dependencies(&[entry("a"), entry_with_deps("b", &["a"])]).unwrap();
        let cloned = result.clone();
        assert_eq!(cloned.order, result.order);
    }

    #[test]
    fn test_with_custom_key_type() {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        struct ModId(u32);

        let entries = vec![
            DepEntry {
                key: ModId(1),
                required: vec![],
                optional: vec![],
            },
            DepEntry {
                key: ModId(2),
                required: vec![ModId(1)],
                optional: vec![],
            },
        ];
        let result = resolve_dependencies(&entries).unwrap();
        assert_eq!(result.order, vec![ModId(1), ModId(2)]);
    }

    // =========================================================================
    // Scale test
    // =========================================================================

    #[test]
    fn test_large_graph() {
        // 100 entries in a linear chain: 0 -> 1 -> 2 -> ... -> 99
        let entries: Vec<DepEntry<usize>> = (0..100)
            .map(|i| {
                if i == 0 {
                    DepEntry {
                        key: i,
                        required: vec![],
                        optional: vec![],
                    }
                } else {
                    DepEntry {
                        key: i,
                        required: vec![i - 1],
                        optional: vec![],
                    }
                }
            })
            .collect();

        let result = resolve_dependencies(&entries).unwrap();
        assert_eq!(result.order.len(), 100);

        // Verify all ordering invariants
        for i in 1..100 {
            let prev_pos = result.order.iter().position(|&k| k == i - 1).unwrap();
            let curr_pos = result.order.iter().position(|&k| k == i).unwrap();
            assert!(prev_pos < curr_pos, "Entry {i} should come after {}", i - 1);
        }
    }
}

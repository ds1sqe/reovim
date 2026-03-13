//! Kahn's algorithm for topological sort over dependency graphs.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    hash::Hash,
};

/// Input entry for dependency resolution.
///
/// Each entry declares its key, required dependencies, optional dependencies,
/// and abstract capabilities (#618).
///
/// Required dependencies must exist in the entry set; missing ones produce an error.
/// Optional dependencies are ordered (if present) but silently skipped if absent.
/// Capabilities enable abstract matching: a module that `requires_caps` a capability
/// is satisfied by any module that `provides_caps` it (implicit ordering edge).
#[derive(Debug, Clone)]
pub struct DepEntry<K> {
    /// Unique identifier for this entry.
    pub key: K,
    /// Required dependencies — must exist in the entry set.
    pub required: Vec<K>,
    /// Optional dependencies — ordered if present, skipped if absent.
    pub optional: Vec<K>,
    /// Capabilities this entry provides (#618).
    pub provides_caps: Vec<&'static str>,
    /// Capabilities this entry requires (#618).
    pub requires_caps: Vec<&'static str>,
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
    /// Required capabilities have no provider (#618).
    /// Each pair is `(dependent_key, capability_name)`.
    UnsatisfiedCapability(Vec<(K, String)>),
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
            Self::UnsatisfiedCapability(pairs) => {
                write!(f, "unsatisfied capabilities: ")?;
                for (i, (key, cap)) in pairs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{key:?} requires capability {cap:?}")?;
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
#[allow(clippy::too_many_lines)]
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

    // Build capability provider map (#618): capability -> first provider key
    let mut cap_providers: HashMap<&str, &K> = HashMap::new();
    for entry in entries {
        for cap in &entry.provides_caps {
            // First-registered wins (deterministic)
            cap_providers.entry(cap).or_insert(&entry.key);
        }
    }

    // Check for unsatisfied capability requirements (#618)
    let mut unsatisfied = Vec::new();
    for entry in entries {
        for cap in &entry.requires_caps {
            if !cap_providers.contains_key(cap) {
                unsatisfied.push((entry.key.clone(), (*cap).to_string()));
            }
        }
    }
    if !unsatisfied.is_empty() {
        return Err(DepgraphError::UnsatisfiedCapability(unsatisfied));
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

        // Capability-based implicit edges (#618): requires_caps -> provider
        for cap in &entry.requires_caps {
            if let Some(&provider) = cap_providers.get(cap)
                && provider != &entry.key
            {
                // Skip self-edges (module provides and requires same cap)
                *in_degree.entry(&entry.key).or_insert(0) += 1;
                adjacency.entry(provider).or_default().push(&entry.key);
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
        // Capability-based reverse deps (#618)
        for cap in &entry.requires_caps {
            if let Some(&provider) = cap_providers.get(cap)
                && provider != &entry.key
            {
                dependents
                    .entry(provider.clone())
                    .or_default()
                    .insert(entry.key.clone());
            }
        }
    }

    Ok(DependencyOrder { order, dependents })
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;

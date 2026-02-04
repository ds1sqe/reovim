//! Dependency resolution using topological sort.
//!
//! Implements Kahn's algorithm for determining module initialization order
//! based on declared dependencies.

use std::collections::{HashMap, HashSet, VecDeque};

use reovim_kernel::api::v1::{ModuleError, ModuleId};

use super::handle::ModuleHandle;

/// Result of dependency resolution.
#[derive(Debug, Clone)]
pub struct DependencyOrder {
    /// Modules in topologically sorted order (dependencies first).
    pub order: Vec<ModuleId>,
    /// Reverse dependency map: module -> modules that depend on it.
    pub dependents: HashMap<ModuleId, HashSet<ModuleId>>,
}

/// Resolve module dependencies using topological sort (Kahn's algorithm).
///
/// # Arguments
///
/// * `modules` - Map of module ID to module handle
///
/// # Returns
///
/// * `Ok(DependencyOrder)` - Sorted order and reverse dependency map
/// * `Err(ModuleError)` - If circular or missing dependencies detected
///
/// # Algorithm
///
/// Kahn's algorithm:
/// 1. Build in-degree map (count of dependencies for each module)
/// 2. Start with modules that have no dependencies (in-degree = 0)
/// 3. Process each module, reducing in-degree of dependents
/// 4. If any modules remain with in-degree > 0, there's a cycle
///
/// # Errors
///
/// Returns error if:
/// - Module depends on itself (self-referential)
/// - Circular dependency detected
/// - Required dependency is missing (and not optional)
pub fn resolve_dependencies(
    modules: &HashMap<ModuleId, ModuleHandle>,
) -> Result<DependencyOrder, ModuleError> {
    // Check for self-referential dependencies FIRST (clearer error message)
    for (id, handle) in modules {
        if handle.dependencies().contains(id) {
            return Err(ModuleError::LoadFailed(format!(
                "module '{}' depends on itself",
                id.as_str()
            )));
        }
    }

    // Build adjacency list and in-degree map
    let mut in_degree: HashMap<&ModuleId, usize> = HashMap::new();
    let mut dependents_list: HashMap<&ModuleId, Vec<&ModuleId>> = HashMap::new();
    let mut missing = Vec::new();

    // Initialize all modules with in-degree 0
    for id in modules.keys() {
        in_degree.entry(id).or_insert(0);
        dependents_list.entry(id).or_default();
    }

    // Build dependency graph
    for (id, handle) in modules {
        for dep_id in handle.dependencies() {
            if let Some(dep_key) = modules.keys().find(|k| **k == dep_id) {
                // Increment in-degree for this module (it depends on dep)
                *in_degree.entry(id).or_insert(0) += 1;
                // Add this module to dep's dependents list
                dependents_list.entry(dep_key).or_default().push(id);
            } else {
                // Check if optional
                if !handle.optional_dependencies().contains(&dep_id) {
                    missing.push((id.clone(), dep_id));
                }
            }
        }

        // Handle optional dependencies if they exist
        for opt_dep_id in handle.optional_dependencies() {
            if let Some(dep_key) = modules.keys().find(|k| **k == opt_dep_id) {
                // Optional dependency exists - add to graph
                *in_degree.entry(id).or_insert(0) += 1;
                dependents_list.entry(dep_key).or_default().push(id);
            }
            // If optional dep doesn't exist, that's fine - skip it
        }
    }

    if !missing.is_empty() {
        let msg = missing
            .iter()
            .map(|(m, d)| format!("'{}' requires '{}'", m.as_str(), d.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(ModuleError::NotFound(format!("missing dependencies: {msg}")));
    }

    // Kahn's algorithm
    let mut queue: VecDeque<&ModuleId> = in_degree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| *id)
        .collect();

    let mut order = Vec::new();

    while let Some(id) = queue.pop_front() {
        order.push(id.clone());

        if let Some(deps) = dependents_list.get(id) {
            for dep_id in deps {
                if let Some(degree) = in_degree.get_mut(dep_id) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(dep_id);
                    }
                }
            }
        }
    }

    // Check for cycles
    if order.len() != modules.len() {
        let in_cycle: Vec<_> = in_degree
            .iter()
            .filter(|(_, d)| **d > 0)
            .map(|(id, _)| id.as_str())
            .collect();
        return Err(ModuleError::LoadFailed(format!("circular dependency detected: {in_cycle:?}")));
    }

    // Build reverse dependency map (owned)
    let mut dependents: HashMap<ModuleId, HashSet<ModuleId>> = HashMap::new();
    for (id, handle) in modules {
        for dep_id in handle.dependencies() {
            if modules.contains_key(&dep_id) {
                dependents.entry(dep_id).or_default().insert(id.clone());
            }
        }
        for opt_dep_id in handle.optional_dependencies() {
            if modules.contains_key(&opt_dep_id) {
                dependents.entry(opt_dep_id).or_default().insert(id.clone());
            }
        }
    }

    Ok(DependencyOrder { order, dependents })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_kernel::api::v1::{Module, ModuleContext, ProbeResult, Version},
    };

    /// Create a test module with specific dependencies.
    struct TestModule {
        id: &'static str,
        deps: Vec<&'static str>,
        opt_deps: Vec<&'static str>,
    }

    impl TestModule {
        fn new(id: &'static str) -> Self {
            Self {
                id,
                deps: Vec::new(),
                opt_deps: Vec::new(),
            }
        }

        fn with_deps(id: &'static str, deps: &[&'static str]) -> Self {
            Self {
                id,
                deps: deps.to_vec(),
                opt_deps: Vec::new(),
            }
        }

        fn with_opt_deps(
            id: &'static str,
            deps: &[&'static str],
            opt_deps: &[&'static str],
        ) -> Self {
            Self {
                id,
                deps: deps.to_vec(),
                opt_deps: opt_deps.to_vec(),
            }
        }
    }

    impl Module for TestModule {
        fn id(&self) -> ModuleId {
            ModuleId::new(self.id)
        }

        fn name(&self) -> &'static str {
            self.id
        }

        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }

        fn dependencies(&self) -> Vec<ModuleId> {
            self.deps.iter().map(|s| ModuleId::new(s)).collect()
        }

        fn optional_dependencies(&self) -> Vec<ModuleId> {
            self.opt_deps.iter().map(|s| ModuleId::new(s)).collect()
        }

        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }

        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    fn create_modules(modules: Vec<TestModule>) -> HashMap<ModuleId, ModuleHandle> {
        modules
            .into_iter()
            .map(|m| {
                let id = m.id();
                (id, ModuleHandle::from_static(m))
            })
            .collect()
    }

    #[test]
    fn test_no_dependencies() {
        let modules = create_modules(vec![
            TestModule::new("a"),
            TestModule::new("b"),
            TestModule::new("c"),
        ]);

        let result = resolve_dependencies(&modules).unwrap();
        assert_eq!(result.order.len(), 3);
    }

    #[test]
    fn test_simple_chain() {
        // C depends on B depends on A
        let modules = create_modules(vec![
            TestModule::new("a"),
            TestModule::with_deps("b", &["a"]),
            TestModule::with_deps("c", &["b"]),
        ]);

        let result = resolve_dependencies(&modules).unwrap();

        let a_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "a")
            .unwrap();
        let b_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "b")
            .unwrap();
        let c_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "c")
            .unwrap();

        assert!(a_idx < b_idx);
        assert!(b_idx < c_idx);
    }

    #[test]
    fn test_diamond_dependency() {
        // A depends on B and C, both depend on D
        let modules = create_modules(vec![
            TestModule::new("d"),
            TestModule::with_deps("b", &["d"]),
            TestModule::with_deps("c", &["d"]),
            TestModule::with_deps("a", &["b", "c"]),
        ]);

        let result = resolve_dependencies(&modules).unwrap();

        let d_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "d")
            .unwrap();
        let b_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "b")
            .unwrap();
        let c_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "c")
            .unwrap();
        let a_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "a")
            .unwrap();

        assert!(d_idx < b_idx);
        assert!(d_idx < c_idx);
        assert!(b_idx < a_idx);
        assert!(c_idx < a_idx);
    }

    #[test]
    fn test_self_referential_dependency() {
        struct SelfRefModule;

        impl Module for SelfRefModule {
            fn id(&self) -> ModuleId {
                ModuleId::new("self-ref")
            }
            fn name(&self) -> &'static str {
                "Self Ref"
            }
            fn version(&self) -> Version {
                Version::new(1, 0, 0)
            }
            fn dependencies(&self) -> Vec<ModuleId> {
                vec![ModuleId::new("self-ref")]
            }
            fn init(&mut self, _: &ModuleContext) -> ProbeResult {
                ProbeResult::Success
            }
            fn exit(&mut self) -> Result<(), ModuleError> {
                Ok(())
            }
        }

        let mut modules = HashMap::new();
        modules.insert(ModuleId::new("self-ref"), ModuleHandle::from_static(SelfRefModule));

        let result = resolve_dependencies(&modules);
        assert!(matches!(result, Err(ModuleError::LoadFailed(_))));
    }

    #[test]
    fn test_circular_dependency() {
        // A depends on B, B depends on A
        let modules = create_modules(vec![
            TestModule::with_deps("a", &["b"]),
            TestModule::with_deps("b", &["a"]),
        ]);

        let result = resolve_dependencies(&modules);
        assert!(matches!(result, Err(ModuleError::LoadFailed(_))));
    }

    #[test]
    fn test_missing_required_dependency() {
        let modules = create_modules(vec![TestModule::with_deps("a", &["nonexistent"])]);

        let result = resolve_dependencies(&modules);
        assert!(matches!(result, Err(ModuleError::NotFound(_))));
    }

    #[test]
    fn test_missing_optional_dependency_ok() {
        let modules = create_modules(vec![TestModule::with_opt_deps("a", &[], &["nonexistent"])]);

        let result = resolve_dependencies(&modules);
        assert!(result.is_ok());
    }

    #[test]
    fn test_optional_dependency_ordering() {
        // A has optional dep on B
        let modules = create_modules(vec![
            TestModule::new("b"),
            TestModule::with_opt_deps("a", &[], &["b"]),
        ]);

        let result = resolve_dependencies(&modules).unwrap();

        let b_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "b")
            .unwrap();
        let a_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "a")
            .unwrap();

        assert!(b_idx < a_idx);
    }

    #[test]
    fn test_dependents_map() {
        let modules = create_modules(vec![
            TestModule::new("a"),
            TestModule::with_deps("b", &["a"]),
            TestModule::with_deps("c", &["a"]),
        ]);

        let result = resolve_dependencies(&modules).unwrap();

        let a_deps = result.dependents.get(&ModuleId::new("a")).unwrap();
        assert!(a_deps.contains(&ModuleId::new("b")));
        assert!(a_deps.contains(&ModuleId::new("c")));
    }

    #[test]
    fn test_disconnected_components() {
        // Component 1: A -> B, Component 2: C -> D (no connection)
        let modules = create_modules(vec![
            TestModule::new("a"),
            TestModule::with_deps("b", &["a"]),
            TestModule::new("c"),
            TestModule::with_deps("d", &["c"]),
        ]);

        let result = resolve_dependencies(&modules).unwrap();

        let a_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "a")
            .unwrap();
        let b_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "b")
            .unwrap();
        let c_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "c")
            .unwrap();
        let d_idx = result
            .order
            .iter()
            .position(|id| id.as_str() == "d")
            .unwrap();

        // Within-component ordering must be correct
        assert!(a_idx < b_idx);
        assert!(c_idx < d_idx);
        // Total should be 4
        assert_eq!(result.order.len(), 4);
    }
}

//! Client module loading, dependency resolution, and lifecycle management.
//!
//! Mirrors the server-side `ModuleLoader` + `ModuleRegistry` pattern adapted
//! for `ClientModule`. Handles factory consumption, topological dependency
//! ordering via `reovim-depgraph`, multi-pass initialization with deferral,
//! and reverse-order shutdown.

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use reovim_depgraph::{DepEntry, resolve_dependencies};

use crate::{
    traits::{ClientModule, ModuleContext},
    types::ProbeResult,
};

/// Factory function that constructs a `ClientModule` instance.
pub type ClientModuleFactory = fn() -> Box<dyn ClientModule>;

/// Maximum number of initialization passes for deferred modules.
const MAX_DEFER_PASSES: usize = 3;

// =============================================================================
// ClientModuleState
// =============================================================================

/// Lifecycle state of a client module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientModuleState {
    /// Constructed but not yet initialized.
    Loaded,
    /// `init()` is currently in progress.
    Initializing,
    /// `init()` succeeded; module is active.
    Running,
    /// `init()` or `exit()` failed (reason stored).
    Failed(String),
}

impl ClientModuleState {
    /// Whether a transition to `target` is valid from the current state.
    #[must_use]
    pub const fn can_transition_to(&self, target: &Self) -> bool {
        matches!(
            (self, target),
            (Self::Loaded, Self::Initializing)
                | (Self::Initializing, Self::Running | Self::Failed(_))
                | (Self::Running, Self::Loaded | Self::Failed(_))
        )
    }
}

impl fmt::Display for ClientModuleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Loaded => write!(f, "Loaded"),
            Self::Initializing => write!(f, "Initializing"),
            Self::Running => write!(f, "Running"),
            Self::Failed(reason) => write!(f, "Failed({reason})"),
        }
    }
}

// =============================================================================
// ClientModuleLoaderError
// =============================================================================

/// Error during client module loading.
#[derive(Debug)]
pub enum ClientModuleLoaderError {
    /// Dependency resolution failed (cycle, missing dep, etc.).
    DependencyResolution(String),
}

impl fmt::Display for ClientModuleLoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DependencyResolution(msg) => {
                write!(f, "client module dependency resolution failed: {msg}")
            }
        }
    }
}

impl std::error::Error for ClientModuleLoaderError {}

// =============================================================================
// ClientModuleLoader
// =============================================================================

/// Loads, resolves dependencies, and manages the lifecycle of client modules.
///
/// Construct via [`ClientModuleLoader::new()`] with a factory map and disabled
/// set. The loader instantiates modules, resolves dependencies via topological
/// sort, and provides `init_all()` / `on_all_loaded()` / `exit_all()` lifecycle
/// methods.
///
/// Uses parallel arrays for modules and states so that callers can borrow
/// `&[Box<dyn ClientModule>]` directly (needed by render engine, notification
/// handler, etc.).
pub struct ClientModuleLoader {
    /// Modules in dependency-resolved order.
    modules: Vec<Box<dyn ClientModule>>,
    /// Per-module lifecycle state (parallel to `modules`).
    states: Vec<ClientModuleState>,
    /// Module kinds in dependency-resolved init order.
    init_order: Vec<String>,
}

/// Result of dependency resolution: reordered modules, states, and init order.
type ResolveResult = Result<
    (Vec<Box<dyn ClientModule>>, Vec<ClientModuleState>, Vec<String>),
    ClientModuleLoaderError,
>;

/// Resolve dependencies and reorder modules by topological sort.
///
/// Shared between `new()` and `from_modules_for_test()`.
#[cfg_attr(coverage_nightly, coverage(off))]
fn resolve_and_reorder(
    modules: Vec<Box<dyn ClientModule>>,
    states: Vec<ClientModuleState>,
) -> ResolveResult {
    if modules.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }

    // Collect module info into owned data (avoids borrow of modules vec).
    let module_info: Vec<(String, Vec<String>, Vec<String>)> = modules
        .iter()
        .map(|m| {
            (
                m.kind().to_string(),
                m.dependencies().iter().map(|s| (*s).to_string()).collect(),
                m.optional_dependencies()
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
            )
        })
        .collect();

    let dep_entries: Vec<DepEntry<String>> = module_info
        .iter()
        .map(|(kind, deps, opt_deps)| DepEntry {
            key: kind.clone(),
            required: deps.clone(),
            optional: opt_deps.clone(),
            provides_caps: Vec::new(),
            requires_caps: Vec::new(),
        })
        .collect();

    let resolved = resolve_dependencies(&dep_entries)
        .map_err(|e| ClientModuleLoaderError::DependencyResolution(format!("{e}")))?;

    // Build kind -> index map for reordering
    let kind_to_idx: HashMap<&str, usize> = modules
        .iter()
        .enumerate()
        .map(|(i, m)| (m.kind(), i))
        .collect();

    let init_order: Vec<String> = resolved.order.clone();

    // Reorder by resolved topological order using Option-swap pattern
    let mut mod_slots: Vec<Option<Box<dyn ClientModule>>> = modules.into_iter().map(Some).collect();
    let mut state_slots: Vec<Option<ClientModuleState>> = states.into_iter().map(Some).collect();
    let mut sorted_mods = Vec::with_capacity(mod_slots.len());
    let mut sorted_states = Vec::with_capacity(state_slots.len());

    for kind in &resolved.order {
        if let Some(&idx) = kind_to_idx.get(kind.as_str())
            && let Some(module) = mod_slots[idx].take()
            && let Some(state) = state_slots[idx].take()
        {
            sorted_mods.push(module);
            sorted_states.push(state);
        }
    }

    Ok((sorted_mods, sorted_states, init_order))
}

impl ClientModuleLoader {
    /// Create a new loader by instantiating modules from factories.
    ///
    /// Filters out disabled modules, instantiates the rest, resolves
    /// dependencies via topological sort, and reorders modules accordingly.
    ///
    /// # Errors
    ///
    /// Returns [`ClientModuleLoaderError::DependencyResolution`] if a required
    /// dependency is missing, a cycle is detected, or self-referential deps exist.
    pub fn new<S: std::hash::BuildHasher, S2: std::hash::BuildHasher>(
        factories: HashMap<&'static str, ClientModuleFactory, S>,
        disabled: &HashSet<String, S2>,
    ) -> Result<Self, ClientModuleLoaderError> {
        Self::new_with_dynamic(factories, disabled, Vec::new())
    }

    /// Create a loader from factory map **plus** pre-built dynamic modules.
    ///
    /// Factories are filtered by the `disabled` set and instantiated. Dynamic
    /// modules are assumed to already be filtered by the discovery layer (see
    /// `clients/tui/src/dynamic_module.rs::discover_dynamic_client_modules`).
    /// Both sets are merged into a single module vector and fed through the
    /// same dependency resolver as [`Self::new`], so a static module can
    /// depend on a dynamic module and vice versa.
    ///
    /// # Errors
    ///
    /// Returns [`ClientModuleLoaderError::DependencyResolution`] on cycle or
    /// missing dependency across the combined set.
    pub fn new_with_dynamic<S: std::hash::BuildHasher, S2: std::hash::BuildHasher>(
        factories: HashMap<&'static str, ClientModuleFactory, S>,
        disabled: &HashSet<String, S2>,
        dynamic_modules: Vec<Box<dyn ClientModule>>,
    ) -> Result<Self, ClientModuleLoaderError> {
        let mut modules: Vec<Box<dyn ClientModule>> = factories
            .into_iter()
            .filter(|(kind, _)| !disabled.contains(*kind))
            .map(|(_, factory)| factory())
            .collect();
        modules.extend(dynamic_modules);
        let states = vec![ClientModuleState::Loaded; modules.len()];

        let (modules, states, init_order) = resolve_and_reorder(modules, states)?;
        Ok(Self {
            modules,
            states,
            init_order,
        })
    }

    /// Initialize all modules in dependency order.
    ///
    /// Uses multi-pass retry for modules that return `ProbeResult::Defer`
    /// (up to 3 passes, matching server behavior). Returns the count of
    /// successfully initialized modules.
    // Multi-pass init with deferred/failed states — tested by integration tests.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn init_all(&mut self, ctx: &ModuleContext) -> usize {
        let mut initialized = vec![false; self.modules.len()];
        let mut success_count = 0;

        for pass in 0..MAX_DEFER_PASSES {
            let mut any_deferred = false;

            for (i, done) in initialized.iter_mut().enumerate() {
                if *done {
                    continue;
                }
                if matches!(self.states[i], ClientModuleState::Failed(_)) {
                    continue;
                }

                self.states[i] = ClientModuleState::Initializing;

                match self.modules[i].init(ctx) {
                    ProbeResult::Success => {
                        self.states[i] = ClientModuleState::Running;
                        *done = true;
                        success_count += 1;
                        tracing::debug!(
                            module = self.modules[i].kind(),
                            "client module initialized (pass {pass})"
                        );
                    }
                    ProbeResult::Defer(reason) => {
                        self.states[i] = ClientModuleState::Loaded;
                        any_deferred = true;
                        tracing::debug!(
                            module = self.modules[i].kind(),
                            reason = %reason,
                            "client module deferred (pass {pass})"
                        );
                    }
                    ProbeResult::Failed(err) => {
                        self.states[i] = ClientModuleState::Failed(err.message().to_owned());
                        *done = true;
                        tracing::warn!(
                            module = self.modules[i].kind(),
                            error = %err.message(),
                            "client module init failed"
                        );
                    }
                }
            }

            if !any_deferred {
                break;
            }
        }

        // Mark permanently deferred modules as failed
        for (i, done) in initialized.iter().enumerate() {
            if !done && !matches!(self.states[i], ClientModuleState::Failed(_)) {
                self.states[i] =
                    ClientModuleState::Failed("permanently deferred after max passes".to_string());
                tracing::warn!(
                    module = self.modules[i].kind(),
                    "client module permanently deferred"
                );
            }
        }

        success_count
    }

    /// Call `on_all_loaded()` on each running module.
    pub fn on_all_loaded(&mut self, ctx: &ModuleContext) {
        for i in 0..self.modules.len() {
            if self.states[i] == ClientModuleState::Running {
                self.modules[i].on_all_loaded(ctx);
            }
        }
    }

    /// Shut down all running modules in reverse dependency order.
    ///
    /// Logs failures but never aborts -- all modules get a chance to clean up.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn exit_all(&mut self) {
        // Build kind -> index map
        let kind_to_idx: HashMap<&str, usize> = self
            .modules
            .iter()
            .enumerate()
            .map(|(i, m)| (m.kind(), i))
            .collect();

        // Exit in reverse init order
        let reverse_order: Vec<usize> = self
            .init_order
            .iter()
            .rev()
            .filter_map(|kind| kind_to_idx.get(kind.as_str()).copied())
            .collect();

        for idx in reverse_order {
            if self.states[idx] != ClientModuleState::Running {
                continue;
            }

            match self.modules[idx].exit() {
                Ok(()) => {
                    self.states[idx] = ClientModuleState::Loaded;
                    tracing::debug!(module = self.modules[idx].kind(), "client module exited");
                }
                Err(err) => {
                    self.states[idx] = ClientModuleState::Failed(err.message().to_owned());
                    tracing::warn!(
                        module = self.modules[idx].kind(),
                        error = %err.message(),
                        "client module exit failed"
                    );
                }
            }
        }
    }

    /// Number of loaded modules (all states).
    #[must_use]
    pub const fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Number of modules in `Running` state.
    #[must_use]
    pub fn running_count(&self) -> usize {
        self.states
            .iter()
            .filter(|s| **s == ClientModuleState::Running)
            .count()
    }

    /// Query the state of a module by kind.
    #[must_use]
    pub fn state(&self, kind: &str) -> Option<&ClientModuleState> {
        self.modules
            .iter()
            .position(|m| m.kind() == kind)
            .map(|i| &self.states[i])
    }

    /// Borrow all modules as a slice of boxed trait objects.
    #[must_use]
    pub fn modules(&self) -> &[Box<dyn ClientModule>] {
        &self.modules
    }

    /// Mutable borrow of all modules as a slice of boxed trait objects.
    pub fn modules_mut_slice(&mut self) -> &mut [Box<dyn ClientModule>] {
        &mut self.modules
    }

    /// Borrow all modules as trait object references.
    #[must_use]
    pub fn as_module_slice(&self) -> Vec<&dyn ClientModule> {
        self.modules.iter().map(AsRef::as_ref).collect()
    }

    /// Mutable iterator over the underlying boxed modules.
    pub fn modules_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn ClientModule>> {
        self.modules.iter_mut()
    }

    /// Consume the loader and return all modules as owned boxes.
    #[must_use]
    pub fn into_modules(self) -> Vec<Box<dyn ClientModule>> {
        self.modules
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl fmt::Debug for ClientModuleLoader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientModuleLoader")
            .field("module_count", &self.modules.len())
            .field("states", &self.states)
            .field("init_order", &self.init_order)
            .finish()
    }
}

#[cfg(test)]
impl ClientModuleLoader {
    /// Test-only constructor that takes pre-built modules (bypasses fn-pointer
    /// factory limitation, enabling stateful mocks with call logs).
    ///
    /// # Errors
    ///
    /// Returns [`ClientModuleLoaderError::DependencyResolution`] on cycle or missing dep.
    pub fn from_modules_for_test(
        modules: Vec<Box<dyn ClientModule>>,
    ) -> Result<Self, ClientModuleLoaderError> {
        let states = vec![ClientModuleState::Loaded; modules.len()];
        let (modules, states, init_order) = resolve_and_reorder(modules, states)?;
        Ok(Self {
            modules,
            states,
            init_order,
        })
    }
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;

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

use crate::{ClientModule, ModuleContext, ProbeResult};

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
// TrackedClientModule
// =============================================================================

/// Internal wrapper tracking a module alongside its lifecycle state.
struct TrackedClientModule {
    module: Box<dyn ClientModule>,
    state: ClientModuleState,
}

// =============================================================================
// ClientModuleLoader
// =============================================================================

/// Loads, resolves dependencies, and manages the lifecycle of client modules.
///
/// Construct via [`ClientModuleLoader::new()`] with a factory map and disabled
/// set. The loader instantiates modules, resolves dependencies via topological
/// sort, and provides `init_all()` / `on_all_loaded()` / `exit_all()` lifecycle
/// methods.
pub struct ClientModuleLoader {
    modules: Vec<TrackedClientModule>,
    /// Module kinds in dependency-resolved init order.
    init_order: Vec<String>,
}

/// Resolve dependencies and reorder modules by topological sort.
///
/// Shared between `new()` and `from_modules_for_test()`.
fn resolve_and_reorder(
    modules: Vec<TrackedClientModule>,
) -> Result<(Vec<TrackedClientModule>, Vec<String>), ClientModuleLoaderError> {
    if modules.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    // Collect module info into owned data (avoids borrow of modules vec).
    let module_info: Vec<(String, Vec<String>, Vec<String>)> = modules
        .iter()
        .map(|t| {
            (
                t.module.kind().to_string(),
                t.module.dependencies().iter().map(|s| (*s).to_string()).collect(),
                t.module
                    .optional_dependencies()
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
        .map(|(i, t)| (t.module.kind(), i))
        .collect();

    let init_order: Vec<String> = resolved.order.clone();

    // Reorder by resolved topological order using Option-swap pattern
    let mut slots: Vec<Option<TrackedClientModule>> =
        modules.into_iter().map(Some).collect();
    let mut sorted = Vec::with_capacity(slots.len());

    for kind in &resolved.order {
        if let Some(&idx) = kind_to_idx.get(kind.as_str())
            && let Some(tracked) = slots[idx].take()
        {
            sorted.push(tracked);
        }
    }

    Ok((sorted, init_order))
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
        let modules: Vec<TrackedClientModule> = factories
            .into_iter()
            .filter(|(kind, _)| !disabled.contains(*kind))
            .map(|(_, factory)| TrackedClientModule {
                module: factory(),
                state: ClientModuleState::Loaded,
            })
            .collect();

        let (modules, init_order) = resolve_and_reorder(modules)?;
        Ok(Self {
            modules,
            init_order,
        })
    }

    /// Initialize all modules in dependency order.
    ///
    /// Uses multi-pass retry for modules that return `ProbeResult::Defer`
    /// (up to 3 passes, matching server behavior). Returns the count of
    /// successfully initialized modules.
    pub fn init_all(&mut self, ctx: &ModuleContext) -> usize {
        let count = self.modules.len();
        let mut initialized = vec![false; count];
        let mut success_count = 0;

        for pass in 0..MAX_DEFER_PASSES {
            let mut any_deferred = false;

            for (i, tracked) in self.modules.iter_mut().enumerate() {
                if initialized[i] {
                    continue;
                }
                if matches!(tracked.state, ClientModuleState::Failed(_)) {
                    continue;
                }

                tracked.state = ClientModuleState::Initializing;

                match tracked.module.init(ctx) {
                    ProbeResult::Success => {
                        tracked.state = ClientModuleState::Running;
                        initialized[i] = true;
                        success_count += 1;
                        tracing::debug!(
                            module = tracked.module.kind(),
                            "client module initialized (pass {pass})"
                        );
                    }
                    ProbeResult::Defer(reason) => {
                        tracked.state = ClientModuleState::Loaded;
                        any_deferred = true;
                        tracing::debug!(
                            module = tracked.module.kind(),
                            reason = %reason,
                            "client module deferred (pass {pass})"
                        );
                    }
                    ProbeResult::Failed(err) => {
                        tracked.state =
                            ClientModuleState::Failed(err.message.clone());
                        initialized[i] = true;
                        tracing::warn!(
                            module = tracked.module.kind(),
                            error = %err.message,
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
        for (i, tracked) in self.modules.iter_mut().enumerate() {
            if !initialized[i]
                && !matches!(tracked.state, ClientModuleState::Failed(_))
            {
                tracked.state = ClientModuleState::Failed(
                    "permanently deferred after max passes".to_string(),
                );
                tracing::warn!(
                    module = tracked.module.kind(),
                    "client module permanently deferred"
                );
            }
        }

        success_count
    }

    /// Call `on_all_loaded()` on each running module.
    pub fn on_all_loaded(&mut self, ctx: &ModuleContext) {
        for tracked in &mut self.modules {
            if tracked.state == ClientModuleState::Running {
                tracked.module.on_all_loaded(ctx);
            }
        }
    }

    /// Shut down all running modules in reverse dependency order.
    ///
    /// Logs failures but never aborts -- all modules get a chance to clean up.
    pub fn exit_all(&mut self) {
        // Build kind -> index map
        let kind_to_idx: HashMap<&str, usize> = self
            .modules
            .iter()
            .enumerate()
            .map(|(i, tracked)| (tracked.module.kind(), i))
            .collect();

        // Exit in reverse init order
        let reverse_order: Vec<usize> = self
            .init_order
            .iter()
            .rev()
            .filter_map(|kind| kind_to_idx.get(kind.as_str()).copied())
            .collect();

        for idx in reverse_order {
            let tracked = &mut self.modules[idx];
            if tracked.state != ClientModuleState::Running {
                continue;
            }

            match tracked.module.exit() {
                Ok(()) => {
                    tracked.state = ClientModuleState::Loaded;
                    tracing::debug!(
                        module = tracked.module.kind(),
                        "client module exited"
                    );
                }
                Err(err) => {
                    tracked.state =
                        ClientModuleState::Failed(err.message.clone());
                    tracing::warn!(
                        module = tracked.module.kind(),
                        error = %err.message,
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
        self.modules
            .iter()
            .filter(|t| t.state == ClientModuleState::Running)
            .count()
    }

    /// Query the state of a module by kind.
    #[must_use]
    pub fn state(&self, kind: &str) -> Option<&ClientModuleState> {
        self.modules
            .iter()
            .find(|t| t.module.kind() == kind)
            .map(|t| &t.state)
    }

    /// Borrow all modules as trait object references.
    #[must_use]
    pub fn as_module_slice(&self) -> Vec<&dyn ClientModule> {
        self.modules.iter().map(|t| t.module.as_ref()).collect()
    }

    /// Mutable iterator over the underlying boxed modules.
    pub fn modules_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut Box<dyn ClientModule>> {
        self.modules.iter_mut().map(|t| &mut t.module)
    }

    /// Consume the loader and return all modules as owned boxes.
    #[must_use]
    pub fn into_modules(self) -> Vec<Box<dyn ClientModule>> {
        self.modules.into_iter().map(|t| t.module).collect()
    }
}

impl fmt::Debug for ClientModuleLoader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientModuleLoader")
            .field("module_count", &self.modules.len())
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
        let tracked: Vec<TrackedClientModule> = modules
            .into_iter()
            .map(|module| TrackedClientModule {
                module,
                state: ClientModuleState::Loaded,
            })
            .collect();

        let (modules, init_order) = resolve_and_reorder(tracked)?;
        Ok(Self {
            modules,
            init_order,
        })
    }
}

#[cfg(test)]
#[path = "loader_tests.rs"]
mod tests;

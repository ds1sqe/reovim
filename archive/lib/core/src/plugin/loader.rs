//! Plugin loader with dependency resolution

use std::{
    any::TypeId,
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use crate::event_bus::EventBus;

use super::{Plugin, PluginContext, PluginId, PluginStateRegistry};

/// Error during plugin loading
#[derive(Debug)]
pub enum PluginError {
    /// A required dependency is missing
    MissingDependency {
        /// Plugin that has the missing dependency
        plugin: PluginId,
        /// Type name of the missing dependency
        dependency_name: &'static str,
    },
    /// Circular dependency detected
    CircularDependency(Vec<PluginId>),
    /// Plugin already loaded
    AlreadyLoaded(PluginId),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingDependency {
                plugin,
                dependency_name,
            } => {
                write!(
                    f,
                    "Plugin '{plugin}' requires dependency '{dependency_name}' which is not loaded"
                )
            }
            Self::CircularDependency(cycle) => {
                write!(f, "Circular dependency detected: ")?;
                for (i, id) in cycle.iter().enumerate() {
                    if i > 0 {
                        write!(f, " -> ")?;
                    }
                    write!(f, "{id}")?;
                }
                Ok(())
            }
            Self::AlreadyLoaded(id) => {
                write!(f, "Plugin '{id}' is already loaded")
            }
        }
    }
}

impl std::error::Error for PluginError {}

/// Plugin loader with dependency resolution
///
/// Collects plugins and loads them in dependency order.
pub struct PluginLoader {
    /// Registered plugins in registration order
    plugins: Vec<Box<dyn Plugin>>,
    /// `TypeId` to plugin index mapping
    type_map: HashMap<TypeId, usize>,
    /// `TypeId` to type name mapping for error messages
    type_names: HashMap<TypeId, &'static str>,
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginLoader {
    /// Create a new empty plugin loader
    #[must_use]
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            type_map: HashMap::new(),
            type_names: HashMap::new(),
        }
    }

    /// Add a plugin to be loaded
    ///
    /// Plugins are added in order, but actual loading respects dependencies.
    pub fn add<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        let type_id = TypeId::of::<P>();
        let idx = self.plugins.len();
        self.type_names.insert(type_id, std::any::type_name::<P>());
        self.plugins.push(Box::new(plugin));
        self.type_map.insert(type_id, idx);
        self
    }

    /// Add multiple plugins using tuple syntax (like Bevy)
    ///
    /// # Example
    /// ```ignore
    /// loader.add_plugins((CorePlugin, EditorPlugin, TelescopePlugin));
    /// ```
    pub fn add_plugins<T: PluginTuple>(&mut self, plugins: T) -> &mut Self {
        plugins.add_to(self);
        self
    }

    /// Load all plugins in dependency order
    ///
    /// # Errors
    /// Returns error if dependencies are missing or circular.
    pub fn load(self, ctx: &mut PluginContext) -> Result<(), PluginError> {
        // Topological sort based on dependencies
        let order = self.resolve_order()?;

        // Build phase
        for idx in &order {
            let plugin = &self.plugins[*idx];
            let plugin_id = plugin.id();
            tracing::debug!(plugin = %plugin_id, "Building plugin");
            ctx.set_current_plugin(plugin_id.to_string());
            plugin.build(ctx);
            ctx.clear_current_plugin();
            ctx.mark_loaded_by_id(self.type_id_for_index(*idx));
        }

        // Finish phase
        for idx in &order {
            let plugin = &self.plugins[*idx];
            plugin.finish(ctx);
        }

        Ok(())
    }

    /// Load all plugins with state registry and event bus integration
    ///
    /// This is the preferred method for loading plugins as it enables:
    /// - Plugin state initialization via `init_state`
    /// - Event bus subscription via `subscribe`
    ///
    /// Returns the loaded plugins for later boot phase execution.
    ///
    /// # Errors
    /// Returns error if dependencies are missing or circular.
    ///
    /// # Panics
    /// Panics if plugin extraction fails (internal invariant violation).
    pub fn load_with_state(
        self,
        ctx: &mut PluginContext,
        state_registry: &Arc<PluginStateRegistry>,
        event_bus: &Arc<EventBus>,
    ) -> Result<Vec<Box<dyn Plugin>>, PluginError> {
        // Topological sort based on dependencies
        let order = self.resolve_order()?;

        // Build phase
        for idx in &order {
            let plugin = &self.plugins[*idx];
            let plugin_id = plugin.id();
            tracing::debug!(plugin = %plugin_id, "Building plugin");
            ctx.set_current_plugin(plugin_id.to_string());
            plugin.build(ctx);
            ctx.clear_current_plugin();
            ctx.mark_loaded_by_id(self.type_id_for_index(*idx));
        }

        // Init state phase - after build, before finish
        for idx in &order {
            let plugin = &self.plugins[*idx];
            tracing::debug!(plugin = %plugin.id(), "Initializing plugin state");
            plugin.init_state(state_registry);
        }

        // Finish phase
        for idx in &order {
            let plugin = &self.plugins[*idx];
            plugin.finish(ctx);
        }

        // Subscribe phase - after finish, allows plugins to subscribe to events
        for idx in &order {
            let plugin = &self.plugins[*idx];
            tracing::debug!(plugin = %plugin.id(), "Subscribing plugin to events");
            plugin.subscribe(event_bus, Arc::clone(state_registry));
        }

        // Return plugins in load order for boot phase
        // Convert Vec to array of Options for O(1) extraction by index
        let mut plugins: Vec<Option<Box<dyn Plugin>>> =
            self.plugins.into_iter().map(Some).collect();
        let mut ordered_plugins = Vec::with_capacity(order.len());

        for idx in order {
            if let Some(plugin) = plugins[idx].take() {
                ordered_plugins.push(plugin);
            }
        }

        Ok(ordered_plugins)
    }

    /// Resolve plugin loading order using topological sort
    fn resolve_order(&self) -> Result<Vec<usize>, PluginError> {
        let n = self.plugins.len();
        let mut in_degree = vec![0usize; n];
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];

        // Build dependency graph
        for (idx, plugin) in self.plugins.iter().enumerate() {
            for dep_type_id in plugin.dependencies() {
                if let Some(&dep_idx) = self.type_map.get(&dep_type_id) {
                    adj[dep_idx].push(idx);
                    in_degree[idx] += 1;
                } else {
                    let dep_name = self
                        .type_names
                        .get(&dep_type_id)
                        .copied()
                        .unwrap_or("unknown");
                    return Err(PluginError::MissingDependency {
                        plugin: plugin.id(),
                        dependency_name: dep_name,
                    });
                }
            }

            // Optional dependencies don't require the plugin to be present
            for dep_type_id in plugin.optional_dependencies() {
                if let Some(&dep_idx) = self.type_map.get(&dep_type_id) {
                    adj[dep_idx].push(idx);
                    in_degree[idx] += 1;
                }
            }
        }

        // Kahn's algorithm for topological sort
        let mut queue: VecDeque<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
        let mut result = Vec::with_capacity(n);

        while let Some(idx) = queue.pop_front() {
            result.push(idx);
            for &next in &adj[idx] {
                in_degree[next] -= 1;
                if in_degree[next] == 0 {
                    queue.push_back(next);
                }
            }
        }

        if result.len() != n {
            // Cycle detected - find and report it
            let cycle = self.find_cycle(&in_degree);
            return Err(PluginError::CircularDependency(cycle));
        }

        Ok(result)
    }

    /// Find `TypeId` for a plugin index
    fn type_id_for_index(&self, idx: usize) -> TypeId {
        for (&type_id, &plugin_idx) in &self.type_map {
            if plugin_idx == idx {
                return type_id;
            }
        }
        unreachable!("Index should always have a TypeId")
    }

    /// Find a cycle in the dependency graph for error reporting
    fn find_cycle(&self, in_degree: &[usize]) -> Vec<PluginId> {
        // Find nodes still with dependencies (part of a cycle)
        let mut cycle_nodes: Vec<PluginId> = in_degree
            .iter()
            .enumerate()
            .filter(|&(_, deg)| *deg > 0)
            .map(|(idx, _)| self.plugins[idx].id())
            .collect();

        // If we can't find specific nodes, just report all remaining
        if cycle_nodes.is_empty() {
            cycle_nodes = self.plugins.iter().map(|p| p.id()).collect();
        }

        cycle_nodes
    }
}

/// Trait for adding multiple plugins at once (tuple support)
pub trait PluginTuple {
    /// Add all plugins in this tuple to the loader
    fn add_to(self, loader: &mut PluginLoader);
}

// Implement for single plugin
impl<P: Plugin> PluginTuple for P {
    fn add_to(self, loader: &mut PluginLoader) {
        loader.add(self);
    }
}

// Implement for tuples of various sizes
impl<P1: Plugin, P2: Plugin> PluginTuple for (P1, P2) {
    fn add_to(self, loader: &mut PluginLoader) {
        loader.add(self.0);
        loader.add(self.1);
    }
}

impl<P1: Plugin, P2: Plugin, P3: Plugin> PluginTuple for (P1, P2, P3) {
    fn add_to(self, loader: &mut PluginLoader) {
        loader.add(self.0);
        loader.add(self.1);
        loader.add(self.2);
    }
}

impl<P1: Plugin, P2: Plugin, P3: Plugin, P4: Plugin> PluginTuple for (P1, P2, P3, P4) {
    fn add_to(self, loader: &mut PluginLoader) {
        loader.add(self.0);
        loader.add(self.1);
        loader.add(self.2);
        loader.add(self.3);
    }
}

impl<P1: Plugin, P2: Plugin, P3: Plugin, P4: Plugin, P5: Plugin> PluginTuple
    for (P1, P2, P3, P4, P5)
{
    fn add_to(self, loader: &mut PluginLoader) {
        loader.add(self.0);
        loader.add(self.1);
        loader.add(self.2);
        loader.add(self.3);
        loader.add(self.4);
    }
}

impl<P1: Plugin, P2: Plugin, P3: Plugin, P4: Plugin, P5: Plugin, P6: Plugin> PluginTuple
    for (P1, P2, P3, P4, P5, P6)
{
    fn add_to(self, loader: &mut PluginLoader) {
        loader.add(self.0);
        loader.add(self.1);
        loader.add(self.2);
        loader.add(self.3);
        loader.add(self.4);
        loader.add(self.5);
    }
}

impl<P1: Plugin, P2: Plugin, P3: Plugin, P4: Plugin, P5: Plugin, P6: Plugin, P7: Plugin> PluginTuple
    for (P1, P2, P3, P4, P5, P6, P7)
{
    fn add_to(self, loader: &mut PluginLoader) {
        loader.add(self.0);
        loader.add(self.1);
        loader.add(self.2);
        loader.add(self.3);
        loader.add(self.4);
        loader.add(self.5);
        loader.add(self.6);
    }
}

impl<P1: Plugin, P2: Plugin, P3: Plugin, P4: Plugin, P5: Plugin, P6: Plugin, P7: Plugin, P8: Plugin>
    PluginTuple for (P1, P2, P3, P4, P5, P6, P7, P8)
{
    fn add_to(self, loader: &mut PluginLoader) {
        loader.add(self.0);
        loader.add(self.1);
        loader.add(self.2);
        loader.add(self.3);
        loader.add(self.4);
        loader.add(self.5);
        loader.add(self.6);
        loader.add(self.7);
    }
}

impl<
    P1: Plugin,
    P2: Plugin,
    P3: Plugin,
    P4: Plugin,
    P5: Plugin,
    P6: Plugin,
    P7: Plugin,
    P8: Plugin,
    P9: Plugin,
> PluginTuple for (P1, P2, P3, P4, P5, P6, P7, P8, P9)
{
    fn add_to(self, loader: &mut PluginLoader) {
        loader.add(self.0);
        loader.add(self.1);
        loader.add(self.2);
        loader.add(self.3);
        loader.add(self.4);
        loader.add(self.5);
        loader.add(self.6);
        loader.add(self.7);
        loader.add(self.8);
    }
}

impl<
    P1: Plugin,
    P2: Plugin,
    P3: Plugin,
    P4: Plugin,
    P5: Plugin,
    P6: Plugin,
    P7: Plugin,
    P8: Plugin,
    P9: Plugin,
    P10: Plugin,
> PluginTuple for (P1, P2, P3, P4, P5, P6, P7, P8, P9, P10)
{
    fn add_to(self, loader: &mut PluginLoader) {
        loader.add(self.0);
        loader.add(self.1);
        loader.add(self.2);
        loader.add(self.3);
        loader.add(self.4);
        loader.add(self.5);
        loader.add(self.6);
        loader.add(self.7);
        loader.add(self.8);
        loader.add(self.9);
    }
}

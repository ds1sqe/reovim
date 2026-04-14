use {reovim_driver_text_session::SessionRuntime, reovim_kernel::api::v1::ServiceRegistry};

use crate::{PickerAction, PickerContext, PickerItem, PreviewContent};

/// Trait for pluggable data sources in the fuzzy finder.
///
/// Pickers define WHAT items are available and what happens when selected.
/// The fuzzy matching engine handles HOW they are filtered and ranked.
///
/// # Sync Design
///
/// `items()` is synchronous. For expensive data sources (file walk, grep),
/// the caller should spawn a background task that feeds items into the
/// engine's `Injector` directly, bypassing `items()`.
///
/// # Service Access
///
/// Both `items()` and `preview()` receive a reference to [`ServiceRegistry`],
/// allowing pickers to pull domain-specific data (buffers, commands, history)
/// without coupling the driver layer to specific data sources.
pub trait Picker: Send + Sync {
    /// Unique name for this picker (e.g. "files", "buffers").
    fn name(&self) -> &'static str;

    /// Human-readable title for the picker UI.
    fn title(&self) -> &'static str;

    /// Prompt prefix (shown before the query input).
    fn prompt(&self) -> &'static str {
        "> "
    }

    /// Produce items to search through.
    ///
    /// For static sources (buffers, commands), returns all items.
    /// For dynamic sources (grep), returns results for current query.
    /// Use `services` to access domain data (e.g. buffer list, command registry).
    fn items(&self, ctx: &PickerContext, services: &ServiceRegistry) -> Vec<PickerItem>;

    /// Resolve the action when a user selects an item.
    fn on_select(&self, item: &PickerItem) -> PickerAction;

    /// Optional: provide preview content for the highlighted item.
    ///
    /// Use `services` to access domain data (e.g. VFS for file reading).
    fn preview(&self, _item: &PickerItem, _services: &ServiceRegistry) -> Option<PreviewContent> {
        None
    }

    /// Whether this picker supports client-side fuzzy filtering.
    ///
    /// If true (default), items are fetched once and nucleo handles filtering.
    /// If false, the picker needs re-fetching on each query change (e.g. live grep).
    fn is_static(&self) -> bool {
        true
    }

    /// Execute the action after item selection.
    ///
    /// Each picker implements this to handle its own action dispatch
    /// (e.g. open file, switch buffer, run command). The microscope module
    /// calls this generically without knowing the specific action type.
    ///
    /// Default is a no-op for pickers that don't need custom execution.
    fn execute(&self, _action: PickerAction, _runtime: &mut SessionRuntime<'_>) {}
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;

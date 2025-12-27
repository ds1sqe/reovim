//! Binding filtering logic for which-key

#![allow(dead_code)] // Will be used once KeyMap accessor is available

use reovim_core::{
    bind::{KeyMap, KeymapScope},
    command::registry::CommandRegistry,
    keystroke::KeySequence,
};

use crate::state::BindingEntry;

/// Filter bindings that match the given prefix
///
/// Returns bindings where the key sequence starts with `pending` prefix,
/// extracting the suffix (keys after prefix) and description.
pub fn filter_bindings(
    keymap: &KeyMap,
    pending: &KeySequence,
    scope: &KeymapScope,
    registry: &CommandRegistry,
) -> Vec<BindingEntry> {
    let mut entries = Vec::new();

    // Get the keymap for the current scope
    let scope_map = keymap.get_scope(scope);

    tracing::debug!(
        "filter_bindings: scope={:?}, pending={:?}, scope_map_len={}",
        scope,
        pending,
        scope_map.len()
    );

    // Log what bindings exist in this scope
    for (keys, _) in scope_map.iter().take(10) {
        tracing::debug!("filter_bindings: available binding key={:?}", keys);
    }

    for (keys, inner) in scope_map {
        // Check if this binding starts with our pending prefix
        if keys.starts_with(pending) && keys != pending {
            // Extract the suffix (keys after the prefix)
            let suffix_keys: Vec<_> = keys.0.iter().skip(pending.0.len()).cloned().collect();
            let suffix = KeySequence(suffix_keys);

            // Get description from hint or command
            let description = inner.get_description(registry);

            entries.push(BindingEntry {
                suffix,
                description,
                group: inner.group,
            });
        }
    }

    // Sort by group, then by key
    entries.sort_by(|a, b| {
        match (a.group, b.group) {
            (Some(ga), Some(gb)) => ga.cmp(gb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| {
            // Compare key sequences by their string representation
            let a_str = a
                .suffix
                .render(reovim_core::keystroke::KeyNotationFormat::Vim);
            let b_str = b
                .suffix
                .render(reovim_core::keystroke::KeyNotationFormat::Vim);
            a_str.cmp(&b_str)
        })
    });

    entries
}

//! Filtering logic and background saturator task for which-key.
//!
//! This module provides:
//! - `filter_bindings()` - Extracts suffixes and descriptions from bindings
//! - `spawn_saturator()` - Background task that updates the cache asynchronously
//!
//! # Architecture
//!
//! The saturator pattern decouples keymap queries from the render thread:
//! ```text
//! Input Handler → FilterRequest → Saturator Task → ArcSwap Cache → Render
//! ```
//!
//! The saturator task runs in the background, processing requests and updating
//! the cache atomically. The render thread reads lock-free via `cache.load()`.

use std::sync::Arc;

use arc_swap::ArcSwap;
use reovim_driver_input::{KeySequence, KeymapQuery};
use reovim_kernel::api::v1::CommandId;
use tokio::sync::mpsc;

use crate::state::{BindingEntry, FilterRequest, SaturatorHandle, WhichKeyCache};

/// Trait for looking up command descriptions.
///
/// This abstraction allows the filter logic to be testable without
/// depending on the full command registry.
pub trait CommandDescriptionProvider: Send + Sync {
    /// Get the description for a command.
    ///
    /// Returns the human-readable description, or `None` if the command
    /// is not found.
    fn description(&self, id: &CommandId) -> Option<&str>;
}

/// Filter bindings by prefix and enrich with descriptions.
///
/// Given a list of (full_sequence, command_id) pairs from the keymap,
/// this function:
/// 1. Extracts the suffix (keys after the prefix)
/// 2. Looks up the command description
/// 3. Returns `BindingEntry` items ready for display
///
/// # Arguments
///
/// * `bindings` - Raw bindings from `KeymapQuery::bindings_with_prefix()`
/// * `prefix` - The prefix that was used to query (for suffix extraction)
/// * `descriptions` - Provider for command descriptions
///
/// # Returns
///
/// A vector of `BindingEntry` items sorted by suffix for consistent display.
pub fn filter_bindings<D: CommandDescriptionProvider>(
    bindings: Vec<(KeySequence, CommandId)>,
    prefix: &KeySequence,
    descriptions: &D,
) -> Vec<BindingEntry> {
    let prefix_len = prefix.len();

    let mut entries: Vec<BindingEntry> = bindings
        .into_iter()
        .map(|(full_seq, cmd_id)| {
            // Extract suffix (keys after prefix)
            let suffix = if full_seq.len() > prefix_len {
                KeySequence::from_keys(&full_seq.as_slice()[prefix_len..])
            } else {
                KeySequence::new()
            };

            // Get description, falling back to command name
            let description = descriptions
                .description(&cmd_id)
                .map(String::from)
                .unwrap_or_else(|| cmd_id.name().to_string());

            BindingEntry::new(suffix, description)
        })
        .collect();

    // Sort by suffix for consistent display order
    entries.sort_by(|a, b| {
        // Compare by string representation of suffix
        format!("{}", a.suffix).cmp(&format!("{}", b.suffix))
    });

    entries
}

/// Spawn the saturator background task.
///
/// The saturator receives `FilterRequest`s through a channel and updates
/// the `WhichKeyCache` atomically. This decouples keymap queries from
/// the render thread.
///
/// # Arguments
///
/// * `cache` - The shared cache to update
/// * `keymap` - Keymap query provider
/// * `descriptions` - Command description provider
///
/// # Channel Design
///
/// The channel has a buffer of 1. If a new request arrives while
/// processing, the old request is dropped (intentional: we only
/// care about the latest state).
///
/// # Returns
///
/// A `SaturatorHandle` containing the sender and task handle.
pub fn spawn_saturator<K, D>(
    cache: Arc<ArcSwap<WhichKeyCache>>,
    keymap: Arc<K>,
    descriptions: Arc<D>,
) -> SaturatorHandle
where
    K: KeymapQuery + 'static,
    D: CommandDescriptionProvider + 'static,
{
    // Buffer of 1: drop stale requests (only latest matters)
    let (tx, mut rx) = mpsc::channel::<FilterRequest>(1);

    let task = tokio::spawn(async move {
        tracing::debug!("which-key: saturator task started");

        while let Some(req) = rx.recv().await {
            tracing::trace!(
                "which-key: processing filter request for mode={}, prefix_len={}",
                req.mode.name(),
                req.prefix.len()
            );

            // Query bindings from keymap
            let bindings = keymap.bindings_with_prefix(&req.mode, &req.prefix);

            // Filter and enrich with descriptions
            let entries = filter_bindings(bindings, &req.prefix, descriptions.as_ref());

            // Update cache atomically
            let mut new_cache = (*cache.load_full()).clone();
            new_cache.bindings = entries;
            cache.store(Arc::new(new_cache));

            tracing::trace!(
                "which-key: cache updated with {} bindings",
                cache.load().bindings.len()
            );
        }

        tracing::debug!("which-key: saturator task shutting down");
    });

    SaturatorHandle::new(tx, task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::FilterRequest;
    use reovim_kernel::api::v1::ModuleId;
    use reovim_kernel::api::v1::ModeId;
    use std::collections::HashMap;

    // Mock description provider for testing
    struct MockDescriptions {
        descriptions: HashMap<CommandId, String>,
    }

    impl MockDescriptions {
        fn new() -> Self {
            Self {
                descriptions: HashMap::new(),
            }
        }

        fn add(&mut self, id: CommandId, desc: &str) {
            self.descriptions.insert(id, desc.to_string());
        }
    }

    impl CommandDescriptionProvider for MockDescriptions {
        fn description(&self, id: &CommandId) -> Option<&str> {
            self.descriptions.get(id).map(String::as_str)
        }
    }

    // Mock keymap for testing
    struct MockKeymap {
        bindings: Vec<(ModeId, KeySequence, CommandId)>,
    }

    impl MockKeymap {
        fn new() -> Self {
            Self { bindings: vec![] }
        }

        fn add(&mut self, mode: ModeId, keys: KeySequence, cmd: CommandId) {
            self.bindings.push((mode, keys, cmd));
        }
    }

    impl KeymapQuery for MockKeymap {
        fn query(
            &self,
            _mode: &ModeId,
            _keys: &KeySequence,
        ) -> reovim_driver_input::KeyLookupState {
            reovim_driver_input::KeyLookupState::NotFound
        }

        fn bindings_with_prefix(
            &self,
            mode: &ModeId,
            prefix: &KeySequence,
        ) -> Vec<(KeySequence, CommandId)> {
            self.bindings
                .iter()
                .filter(|(m, k, _)| m == mode && k.starts_with(prefix) && k != prefix)
                .map(|(_, k, c)| (k.clone(), c.clone()))
                .collect()
        }
    }

    fn test_module() -> ModuleId {
        ModuleId::new("test")
    }

    fn test_mode() -> ModeId {
        ModeId::new(test_module(), "normal")
    }

    fn test_cmd(name: &'static str) -> CommandId {
        CommandId::new(test_module(), name)
    }

    #[test]
    fn test_filter_extracts_suffix() {
        let mut descriptions = MockDescriptions::new();
        descriptions.add(test_cmd("goto-top"), "Go to top of file");

        let prefix = KeySequence::parse("g").unwrap();
        let full = KeySequence::parse("gg").unwrap();

        let bindings = vec![(full, test_cmd("goto-top"))];
        let entries = filter_bindings(bindings, &prefix, &descriptions);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].suffix.len(), 1); // Just 'g' as suffix
        assert_eq!(entries[0].description, "Go to top of file");
    }

    #[test]
    fn test_filter_gets_descriptions() {
        let mut descriptions = MockDescriptions::new();
        descriptions.add(test_cmd("goto-def"), "Go to definition");

        let prefix = KeySequence::parse("g").unwrap();
        let full = KeySequence::parse("gd").unwrap();

        let bindings = vec![(full, test_cmd("goto-def"))];
        let entries = filter_bindings(bindings, &prefix, &descriptions);

        assert_eq!(entries[0].description, "Go to definition");
    }

    #[test]
    fn test_filter_fallback_description() {
        let descriptions = MockDescriptions::new(); // Empty - no descriptions

        let prefix = KeySequence::parse("g").unwrap();
        let full = KeySequence::parse("gx").unwrap();

        let bindings = vec![(full, test_cmd("unknown-cmd"))];
        let entries = filter_bindings(bindings, &prefix, &descriptions);

        // Falls back to command name
        assert_eq!(entries[0].description, "unknown-cmd");
    }

    #[test]
    fn test_filter_empty_prefix() {
        let mut descriptions = MockDescriptions::new();
        descriptions.add(test_cmd("down"), "Move down");
        descriptions.add(test_cmd("up"), "Move up");

        let prefix = KeySequence::new(); // Empty prefix
        let j = KeySequence::parse("j").unwrap();
        let k = KeySequence::parse("k").unwrap();

        let bindings = vec![(j, test_cmd("down")), (k, test_cmd("up"))];
        let entries = filter_bindings(bindings, &prefix, &descriptions);

        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_filter_no_matches() {
        let descriptions = MockDescriptions::new();
        let prefix = KeySequence::parse("z").unwrap();

        let bindings = vec![]; // No bindings match
        let entries = filter_bindings(bindings, &prefix, &descriptions);

        assert!(entries.is_empty());
    }

    #[test]
    fn test_filter_sorts_by_suffix() {
        let mut descriptions = MockDescriptions::new();
        descriptions.add(test_cmd("cmd-b"), "B command");
        descriptions.add(test_cmd("cmd-a"), "A command");
        descriptions.add(test_cmd("cmd-c"), "C command");

        let prefix = KeySequence::parse("g").unwrap();

        let bindings = vec![
            (KeySequence::parse("gc").unwrap(), test_cmd("cmd-c")),
            (KeySequence::parse("ga").unwrap(), test_cmd("cmd-a")),
            (KeySequence::parse("gb").unwrap(), test_cmd("cmd-b")),
        ];
        let entries = filter_bindings(bindings, &prefix, &descriptions);

        // Should be sorted by suffix: a, b, c
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].description, "A command");
        assert_eq!(entries[1].description, "B command");
        assert_eq!(entries[2].description, "C command");
    }

    #[tokio::test]
    async fn test_saturator_updates_cache() {
        let cache = Arc::new(ArcSwap::new(Arc::new(WhichKeyCache::new())));

        let mut keymap = MockKeymap::new();
        let mode = test_mode();
        keymap.add(
            mode.clone(),
            KeySequence::parse("gg").unwrap(),
            test_cmd("goto-top"),
        );

        let mut descriptions = MockDescriptions::new();
        descriptions.add(test_cmd("goto-top"), "Go to top");

        let handle = spawn_saturator(
            Arc::clone(&cache),
            Arc::new(keymap),
            Arc::new(descriptions),
        );

        // Send a filter request
        let req = FilterRequest::new(mode, KeySequence::parse("g").unwrap());
        handle.tx.send(req).await.unwrap();

        // Give the saturator time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Check cache was updated
        let loaded = cache.load();
        assert_eq!(loaded.bindings.len(), 1);
        assert_eq!(loaded.bindings[0].description, "Go to top");
    }

    #[tokio::test]
    async fn test_saturator_handles_channel_close() {
        let cache = Arc::new(ArcSwap::new(Arc::new(WhichKeyCache::new())));
        let keymap = Arc::new(MockKeymap::new());
        let descriptions = Arc::new(MockDescriptions::new());

        let handle = spawn_saturator(Arc::clone(&cache), keymap, descriptions);

        // Drop the sender to close the channel
        drop(handle.tx);

        // Task should complete gracefully
        let result = tokio::time::timeout(
            tokio::time::Duration::from_millis(100),
            handle.task,
        )
        .await;

        assert!(result.is_ok(), "Task should complete when channel closes");
    }

    #[tokio::test]
    async fn test_saturator_handle_is_alive() {
        // This test verifies the SaturatorHandle::is_alive() method
        let (tx, _rx) = mpsc::channel::<FilterRequest>(1);
        let task = tokio::task::spawn(async {
            // Sleep briefly so we can check is_alive
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        });
        let handle = SaturatorHandle::new(tx, task);

        // Task should be alive initially
        assert!(handle.is_alive());

        // Wait for task to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Task should be finished now
        assert!(!handle.is_alive());
    }
}

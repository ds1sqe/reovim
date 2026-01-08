//! Background task for filtering keybindings

use std::sync::Arc;

use {
    arc_swap::ArcSwap,
    reovim_core::{
        bind::{KeyMap, KeymapScope},
        command::registry::CommandRegistry,
        event::RuntimeEvent,
        keystroke::KeySequence,
    },
    tokio::sync::mpsc,
};

use crate::{filter::filter_bindings, state::WhichKeyCache};

/// Request sent to saturator
#[derive(Debug)]
pub struct WhichKeyRequest {
    /// Current pending keys
    pub pending_keys: KeySequence,
    /// Current mode scope for filtering
    pub scope: KeymapScope,
}

/// Handle to communicate with saturator task
pub struct WhichKeySaturatorHandle {
    pub tx: mpsc::Sender<WhichKeyRequest>,
}

impl WhichKeySaturatorHandle {
    /// Non-blocking request (uses `try_send` - drops if busy)
    #[allow(dead_code)] // Will be used once filtering is wired up
    pub fn request_filter(&self, request: WhichKeyRequest) {
        let _ = self.tx.try_send(request);
    }
}

/// Spawn background saturator task
pub fn spawn_whichkey_saturator(
    cache: Arc<ArcSwap<WhichKeyCache>>,
    event_tx: mpsc::Sender<RuntimeEvent>,
    keymap: Arc<KeyMap>,
    command_registry: Arc<CommandRegistry>,
) -> WhichKeySaturatorHandle {
    let (tx, mut rx) = mpsc::channel::<WhichKeyRequest>(1); // Buffer=1

    tokio::spawn(async move {
        tracing::info!("WhichKey saturator task started");
        while let Some(request) = rx.recv().await {
            tracing::info!(
                "WhichKey saturator: received request, pending_keys={:?}, scope={:?}",
                request.pending_keys,
                request.scope
            );

            // Filter bindings (potentially slow with many bindings)
            let bindings =
                filter_bindings(&keymap, &request.pending_keys, &request.scope, &command_registry);
            tracing::info!("WhichKey saturator: filtered {} bindings", bindings.len());

            // Update only the bindings field, preserving other state
            // (initial_prefix, current_filter, visible are managed by event handlers)
            let current = cache.load_full();
            let mut new_cache = (*current).clone();
            new_cache.bindings = bindings;
            cache.store(Arc::new(new_cache));

            // Signal re-render
            tracing::info!("WhichKey saturator: sending RenderSignal");
            let _ = event_tx.send(RuntimeEvent::render_signal()).await;
        }
        tracing::warn!("WhichKey saturator task ended");
    });

    WhichKeySaturatorHandle { tx }
}

//! `TextKeyDispatchProvider` implementation wrapping `ResolverRegistry`.
//!
//! Moves the full key dispatch pipeline from the server layer into the
//! driver layer. The server calls `dispatch_key` and gets back
//! `(handled, StateChanges)` — it never sees `ResolveResult`,
//! `ModeTransition`, or resolver internals.

use std::sync::Arc;

use {
    reovim_driver_text_session::{
        ExtensionMap, PopResult, SessionRuntime, TextKeyDispatchProvider,
        api::{ChangeTracker, CommandExecutor, ModeApi, StateChanges},
    },
    reovim_input_codec::KeyEvent as CodecKeyEvent,
    reovim_kernel::api::v1::ModeId,
    reovim_subsys_command_types::{ArgValue, CommandContext, RuntimeSignal},
    reovim_subsys_input::KeyEvent as LegacyKeyEvent,
};

use crate::{KeySequence, KeymapQuery, ModeState, ModeTransition, PendingBindings, ResolveResult};

/// Implementation of `TextKeyDispatchProvider` wrapping `ResolverRegistry`.
///
/// Created by the server at session startup. Injects the resolver and
/// keymap registries into the domain driver.
pub struct ResolverDispatchProvider {
    resolver_registry: crate::ResolverRegistry,
    keymap: Arc<dyn KeymapQuery>,
}

impl ResolverDispatchProvider {
    /// Create a new dispatch provider.
    ///
    /// # Arguments
    ///
    /// * `resolver_registry` — owned `ResolverRegistry` (moved from `SessionState`)
    /// * `keymap` — keymap query (server's `KeymapRegistry` behind this trait)
    #[must_use]
    pub fn new(resolver_registry: crate::ResolverRegistry, keymap: Arc<dyn KeymapQuery>) -> Self {
        Self {
            resolver_registry,
            keymap,
        }
    }

    /// Access the resolver registry (for server-side module registration).
    #[must_use]
    pub const fn resolver_registry(&self) -> &crate::ResolverRegistry {
        &self.resolver_registry
    }

    /// Mutable access to the resolver registry (for module registration).
    pub const fn resolver_registry_mut(&mut self) -> &mut crate::ResolverRegistry {
        &mut self.resolver_registry
    }

    /// Handle a `ResolveResult` — execute commands, apply mode transitions,
    /// process inject keys, manage pending bindings.
    ///
    /// This is the logic that was in `InputServiceImpl::handle_resolve_result`
    /// and `SessionState::resolve_key_for_client` (`PendingBindings` handling).
    fn handle_result(
        &self,
        result: ResolveResult,
        _key: &CodecKeyEvent,
        runtime: &mut SessionRuntime<'_>,
        shared_ext: &mut ExtensionMap,
        client_ext: &mut ExtensionMap,
        executor: &dyn CommandExecutor,
    ) -> bool {
        match result {
            ResolveResult::Execute(cmd_id, ctx) => {
                let cmd_ctx = resolve_to_command_context(&ctx);

                // Execute command via the injected executor
                if let Some(handle) = executor.get_handle(&cmd_id) {
                    let _cmd_result = handle.execute(runtime, &cmd_ctx);
                }

                // on_command_complete for pending operators (e.g., delete after motion)
                self.handle_command_complete_chain(runtime, shared_ext, client_ext, executor);

                true
            }

            ResolveResult::InsertChar { .. } => {
                // InsertChar should be handled by resolvers via SessionApi.
                // Defensive no-op — the character is dropped.
                tracing::error!(
                    "InsertChar reached dispatch — resolvers must handle insertion via SessionApi"
                );
                true
            }

            ResolveResult::ModeTransition(transition) => {
                self.apply_mode_transition(transition, runtime, shared_ext, client_ext, executor);
                true
            }

            ResolveResult::Completed | ResolveResult::Pending => true,
            ResolveResult::NotHandled => false,

            ResolveResult::InjectKeys {
                keys,
                exit_macro_playback: _,
            } => {
                // Macro playback — recursively resolve injected keys.
                for injected_key in &keys {
                    let mode = runtime.current_mode().clone();
                    let mut mode_state = ModeState::new(mode.clone());

                    let resolve_result = self.resolver_registry.resolve_with_session(
                        &mode,
                        injected_key,
                        &mut mode_state,
                        &*self.keymap,
                        runtime,
                        shared_ext,
                        client_ext,
                    );

                    if let Some(result) = resolve_result {
                        // Prevent infinite recursion: don't recurse on nested InjectKeys
                        if !matches!(result, ResolveResult::InjectKeys { .. }) {
                            self.handle_result(
                                result,
                                injected_key,
                                runtime,
                                shared_ext,
                                client_ext,
                                executor,
                            );
                        }
                    }
                }
                true
            }
        }
    }

    /// Apply a mode transition to the runtime's mode stack.
    fn apply_mode_transition(
        &self,
        transition: ModeTransition,
        runtime: &mut SessionRuntime<'_>,
        shared_ext: &mut ExtensionMap,
        client_ext: &mut ExtensionMap,
        executor: &dyn CommandExecutor,
    ) {
        match &transition {
            ModeTransition::Push { mode, context } => {
                tracing::debug!(?mode, "Pushing mode");
                runtime.push_mode(mode.clone(), context.clone());
            }
            ModeTransition::Pop { result: _ } => {
                let _ = runtime.pop_mode(None);
            }
            ModeTransition::Set { mode, context } => {
                tracing::debug!(?mode, "Setting mode");
                runtime.set_mode(mode.clone(), context.clone());
            }
        }

        // Handle pop result (execute command from operator-pending)
        if let ModeTransition::Pop {
            result: Some(pop_result),
        } = transition
        {
            Self::handle_pop_result(pop_result, runtime, executor);
        }

        // Deferred motion completion: pending operators may need on_command_complete
        self.handle_command_complete_chain(runtime, shared_ext, client_ext, executor);
    }

    /// Handle a `PopResult` — execute the command or inject keys.
    fn handle_pop_result(
        result: PopResult,
        runtime: &mut SessionRuntime<'_>,
        executor: &dyn CommandExecutor,
    ) {
        match result {
            PopResult::ExecuteCommand { command, args } => {
                let mut cmd_ctx = CommandContext::new();
                for (key, value) in args {
                    cmd_ctx.set(&key, value);
                }

                if let Some(handle) = executor.get_handle(&command) {
                    let _cmd_result = handle.execute(runtime, &cmd_ctx);
                }
            }
            PopResult::Cancelled | PopResult::Data { .. } => {
                // No action needed
            }
        }
    }

    /// Chain of `on_command_complete` calls for nested operator-pending modes.
    fn handle_command_complete_chain(
        &self,
        runtime: &mut SessionRuntime<'_>,
        shared_ext: &mut ExtensionMap,
        client_ext: &mut ExtensionMap,
        executor: &dyn CommandExecutor,
    ) {
        let mode = runtime.current_mode().clone();
        if let Some(resolver) = self.resolver_registry.get(&mode)
            && let Some(complete_transition) =
                resolver.on_command_complete(runtime, shared_ext, client_ext)
        {
            // Apply the transition
            match &complete_transition {
                ModeTransition::Pop { .. } => {
                    let _ = runtime.pop_mode(None);
                }
                ModeTransition::Push { mode, context } => {
                    runtime.push_mode(mode.clone(), context.clone());
                }
                ModeTransition::Set { mode, context } => {
                    runtime.set_mode(mode.clone(), context.clone());
                }
            }

            // Pop with result → execute and recurse
            if let ModeTransition::Pop {
                result: Some(nested_result),
            } = complete_transition
            {
                Self::handle_pop_result(nested_result, runtime, executor);

                // Recurse for further completions
                self.handle_command_complete_chain(runtime, shared_ext, client_ext, executor);
            }
        }
    }

    /// Populate `PendingBindings` in client extensions for whichkey hints.
    fn populate_pending_bindings(
        &self,
        result: &ResolveResult,
        key: &CodecKeyEvent,
        mode: &ModeId,
        client_ext: &mut ExtensionMap,
    ) {
        match result {
            ResolveResult::Pending => {
                let pending = self.resolver_registry.pending_keys_for(mode);
                if !pending.is_empty() {
                    let mut continuations = self.keymap.bindings_with_prefix(mode, &pending);
                    if let Some(resolver) = self.resolver_registry.get(mode)
                        && let Some(parent) = resolver.inherits_from()
                    {
                        let parent_bindings = self.keymap.bindings_with_prefix(parent, &pending);
                        continuations.extend(parent_bindings);
                    }
                    let pb = client_ext.get_or_insert::<PendingBindings>();
                    pb.pending_keys = pending;
                    pb.mode = mode.clone();
                    pb.continuations = continuations;
                }
            }
            ResolveResult::ModeTransition(ModeTransition::Push {
                mode: target_mode, ..
            }) => {
                let trigger_key = KeySequence::from_keys(&[*key]);
                let empty = KeySequence::new();
                let mut continuations = self.keymap.bindings_with_prefix(target_mode, &empty);
                if let Some(resolver) = self.resolver_registry.get(target_mode)
                    && let Some(parent) = resolver.inherits_from()
                {
                    let parent_bindings = self.keymap.bindings_with_prefix(parent, &empty);
                    continuations.extend(parent_bindings);
                }
                if !continuations.is_empty() {
                    let pb = client_ext.get_or_insert::<PendingBindings>();
                    pb.mode_prefix = trigger_key;
                    pb.pending_keys = KeySequence::new();
                    pb.mode = target_mode.clone();
                    pb.continuations = continuations;
                }
            }
            _ => {
                // Non-pending, non-push: clear pending bindings
                if let Some(pb) = client_ext.get_mut::<PendingBindings>() {
                    pb.clear();
                }
            }
        }
    }
}

impl TextKeyDispatchProvider for ResolverDispatchProvider {
    fn dispatch_key(
        &self,
        runtime: &mut SessionRuntime<'_>,
        key: &LegacyKeyEvent,
        shared_ext: &mut ExtensionMap,
        client_ext: &mut ExtensionMap,
        executor: &dyn CommandExecutor,
    ) -> (bool, StateChanges) {
        let mode = runtime.current_mode().clone();
        let mut mode_state = ModeState::new(mode.clone());
        let key: CodecKeyEvent = key.clone().into();

        // Step 1: Resolve the key
        let resolve_result = self.resolver_registry.resolve_with_session(
            &mode,
            &key,
            &mut mode_state,
            &*self.keymap,
            runtime,
            shared_ext,
            client_ext,
        );

        let Some(result) = resolve_result else {
            // No resolver for this mode
            return (false, StateChanges::new());
        };

        // Step 2: Populate PendingBindings (before handling, to capture state)
        self.populate_pending_bindings(&result, &key, &mode, client_ext);

        // Step 3: Handle the resolve result (commands, transitions, inject keys)
        // All state changes accumulate in the runtime's ChangeTracker.
        let handled = self.handle_result(result, &key, runtime, shared_ext, client_ext, executor);

        // Step 4: Take accumulated changes from the runtime
        let mut changes = ChangeTracker::take_changes(runtime);

        // Step 5: Translate runtime signals to change flags
        for signal in runtime.take_signals() {
            match signal {
                RuntimeSignal::Quit => {
                    changes.record_quit_requested();
                }
            }
        }

        (handled, changes)
    }
}

/// Convert `ResolveContext` to `CommandContext`.
///
/// Moved from `InputServiceImpl::resolve_to_command_context` in the server.
fn resolve_to_command_context(ctx: &crate::ResolveContext) -> CommandContext {
    let mut cmd_ctx = CommandContext::new();
    if let Some(count) = ctx.count {
        cmd_ctx.set("count", ArgValue::Count(count));
    }
    if let Some(reg) = ctx.register {
        cmd_ctx.set("register", ArgValue::Register(reg));
    }
    for (key, value) in &ctx.metadata {
        let converted = match value {
            crate::ArgValue::Bool(b) => Some(ArgValue::Bool(*b)),
            crate::ArgValue::String(s) => Some(ArgValue::String(s.clone())),
            crate::ArgValue::Char(c) => Some(ArgValue::Char(*c)),
            crate::ArgValue::Int(n) => usize::try_from(*n).ok().map(ArgValue::Count),
            crate::ArgValue::Uint(n) => usize::try_from(*n).ok().map(ArgValue::Count),
            crate::ArgValue::Position(p) => Some(ArgValue::Position(p.line, p.column)),
            crate::ArgValue::Float(_) | crate::ArgValue::Range { .. } => None,
        };
        if let Some(arg_value) = converted {
            cmd_ctx.set(key, arg_value);
        }
    }
    cmd_ctx
}

#[cfg(test)]
#[path = "dispatch_provider_tests.rs"]
mod tests;

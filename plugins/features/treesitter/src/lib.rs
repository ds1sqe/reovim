//! Treesitter plugin for reovim
//!
//! Provides syntax highlighting, code folding, and semantic text objects
//! via tree-sitter parsing. Language support is provided by separate
//! language plugins that register with this infrastructure plugin.

use std::sync::Arc;

use {
    reovim_core::{
        event::RuntimeEvent,
        event_bus::{EventBus, EventResult, FileOpened},
        plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    },
    tokio::sync::mpsc,
};

pub mod command;
pub mod context;
pub mod edit;
pub mod events;
pub mod factory;
pub mod highlighter;
pub mod injection;
pub mod manager;
pub mod parser;
pub mod queries;
pub mod registry;
pub mod state;
pub mod syntax;
pub mod text_objects;
pub mod theme;

pub use {
    command::{JumpToNextScope, JumpToParentScope, JumpToPrevScope},
    context::TreesitterContextProvider,
    edit::BufferEdit,
    events::{
        HighlightsReady, ParseCompleted, ParseRequest, RegisterLanguage, TreesitterFoldRanges,
    },
    highlighter::Highlighter,
    injection::{InjectionDetector, InjectionLayer, InjectionManager, InjectionRegion},
    manager::TreesitterManager,
    parser::BufferParser,
    queries::{QueryCache, QueryType},
    registry::{LanguageRegistry, LanguageSupport, RegisteredLanguage},
    state::SharedTreesitterManager,
    theme::TreesitterTheme,
};

/// Re-export tree-sitter types for language plugins
pub use tree_sitter::Language;

/// Treesitter infrastructure plugin
///
/// Manages tree-sitter parsing and highlighting for all buffers.
/// Language support is provided dynamically by language plugins
/// that emit `RegisterLanguage` events.
pub struct TreesitterPlugin {
    manager: Arc<SharedTreesitterManager>,
}

impl Default for TreesitterPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl TreesitterPlugin {
    /// Create a new treesitter plugin
    pub fn new() -> Self {
        Self {
            manager: Arc::new(SharedTreesitterManager::new()),
        }
    }
}

impl Plugin for TreesitterPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:treesitter")
    }

    fn name(&self) -> &'static str {
        "Treesitter"
    }

    fn description(&self) -> &'static str {
        "Syntax highlighting and semantic analysis via tree-sitter"
    }

    fn build(&self, ctx: &mut PluginContext) {
        use reovim_core::{
            bind::{CommandRef, KeymapScope},
            command::id::CommandId,
            keys,
        };

        // Register navigation commands
        let _ = ctx.register_command(command::JumpToParentScope);
        let _ = ctx.register_command(command::JumpToPrevScope);
        let _ = ctx.register_command(command::JumpToNextScope);

        // Bind keys in normal mode
        ctx.bind_key_scoped(
            KeymapScope::editor_normal(),
            keys!['g' 'u'],
            CommandRef::Registered(CommandId::new("jump_to_parent_scope")),
        );

        ctx.bind_key_scoped(
            KeymapScope::editor_normal(),
            keys!['[' 's'],
            CommandRef::Registered(CommandId::new("jump_to_prev_scope")),
        );

        ctx.bind_key_scoped(
            KeymapScope::editor_normal(),
            keys![']' 's'],
            CommandRef::Registered(CommandId::new("jump_to_next_scope")),
        );

        tracing::debug!("TreesitterPlugin: registered render stage and navigation commands");
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register as the semantic text object source
        registry.set_text_object_source(Arc::clone(&self.manager) as _);

        // Store in plugin state registry for other plugins to access
        registry.register(Arc::clone(&self.manager));

        // Register the syntax factory for buffer-centric highlighting
        let factory =
            Arc::new(crate::factory::TreesitterSyntaxFactory::new(Arc::clone(&self.manager)));
        registry.set_syntax_factory(factory);

        // Register the context provider for scope detection
        let context_provider =
            Arc::new(crate::context::TreesitterContextProvider::new(Arc::clone(&self.manager)));
        registry.register_context_provider(context_provider);

        tracing::debug!(
            "TreesitterPlugin: initialized state with syntax factory and context provider"
        );
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Handle language registration from language plugins
        {
            let state = Arc::clone(&state);
            bus.subscribe::<RegisterLanguage, _>(100, move |event, ctx| {
                state.with_mut::<Arc<SharedTreesitterManager>, _, _>(|manager| {
                    manager.with_mut(|m| {
                        m.register_language(Arc::clone(&event.language));
                    });
                    tracing::info!(
                        language_id = %event.language.language_id(),
                        "Registered language with treesitter"
                    );
                });
                // Trigger re-render so injection system can create layers for this language
                ctx.request_render();
                EventResult::Handled
            });
        }

        // Handle file open - detect language and init parser
        {
            let state = Arc::clone(&state);
            bus.subscribe::<FileOpened, _>(100, move |event, _ctx| {
                state.with_mut::<Arc<SharedTreesitterManager>, _, _>(|manager| {
                    let language_id =
                        manager.with_mut(|m| m.init_buffer(event.buffer_id, Some(&event.path)));

                    if let Some(lang_id) = language_id {
                        tracing::debug!(
                            buffer_id = event.buffer_id,
                            language_id = %lang_id,
                            "Initialized treesitter for buffer"
                        );
                    }
                });
                EventResult::Handled
            });
        }

        // Handle buffer close - cleanup parser state
        {
            use reovim_core::event_bus::BufferClosed;
            let state = Arc::clone(&state);
            bus.subscribe::<BufferClosed, _>(100, move |event, _ctx| {
                state.with_mut::<Arc<SharedTreesitterManager>, _, _>(|manager| {
                    manager.with_mut(|m| {
                        m.remove_buffer(event.buffer_id);
                    });
                });
                EventResult::Handled
            });
        }

        // Handle buffer modifications - schedule reparse
        {
            use reovim_core::event_bus::BufferModified;
            let state = Arc::clone(&state);
            bus.subscribe::<BufferModified, _>(100, move |event, _ctx| {
                state.with_mut::<Arc<SharedTreesitterManager>, _, _>(|manager| {
                    manager.with_mut(|m| {
                        if m.has_parser(event.buffer_id) {
                            m.schedule_reparse(event.buffer_id);
                        }
                    });
                });
                EventResult::Handled
            });
        }

        // Handle parse requests
        {
            let state = Arc::clone(&state);
            bus.subscribe::<ParseRequest, _>(100, move |event, ctx| {
                let result = state.with_mut::<Arc<SharedTreesitterManager>, _, _>(|manager| {
                    let (language_id, has_parser) = manager.with(|m| {
                        (m.buffer_language(event.buffer_id), m.has_parser(event.buffer_id))
                    });

                    if has_parser { language_id } else { None }
                });

                if let Some(Some(lang_id)) = result {
                    ctx.emit(ParseCompleted {
                        buffer_id: event.buffer_id,
                        language_id: lang_id,
                    });
                }
                EventResult::Handled
            });
        }

        // === Navigation command handlers ===

        // Jump to parent scope
        {
            let _state = Arc::clone(&state);
            bus.subscribe::<command::JumpToParentScope, _>(100, move |_event, _ctx| {
                // TODO: Implement full parent scope navigation
                // Need to:
                // 1. Get active buffer ID and cursor position from runtime state
                // 2. Get buffer content
                // 3. Query context from provider registry
                // 4. Find parent scope (second-to-last item)
                // 5. Emit RequestCursorMove to parent start line
                EventResult::NotHandled
            });
        }

        // Jump to previous scope
        {
            let _state = Arc::clone(&state);
            bus.subscribe::<command::JumpToPrevScope, _>(100, move |_event, _ctx| {
                // TODO: Implement - find nearest scope header above cursor
                EventResult::NotHandled
            });
        }

        // Jump to next scope
        {
            let _state = Arc::clone(&state);
            bus.subscribe::<command::JumpToNextScope, _>(100, move |_event, _ctx| {
                // TODO: Implement - find nearest scope header below cursor
                EventResult::NotHandled
            });
        }

        tracing::debug!("TreesitterPlugin: subscribed to events");
    }

    fn boot(
        &self,
        _bus: &EventBus,
        _state: Arc<PluginStateRegistry>,
        _event_tx: Option<mpsc::Sender<RuntimeEvent>>,
    ) {
        // Spawn background thread to pre-compile all queries
        // This runs after all languages are registered, before first file opens
        let manager = Arc::clone(&self.manager);
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let count = manager.with(|m| m.precompile_all_queries());
            let elapsed = start.elapsed();
            tracing::info!(
                queries = count,
                elapsed_ms = elapsed.as_millis(),
                "Background query precompilation finished"
            );
        });
        tracing::debug!("TreesitterPlugin: spawned background query precompilation");
    }
}

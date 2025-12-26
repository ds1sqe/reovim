//! Extensible option system for editor settings.
//!
//! This module provides a plugin-extensible settings system that allows:
//!
//! - **Core options**: Built-in options like `number`, `relativenumber`, `tabwidth`
//! - **Plugin options**: Plugins can register their own options dynamically
//! - **Validation**: Type-safe values with constraints (min/max, choices)
//! - **Persistence**: Options are saved to/loaded from TOML profiles
//!
//! # Architecture
//!
//! The option system follows the established `RegisterLanguage` pattern:
//!
//! 1. Plugins emit `RegisterOption` events to register their options
//! 2. The runtime subscribes and stores specs in `OptionRegistry`
//! 3. `:set` commands consult the registry for dynamic option lookup
//! 4. `OptionChanged` events notify plugins when values change
//!
//! # Example: Registering Plugin Options
//!
//! ```ignore
//! impl Plugin for MyPlugin {
//!     fn build(&self, ctx: &mut PluginContext) {
//!         ctx.option("my_setting")
//!             .description("My plugin setting")
//!             .default_bool(true)
//!             .register()?;
//!     }
//!
//!     fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
//!         bus.subscribe::<OptionChanged, _>(100, move |event, ctx| {
//!             if event.name == "plugin.myplugin.my_setting" {
//!                 // React to change
//!             }
//!             EventResult::Handled
//!         });
//!     }
//! }
//! ```
//!
//! # Profile Format
//!
//! Plugin options are stored in nested TOML sections:
//!
//! ```toml
//! [plugin.treesitter]
//! highlight_timeout_ms = 100
//! incremental_parse = true
//!
//! [plugin.completion]
//! auto_trigger = true
//! min_prefix = 2
//! ```

mod events;
mod registry;
mod spec;

pub use {
    events::{ChangeSource, OptionChanged, QueryOption, RegisterOption, ResetOption},
    registry::{OptionBuilder, OptionError, OptionRegistry},
    spec::{
        DependencyKind, OptionCategory, OptionConstraint, OptionDependency, OptionScope,
        OptionSpec, OptionValue,
    },
};

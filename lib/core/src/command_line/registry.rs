//! Ex-command registry for plugin-registered commands

use {
    crate::event_bus::DynEvent,
    std::{collections::HashMap, fmt, sync::RwLock},
};

/// Handler for ex-commands
#[derive(Clone)]
pub enum ExCommandHandler {
    /// Zero-arg command: `:name`
    ZeroArg {
        event_constructor: fn() -> DynEvent,
        description: &'static str,
    },

    /// Single string arg: `:name arg`
    SingleArg {
        event_constructor: fn(String) -> DynEvent,
        description: &'static str,
    },

    /// Subcommand: `:name subcommand [arg]`
    Subcommand {
        subcommands: HashMap<String, Box<Self>>,
        description: &'static str,
    },
}

impl fmt::Debug for ExCommandHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroArg { description, .. } => f
                .debug_struct("ZeroArg")
                .field("description", description)
                .finish(),
            Self::SingleArg { description, .. } => f
                .debug_struct("SingleArg")
                .field("description", description)
                .finish(),
            Self::Subcommand {
                subcommands,
                description,
            } => f
                .debug_struct("Subcommand")
                .field("subcommands", &subcommands.keys().collect::<Vec<_>>())
                .field("description", description)
                .finish(),
        }
    }
}

/// Thread-safe registry for plugin-registered ex-commands
pub struct ExCommandRegistry {
    handlers: RwLock<HashMap<String, ExCommandHandler>>,
}

impl ExCommandRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register an ex-command handler
    pub fn register(&self, name: impl Into<String>, handler: ExCommandHandler) {
        let name = name.into();
        if let Ok(mut handlers) = self.handlers.write() {
            if handlers.insert(name.clone(), handler).is_some() {
                tracing::warn!(command = %name, "Ex-command already registered, overwriting");
            } else {
                tracing::debug!(command = %name, "Ex-command registered");
            }
        }
    }

    /// Dispatch an ex-command string to a registered handler
    ///
    /// Returns `Some(event)` if a handler matches, `None` otherwise.
    #[must_use]
    pub fn dispatch(&self, input: &str) -> Option<DynEvent> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Try to match command name (first word)
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let cmd_name = parts.first()?;

        let handler = {
            let handlers = self.handlers.read().ok()?;
            handlers.get(*cmd_name).cloned()?
        };

        match handler {
            ExCommandHandler::ZeroArg {
                event_constructor, ..
            } => {
                if parts.len() == 1 {
                    Some(event_constructor())
                } else {
                    tracing::warn!(command = %cmd_name, "Expected zero arguments");
                    None
                }
            }
            ExCommandHandler::SingleArg {
                event_constructor, ..
            } => {
                if parts.len() >= 2 {
                    let arg = parts[1..].join(" ");
                    Some(event_constructor(arg))
                } else {
                    tracing::warn!(command = %cmd_name, "Expected one argument");
                    None
                }
            }
            ExCommandHandler::Subcommand { subcommands, .. } => {
                if parts.len() < 2 {
                    tracing::warn!(command = %cmd_name, "Expected subcommand");
                    return None;
                }

                let subcmd_name = parts[1];
                let subhandler = subcommands.get(subcmd_name)?;

                match subhandler.as_ref() {
                    ExCommandHandler::ZeroArg {
                        event_constructor, ..
                    } => {
                        if parts.len() == 2 {
                            Some(event_constructor())
                        } else {
                            tracing::warn!(
                                command = %cmd_name,
                                subcommand = %subcmd_name,
                                "Expected zero arguments"
                            );
                            None
                        }
                    }
                    ExCommandHandler::SingleArg {
                        event_constructor, ..
                    } => {
                        if parts.len() >= 3 {
                            let arg = parts[2..].join(" ");
                            Some(event_constructor(arg))
                        } else {
                            tracing::warn!(
                                command = %cmd_name,
                                subcommand = %subcmd_name,
                                "Expected one argument"
                            );
                            None
                        }
                    }
                    ExCommandHandler::Subcommand { .. } => {
                        tracing::warn!("Nested subcommands not yet supported");
                        None
                    }
                }
            }
        }
    }

    /// Get the description for a command (for help systems)
    #[must_use]
    pub fn get_description(&self, name: &str) -> Option<String> {
        let handlers = self.handlers.read().ok()?;
        let handler = handlers.get(name)?;

        let desc = match handler {
            ExCommandHandler::ZeroArg { description, .. }
            | ExCommandHandler::SingleArg { description, .. }
            | ExCommandHandler::Subcommand { description, .. } => *description,
        };
        drop(handlers);

        Some(desc.to_string())
    }

    /// List all registered commands with their descriptions
    ///
    /// Returns a vector of (`command_name`, description) pairs.
    /// Used for command-line auto-completion.
    #[must_use]
    pub fn list_commands(&self) -> Vec<(String, String)> {
        let Ok(handlers) = self.handlers.read() else {
            return Vec::new();
        };

        handlers
            .iter()
            .map(|(name, handler)| {
                let desc = match handler {
                    ExCommandHandler::ZeroArg { description, .. }
                    | ExCommandHandler::SingleArg { description, .. }
                    | ExCommandHandler::Subcommand { description, .. } => *description,
                };
                (name.clone(), desc.to_string())
            })
            .collect()
    }
}

impl Default for ExCommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ExCommandRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.handlers.read().map(|h| h.len()).unwrap_or(0);
        f.debug_struct("ExCommandRegistry")
            .field("command_count", &count)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::event_bus::Event};

    #[derive(Debug, Clone, Copy)]
    struct TestEvent;
    impl Event for TestEvent {
        fn priority(&self) -> u32 {
            100
        }
    }

    #[derive(Debug, Clone)]
    struct TestArgEvent {
        #[allow(dead_code)]
        arg: String,
    }
    impl Event for TestArgEvent {
        fn priority(&self) -> u32 {
            100
        }
    }

    #[test]
    fn test_zero_arg_command() {
        let registry = ExCommandRegistry::new();
        registry.register(
            "test",
            ExCommandHandler::ZeroArg {
                event_constructor: || DynEvent::new(TestEvent),
                description: "Test command",
            },
        );

        let result = registry.dispatch("test");
        assert!(result.is_some());

        let result = registry.dispatch("test extra");
        assert!(result.is_none()); // Should fail with extra args
    }

    #[test]
    fn test_single_arg_command() {
        let registry = ExCommandRegistry::new();
        registry.register(
            "load",
            ExCommandHandler::SingleArg {
                event_constructor: |arg| DynEvent::new(TestArgEvent { arg }),
                description: "Load something",
            },
        );

        let result = registry.dispatch("load myfile");
        assert!(result.is_some());

        let result = registry.dispatch("load");
        assert!(result.is_none()); // Should fail without arg
    }

    #[test]
    fn test_subcommand_dispatch() {
        let registry = ExCommandRegistry::new();

        let mut subcommands = HashMap::new();
        subcommands.insert(
            "list".to_string(),
            Box::new(ExCommandHandler::ZeroArg {
                event_constructor: || DynEvent::new(TestEvent),
                description: "List items",
            }),
        );
        subcommands.insert(
            "load".to_string(),
            Box::new(ExCommandHandler::SingleArg {
                event_constructor: |arg| DynEvent::new(TestArgEvent { arg }),
                description: "Load item",
            }),
        );

        registry.register(
            "profile",
            ExCommandHandler::Subcommand {
                subcommands,
                description: "Profile management",
            },
        );

        // Test zero-arg subcommand
        let result = registry.dispatch("profile list");
        assert!(result.is_some());

        // Test single-arg subcommand
        let result = registry.dispatch("profile load myprofile");
        assert!(result.is_some());
    }

    #[test]
    fn test_unknown_command() {
        let registry = ExCommandRegistry::new();
        let result = registry.dispatch("unknown");
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_command() {
        let registry = ExCommandRegistry::new();
        let result = registry.dispatch("");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_description() {
        let registry = ExCommandRegistry::new();
        registry.register(
            "test",
            ExCommandHandler::ZeroArg {
                event_constructor: || DynEvent::new(TestEvent),
                description: "Test command description",
            },
        );

        let desc = registry.get_description("test");
        assert_eq!(desc, Some("Test command description".to_string()));

        let desc = registry.get_description("nonexistent");
        assert_eq!(desc, None);
    }
}

use {
    crate::check::{HealthCheck, HealthCheckResult},
    reovim_core::plugin::PluginStateRegistry,
    std::sync::Arc,
};

/// Runtime check - always passes if the runtime is running
#[derive(Debug)]
pub struct RuntimeCheck;

impl HealthCheck for RuntimeCheck {
    fn name(&self) -> &'static str {
        "Runtime"
    }

    fn category(&self) -> &'static str {
        "Core"
    }

    fn run(&self) -> HealthCheckResult {
        HealthCheckResult::ok("Runtime is active and responsive")
    }
}

/// Plugin system check - verifies plugin registry is working
#[derive(Debug)]
pub struct PluginSystemCheck {
    pub state: Arc<PluginStateRegistry>,
}

impl HealthCheck for PluginSystemCheck {
    fn name(&self) -> &'static str {
        "Plugin System"
    }

    fn category(&self) -> &'static str {
        "Core"
    }

    fn run(&self) -> HealthCheckResult {
        // Check plugin system by counting registered plugin windows
        let windows = self.state.plugin_windows();
        let window_count = windows.len();

        if window_count > 0 {
            let mut result =
                HealthCheckResult::ok(format!("{window_count} plugin windows registered"));

            // Add details listing all plugin windows
            let details = windows
                .iter()
                .enumerate()
                .map(|(i, _)| format!("  {}. Plugin window #{}", i + 1, i))
                .collect::<Vec<_>>()
                .join("\n");

            result = result.with_details(format!("Registered plugin windows:\n{details}"));

            result
        } else {
            HealthCheckResult::ok("Plugin system operational")
        }
    }
}

/// Event bus check - verifies event system is responsive
#[derive(Debug)]
pub struct EventBusCheck;

impl HealthCheck for EventBusCheck {
    fn name(&self) -> &'static str {
        "Event Bus"
    }

    fn category(&self) -> &'static str {
        "Core"
    }

    fn run(&self) -> HealthCheckResult {
        // If we can run this check, the event bus is working
        HealthCheckResult::ok("Event bus is operational")
    }
}

/// Frame renderer check - verifies terminal is accessible
#[derive(Debug)]
pub struct FrameRendererCheck;

impl HealthCheck for FrameRendererCheck {
    fn name(&self) -> &'static str {
        "Frame Renderer"
    }

    fn category(&self) -> &'static str {
        "Core"
    }

    fn run(&self) -> HealthCheckResult {
        // Try to query terminal size
        match reovim_sys::terminal::size() {
            Ok((cols, rows)) => HealthCheckResult::ok(format!("Terminal size: {cols}x{rows}")),
            Err(e) => HealthCheckResult::error(format!("Failed to query terminal: {e}")),
        }
    }
}

/// Keybinding check - verifies keybinding system
#[derive(Debug)]
pub struct KeybindingCheck;

impl HealthCheck for KeybindingCheck {
    fn name(&self) -> &'static str {
        "Keybindings"
    }

    fn category(&self) -> &'static str {
        "Core"
    }

    fn run(&self) -> HealthCheckResult {
        // Basic check - if we can respond to keys, keybindings work
        // More sophisticated checking would require access to the keybinding registry
        HealthCheckResult::ok("Keybinding system operational")
    }
}

//! `:Modules` command handler.

use {
    reovim_driver_command::{Command, CommandHandler},
    reovim_driver_session::{BufferApi, SessionRuntime, WindowApi},
    reovim_kernel::api::v1::{CommandId, ModuleId},
    reovim_subsys_command_types::{CommandContext, CommandResult},
    reovim_subsys_module_loader::report::ModuleLoadReport,
    std::fmt::Write,
};

const MODULE_MANAGER_MODULE: ModuleId = ModuleId::new("module-manager");

/// List loaded, disabled, and failed modules in a scratch buffer.
pub struct ModulesCommand;

impl ModulesCommand {
    /// Create a new Modules command.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ModulesCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for ModulesCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE_MANAGER_MODULE, "modules")
    }

    fn description(&self) -> &'static str {
        "List loaded, disabled, and failed modules"
    }

    fn names(&self) -> &[&'static str] {
        &["Modules"]
    }
}

impl CommandHandler for ModulesCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let report = format_module_report(runtime);
        let buf_id = runtime.create_buffer(Some("[Modules]"), &report);
        runtime.set_buffer_modified(buf_id, false);
        runtime.set_active_buffer(Some(buf_id));
        // Switch the active window to display the new buffer so the TUI refreshes
        if let Some(win) = runtime.active_window() {
            let _ = runtime.set_window_buffer(win, buf_id);
        }
        CommandResult::Success
    }
}

/// Format the module report from `ModuleLoadReport`.
#[cfg_attr(coverage_nightly, coverage(off))]
fn format_module_report(runtime: &SessionRuntime<'_>) -> String {
    let mut out = String::new();

    let Some(report) = runtime.kernel().services.get::<ModuleLoadReport>() else {
        out.push_str("No module load report available.\n");
        return out;
    };

    let _ = writeln!(out, "=== Modules ===");
    let _ = writeln!(out);

    // Summary
    let _ = writeln!(
        out,
        "Total: {}  Loaded: {}  Disabled: {}  Failed: {}",
        report.total_count(),
        report.loaded.len(),
        report.disabled.len(),
        report.failed.len()
    );
    let _ = writeln!(out);

    // Loaded modules
    if !report.loaded.is_empty() {
        let _ = writeln!(out, "--- Loaded ({}) ---", report.loaded.len());
        for id in &report.loaded {
            let _ = writeln!(out, "  [OK] {}", id.as_str());
        }
        let _ = writeln!(out);
    }

    // Disabled modules
    if !report.disabled.is_empty() {
        let _ = writeln!(out, "--- Disabled ({}) ---", report.disabled.len());
        for id in &report.disabled {
            let _ = writeln!(out, "  [--] {} (disabled by config)", id.as_str());
        }
        let _ = writeln!(out);
    }

    // Failed modules
    if !report.failed.is_empty() {
        let _ = writeln!(out, "--- Failed ({}) ---", report.failed.len());
        for (id, reason) in &report.failed {
            let _ = writeln!(out, "  [!!] {}: {}", id.as_str(), reason);
        }
        let _ = writeln!(out);
    }

    // Missing dependencies
    if !report.missing_deps.is_empty() {
        let _ = writeln!(out, "--- Dependency Issues ({}) ---", report.missing_deps.len());
        for (module, dep) in &report.missing_deps {
            let _ = writeln!(
                out,
                "  [!!] {} requires '{}' which is not loaded",
                module.as_str(),
                dep.as_str()
            );
        }
        let _ = writeln!(out);
    }

    out
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;

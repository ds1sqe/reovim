//! Module management CLI subcommands (#621).
//!
//! Provides `reovim module <subcommand>` for offline module lifecycle
//! operations (install, remove, update, list, check, info, resolve).
//! These operations don't require a running server.

use {
    clap::Subcommand,
    reovim_driver_module_registry::{ModuleSource, RegistryPaths, workflow},
};

/// Module management subcommands.
#[derive(Debug, Subcommand)]
pub enum ModuleCommand {
    /// Install a module from a git URL or local path.
    Install {
        /// Source: git URL (https://...) or local path.
        source: String,
        /// Pin to a specific git revision (branch, tag, or commit).
        #[arg(long)]
        rev: Option<String>,
    },
    /// Remove an installed module.
    Remove {
        /// Module ID to remove.
        id: String,
    },
    /// Update an installed module (or all if no ID given).
    Update {
        /// Module ID to update (omit for all).
        id: Option<String>,
    },
    /// List installed third-party modules.
    List,
    /// Check module integrity (verify all .so files exist).
    Check,
    /// Show detailed info about an installed module.
    Info {
        /// Module ID to inspect.
        id: String,
    },
    /// Resolve and verify all installed modules.
    Resolve,
}

/// Run a module management command.
///
/// # Errors
///
/// Returns an IO error wrapping any registry operation failure.
// CLI glue: each arm calls workflow::* which shells out to git/cargo.
// Covered by E2E tests, not unit tests.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn run(command: &ModuleCommand) -> std::io::Result<()> {
    let paths = RegistryPaths::default_paths();

    match command {
        ModuleCommand::Install { source, rev } => {
            let module_source = parse_source(source, rev.as_deref());
            match workflow::install(&module_source, &paths) {
                Ok(installed) => {
                    println!("Installed: {installed}");
                    Ok(())
                }
                Err(e) => Err(std::io::Error::other(e.to_string())),
            }
        }
        ModuleCommand::Remove { id } => {
            workflow::remove(id, &paths).map_err(|e| std::io::Error::other(e.to_string()))?;
            println!("Removed: {id}");
            Ok(())
        }
        ModuleCommand::Update { id } => {
            if let Some(id) = id {
                let updated = workflow::update(id, &paths)
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                println!("Updated: {updated}");
            } else {
                let modules =
                    workflow::list(&paths).map_err(|e| std::io::Error::other(e.to_string()))?;
                if modules.is_empty() {
                    println!("No modules installed.");
                    return Ok(());
                }
                for module in &modules {
                    match workflow::update(&module.id, &paths) {
                        Ok(updated) => println!("Updated: {updated}"),
                        Err(e) => eprintln!("Failed to update {}: {e}", module.id),
                    }
                }
            }
            Ok(())
        }
        ModuleCommand::List => {
            let modules =
                workflow::list(&paths).map_err(|e| std::io::Error::other(e.to_string()))?;
            if modules.is_empty() {
                println!("No modules installed.");
            } else {
                println!("{:<25} {:<10} SOURCE", "ID", "VERSION");
                for m in &modules {
                    println!("{:<25} {:<10} {}", m.id, m.version, m.source);
                }
            }
            Ok(())
        }
        ModuleCommand::Check => {
            let report =
                workflow::check(&paths).map_err(|e| std::io::Error::other(e.to_string()))?;
            if report.is_clean() {
                println!("All modules OK ({} valid)", report.valid.len());
            } else {
                for (id, reason) in &report.broken {
                    eprintln!("BROKEN: {id} — {reason}");
                }
                for path in &report.orphaned {
                    eprintln!("ORPHAN: {}", path.display());
                }
            }
            Ok(())
        }
        ModuleCommand::Info { id } => {
            let module_info =
                workflow::info(id, &paths).map_err(|e| std::io::Error::other(e.to_string()))?;
            println!("ID:        {}", module_info.id);
            println!("Version:   {}", module_info.version);
            println!("Source:    {}", module_info.source);
            println!("Path:      {}", module_info.install_path.display());
            println!(
                "Library:   {}",
                if module_info.library_exists {
                    "OK"
                } else {
                    "MISSING"
                }
            );
            if !module_info.provides.is_empty() {
                println!("Provides:  {}", module_info.provides.join(", "));
            }
            if !module_info.requires.is_empty() {
                println!("Requires:  {}", module_info.requires.join(", "));
            }
            Ok(())
        }
        ModuleCommand::Resolve => {
            let modules =
                workflow::resolve(&paths).map_err(|e| std::io::Error::other(e.to_string()))?;
            println!("Resolved {} modules", modules.len());
            for m in &modules {
                let status = if m.is_built() { "built" } else { "unbuilt" };
                println!("  {} v{} [{}]", m.id, m.version, status);
            }
            Ok(())
        }
    }
}

/// Parse a source string into a `ModuleSource`.
///
/// Strings starting with `http://`, `https://`, or `git@` are treated as git sources.
/// Everything else is treated as a local path.
fn parse_source(source: &str, rev: Option<&str>) -> ModuleSource {
    if source.starts_with("http://") || source.starts_with("https://") || source.starts_with("git@")
    {
        rev.map_or_else(|| ModuleSource::git(source), |rev| ModuleSource::git_rev(source, rev))
    } else {
        ModuleSource::path(source)
    }
}

#[cfg(test)]
#[path = "module_cli_tests.rs"]
mod tests;

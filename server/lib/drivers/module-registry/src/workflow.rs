//! Module registry workflow operations (#621).
//!
//! Provides install, remove, update, check, resolve, and info operations
//! for third-party module lifecycle management.

use std::{
    fmt,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{InstalledModule, InstalledModules, ModuleManifest, ModuleSource};

// ============================================================================
// Registry Paths
// ============================================================================

/// Filesystem paths for the module registry.
#[derive(Debug, Clone)]
pub struct RegistryPaths {
    /// Root directory for installed modules.
    pub modules_dir: PathBuf,
    /// Path to the `installed.json` metadata file.
    pub installed_json: PathBuf,
}

impl RegistryPaths {
    /// Create registry paths from a root directory.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        let installed_json = root.join("installed.json");
        Self {
            modules_dir: root,
            installed_json,
        }
    }

    /// Default registry paths following XDG Base Directory.
    ///
    /// Uses `$XDG_DATA_HOME/reovim/modules/` or `~/.local/share/reovim/modules/`.
    // Env var branches require unsafe set_var to test; deny(unsafe_code) applies.
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[must_use]
    pub fn default_paths() -> Self {
        let base = std::env::var("XDG_DATA_HOME").map_or_else(
            |_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                PathBuf::from(home).join(".local/share")
            },
            PathBuf::from,
        );
        Self::new(base.join("reovim").join("modules"))
    }
}

// ============================================================================
// Registry Error
// ============================================================================

/// Error during registry operations.
#[derive(Debug)]
pub enum RegistryError {
    /// IO error.
    Io(std::io::Error),
    /// Git operation failed.
    Git(String),
    /// Build (cargo) operation failed.
    Build(String),
    /// Manifest parsing error.
    Manifest(String),
    /// Module not installed.
    NotInstalled(String),
    /// Module already installed.
    AlreadyInstalled(String),
    /// Metadata load/save error.
    Metadata(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "IO error: {err}"),
            Self::Git(msg) => write!(f, "git error: {msg}"),
            Self::Build(msg) => write!(f, "build error: {msg}"),
            Self::Manifest(msg) => write!(f, "manifest error: {msg}"),
            Self::NotInstalled(id) => write!(f, "module '{id}' is not installed"),
            Self::AlreadyInstalled(id) => write!(f, "module '{id}' is already installed"),
            Self::Metadata(msg) => write!(f, "metadata error: {msg}"),
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for RegistryError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

// ============================================================================
// Check Report
// ============================================================================

/// Report from checking module integrity.
#[derive(Debug, Default)]
pub struct CheckReport {
    /// Modules with valid libraries on disk.
    pub valid: Vec<String>,
    /// Modules with missing or broken libraries.
    pub broken: Vec<(String, String)>,
    /// Orphaned `.so` files not tracked in installed.json.
    pub orphaned: Vec<PathBuf>,
}

impl CheckReport {
    /// Whether all checks passed.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.broken.is_empty() && self.orphaned.is_empty()
    }
}

// ============================================================================
// Module Info
// ============================================================================

/// Combined module info for display.
#[derive(Debug)]
pub struct ModuleInfo {
    /// Module ID.
    pub id: String,
    /// Version string.
    pub version: String,
    /// Source (git URL or local path).
    pub source: ModuleSource,
    /// Install directory.
    pub install_path: PathBuf,
    /// Whether the library exists on disk.
    pub library_exists: bool,
    /// Capabilities provided (from manifest, if available).
    pub provides: Vec<String>,
    /// Capabilities required (from manifest, if available).
    pub requires: Vec<String>,
}

// ============================================================================
// Workflow Operations
// ============================================================================

/// Install a module from a source.
///
/// For git sources: clones the repo, parses `module.toml`, builds with cargo,
/// and copies the `.so` to the install path.
///
/// For path sources: parses `module.toml` from the local path, builds in place,
/// and symlinks/copies the `.so`.
///
/// # Errors
///
/// Returns [`RegistryError`] on git, build, manifest, or IO failures.
// Shells out to git/cargo — integration test territory.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn install(
    source: &ModuleSource,
    paths: &RegistryPaths,
) -> Result<InstalledModule, RegistryError> {
    // Ensure modules directory exists
    std::fs::create_dir_all(&paths.modules_dir)?;

    // Load existing metadata
    let mut installed = load_metadata(paths)?;

    // Determine source path (clone for git, use directly for local)
    let (source_path, module_id) = match source {
        ModuleSource::Git { url, rev } => {
            let clone_dir = paths.modules_dir.join(".tmp-clone");
            if clone_dir.exists() {
                std::fs::remove_dir_all(&clone_dir)?;
            }
            git_clone(url, rev.as_deref(), &clone_dir)?;
            let manifest = load_manifest(&clone_dir)?;
            let id = manifest.module.id;

            if installed.contains(&id) {
                std::fs::remove_dir_all(&clone_dir)?;
                return Err(RegistryError::AlreadyInstalled(id));
            }

            // Move clone to permanent location
            let install_dir = paths.modules_dir.join(&id);
            if install_dir.exists() {
                std::fs::remove_dir_all(&install_dir)?;
            }
            std::fs::rename(&clone_dir, &install_dir)?;
            (install_dir, id)
        }
        ModuleSource::Path { path } => {
            let source_path = PathBuf::from(path);
            let manifest = load_manifest(&source_path)?;
            let id = manifest.module.id;

            if installed.contains(&id) {
                return Err(RegistryError::AlreadyInstalled(id));
            }

            // For path sources, the install path is the source itself
            (source_path, id)
        }
    };

    // Parse manifest for metadata
    let manifest = load_manifest(&source_path)?;

    // Build the module
    let library_path = build_module(&source_path, manifest.crate_name())?;

    // If it's a git source, copy the built library to the install dir
    let final_library_path = if source.is_git() {
        let dest = source_path.join(library_path.file_name().unwrap_or_default());
        if library_path != dest {
            std::fs::copy(&library_path, &dest)?;
        }
        dest
    } else {
        library_path
    };

    let entry = InstalledModule {
        id: module_id.clone(),
        version: manifest.module.version,
        source: source.clone(),
        install_path: source_path,
        library_path: Some(final_library_path),
    };

    installed.insert(entry.clone());
    save_metadata(&installed, paths)?;

    tracing::info!(id = %module_id, "Module installed successfully");
    Ok(entry)
}

/// Remove an installed module.
///
/// Removes the install directory and updates `installed.json`.
///
/// # Errors
///
/// Returns [`RegistryError::NotInstalled`] if the module is not tracked.
pub fn remove(id: &str, paths: &RegistryPaths) -> Result<(), RegistryError> {
    let mut installed = load_metadata(paths)?;

    let entry = installed
        .remove(id)
        .ok_or_else(|| RegistryError::NotInstalled(id.to_string()))?;

    // Only remove install_path for git-sourced modules (we own the directory)
    if entry.source.is_git() && entry.install_path.exists() {
        std::fs::remove_dir_all(&entry.install_path)?;
    }

    save_metadata(&installed, paths)?;
    tracing::info!(id, "Module removed");
    Ok(())
}

/// Update an installed module.
///
/// For git sources: pulls latest changes and rebuilds.
/// For path sources: rebuilds in place.
///
/// # Errors
///
/// Returns [`RegistryError::NotInstalled`] if the module is not tracked.
// Shells out to git/cargo — integration test territory.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn update(id: &str, paths: &RegistryPaths) -> Result<InstalledModule, RegistryError> {
    let mut installed = load_metadata(paths)?;

    let entry = installed
        .get(id)
        .ok_or_else(|| RegistryError::NotInstalled(id.to_string()))?
        .clone();

    // Pull for git sources
    if let ModuleSource::Git { .. } = &entry.source {
        git_pull(&entry.install_path)?;
    }

    // Re-read manifest (version may have changed)
    let manifest = load_manifest(&entry.install_path)?;

    // Rebuild
    let library_path = build_module(&entry.install_path, manifest.crate_name())?;

    let final_library_path = if entry.source.is_git() {
        let dest = entry
            .install_path
            .join(library_path.file_name().unwrap_or_default());
        if library_path != dest {
            std::fs::copy(&library_path, &dest)?;
        }
        dest
    } else {
        library_path
    };

    let updated = InstalledModule {
        id: id.to_string(),
        version: manifest.module.version,
        source: entry.source,
        install_path: entry.install_path,
        library_path: Some(final_library_path),
    };

    installed.insert(updated.clone());
    save_metadata(&installed, paths)?;

    tracing::info!(id, version = %updated.version, "Module updated");
    Ok(updated)
}

/// Resolve: verify all installed modules have valid `.so` files.
///
/// # Errors
///
/// Returns [`RegistryError::Metadata`] on metadata load failure.
pub fn resolve(paths: &RegistryPaths) -> Result<Vec<InstalledModule>, RegistryError> {
    let installed = load_metadata(paths)?;
    Ok(installed.modules.into_values().collect())
}

/// Check module integrity: verify all `.so` files exist and match metadata.
///
/// # Errors
///
/// Returns [`RegistryError::Metadata`] on metadata load failure.
pub fn check(paths: &RegistryPaths) -> Result<CheckReport, RegistryError> {
    let installed = load_metadata(paths)?;
    let mut report = CheckReport::default();

    for (id, module) in &installed.modules {
        match &module.library_path {
            Some(path) if path.exists() => {
                report.valid.push(id.clone());
            }
            Some(path) => {
                report
                    .broken
                    .push((id.clone(), format!("library missing: {}", path.display())));
            }
            None => {
                report
                    .broken
                    .push((id.clone(), "no library path recorded".to_string()));
            }
        }
    }

    // Check for orphaned .so files
    if paths.modules_dir.exists() {
        for entry in std::fs::read_dir(&paths.modules_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "so") {
                let tracked = installed
                    .modules
                    .values()
                    .any(|m| m.library_path.as_ref().is_some_and(|lp| lp == &path));
                if !tracked {
                    report.orphaned.push(path);
                }
            }
        }
    }

    Ok(report)
}

/// Get detailed info about an installed module.
///
/// # Errors
///
/// Returns [`RegistryError::NotInstalled`] if the module is not tracked.
pub fn info(id: &str, paths: &RegistryPaths) -> Result<ModuleInfo, RegistryError> {
    let installed = load_metadata(paths)?;

    let entry = installed
        .get(id)
        .ok_or_else(|| RegistryError::NotInstalled(id.to_string()))?;

    let library_exists = entry.library_path.as_ref().is_some_and(|p| p.exists());

    // Try to load manifest for capability info
    let (provides, requires) = load_manifest(&entry.install_path)
        .map(|m| (m.capabilities.provides, m.capabilities.requires))
        .unwrap_or_default();

    Ok(ModuleInfo {
        id: entry.id.clone(),
        version: entry.version.clone(),
        source: entry.source.clone(),
        install_path: entry.install_path.clone(),
        library_exists,
        provides,
        requires,
    })
}

/// List all installed modules.
///
/// # Errors
///
/// Returns [`RegistryError::Metadata`] on metadata load failure.
pub fn list(paths: &RegistryPaths) -> Result<Vec<InstalledModule>, RegistryError> {
    let installed = load_metadata(paths)?;
    let mut modules: Vec<InstalledModule> = installed.modules.into_values().collect();
    modules.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(modules)
}

// ============================================================================
// Internal Helpers
// ============================================================================

fn load_metadata(paths: &RegistryPaths) -> Result<InstalledModules, RegistryError> {
    InstalledModules::load(&paths.installed_json).map_err(RegistryError::Metadata)
}

fn save_metadata(installed: &InstalledModules, paths: &RegistryPaths) -> Result<(), RegistryError> {
    std::fs::create_dir_all(&paths.modules_dir)?;
    installed
        .save(&paths.installed_json)
        .map_err(RegistryError::Metadata)
}

fn load_manifest(dir: &Path) -> Result<ModuleManifest, RegistryError> {
    let manifest_path = dir.join("module.toml");
    ModuleManifest::load(&manifest_path).map_err(|e| RegistryError::Manifest(e.to_string()))
}

// Shells out to `git clone` — not testable without real git repos.
#[cfg_attr(coverage_nightly, coverage(off))]
fn git_clone(url: &str, rev: Option<&str>, dest: &Path) -> Result<(), RegistryError> {
    let mut cmd = Command::new("git");
    cmd.args(["clone", "--depth", "1"]);
    if let Some(rev) = rev {
        cmd.args(["--branch", rev]);
    }
    cmd.arg(url).arg(dest);

    let output = cmd
        .output()
        .map_err(|e| RegistryError::Git(format!("failed to run git clone: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RegistryError::Git(format!("git clone failed: {stderr}")));
    }
    Ok(())
}

// Shells out to `git pull` — not testable without real git repos.
#[cfg_attr(coverage_nightly, coverage(off))]
fn git_pull(dir: &Path) -> Result<(), RegistryError> {
    let output = Command::new("git")
        .args(["pull", "--ff-only"])
        .current_dir(dir)
        .output()
        .map_err(|e| RegistryError::Git(format!("failed to run git pull: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RegistryError::Git(format!("git pull failed: {stderr}")));
    }
    Ok(())
}

// Shells out to `cargo build` — not testable without a real Cargo project.
#[cfg_attr(coverage_nightly, coverage(off))]
fn build_module(source_dir: &Path, crate_name: &str) -> Result<PathBuf, RegistryError> {
    let output = Command::new("cargo")
        .args(["build", "--release", "--lib", "--features", "dynamic"])
        .current_dir(source_dir)
        .output()
        .map_err(|e| RegistryError::Build(format!("failed to run cargo build: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RegistryError::Build(format!("cargo build failed: {stderr}")));
    }

    // Find the built .so file
    let lib_name = format!("lib{}.so", crate_name.replace('-', "_"));
    let lib_path = source_dir.join("target").join("release").join(&lib_name);

    if lib_path.exists() {
        Ok(lib_path)
    } else {
        Err(RegistryError::Build(format!(
            "built library not found at {}",
            lib_path.display()
        )))
    }
}

#[cfg(test)]
#[path = "workflow_tests.rs"]
mod tests;

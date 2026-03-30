//! Diagnostic collection and formatting.

use {
    reovim_driver_clipboard::{ClipboardKey, ClipboardProviderRegistry},
    reovim_driver_lsp::{LspKey, LspProviderRegistry},
    reovim_driver_module_loader::report::ModuleLoadReport,
    reovim_driver_syntax::{LanguageInfoStore, SyntaxFactoryStore},
    reovim_kernel::api::v1::{API_VERSION_STR, KernelContext},
    std::fmt::{self, Write},
};

/// Status indicator for a diagnostic entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Check passed.
    Ok,
    /// Non-critical warning.
    Warning,
    /// Informational (no pass/fail).
    Info,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => f.write_str("[OK]"),
            Self::Warning => f.write_str("[!!]"),
            Self::Info => f.write_str("[--]"),
        }
    }
}

/// A single diagnostic entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticEntry {
    /// Status indicator.
    pub status: Status,
    /// Short label.
    pub label: String,
    /// Detail text.
    pub detail: String,
}

impl DiagnosticEntry {
    /// Create a new entry.
    #[must_use]
    pub fn new(status: Status, label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            status,
            label: label.into(),
            detail: detail.into(),
        }
    }
}

/// A section of diagnostic entries under a title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticSection {
    /// Section header.
    pub title: &'static str,
    /// Entries in this section.
    pub entries: Vec<DiagnosticEntry>,
}

impl DiagnosticSection {
    /// Create a new section.
    #[must_use]
    pub const fn new(title: &'static str) -> Self {
        Self {
            title,
            entries: Vec::new(),
        }
    }
}

/// Collect system information.
#[must_use]
pub fn collect_system() -> DiagnosticSection {
    let mut section = DiagnosticSection::new("System");
    section.entries.push(DiagnosticEntry::new(
        Status::Ok,
        "reovim version",
        env!("CARGO_PKG_VERSION"),
    ));
    section
        .entries
        .push(DiagnosticEntry::new(Status::Ok, "API version", API_VERSION_STR));
    section.entries.push(DiagnosticEntry::new(
        Status::Ok,
        "Platform",
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
    ));
    section
}

/// Collect LSP provider information.
#[cfg_attr(coverage_nightly, coverage(off))]
#[must_use]
pub fn collect_lsp(kernel: &KernelContext) -> DiagnosticSection {
    let mut section = DiagnosticSection::new("Language Servers");

    let Some(registry) = kernel.services.get::<LspProviderRegistry>() else {
        section
            .entries
            .push(DiagnosticEntry::new(Status::Info, "LSP", "no providers registered"));
        return section;
    };

    let keys = registry.keys();
    if keys.is_empty() {
        section
            .entries
            .push(DiagnosticEntry::new(Status::Info, "LSP", "no providers registered"));
        return section;
    }

    for key in &keys {
        let label = match key {
            LspKey::Default => "default".to_string(),
            LspKey::Language(lang) => lang.clone(),
        };

        if let Some(provider) = registry.get(key) {
            let active = provider.is_active();
            let info = provider.server_info();
            let status_str = if active { "active" } else { "inactive" };
            let detail = info.map_or_else(
                || format!("{status_str} (no server info)"),
                |si| {
                    let version = si.version.as_deref().unwrap_or("unknown");
                    format!("{} {version} ({status_str})", si.name)
                },
            );
            let status = if active { Status::Ok } else { Status::Warning };
            section
                .entries
                .push(DiagnosticEntry::new(status, label, detail));
        }
    }

    section
}

/// Collect treesitter/syntax information.
#[must_use]
pub fn collect_treesitter(kernel: &KernelContext) -> DiagnosticSection {
    let mut section = DiagnosticSection::new("Syntax Highlighting");

    let factory_count = kernel
        .services
        .get::<SyntaxFactoryStore>()
        .map_or(0, |s| s.len());

    let lang_count = kernel
        .services
        .get::<LanguageInfoStore>()
        .map_or(0, |s| s.len());

    if factory_count == 0 && lang_count == 0 {
        section.entries.push(DiagnosticEntry::new(
            Status::Info,
            "Syntax",
            "no grammars registered",
        ));
    } else {
        if factory_count > 0 {
            section.entries.push(DiagnosticEntry::new(
                Status::Ok,
                "Syntax factories",
                format!("{factory_count} registered"),
            ));
        }
        if lang_count > 0 {
            section.entries.push(DiagnosticEntry::new(
                Status::Ok,
                "Language definitions",
                format!("{lang_count} registered"),
            ));
        }
    }

    section
}

/// Collect clipboard availability.
#[must_use]
pub fn collect_clipboard(kernel: &KernelContext) -> DiagnosticSection {
    let mut section = DiagnosticSection::new("Clipboard");

    let Some(registry) = kernel.services.get::<ClipboardProviderRegistry>() else {
        section.entries.push(DiagnosticEntry::new(
            Status::Warning,
            "Clipboard",
            "no providers registered",
        ));
        return section;
    };

    let Some(provider) = registry.get(&ClipboardKey::Default) else {
        section.entries.push(DiagnosticEntry::new(
            Status::Warning,
            "Clipboard",
            "default provider not found",
        ));
        return section;
    };

    let clipboard = provider.clipboard_available();
    let selection = provider.selection_available();

    section.entries.push(DiagnosticEntry::new(
        if clipboard {
            Status::Ok
        } else {
            Status::Warning
        },
        "System clipboard",
        if clipboard {
            "available"
        } else {
            "not available"
        },
    ));
    section.entries.push(DiagnosticEntry::new(
        if selection { Status::Ok } else { Status::Info },
        "Selection clipboard",
        if selection {
            "available"
        } else {
            "not available"
        },
    ));

    section
}

/// Collect option registry state.
#[cfg_attr(coverage_nightly, coverage(off))]
#[must_use]
pub fn collect_options(kernel: &KernelContext) -> DiagnosticSection {
    let mut section = DiagnosticSection::new("Options");
    let options = &kernel.options;
    let all = options.list_all();
    let total = all.len();

    let mut changed = 0u32;
    for name in &all {
        if let Some((spec, current)) = options.get_spec(name).zip(options.get_global(name))
            && current != spec.default
        {
            changed += 1;
        }
    }

    section.entries.push(DiagnosticEntry::new(
        Status::Ok,
        "Options registered",
        format!("{total}"),
    ));

    if changed > 0 {
        section.entries.push(DiagnosticEntry::new(
            Status::Info,
            "Changed from defaults",
            format!("{changed}"),
        ));
    }

    section
}

/// Collect loaded module status from `ModuleLoadReport`.
#[must_use]
pub fn collect_modules(kernel: &KernelContext) -> DiagnosticSection {
    let mut section = DiagnosticSection::new("Modules");

    let Some(report) = kernel.services.get::<ModuleLoadReport>() else {
        section.entries.push(DiagnosticEntry::new(
            Status::Info,
            "Modules",
            "no load report available",
        ));
        return section;
    };

    for id in &report.loaded {
        section
            .entries
            .push(DiagnosticEntry::new(Status::Ok, id.as_str(), "loaded"));
    }
    for (id, reason) in &report.failed {
        section
            .entries
            .push(DiagnosticEntry::new(Status::Warning, id.as_str(), reason));
    }
    for id in &report.disabled {
        section
            .entries
            .push(DiagnosticEntry::new(Status::Info, id.as_str(), "disabled by config"));
    }

    section
}

/// Collect dependency issues from `ModuleLoadReport`.
#[must_use]
pub fn collect_dependencies(kernel: &KernelContext) -> DiagnosticSection {
    let mut section = DiagnosticSection::new("Dependencies");

    let Some(report) = kernel.services.get::<ModuleLoadReport>() else {
        section.entries.push(DiagnosticEntry::new(
            Status::Info,
            "Dependencies",
            "no load report available",
        ));
        return section;
    };

    if report.missing_deps.is_empty() {
        section
            .entries
            .push(DiagnosticEntry::new(Status::Ok, "Dependencies", "all satisfied"));
    } else {
        for (module, dep) in &report.missing_deps {
            section.entries.push(DiagnosticEntry::new(
                Status::Warning,
                module.as_str(),
                format!("requires '{}' which is not loaded", dep.as_str()),
            ));
        }
    }

    section
}

/// Collect configuration path status from `ModuleLoadReport`.
#[must_use]
pub fn collect_configuration(kernel: &KernelContext) -> DiagnosticSection {
    let mut section = DiagnosticSection::new("Configuration");

    let Some(report) = kernel.services.get::<ModuleLoadReport>() else {
        section.entries.push(DiagnosticEntry::new(
            Status::Info,
            "Configuration",
            "no load report available",
        ));
        return section;
    };

    match &report.config_path {
        Some(path) => {
            section.entries.push(DiagnosticEntry::new(
                Status::Ok,
                "Config file",
                path.display().to_string(),
            ));
        }
        None => {
            section.entries.push(DiagnosticEntry::new(
                Status::Info,
                "Config file",
                "not found (using defaults)",
            ));
        }
    }

    for path in &report.search_paths {
        section.entries.push(DiagnosticEntry::new(
            Status::Info,
            "Module search path",
            path.display().to_string(),
        ));
    }

    if report.isolation_active {
        section.entries.push(DiagnosticEntry::new(
            Status::Ok,
            "Isolation",
            "active (env override)",
        ));
    }

    section
}

/// Collect all diagnostic sections.
#[must_use]
pub fn collect_all(kernel: &KernelContext) -> Vec<DiagnosticSection> {
    vec![
        collect_system(),
        collect_modules(kernel),
        collect_dependencies(kernel),
        collect_configuration(kernel),
        collect_lsp(kernel),
        collect_treesitter(kernel),
        collect_clipboard(kernel),
        collect_options(kernel),
    ]
}

/// Format diagnostic sections into a plain-text report.
#[must_use]
pub fn format_report(sections: &[DiagnosticSection]) -> String {
    let mut out = String::new();

    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let _ = writeln!(out, "=== {} ===", section.title);

        if section.entries.is_empty() {
            out.push_str("  (no data)\n");
        } else {
            for entry in &section.entries {
                let _ = writeln!(out, "  {} {}: {}", entry.status, entry.label, entry.detail);
            }
        }
    }

    out
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;

//! Static module factories for builtin modules (#620).
//!
//! This module provides the factory map for all builtin server modules
//! when the `static-modules` feature is enabled. It replaces the
//! `DefaultsModule` god-crate with direct imports gated behind a feature flag.
//!
//! When `static-modules` is disabled, modules are loaded dynamically from
//! `.so` files via `ModuleLoader`.

use std::collections::HashMap;

use reovim_kernel::api::v1::Module;

/// Type alias for module factory functions.
pub type ModuleFactory = fn() -> Box<dyn Module>;

/// Get the builtin module registry: a map from module ID to factory function.
///
/// This is the canonical list of all builtin server modules for static linking.
/// Bootstrap iterates this registry and calls factories for enabled modules only.
/// Ordering comes from the `builtins.toml` manifest, not from this map.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn builtin_registry() -> HashMap<&'static str, ModuleFactory> {
    let mut map: HashMap<&'static str, ModuleFactory> = HashMap::with_capacity(45);

    // Tier 1: Service modules
    map.insert("undo", || Box::new(reovim_module_undo::UndoModule::new()));
    map.insert("buffer-simple", || {
        Box::new(reovim_module_buffer_simple::BufferSimpleModule::new())
    });
    map.insert("search", || Box::new(reovim_module_search::SearchModule::new()));
    map.insert("scratch-buffer", || {
        Box::new(reovim_module_scratch_buffer::ScratchBufferModule::new())
    });
    map.insert("vfs-local", || Box::new(reovim_module_vfs_local::VfsLocalModule::new()));
    map.insert("clipboard", || Box::new(reovim_module_clipboard::ClipboardModule::new()));
    map.insert("codec-utf8", || Box::new(reovim_content_codec_utf8::CodecUtf8Module::new()));
    map.insert("codec-hex", || Box::new(reovim_content_codec_hex::CodecHexModule::new()));
    map.insert("codec-cjk", || Box::new(reovim_content_codec_cjk::CodecCjkModule::new()));
    map.insert("codec-legacy", || {
        Box::new(reovim_content_codec_legacy::CodecLegacyModule::new())
    });
    map.insert("codec-pdf", || Box::new(reovim_content_codec_pdf::CodecPdfModule::new()));
    map.insert("codec-binary-struct", || {
        Box::new(reovim_content_codec_binary_struct::CodecBinaryStructModule::new())
    });
    map.insert("codec-rlib", || Box::new(reovim_content_codec_rlib::CodecRlibModule::new()));
    map.insert("codec-csv", || Box::new(reovim_content_codec_csv::CodecCsvModule::new()));
    map.insert("layout", || Box::new(reovim_module_layout::LayoutModule::new()));

    // Tier 2: Utility modules
    map.insert("keymap", || Box::new(reovim_module_keymap::KeymapModule));
    map.insert("commands", || Box::new(reovim_module_commands::CommandsModule));

    // Tier 3: Policy modules
    map.insert("editor", || Box::new(reovim_module_editor::EditorModule));
    map.insert("motions", || Box::new(reovim_module_motions::MotionsModule));
    map.insert("vim", || Box::new(reovim_module_vim::VimModule::new()));
    map.insert("window-ops", || Box::new(reovim_module_window_ops::WindowOps::new()));

    // Tier 3: Git provider (#530) — must init before consumers and pickers
    map.insert("git", || Box::new(reovim_module_git::GitModule::new()));

    // Tier 3: Git consumer modules (#530)
    map.insert("git-signs", || Box::new(reovim_module_git_signs::GitSignsModule::new()));
    map.insert("git-statusline", || {
        Box::new(reovim_module_git_statusline::GitStatuslineModule::new())
    });
    map.insert("git-blame", || Box::new(reovim_module_git_blame::GitBlameModule::new()));

    // Tier 3: Extension bridge modules
    map.insert("cmdline", || Box::new(reovim_module_cmdline::CmdlineModule::new()));
    map.insert("whichkey", || Box::new(reovim_module_whichkey::WhichKeyModule::new()));
    map.insert("notification", || {
        Box::new(reovim_module_notification::NotificationModule::new())
    });

    // Tier 4: Picker data providers (#522)
    map.insert("picker-files", || Box::new(reovim_picker_files::PickerFilesModule::new()));
    map.insert("picker-buffers", || Box::new(reovim_picker_buffers::PickerBuffersModule::new()));
    map.insert("picker-commands", || {
        Box::new(reovim_picker_commands::PickerCommandsModule::new())
    });
    map.insert("picker-grep", || Box::new(reovim_picker_grep::PickerGrepModule::new()));
    map.insert("picker-options", || Box::new(reovim_picker_options::PickerOptionsModule::new()));

    // Tier 4: Git picker providers (#530)
    map.insert("picker-git-branches", || {
        Box::new(reovim_picker_git_branches::PickerGitBranchesModule::new())
    });
    map.insert("picker-git-log", || Box::new(reovim_picker_git_log::PickerGitLogModule::new()));
    map.insert("picker-git-stash", || {
        Box::new(reovim_picker_git_stash::PickerGitStashModule::new())
    });
    map.insert("picker-git-status", || {
        Box::new(reovim_picker_git_status::PickerGitStatusModule::new())
    });

    // Tier 4: Picker orchestration
    map.insert("microscope", || Box::new(reovim_module_microscope::MicroscopeModule::new()));

    // Tier 4: Syntax highlighting modules
    map.insert("treesitter-rust", || {
        Box::new(reovim_module_treesitter_rust::TreesitterRustModule::new())
    });
    map.insert("treesitter-markdown", || {
        Box::new(reovim_module_treesitter_markdown::TreesitterMarkdownModule::new())
    });

    // Tier 4: Code intelligence modules
    map.insert("lsp", || Box::new(reovim_module_lsp::LspModule::new()));
    map.insert("lsp-navigation", || {
        Box::new(reovim_module_lsp_navigation::LspNavigationModule::new())
    });

    // Tier 4: Text objects
    map.insert("textobjects", || Box::new(reovim_module_textobjects::TextObjectsModule::new()));

    // Tier 4: Snippet expansion (#136)
    map.insert("snippet", || Box::new(reovim_module_snippet::SnippetModule::new()));

    // Tier 4: Jump navigation and code folding (#524)
    map.insert(
        "range-finder",
        || Box::new(reovim_module_range_finder::RangeFinderModule::new()),
    );

    // Tier 4: Completion engine (#521)
    map.insert("completion", || Box::new(reovim_module_completion::CompletionModule::new()));

    // Tier 4: File explorer (#523)
    map.insert("explorer", || Box::new(reovim_module_explorer::ExplorerModule::new()));

    // Tier 4: Tetromino game (#537)
    map.insert("tetromino", || Box::new(reovim_module_tetromino::TetrominoModule::new()));

    // Tier 4: Configuration profiles (#593)
    map.insert("profiles", || Box::new(reovim_module_profiles::ProfilesModule::new()));

    // Tier 4: Health-check diagnostic command (#594)
    map.insert(
        "health-check",
        || Box::new(reovim_module_health_check::HealthCheckModule::new()),
    );

    // Tier 4: Module manager command (#622)
    map.insert("module-manager", || {
        Box::new(reovim_module_module_manager::ModuleManagerModule::new())
    });

    // Tier 4: Word reference highlight (#664)
    map.insert("illuminate", || Box::new(reovim_module_illuminate::IlluminateModule::new()));

    // Tier 4: Format-on-save (#667)
    map.insert("format", || Box::new(reovim_module_format::FormatModule::new()));

    // Tier 4: Diagnostics panel (#665)
    map.insert("diagnostics-panel", || {
        Box::new(reovim_module_diagnostics_panel::DiagnosticsPanelModule::new())
    });

    // Tier 4: Bufferline (#662)
    map.insert("bufferline", || Box::new(reovim_module_bufferline::BufferlineModule::new()));

    map
}

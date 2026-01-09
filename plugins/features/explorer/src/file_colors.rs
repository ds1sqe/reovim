//! File type color mapping for the explorer
//!
//! Maps file extensions and names to semantic colors from the theme.

use reovim_core::highlight::{Style, Theme};

use super::node::FileNode;

/// Categories for file type coloring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // All variants are used through categorize_extension
enum FileCategory {
    Directory,
    SourceCode,
    Configuration,
    Documentation,
    Data,
    Media,
    Special,
    Lock,
    Hidden,
    Unknown,
}

/// Get the appropriate color style for a file node based on its type
#[must_use]
pub fn get_file_color(node: &FileNode, theme: &Theme) -> Style {
    // Directories get special treatment
    if node.is_dir() {
        return theme.file_explorer.directory.clone();
    }

    // Hidden files are dimmed
    if node.is_hidden {
        return theme.file_explorer.hidden_file.clone();
    }

    // Check for special filenames first
    let name_lower = node.name.to_lowercase();
    if let Some(style) = get_special_file_color(&name_lower, theme) {
        return style;
    }

    // Categorize by extension
    let category = if let Some(ext) = node.path.extension() {
        if let Some(ext_str) = ext.to_str() {
            categorize_extension(ext_str)
        } else {
            FileCategory::Unknown
        }
    } else {
        FileCategory::Unknown
    };

    // Map category to theme color
    match category {
        FileCategory::Directory => theme.file_explorer.directory.clone(),
        FileCategory::SourceCode => theme.file_explorer.source_file.clone(),
        FileCategory::Configuration => theme.file_explorer.config_file.clone(),
        FileCategory::Documentation => theme.file_explorer.doc_file.clone(),
        FileCategory::Data => theme.file_explorer.data_file.clone(),
        FileCategory::Media => theme.file_explorer.media_file.clone(),
        FileCategory::Special => theme.file_explorer.special_file.clone(),
        FileCategory::Lock => theme.file_explorer.lock_file.clone(),
        FileCategory::Hidden => theme.file_explorer.hidden_file.clone(),
        FileCategory::Unknown => theme.base.default.clone(),
    }
}

/// Check for special filenames that get specific colors
fn get_special_file_color(name_lower: &str, theme: &Theme) -> Option<Style> {
    match name_lower {
        // License files
        "license" | "licence" | "license.md" | "licence.md" | "license.txt" | "licence.txt" => {
            Some(theme.file_explorer.special_file.clone())
        }
        // Documentation files
        "readme" | "readme.md" | "readme.txt" | "changelog" | "changelog.md" => {
            Some(theme.file_explorer.doc_file.clone())
        }
        // Lock files
        "cargo.lock" | "package-lock.json" | "yarn.lock" | "gemfile.lock" | "poetry.lock" => {
            Some(theme.file_explorer.lock_file.clone())
        }
        // Build/config files
        "makefile" | "dockerfile" | "rakefile" | "cmakelists.txt" => {
            Some(theme.file_explorer.config_file.clone())
        }
        _ => None,
    }
}

/// Categorize a file extension into a semantic category
fn categorize_extension(ext: &str) -> FileCategory {
    match ext.to_lowercase().as_str() {
        // Source code - programming languages
        "rs" | "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "hxx" | "go" | "java" | "kt" | "kts"
        | "scala" | "swift" | "py" | "rb" | "php" | "js" | "jsx" | "ts" | "tsx" | "vue"
        | "svelte" | "lua" | "pl" | "pm" | "sh" | "bash" | "zsh" | "fish" | "cs" | "fs" | "vb"
        | "asm" | "s" | "dart" | "elm" | "erl" | "ex" | "exs" | "hs" | "clj" | "cljs" | "vim"
        | "v" => FileCategory::SourceCode,

        // Configuration files
        "toml" | "yaml" | "yml" | "json" | "ini" | "conf" | "cfg" | "config" | "xml" | "env"
        | "properties" | "gradle" | "pom" => FileCategory::Configuration,

        // Documentation
        "md" | "markdown" | "rst" | "txt" | "adoc" | "tex" | "org" => FileCategory::Documentation,

        // Data files
        "csv" | "tsv" | "sql" | "db" | "sqlite" | "sqlite3" => FileCategory::Data,

        // Media files
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "ico" | "webp" | "mp3" | "mp4" | "avi"
        | "mkv" | "wav" | "flac" | "ogg" | "webm" => FileCategory::Media,

        // Lock files
        "lock" => FileCategory::Lock,

        // Default
        _ => FileCategory::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// Helper to create a test node
    fn create_test_node(name: &str, is_dir: bool, is_hidden: bool) -> FileNode {
        use crate::node::NodeType;

        let path = PathBuf::from(if is_dir {
            format!("/tmp/{}/", name)
        } else {
            format!("/tmp/{}", name)
        });

        let node_type = if is_dir {
            NodeType::Directory {
                expanded: false,
                children: Vec::new(),
            }
        } else {
            NodeType::File {
                size: 0,
                created: None,
                modified: None,
            }
        };

        FileNode {
            name: name.to_string(),
            path,
            node_type,
            depth: 1,
            is_hidden,
        }
    }

    #[test]
    fn test_directory_color() {
        let theme = Theme::dark();
        let node = create_test_node("src", true, false);
        let color = get_file_color(&node, &theme);

        // Directories should use theme.file_explorer.directory
        assert_eq!(color.fg, theme.file_explorer.directory.fg);
    }

    #[test]
    fn test_rust_source_color() {
        let theme = Theme::dark();
        let node = create_test_node("main.rs", false, false);
        let color = get_file_color(&node, &theme);

        // Rust files should use theme.file_explorer.source_file
        assert_eq!(color.fg, theme.file_explorer.source_file.fg);
    }

    #[test]
    fn test_toml_config_color() {
        let theme = Theme::dark();
        let node = create_test_node("Cargo.toml", false, false);
        let color = get_file_color(&node, &theme);

        // TOML files should use theme.file_explorer.config_file
        assert_eq!(color.fg, theme.file_explorer.config_file.fg);
    }

    #[test]
    fn test_markdown_doc_color() {
        let theme = Theme::dark();
        let node = create_test_node("README.md", false, false);
        let color = get_file_color(&node, &theme);

        // README.md is a special file, should use file_explorer.doc_file
        assert_eq!(color.fg, theme.file_explorer.doc_file.fg);
    }

    #[test]
    fn test_license_special_color() {
        let theme = Theme::dark();
        let node = create_test_node("LICENSE", false, false);
        let color = get_file_color(&node, &theme);

        // LICENSE should use theme.file_explorer.special_file
        assert_eq!(color.fg, theme.file_explorer.special_file.fg);
    }

    #[test]
    fn test_lock_file_color() {
        let theme = Theme::dark();
        let node = create_test_node("Cargo.lock", false, false);
        let color = get_file_color(&node, &theme);

        // Lock files should use theme.file_explorer.lock_file
        assert_eq!(color.fg, theme.file_explorer.lock_file.fg);
    }

    #[test]
    fn test_hidden_file_color() {
        let theme = Theme::dark();
        let node = create_test_node(".gitignore", false, true);
        let color = get_file_color(&node, &theme);

        // Hidden files should use theme.file_explorer.hidden_file
        assert_eq!(color.fg, theme.file_explorer.hidden_file.fg);
    }

    #[test]
    fn test_categorize_source_extensions() {
        assert_eq!(categorize_extension("rs"), FileCategory::SourceCode);
        assert_eq!(categorize_extension("py"), FileCategory::SourceCode);
        assert_eq!(categorize_extension("js"), FileCategory::SourceCode);
        assert_eq!(categorize_extension("go"), FileCategory::SourceCode);
    }

    #[test]
    fn test_categorize_config_extensions() {
        assert_eq!(categorize_extension("toml"), FileCategory::Configuration);
        assert_eq!(categorize_extension("yaml"), FileCategory::Configuration);
        assert_eq!(categorize_extension("json"), FileCategory::Configuration);
    }

    #[test]
    fn test_categorize_unknown_extension() {
        assert_eq!(categorize_extension("xyz"), FileCategory::Unknown);
        assert_eq!(categorize_extension("unknown"), FileCategory::Unknown);
    }
}

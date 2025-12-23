//! Built-in file type icons
//!
//! Provides icons for common file extensions, directories, and symlinks.

use super::{IconSet, registry::IconProvider};

/// Built-in file icon provider (priority 0)
pub struct BuiltinFileIconProvider;

impl IconProvider for BuiltinFileIconProvider {
    #[allow(clippy::too_many_lines)]
    fn file_icon(&self, ext: &str, set: IconSet) -> Option<&'static str> {
        let icon = match ext {
            // Rust
            "rs" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🦀 ",
                IconSet::Ascii => "rs ",
            },
            // Python
            "py" | "pyi" | "pyc" | "pyd" | "pyw" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🐍 ",
                IconSet::Ascii => "py ",
            },
            // JavaScript / TypeScript
            "js" | "mjs" | "cjs" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "JS ",
                IconSet::Ascii => "js ",
            },
            "ts" | "mts" | "cts" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "TS ",
                IconSet::Ascii => "ts ",
            },
            "jsx" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "⚛ ",
                IconSet::Ascii => "jsx",
            },
            "tsx" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "⚛ ",
                IconSet::Ascii => "tsx",
            },
            // C / C++
            "c" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "C ",
                IconSet::Ascii => "c ",
            },
            "h" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "H ",
                IconSet::Ascii => "h ",
            },
            "cpp" | "cc" | "cxx" | "c++" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "C++ ",
                IconSet::Ascii => "cpp",
            },
            "hpp" | "hh" | "hxx" | "h++" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "H++ ",
                IconSet::Ascii => "hpp",
            },
            // Go
            "go" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "Go ",
                IconSet::Ascii => "go ",
            },
            // Java
            "java" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "☕ ",
                IconSet::Ascii => "jav",
            },
            "jar" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "📦 ",
                IconSet::Ascii => "jar",
            },
            // Lua
            "lua" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🌙 ",
                IconSet::Ascii => "lua",
            },
            // Ruby
            "rb" | "erb" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "💎 ",
                IconSet::Ascii => "rb ",
            },
            // PHP
            "php" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🐘 ",
                IconSet::Ascii => "php",
            },
            // Shell
            "sh" | "bash" | "zsh" | "fish" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "$ ",
                IconSet::Ascii => "sh ",
            },
            // Config files
            "json" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "{} ",
                IconSet::Ascii => "jsn",
            },
            "toml" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "[T] ",
                IconSet::Ascii => "tml",
            },
            "yaml" | "yml" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "⚙ ",
                IconSet::Ascii => "yml",
            },
            "xml" => match set {
                IconSet::Nerd => "󰗀 ",
                IconSet::Unicode => "<> ",
                IconSet::Ascii => "xml",
            },
            "ini" | "cfg" | "conf" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "⚙ ",
                IconSet::Ascii => "cfg",
            },
            // Markdown / Documentation
            "md" | "markdown" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "📝 ",
                IconSet::Ascii => "md ",
            },
            "txt" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "📄 ",
                IconSet::Ascii => "txt",
            },
            "rst" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "📄 ",
                IconSet::Ascii => "rst",
            },
            // Web
            "html" | "htm" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🌐 ",
                IconSet::Ascii => "htm",
            },
            "css" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🎨 ",
                IconSet::Ascii => "css",
            },
            "scss" | "sass" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🎨 ",
                IconSet::Ascii => "scs",
            },
            "vue" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "V ",
                IconSet::Ascii => "vue",
            },
            "svelte" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "S ",
                IconSet::Ascii => "svl",
            },
            // Images
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "webp" | "svg" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🖼 ",
                IconSet::Ascii => "img",
            },
            // Videos
            "mp4" | "mkv" | "avi" | "mov" | "webm" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🎬 ",
                IconSet::Ascii => "vid",
            },
            // Audio
            "mp3" | "wav" | "flac" | "ogg" | "m4a" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🎵 ",
                IconSet::Ascii => "aud",
            },
            // Archives
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "📦 ",
                IconSet::Ascii => "zip",
            },
            // Databases
            "sql" | "db" | "sqlite" | "sqlite3" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🗄 ",
                IconSet::Ascii => "db ",
            },
            // Docker
            "dockerfile" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🐳 ",
                IconSet::Ascii => "dkr",
            },
            // Git
            "gitignore" | "gitattributes" | "gitmodules" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "⎇ ",
                IconSet::Ascii => "git",
            },
            // Lock files
            "lock" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "🔒 ",
                IconSet::Ascii => "lck",
            },
            // Logs
            "log" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "📋 ",
                IconSet::Ascii => "log",
            },
            // Vim
            "vim" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "V ",
                IconSet::Ascii => "vim",
            },
            // License
            "license" | "licence" => match set {
                IconSet::Nerd => " ",
                IconSet::Unicode => "📜 ",
                IconSet::Ascii => "lic",
            },
            // Not found - return None to fall through
            _ => return None,
        };
        Some(icon)
    }

    fn dir_icon(&self, expanded: bool, set: IconSet) -> Option<&'static str> {
        let icon = if expanded {
            match set {
                IconSet::Nerd => "󰝰 ",
                IconSet::Unicode => "📂 ",
                IconSet::Ascii => "v ",
            }
        } else {
            match set {
                IconSet::Nerd => "󰉋 ",
                IconSet::Unicode => "📁 ",
                IconSet::Ascii => "> ",
            }
        };
        Some(icon)
    }

    fn priority(&self) -> u32 {
        0 // Built-in, lowest priority
    }

    fn name(&self) -> &'static str {
        "builtin-file"
    }
}

/// Get icon for symlink
#[must_use]
pub const fn symlink_icon(broken: bool, set: IconSet) -> &'static str {
    if broken {
        match set {
            IconSet::Nerd => " ",
            IconSet::Unicode => "⚠ ",
            IconSet::Ascii => "! ",
        }
    } else {
        match set {
            IconSet::Nerd => " ",
            IconSet::Unicode => "🔗 ",
            IconSet::Ascii => "@ ",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_icon() {
        let provider = BuiltinFileIconProvider;
        assert!(provider.file_icon("rs", IconSet::Nerd).is_some());
        assert!(provider.file_icon("rs", IconSet::Unicode).is_some());
        assert!(provider.file_icon("rs", IconSet::Ascii).is_some());
    }

    #[test]
    fn test_unknown_extension() {
        let provider = BuiltinFileIconProvider;
        assert!(provider.file_icon("xyz123", IconSet::Nerd).is_none());
    }

    #[test]
    fn test_dir_icon() {
        let provider = BuiltinFileIconProvider;
        assert!(provider.dir_icon(true, IconSet::Nerd).is_some());
        assert!(provider.dir_icon(false, IconSet::Nerd).is_some());
    }

    #[test]
    fn test_toml_icon() {
        let provider = BuiltinFileIconProvider;
        let icon = provider.file_icon("toml", IconSet::Nerd);
        assert!(icon.is_some(), "toml should have an icon");
        assert_eq!(icon.unwrap(), " ", "toml Nerd icon should be ");
    }

    #[test]
    fn test_md_icon() {
        let provider = BuiltinFileIconProvider;
        let icon = provider.file_icon("md", IconSet::Nerd);
        assert!(icon.is_some(), "md should have an icon");
        assert_eq!(icon.unwrap(), " ", "md Nerd icon should be ");
    }

    #[test]
    fn test_json_icon() {
        let provider = BuiltinFileIconProvider;
        let icon = provider.file_icon("json", IconSet::Nerd);
        assert!(icon.is_some(), "json should have an icon");
        assert_eq!(icon.unwrap(), " ", "json Nerd icon should be ");
    }
}

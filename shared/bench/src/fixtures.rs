//! VFS tree fixture factories for benchmarking.
//!
//! Provides pre-built [`MockVfs`] trees of various shapes and sizes.
//! Used by module benchmarks to create deterministic test data.

use {reovim_driver_vfs::MockVfs, std::path::PathBuf};

/// Pre-built VFS tree fixture for benchmarking.
///
/// Contains a [`MockVfs`] populated with files and directories,
/// and the root path where the tree starts.
pub struct TreeFixture {
    /// The populated mock VFS.
    pub vfs: MockVfs,
    /// Root directory of the fixture tree.
    pub root: PathBuf,
}

impl TreeFixture {
    /// Flat directory with `file_count` files, no subdirectories.
    ///
    /// Files are named `000.rs`, `001.rs`, etc.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_bench_utils::fixtures::TreeFixture;
    ///
    /// let fixture = TreeFixture::flat(100);
    /// // fixture.vfs has /bench/ with 100 .rs files
    /// ```
    #[must_use]
    pub fn flat(file_count: usize) -> Self {
        let vfs = MockVfs::new();
        let root = PathBuf::from("/bench");
        vfs.add_dir(&root);

        let width = digit_width(file_count);
        for i in 0..file_count {
            let name = format!("/bench/{i:0width$}.rs");
            vfs.add_file_str(&name, "fn main() {}");
        }

        Self { vfs, root }
    }

    /// Nested tree with `depth` levels of subdirectories.
    ///
    /// Each directory contains `files_per_dir` files and one subdirectory
    /// (except the deepest level which has only files).
    ///
    /// Total files = `depth * files_per_dir`.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_bench_utils::fixtures::TreeFixture;
    ///
    /// let fixture = TreeFixture::nested(3, 5);
    /// // /bench/
    /// //   ├── 00.rs, 01.rs, ..., 04.rs
    /// //   └── src/
    /// //       ├── 00.rs, ..., 04.rs
    /// //       └── src/
    /// //           └── 00.rs, ..., 04.rs
    /// ```
    #[must_use]
    pub fn nested(depth: usize, files_per_dir: usize) -> Self {
        let vfs = MockVfs::new();
        let root = PathBuf::from("/bench");
        vfs.add_dir(&root);

        let width = digit_width(files_per_dir);
        let mut current = root.clone();
        for level in 0..depth {
            for i in 0..files_per_dir {
                let file_path = current.join(format!("{i:0width$}.rs"));
                vfs.add_file_str(&file_path, "fn main() {}");
            }
            // Add subdirectory for next level (except last)
            if level < depth - 1 {
                let subdir = current.join("src");
                vfs.add_dir(&subdir);
                current = subdir;
            }
        }

        Self { vfs, root }
    }

    /// Realistic project layout with `src/`, `tests/`, `docs/` structure.
    ///
    /// Distributes `total_files` across directories with varied nesting:
    /// - `src/` gets 60% of files (2 levels deep)
    /// - `tests/` gets 25% of files (1 level)
    /// - `docs/` gets 15% of files (1 level)
    /// - Root also contains `Cargo.toml` and `README.md`
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_bench_utils::fixtures::TreeFixture;
    ///
    /// let fixture = TreeFixture::project(100);
    /// ```
    #[must_use]
    pub fn project(total_files: usize) -> Self {
        let vfs = MockVfs::new();
        let root = PathBuf::from("/bench");
        vfs.add_dir(&root);

        // Root-level config files
        vfs.add_file_str("/bench/Cargo.toml", "[package]\nname = \"bench\"");
        vfs.add_file_str("/bench/README.md", "# Bench");

        if total_files == 0 {
            return Self { vfs, root };
        }

        // Distribute files across directories
        let src_count = total_files * 60 / 100;
        let test_count = total_files * 25 / 100;
        let doc_count = total_files
            .saturating_sub(src_count)
            .saturating_sub(test_count);

        // src/ — split between root and a nested module/
        let src = PathBuf::from("/bench/src");
        vfs.add_dir(&src);
        let src_root_count = src_count / 2;
        let src_mod_count = src_count - src_root_count;
        let width = digit_width(src_root_count);
        for i in 0..src_root_count {
            vfs.add_file_str(src.join(format!("{i:0width$}.rs")), "fn main() {}");
        }
        if src_mod_count > 0 {
            let module_dir = src.join("module");
            vfs.add_dir(&module_dir);
            let width = digit_width(src_mod_count);
            for i in 0..src_mod_count {
                vfs.add_file_str(module_dir.join(format!("{i:0width$}.rs")), "fn main() {}");
            }
        }

        // tests/
        let tests = PathBuf::from("/bench/tests");
        vfs.add_dir(&tests);
        let width = digit_width(test_count);
        for i in 0..test_count {
            vfs.add_file_str(tests.join(format!("test_{i:0width$}.rs")), "#[test] fn t() {}");
        }

        // docs/
        let docs = PathBuf::from("/bench/docs");
        vfs.add_dir(&docs);
        let width = digit_width(doc_count);
        for i in 0..doc_count {
            vfs.add_file_str(docs.join(format!("doc_{i:0width$}.md")), "# Doc");
        }

        Self { vfs, root }
    }
}

/// Calculate the number of digits needed for zero-padded formatting.
const fn digit_width(count: usize) -> usize {
    if count <= 1 {
        return 1;
    }
    // Number of digits in (count - 1)
    let mut max_val = count - 1;
    let mut digits = 0;
    while max_val > 0 {
        digits += 1;
        max_val /= 10;
    }
    digits
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_vfs::VfsDriver};

    #[test]
    fn test_digit_width() {
        assert_eq!(digit_width(0), 1);
        assert_eq!(digit_width(1), 1);
        assert_eq!(digit_width(9), 1);
        assert_eq!(digit_width(10), 1);
        assert_eq!(digit_width(11), 2);
        assert_eq!(digit_width(100), 2);
        assert_eq!(digit_width(101), 3);
        assert_eq!(digit_width(1000), 3);
        assert_eq!(digit_width(1001), 4);
    }

    #[test]
    fn test_flat_file_names_are_zero_padded() {
        let fixture = TreeFixture::flat(15);
        // Should have files 00.rs through 14.rs (2-digit padding)
        let entries = fixture.vfs.list_dir(&fixture.root).unwrap();
        let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
        assert!(names.contains(&"00.rs".to_string()));
        assert!(names.contains(&"14.rs".to_string()));
    }

    #[test]
    fn test_nested_total_files() {
        let fixture = TreeFixture::nested(4, 3);
        // 4 levels * 3 files = 12 files total
        let count = count_files_recursive(&fixture.vfs, &fixture.root);
        assert_eq!(count, 12);
    }

    #[test]
    fn test_nested_depth_one() {
        let fixture = TreeFixture::nested(1, 5);
        // 1 level, 5 files, no subdirs
        let entries = fixture.vfs.list_dir(&fixture.root).unwrap();
        assert_eq!(entries.len(), 5);
        assert!(entries.iter().all(|e| e.is_file));
    }

    #[test]
    fn test_project_has_root_files() {
        let fixture = TreeFixture::project(50);
        let entries = fixture.vfs.list_dir(&fixture.root).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Cargo.toml"));
        assert!(names.contains(&"README.md"));
        assert!(names.contains(&"src"));
        assert!(names.contains(&"tests"));
        assert!(names.contains(&"docs"));
    }

    #[test]
    fn test_project_zero_files() {
        let fixture = TreeFixture::project(0);
        let entries = fixture.vfs.list_dir(&fixture.root).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        // Only root config files
        assert!(names.contains(&"Cargo.toml"));
        assert!(names.contains(&"README.md"));
    }

    /// Recursively count all files (not directories) under a path.
    fn count_files_recursive(vfs: &MockVfs, path: &std::path::Path) -> usize {
        let Ok(entries) = vfs.list_dir(path) else {
            return 0;
        };
        let mut count = 0;
        for entry in &entries {
            if entry.is_file {
                count += 1;
            } else if entry.is_dir {
                count += count_files_recursive(vfs, &entry.path);
            }
        }
        count
    }
}

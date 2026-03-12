use super::*;

#[test]
fn test_normalize_current_dir() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.normalize(Path::new("./foo/bar")), PathBuf::from("foo/bar"));
    assert_eq!(normalizer.normalize(Path::new("./foo/./bar")), PathBuf::from("foo/bar"));
}

#[test]
fn test_normalize_parent_dir() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.normalize(Path::new("foo/../bar")), PathBuf::from("bar"));
    assert_eq!(normalizer.normalize(Path::new("foo/baz/../bar")), PathBuf::from("foo/bar"));
}

#[test]
fn test_normalize_absolute() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.normalize(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
}

#[test]
fn test_normalize_empty() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.normalize(Path::new("")), PathBuf::from("."));
    assert_eq!(normalizer.normalize(Path::new(".")), PathBuf::from("."));
}

#[test]
fn test_normalize_relative_parent() {
    let normalizer = StandardPathNormalizer;
    // Going up from empty should preserve ..
    assert_eq!(normalizer.normalize(Path::new("../foo")), PathBuf::from("../foo"));
    assert_eq!(normalizer.normalize(Path::new("../../foo")), PathBuf::from("../../foo"));
}

#[test]
fn test_is_absolute() {
    let normalizer = StandardPathNormalizer;
    assert!(normalizer.is_absolute(Path::new("/absolute/path")));
    assert!(!normalizer.is_absolute(Path::new("relative/path")));
    assert!(!normalizer.is_absolute(Path::new("./relative")));
}

#[test]
fn test_join() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(
        normalizer.join(Path::new("/base"), Path::new("rel")),
        PathBuf::from("/base/rel")
    );
    // Absolute relative replaces base
    assert_eq!(normalizer.join(Path::new("/base"), Path::new("/abs")), PathBuf::from("/abs"));
}

#[test]
fn test_parent() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.parent(Path::new("/foo/bar")), Some(PathBuf::from("/foo")));
    assert_eq!(normalizer.parent(Path::new("/foo")), Some(PathBuf::from("/")));
    // Root path has no parent (terminates in root)
    assert_eq!(normalizer.parent(Path::new("/")), None);
}

#[test]
fn test_file_name() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.file_name(Path::new("/foo/bar.txt")), Some(OsStr::new("bar.txt")));
    assert_eq!(normalizer.file_name(Path::new("/foo/")), Some(OsStr::new("foo")));
}

#[test]
fn test_extension() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.extension(Path::new("/foo/bar.txt")), Some(OsStr::new("txt")));
    assert_eq!(normalizer.extension(Path::new("/foo/bar.tar.gz")), Some(OsStr::new("gz")));
    assert_eq!(normalizer.extension(Path::new("/foo/bar")), None);
}

#[test]
fn test_stem() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.stem(Path::new("/foo/bar.txt")), Some(OsStr::new("bar")));
    assert_eq!(normalizer.stem(Path::new("/foo/bar.tar.gz")), Some(OsStr::new("bar.tar")));
}

#[test]
fn test_to_string_lossy() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.to_string_lossy(Path::new("/foo/bar")), "/foo/bar");
}

#[test]
fn test_starts_with() {
    let normalizer = StandardPathNormalizer;
    assert!(normalizer.starts_with(Path::new("/foo/bar"), Path::new("/foo")));
    assert!(!normalizer.starts_with(Path::new("/foo/bar"), Path::new("/bar")));
}

#[test]
fn test_ends_with() {
    let normalizer = StandardPathNormalizer;
    assert!(normalizer.ends_with(Path::new("/foo/bar"), Path::new("bar")));
    assert!(!normalizer.ends_with(Path::new("/foo/bar"), Path::new("foo")));
}

#[test]
fn test_strip_prefix() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(
        normalizer.strip_prefix(Path::new("/foo/bar/baz"), Path::new("/foo")),
        Some(PathBuf::from("bar/baz"))
    );
    assert_eq!(normalizer.strip_prefix(Path::new("/foo/bar"), Path::new("/other")), None);
}

#[test]
fn test_relative_to() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(
        normalizer.relative_to(Path::new("/foo/bar/baz"), Path::new("/foo")),
        Some(PathBuf::from("bar/baz"))
    );
    assert_eq!(normalizer.relative_to(Path::new("/foo/bar"), Path::new("/other")), None);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_canonicalize_nonexistent() {
    let normalizer = StandardPathNormalizer;
    let result = normalizer.canonicalize(Path::new("/nonexistent_path_xyz_abc_123"));
    assert!(result.is_err());
    // Should produce a NotFound error specifically
    match result.unwrap_err() {
        VfsError::NotFound(p) => {
            assert_eq!(p, PathBuf::from("/nonexistent_path_xyz_abc_123"));
        }
        other => panic!("Expected VfsError::NotFound, got: {other:?}"),
    }
}

#[test]
fn test_canonicalize_io_error_non_not_found() {
    let normalizer = StandardPathNormalizer;
    // Path with component longer than NAME_MAX (255 on Linux) triggers ENAMETOOLONG
    let long_name = "a".repeat(300);
    let result = normalizer.canonicalize(Path::new(&format!("/tmp/{long_name}")));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), VfsError::Io(_)));
}

#[test]
fn test_canonicalize_existing() {
    let normalizer = StandardPathNormalizer;
    // /tmp should always exist on Linux
    let result = normalizer.canonicalize(Path::new("/tmp"));
    assert!(result.is_ok());
    assert!(result.unwrap().is_absolute());
}

#[test]
fn test_normalize_multiple_parent_dirs() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.normalize(Path::new("a/b/c/../../d")), PathBuf::from("a/d"));
}

#[test]
fn test_normalize_root_with_parent() {
    let normalizer = StandardPathNormalizer;
    // Going above root stays at root
    assert_eq!(normalizer.normalize(Path::new("/a/..")), PathBuf::from("/"));
}

#[test]
fn test_parent_of_file() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.parent(Path::new("foo/bar.txt")), Some(PathBuf::from("foo")));
}

#[test]
fn test_parent_of_single_component() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.parent(Path::new("foo")), Some(PathBuf::from("")));
}

#[test]
fn test_file_name_none_case() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.file_name(Path::new("/")), None);
    assert_eq!(normalizer.file_name(Path::new("..")), None);
}

#[test]
fn test_extension_dotfile() {
    let normalizer = StandardPathNormalizer;
    // .gitignore has no extension in Rust's Path
    assert_eq!(normalizer.extension(Path::new(".gitignore")), None);
}

#[test]
fn test_stem_no_extension() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.stem(Path::new("/foo/bar")), Some(OsStr::new("bar")));
}

#[test]
fn test_stem_with_extension() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.stem(Path::new("file.rs")), Some(OsStr::new("file")));
}

#[test]
fn test_components() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.components(Path::new("/foo/bar")).count(), 3); // RootDir, "foo", "bar"
}

#[test]
fn test_default_trait() {
    let normalizer = StandardPathNormalizer;
    assert!(normalizer.is_absolute(Path::new("/test")));
}

#[test]
fn test_normalize_only_dots() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.normalize(Path::new("./.")), PathBuf::from("."));
}

#[test]
fn test_join_with_empty() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(normalizer.join(Path::new("/base"), Path::new("")), PathBuf::from("/base"));
}

#[test]
fn test_starts_with_same_path() {
    let normalizer = StandardPathNormalizer;
    assert!(normalizer.starts_with(Path::new("/foo"), Path::new("/foo")));
}

#[test]
fn test_ends_with_full_path() {
    let normalizer = StandardPathNormalizer;
    assert!(normalizer.ends_with(Path::new("/foo/bar"), Path::new("/foo/bar")));
}

#[test]
fn test_strip_prefix_same_path() {
    let normalizer = StandardPathNormalizer;
    assert_eq!(
        normalizer.strip_prefix(Path::new("/foo"), Path::new("/foo")),
        Some(PathBuf::from(""))
    );
}

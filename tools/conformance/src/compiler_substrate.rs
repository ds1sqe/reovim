//! Exact, bounded compiler-substrate inventory.

use std::{
    collections::BTreeSet,
    fmt, fs,
    fs::{File, Metadata},
    path::{Path, PathBuf},
};

use crate::digest;

const AGGREGATE_DOMAIN: &[u8] = b"reovim-compiler-substrate/v1\0";

/// Hard limits for one compiler-substrate observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub path_bytes: usize,
    pub entry_count: usize,
    pub total_bytes: u64,
}

/// Exact roots selected by the Product execution coordinator.
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    pub compiler_support_root: &'a Path,
    pub target_libdir: &'a Path,
}

/// Why one retained substrate file is visible to rustc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryKind {
    CompilerSupport,
    TargetLibdir,
}

impl EntryKind {
    const fn tag(self) -> u8 {
        match self {
            Self::CompilerSupport => 1,
            Self::TargetLibdir => 2,
        }
    }
}

/// One exact regular file in the selected compiler substrate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    pub kind: EntryKind,
    pub canonical_path: PathBuf,
    pub content_digest: String,
    pub size_bytes: u64,
}

/// Complete deterministic observation of both selected roots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub compiler_support_root: PathBuf,
    pub target_libdir: PathBuf,
    pub entries: Vec<Entry>,
    pub entry_count: usize,
    pub total_bytes: u64,
    pub aggregate_digest: String,
}

/// Typed compiler-substrate observation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: &'static str,
    pub path: Option<PathBuf>,
    pub message: String,
}

impl Error {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, path: None, message: message.into() }
    }

    fn at(code: &'static str, path: &Path, message: impl Into<String>) -> Self {
        Self { code, path: Some(path.to_path_buf()), message: message.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.path {
            return write!(
                formatter,
                "{}: {}: {}",
                self.code,
                path.display(),
                self.message
            );
        }
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

/// Inventories the exact canonical files below both selected roots.
///
/// Zero-byte regular files are retained. Symlinks, special files, aliases,
/// overlapping roots, path escape, drift, duplicates, and exhausted limits
/// fail closed.
///
/// # Errors
///
/// Returns a typed error when an input is invalid, the filesystem cannot be
/// observed exactly, or a committed bound is exhausted.
pub fn observe(
    request: Request<'_>,
    limits: Limits,
) -> Result<Observation, Error> {
    validate_limits(limits)?;
    let support =
        canonical_directory(request.compiler_support_root, limits.path_bytes)?;
    let target = canonical_directory(request.target_libdir, limits.path_bytes)?;
    reject_overlapping_roots(&support, &target)?;

    let mut walker = Walker::new(limits);
    walker.walk_root(&support, EntryKind::CompilerSupport)?;
    walker.require_kind(EntryKind::CompilerSupport)?;
    walker.walk_root(&target, EntryKind::TargetLibdir)?;
    walker.require_kind(EntryKind::TargetLibdir)?;
    walker.finish(support, target)
}

/// Requires two independently produced observations to be identical.
///
/// # Errors
///
/// Returns `SUBSTRATE_PRE_POST_DRIFT` for any changed root, entry, byte count,
/// or aggregate identity.
pub fn require_unchanged(
    before: &Observation,
    after: &Observation,
) -> Result<(), Error> {
    if before == after {
        return Ok(());
    }
    Err(Error::new(
        "SUBSTRATE_PRE_POST_DRIFT",
        "compiler substrate changed between observations",
    ))
}

fn validate_limits(limits: Limits) -> Result<(), Error> {
    if limits.path_bytes == 0
        || limits.entry_count == 0
        || limits.total_bytes == 0
    {
        return Err(Error::new(
            "SUBSTRATE_ZERO_LIMIT",
            "substrate limits must be positive",
        ));
    }
    Ok(())
}

fn canonical_directory(
    path: &Path,
    path_limit: usize,
) -> Result<PathBuf, Error> {
    validate_absolute_path(path, path_limit)?;
    let metadata = fs::symlink_metadata(path).map_err(|failure| {
        Error::at("SUBSTRATE_DIRECTORY_METADATA", path, failure.to_string())
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::at(
            "SUBSTRATE_DIRECTORY_KIND",
            path,
            "root must be a real directory",
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|failure| {
        Error::at("SUBSTRATE_PATH_CANONICALIZE", path, failure.to_string())
    })?;
    if canonical != path {
        return Err(Error::at(
            "SUBSTRATE_PATH_NOT_CANONICAL",
            path,
            "root path differs from its canonical identity",
        ));
    }
    Ok(canonical)
}

fn validate_absolute_path(path: &Path, path_limit: usize) -> Result<(), Error> {
    let text = path.to_str().ok_or_else(|| {
        Error::at("SUBSTRATE_PATH_UTF8", path, "path is not UTF-8")
    })?;
    if text.len() > path_limit {
        return Err(Error::at(
            "SUBSTRATE_PATH_LIMIT",
            path,
            format!("path exceeds {path_limit} bytes"),
        ));
    }
    if normalized_absolute(text) {
        return Ok(());
    }
    Err(Error::at(
        "SUBSTRATE_PATH_FORM",
        path,
        "path must be normalized absolute UTF-8",
    ))
}

fn normalized_absolute(text: &str) -> bool {
    text.starts_with('/')
        && (text == "/" || !text.ends_with('/'))
        && !text.contains(['\0', '\r', '\n', '\\'])
        && !text.contains("//")
        && !text.split('/').any(|part| matches!(part, "." | ".."))
}

fn reject_overlapping_roots(
    support: &Path,
    target: &Path,
) -> Result<(), Error> {
    if support.starts_with(target) || target.starts_with(support) {
        return Err(Error::new(
            "SUBSTRATE_ROOT_OVERLAP",
            "compiler-support and target-libdir roots overlap",
        ));
    }
    Ok(())
}

struct Walker {
    limits: Limits,
    visited: usize,
    total_bytes: u64,
    seen_paths: BTreeSet<PathBuf>,
    entries: Vec<Entry>,
}

impl Walker {
    const fn new(limits: Limits) -> Self {
        Self {
            limits,
            visited: 0,
            total_bytes: 0,
            seen_paths: BTreeSet::new(),
            entries: Vec::new(),
        }
    }

    fn walk_root(&mut self, root: &Path, kind: EntryKind) -> Result<(), Error> {
        self.walk_directory(root, root, kind)
    }

    fn walk_directory(
        &mut self,
        root: &Path,
        directory: &Path,
        kind: EntryKind,
    ) -> Result<(), Error> {
        let initial = directory_metadata(directory)?;
        let children = self.read_children(directory)?;
        for child in children {
            self.visit(root, &child, kind)?;
        }
        let final_state = directory_metadata(directory)?;
        require_same_metadata(&initial, &final_state, directory)
    }

    fn read_children(
        &mut self,
        directory: &Path,
    ) -> Result<Vec<PathBuf>, Error> {
        let iterator = fs::read_dir(directory).map_err(|failure| {
            Error::at(
                "SUBSTRATE_DIRECTORY_READ",
                directory,
                failure.to_string(),
            )
        })?;
        let mut children = Vec::new();
        for result in iterator {
            self.charge_visible_entry(directory)?;
            let entry = directory_entry(result, directory)?;
            children.push(entry.path());
        }
        children.sort();
        Ok(children)
    }

    fn charge_visible_entry(&mut self, path: &Path) -> Result<(), Error> {
        self.visited = self.visited.checked_add(1).ok_or_else(|| {
            Error::at(
                "SUBSTRATE_ENTRY_OVERFLOW",
                path,
                "visible entry count overflow",
            )
        })?;
        if self.visited <= self.limits.entry_count {
            return Ok(());
        }
        Err(Error::at(
            "SUBSTRATE_ENTRY_COUNT_LIMIT",
            path,
            format!("visible entries exceed {}", self.limits.entry_count),
        ))
    }

    fn visit(
        &mut self,
        root: &Path,
        path: &Path,
        kind: EntryKind,
    ) -> Result<(), Error> {
        validate_absolute_path(path, self.limits.path_bytes)?;
        let metadata = fs::symlink_metadata(path).map_err(|failure| {
            Error::at("SUBSTRATE_ENTRY_METADATA", path, failure.to_string())
        })?;
        if metadata.file_type().is_symlink() {
            return Err(Error::at(
                "SUBSTRATE_SYMLINK",
                path,
                "symlink is outside the compiler-substrate profile",
            ));
        }
        validate_child_identity(root, path)?;
        if metadata.is_dir() {
            return self.walk_directory(root, path, kind);
        }
        if !metadata.is_file() {
            return Err(Error::at(
                "SUBSTRATE_NONREGULAR",
                path,
                "entry is neither a directory nor a regular file",
            ));
        }
        self.observe_file(path, kind, &metadata)
    }

    fn observe_file(
        &mut self,
        path: &Path,
        kind: EntryKind,
        initial: &Metadata,
    ) -> Result<(), Error> {
        if !self.seen_paths.insert(path.to_path_buf()) {
            return Err(Error::at(
                "SUBSTRATE_FILE_DUPLICATE",
                path,
                "canonical file path is duplicated",
            ));
        }
        let size = initial.len();
        self.charge_file_bytes(path, size)?;
        let file = File::open(path).map_err(|failure| {
            Error::at("SUBSTRATE_FILE_OPEN", path, failure.to_string())
        })?;
        require_same_metadata(
            initial,
            &file.metadata().map_err(|failure| {
                Error::at("SUBSTRATE_FILE_METADATA", path, failure.to_string())
            })?,
            path,
        )?;
        let digest =
            digest::file_handle_bounded(&file, size).map_err(|failure| {
                Error::at("SUBSTRATE_FILE_DIGEST", path, failure.to_string())
            })?;
        require_file_stable(path, initial, &file)?;
        self.entries.push(Entry {
            kind,
            canonical_path: path.to_path_buf(),
            content_digest: digest,
            size_bytes: size,
        });
        Ok(())
    }

    fn charge_file_bytes(
        &mut self,
        path: &Path,
        size: u64,
    ) -> Result<(), Error> {
        let total = self.total_bytes.checked_add(size).ok_or_else(|| {
            Error::at(
                "SUBSTRATE_TOTAL_BYTE_OVERFLOW",
                path,
                "substrate byte count overflow",
            )
        })?;
        if total > self.limits.total_bytes {
            return Err(Error::at(
                "SUBSTRATE_TOTAL_BYTE_LIMIT",
                path,
                format!("substrate bytes exceed {}", self.limits.total_bytes),
            ));
        }
        self.total_bytes = total;
        Ok(())
    }

    fn require_kind(&self, kind: EntryKind) -> Result<(), Error> {
        if self.entries.iter().any(|entry| entry.kind == kind) {
            return Ok(());
        }
        Err(Error::new(
            "SUBSTRATE_ROOT_EMPTY",
            "each substrate root must contain a regular file",
        ))
    }

    fn finish(
        mut self,
        support: PathBuf,
        target: PathBuf,
    ) -> Result<Observation, Error> {
        self.entries.sort();
        if self.entries.len() > self.limits.entry_count {
            return Err(Error::new(
                "SUBSTRATE_FILE_COUNT_LIMIT",
                "retained file count exceeds the entry limit",
            ));
        }
        let aggregate_digest = aggregate_digest(&self.entries)?;
        Ok(Observation {
            compiler_support_root: support,
            target_libdir: target,
            entry_count: self.entries.len(),
            total_bytes: self.total_bytes,
            entries: self.entries,
            aggregate_digest,
        })
    }
}

fn directory_entry(
    result: std::io::Result<fs::DirEntry>,
    directory: &Path,
) -> Result<fs::DirEntry, Error> {
    result.map_err(|failure| {
        Error::at("SUBSTRATE_DIRECTORY_ENTRY", directory, failure.to_string())
    })
}

fn directory_metadata(path: &Path) -> Result<Metadata, Error> {
    let metadata = fs::symlink_metadata(path).map_err(|failure| {
        Error::at("SUBSTRATE_DIRECTORY_RECHECK", path, failure.to_string())
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::at(
            "SUBSTRATE_DIRECTORY_DRIFT",
            path,
            "directory identity changed during traversal",
        ));
    }
    Ok(metadata)
}

fn require_file_stable(
    path: &Path,
    initial: &Metadata,
    file: &File,
) -> Result<(), Error> {
    let held = file.metadata().map_err(|failure| {
        Error::at("SUBSTRATE_FILE_RECHECK", path, failure.to_string())
    })?;
    let named = fs::symlink_metadata(path).map_err(|failure| {
        Error::at("SUBSTRATE_FILE_RECHECK", path, failure.to_string())
    })?;
    require_same_metadata(initial, &held, path)?;
    require_same_metadata(initial, &named, path)
}

fn validate_child_identity(root: &Path, path: &Path) -> Result<(), Error> {
    let canonical = fs::canonicalize(path).map_err(|failure| {
        Error::at("SUBSTRATE_PATH_CANONICALIZE", path, failure.to_string())
    })?;
    if canonical != path {
        return Err(Error::at(
            "SUBSTRATE_PATH_NOT_CANONICAL",
            path,
            "entry path differs from its canonical identity",
        ));
    }
    if canonical.starts_with(root) {
        return Ok(());
    }
    Err(Error::at(
        "SUBSTRATE_PATH_ESCAPE",
        path,
        "canonical entry escapes its selected root",
    ))
}

fn require_same_metadata(
    expected: &Metadata,
    actual: &Metadata,
    path: &Path,
) -> Result<(), Error> {
    if same_metadata(expected, actual) {
        return Ok(());
    }
    Err(Error::at(
        "SUBSTRATE_FILE_DRIFT",
        path,
        "file identity changed while it was observed",
    ))
}

#[cfg(unix)]
fn same_metadata(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn same_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.file_type() == right.file_type()
        && left.permissions() == right.permissions()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}

fn aggregate_digest(entries: &[Entry]) -> Result<String, Error> {
    let mut input = Vec::new();
    input.extend_from_slice(AGGREGATE_DOMAIN);
    append_u64(&mut input, entries.len())?;
    for entry in entries {
        append_entry(&mut input, entry)?;
    }
    Ok(digest::bytes(&input))
}

fn append_entry(input: &mut Vec<u8>, entry: &Entry) -> Result<(), Error> {
    let path = entry.canonical_path.to_str().ok_or_else(|| {
        Error::at(
            "SUBSTRATE_PATH_UTF8",
            &entry.canonical_path,
            "retained path is not UTF-8",
        )
    })?;
    input.push(entry.kind.tag());
    append_bytes(input, path.as_bytes())?;
    append_u64_value(input, entry.size_bytes);
    append_bytes(input, entry.content_digest.as_bytes())
}

fn append_bytes(input: &mut Vec<u8>, value: &[u8]) -> Result<(), Error> {
    append_u64(input, value.len())?;
    input.extend_from_slice(value);
    Ok(())
}

fn append_u64(input: &mut Vec<u8>, value: usize) -> Result<(), Error> {
    let value = u64::try_from(value).map_err(|_| {
        Error::new(
            "SUBSTRATE_AGGREGATE_OVERFLOW",
            "aggregate field length does not fit u64",
        )
    })?;
    append_u64_value(input, value);
    Ok(())
}

fn append_u64_value(input: &mut Vec<u8>, value: u64) {
    input.extend_from_slice(&value.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{EntryKind, Limits, Request};

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "reovim-substrate-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&root).unwrap();
            Self(root.canonicalize().unwrap())
        }

        fn directory(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir_all(&path).unwrap();
            path
        }

        fn write(&self, relative: &str, bytes: &[u8]) {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn limits() -> Limits {
        Limits { path_bytes: 4096, entry_count: 32, total_bytes: 1024 }
    }

    fn request<'a>(support: &'a Path, target: &'a Path) -> Request<'a> {
        Request { compiler_support_root: support, target_libdir: target }
    }

    #[test]
    fn observes_sorted_files_and_preserves_zero_byte_entries() {
        let tree = TempTree::new();
        let support = tree.directory("support");
        let target = tree.directory("target");
        tree.write("support/z.so", b"support");
        tree.write("support/a.empty", b"");
        tree.write("target/libcore.rlib", b"target");

        let first =
            super::observe(request(&support, &target), limits()).unwrap();
        let second =
            super::observe(request(&support, &target), limits()).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.entry_count, 3);
        assert_eq!(first.total_bytes, 13);
        assert_eq!(first.entries[0].canonical_path, support.join("a.empty"));
        assert_eq!(first.entries[0].size_bytes, 0);
        assert_eq!(first.entries[0].kind, EntryKind::CompilerSupport);
        assert_eq!(first.entries[2].kind, EntryKind::TargetLibdir);
    }

    #[test]
    fn rejects_overlapping_roots_and_exhausted_bounds() {
        let tree = TempTree::new();
        let support = tree.directory("support");
        let target = tree.directory("support/target");
        tree.write("support/compiler", b"1234");
        tree.write("support/target/lib.rlib", b"5678");

        let overlap =
            super::observe(request(&support, &target), limits()).unwrap_err();
        assert_eq!(overlap.code, "SUBSTRATE_ROOT_OVERLAP");

        let separate = tree.directory("separate");
        tree.write("separate/lib.rlib", b"5678");
        let mut bounded = limits();
        bounded.total_bytes = 7;
        let bytes =
            super::observe(request(&support, &separate), bounded).unwrap_err();
        assert_eq!(bytes.code, "SUBSTRATE_TOTAL_BYTE_LIMIT");

        bounded = limits();
        bounded.entry_count = 1;
        let entries =
            super::observe(request(&support, &separate), bounded).unwrap_err();
        assert_eq!(entries.code, "SUBSTRATE_ENTRY_COUNT_LIMIT");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_and_pre_post_drift() {
        use std::os::unix::fs::symlink;

        let tree = TempTree::new();
        let support = tree.directory("support");
        let target = tree.directory("target");
        tree.write("support/compiler", b"first");
        tree.write("target/lib.rlib", b"target");

        let before =
            super::observe(request(&support, &target), limits()).unwrap();
        tree.write("support/compiler", b"other");
        let after =
            super::observe(request(&support, &target), limits()).unwrap();
        let drift = super::require_unchanged(&before, &after).unwrap_err();
        assert_eq!(drift.code, "SUBSTRATE_PRE_POST_DRIFT");

        symlink("compiler", support.join("alias")).unwrap();
        let link =
            super::observe(request(&support, &target), limits()).unwrap_err();
        assert_eq!(link.code, "SUBSTRATE_SYMLINK");
    }
}

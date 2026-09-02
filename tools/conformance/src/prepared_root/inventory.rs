//! Exact bounded exposure-source inventory.

use std::{
    collections::BTreeSet,
    fs,
    fs::{File, Metadata},
    path::{Path, PathBuf},
};

use crate::digest;

use super::model::{
    Error, ExposureKind, InventoryEntry, InventoryEntryKind, InventoryIdentity,
    InventoryLimits,
};

const INVENTORY_DOMAIN: &[u8] = b"reovim-exposure-inventory/v1\0";

impl InventoryEntryKind {
    const fn tag(self) -> u8 {
        match self {
            Self::File => 1,
            Self::Directory => 2,
            Self::CharacterDevice => 3,
        }
    }
}

pub(super) fn observe(
    path: &Path,
    kind: ExposureKind,
    limits: InventoryLimits,
) -> Result<InventoryIdentity, Error> {
    validate_limits(limits)?;
    let root = canonical_root(path, kind, limits.path_bytes)?;
    let mut observer = Observer::new(root, limits);
    observer.observe_root(kind)?;
    observer.finish()
}

fn validate_limits(limits: InventoryLimits) -> Result<(), Error> {
    if limits.path_bytes == 0
        || limits.entry_count == 0
        || limits.total_bytes == 0
    {
        return Err(Error::new(
            "PREPARED_INVENTORY_ZERO_LIMIT",
            "inventory limits must be positive",
        ));
    }
    Ok(())
}

fn canonical_root(
    path: &Path,
    kind: ExposureKind,
    path_limit: usize,
) -> Result<PathBuf, Error> {
    validate_absolute(path, path_limit)?;
    let metadata = fs::symlink_metadata(path).map_err(|failure| {
        Error::at(
            "PREPARED_INVENTORY_METADATA",
            path.display().to_string(),
            failure.to_string(),
        )
    })?;
    if metadata.file_type().is_symlink() || !matches_kind(&metadata, kind) {
        return Err(Error::at(
            "PREPARED_INVENTORY_ROOT_KIND",
            path.display().to_string(),
            "exposure source differs from its declared kind",
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|failure| {
        Error::at(
            "PREPARED_INVENTORY_CANONICALIZE",
            path.display().to_string(),
            failure.to_string(),
        )
    })?;
    if canonical == path {
        return Ok(canonical);
    }
    Err(Error::at(
        "PREPARED_INVENTORY_PATH_ALIAS",
        path.display().to_string(),
        "exposure source is not its canonical path",
    ))
}

fn matches_kind(metadata: &Metadata, kind: ExposureKind) -> bool {
    match kind {
        ExposureKind::File => metadata.is_file(),
        ExposureKind::Directory => metadata.is_dir(),
        ExposureKind::CharacterDevice => character_device(metadata),
    }
}

fn validate_absolute(path: &Path, limit: usize) -> Result<(), Error> {
    let text = path.to_str().ok_or_else(|| {
        Error::at(
            "PREPARED_INVENTORY_PATH_UTF8",
            path.display().to_string(),
            "path is not UTF-8",
        )
    })?;
    let normalized = text.starts_with('/')
        && (text == "/" || !text.ends_with('/'))
        && !text.contains(['\0', '\r', '\n', '\\'])
        && !text.contains("//")
        && !text.split('/').any(|part| matches!(part, "." | ".."));
    if normalized && text.len() <= limit {
        return Ok(());
    }
    Err(Error::at(
        "PREPARED_INVENTORY_PATH_FORM",
        text,
        format!("path is not normalized or exceeds {limit} bytes"),
    ))
}

struct Observer {
    root: PathBuf,
    limits: InventoryLimits,
    visited: usize,
    total_bytes: u64,
    paths: BTreeSet<PathBuf>,
    entries: Vec<InventoryEntry>,
}

impl Observer {
    const fn new(root: PathBuf, limits: InventoryLimits) -> Self {
        Self {
            root,
            limits,
            visited: 0,
            total_bytes: 0,
            paths: BTreeSet::new(),
            entries: Vec::new(),
        }
    }

    fn observe_root(&mut self, kind: ExposureKind) -> Result<(), Error> {
        let root = self.root.clone();
        self.charge_entry(&root)?;
        match kind {
            ExposureKind::File => self.observe_file(&root),
            ExposureKind::Directory => self.observe_directory(&root),
            ExposureKind::CharacterDevice => self.observe_device(&root),
        }
    }

    fn observe_directory(&mut self, directory: &Path) -> Result<(), Error> {
        self.record_directory(directory)?;
        let initial = directory_metadata(directory)?;
        let children = self.read_children(directory)?;
        for child in children {
            self.observe_child(&child)?;
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
                "PREPARED_INVENTORY_DIRECTORY_READ",
                directory.display().to_string(),
                failure.to_string(),
            )
        })?;
        let mut children = Vec::new();
        for result in iterator {
            self.charge_entry(directory)?;
            children.push(directory_entry(result, directory)?.path());
        }
        children.sort();
        Ok(children)
    }

    fn charge_entry(&mut self, path: &Path) -> Result<(), Error> {
        self.visited = self.visited.checked_add(1).ok_or_else(|| {
            Error::at(
                "PREPARED_INVENTORY_ENTRY_OVERFLOW",
                path.display().to_string(),
                "entry count overflow",
            )
        })?;
        if self.visited <= self.limits.entry_count {
            return Ok(());
        }
        Err(Error::at(
            "PREPARED_INVENTORY_ENTRY_LIMIT",
            path.display().to_string(),
            format!("entries reach {}", self.limits.entry_count),
        ))
    }

    fn observe_child(&mut self, path: &Path) -> Result<(), Error> {
        validate_child(&self.root, path, self.limits.path_bytes)?;
        let metadata = fs::symlink_metadata(path).map_err(|failure| {
            Error::at(
                "PREPARED_INVENTORY_ENTRY_METADATA",
                path.display().to_string(),
                failure.to_string(),
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(Error::at(
                "PREPARED_INVENTORY_SYMLINK",
                path.display().to_string(),
                "symlink is outside the exposure profile",
            ));
        }
        if metadata.is_dir() {
            return self.observe_directory(path);
        }
        if metadata.is_file() {
            return self.observe_file(path);
        }
        Err(Error::at(
            "PREPARED_INVENTORY_NONREGULAR",
            path.display().to_string(),
            "entry is not a regular file or directory",
        ))
    }

    fn record_directory(&mut self, path: &Path) -> Result<(), Error> {
        let relative = self.relative(path)?;
        let metadata = directory_metadata(path)?;
        self.insert_path(&relative)?;
        self.entries.push(InventoryEntry {
            relative_path: relative,
            kind: InventoryEntryKind::Directory,
            mode: file_mode(&metadata),
            size_bytes: 0,
            content_digest: None,
            device_identity: None,
        });
        self.require_retained_count(path)
    }

    fn observe_file(&mut self, path: &Path) -> Result<(), Error> {
        let relative = self.relative(path)?;
        self.insert_path(&relative)?;
        let initial = fs::symlink_metadata(path).map_err(|failure| {
            Error::at(
                "PREPARED_INVENTORY_FILE_METADATA",
                path.display().to_string(),
                failure.to_string(),
            )
        })?;
        if initial.file_type().is_symlink() || !initial.is_file() {
            return Err(Error::at(
                "PREPARED_INVENTORY_FILE_KIND",
                path.display().to_string(),
                "file identity changed before observation",
            ));
        }
        self.charge_bytes(path, initial.len())?;
        let file = File::open(path).map_err(|failure| {
            Error::at(
                "PREPARED_INVENTORY_FILE_OPEN",
                path.display().to_string(),
                failure.to_string(),
            )
        })?;
        require_same_metadata(&initial, &file_metadata(&file, path)?, path)?;
        let content_digest = digest::file_handle_bounded(&file, initial.len())
            .map_err(|failure| {
                Error::at(
                    "PREPARED_INVENTORY_FILE_DIGEST",
                    path.display().to_string(),
                    failure.to_string(),
                )
            })?;
        require_file_stable(path, &initial, &file)?;
        self.entries.push(InventoryEntry {
            relative_path: relative,
            kind: InventoryEntryKind::File,
            mode: file_mode(&initial),
            size_bytes: initial.len(),
            content_digest: Some(content_digest),
            device_identity: None,
        });
        self.require_retained_count(path)
    }

    fn observe_device(&mut self, path: &Path) -> Result<(), Error> {
        let relative = self.relative(path)?;
        self.insert_path(&relative)?;
        let metadata = fs::symlink_metadata(path).map_err(|failure| {
            Error::at(
                "PREPARED_INVENTORY_DEVICE_METADATA",
                path.display().to_string(),
                failure.to_string(),
            )
        })?;
        if !character_device(&metadata) || !null_device(&metadata) {
            return Err(Error::at(
                "PREPARED_DEVICE_IDENTITY",
                path.display().to_string(),
                "only the exact null character device is admitted",
            ));
        }
        self.entries.push(InventoryEntry {
            relative_path: relative,
            kind: InventoryEntryKind::CharacterDevice,
            mode: file_mode(&metadata),
            size_bytes: 0,
            content_digest: None,
            device_identity: Some(device_identity(&metadata)),
        });
        self.require_retained_count(path)
    }

    fn relative(&self, path: &Path) -> Result<PathBuf, Error> {
        if path == self.root {
            return Ok(PathBuf::from("."));
        }
        path.strip_prefix(&self.root).map(Path::to_path_buf).map_err(
            |failure| {
                Error::at(
                    "PREPARED_INVENTORY_PATH_ESCAPE",
                    path.display().to_string(),
                    failure.to_string(),
                )
            },
        )
    }

    fn insert_path(&mut self, path: &Path) -> Result<(), Error> {
        if self.paths.insert(path.to_path_buf()) {
            return Ok(());
        }
        Err(Error::at(
            "PREPARED_INVENTORY_DUPLICATE",
            path.display().to_string(),
            "relative path is duplicated",
        ))
    }

    fn charge_bytes(&mut self, path: &Path, bytes: u64) -> Result<(), Error> {
        let total = self.total_bytes.checked_add(bytes).ok_or_else(|| {
            Error::at(
                "PREPARED_INVENTORY_BYTE_OVERFLOW",
                path.display().to_string(),
                "inventory byte count overflow",
            )
        })?;
        if total > self.limits.total_bytes {
            return Err(Error::at(
                "PREPARED_INVENTORY_BYTE_LIMIT",
                path.display().to_string(),
                format!("inventory bytes exceed {}", self.limits.total_bytes),
            ));
        }
        self.total_bytes = total;
        Ok(())
    }

    fn require_retained_count(&self, path: &Path) -> Result<(), Error> {
        if self.entries.len() <= self.limits.entry_count {
            return Ok(());
        }
        Err(Error::at(
            "PREPARED_INVENTORY_ENTRY_LIMIT",
            path.display().to_string(),
            format!("entries exceed {}", self.limits.entry_count),
        ))
    }

    fn finish(mut self) -> Result<InventoryIdentity, Error> {
        self.entries.sort();
        let aggregate_digest = aggregate_digest(&self.entries)?;
        Ok(InventoryIdentity {
            entry_count: self.entries.len(),
            total_bytes: self.total_bytes,
            aggregate_digest,
            entries: self.entries,
        })
    }
}

fn validate_child(root: &Path, path: &Path, limit: usize) -> Result<(), Error> {
    validate_absolute(path, limit)?;
    let canonical = fs::canonicalize(path).map_err(|failure| {
        Error::at(
            "PREPARED_INVENTORY_CANONICALIZE",
            path.display().to_string(),
            failure.to_string(),
        )
    })?;
    if canonical == path && canonical.starts_with(root) {
        return Ok(());
    }
    Err(Error::at(
        "PREPARED_INVENTORY_PATH_ESCAPE",
        path.display().to_string(),
        "entry is aliased or escapes its exposure root",
    ))
}

fn directory_entry(
    result: std::io::Result<fs::DirEntry>,
    directory: &Path,
) -> Result<fs::DirEntry, Error> {
    result.map_err(|failure| {
        Error::at(
            "PREPARED_INVENTORY_DIRECTORY_ENTRY",
            directory.display().to_string(),
            failure.to_string(),
        )
    })
}

fn directory_metadata(path: &Path) -> Result<Metadata, Error> {
    let metadata = fs::symlink_metadata(path).map_err(|failure| {
        Error::at(
            "PREPARED_INVENTORY_DIRECTORY_RECHECK",
            path.display().to_string(),
            failure.to_string(),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::at(
            "PREPARED_INVENTORY_DIRECTORY_DRIFT",
            path.display().to_string(),
            "directory identity changed during traversal",
        ));
    }
    Ok(metadata)
}

fn file_metadata(file: &File, path: &Path) -> Result<Metadata, Error> {
    file.metadata().map_err(|failure| {
        Error::at(
            "PREPARED_INVENTORY_FILE_METADATA",
            path.display().to_string(),
            failure.to_string(),
        )
    })
}

fn require_file_stable(
    path: &Path,
    initial: &Metadata,
    file: &File,
) -> Result<(), Error> {
    let held = file_metadata(file, path)?;
    let named = fs::symlink_metadata(path).map_err(|failure| {
        Error::at(
            "PREPARED_INVENTORY_FILE_RECHECK",
            path.display().to_string(),
            failure.to_string(),
        )
    })?;
    require_same_metadata(initial, &held, path)?;
    require_same_metadata(initial, &named, path)
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
        "PREPARED_INVENTORY_DRIFT",
        path.display().to_string(),
        "filesystem identity changed during observation",
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

#[cfg(unix)]
fn file_mode(metadata: &Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt as _;

    metadata.mode()
}

#[cfg(not(unix))]
const fn file_mode(_metadata: &Metadata) -> u32 {
    0
}

pub(super) fn validate_identity(
    identity: &InventoryIdentity,
) -> Result<(), Error> {
    let ordered = identity.entries.windows(2).all(|pair| pair[0] < pair[1]);
    let mut total = 0_u64;
    let mut paths = BTreeSet::new();
    for entry in &identity.entries {
        validate_retained_entry(entry)?;
        if !paths.insert(&entry.relative_path) {
            return Err(Error::new(
                "PREPARED_INVENTORY_DUPLICATE",
                "retained inventory path is duplicated",
            ));
        }
        total = total.checked_add(entry.size_bytes).ok_or_else(|| {
            Error::new(
                "PREPARED_INVENTORY_BYTE_OVERFLOW",
                "retained inventory bytes overflow",
            )
        })?;
    }
    let digest = aggregate_digest(&identity.entries)?;
    if !identity.entries.is_empty()
        && identity.entries[0].relative_path == Path::new(".")
        && ordered
        && identity.entry_count == identity.entries.len()
        && identity.total_bytes == total
        && identity.aggregate_digest == digest
    {
        return Ok(());
    }
    Err(Error::new(
        "PREPARED_INVENTORY_IDENTITY",
        "retained inventory entries, totals, or digest differ",
    ))
}

#[cfg(test)]
pub(super) fn identity_from_entries(
    mut entries: Vec<InventoryEntry>,
) -> Result<InventoryIdentity, Error> {
    entries.sort();
    let total_bytes = entries.iter().try_fold(0_u64, |total, entry| {
        total.checked_add(entry.size_bytes).ok_or_else(|| {
            Error::new(
                "PREPARED_INVENTORY_BYTE_OVERFLOW",
                "retained inventory bytes overflow",
            )
        })
    })?;
    let aggregate_digest = aggregate_digest(&entries)?;
    let identity = InventoryIdentity {
        entry_count: entries.len(),
        total_bytes,
        aggregate_digest,
        entries,
    };
    validate_identity(&identity)?;
    Ok(identity)
}

fn validate_retained_entry(entry: &InventoryEntry) -> Result<(), Error> {
    let path = entry.relative_path.to_str().ok_or_else(|| {
        Error::new(
            "PREPARED_INVENTORY_PATH_UTF8",
            "retained inventory path is not UTF-8",
        )
    })?;
    let normalized = path == "."
        || (!path.is_empty()
            && !path.starts_with('/')
            && !path.ends_with('/')
            && !path.contains(['\0', '\r', '\n', '\\'])
            && !path.contains("//")
            && path.split('/').all(|part| !matches!(part, "." | ".." | "")));
    let shape = match entry.kind {
        InventoryEntryKind::File => {
            entry.content_digest.is_some() && entry.device_identity.is_none()
        }
        InventoryEntryKind::Directory => {
            entry.size_bytes == 0
                && entry.content_digest.is_none()
                && entry.device_identity.is_none()
        }
        InventoryEntryKind::CharacterDevice => {
            entry.size_bytes == 0
                && entry.content_digest.is_none()
                && entry.device_identity.is_some()
        }
    };
    if normalized && entry.mode != 0 && shape {
        return Ok(());
    }
    Err(Error::at(
        "PREPARED_INVENTORY_ENTRY_IDENTITY",
        path,
        "retained inventory entry has invalid path, mode, or kind fields",
    ))
}

pub(super) fn aggregate_digest(
    entries: &[InventoryEntry],
) -> Result<String, Error> {
    let mut input = Vec::new();
    input.extend_from_slice(INVENTORY_DOMAIN);
    append_length(&mut input, entries.len())?;
    for entry in entries {
        append_entry(&mut input, entry)?;
    }
    Ok(digest::bytes(&input))
}

fn append_entry(
    input: &mut Vec<u8>,
    entry: &InventoryEntry,
) -> Result<(), Error> {
    let path = entry.relative_path.to_str().ok_or_else(|| {
        Error::at(
            "PREPARED_INVENTORY_PATH_UTF8",
            entry.relative_path.display().to_string(),
            "relative path is not UTF-8",
        )
    })?;
    input.push(entry.kind.tag());
    append_bytes(input, path.as_bytes())?;
    input.extend_from_slice(&entry.mode.to_be_bytes());
    input.extend_from_slice(&entry.size_bytes.to_be_bytes());
    let digest = entry.content_digest.as_deref().unwrap_or("");
    append_bytes(input, digest.as_bytes())?;
    let device = entry.device_identity.unwrap_or(0);
    input.extend_from_slice(&device.to_be_bytes());
    Ok(())
}

fn append_bytes(input: &mut Vec<u8>, value: &[u8]) -> Result<(), Error> {
    append_length(input, value.len())?;
    input.extend_from_slice(value);
    Ok(())
}

fn append_length(input: &mut Vec<u8>, length: usize) -> Result<(), Error> {
    let length = u64::try_from(length).map_err(|_| {
        Error::new(
            "PREPARED_INVENTORY_LENGTH_OVERFLOW",
            "aggregate field length does not fit u64",
        )
    })?;
    input.extend_from_slice(&length.to_be_bytes());
    Ok(())
}

#[cfg(unix)]
fn character_device(metadata: &Metadata) -> bool {
    use std::os::unix::fs::FileTypeExt as _;

    metadata.file_type().is_char_device()
}

#[cfg(not(unix))]
const fn character_device(_metadata: &Metadata) -> bool {
    false
}

#[cfg(unix)]
fn device_identity(metadata: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;

    metadata.rdev()
}

#[cfg(not(unix))]
const fn device_identity(_metadata: &Metadata) -> u64 {
    0
}

#[cfg(target_os = "linux")]
fn null_device(metadata: &Metadata) -> bool {
    let device = device_identity(metadata);
    nix::sys::stat::major(device) == 1 && nix::sys::stat::minor(device) == 3
}

#[cfg(not(target_os = "linux"))]
const fn null_device(_metadata: &Metadata) -> bool {
    false
}

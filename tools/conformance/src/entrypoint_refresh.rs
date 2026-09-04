//! Bounded refresh of one reviewed repository entrypoint.

use std::{
    fmt, fs,
    io::Write as _,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    ENTRYPOINT_REVIEWS_KEY, TOPOLOGY_KEY, digest,
    entrypoint_reviews::{Admission, Registry, ReviewedEntrypoint},
    json,
    policy::relative_path,
    source::read_bounded,
    topology::{HARD_LIMITS, RepositoryEntrypoint, Topology},
};

const RESPONSE_SCHEMA: &str = "reovim-entrypoint-refresh/v1";
const REVIEW_MARKER: &str = "- Exact content digest:\n";
const MAX_UTC_SECONDS: u64 = 253_402_300_799;

/// Request to refresh one registered script or workflow review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrypointRefreshRequest {
    /// Absolute repository root containing the fixed registries.
    pub root: PathBuf,
    /// Normalized repository-relative registered entrypoint path.
    pub entrypoint: PathBuf,
}

/// Successfully applied entrypoint refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrypointRefreshOutcome {
    /// Response schema emitted by the command.
    pub schema: &'static str,
    /// Refreshed repository-relative entrypoint path.
    pub entrypoint: PathBuf,
    /// Digest of the current entrypoint bytes.
    pub content_digest: String,
    /// Registry-derived focused review record.
    pub review_record: PathBuf,
    /// Digest of the refreshed review record.
    pub review_record_digest: String,
    /// Single UTC timestamp applied to the review registry.
    pub reviewed_at: String,
}

/// Mutation certainty when an entrypoint refresh fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshEffect {
    /// Every candidate replaced its target; only response delivery failed.
    Applied,
    /// Candidate validation or staging failed before replacement.
    DefinitelyNotApplied,
    /// Replacement failed and complete rollback could not be proved.
    OutcomeUnknown,
}

impl RefreshEffect {
    /// Stable response spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::DefinitelyNotApplied => "definitely_not_applied",
            Self::OutcomeUnknown => "outcome_unknown",
        }
    }
}

/// Typed refresh failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrypointRefreshError {
    /// Stable failure category.
    pub code: &'static str,
    /// Human-readable bounded diagnostic.
    pub message: String,
    /// Whether tracked output may have changed.
    pub effect: RefreshEffect,
}

impl fmt::Display for EntrypointRefreshError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for EntrypointRefreshError {}

/// Refreshes one exact registered entrypoint and its linked review records.
///
/// # Errors
///
/// Returns a typed failure when selection, bounded reads, canonical markers,
/// candidate validation, staging, replacement, rollback, or sync fails.
pub fn refresh_entrypoint_review(
    request: &EntrypointRefreshRequest,
) -> Result<EntrypointRefreshOutcome, EntrypointRefreshError> {
    validate_request(request)?;
    let topology_path = request.root.join(TOPOLOGY_KEY);
    let registry_path = request.root.join(ENTRYPOINT_REVIEWS_KEY);
    let (topology_bytes, topology_value) = read_json(&topology_path)?;
    let topology = Topology::parse(&topology_value).map_err(config_error)?;
    let (registry_bytes, registry_value) = read_json(&registry_path)?;
    let registry = Registry::parse(&registry_value).map_err(config_error)?;
    let selected = select(&registry, &topology, &request.entrypoint)?;

    let source_path = request.root.join(&request.entrypoint);
    let source_bytes = read_regular(&source_path)?;
    let content_digest = digest::bytes(&source_bytes);
    let record_path = request.root.join(&selected.review_record);
    let record_bytes = read_regular(&record_path)?;
    let next_record = refresh_review_marker(&record_bytes, &content_digest)?;
    let review_record_digest = digest::bytes(&next_record);
    let reviewed_at = utc_timestamp(SystemTime::now())?;

    let next_reviews = refresh_registry(
        &registry_bytes,
        &registry,
        &selected,
        &content_digest,
        &review_record_digest,
        &reviewed_at,
    )?;
    let next_topology =
        refresh_topology(&topology_bytes, &selected, &content_digest)?;
    validate_candidates(
        &next_reviews,
        &next_topology,
        &selected,
        &content_digest,
        &review_record_digest,
        &reviewed_at,
    )?;

    let updates = vec![
        FileUpdate::new(record_path, record_bytes, next_record),
        FileUpdate::new(registry_path, registry_bytes, next_reviews),
        FileUpdate::new(topology_path, topology_bytes, next_topology),
    ];
    publish_updates(&updates)?;

    Ok(EntrypointRefreshOutcome {
        schema: RESPONSE_SCHEMA,
        entrypoint: request.entrypoint.clone(),
        content_digest,
        review_record: selected.review_record,
        review_record_digest,
        reviewed_at,
    })
}

fn validate_request(
    request: &EntrypointRefreshRequest,
) -> Result<(), EntrypointRefreshError> {
    if !request.root.is_absolute() {
        return Err(not_applied(
            "ENTRYPOINT_REFRESH_ROOT",
            "--root must be absolute",
        ));
    }
    let root = fs::symlink_metadata(&request.root).map_err(|error| {
        io_error("ENTRYPOINT_REFRESH_ROOT", &request.root, &error)
    })?;
    if !root.is_dir() || root.file_type().is_symlink() {
        return Err(not_applied(
            "ENTRYPOINT_REFRESH_ROOT",
            "--root must name a non-symlink directory",
        ));
    }
    let raw = request.entrypoint.to_str().ok_or_else(|| {
        not_applied("ENTRYPOINT_REFRESH_PATH", "--path must be valid UTF-8")
    })?;
    let normalized = relative_path(raw, "--path", false).map_err(|error| {
        not_applied("ENTRYPOINT_REFRESH_PATH", error.to_string())
    })?;
    if normalized != request.entrypoint {
        return Err(not_applied(
            "ENTRYPOINT_REFRESH_PATH",
            "--path must use its normalized repository-relative form",
        ));
    }
    Ok(())
}

fn read_json(
    path: &Path,
) -> Result<(Vec<u8>, serde_json::Value), EntrypointRefreshError> {
    require_regular(path)?;
    json::read(
        path,
        HARD_LIMITS.config_bytes,
        HARD_LIMITS.json_depth,
        HARD_LIMITS.json_container_items,
    )
    .map_err(config_error)
}

fn read_regular(path: &Path) -> Result<Vec<u8>, EntrypointRefreshError> {
    require_regular(path)?;
    read_bounded(path, HARD_LIMITS.source_file_bytes).map_err(|message| {
        not_applied(
            "ENTRYPOINT_REFRESH_READ",
            format!("{}: {message}", path.display()),
        )
    })
}

fn require_regular(path: &Path) -> Result<(), EntrypointRefreshError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error("ENTRYPOINT_REFRESH_INPUT", path, &error))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(not_applied(
            "ENTRYPOINT_REFRESH_INPUT",
            format!("{} is not a regular non-symlink file", path.display()),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct SelectedReview {
    entrypoint: PathBuf,
    content_digest: String,
    review_record: PathBuf,
    review_record_digest: String,
}

fn select(
    registry: &Registry,
    topology: &Topology,
    path: &Path,
) -> Result<SelectedReview, EntrypointRefreshError> {
    let admission = registry
        .reviews
        .iter()
        .find(|review| reviewed_path(review) == path)
        .ok_or_else(|| {
            not_applied(
                "ENTRYPOINT_REFRESH_UNREGISTERED",
                format!("{} has no exact entrypoint review", path.display()),
            )
        })?;
    let topology_digest = topology
        .repository_entrypoints
        .iter()
        .find_map(|entrypoint| topology_digest(entrypoint, path))
        .ok_or_else(|| {
            not_applied(
                "ENTRYPOINT_REFRESH_TOPOLOGY",
                format!("{} is absent from topology", path.display()),
            )
        })?;
    if admission.entrypoint.content_digest() != topology_digest {
        return Err(not_applied(
            "ENTRYPOINT_REFRESH_DIVERGED",
            "review and topology do not share one starting digest",
        ));
    }
    Ok(SelectedReview {
        entrypoint: path.to_path_buf(),
        content_digest: topology_digest.to_owned(),
        review_record: admission.review_record.clone(),
        review_record_digest: admission.review_record_digest.clone(),
    })
}

fn reviewed_path(review: &Admission) -> &Path {
    match &review.entrypoint {
        ReviewedEntrypoint::Script { path, .. }
        | ReviewedEntrypoint::Workflow { path, .. } => path,
    }
}

fn topology_digest<'a>(
    entrypoint: &'a RepositoryEntrypoint,
    path: &Path,
) -> Option<&'a str> {
    match entrypoint {
        RepositoryEntrypoint::Script { path: candidate, content_digest }
        | RepositoryEntrypoint::Workflow { path: candidate, content_digest }
            if candidate == path =>
        {
            Some(content_digest)
        }
        _ => None,
    }
}

fn refresh_review_marker(
    bytes: &[u8],
    digest: &str,
) -> Result<Vec<u8>, EntrypointRefreshError> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        not_applied("ENTRYPOINT_REFRESH_REVIEW_UTF8", error.to_string())
    })?;
    let marker = unique_offset(text, REVIEW_MARKER)?;
    let line_start = marker + REVIEW_MARKER.len();
    let tail = &text[line_start..];
    let line_len = tail.find('\n').unwrap_or(tail.len());
    let old_line = &tail[..line_len];
    if !review_digest_line(old_line) {
        return Err(not_applied(
            "ENTRYPOINT_REFRESH_REVIEW_MARKER",
            "exact content digest marker has a noncanonical value line",
        ));
    }
    let replacement = format!("  `{digest}`");
    let end = line_start + line_len;
    let mut output =
        Vec::with_capacity(bytes.len() - old_line.len() + replacement.len());
    output.extend_from_slice(&bytes[..line_start]);
    output.extend_from_slice(replacement.as_bytes());
    output.extend_from_slice(&bytes[end..]);
    Ok(output)
}

fn review_digest_line(line: &str) -> bool {
    let Some(value) = line
        .strip_prefix("  `sha256:")
        .and_then(|value| value.strip_suffix('`'))
    else {
        return false;
    };
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn refresh_registry(
    bytes: &[u8],
    registry: &Registry,
    selected: &SelectedReview,
    content_digest: &str,
    review_digest: &str,
    reviewed_at: &str,
) -> Result<Vec<u8>, EntrypointRefreshError> {
    let text = utf8_config(bytes)?;
    let old_time = format!("  \"reviewed_at\": \"{}\",", registry.reviewed_at);
    let new_time = format!("  \"reviewed_at\": \"{reviewed_at}\",");
    let with_time = replace_once(text.as_bytes(), &old_time, &new_time)?;
    let old_entry =
        entrypoint_fragment(&selected.entrypoint, &selected.content_digest)?;
    let new_entry = entrypoint_fragment(&selected.entrypoint, content_digest)?;
    let with_entry = replace_once(&with_time, &old_entry, &new_entry)?;
    let old_record = review_fragment(
        &selected.review_record,
        &selected.review_record_digest,
    )?;
    let new_record = review_fragment(&selected.review_record, review_digest)?;
    replace_once(&with_entry, &old_record, &new_record)
}

fn refresh_topology(
    bytes: &[u8],
    selected: &SelectedReview,
    content_digest: &str,
) -> Result<Vec<u8>, EntrypointRefreshError> {
    let text = utf8_config(bytes)?;
    let old =
        entrypoint_fragment(&selected.entrypoint, &selected.content_digest)?;
    let new = entrypoint_fragment(&selected.entrypoint, content_digest)?;
    replace_once(text.as_bytes(), &old, &new)
}

fn entrypoint_fragment(
    path: &Path,
    digest: &str,
) -> Result<String, EntrypointRefreshError> {
    let path = json_path(path)?;
    Ok(format!(
        "      \"path\": {path},\n      \"content_digest\": \"{digest}\""
    ))
}

fn review_fragment(
    path: &Path,
    digest: &str,
) -> Result<String, EntrypointRefreshError> {
    let path = json_path(path)?;
    Ok(format!(
        concat!(
            "      \"review_record\": {path},\n",
            "      \"review_record_digest\": \"{digest}\"",
        ),
        path = path,
        digest = digest,
    ))
}

fn json_path(path: &Path) -> Result<String, EntrypointRefreshError> {
    let value = path.to_str().ok_or_else(|| {
        not_applied(
            "ENTRYPOINT_REFRESH_PATH",
            "registered path is not valid UTF-8",
        )
    })?;
    serde_json::to_string(value).map_err(|error| {
        not_applied("ENTRYPOINT_REFRESH_JSON", error.to_string())
    })
}

fn utf8_config(bytes: &[u8]) -> Result<&str, EntrypointRefreshError> {
    std::str::from_utf8(bytes).map_err(|error| {
        not_applied("ENTRYPOINT_REFRESH_JSON_UTF8", error.to_string())
    })
}

fn unique_offset(
    text: &str,
    pattern: &str,
) -> Result<usize, EntrypointRefreshError> {
    let mut matches = text.match_indices(pattern);
    let first = matches.next().map(|(offset, _)| offset);
    if first.is_none() || matches.next().is_some() {
        return Err(not_applied(
            "ENTRYPOINT_REFRESH_CANONICAL_MATCH",
            "expected exactly one canonical field or marker",
        ));
    }
    Ok(first.unwrap_or_default())
}

fn replace_once(
    bytes: &[u8],
    old: &str,
    new: &str,
) -> Result<Vec<u8>, EntrypointRefreshError> {
    let text = utf8_config(bytes)?;
    let offset = unique_offset(text, old)?;
    if old == new {
        return Ok(bytes.to_vec());
    }
    let mut output = Vec::with_capacity(bytes.len() - old.len() + new.len());
    output.extend_from_slice(&bytes[..offset]);
    output.extend_from_slice(new.as_bytes());
    output.extend_from_slice(&bytes[offset + old.len()..]);
    Ok(output)
}

fn validate_candidates(
    reviews: &[u8],
    topology: &[u8],
    selected: &SelectedReview,
    content_digest: &str,
    review_digest: &str,
    reviewed_at: &str,
) -> Result<(), EntrypointRefreshError> {
    let review_value = json::parse(
        reviews,
        HARD_LIMITS.json_depth,
        HARD_LIMITS.json_container_items,
    )
    .map_err(config_error)?;
    let registry = Registry::parse(&review_value).map_err(config_error)?;
    let topology_value = json::parse(
        topology,
        HARD_LIMITS.json_depth,
        HARD_LIMITS.json_container_items,
    )
    .map_err(config_error)?;
    let topology = Topology::parse(&topology_value).map_err(config_error)?;
    let next = select(&registry, &topology, &selected.entrypoint)?;
    if next.content_digest != content_digest
        || next.review_record != selected.review_record
        || next.review_record_digest != review_digest
        || registry.reviewed_at != reviewed_at
    {
        return Err(not_applied(
            "ENTRYPOINT_REFRESH_CANDIDATE",
            "candidate semantic fields differ from computed values",
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct FileUpdate {
    path: PathBuf,
    original: Vec<u8>,
    candidate: Vec<u8>,
}

impl FileUpdate {
    const fn new(path: PathBuf, original: Vec<u8>, candidate: Vec<u8>) -> Self {
        Self { path, original, candidate }
    }
}

#[derive(Debug)]
struct StagedUpdate {
    temporary: PathBuf,
}

fn publish_updates(
    updates: &[FileUpdate],
) -> Result<(), EntrypointRefreshError> {
    publish_updates_with(updates, |source, target, _index| {
        fs::rename(source, target)
    })
}

fn publish_updates_with<R>(
    updates: &[FileUpdate],
    mut rename: R,
) -> Result<(), EntrypointRefreshError>
where
    R: FnMut(&Path, &Path, usize) -> std::io::Result<()>,
{
    let mut staged = Vec::with_capacity(updates.len());
    for (index, update) in updates.iter().enumerate() {
        let temporary = match stage(update, index, "candidate") {
            Ok(temporary) => temporary,
            Err(error) => {
                cleanup_staged(&staged);
                return Err(error);
            }
        };
        staged.push(StagedUpdate { temporary });
    }
    if let Err(error) = verify_unchanged(updates) {
        cleanup_staged(&staged);
        return Err(error);
    }
    let mut applied = 0_usize;
    for (index, update) in updates.iter().enumerate() {
        if let Err(error) = verify_one_unchanged(update) {
            return changed_during_publish(
                updates, &staged, applied, update, &error,
            );
        }
        let renamed = rename(&staged[index].temporary, &update.path, index);
        if let Err(error) = renamed {
            return publish_failure(updates, &staged, applied, update, &error);
        }
        applied += 1;
        if let Err(error) = sync_parent(&update.path) {
            return publish_failure(updates, &staged, applied, update, &error);
        }
    }
    cleanup_staged(&staged);
    Ok(())
}

fn verify_unchanged(
    updates: &[FileUpdate],
) -> Result<(), EntrypointRefreshError> {
    for update in updates {
        verify_one_unchanged(update)?;
    }
    Ok(())
}

fn verify_one_unchanged(
    update: &FileUpdate,
) -> Result<(), EntrypointRefreshError> {
    let current = read_regular(&update.path)?;
    if current != update.original {
        return Err(not_applied(
            "ENTRYPOINT_REFRESH_CHANGED",
            format!("{} changed during refresh", update.path.display()),
        ));
    }
    Ok(())
}

fn changed_during_publish(
    updates: &[FileUpdate],
    staged: &[StagedUpdate],
    applied: usize,
    update: &FileUpdate,
    error: &EntrypointRefreshError,
) -> Result<(), EntrypointRefreshError> {
    let rolled_back = rollback(updates, applied);
    cleanup_staged(staged);
    let effect = if rolled_back {
        RefreshEffect::DefinitelyNotApplied
    } else {
        RefreshEffect::OutcomeUnknown
    };
    Err(EntrypointRefreshError {
        code: error.code,
        message: format!("{}: {}", update.path.display(), error.message),
        effect,
    })
}

fn publish_failure(
    updates: &[FileUpdate],
    staged: &[StagedUpdate],
    applied: usize,
    update: &FileUpdate,
    error: &std::io::Error,
) -> Result<(), EntrypointRefreshError> {
    let rolled_back = rollback(updates, applied);
    cleanup_staged(staged);
    let effect = if rolled_back {
        RefreshEffect::DefinitelyNotApplied
    } else {
        RefreshEffect::OutcomeUnknown
    };
    Err(EntrypointRefreshError {
        code: "ENTRYPOINT_REFRESH_PUBLISH",
        message: format!("{}: {error}", update.path.display()),
        effect,
    })
}

fn stage(
    update: &FileUpdate,
    index: usize,
    purpose: &str,
) -> Result<PathBuf, EntrypointRefreshError> {
    let file_name = update.path.file_name().and_then(|name| name.to_str());
    let parent = update.path.parent();
    let (Some(file_name), Some(parent)) = (file_name, parent) else {
        return Err(not_applied(
            "ENTRYPOINT_REFRESH_OUTPUT",
            "tracked output path has no UTF-8 file name or parent",
        ));
    };
    let temporary = parent.join(format!(
        ".{file_name}.reovim-refresh-{}-{index}-{purpose}.tmp",
        std::process::id(),
    ));
    let result = write_temporary(&update.path, &temporary, &update.candidate);
    if let Err(error) = result {
        return Err(not_applied(
            "ENTRYPOINT_REFRESH_STAGE",
            format!("{}: {error}", temporary.display()),
        ));
    }
    Ok(temporary)
}

fn write_temporary(
    target: &Path,
    temporary: &Path,
    bytes: &[u8],
) -> std::io::Result<()> {
    let permissions = fs::metadata(target)?.permissions();
    let mut file =
        fs::OpenOptions::new().write(true).create_new(true).open(temporary)?;
    let result = write_open_temporary(&mut file, temporary, permissions, bytes);
    drop(file);
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn write_open_temporary(
    file: &mut fs::File,
    temporary: &Path,
    permissions: fs::Permissions,
    bytes: &[u8],
) -> std::io::Result<()> {
    fs::set_permissions(temporary, permissions)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()
}

fn sync_parent(path: &Path) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("output path has no parent"))?;
    fs::File::open(parent)?.sync_all()
}

fn rollback(updates: &[FileUpdate], applied: usize) -> bool {
    let mut complete = true;
    for index in (0..applied).rev() {
        complete &= restore_update(&updates[index], index);
    }
    complete
}

fn restore_update(update: &FileUpdate, index: usize) -> bool {
    let rollback = FileUpdate::new(
        update.path.clone(),
        Vec::new(),
        update.original.clone(),
    );
    let Ok(temporary) = stage(&rollback, index, "rollback") else {
        return false;
    };
    let result = fs::rename(&temporary, &update.path)
        .and_then(|()| sync_parent(&update.path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.is_ok()
}

fn cleanup_staged(staged: &[StagedUpdate]) {
    for update in staged {
        let _ = fs::remove_file(&update.temporary);
    }
}

fn utc_timestamp(time: SystemTime) -> Result<String, EntrypointRefreshError> {
    let seconds = time.duration_since(UNIX_EPOCH).map_err(|error| {
        not_applied("ENTRYPOINT_REFRESH_TIME", error.to_string())
    })?;
    let seconds = seconds.as_secs();
    if seconds > MAX_UTC_SECONDS {
        return Err(not_applied(
            "ENTRYPOINT_REFRESH_TIME",
            "current UTC value is outside the four-digit year profile",
        ));
    }
    let days = i64::try_from(seconds / 86_400).map_err(|error| {
        not_applied("ENTRYPOINT_REFRESH_TIME", error.to_string())
    })?;
    let within_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = within_day / 3_600;
    let minute = within_day % 3_600 / 60;
    let second = within_day % 60;
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524
        - day_of_era / 146_096)
        / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

fn config_error(error: json::ConfigError) -> EntrypointRefreshError {
    let json::ConfigError { code, path, message } = error;
    not_applied(
        "ENTRYPOINT_REFRESH_CONFIG",
        format!("{code} at {path}: {message}"),
    )
}

fn io_error(
    code: &'static str,
    path: &Path,
    error: &std::io::Error,
) -> EntrypointRefreshError {
    not_applied(code, format!("{}: {error}", path.display()))
}

fn not_applied(
    code: &'static str,
    message: impl Into<String>,
) -> EntrypointRefreshError {
    EntrypointRefreshError {
        code,
        message: message.into(),
        effect: RefreshEffect::DefinitelyNotApplied,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };

    use super::{
        FileUpdate, MAX_UTC_SECONDS, REVIEW_MARKER, RefreshEffect, UNIX_EPOCH,
        publish_updates, publish_updates_with, refresh_review_marker,
        replace_once, utc_timestamp,
    };

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn create() -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "reovim-entrypoint-refresh-{}-{sequence}",
                std::process::id(),
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn join(&self, name: &str) -> PathBuf {
            self.path.join(name)
        }

        fn assert_no_staged_files(&self) {
            let has_staged = fs::read_dir(&self.path)
                .unwrap()
                .any(directory_entry_is_staged);
            assert!(!has_staged);
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).unwrap();
        }
    }

    fn write(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
    }

    fn directory_entry_is_staged(
        entry: Result<fs::DirEntry, std::io::Error>,
    ) -> bool {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".reovim-refresh-")
    }

    fn fail_second_rename(
        source: &Path,
        target: &Path,
        index: usize,
    ) -> std::io::Result<()> {
        if index == 1 {
            return Err(std::io::Error::other(
                "injected second replacement failure",
            ));
        }
        fs::rename(source, target)
    }

    #[test]
    fn utc_timestamp_covers_epoch_and_gregorian_leap_day() {
        assert_eq!(utc_timestamp(UNIX_EPOCH).unwrap(), "1970-01-01T00:00:00Z");
        let leap_day = UNIX_EPOCH + Duration::from_hours(264_384);
        assert_eq!(utc_timestamp(leap_day).unwrap(), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn utc_timestamp_rejects_values_outside_the_supported_range() {
        let before_epoch = UNIX_EPOCH - Duration::from_secs(1);
        let after_profile =
            UNIX_EPOCH + Duration::from_secs(MAX_UTC_SECONDS + 1);
        assert!(utc_timestamp(before_epoch).is_err());
        assert!(utc_timestamp(after_profile).is_err());
    }

    #[test]
    fn exact_replacement_rejects_absent_and_duplicate_fields() {
        assert!(replace_once(b"alpha", "beta", "gamma").is_err());
        assert!(replace_once(b"alpha alpha", "alpha", "beta").is_err());
        assert_eq!(replace_once(b"alpha", "alpha", "beta").unwrap(), b"beta");
    }

    #[test]
    fn review_marker_changes_only_its_canonical_digest_line() {
        let prior = format!(
            "# Review\n\n{REVIEW_MARKER}  `sha256:{}`\n\nBody.\n",
            "1".repeat(64),
        );
        let next_digest = format!("sha256:{}", "a".repeat(64));
        let next =
            refresh_review_marker(prior.as_bytes(), &next_digest).unwrap();
        let next = String::from_utf8(next).unwrap();
        assert!(next.contains(&format!("  `{next_digest}`")));
        assert!(next.ends_with("\n\nBody.\n"));
    }

    #[test]
    fn review_marker_rejects_malformed_and_duplicate_markers() {
        let digest = format!("sha256:{}", "a".repeat(64));
        let malformed = format!("{REVIEW_MARKER}  `sha256:not-a-digest`\n");
        let duplicate = format!(
            "{REVIEW_MARKER}  `{digest}`\n{REVIEW_MARKER}  `{digest}`\n"
        );
        assert!(refresh_review_marker(malformed.as_bytes(), &digest).is_err());
        assert!(refresh_review_marker(duplicate.as_bytes(), &digest).is_err());
    }

    #[test]
    fn stage_failure_preserves_targets_and_removes_temporary_files() {
        let directory = TestDirectory::create();
        let first = directory.join("first.json");
        let absent = directory.join("absent.json");
        write(&first, b"first-old");
        let updates = [
            FileUpdate::new(
                first.clone(),
                b"first-old".to_vec(),
                b"first-new".to_vec(),
            ),
            FileUpdate::new(
                absent.clone(),
                b"absent-old".to_vec(),
                b"absent-new".to_vec(),
            ),
        ];

        let error = publish_updates(&updates).unwrap_err();

        assert_eq!(error.effect, RefreshEffect::DefinitelyNotApplied);
        assert_eq!(fs::read(first).unwrap(), b"first-old");
        assert!(!absent.exists());
        directory.assert_no_staged_files();
    }

    #[test]
    fn competing_temporary_file_is_never_removed() {
        let directory = TestDirectory::create();
        let target = directory.join("target.json");
        write(&target, b"target-old");
        let temporary_name = format!(
            ".target.json.reovim-refresh-{}-0-candidate.tmp",
            std::process::id(),
        );
        let temporary = directory.join(&temporary_name);
        write(&temporary, b"competitor-owned");
        let updates = [FileUpdate::new(
            target.clone(),
            b"target-old".to_vec(),
            b"target-new".to_vec(),
        )];

        let error = publish_updates(&updates).unwrap_err();

        assert_eq!(error.effect, RefreshEffect::DefinitelyNotApplied);
        assert_eq!(fs::read(target).unwrap(), b"target-old");
        assert_eq!(fs::read(temporary).unwrap(), b"competitor-owned");
    }

    #[test]
    fn replacement_failure_rolls_back_and_removes_temporary_files() {
        let directory = TestDirectory::create();
        let first = directory.join("first.json");
        let second = directory.join("second.json");
        write(&first, b"first-old");
        write(&second, b"second-old");
        let updates = [
            FileUpdate::new(
                first.clone(),
                b"first-old".to_vec(),
                b"first-new".to_vec(),
            ),
            FileUpdate::new(
                second.clone(),
                b"second-old".to_vec(),
                b"second-new".to_vec(),
            ),
        ];

        let error =
            publish_updates_with(&updates, fail_second_rename).unwrap_err();

        assert_eq!(error.effect, RefreshEffect::DefinitelyNotApplied);
        assert_eq!(fs::read(first).unwrap(), b"first-old");
        assert_eq!(fs::read(second).unwrap(), b"second-old");
        directory.assert_no_staged_files();
    }
}

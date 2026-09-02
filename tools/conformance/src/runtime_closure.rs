//! Direct, bounded ELF runtime closure for the Linux prepared root.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt, fs,
    path::{Component, Path, PathBuf},
};

use crate::digest;

const ELF_HEADER_BYTES: usize = 64;
const PROGRAM_HEADER_BYTES: usize = 56;
const DYNAMIC_ENTRY_BYTES: usize = 16;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const DT_NULL: i64 = 0;
const DT_NEEDED: i64 = 1;
const DT_STRTAB: i64 = 5;
const DT_STRSZ: i64 = 10;
const DT_RPATH: i64 = 15;
const DT_RUNPATH: i64 = 29;

/// Hard limits for one runtime-closure observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub path_bytes: usize,
    pub object_count: usize,
    pub file_bytes: u64,
    pub total_bytes: u64,
    pub dynamic_entries: usize,
    pub string_bytes: usize,
}

/// One executable or pre-exposed shared object and its in-root path.
#[derive(Debug, Clone, Copy)]
pub struct ObjectRequest<'a> {
    pub source: &'a Path,
    pub destination: &'a Path,
}

/// One exact host file that must be mounted at a runtime loader path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Mapping {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub content_digest: String,
    pub size_bytes: u64,
}

/// Complete system-runtime mapping set for the supplied objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub mappings: Vec<Mapping>,
    pub object_count: usize,
    pub total_bytes: u64,
    pub aggregate_digest: String,
}

/// Typed ELF, resolution, identity, or bound failure.
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

/// Resolves the direct ELF loader closure without executing another helper.
///
/// `roots` are the executables whose runtime closure is required. `provided`
/// are shared objects already exposed elsewhere, such as rustc driver files.
/// Returned mappings contain only additional system-runtime files.
///
/// # Errors
///
/// Returns a typed error for unsupported ELF input, unresolved libraries,
/// ambiguous mappings, identity drift, or an exhausted bound.
pub fn observe(
    roots: &[ObjectRequest<'_>],
    provided: &[ObjectRequest<'_>],
    limits: Limits,
) -> Result<Observation, Error> {
    validate_limits(limits)?;
    if roots.is_empty() {
        return Err(Error::new(
            "RUNTIME_CLOSURE_ROOTS_EMPTY",
            "at least one runtime root is required",
        ));
    }
    let provided = provided_map(provided, limits)?;
    let mut observer = Observer::new(provided, limits);
    for root in roots {
        observer.enqueue_request(root)?;
    }
    observer.enqueue_provided();
    observer.run()
}

fn validate_limits(limits: Limits) -> Result<(), Error> {
    if limits.path_bytes == 0
        || limits.object_count == 0
        || limits.file_bytes == 0
        || limits.total_bytes == 0
        || limits.dynamic_entries == 0
        || limits.string_bytes == 0
    {
        return Err(Error::new(
            "RUNTIME_CLOSURE_ZERO_LIMIT",
            "runtime-closure limits must be positive",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Object {
    source: PathBuf,
    lookup_path: PathBuf,
    destination: PathBuf,
}

fn provided_map(
    requests: &[ObjectRequest<'_>],
    limits: Limits,
) -> Result<BTreeMap<PathBuf, PathBuf>, Error> {
    let mut values = BTreeMap::new();
    let mut destinations = BTreeMap::new();
    for request in requests {
        let object = checked_object(request, limits)?;
        insert_provided(&mut values, &object)?;
        insert_destination(
            &mut destinations,
            &object.destination,
            &object.source,
        )?;
    }
    Ok(values)
}

fn insert_provided(
    values: &mut BTreeMap<PathBuf, PathBuf>,
    object: &Object,
) -> Result<(), Error> {
    match values.insert(object.source.clone(), object.destination.clone()) {
        None => Ok(()),
        Some(existing) if existing == object.destination => Ok(()),
        Some(_) => Err(Error::at(
            "RUNTIME_CLOSURE_PROVIDED_ALIAS",
            &object.source,
            "provided object has two prepared paths",
        )),
    }
}

fn insert_destination(
    destinations: &mut BTreeMap<PathBuf, PathBuf>,
    destination: &Path,
    source: &Path,
) -> Result<(), Error> {
    let Some(existing) =
        destinations.insert(destination.to_path_buf(), source.to_path_buf())
    else {
        return Ok(());
    };
    if existing == source {
        return Ok(());
    }
    Err(Error::at(
        "RUNTIME_CLOSURE_DESTINATION_ALIAS",
        destination,
        "prepared path selects two host files",
    ))
}

fn checked_object(
    request: &ObjectRequest<'_>,
    limits: Limits,
) -> Result<Object, Error> {
    validate_absolute(request.source, limits.path_bytes)?;
    validate_absolute(request.destination, limits.path_bytes)?;
    let source = canonical_regular_file(request.source, limits.file_bytes)?;
    if source != request.source {
        return Err(Error::at(
            "RUNTIME_CLOSURE_SOURCE_ALIAS",
            request.source,
            "object source must be canonical",
        ));
    }
    Ok(Object {
        source,
        lookup_path: request.source.to_path_buf(),
        destination: request.destination.to_path_buf(),
    })
}

struct Observer {
    provided: BTreeMap<PathBuf, PathBuf>,
    limits: Limits,
    pending: VecDeque<Object>,
    seen: BTreeSet<(PathBuf, PathBuf)>,
    mappings: BTreeMap<PathBuf, Mapping>,
    total_bytes: u64,
}

impl Observer {
    const fn new(provided: BTreeMap<PathBuf, PathBuf>, limits: Limits) -> Self {
        Self {
            provided,
            limits,
            pending: VecDeque::new(),
            seen: BTreeSet::new(),
            mappings: BTreeMap::new(),
            total_bytes: 0,
        }
    }

    fn enqueue_request(
        &mut self,
        request: &ObjectRequest<'_>,
    ) -> Result<(), Error> {
        self.pending.push_back(checked_object(request, self.limits)?);
        Ok(())
    }

    fn enqueue_provided(&mut self) {
        let values = self
            .provided
            .iter()
            .map(|(source, destination)| Object {
                source: source.clone(),
                lookup_path: source.clone(),
                destination: destination.clone(),
            })
            .collect::<Vec<_>>();
        for object in values {
            self.pending.push_back(object);
        }
    }

    fn run(mut self) -> Result<Observation, Error> {
        while let Some(object) = self.pending.pop_front() {
            self.observe_object(&object)?;
        }
        let mappings = self.mappings.into_values().collect::<Vec<_>>();
        let aggregate_digest = mapping_digest(&mappings)?;
        Ok(Observation {
            object_count: self.seen.len(),
            total_bytes: self.total_bytes,
            mappings,
            aggregate_digest,
        })
    }

    fn observe_object(&mut self, object: &Object) -> Result<(), Error> {
        let identity = (object.source.clone(), object.destination.clone());
        if !self.seen.insert(identity) {
            return Ok(());
        }
        if self.seen.len() > self.limits.object_count {
            return Err(Error::at(
                "RUNTIME_CLOSURE_OBJECT_LIMIT",
                &object.source,
                format!("objects exceed {}", self.limits.object_count),
            ));
        }
        let bytes = read_object(&object.source, self.limits.file_bytes)?;
        let dynamic = parse_elf(&object.source, &bytes, self.limits)?;
        if let Some(interpreter) = dynamic.interpreter {
            self.add_interpreter(&interpreter)?;
        }
        for needed in dynamic.needed {
            self.add_needed(object, &dynamic.runpaths, &needed)?;
        }
        Ok(())
    }

    fn add_interpreter(&mut self, destination: &Path) -> Result<(), Error> {
        validate_absolute(destination, self.limits.path_bytes)?;
        let source =
            canonical_regular_file(destination, self.limits.file_bytes)?;
        self.add_system_mapping(source.clone(), destination.to_path_buf())?;
        self.pending.push_back(Object {
            source,
            lookup_path: destination.to_path_buf(),
            destination: destination.to_path_buf(),
        });
        Ok(())
    }

    fn add_needed(
        &mut self,
        object: &Object,
        runpaths: &[String],
        needed: &str,
    ) -> Result<(), Error> {
        validate_library_name(needed, self.limits.path_bytes)?;
        let resolved = resolve_library(object, runpaths, needed, self.limits)?;
        if let Some(destination) = self.provided.get(&resolved.source) {
            require_provided_destination(destination, &resolved)?;
        } else {
            self.add_system_mapping(
                resolved.source.clone(),
                resolved.destination.clone(),
            )?;
        }
        self.pending.push_back(resolved);
        Ok(())
    }

    fn add_system_mapping(
        &mut self,
        source: PathBuf,
        destination: PathBuf,
    ) -> Result<(), Error> {
        let metadata = fs::metadata(&source).map_err(|failure| {
            Error::at("RUNTIME_CLOSURE_METADATA", &source, failure.to_string())
        })?;
        let size = metadata.len();
        let content_digest = digest::file_bounded(
            &source,
            self.limits.file_bytes,
        )
        .map_err(|failure| {
            Error::at("RUNTIME_CLOSURE_DIGEST", &source, failure.to_string())
        })?;
        let mapping = Mapping {
            source,
            destination: destination.clone(),
            content_digest,
            size_bytes: size,
        };
        if existing_mapping(self.mappings.get(&destination), &mapping)? {
            return Ok(());
        }
        self.charge_mapping(size, &mapping.source)?;
        self.mappings.insert(destination, mapping);
        Ok(())
    }

    fn charge_mapping(&mut self, bytes: u64, path: &Path) -> Result<(), Error> {
        let total = self.total_bytes.checked_add(bytes).ok_or_else(|| {
            Error::at(
                "RUNTIME_CLOSURE_BYTE_OVERFLOW",
                path,
                "runtime byte count overflow",
            )
        })?;
        if total > self.limits.total_bytes {
            return Err(Error::at(
                "RUNTIME_CLOSURE_TOTAL_LIMIT",
                path,
                format!("runtime bytes exceed {}", self.limits.total_bytes),
            ));
        }
        self.total_bytes = total;
        Ok(())
    }
}

fn require_provided_destination(
    destination: &Path,
    resolved: &Object,
) -> Result<(), Error> {
    if destination == resolved.destination {
        return Ok(());
    }
    Err(Error::at(
        "RUNTIME_CLOSURE_PROVIDED_DESTINATION",
        &resolved.source,
        "provided path differs from loader resolution",
    ))
}

fn existing_mapping(
    existing: Option<&Mapping>,
    mapping: &Mapping,
) -> Result<bool, Error> {
    match existing {
        None => Ok(false),
        Some(existing) if existing == mapping => Ok(true),
        Some(_) => Err(Error::at(
            "RUNTIME_CLOSURE_MAPPING_ALIAS",
            &mapping.destination,
            "runtime destination changed identity",
        )),
    }
}

fn read_object(path: &Path, limit: u64) -> Result<Vec<u8>, Error> {
    let metadata = fs::metadata(path).map_err(|failure| {
        Error::at("RUNTIME_CLOSURE_METADATA", path, failure.to_string())
    })?;
    if metadata.len() == 0 || metadata.len() > limit {
        return Err(Error::at(
            "RUNTIME_CLOSURE_FILE_LIMIT",
            path,
            format!("file size is outside 1..={limit}"),
        ));
    }
    let bytes = fs::read(path).map_err(|failure| {
        Error::at("RUNTIME_CLOSURE_READ", path, failure.to_string())
    })?;
    let observed = u64::try_from(bytes.len()).map_err(|_| {
        Error::at(
            "RUNTIME_CLOSURE_FILE_SIZE",
            path,
            "file size does not fit u64",
        )
    })?;
    if observed != metadata.len() {
        return Err(Error::at(
            "RUNTIME_CLOSURE_FILE_DRIFT",
            path,
            "file size changed while reading",
        ));
    }
    Ok(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DynamicInfo {
    interpreter: Option<PathBuf>,
    needed: Vec<String>,
    runpaths: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    kind: u32,
    offset: u64,
    address: u64,
    file_bytes: u64,
}

fn parse_elf(
    path: &Path,
    bytes: &[u8],
    limits: Limits,
) -> Result<DynamicInfo, Error> {
    validate_elf_header(path, bytes)?;
    let segments = program_segments(path, bytes, limits)?;
    let interpreter = interpreter(path, bytes, &segments, limits)?;
    let dynamic = dynamic_entries(path, bytes, &segments, limits)?;
    let Some(entries) = dynamic else {
        return Ok(DynamicInfo {
            interpreter,
            needed: Vec::new(),
            runpaths: Vec::new(),
        });
    };
    parse_dynamic_strings(path, bytes, &segments, &entries, interpreter, limits)
}

fn validate_elf_header(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let valid = bytes.len() >= ELF_HEADER_BYTES
        && bytes.get(..4) == Some(b"\x7fELF")
        && bytes[4] == 2
        && bytes[5] == 1
        && bytes[6] == 1
        && read_u16(bytes, 18) == Some(62)
        && read_u32(bytes, 20) == Some(1)
        && read_u16(bytes, 52) == Some(64)
        && read_u16(bytes, 54) == Some(56);
    if valid {
        return Ok(());
    }
    Err(Error::at(
        "RUNTIME_CLOSURE_ELF_PROFILE",
        path,
        "expected little-endian ELF64 x86-64 with standard headers",
    ))
}

fn program_segments(
    path: &Path,
    bytes: &[u8],
    limits: Limits,
) -> Result<Vec<Segment>, Error> {
    let offset = usize_value(path, read_u64(bytes, 32), "program offset")?;
    let count = usize::from(read_u16(bytes, 56).ok_or_else(|| {
        Error::at("RUNTIME_CLOSURE_ELF_HEADER", path, "program count is absent")
    })?);
    if count == 0 || count > limits.dynamic_entries {
        return Err(Error::at(
            "RUNTIME_CLOSURE_PROGRAM_LIMIT",
            path,
            format!("program headers outside 1..={}", limits.dynamic_entries),
        ));
    }
    let mut segments = Vec::with_capacity(count);
    for index in 0..count {
        let start = checked_table_offset(
            path,
            offset,
            index,
            PROGRAM_HEADER_BYTES,
            bytes.len(),
        )?;
        segments.push(Segment {
            kind: required_u32(path, bytes, start, "segment kind")?,
            offset: required_u64(path, bytes, start + 8, "segment offset")?,
            address: required_u64(path, bytes, start + 16, "segment address")?,
            file_bytes: required_u64(
                path,
                bytes,
                start + 32,
                "segment file size",
            )?,
        });
    }
    Ok(segments)
}

fn interpreter(
    path: &Path,
    bytes: &[u8],
    segments: &[Segment],
    limits: Limits,
) -> Result<Option<PathBuf>, Error> {
    let values = segments
        .iter()
        .filter(|segment| segment.kind == PT_INTERP)
        .collect::<Vec<_>>();
    if values.len() > 1 {
        return Err(Error::at(
            "RUNTIME_CLOSURE_INTERPRETER_COUNT",
            path,
            "ELF object has multiple interpreters",
        ));
    }
    let Some(segment) = values.first() else {
        return Ok(None);
    };
    let value = segment_string(path, bytes, segment, limits.string_bytes)?;
    let value = PathBuf::from(value);
    validate_absolute(&value, limits.path_bytes)?;
    Ok(Some(value))
}

fn dynamic_entries(
    path: &Path,
    bytes: &[u8],
    segments: &[Segment],
    limits: Limits,
) -> Result<Option<Vec<(i64, u64)>>, Error> {
    let values = segments
        .iter()
        .filter(|segment| segment.kind == PT_DYNAMIC)
        .collect::<Vec<_>>();
    if values.len() > 1 {
        return Err(Error::at(
            "RUNTIME_CLOSURE_DYNAMIC_COUNT",
            path,
            "ELF object has multiple dynamic segments",
        ));
    }
    let Some(segment) = values.first() else {
        return Ok(None);
    };
    let start = usize_value(path, Some(segment.offset), "dynamic offset")?;
    let size = usize_value(path, Some(segment.file_bytes), "dynamic size")?;
    let end = start.checked_add(size).ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_DYNAMIC_OVERFLOW",
            path,
            "dynamic range overflow",
        )
    })?;
    if end > bytes.len() || size % DYNAMIC_ENTRY_BYTES != 0 {
        return Err(Error::at(
            "RUNTIME_CLOSURE_DYNAMIC_RANGE",
            path,
            "dynamic segment is outside the file or misaligned",
        ));
    }
    let count = size / DYNAMIC_ENTRY_BYTES;
    if count > limits.dynamic_entries {
        return Err(Error::at(
            "RUNTIME_CLOSURE_DYNAMIC_LIMIT",
            path,
            format!("dynamic entries exceed {}", limits.dynamic_entries),
        ));
    }
    parse_dynamic_entry_values(path, bytes, start, count)
}

fn parse_dynamic_entry_values(
    path: &Path,
    bytes: &[u8],
    start: usize,
    count: usize,
) -> Result<Option<Vec<(i64, u64)>>, Error> {
    let mut values = Vec::new();
    for index in 0..count {
        let offset = start + index * DYNAMIC_ENTRY_BYTES;
        let tag = required_i64(path, bytes, offset, "dynamic tag")?;
        let value = required_u64(path, bytes, offset + 8, "dynamic value")?;
        if tag == DT_NULL {
            return Ok(Some(values));
        }
        values.push((tag, value));
    }
    Err(Error::at(
        "RUNTIME_CLOSURE_DYNAMIC_TERMINATOR",
        path,
        "dynamic segment lacks DT_NULL",
    ))
}

fn parse_dynamic_strings(
    path: &Path,
    bytes: &[u8],
    segments: &[Segment],
    entries: &[(i64, u64)],
    interpreter: Option<PathBuf>,
    limits: Limits,
) -> Result<DynamicInfo, Error> {
    let table_address = unique_dynamic_value(path, entries, DT_STRTAB)?;
    let table_size = unique_dynamic_value(path, entries, DT_STRSZ)?;
    let table_address = table_address.ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_STRING_TABLE",
            path,
            "dynamic object lacks DT_STRTAB",
        )
    })?;
    let table_size = table_size.ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_STRING_TABLE",
            path,
            "dynamic object lacks DT_STRSZ",
        )
    })?;
    let table = string_table(path, bytes, segments, table_address, table_size)?;
    let needed =
        dynamic_string_values(path, entries, DT_NEEDED, table, limits)?;
    let runpath =
        dynamic_string_values(path, entries, DT_RUNPATH, table, limits)?;
    let rpath = dynamic_string_values(path, entries, DT_RPATH, table, limits)?;
    if !rpath.is_empty() {
        return Err(Error::at(
            "RUNTIME_CLOSURE_RPATH_UNSUPPORTED",
            path,
            "inherited DT_RPATH semantics are outside the closed profile",
        ));
    }
    let mut runpaths = Vec::new();
    for value in runpath {
        runpaths.extend(parse_runpath(path, &value, limits)?);
    }
    Ok(DynamicInfo { interpreter, needed, runpaths })
}

fn unique_dynamic_value(
    path: &Path,
    entries: &[(i64, u64)],
    wanted: i64,
) -> Result<Option<u64>, Error> {
    let values = entries
        .iter()
        .filter_map(|(tag, value)| (*tag == wanted).then_some(*value))
        .collect::<Vec<_>>();
    if values.len() <= 1 {
        return Ok(values.first().copied());
    }
    Err(Error::at(
        "RUNTIME_CLOSURE_DYNAMIC_DUPLICATE",
        path,
        format!("dynamic tag {wanted} is duplicated"),
    ))
}

fn string_table<'a>(
    path: &Path,
    bytes: &'a [u8],
    segments: &[Segment],
    address: u64,
    size: u64,
) -> Result<&'a [u8], Error> {
    let offset = virtual_file_offset(path, segments, address)?;
    let size = usize_value(path, Some(size), "string table size")?;
    let end = offset.checked_add(size).ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_STRING_OVERFLOW",
            path,
            "string table range overflow",
        )
    })?;
    bytes.get(offset..end).ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_STRING_RANGE",
            path,
            "string table is outside the file",
        )
    })
}

fn virtual_file_offset(
    path: &Path,
    segments: &[Segment],
    address: u64,
) -> Result<usize, Error> {
    for segment in segments.iter().filter(|item| item.kind == PT_LOAD) {
        let end = segment.address.checked_add(segment.file_bytes);
        if end.is_some_and(|end| address >= segment.address && address < end) {
            let delta = address - segment.address;
            let offset = segment.offset.checked_add(delta);
            return usize_value(path, offset, "virtual file offset");
        }
    }
    Err(Error::at(
        "RUNTIME_CLOSURE_VIRTUAL_RANGE",
        path,
        "dynamic string table is outside a file-backed load segment",
    ))
}

fn dynamic_string_values(
    path: &Path,
    entries: &[(i64, u64)],
    wanted: i64,
    table: &[u8],
    limits: Limits,
) -> Result<Vec<String>, Error> {
    let mut values = BTreeSet::new();
    for (_, offset) in entries.iter().filter(|(tag, _)| *tag == wanted) {
        let offset = usize_value(path, Some(*offset), "string offset")?;
        let text = terminated_string(path, table, offset, limits.string_bytes)?;
        if !values.insert(text) {
            return Err(Error::at(
                "RUNTIME_CLOSURE_DYNAMIC_STRING_DUPLICATE",
                path,
                format!("dynamic tag {wanted} repeats a string"),
            ));
        }
    }
    Ok(values.into_iter().collect())
}

fn segment_string(
    path: &Path,
    bytes: &[u8],
    segment: &Segment,
    limit: usize,
) -> Result<String, Error> {
    let start = usize_value(path, Some(segment.offset), "segment offset")?;
    let size = usize_value(path, Some(segment.file_bytes), "segment size")?;
    let end = start.checked_add(size).ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_SEGMENT_OVERFLOW",
            path,
            "segment range overflow",
        )
    })?;
    let value = bytes.get(start..end).ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_SEGMENT_RANGE",
            path,
            "segment is outside the file",
        )
    })?;
    let text = terminated_string(path, value, 0, limit)?;
    if text.len() + 1 != value.len() {
        return Err(Error::at(
            "RUNTIME_CLOSURE_INTERPRETER_PADDING",
            path,
            "interpreter segment has bytes after its terminator",
        ));
    }
    Ok(text)
}

fn terminated_string(
    path: &Path,
    bytes: &[u8],
    offset: usize,
    limit: usize,
) -> Result<String, Error> {
    let remaining = bytes.get(offset..).ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_STRING_OFFSET",
            path,
            "string offset is outside its table",
        )
    })?;
    let length =
        remaining.iter().position(|byte| *byte == 0).ok_or_else(|| {
            Error::at(
                "RUNTIME_CLOSURE_STRING_TERMINATOR",
                path,
                "dynamic string is not NUL terminated",
            )
        })?;
    if length == 0 || length > limit {
        return Err(Error::at(
            "RUNTIME_CLOSURE_STRING_LIMIT",
            path,
            format!("dynamic string is outside 1..={limit} bytes"),
        ));
    }
    let value =
        std::str::from_utf8(&remaining[..length]).map_err(|failure| {
            Error::at("RUNTIME_CLOSURE_STRING_UTF8", path, failure.to_string())
        })?;
    if value.chars().any(char::is_control) {
        return Err(Error::at(
            "RUNTIME_CLOSURE_STRING_CONTROL",
            path,
            "dynamic string contains control text",
        ));
    }
    Ok(value.to_owned())
}

fn parse_runpath(
    path: &Path,
    value: &str,
    limits: Limits,
) -> Result<Vec<String>, Error> {
    let mut seen = BTreeSet::new();
    let mut paths = Vec::new();
    for entry in value.split(':') {
        if entry.is_empty() || entry.len() > limits.path_bytes {
            return Err(Error::at(
                "RUNTIME_CLOSURE_RUNPATH_FORM",
                path,
                "runpath entries must be nonempty and bounded",
            ));
        }
        if !entry.starts_with("$ORIGIN") && !entry.starts_with("${ORIGIN}") {
            return Err(Error::at(
                "RUNTIME_CLOSURE_RUNPATH_UNSUPPORTED",
                path,
                "only origin-relative runpaths are supported",
            ));
        }
        if !seen.insert(entry.to_owned()) {
            return Err(Error::at(
                "RUNTIME_CLOSURE_RUNPATH_DUPLICATE",
                path,
                "runpath entry is duplicated",
            ));
        }
        paths.push(entry.to_owned());
    }
    Ok(paths)
}

fn resolve_library(
    object: &Object,
    runpaths: &[String],
    needed: &str,
    limits: Limits,
) -> Result<Object, Error> {
    for (host, prepared) in search_directories(object, runpaths, limits)? {
        let lookup = host.join(needed);
        let metadata = fs::metadata(&lookup);
        if metadata.as_ref().is_ok_and(std::fs::Metadata::is_file) {
            let source = canonical_regular_file(&lookup, limits.file_bytes)?;
            return Ok(Object {
                source,
                lookup_path: lookup,
                destination: prepared.join(needed),
            });
        }
        require_absent_or_not_found(&lookup, metadata)?;
    }
    Err(Error::at(
        "RUNTIME_CLOSURE_UNRESOLVED",
        &object.source,
        format!("could not resolve shared library {needed}"),
    ))
}

fn require_absent_or_not_found(
    path: &Path,
    metadata: std::io::Result<std::fs::Metadata>,
) -> Result<(), Error> {
    match metadata {
        Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => {
            Ok(())
        }
        Err(failure) => Err(Error::at(
            "RUNTIME_CLOSURE_RESOLVE_METADATA",
            path,
            failure.to_string(),
        )),
        Ok(_) => Ok(()),
    }
}

fn search_directories(
    object: &Object,
    runpaths: &[String],
    limits: Limits,
) -> Result<Vec<(PathBuf, PathBuf)>, Error> {
    let mut values = Vec::new();
    for runpath in runpaths {
        values.push((
            expand_origin(&object.lookup_path, runpath, limits.path_bytes)?,
            expand_origin(&object.destination, runpath, limits.path_bytes)?,
        ));
    }
    for value in default_search_directories(limits.path_bytes)? {
        values.push((value.clone(), value));
    }
    Ok(values)
}

fn expand_origin(
    object: &Path,
    value: &str,
    limit: usize,
) -> Result<PathBuf, Error> {
    let origin = object.parent().ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_ORIGIN",
            object,
            "object has no parent directory",
        )
    })?;
    let suffix = value
        .strip_prefix("$ORIGIN")
        .or_else(|| value.strip_prefix("${ORIGIN}"))
        .ok_or_else(|| {
            Error::at(
                "RUNTIME_CLOSURE_ORIGIN",
                object,
                "runpath does not begin with an origin token",
            )
        })?;
    if !suffix.is_empty() && !suffix.starts_with('/') {
        return Err(Error::at(
            "RUNTIME_CLOSURE_ORIGIN_SUFFIX",
            object,
            "origin suffix must be empty or slash-prefixed",
        ));
    }
    normalize_absolute(&origin.join(suffix.trim_start_matches('/')), limit)
}

fn normalize_absolute(path: &Path, limit: usize) -> Result<PathBuf, Error> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir => parts.clear(),
            Component::Normal(value) => parts.push(value.to_owned()),
            Component::ParentDir if !parts.is_empty() => {
                parts.pop();
            }
            Component::CurDir => {}
            _ => {
                return Err(Error::at(
                    "RUNTIME_CLOSURE_PATH_ESCAPE",
                    path,
                    "path escapes its absolute root",
                ));
            }
        }
    }
    let mut normalized = PathBuf::from("/");
    normalized.extend(parts);
    validate_absolute(&normalized, limit)?;
    Ok(normalized)
}

fn default_search_directories(
    path_limit: usize,
) -> Result<Vec<PathBuf>, Error> {
    let mut retained = BTreeSet::new();
    let mut output = Vec::new();
    for path in [
        "/lib64",
        "/usr/lib64",
        "/lib/x86_64-linux-gnu",
        "/usr/lib/x86_64-linux-gnu",
        "/lib",
        "/usr/lib",
    ] {
        let path = Path::new(path);
        let canonical = match fs::canonicalize(path) {
            Ok(value) => value,
            Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }
            Err(failure) => {
                return Err(Error::at(
                    "RUNTIME_CLOSURE_SEARCH_DIRECTORY",
                    path,
                    failure.to_string(),
                ));
            }
        };
        validate_absolute(&canonical, path_limit)?;
        if canonical.is_dir() && retained.insert(canonical.clone()) {
            output.push(canonical);
        }
    }
    if output.is_empty() {
        return Err(Error::new(
            "RUNTIME_CLOSURE_SEARCH_DIRECTORY",
            "host has no admitted canonical loader search directory",
        ));
    }
    Ok(output)
}

fn canonical_regular_file(path: &Path, limit: u64) -> Result<PathBuf, Error> {
    let canonical = fs::canonicalize(path).map_err(|failure| {
        Error::at("RUNTIME_CLOSURE_CANONICALIZE", path, failure.to_string())
    })?;
    let metadata = fs::symlink_metadata(&canonical).map_err(|failure| {
        Error::at("RUNTIME_CLOSURE_METADATA", &canonical, failure.to_string())
    })?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > limit
    {
        return Err(Error::at(
            "RUNTIME_CLOSURE_FILE_PROFILE",
            &canonical,
            format!("expected regular file with size in 1..={limit}"),
        ));
    }
    Ok(canonical)
}

fn validate_absolute(path: &Path, limit: usize) -> Result<(), Error> {
    let text = path.to_str().ok_or_else(|| {
        Error::at("RUNTIME_CLOSURE_PATH_UTF8", path, "path is not UTF-8")
    })?;
    let valid = text.starts_with('/')
        && (text == "/" || !text.ends_with('/'))
        && !text.contains(['\0', '\r', '\n', '\\'])
        && !text.contains("//")
        && !text.split('/').any(|part| matches!(part, "." | ".."));
    if valid && text.len() <= limit {
        return Ok(());
    }
    Err(Error::at(
        "RUNTIME_CLOSURE_PATH_FORM",
        path,
        format!("path is not normalized or exceeds {limit} bytes"),
    ))
}

fn validate_library_name(value: &str, limit: usize) -> Result<(), Error> {
    let valid = !value.is_empty()
        && value.len() <= limit
        && !value.contains(['/', '\\', '\0', '\r', '\n'])
        && !value.chars().any(char::is_control)
        && value != "."
        && value != "..";
    if valid {
        return Ok(());
    }
    Err(Error::new(
        "RUNTIME_CLOSURE_LIBRARY_NAME",
        format!("invalid needed-library name {value:?}"),
    ))
}

fn checked_table_offset(
    path: &Path,
    base: usize,
    index: usize,
    width: usize,
    file_len: usize,
) -> Result<usize, Error> {
    let delta = index.checked_mul(width).ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_TABLE_OVERFLOW",
            path,
            "table index overflow",
        )
    })?;
    let start = base.checked_add(delta).ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_TABLE_OVERFLOW",
            path,
            "table offset overflow",
        )
    })?;
    let end = start.checked_add(width).ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_TABLE_OVERFLOW",
            path,
            "table range overflow",
        )
    })?;
    if end <= file_len {
        return Ok(start);
    }
    Err(Error::at(
        "RUNTIME_CLOSURE_TABLE_RANGE",
        path,
        "table entry is outside the file",
    ))
}

fn usize_value(
    path: &Path,
    value: Option<u64>,
    name: &str,
) -> Result<usize, Error> {
    let value = value.ok_or_else(|| {
        Error::at("RUNTIME_CLOSURE_INTEGER", path, format!("{name} is absent"))
    })?;
    usize::try_from(value).map_err(|_| {
        Error::at(
            "RUNTIME_CLOSURE_INTEGER",
            path,
            format!("{name} does not fit usize"),
        )
    })
}

fn required_u32(
    path: &Path,
    bytes: &[u8],
    offset: usize,
    name: &str,
) -> Result<u32, Error> {
    read_u32(bytes, offset).ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_INTEGER",
            path,
            format!("{name} is outside the file"),
        )
    })
}

fn required_u64(
    path: &Path,
    bytes: &[u8],
    offset: usize,
    name: &str,
) -> Result<u64, Error> {
    read_u64(bytes, offset).ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_INTEGER",
            path,
            format!("{name} is outside the file"),
        )
    })
}

fn required_i64(
    path: &Path,
    bytes: &[u8],
    offset: usize,
    name: &str,
) -> Result<i64, Error> {
    read_i64(bytes, offset).ok_or_else(|| {
        Error::at(
            "RUNTIME_CLOSURE_INTEGER",
            path,
            format!("{name} is outside the file"),
        )
    })
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let value: [u8; 2] = bytes.get(offset..offset + 2)?.try_into().ok()?;
    Some(u16::from_le_bytes(value))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let value: [u8; 4] = bytes.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(value))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let value: [u8; 8] = bytes.get(offset..offset + 8)?.try_into().ok()?;
    Some(u64::from_le_bytes(value))
}

fn read_i64(bytes: &[u8], offset: usize) -> Option<i64> {
    let value: [u8; 8] = bytes.get(offset..offset + 8)?.try_into().ok()?;
    Some(i64::from_le_bytes(value))
}

fn mapping_digest(mappings: &[Mapping]) -> Result<String, Error> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"reovim-runtime-closure/v1\0");
    for mapping in mappings {
        frame_path(&mut bytes, &mapping.source)?;
        frame_path(&mut bytes, &mapping.destination)?;
        frame_bytes(&mut bytes, mapping.content_digest.as_bytes())?;
        bytes.extend_from_slice(&mapping.size_bytes.to_be_bytes());
    }
    Ok(digest::bytes(&bytes))
}

fn frame_path(output: &mut Vec<u8>, path: &Path) -> Result<(), Error> {
    let value = path.to_str().ok_or_else(|| {
        Error::at("RUNTIME_CLOSURE_PATH_UTF8", path, "digest path is not UTF-8")
    })?;
    frame_bytes(output, value.as_bytes())
}

fn frame_bytes(output: &mut Vec<u8>, value: &[u8]) -> Result<(), Error> {
    let length = u64::try_from(value.len()).map_err(|_| {
        Error::new(
            "RUNTIME_CLOSURE_DIGEST_LENGTH",
            "digest field length does not fit u64",
        )
    })?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Limits, ObjectRequest};

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn current_executable() -> std::path::PathBuf {
        std::env::current_exe().unwrap().canonicalize().unwrap()
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn observe_current(limits: Limits) -> super::Observation {
        let executable = current_executable();
        super::observe(
            &[ObjectRequest {
                source: &executable,
                destination: std::path::Path::new("/checker/test"),
            }],
            &[],
            limits,
        )
        .unwrap()
    }

    fn limits() -> Limits {
        Limits {
            path_bytes: 4096,
            object_count: 128,
            file_bytes: 128 * 1024 * 1024,
            total_bytes: 512 * 1024 * 1024,
            dynamic_entries: 4096,
            string_bytes: 4096,
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn observes_current_test_runtime_without_helper_execution() {
        let observed = observe_current(limits());
        assert_ne!(observed.mappings, Vec::new());
        assert!(observed.total_bytes > 0);
        assert!(observed.object_count > 1);
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn repeated_observation_is_identity_equal() {
        assert_eq!(observe_current(limits()), observe_current(limits()));
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn runtime_byte_limit_is_enforced() {
        let executable = current_executable();
        let mut bounded = limits();
        bounded.total_bytes = 1;
        let failure = super::observe(
            &[ObjectRequest {
                source: &executable,
                destination: std::path::Path::new("/checker/test"),
            }],
            &[],
            bounded,
        )
        .unwrap_err();
        assert_eq!(failure.code, "RUNTIME_CLOSURE_TOTAL_LIMIT");
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn provided_object_cannot_change_loader_destination() {
        let executable = current_executable();
        let observed = observe_current(limits());
        let supplied = &observed.mappings[0];
        let failure = super::observe(
            &[ObjectRequest {
                source: &executable,
                destination: std::path::Path::new("/checker/test"),
            }],
            &[ObjectRequest {
                source: &supplied.source,
                destination: std::path::Path::new("/wrong/runtime-path"),
            }],
            limits(),
        )
        .unwrap_err();
        assert_eq!(failure.code, "RUNTIME_CLOSURE_PROVIDED_DESTINATION");
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn malformed_elf_magic_is_rejected() {
        let executable = current_executable();
        let mut bytes = std::fs::read(&executable).unwrap();
        bytes[0] = 0;
        let failure =
            super::parse_elf(&executable, &bytes, limits()).unwrap_err();
        assert_eq!(failure.code, "RUNTIME_CLOSURE_ELF_PROFILE");
    }

    #[test]
    fn absolute_runpath_is_not_in_the_closed_profile() {
        let failure = super::parse_runpath(
            std::path::Path::new("/object"),
            "/usr/lib",
            limits(),
        )
        .unwrap_err();
        assert_eq!(failure.code, "RUNTIME_CLOSURE_RUNPATH_UNSUPPORTED");
    }

    #[test]
    fn origin_runpath_preserves_loader_search_order() {
        let parsed = super::parse_runpath(
            std::path::Path::new("/object"),
            "$ORIGIN/z:$ORIGIN/a",
            limits(),
        )
        .unwrap();
        assert_eq!(parsed, ["$ORIGIN/z", "$ORIGIN/a"]);
    }

    #[test]
    fn rejects_zero_limits_before_reading_an_object() {
        let mut invalid = limits();
        invalid.object_count = 0;
        let failure = super::observe(&[], &[], invalid).unwrap_err();
        assert_eq!(failure.code, "RUNTIME_CLOSURE_ZERO_LIMIT");
    }
}

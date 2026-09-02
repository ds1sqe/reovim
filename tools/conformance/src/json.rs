//! Closed-object JSON parsing helpers.

use std::{collections::BTreeSet, fmt, fs, io, io::Read as _, path::Path};

use serde_json::{Map, Value};

/// A stable, path-addressed configuration failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

impl ConfigError {
    pub fn new(
        code: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self { code, path: path.into(), message: message.into() }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}: {}", self.code, self.path, self.message)
    }
}

impl std::error::Error for ConfigError {}

/// Reads bounded JSON, rejecting duplicate object members before `Value`
/// construction can erase them.
pub fn read(
    path: &Path,
    byte_limit: usize,
    depth_limit: usize,
    container_item_limit: usize,
) -> Result<(Vec<u8>, Value), ConfigError> {
    let metadata =
        fs::metadata(path).map_err(|error| io_error(path, &error))?;
    if metadata.len() > byte_limit as u64 {
        return Err(ConfigError::new(
            "CFG_SIZE_LIMIT",
            "$",
            format!(
                "{} is {} bytes; limit is {byte_limit}",
                path.display(),
                metadata.len()
            ),
        ));
    }
    let capacity = usize::try_from(metadata.len()).map_err(|error| {
        ConfigError::new(
            "CFG_SIZE_LIMIT",
            "$",
            format!("{}: {error}", path.display()),
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    fs::File::open(path)
        .map_err(|error| io_error(path, &error))?
        .take(u64::try_from(byte_limit).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(path, &error))?;
    if bytes.len() > byte_limit {
        return Err(ConfigError::new(
            "CFG_SIZE_LIMIT",
            "$",
            format!("{} grew beyond {byte_limit} bytes", path.display()),
        ));
    }
    let after = fs::metadata(path).map_err(|error| io_error(path, &error))?;
    if after.len() != metadata.len()
        || after.len()
            != u64::try_from(bytes.len()).map_err(|error| {
                ConfigError::new(
                    "CFG_SIZE_LIMIT",
                    "$",
                    format!("{}: {error}", path.display()),
                )
            })?
    {
        return Err(ConfigError::new(
            "CFG_CHANGED",
            "$",
            format!("{} changed while being read", path.display()),
        ));
    }
    let value = parse(&bytes, depth_limit, container_item_limit).map_err(
        |mut error| {
            error.message = format!("{}: {}", path.display(), error.message);
            error
        },
    )?;
    Ok((bytes, value))
}

/// Parses bounded-depth JSON bytes while retaining duplicate-key rejection.
pub fn parse(
    bytes: &[u8],
    depth_limit: usize,
    container_item_limit: usize,
) -> Result<Value, ConfigError> {
    reject_duplicate_keys(bytes, depth_limit, container_item_limit)?;
    serde_json::from_slice(bytes).map_err(|error| {
        ConfigError::new("CFG_JSON_SYNTAX", "$", error.to_string())
    })
}

pub fn object<'a>(
    value: &'a Value,
    path: &str,
    required: &[&str],
    optional: &[&str],
) -> Result<&'a Map<String, Value>, ConfigError> {
    let object = value
        .as_object()
        .ok_or_else(|| ConfigError::new("CFG_TYPE", path, "expected object"))?;
    for name in object.keys() {
        if !required.contains(&name.as_str())
            && !optional.contains(&name.as_str())
        {
            return Err(ConfigError::new(
                "CFG_UNKNOWN_FIELD",
                child(path, name),
                "field is not admitted by this schema",
            ));
        }
    }
    for name in required {
        if !object.contains_key(*name) {
            return Err(ConfigError::new(
                "CFG_MISSING_FIELD",
                child(path, name),
                "required field is absent",
            ));
        }
    }
    Ok(object)
}

pub fn array<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a [Value], ConfigError> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| ConfigError::new("CFG_TYPE", path, "expected array"))
}

pub fn string<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a str, ConfigError> {
    value
        .as_str()
        .ok_or_else(|| ConfigError::new("CFG_TYPE", path, "expected string"))
}

pub fn boolean(value: &Value, path: &str) -> Result<bool, ConfigError> {
    value
        .as_bool()
        .ok_or_else(|| ConfigError::new("CFG_TYPE", path, "expected boolean"))
}

pub fn unsigned(value: &Value, path: &str) -> Result<u64, ConfigError> {
    value.as_u64().ok_or_else(|| {
        ConfigError::new("CFG_TYPE", path, "expected unsigned integer")
    })
}

#[must_use]
pub fn child(parent: &str, field: &str) -> String {
    if parent == "$" {
        format!("$.{field}")
    } else {
        format!("{parent}.{field}")
    }
}

#[must_use]
pub fn index(parent: &str, item: usize) -> String {
    format!("{parent}[{item}]")
}

fn io_error(path: &Path, error: &io::Error) -> ConfigError {
    ConfigError::new("CFG_IO", "$", format!("{}: {error}", path.display()))
}

fn reject_duplicate_keys(
    input: &[u8],
    depth_limit: usize,
    container_item_limit: usize,
) -> Result<(), ConfigError> {
    let mut scanner = Scanner {
        input,
        cursor: 0,
        depth_limit,
        container_item_limit,
        container_items: 0,
    };
    scanner.value("$", 1)?;
    scanner.whitespace();
    if scanner.cursor != scanner.input.len() {
        return Err(scanner.syntax("$", "trailing bytes"));
    }
    Ok(())
}

struct Scanner<'a> {
    input: &'a [u8],
    cursor: usize,
    depth_limit: usize,
    container_item_limit: usize,
    container_items: usize,
}

impl Scanner<'_> {
    fn value(&mut self, path: &str, depth: usize) -> Result<(), ConfigError> {
        if depth > self.depth_limit {
            return Err(ConfigError::new(
                "CFG_JSON_DEPTH",
                path,
                format!("JSON nesting exceeds limit {}", self.depth_limit),
            ));
        }
        self.whitespace();
        match self.peek() {
            Some(b'{') => scan_object(self, path, depth),
            Some(b'[') => scan_array(self, path, depth),
            Some(b'"') => scan_string(self, path).map(|_| ()),
            Some(b't') => self.literal(path, b"true"),
            Some(b'f') => self.literal(path, b"false"),
            Some(b'n') => self.literal(path, b"null"),
            Some(b'-' | b'0'..=b'9') => self.number(path),
            _ => Err(self.syntax(path, "expected JSON value")),
        }
    }

    fn literal(
        &mut self,
        path: &str,
        expected: &[u8],
    ) -> Result<(), ConfigError> {
        if self.input.get(self.cursor..self.cursor + expected.len())
            == Some(expected)
        {
            self.cursor += expected.len();
            Ok(())
        } else {
            Err(self.syntax(path, "invalid literal"))
        }
    }

    fn number(&mut self, path: &str) -> Result<(), ConfigError> {
        let start = self.cursor;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.cursor += 1;
        }
        serde_json::from_slice::<serde_json::Number>(
            &self.input[start..self.cursor],
        )
        .map(|_| ())
        .map_err(|error| self.syntax(path, &error.to_string()))
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.cursor += 1;
        }
    }

    fn count_item(&mut self, path: &str) -> Result<(), ConfigError> {
        self.container_items = self.container_items.saturating_add(1);
        if self.container_items > self.container_item_limit {
            Err(ConfigError::new(
                "CFG_JSON_ITEMS",
                path,
                format!(
                    "JSON container items exceed limit {}",
                    self.container_item_limit
                ),
            ))
        } else {
            Ok(())
        }
    }

    fn take(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.cursor).copied()
    }

    fn syntax(&self, path: &str, message: &str) -> ConfigError {
        ConfigError::new(
            "CFG_JSON_SYNTAX",
            path,
            format!("byte {}: {message}", self.cursor),
        )
    }
}

fn scan_object(
    scanner: &mut Scanner<'_>,
    path: &str,
    depth: usize,
) -> Result<(), ConfigError> {
    scanner.cursor += 1;
    scanner.whitespace();
    if scanner.take(b'}') {
        return Ok(());
    }
    let mut keys = BTreeSet::new();
    loop {
        let complete = scan_object_item(scanner, path, depth, &mut keys)?;
        if complete {
            return Ok(());
        }
    }
}

fn scan_object_item(
    scanner: &mut Scanner<'_>,
    path: &str,
    depth: usize,
    keys: &mut BTreeSet<String>,
) -> Result<bool, ConfigError> {
    scanner.count_item(path)?;
    scanner.whitespace();
    let key = scan_string(scanner, path)?;
    let key_path = child(path, &key);
    if !keys.insert(key) {
        return Err(ConfigError::new(
            "CFG_DUPLICATE_FIELD",
            key_path,
            "duplicate object member",
        ));
    }
    scanner.whitespace();
    if !scanner.take(b':') {
        return Err(scanner.syntax(path, "expected ':'"));
    }
    scanner.value(&key_path, depth + 1)?;
    scanner.whitespace();
    if scanner.take(b'}') {
        return Ok(true);
    }
    if !scanner.take(b',') {
        return Err(scanner.syntax(path, "expected ',' or '}'"));
    }
    Ok(false)
}

fn scan_array(
    scanner: &mut Scanner<'_>,
    path: &str,
    depth: usize,
) -> Result<(), ConfigError> {
    scanner.cursor += 1;
    scanner.whitespace();
    if scanner.take(b']') {
        return Ok(());
    }
    let mut item = 0;
    loop {
        let complete = scan_array_item(scanner, path, depth, item)?;
        if complete {
            return Ok(());
        }
        item += 1;
    }
}

fn scan_array_item(
    scanner: &mut Scanner<'_>,
    path: &str,
    depth: usize,
    item: usize,
) -> Result<bool, ConfigError> {
    scanner.count_item(path)?;
    scanner.value(&index(path, item), depth + 1)?;
    scanner.whitespace();
    if scanner.take(b']') {
        return Ok(true);
    }
    if !scanner.take(b',') {
        return Err(scanner.syntax(path, "expected ',' or ']'"));
    }
    Ok(false)
}

fn scan_string(
    scanner: &mut Scanner<'_>,
    path: &str,
) -> Result<String, ConfigError> {
    if !scanner.take(b'"') {
        return Err(scanner.syntax(path, "expected string"));
    }
    let start = scanner.cursor - 1;
    let mut escaped = false;
    while let Some(byte) = scanner.peek() {
        scanner.cursor += 1;
        if let Some(value) =
            scan_string_byte(scanner, path, start, &mut escaped, byte)?
        {
            return Ok(value);
        }
    }
    Err(scanner.syntax(path, "unterminated string"))
}

fn scan_string_byte(
    scanner: &Scanner<'_>,
    path: &str,
    start: usize,
    escaped: &mut bool,
    byte: u8,
) -> Result<Option<String>, ConfigError> {
    if *escaped {
        *escaped = false;
        return Ok(None);
    }
    if byte == b'\\' {
        *escaped = true;
        return Ok(None);
    }
    if byte == b'"' {
        let value =
            serde_json::from_slice(&scanner.input[start..scanner.cursor])
                .map_err(|error| scanner.syntax(path, &error.to_string()))?;
        return Ok(Some(value));
    }
    if byte < 0x20 {
        return Err(scanner.syntax(path, "control byte in string"));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    #[test]
    fn duplicate_keys_are_rejected_before_value_construction() {
        let error = super::reject_duplicate_keys(br#"{"a":1,"a":2}"#, 16, 16)
            .unwrap_err();
        assert_eq!(error.code, "CFG_DUPLICATE_FIELD");
        assert_eq!(error.path, "$.a");
    }

    #[test]
    fn nested_json_without_duplicates_is_accepted() {
        super::reject_duplicate_keys(
            br#"{"a":[{"b":"x\\\"y"}],"c":null}"#,
            16,
            16,
        )
        .unwrap();
    }

    #[test]
    fn depth_and_item_limits_are_hard() {
        super::reject_duplicate_keys(br"[[[]]]", 3, 3).unwrap();
        assert_eq!(
            super::reject_duplicate_keys(br"[[[]]]", 2, 3).unwrap_err().code,
            "CFG_JSON_DEPTH"
        );
        assert_eq!(
            super::reject_duplicate_keys(br"[1,2]", 2, 1).unwrap_err().code,
            "CFG_JSON_ITEMS"
        );
    }
}

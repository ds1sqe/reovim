//! `PyO3` type bindings for kernel types.
//!
//! This module provides Python-visible wrappers for kernel types used in module
//! definitions: `ModuleId`, `Version`, and `ProbeResult`.

use {pyo3::prelude::*, reovim_kernel::api::v1};

// ============================================================================
// ModuleId
// ============================================================================

/// Python-visible `ModuleId` wrapper.
///
/// Represents a unique module identifier. Use kebab-case names like
/// "my-module" or "lang-python".
///
/// # Python Usage
///
/// ```python
/// from reovim import ModuleId
///
/// id = ModuleId("my-module")
/// print(id.as_str())  # "my-module"
/// print(str(id))      # "my-module"
/// ```
#[pyclass(name = "ModuleId", module = "reovim")]
#[derive(Clone)]
pub struct PyModuleId(pub(crate) v1::ModuleId);

#[pymethods]
impl PyModuleId {
    /// Create a new module identifier.
    ///
    /// # Arguments
    ///
    /// * `id` - The module identifier string (kebab-case recommended)
    #[new]
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self(v1::ModuleId::from_string(id.to_string()))
    }

    /// Get the identifier as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    fn __repr__(&self) -> String {
        format!("ModuleId('{}')", self.0.as_str())
    }

    fn __str__(&self) -> &str {
        self.0.as_str()
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.0.as_str().hash(&mut hasher);
        hasher.finish()
    }
}

impl From<v1::ModuleId> for PyModuleId {
    fn from(id: v1::ModuleId) -> Self {
        Self(id)
    }
}

impl From<PyModuleId> for v1::ModuleId {
    fn from(id: PyModuleId) -> Self {
        id.0
    }
}

// ============================================================================
// Version
// ============================================================================

/// Python-visible `Version` wrapper.
///
/// Represents a semantic version (major, minor, patch).
///
/// # Python Usage
///
/// ```python
/// from reovim import Version
///
/// v = Version(1, 2, 3)
/// print(v.major)  # 1
/// print(v.minor)  # 2
/// print(v.patch)  # 3
/// print(str(v))   # "1.2.3"
/// ```
#[pyclass(name = "Version", module = "reovim")]
#[derive(Clone, Copy)]
pub struct PyVersion(pub(crate) v1::Version);

// PyO3 methods cannot be `const` since they're called from Python.
#[allow(clippy::missing_const_for_fn)]
#[pymethods]
impl PyVersion {
    /// Create a new version.
    ///
    /// # Arguments
    ///
    /// * `major` - Major version (breaking changes)
    /// * `minor` - Minor version (backwards-compatible additions)
    /// * `patch` - Patch version (backwards-compatible fixes)
    #[new]
    #[must_use]
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self(v1::Version::new(major, minor, patch))
    }

    /// Major version number.
    #[getter]
    #[must_use]
    pub const fn major(&self) -> u32 {
        self.0.major
    }

    /// Minor version number.
    #[getter]
    #[must_use]
    pub const fn minor(&self) -> u32 {
        self.0.minor
    }

    /// Patch version number.
    #[getter]
    #[must_use]
    pub const fn patch(&self) -> u32 {
        self.0.patch
    }

    fn __repr__(&self) -> String {
        format!("Version({}, {}, {})", self.0.major, self.0.minor, self.0.patch)
    }

    fn __str__(&self) -> String {
        format!("{}.{}.{}", self.0.major, self.0.minor, self.0.patch)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    fn __lt__(&self, other: &Self) -> bool {
        self.0 < other.0
    }

    fn __le__(&self, other: &Self) -> bool {
        self.0 <= other.0
    }

    fn __gt__(&self, other: &Self) -> bool {
        self.0 > other.0
    }

    fn __ge__(&self, other: &Self) -> bool {
        self.0 >= other.0
    }

    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish()
    }
}

impl From<v1::Version> for PyVersion {
    fn from(ver: v1::Version) -> Self {
        Self(ver)
    }
}

impl From<PyVersion> for v1::Version {
    fn from(ver: PyVersion) -> Self {
        ver.0
    }
}

// ============================================================================
// ProbeResult
// ============================================================================

/// Internal representation of probe result variants.
#[derive(Clone)]
pub(crate) enum ProbeResultInner {
    Success,
    Defer(String),
    Failed(String),
}

/// Python-visible `ProbeResult` enum.
///
/// Result of module probe/initialization:
/// - `Success` - Module initialized successfully
/// - `Defer(reason)` - Retry later (like Linux `-EPROBE_DEFER`)
/// - `Failed(reason)` - Permanent failure
///
/// # Python Usage
///
/// ```python
/// from reovim import ProbeResult
///
/// # Success
/// result = ProbeResult.Success
///
/// # Defer (retry later)
/// result = ProbeResult.Defer("waiting for treesitter")
///
/// # Failed (permanent)
/// result = ProbeResult.Failed("configuration error")
/// ```
#[pyclass(name = "ProbeResult", module = "reovim")]
#[derive(Clone)]
pub struct PyProbeResult {
    pub(crate) inner: ProbeResultInner,
}

// PyO3 methods cannot be `const` since they're called from Python.
#[allow(clippy::missing_const_for_fn)]
#[pymethods]
impl PyProbeResult {
    /// Create a success result.
    ///
    /// Use this to indicate the module initialized successfully.
    #[staticmethod]
    #[pyo3(name = "Success", signature = ())]
    #[must_use]
    pub fn success() -> Self {
        Self {
            inner: ProbeResultInner::Success,
        }
    }

    /// Create a deferred result.
    ///
    /// Use this to indicate the module should be retried later.
    /// This is like Linux's `-EPROBE_DEFER` for driver probing.
    ///
    /// # Arguments
    ///
    /// * `reason` - Why initialization is being deferred
    #[staticmethod]
    #[pyo3(name = "Defer")]
    #[must_use]
    pub fn defer(reason: &str) -> Self {
        Self {
            inner: ProbeResultInner::Defer(reason.to_string()),
        }
    }

    /// Create a failed result.
    ///
    /// Use this to indicate a permanent failure that should not be retried.
    ///
    /// # Arguments
    ///
    /// * `reason` - Why initialization failed
    #[staticmethod]
    #[pyo3(name = "Failed")]
    #[must_use]
    pub fn failed(reason: &str) -> Self {
        Self {
            inner: ProbeResultInner::Failed(reason.to_string()),
        }
    }

    /// Check if this is a success result.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self.inner, ProbeResultInner::Success)
    }

    /// Check if this is a deferred result.
    #[must_use]
    pub fn is_defer(&self) -> bool {
        matches!(self.inner, ProbeResultInner::Defer(_))
    }

    /// Check if this is a failed result.
    #[must_use]
    pub fn is_failed(&self) -> bool {
        matches!(self.inner, ProbeResultInner::Failed(_))
    }

    /// Get the defer reason if this is a deferred result.
    #[must_use]
    pub fn defer_reason(&self) -> Option<String> {
        match &self.inner {
            ProbeResultInner::Defer(reason) => Some(reason.clone()),
            _ => None,
        }
    }

    /// Get the failure reason if this is a failed result.
    #[must_use]
    pub fn failure_reason(&self) -> Option<String> {
        match &self.inner {
            ProbeResultInner::Failed(reason) => Some(reason.clone()),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            ProbeResultInner::Success => "ProbeResult.Success".to_string(),
            ProbeResultInner::Defer(reason) => format!("ProbeResult.Defer('{reason}')"),
            ProbeResultInner::Failed(reason) => format!("ProbeResult.Failed('{reason}')"),
        }
    }

    fn __str__(&self) -> String {
        match &self.inner {
            ProbeResultInner::Success => "Success".to_string(),
            ProbeResultInner::Defer(reason) => format!("Defer: {reason}"),
            ProbeResultInner::Failed(reason) => format!("Failed: {reason}"),
        }
    }

    fn __eq__(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (ProbeResultInner::Success, ProbeResultInner::Success) => true,
            (ProbeResultInner::Defer(a), ProbeResultInner::Defer(b))
            | (ProbeResultInner::Failed(a), ProbeResultInner::Failed(b)) => a == b,
            _ => false,
        }
    }
}

impl PyProbeResult {
    /// Convert to kernel `ProbeResult`.
    #[must_use]
    pub fn to_kernel(&self) -> v1::ProbeResult {
        match &self.inner {
            ProbeResultInner::Success => v1::ProbeResult::Success,
            ProbeResultInner::Defer(reason) => v1::ProbeResult::Defer(reason.clone()),
            ProbeResultInner::Failed(reason) => {
                v1::ProbeResult::Failed(v1::ModuleError::InitFailed(reason.clone()))
            }
        }
    }

    /// Create from kernel `ProbeResult`.
    #[must_use]
    pub fn from_kernel(result: &v1::ProbeResult) -> Self {
        match result {
            v1::ProbeResult::Success => Self::success(),
            v1::ProbeResult::Defer(reason) => Self::defer(reason),
            v1::ProbeResult::Failed(err) => Self::failed(&err.to_string()),
        }
    }
}

// ============================================================================
// CommandRegistration
// ============================================================================

/// Python-visible `CommandRegistration` builder.
///
/// Registers a command that can be invoked by keybindings or other commands.
///
/// # Python Usage
///
/// ```python
/// from reovim import CommandRegistration
///
/// reg = CommandRegistration("my-command")
/// reg = reg.with_name("My Command")
/// reg = reg.with_description("Does something useful")
/// reg = reg.with_category("edit")
/// reg = reg.with_count()  # Accepts count prefix like 5j
/// reg = reg.with_motion()  # Accepts motion like dw
/// reg = reg.with_text_modifying()  # Modifies buffer text
/// ```
// Allow excessive bools - this mirrors the kernel's CommandRegistration struct
// which has the same boolean flags for command capabilities.
#[allow(clippy::struct_excessive_bools)]
#[pyclass(name = "CommandRegistration", module = "reovim")]
#[derive(Clone)]
pub struct PyCommandRegistration {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) category: Option<String>,
    pub(crate) accepts_count: bool,
    pub(crate) accepts_motion: bool,
    pub(crate) is_jump: bool,
    pub(crate) is_text_modifying: bool,
}

#[pymethods]
impl PyCommandRegistration {
    /// Create a new command registration.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique command identifier (e.g., "delete", "yank")
    #[new]
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            name: String::new(),
            description: String::new(),
            category: None,
            accepts_count: false,
            accepts_motion: false,
            is_jump: false,
            is_text_modifying: false,
        }
    }

    /// Set the display name (chainable).
    #[must_use]
    pub fn with_name(&self, name: &str) -> Self {
        let mut new = self.clone();
        new.name = name.to_string();
        new
    }

    /// Set the description (chainable).
    #[must_use]
    pub fn with_description(&self, description: &str) -> Self {
        let mut new = self.clone();
        new.description = description.to_string();
        new
    }

    /// Set the category (chainable).
    #[must_use]
    pub fn with_category(&self, category: &str) -> Self {
        let mut new = self.clone();
        new.category = Some(category.to_string());
        new
    }

    /// Mark command as accepting a count prefix (chainable).
    #[must_use]
    pub fn with_count(&self) -> Self {
        let mut new = self.clone();
        new.accepts_count = true;
        new
    }

    /// Mark command as accepting a motion (chainable).
    #[must_use]
    pub fn with_motion(&self) -> Self {
        let mut new = self.clone();
        new.accepts_motion = true;
        new
    }

    /// Mark command as a jump (recorded in jump list) (chainable).
    #[must_use]
    pub fn with_jump(&self) -> Self {
        let mut new = self.clone();
        new.is_jump = true;
        new
    }

    /// Mark command as text-modifying (chainable).
    #[must_use]
    pub fn with_text_modifying(&self) -> Self {
        let mut new = self.clone();
        new.is_text_modifying = true;
        new
    }

    fn __repr__(&self) -> String {
        format!("CommandRegistration('{}')", self.id)
    }
}

impl PyCommandRegistration {
    /// Convert to kernel `CommandRegistration`.
    ///
    /// Note: This leaks strings to get `&'static str`. This is acceptable
    /// because commands are typically registered once and live for the
    /// program's lifetime.
    #[must_use]
    pub fn to_kernel(&self) -> v1::CommandRegistration {
        let mut reg = v1::CommandRegistration::new(Box::leak(self.id.clone().into_boxed_str()))
            .with_name(Box::leak(self.name.clone().into_boxed_str()))
            .with_description(Box::leak(self.description.clone().into_boxed_str()));
        if let Some(cat) = &self.category {
            reg = reg.with_category(Box::leak(cat.clone().into_boxed_str()));
        }
        if self.accepts_count {
            reg = reg.with_count();
        }
        if self.accepts_motion {
            reg = reg.with_motion();
        }
        if self.is_jump {
            reg = reg.with_jump();
        }
        if self.is_text_modifying {
            reg = reg.with_text_modifying();
        }
        reg
    }
}

// ============================================================================
// KeybindingRegistration
// ============================================================================

/// Python-visible `KeybindingRegistration` builder.
///
/// Registers a keybinding that invokes a command.
///
/// # Python Usage
///
/// ```python
/// from reovim import KeybindingRegistration
///
/// # Basic keybinding
/// reg = KeybindingRegistration("dd", "delete-line")
///
/// # With options
/// reg = KeybindingRegistration("<C-w>h", "window-left")
/// reg = reg.with_modes(["normal"])
/// reg = reg.with_description("Move to left window")
/// reg = reg.with_category("window")
/// reg = reg.with_priority(50)
/// ```
#[pyclass(name = "KeybindingRegistration", module = "reovim")]
#[derive(Clone)]
pub struct PyKeybindingRegistration {
    pub(crate) keys: String,
    pub(crate) command_id: String,
    pub(crate) modes: Vec<String>,
    pub(crate) description: String,
    pub(crate) category: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) priority: u32,
}

#[pymethods]
impl PyKeybindingRegistration {
    /// Create a new keybinding registration.
    ///
    /// # Arguments
    ///
    /// * `keys` - Key sequence in vim notation (e.g., "dd", "<C-w>h")
    /// * `command_id` - Command ID to invoke
    #[new]
    #[must_use]
    pub fn new(keys: &str, command_id: &str) -> Self {
        Self {
            keys: keys.to_string(),
            command_id: command_id.to_string(),
            modes: Vec::new(),
            description: String::new(),
            category: None,
            enabled: true,
            priority: 100, // Default plugin priority
        }
    }

    /// Set active modes (chainable).
    ///
    /// Empty list means all modes.
    #[must_use]
    pub fn with_modes(&self, modes: Vec<String>) -> Self {
        let mut new = self.clone();
        new.modes = modes;
        new
    }

    /// Set description (chainable).
    #[must_use]
    pub fn with_description(&self, description: &str) -> Self {
        let mut new = self.clone();
        new.description = description.to_string();
        new
    }

    /// Set category (chainable).
    #[must_use]
    pub fn with_category(&self, category: &str) -> Self {
        let mut new = self.clone();
        new.category = Some(category.to_string());
        new
    }

    /// Disable the keybinding (chainable).
    #[must_use]
    pub fn with_disabled(&self) -> Self {
        let mut new = self.clone();
        new.enabled = false;
        new
    }

    /// Set priority (chainable).
    ///
    /// Lower values = higher priority.
    #[must_use]
    pub fn with_priority(&self, priority: u32) -> Self {
        let mut new = self.clone();
        new.priority = priority;
        new
    }

    fn __repr__(&self) -> String {
        format!("KeybindingRegistration('{}', '{}')", self.keys, self.command_id)
    }
}

impl PyKeybindingRegistration {
    /// Convert to kernel `KeybindingRegistration`.
    ///
    /// Note: This leaks strings to get `&'static str`.
    #[must_use]
    pub fn to_kernel(&self) -> v1::KeybindingRegistration {
        let modes: &'static [&'static str] = Box::leak(
            self.modes
                .iter()
                .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );

        v1::KeybindingRegistration {
            keys: Box::leak(self.keys.clone().into_boxed_str()),
            command_id: v1::CommandId::from_qualified_leaked(self.command_id.clone()),
            modes,
            description: Box::leak(self.description.clone().into_boxed_str()),
            category: self
                .category
                .as_ref()
                .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str),
            enabled: self.enabled,
            priority: self.priority,
            depends_on: &[],
            flags: v1::RegistrationFlags::new(),
        }
    }
}

// ============================================================================
// EventHandlerRegistration
// ============================================================================

/// Python-visible `EventHandlerRegistration` builder.
///
/// Registers an event handler for editor events.
///
/// # Python Usage
///
/// ```python
/// from reovim import EventHandlerRegistration
///
/// reg = EventHandlerRegistration("BufferChanged")
/// reg = reg.with_priority(50)
/// reg = reg.with_description("Update syntax highlighting")
/// reg = reg.with_once()  # One-shot handler
/// ```
#[pyclass(name = "EventHandlerRegistration", module = "reovim")]
#[derive(Clone)]
pub struct PyEventHandlerRegistration {
    pub(crate) event_type: String,
    pub(crate) priority: u32,
    pub(crate) description: String,
    pub(crate) once: bool,
    pub(crate) target_component: Option<String>,
}

#[pymethods]
impl PyEventHandlerRegistration {
    /// Create a new event handler registration.
    ///
    /// # Arguments
    ///
    /// * `event_type` - Event type name (e.g., `BufferChanged`, `CursorMoved`)
    #[new]
    #[must_use]
    pub fn new(event_type: &str) -> Self {
        Self {
            event_type: event_type.to_string(),
            priority: 100, // Default plugin priority
            description: String::new(),
            once: false,
            target_component: None,
        }
    }

    /// Set priority (chainable).
    ///
    /// Lower values = called earlier.
    /// Convention: 0-50 core, 100 plugin, 200+ cleanup.
    #[must_use]
    pub fn with_priority(&self, priority: u32) -> Self {
        let mut new = self.clone();
        new.priority = priority;
        new
    }

    /// Set description (chainable).
    #[must_use]
    pub fn with_description(&self, description: &str) -> Self {
        let mut new = self.clone();
        new.description = description.to_string();
        new
    }

    /// Mark as one-shot handler (chainable).
    ///
    /// Handler will auto-unsubscribe after first event.
    #[must_use]
    pub fn with_once(&self) -> Self {
        let mut new = self.clone();
        new.once = true;
        new
    }

    /// Set target component (chainable).
    #[must_use]
    pub fn with_target(&self, component: &str) -> Self {
        let mut new = self.clone();
        new.target_component = Some(component.to_string());
        new
    }

    fn __repr__(&self) -> String {
        format!("EventHandlerRegistration('{}')", self.event_type)
    }
}

impl PyEventHandlerRegistration {
    /// Convert to kernel `EventHandlerRegistration`.
    ///
    /// Note: This leaks strings to get `&'static str`.
    #[must_use]
    pub fn to_kernel(&self) -> v1::EventHandlerRegistration {
        v1::EventHandlerRegistration {
            event_type: Box::leak(self.event_type.clone().into_boxed_str()),
            priority: self.priority,
            description: Box::leak(self.description.clone().into_boxed_str()),
            once: self.once,
            target_component: self
                .target_component
                .as_ref()
                .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str),
            depends_on: &[],
            flags: v1::RegistrationFlags::new(),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ModuleId tests
    #[test]
    fn test_py_module_id_creation() {
        let id = PyModuleId::new("test-module");
        assert_eq!(id.as_str(), "test-module");
    }

    #[test]
    fn test_py_module_id_repr() {
        let id = PyModuleId::new("my-module");
        assert_eq!(id.__repr__(), "ModuleId('my-module')");
        assert_eq!(id.__str__(), "my-module");
    }

    #[test]
    fn test_py_module_id_equality() {
        let id1 = PyModuleId::new("test");
        let id2 = PyModuleId::new("test");
        let id3 = PyModuleId::new("other");
        assert!(id1.__eq__(&id2));
        assert!(!id1.__eq__(&id3));
    }

    #[test]
    fn test_py_module_id_hash() {
        let id1 = PyModuleId::new("test");
        let id2 = PyModuleId::new("test");
        assert_eq!(id1.__hash__(), id2.__hash__());
    }

    #[test]
    fn test_py_module_id_conversion() {
        let py_id = PyModuleId::new("test");
        let kernel_id: v1::ModuleId = py_id.clone().into();
        let back: PyModuleId = kernel_id.into();
        assert_eq!(py_id.as_str(), back.as_str());
    }

    // Version tests
    #[test]
    fn test_py_version_creation() {
        let ver = PyVersion::new(1, 2, 3);
        assert_eq!(ver.major(), 1);
        assert_eq!(ver.minor(), 2);
        assert_eq!(ver.patch(), 3);
    }

    #[test]
    fn test_py_version_repr() {
        let ver = PyVersion::new(1, 2, 3);
        assert_eq!(ver.__repr__(), "Version(1, 2, 3)");
        assert_eq!(ver.__str__(), "1.2.3");
    }

    #[test]
    fn test_py_version_equality() {
        let v1 = PyVersion::new(1, 0, 0);
        let v2 = PyVersion::new(1, 0, 0);
        let v3 = PyVersion::new(2, 0, 0);
        assert!(v1.__eq__(&v2));
        assert!(!v1.__eq__(&v3));
    }

    #[test]
    fn test_py_version_ordering() {
        let v1 = PyVersion::new(1, 0, 0);
        let v2 = PyVersion::new(1, 1, 0);
        let v3 = PyVersion::new(2, 0, 0);

        assert!(v1.__lt__(&v2));
        assert!(v1.__le__(&v2));
        assert!(v2.__lt__(&v3));
        assert!(v3.__gt__(&v1));
        assert!(v3.__ge__(&v1));
    }

    #[test]
    fn test_py_version_conversion() {
        let py_ver = PyVersion::new(1, 2, 3);
        let kernel_ver: v1::Version = py_ver.into();
        let back: PyVersion = kernel_ver.into();
        assert_eq!(py_ver.major(), back.major());
        assert_eq!(py_ver.minor(), back.minor());
        assert_eq!(py_ver.patch(), back.patch());
    }

    // ProbeResult tests
    #[test]
    fn test_py_probe_result_success() {
        let result = PyProbeResult::success();
        assert!(result.is_success());
        assert!(!result.is_defer());
        assert!(!result.is_failed());
        assert!(result.defer_reason().is_none());
        assert!(result.failure_reason().is_none());
    }

    #[test]
    fn test_py_probe_result_defer() {
        let result = PyProbeResult::defer("waiting for dependency");
        assert!(!result.is_success());
        assert!(result.is_defer());
        assert!(!result.is_failed());
        assert_eq!(result.defer_reason(), Some("waiting for dependency".to_string()));
        assert!(result.failure_reason().is_none());
    }

    #[test]
    fn test_py_probe_result_failed() {
        let result = PyProbeResult::failed("config error");
        assert!(!result.is_success());
        assert!(!result.is_defer());
        assert!(result.is_failed());
        assert!(result.defer_reason().is_none());
        assert_eq!(result.failure_reason(), Some("config error".to_string()));
    }

    #[test]
    fn test_py_probe_result_repr() {
        assert_eq!(PyProbeResult::success().__repr__(), "ProbeResult.Success");
        assert_eq!(PyProbeResult::defer("reason").__repr__(), "ProbeResult.Defer('reason')");
        assert_eq!(PyProbeResult::failed("error").__repr__(), "ProbeResult.Failed('error')");
    }

    #[test]
    fn test_py_probe_result_str() {
        assert_eq!(PyProbeResult::success().__str__(), "Success");
        assert_eq!(PyProbeResult::defer("reason").__str__(), "Defer: reason");
        assert_eq!(PyProbeResult::failed("error").__str__(), "Failed: error");
    }

    #[test]
    fn test_py_probe_result_equality() {
        assert!(PyProbeResult::success().__eq__(&PyProbeResult::success()));
        assert!(PyProbeResult::defer("a").__eq__(&PyProbeResult::defer("a")));
        assert!(PyProbeResult::failed("a").__eq__(&PyProbeResult::failed("a")));
        assert!(!PyProbeResult::success().__eq__(&PyProbeResult::defer("a")));
        assert!(!PyProbeResult::defer("a").__eq__(&PyProbeResult::defer("b")));
    }

    #[test]
    fn test_py_probe_result_to_kernel() {
        let success = PyProbeResult::success();
        assert!(matches!(success.to_kernel(), v1::ProbeResult::Success));

        let defer = PyProbeResult::defer("reason");
        if let v1::ProbeResult::Defer(reason) = defer.to_kernel() {
            assert_eq!(reason, "reason");
        } else {
            panic!("expected Defer");
        }

        let failed = PyProbeResult::failed("error");
        if let v1::ProbeResult::Failed(err) = failed.to_kernel() {
            assert!(err.to_string().contains("error"));
        } else {
            panic!("expected Failed");
        }
    }

    #[test]
    fn test_py_probe_result_from_kernel() {
        let success = PyProbeResult::from_kernel(&v1::ProbeResult::Success);
        assert!(success.is_success());

        let defer = PyProbeResult::from_kernel(&v1::ProbeResult::Defer("reason".to_string()));
        assert!(defer.is_defer());
        assert_eq!(defer.defer_reason(), Some("reason".to_string()));

        let failed = PyProbeResult::from_kernel(&v1::ProbeResult::Failed(
            v1::ModuleError::InitFailed("error".to_string()),
        ));
        assert!(failed.is_failed());
    }

    // ========================================================================
    // CommandRegistration tests
    // ========================================================================

    #[test]
    fn test_py_command_registration_creation() {
        let reg = PyCommandRegistration::new("my-command");
        assert_eq!(reg.id, "my-command");
        assert!(reg.name.is_empty());
        assert!(reg.description.is_empty());
        assert!(reg.category.is_none());
        assert!(!reg.accepts_count);
        assert!(!reg.accepts_motion);
        assert!(!reg.is_jump);
        assert!(!reg.is_text_modifying);
    }

    #[test]
    fn test_py_command_registration_builder() {
        let reg = PyCommandRegistration::new("delete")
            .with_name("Delete")
            .with_description("Delete text")
            .with_category("edit")
            .with_count()
            .with_motion()
            .with_jump()
            .with_text_modifying();

        assert_eq!(reg.id, "delete");
        assert_eq!(reg.name, "Delete");
        assert_eq!(reg.description, "Delete text");
        assert_eq!(reg.category, Some("edit".to_string()));
        assert!(reg.accepts_count);
        assert!(reg.accepts_motion);
        assert!(reg.is_jump);
        assert!(reg.is_text_modifying);
    }

    #[test]
    fn test_py_command_registration_repr() {
        let reg = PyCommandRegistration::new("test-cmd");
        assert_eq!(reg.__repr__(), "CommandRegistration('test-cmd')");
    }

    #[test]
    fn test_py_command_registration_to_kernel() {
        let reg = PyCommandRegistration::new("yank")
            .with_name("Yank")
            .with_description("Copy text")
            .with_category("edit")
            .with_count()
            .with_motion();

        let kernel_reg = reg.to_kernel();
        assert_eq!(kernel_reg.id, "yank");
        assert_eq!(kernel_reg.name, "Yank");
        assert_eq!(kernel_reg.description, "Copy text");
        assert_eq!(kernel_reg.category, Some("edit"));
        assert!(kernel_reg.accepts_count());
        assert!(kernel_reg.accepts_motion());
        assert!(!kernel_reg.is_jump());
        assert!(!kernel_reg.is_text_modifying());
    }

    // ========================================================================
    // KeybindingRegistration tests
    // ========================================================================

    #[test]
    fn test_py_keybinding_registration_creation() {
        let reg = PyKeybindingRegistration::new("dd", "delete-line");
        assert_eq!(reg.keys, "dd");
        assert_eq!(reg.command_id, "delete-line");
        assert!(reg.modes.is_empty());
        assert!(reg.description.is_empty());
        assert!(reg.category.is_none());
        assert!(reg.enabled);
        assert_eq!(reg.priority, 100);
    }

    #[test]
    fn test_py_keybinding_registration_builder() {
        let reg = PyKeybindingRegistration::new("<C-w>h", "window-left")
            .with_modes(vec!["normal".to_string()])
            .with_description("Move to left window")
            .with_category("window")
            .with_priority(50)
            .with_disabled();

        assert_eq!(reg.keys, "<C-w>h");
        assert_eq!(reg.command_id, "window-left");
        assert_eq!(reg.modes, vec!["normal"]);
        assert_eq!(reg.description, "Move to left window");
        assert_eq!(reg.category, Some("window".to_string()));
        assert!(!reg.enabled);
        assert_eq!(reg.priority, 50);
    }

    #[test]
    fn test_py_keybinding_registration_repr() {
        let reg = PyKeybindingRegistration::new("j", "move-down");
        assert_eq!(reg.__repr__(), "KeybindingRegistration('j', 'move-down')");
    }

    #[test]
    fn test_py_keybinding_registration_to_kernel() {
        let reg = PyKeybindingRegistration::new("dd", "editor:delete-line")
            .with_modes(vec!["normal".to_string(), "visual".to_string()])
            .with_description("Delete entire line")
            .with_priority(10);

        let kernel_reg = reg.to_kernel();
        assert_eq!(kernel_reg.keys, "dd");
        assert_eq!(kernel_reg.command_id.module().as_str(), "editor");
        assert_eq!(kernel_reg.command_id.name(), "delete-line");
        assert_eq!(kernel_reg.modes.len(), 2);
        assert_eq!(kernel_reg.modes[0], "normal");
        assert_eq!(kernel_reg.modes[1], "visual");
        assert_eq!(kernel_reg.description, "Delete entire line");
        assert!(kernel_reg.enabled);
        assert_eq!(kernel_reg.priority, 10);
    }

    // ========================================================================
    // EventHandlerRegistration tests
    // ========================================================================

    #[test]
    fn test_py_event_handler_registration_creation() {
        let reg = PyEventHandlerRegistration::new("BufferChanged");
        assert_eq!(reg.event_type, "BufferChanged");
        assert_eq!(reg.priority, 100);
        assert!(reg.description.is_empty());
        assert!(!reg.once);
        assert!(reg.target_component.is_none());
    }

    #[test]
    fn test_py_event_handler_registration_builder() {
        let reg = PyEventHandlerRegistration::new("CursorMoved")
            .with_priority(50)
            .with_description("Track cursor movement")
            .with_once()
            .with_target("editor");

        assert_eq!(reg.event_type, "CursorMoved");
        assert_eq!(reg.priority, 50);
        assert_eq!(reg.description, "Track cursor movement");
        assert!(reg.once);
        assert_eq!(reg.target_component, Some("editor".to_string()));
    }

    #[test]
    fn test_py_event_handler_registration_repr() {
        let reg = PyEventHandlerRegistration::new("ModeChanged");
        assert_eq!(reg.__repr__(), "EventHandlerRegistration('ModeChanged')");
    }

    #[test]
    fn test_py_event_handler_registration_to_kernel() {
        let reg = PyEventHandlerRegistration::new("BufferWrite")
            .with_priority(25)
            .with_description("Auto-save handler")
            .with_once()
            .with_target("buffer");

        let kernel_reg = reg.to_kernel();
        assert_eq!(kernel_reg.event_type, "BufferWrite");
        assert_eq!(kernel_reg.priority, 25);
        assert_eq!(kernel_reg.description, "Auto-save handler");
        assert!(kernel_reg.once);
        assert_eq!(kernel_reg.target_component, Some("buffer"));
    }
}

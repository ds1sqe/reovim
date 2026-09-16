//! Checked API declarations for native and service operation binding.
//!
//! ```
//! use reovim_kernel_interface::declaration::Declaration;
//! assert!(Declaration::decode(&[0; 20]).is_err()); // Zero API is reserved.
//! ```

use alloc::{boxed::Box, vec::Vec};

use crate::Error;

mod decode;
mod references;
mod value;
pub use value::ValueLimits;

/// Validated canonical declaration, independent of provider code and storage.
///
/// Equality compares the complete declaration, including API identity.
/// ```
/// use reovim_kernel_interface::declaration::Declaration;
/// let mut empty = [0; 20];
/// empty[0] = 7; // Nonzero API key; no offered operations.
/// let api = Declaration::decode(&empty).unwrap();
/// assert_eq!(api.api()[0], 7);
/// assert!(api.methods().is_empty());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Declaration {
    api: [u8; 16],
    methods: Vec<Method>,
    bytes: Box<[u8]>,
}

impl Declaration {
    /// Copies a complete declaration after checking its bounded grammar.
    ///
    /// # Errors
    /// Returns Invalid for malformed declarations and Pressure for admission
    /// bounds or allocation failure. No provider callback executes here.
    /// ```
    /// use reovim_kernel_interface::{Error, declaration::Declaration};
    /// assert_eq!(Declaration::decode(&[1]), Err(Error::INVALID));
    /// ```
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        decode::declaration(bytes)
    }

    /// Returns the API package's opaque identity, never an access grant.
    /// Example: `offered.api() == expected.api()` checks only the key;
    /// `offered == expected` checks the complete declaration.
    #[must_use]
    pub const fn api(&self) -> &[u8; 16] {
        &self.api
    }

    /// Returns validated methods ordered by operation family and number.
    /// Example: `api.methods().iter().find(|m| m.name() == "section")` supports
    /// diagnostic/CLI discovery without interpreting the name as dispatch.
    #[must_use]
    pub fn methods(&self) -> &[Method] {
        &self.methods
    }

    /// Returns canonical descriptor bytes for native/service advertisement.
    /// Example: `Declaration::decode(api.bytes())? == api` round-trips the
    /// full definition; callers may not substitute an output schema.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Selection within one bound interface.
/// ```
/// use reovim_kernel_interface::declaration::Operation;
/// assert_ne!(Operation::Exec, Operation::Ioctl(1));
/// ```
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Operation {
    /// Invoke the bound executable. Example: `Operation::Exec` has no opcode.
    Exec,
    /// Select a local control. Example: `Operation::Ioctl(2)` selects command
    /// two; zero is rejected when decoding a declaration.
    Ioctl(u32),
}

/// Provider authority required by the method, separate from operation family.
/// ```
/// use reovim_kernel_interface::declaration::Dispatch;
/// assert_ne!(Dispatch::Edit, Dispatch::Call);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dispatch {
    /// Ordinary pending call, e.g. `Dispatch::Call` for section inspection.
    Call,
    /// State preparation. Example: a splice uses `Dispatch::Edit`, so only
    /// Core can install its candidate and History transition.
    Edit,
    /// Creation staging. Example: Parse uses `Dispatch::Factory` to return
    /// an owned immutable tree subject.
    Factory,
}

/// One validated method; construction is restricted to declaration decoding.
///
/// Example: select a method with `api.methods().iter().find(|method|
/// method.operation() == Operation::Ioctl(2))` before checking its input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Method {
    operation: Operation,
    dispatch: Dispatch,
    access: u32,
    activity: u8,
    name: Box<str>,
    input: ValueType,
    output: ValueType,
    progress: ValueType,
    created: Option<ValueType>,
}

impl Method {
    /// Returns the local selector. Example: `method.operation()` distinguishes
    /// Exec from Ioctl without inspecting its diagnostic name.
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }

    /// Returns its preparation authority. Example: `method.dispatch() ==
    /// Dispatch::Edit` requires a state preparation table, not mutable control.
    #[must_use]
    pub const fn dispatch(&self) -> Dispatch {
        self.dispatch
    }

    /// Returns required READ=1, WRITE=2, EXEC=4 subject rights.
    /// Example: `held & method.access() == method.access()` checks inclusion;
    /// Runtime separately checks liveness and each transferred reference.
    #[must_use]
    pub const fn access(&self) -> u32 {
        self.access
    }

    /// Returns allowed READ=1, WRITE=2, STATUS=4 metadata events.
    /// Example: `reported & !method.activity() == 0` rejects undeclared events.
    #[must_use]
    pub const fn activity(&self) -> u8 {
        self.activity
    }

    /// Returns the diagnostic/CLI name. Example: `method.name()` can label a
    /// command listing; it does not identify the API or grant invocation.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns its fixed argument grammar. Example: `method.input().bytes()`
    /// describes the canonical input for a generic caller.
    #[must_use]
    pub const fn input(&self) -> &ValueType {
        &self.input
    }

    /// Returns its fixed result grammar. Example: `method.output().bytes()`
    /// selects decoding even when a provider reports malformed result bytes.
    #[must_use]
    pub const fn output(&self) -> &ValueType {
        &self.output
    }

    /// Returns the UNKNOWN progress grammar, which cannot contain Ref.
    /// Example: `method.progress().validate(data, 0, limits, check)?` checks
    /// copied evidence without transferring access from the pending request.
    #[must_use]
    pub const fn progress(&self) -> &ValueType {
        &self.progress
    }

    /// Returns the Factory's primary Ref grammar; Call and Edit return None.
    /// Example: a Parse factory qualifies its created access with
    /// `Ref(TreeSnapshot, READ)` independently of its auxiliary output.
    #[must_use]
    pub const fn created(&self) -> Option<&ValueType> {
        self.created.as_ref()
    }
}

/// A finite, bounded canonical value grammar, without domain storage types.
/// ```
/// use reovim_kernel_interface::declaration::ValueType;
/// // UTF-8 with a maximum of 256 bytes.
/// let text = ValueType::decode(&[8, 0, 1, 0, 0]).unwrap();
/// assert_ne!(text, ValueType::decode(&[4]).unwrap()); // U64.
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueType(Box<[u8]>);

impl ValueType {
    /// Bounds distinct reference slots before execution, capped by admission.
    /// Example: Unit needs no reservation; one Ref needs at most one slot.
    /// ```
    /// use reovim_kernel_interface::declaration::ValueType;
    /// assert_eq!(ValueType::decode(&[0]).unwrap().reference_bound(8), 0);
    /// ```
    #[must_use]
    pub fn reference_bound(&self, limit: usize) -> usize {
        references::bound(self, limit)
    }

    /// Checks canonical values and every declared reference use.
    ///
    /// `check_reference` validates owner, liveness, rights and complete API
    /// compatibility, including zero-right access, using Runtime metadata.
    /// It checks metadata only. Later input may reject, so it cannot run
    /// providers or adopt ownership.
    /// Repeated indices are checked per use and cannot amplify grants.
    ///
    /// # Errors
    /// Returns Invalid for malformed values or unused slots, Pressure for
    /// bounds/allocation failure, or the reference check's error.
    /// ```
    /// use reovim_kernel_interface::declaration::{
    ///     ValueLimits, ValueType,
    /// };
    /// let boolean = ValueType::decode(&[1]).unwrap();
    /// let limits = ValueLimits {
    ///     bytes: 8, elements: 8, references: 0,
    /// };
    /// assert!(boolean.validate(&[1], 0, limits, |_, _, _| unreachable!())
    ///     .is_ok());
    /// assert!(boolean.validate(&[2], 0, limits, |_, _, _| unreachable!())
    ///     .is_err());
    /// ```
    pub fn validate<F>(
        &self,
        data: &[u8],
        references: usize,
        limits: ValueLimits,
        check_reference: F,
    ) -> Result<(), Error>
    where
        F: FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
    {
        value::validate(self, data, references, limits, check_reference)
    }

    pub(crate) fn prefix<'a>(
        &self,
        data: &'a [u8],
        references: usize,
        limits: ValueLimits,
        check: impl FnMut(u32, Option<[u8; 16]>, u32) -> Result<(), Error>,
    ) -> Result<&'a [u8], Error> {
        value::prefix(self, data, references, limits, check)
    }

    /// Validates and copies one complete Type descriptor.
    ///
    /// # Errors
    /// Returns Invalid for malformed types; Pressure for admission bounds or
    /// allocation failure. This checks a grammar, not actual values or grants.
    /// ```
    /// use reovim_kernel_interface::declaration::ValueType;
    /// assert!(ValueType::decode(&[255]).is_err()); // Unknown grammar tag.
    /// ```
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        decode::value_type(bytes)
    }

    /// Returns the validated canonical Type descriptor.
    /// ```
    /// use reovim_kernel_interface::declaration::ValueType;
    /// assert_eq!(ValueType::decode(&[1]).unwrap().bytes(), &[1]); // Bool.
    /// ```
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

//! Versioned safety declarations for opaque carrier families.

use core::fmt;

use crate::contract::{CompatibleVersionRange, ContractId};

/// Ownership held by a value crossing a carrier boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnershipModel {
    /// The receiver owns the carrier value.
    Owned,
    /// The receiver holds only a bounded borrow.
    Borrowed,
}

/// Borrow operations admitted for a carrier family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorrowModel {
    /// A carrier value cannot be borrowed across the boundary.
    Forbidden,
    /// Immutable shared borrowing is admitted.
    Shared,
    /// One exclusive borrow is admitted.
    Exclusive,
}

/// Copy behavior admitted for a carrier family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyModel {
    /// Ownership moves and the carrier is not copied.
    MoveOnly,
    /// A bitwise-independent value copy is admitted.
    Copy,
}

/// Drop behavior owned by a carrier value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropModel {
    /// Dropping the value releases no separately owned state.
    Trivial,
    /// Dropping the value releases state owned by the value.
    ReleasesOwnedState,
}

/// Lifetime relation required by a carrier family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifetimeModel {
    /// The carrier is a self-contained value.
    Value,
    /// The carrier is valid only for one instance lifetime.
    InstanceBound,
    /// The carrier is valid only for one incarnation lifetime.
    IncarnationBound,
}

/// Result required when carrier type or schema identity differs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeMismatch {
    /// Reject the value without reinterpretation or fallback.
    Reject,
}

/// Thread movement admitted for a carrier family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadTransfer {
    /// The value may move between threads under its sharing rule.
    Transferable,
    /// The value remains on its declared owner thread.
    ThreadAffine,
}

/// Concurrent sharing admitted for a carrier family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharingModel {
    /// One owner has exclusive access.
    Exclusive,
    /// Immutable sharing is admitted.
    Immutable,
    /// Sharing requires contract-declared synchronization.
    Synchronized,
}

/// Invocation behavior of a carrier family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvocationModel {
    /// The carrier holds data and cannot be invoked.
    DataOnly,
    /// Invocation is admitted without a user callback.
    NoUserCallback,
    /// Invocation may enter a separately declared user callback.
    UserCallback,
}

/// Panic and unwind behavior at a carrier boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnwindModel {
    /// The carrier is not invocable and cannot unwind by contract.
    NotInvocable,
    /// Unwind is contained and translated at the boundary.
    Contained,
    /// Unwind across the boundary is forbidden.
    ForbiddenAcrossBoundary,
}

/// Unsafe implementation boundary admitted for a carrier family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsafeBoundary {
    /// The carrier implementation contains no unsafe boundary.
    None,
    /// One separately identified and reviewed boundary is admitted.
    Reviewed(ContractId),
}

/// Reason one memory-safety declaration is internally contradictory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryCoherenceError {
    /// Borrowed ownership requires an admitted borrow across the boundary.
    BorrowedWithoutBorrow,
    /// Borrowed ownership cannot claim a self-contained value lifetime.
    BorrowedValueLifetime,
    /// A borrowed value cannot release state that it does not own.
    BorrowedReleasesOwnedState,
    /// Copying cannot duplicate authority to release owned state.
    CopyDuplicatesReleaseAuthority,
}

/// Failure to construct one coherent carrier-safety declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarrierDescriptorError {
    /// The memory declaration contains contradictory authority.
    Memory(MemoryCoherenceError),
    /// Invocation and unwind behavior cannot both hold.
    InvocationUnwind {
        /// Rejected invocation model.
        invocation: InvocationModel,
        /// Rejected unwind model.
        unwind: UnwindModel,
    },
}

impl CarrierDescriptorError {
    const fn invocation_unwind(
        invocation: InvocationModel,
        unwind: UnwindModel,
    ) -> Self {
        Self::InvocationUnwind { invocation, unwind }
    }

    const fn memory_conflict(error: MemoryCoherenceError) -> Self {
        Self::Memory(error)
    }

    /// Returns the rejected memory relation, when memory was incoherent.
    #[must_use]
    pub const fn memory(self) -> Option<MemoryCoherenceError> {
        match self {
            Self::Memory(error) => Some(error),
            Self::InvocationUnwind { .. } => None,
        }
    }

    /// Returns the rejected invocation model, when invocation was incoherent.
    #[must_use]
    pub const fn invocation(self) -> Option<InvocationModel> {
        match self {
            Self::Memory(_) => None,
            Self::InvocationUnwind { invocation, .. } => Some(invocation),
        }
    }

    /// Returns the rejected unwind model, when invocation was incoherent.
    #[must_use]
    pub const fn unwind(self) -> Option<UnwindModel> {
        match self {
            Self::Memory(_) => None,
            Self::InvocationUnwind { unwind, .. } => Some(unwind),
        }
    }
}

impl fmt::Display for CarrierDescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Memory(_) => "carrier memory-safety models conflict",
            Self::InvocationUnwind { .. } => {
                "carrier invocation and unwind models conflict"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CarrierDescriptorError {}

/// Ownership, borrow, copy, drop, and lifetime declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemorySafety {
    ownership: OwnershipModel,
    borrowing: BorrowModel,
    copying: CopyModel,
    dropping: DropModel,
    lifetime: LifetimeModel,
}

impl MemorySafety {
    /// Constructs explicit memory-safety terms.
    ///
    /// Cross-term coherence is checked by [`CarrierSafetyDescriptor::new`].
    #[must_use]
    pub const fn new(
        ownership: OwnershipModel,
        borrowing: BorrowModel,
        copying: CopyModel,
        dropping: DropModel,
        lifetime: LifetimeModel,
    ) -> Self {
        Self { ownership, borrowing, copying, dropping, lifetime }
    }

    /// Returns the ownership model.
    #[must_use]
    pub const fn ownership(self) -> OwnershipModel {
        self.ownership
    }

    /// Returns the borrowing model.
    #[must_use]
    pub const fn borrowing(self) -> BorrowModel {
        self.borrowing
    }

    /// Returns the copy model.
    #[must_use]
    pub const fn copying(self) -> CopyModel {
        self.copying
    }

    /// Returns the drop model.
    #[must_use]
    pub const fn dropping(self) -> DropModel {
        self.dropping
    }

    /// Returns the lifetime model.
    #[must_use]
    pub const fn lifetime(self) -> LifetimeModel {
        self.lifetime
    }
}

/// Thread-transfer and sharing declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConcurrencySafety {
    thread_transfer: ThreadTransfer,
    sharing: SharingModel,
}

impl ConcurrencySafety {
    /// Constructs an explicit concurrency-safety declaration.
    #[must_use]
    pub const fn new(
        thread_transfer: ThreadTransfer,
        sharing: SharingModel,
    ) -> Self {
        Self { thread_transfer, sharing }
    }

    /// Returns the thread-transfer model.
    #[must_use]
    pub const fn thread_transfer(self) -> ThreadTransfer {
        self.thread_transfer
    }

    /// Returns the sharing model.
    #[must_use]
    pub const fn sharing(self) -> SharingModel {
        self.sharing
    }
}

/// Invocation and panic/unwind declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvocationSafety {
    invocation: InvocationModel,
    unwind: UnwindModel,
}

impl InvocationSafety {
    /// Constructs an explicit invocation-safety declaration.
    #[must_use]
    pub const fn new(invocation: InvocationModel, unwind: UnwindModel) -> Self {
        Self { invocation, unwind }
    }

    /// Returns the invocation model.
    #[must_use]
    pub const fn invocation(self) -> InvocationModel {
        self.invocation
    }

    /// Returns the unwind model.
    #[must_use]
    pub const fn unwind(self) -> UnwindModel {
        self.unwind
    }
}

/// Complete versioned safety declaration for one opaque carrier family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CarrierSafetyDescriptor {
    type_id: ContractId,
    schema_versions: CompatibleVersionRange,
    memory: MemorySafety,
    concurrency: ConcurrencySafety,
    invocation: InvocationSafety,
    mismatch: TypeMismatch,
    unsafe_boundary: UnsafeBoundary,
}

impl CarrierSafetyDescriptor {
    /// Constructs a complete carrier declaration.
    ///
    /// # Errors
    ///
    /// Returns [`CarrierDescriptorError`] when memory authority or callable
    /// and unwind behavior do not form one coherent boundary contract.
    pub const fn new(
        type_id: ContractId,
        schema_versions: CompatibleVersionRange,
        memory: MemorySafety,
        concurrency: ConcurrencySafety,
        invocation: InvocationSafety,
        mismatch: TypeMismatch,
        unsafe_boundary: UnsafeBoundary,
    ) -> Result<Self, CarrierDescriptorError> {
        if let Some(error) = memory_coherence_error(memory) {
            return Err(CarrierDescriptorError::memory_conflict(error));
        }
        if !invocation_unwind_coherent(invocation, invocation.unwind()) {
            return Err(CarrierDescriptorError::invocation_unwind(
                invocation.invocation(),
                invocation.unwind(),
            ));
        }
        Ok(Self {
            type_id,
            schema_versions,
            memory,
            concurrency,
            invocation,
            mismatch,
            unsafe_boundary,
        })
    }

    /// Returns the carrier type/schema family identity.
    #[must_use]
    pub const fn type_id(self) -> ContractId {
        self.type_id
    }

    /// Returns the compatible carrier schema versions.
    #[must_use]
    pub const fn schema_versions(self) -> CompatibleVersionRange {
        self.schema_versions
    }

    /// Returns the memory-safety declaration.
    #[must_use]
    pub const fn memory(self) -> MemorySafety {
        self.memory
    }

    /// Returns the concurrency-safety declaration.
    #[must_use]
    pub const fn concurrency(self) -> ConcurrencySafety {
        self.concurrency
    }

    /// Returns the invocation-safety declaration.
    #[must_use]
    pub const fn invocation(self) -> InvocationSafety {
        self.invocation
    }

    /// Returns the required mismatch outcome.
    #[must_use]
    pub const fn mismatch(self) -> TypeMismatch {
        self.mismatch
    }

    /// Returns the declared unsafe implementation boundary.
    #[must_use]
    pub const fn unsafe_boundary(self) -> UnsafeBoundary {
        self.unsafe_boundary
    }
}

const fn memory_coherence_error(
    memory: MemorySafety,
) -> Option<MemoryCoherenceError> {
    if let Some(error) = ownership_borrow_error(memory) {
        return Some(error);
    }
    if let Some(error) = ownership_lifetime_error(memory) {
        return Some(error);
    }
    if let Some(error) = ownership_drop_error(memory) {
        return Some(error);
    }
    copy_drop_error(memory)
}

const fn ownership_borrow_error(
    memory: MemorySafety,
) -> Option<MemoryCoherenceError> {
    match (memory.ownership(), memory.borrowing()) {
        (OwnershipModel::Borrowed, BorrowModel::Forbidden) => {
            Some(MemoryCoherenceError::BorrowedWithoutBorrow)
        }
        _ => None,
    }
}

const fn ownership_lifetime_error(
    memory: MemorySafety,
) -> Option<MemoryCoherenceError> {
    match (memory.ownership(), memory.lifetime()) {
        (OwnershipModel::Borrowed, LifetimeModel::Value) => {
            Some(MemoryCoherenceError::BorrowedValueLifetime)
        }
        _ => None,
    }
}

const fn ownership_drop_error(
    memory: MemorySafety,
) -> Option<MemoryCoherenceError> {
    match (memory.ownership(), memory.dropping()) {
        (OwnershipModel::Borrowed, DropModel::ReleasesOwnedState) => {
            Some(MemoryCoherenceError::BorrowedReleasesOwnedState)
        }
        _ => None,
    }
}

const fn copy_drop_error(memory: MemorySafety) -> Option<MemoryCoherenceError> {
    match (memory.copying(), memory.dropping()) {
        (CopyModel::Copy, DropModel::ReleasesOwnedState) => {
            Some(MemoryCoherenceError::CopyDuplicatesReleaseAuthority)
        }
        _ => None,
    }
}

const fn invocation_unwind_coherent(
    invocation: InvocationSafety,
    unwind: UnwindModel,
) -> bool {
    let code = unwind_code(unwind);
    match invocation.invocation() {
        InvocationModel::DataOnly => code == 0,
        InvocationModel::NoUserCallback | InvocationModel::UserCallback => {
            code != 0
        }
    }
}

const fn unwind_code(unwind: UnwindModel) -> u8 {
    match unwind {
        UnwindModel::NotInvocable => 0,
        UnwindModel::Contained => 1,
        UnwindModel::ForbiddenAcrossBoundary => 2,
    }
}

//! Public carrier-safety vocabulary used across Kernel Interface contracts.

pub use ::reovim_foundation::carrier::{
    BorrowModel, CarrierDescriptorError, CarrierSafetyDescriptor,
    ConcurrencySafety, CopyModel, DropModel, InvocationModel, InvocationSafety,
    LifetimeModel, MemoryCoherenceError, MemorySafety, OwnershipModel,
    SharingModel, ThreadTransfer, TypeMismatch, UnsafeBoundary, UnwindModel,
};

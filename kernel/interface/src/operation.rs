//! Orthogonal operation and contract-selection descriptors.

use core::fmt;

use reovim_foundation::contract::{
    CompatibleVersionRange, ContractId, ContractVersion,
};

/// Whether invocation returns after bounded work or may wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvocationClass {
    /// Returns after bounded in-process work without external I/O or waiting.
    NonBlockingInvocation,
    /// May wait only within a declared deadline and cancellation contract.
    CallerMayWait,
}

/// Whether semantic terminal completion is inline or deferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionClass {
    /// Completes inside one non-blocking invocation.
    InlineTerminal,
    /// Remains owned after admission and terminates later.
    DeferredTerminal,
}

/// Whether an operation delivers one result or a bounded stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryClass {
    /// Delivers one bounded terminal result.
    Single,
    /// Delivers a bounded ordered sequence under a stream contract.
    Stream,
}

/// Where the observable effect of an operation occurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectClass {
    /// Performs pure computation or changes only Reovim-owned local state.
    LocalOnly,
    /// Observes external state without claiming target mutation.
    ExternalObservation,
    /// May mutate an external target under explicit effect semantics.
    ExternalMutation,
}

/// Whether an operation may enter user-controlled code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallbackClass {
    /// Invokes no user, Domain-policy, or extension callback.
    NoUserCallback,
    /// May invoke a callback only under its separately declared contract.
    MayInvokeUserCallback,
}

/// Whether nested entry into the same operation is admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReentrancyClass {
    /// Nested entry is refused or follows a bounded deferred rule.
    NonReentrant,
    /// Nested entry is admitted only within declared bounds and ordering.
    Reentrant,
}

/// An invalid relation between otherwise independent operation axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationDescriptorError {
    /// Inline completion cannot use an invocation that may wait.
    InlineMayNotWait,
    /// Stream delivery requires deferred terminal completion.
    StreamRequiresDeferred,
    /// Inline completion cannot invoke user-controlled code.
    InlineMayNotInvokeCallback,
}

impl fmt::Display for OperationDescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InlineMayNotWait => "inline completion may not wait",
            Self::StreamRequiresDeferred => {
                "stream delivery requires deferred completion"
            }
            Self::InlineMayNotInvokeCallback => {
                "inline completion may not invoke a user callback"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for OperationDescriptorError {}

/// One validated declaration of all six operation axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationDescriptor {
    invocation: InvocationClass,
    completion: CompletionClass,
    delivery: DeliveryClass,
    effect: EffectClass,
    callback: CallbackClass,
    reentrancy: ReentrancyClass,
}

impl OperationDescriptor {
    /// Constructs a descriptor after validating cross-axis requirements.
    ///
    /// # Errors
    ///
    /// Returns a typed error when inline completion may wait, stream, or invoke
    /// user-controlled code.
    pub const fn new(
        invocation: InvocationClass,
        completion: CompletionClass,
        delivery: DeliveryClass,
        effect: EffectClass,
        callback: CallbackClass,
        reentrancy: ReentrancyClass,
    ) -> Result<Self, OperationDescriptorError> {
        if inline_may_wait(invocation, completion) {
            return Err(OperationDescriptorError::InlineMayNotWait);
        }
        if inline_streams(completion, delivery) {
            return Err(OperationDescriptorError::StreamRequiresDeferred);
        }
        if inline_calls_user(completion, callback) {
            return Err(OperationDescriptorError::InlineMayNotInvokeCallback);
        }
        Ok(Self {
            invocation,
            completion,
            delivery,
            effect,
            callback,
            reentrancy,
        })
    }

    /// Returns the fixed inline, single-result, local operation shape.
    #[must_use]
    pub const fn local_inline() -> Self {
        Self {
            invocation: InvocationClass::NonBlockingInvocation,
            completion: CompletionClass::InlineTerminal,
            delivery: DeliveryClass::Single,
            effect: EffectClass::LocalOnly,
            callback: CallbackClass::NoUserCallback,
            reentrancy: ReentrancyClass::NonReentrant,
        }
    }

    /// Returns the fixed deferred, single-result, local operation shape.
    #[must_use]
    pub(crate) const fn local_deferred() -> Self {
        Self {
            invocation: InvocationClass::CallerMayWait,
            completion: CompletionClass::DeferredTerminal,
            delivery: DeliveryClass::Single,
            effect: EffectClass::LocalOnly,
            callback: CallbackClass::NoUserCallback,
            reentrancy: ReentrancyClass::NonReentrant,
        }
    }

    /// Returns the fixed inline external-observation operation shape.
    #[must_use]
    pub const fn external_observation_inline() -> Self {
        Self {
            invocation: InvocationClass::NonBlockingInvocation,
            completion: CompletionClass::InlineTerminal,
            delivery: DeliveryClass::Single,
            effect: EffectClass::ExternalObservation,
            callback: CallbackClass::NoUserCallback,
            reentrancy: ReentrancyClass::NonReentrant,
        }
    }

    /// Returns the invocation classification.
    #[must_use]
    pub const fn invocation(self) -> InvocationClass {
        self.invocation
    }

    /// Returns the completion classification.
    #[must_use]
    pub const fn completion(self) -> CompletionClass {
        self.completion
    }

    /// Returns the delivery classification.
    #[must_use]
    pub const fn delivery(self) -> DeliveryClass {
        self.delivery
    }

    /// Returns the effect classification.
    #[must_use]
    pub const fn effect(self) -> EffectClass {
        self.effect
    }

    /// Returns the callback classification.
    #[must_use]
    pub const fn callback(self) -> CallbackClass {
        self.callback
    }

    /// Returns the reentrancy classification.
    #[must_use]
    pub const fn reentrancy(self) -> ReentrancyClass {
        self.reentrancy
    }
}

/// Failure to select one compatible contract version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractSelectionError {
    /// The compatible range placed its minimum above its maximum.
    InvertedRange {
        /// Rejected inclusive lower version bound.
        minimum: u64,
        /// Rejected inclusive upper version bound.
        maximum: u64,
    },
    /// The selected version lies outside the compatible range.
    SelectedOutsideRange {
        /// Rejected selected contract version.
        selected: u64,
        /// Inclusive lower compatible version bound.
        minimum: u64,
        /// Inclusive upper compatible version bound.
        maximum: u64,
    },
}

impl fmt::Display for ContractSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvertedRange { .. } => {
                "compatible contract range is inverted"
            }
            Self::SelectedOutsideRange { .. } => {
                "selected contract version is incompatible"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ContractSelectionError {}

/// One explicit contract identity and compatible selected version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContractSelection {
    identity: ContractId,
    selected: ContractVersion,
    compatible: CompatibleVersionRange,
}

impl ContractSelection {
    /// Constructs a selection from contract-authority supplied scalars.
    ///
    /// # Errors
    ///
    /// Returns [`ContractSelectionError::InvertedRange`] when the range is
    /// inverted, or [`ContractSelectionError::SelectedOutsideRange`] when the
    /// selected version is not compatible.
    pub const fn new(
        identity: u128,
        selected: u64,
        minimum: u64,
        maximum: u64,
    ) -> Result<Self, ContractSelectionError> {
        let minimum_version = ContractVersion::new(minimum);
        let maximum_version = ContractVersion::new(maximum);
        let Ok(compatible) =
            CompatibleVersionRange::new(minimum_version, maximum_version)
        else {
            return Err(ContractSelectionError::InvertedRange {
                minimum,
                maximum,
            });
        };
        let selected_version = ContractVersion::new(selected);
        if !compatible.contains(selected_version) {
            return Err(ContractSelectionError::SelectedOutsideRange {
                selected,
                minimum,
                maximum,
            });
        }
        Ok(Self {
            identity: ContractId::new(identity),
            selected: selected_version,
            compatible,
        })
    }

    /// Returns the stable contract-family identity scalar.
    #[must_use]
    pub const fn identity(self) -> u128 {
        self.identity.value()
    }

    /// Returns the selected contract version scalar.
    #[must_use]
    pub const fn selected(self) -> u64 {
        self.selected.value()
    }

    /// Returns the inclusive compatible minimum version scalar.
    #[must_use]
    pub const fn minimum(self) -> u64 {
        self.compatible.minimum().value()
    }

    /// Returns the inclusive compatible maximum version scalar.
    #[must_use]
    pub const fn maximum(self) -> u64 {
        self.compatible.maximum().value()
    }

    /// Reports whether another version is compatible with this selection.
    #[must_use]
    pub const fn supports(self, version: u64) -> bool {
        self.compatible.contains(ContractVersion::new(version))
    }
}

const fn inline_may_wait(
    invocation: InvocationClass,
    completion: CompletionClass,
) -> bool {
    invocation_code(invocation) == 1 && completion_code(completion) == 0
}

const fn inline_streams(
    completion: CompletionClass,
    delivery: DeliveryClass,
) -> bool {
    completion_code(completion) == 0 && delivery_code(delivery) == 1
}

const fn inline_calls_user(
    completion: CompletionClass,
    callback: CallbackClass,
) -> bool {
    completion_code(completion) == 0 && callback_code(callback) == 1
}

const fn invocation_code(invocation: InvocationClass) -> u8 {
    match invocation {
        InvocationClass::NonBlockingInvocation => 0,
        InvocationClass::CallerMayWait => 1,
    }
}

const fn completion_code(completion: CompletionClass) -> u8 {
    match completion {
        CompletionClass::InlineTerminal => 0,
        CompletionClass::DeferredTerminal => 1,
    }
}

const fn delivery_code(delivery: DeliveryClass) -> u8 {
    match delivery {
        DeliveryClass::Single => 0,
        DeliveryClass::Stream => 1,
    }
}

const fn callback_code(callback: CallbackClass) -> u8 {
    match callback {
        CallbackClass::NoUserCallback => 0,
        CallbackClass::MayInvokeUserCallback => 1,
    }
}

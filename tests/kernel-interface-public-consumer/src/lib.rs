#![no_std]
#![forbid(unsafe_code)]
//! Executed external consumer of Kernel Interface contracts.

extern crate alloc;

mod buffer;
mod clock;
mod diagnostic;
mod effect;
mod event;
pub mod first_edit;
mod interaction;
mod lifecycle;
mod operation;
mod output_health;
mod output_health_state;
mod public_items;
mod resource;
mod semantic;

use core::{error::Error, fmt::Debug};
use reovim_kernel_interface::buffer::plan::BufferPlanDescriptor;
use reovim_kernel_interface::semantic::bounds::SemanticPolicyCallCharge;
use reovim_kernel_interface::semantic::descriptor::{
    DomainPolicyDescriptor, RepresentationPolicyDescriptor,
};
use reovim_kernel_interface::semantic::intent::SemanticIntent;

/// Public values for hosted Interaction payload-redaction tests.
pub struct InteractionDebugInputs {
    /// Selected Session descriptor.
    pub session:
        reovim_kernel_interface::interaction::descriptor::
            InteractionSessionDescriptor,
    /// Selected Representation attachment descriptor.
    pub attachment:
        reovim_kernel_interface::interaction::descriptor::
            RepresentationAttachmentDescriptor,
    /// Move-only outer Intent with a recognizable opaque payload.
    pub intent: reovim_kernel_interface::interaction::intent::InteractionIntent,
    /// Initial payload-bearing full Representation Update.
    pub initial:
        reovim_kernel_interface::interaction::update::RepresentationUpdate,
    /// Payload-bearing delta Representation Update.
    pub delta:
        reovim_kernel_interface::interaction::update::RepresentationUpdate,
}

/// Hosted-test inputs for checking policy unwind ownership at the public API.
pub struct SemanticPanicInputs {
    /// Explicit observed unit state for structural boundary fixtures only.
    pub state: reovim_kernel_interface::resource::state::ResourceWorkingState<
        'static,
        (),
    >,
    /// Selected Domain provider descriptor.
    pub domain: DomainPolicyDescriptor,
    /// Selected Representation provider descriptor.
    pub representation: RepresentationPolicyDescriptor,
    /// Exact Buffer-plan descriptor supplied to lowering.
    pub buffer: BufferPlanDescriptor,
    /// Runtime-retained immutable Intent owner.
    pub intent: SemanticIntent,
    /// Valid Domain callback reservation.
    pub domain_charge: SemanticPolicyCallCharge,
    /// Valid Representation callback reservation.
    pub representation_charge: SemanticPolicyCallCharge,
}

impl SemanticPanicInputs {
    /// Borrows the structural fixture's intent and actual observed unit value.
    #[must_use]
    pub const fn input(
        &self,
    ) -> reovim_kernel_interface::semantic::state::SemanticPolicyInput<'_, ()>
    {
        reovim_kernel_interface::semantic::state::SemanticPolicyInput {
            intent: &self.intent,
            state: &self.state,
        }
    }
}

const _: [(); 1] = [(); public_const_cases() as usize];

const fn public_const_cases() -> bool {
    buffer::const_cases()
        && operation::const_cases()
        && lifecycle::const_cases()
        && clock::const_cases()
        && diagnostic::const_cases()
        && effect::const_cases()
        && event::const_cases()
        && interaction::const_cases()
        && output_health::const_cases()
        && output_health_state::const_cases()
        && resource::const_cases()
        && semantic::const_cases()
}

/// Exercises the admitted Kernel Interface contracts through their public API.
#[must_use]
pub fn exercise_public_api() -> bool {
    public_items::witness()
        && buffer::exercise()
        && operation::exercise()
        && lifecycle::exercise()
        && clock::exercise()
        && diagnostic::exercise()
        && event::exercise()
        && interaction::exercise()
        && output_health::exercise()
        && output_health_state::exercise()
        && resource::exercise()
        && semantic::exercise()
}

/// Builds exact public values for the hosted caught-panic ownership tests.
#[must_use]
pub fn semantic_panic_inputs() -> Option<SemanticPanicInputs> {
    semantic::panic_inputs()
}

/// Reuses the selected public descriptors for native-state behavior tests.
#[must_use]
pub fn resource_state_fixture() -> Option<(
    reovim_kernel_interface::resource::access::ResourceAccessDescriptor,
    BufferPlanDescriptor,
)> {
    Some((resource::descriptor()?, buffer::descriptor()?))
}

/// Builds exact public values for hosted Interaction Debug tests.
#[must_use]
pub fn interaction_debug_inputs() -> Option<InteractionDebugInputs> {
    interaction::debug_inputs()
}

const fn derive_witness<T>() -> bool
where
    T: ::core::clone::Clone
        + ::core::marker::Copy
        + ::core::fmt::Debug
        + ::core::cmp::PartialEq
        + ::core::cmp::Eq,
{
    let _layout = (core::mem::size_of::<T>(), core::mem::align_of::<T>());
    true
}

const fn clone_copy_witness<T>() -> bool
where
    T: ::core::clone::Clone + ::core::marker::Copy,
{
    let _layout = (core::mem::size_of::<T>(), core::mem::align_of::<T>());
    true
}

const fn clone_debug_eq_witness<T>() -> bool
where
    T: ::core::clone::Clone
        + ::core::fmt::Debug
        + ::core::cmp::PartialEq
        + ::core::cmp::Eq,
{
    let _layout = (core::mem::size_of::<T>(), core::mem::align_of::<T>());
    true
}

const fn send_sync_witness<T>() -> bool
where
    T: ::core::marker::Send + ::core::marker::Sync,
{
    let _layout = (core::mem::size_of::<T>(), core::mem::align_of::<T>());
    true
}

fn traits<T>(value: T) -> bool
where
    T: Clone + Copy + Debug + Eq,
{
    let cloned = clone_value(&value);
    let (left, right) = copy_pair(cloned);
    require_debug(&left);
    left == right
}

fn clone_value<T: Clone>(value: &T) -> T {
    value.clone()
}

const fn copy_pair<T: Copy>(value: T) -> (T, T) {
    (value, value)
}

const fn require_debug<T: Debug>(_value: &T) {}

const fn error_traits<T>(_value: &T) -> bool
where
    T: Error,
{
    true
}

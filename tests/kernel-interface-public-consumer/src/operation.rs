use reovim_kernel_interface::operation::{
    CallbackClass, CompletionClass, ContractSelection, ContractSelectionError,
    DeliveryClass, EffectClass, InvocationClass, OperationDescriptor,
    OperationDescriptorError, ReentrancyClass,
};

use super::{error_traits, traits};

pub const fn const_cases() -> bool {
    selection_const_cases() && descriptor_const_cases() && derive_const_cases()
}

const fn selection_const_cases() -> bool {
    let Ok(selection) = ContractSelection::new(11, 2, 1, 3) else {
        return false;
    };
    let inverted = ContractSelection::new(11, 2, 3, 1);
    let outside = ContractSelection::new(11, 4, 1, 3);

    selection.identity() == 11
        && selection.selected() == 2
        && selection.minimum() == 1
        && selection.maximum() == 3
        && selection.supports(1)
        && selection.supports(3)
        && !selection.supports(4)
        && inverted_selection_code(inverted) == 1
        && outside_selection_code(outside) == 1
}

const fn inverted_selection_code(
    result: Result<ContractSelection, ContractSelectionError>,
) -> u8 {
    match result {
        Err(ContractSelectionError::InvertedRange {
            minimum: 3,
            maximum: 1,
        }) => 1,
        _ => 0,
    }
}

const fn outside_selection_code(
    result: Result<ContractSelection, ContractSelectionError>,
) -> u8 {
    match result {
        Err(ContractSelectionError::SelectedOutsideRange {
            selected: 4,
            minimum: 1,
            maximum: 3,
        }) => 1,
        _ => 0,
    }
}

const fn descriptor_const_cases() -> bool {
    let local = OperationDescriptor::local_inline();
    let external = OperationDescriptor::external_observation_inline();
    let deferred = OperationDescriptor::new(
        InvocationClass::CallerMayWait,
        CompletionClass::DeferredTerminal,
        DeliveryClass::Stream,
        EffectClass::ExternalMutation,
        CallbackClass::MayInvokeUserCallback,
        ReentrancyClass::Reentrant,
    );
    let Ok(deferred) = deferred else { return false };

    local_descriptor(local)
        && effect_code(external.effect()) == 1
        && deferred_descriptor(deferred)
        && invalid_descriptor_cases()
}

const fn local_descriptor(descriptor: OperationDescriptor) -> bool {
    invocation_code(descriptor.invocation()) == 0
        && completion_code(descriptor.completion()) == 0
        && delivery_code(descriptor.delivery()) == 0
        && effect_code(descriptor.effect()) == 0
        && callback_code(descriptor.callback()) == 0
        && reentrancy_code(descriptor.reentrancy()) == 0
}

const fn deferred_descriptor(descriptor: OperationDescriptor) -> bool {
    invocation_code(descriptor.invocation()) == 1
        && completion_code(descriptor.completion()) == 1
        && delivery_code(descriptor.delivery()) == 1
        && effect_code(descriptor.effect()) == 2
        && callback_code(descriptor.callback()) == 1
        && reentrancy_code(descriptor.reentrancy()) == 1
}

const fn invocation_code(value: InvocationClass) -> u8 {
    match value {
        InvocationClass::NonBlockingInvocation => 0,
        InvocationClass::CallerMayWait => 1,
    }
}

const fn completion_code(value: CompletionClass) -> u8 {
    match value {
        CompletionClass::InlineTerminal => 0,
        CompletionClass::DeferredTerminal => 1,
    }
}

const fn delivery_code(value: DeliveryClass) -> u8 {
    match value {
        DeliveryClass::Single => 0,
        DeliveryClass::Stream => 1,
    }
}

const fn effect_code(value: EffectClass) -> u8 {
    match value {
        EffectClass::LocalOnly => 0,
        EffectClass::ExternalObservation => 1,
        EffectClass::ExternalMutation => 2,
    }
}

const fn callback_code(value: CallbackClass) -> u8 {
    match value {
        CallbackClass::NoUserCallback => 0,
        CallbackClass::MayInvokeUserCallback => 1,
    }
}

const fn reentrancy_code(value: ReentrancyClass) -> u8 {
    match value {
        ReentrancyClass::NonReentrant => 0,
        ReentrancyClass::Reentrant => 1,
    }
}

const fn invalid_descriptor_cases() -> bool {
    inline_wait_error() && inline_stream_error() && inline_callback_error()
}

const fn inline_wait_error() -> bool {
    let result = OperationDescriptor::new(
        InvocationClass::CallerMayWait,
        CompletionClass::InlineTerminal,
        DeliveryClass::Single,
        EffectClass::LocalOnly,
        CallbackClass::NoUserCallback,
        ReentrancyClass::NonReentrant,
    );
    descriptor_error(result, OperationDescriptorError::InlineMayNotWait)
}

const fn inline_stream_error() -> bool {
    let result = OperationDescriptor::new(
        InvocationClass::NonBlockingInvocation,
        CompletionClass::InlineTerminal,
        DeliveryClass::Stream,
        EffectClass::LocalOnly,
        CallbackClass::NoUserCallback,
        ReentrancyClass::NonReentrant,
    );
    descriptor_error(result, OperationDescriptorError::StreamRequiresDeferred)
}

const fn inline_callback_error() -> bool {
    let result = OperationDescriptor::new(
        InvocationClass::NonBlockingInvocation,
        CompletionClass::InlineTerminal,
        DeliveryClass::Single,
        EffectClass::LocalOnly,
        CallbackClass::MayInvokeUserCallback,
        ReentrancyClass::NonReentrant,
    );
    descriptor_error(
        result,
        OperationDescriptorError::InlineMayNotInvokeCallback,
    )
}

const fn descriptor_error(
    result: Result<OperationDescriptor, OperationDescriptorError>,
    expected: OperationDescriptorError,
) -> bool {
    match result {
        Err(actual) => actual as u8 == expected as u8,
        Ok(_) => false,
    }
}

const fn derive_const_cases() -> bool {
    invocation_class_derive()
        && crate::derive_witness::<
            ::reovim_kernel_interface::operation::CompletionClass,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::operation::DeliveryClass,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::operation::EffectClass,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::operation::CallbackClass,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::operation::ReentrancyClass,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::operation::OperationDescriptorError,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::operation::OperationDescriptor,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::operation::ContractSelectionError,
        >()
        && crate::derive_witness::<
            ::reovim_kernel_interface::operation::ContractSelection,
        >()
}

const fn invocation_class_derive() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::operation::
            // The source proof requires the exact external public path.
            InvocationClass,
    >()
}

pub fn exercise() -> bool {
    invocation_variants()
        && completion_variants()
        && delivery_variants()
        && effect_variants()
        && callback_variants()
        && reentrancy_variants()
        && error_variants()
        && descriptor_values()
}

fn invocation_variants() -> bool {
    traits(InvocationClass::NonBlockingInvocation)
        && traits(InvocationClass::CallerMayWait)
}

fn completion_variants() -> bool {
    traits(CompletionClass::InlineTerminal)
        && traits(CompletionClass::DeferredTerminal)
}

fn delivery_variants() -> bool {
    traits(DeliveryClass::Single) && traits(DeliveryClass::Stream)
}

fn effect_variants() -> bool {
    traits(EffectClass::LocalOnly)
        && traits(EffectClass::ExternalObservation)
        && traits(EffectClass::ExternalMutation)
}

fn callback_variants() -> bool {
    traits(CallbackClass::NoUserCallback)
        && traits(CallbackClass::MayInvokeUserCallback)
}

fn reentrancy_variants() -> bool {
    traits(ReentrancyClass::NonReentrant) && traits(ReentrancyClass::Reentrant)
}

fn error_variants() -> bool {
    let descriptor = OperationDescriptorError::InlineMayNotWait;
    let range =
        ContractSelectionError::InvertedRange { minimum: 2, maximum: 1 };
    let outside = ContractSelectionError::SelectedOutsideRange {
        selected: 3,
        minimum: 1,
        maximum: 2,
    };

    traits(descriptor)
        && traits(OperationDescriptorError::StreamRequiresDeferred)
        && traits(OperationDescriptorError::InlineMayNotInvokeCallback)
        && traits(range)
        && traits(outside)
        && error_traits(&descriptor)
        && error_traits(&range)
}

fn descriptor_values() -> bool {
    let Ok(selection) = ContractSelection::new(21, 1, 1, 2) else {
        return false;
    };
    traits(selection)
        && traits(OperationDescriptor::local_inline())
        && traits(OperationDescriptor::local_deferred())
        && traits(OperationDescriptor::external_observation_inline())
        && traits(OperationDescriptor::external_observation_deferred())
        && traits(OperationDescriptor::external_observation_stream())
}

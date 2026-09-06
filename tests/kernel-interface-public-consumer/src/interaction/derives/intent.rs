pub const fn all() -> bool {
    copy_values() && delivery_values() && owners()
}

const fn copy_values() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::interaction::intent::
            InteractionIntentError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::outcome::
            InteractionOutcomeCause,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::outcome::
            InteractionAcceptedState,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::outcome::
            InteractionOutcomeStatus,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::outcome::
            InteractionOutcomeError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::outcome::InteractionOutcome,
    >()
}

const fn delivery_values() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionIntentAdmissionStatus,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionIntentAdmissionError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionCancellationRequestError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionCancellationRequest,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionCancellationResult,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionOutcomeRequest,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionOutcomeRequestError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionOutcomeUnavailable,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionOutcomeDeliveryStatus,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionOutcomeDelivery,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionReconcileRequestError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionReconcileRequest,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionReconciledEffect,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionReconcileUnavailable,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionReconcileStatus,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionReconcileResult,
    >()
}

const fn owners() -> bool {
    crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::intent::InteractionIntent,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::intent::
            InteractionIntentBindingOutcome,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::intent::
            InteractionIntentRejection,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionIntentNonAdmission,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionIntentNonAdmissionBindingOutcome,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionIntentNonAdmissionRejection,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::intent::
            InteractionIntentAdmission,
    >()
}

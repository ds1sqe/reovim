pub const fn all() -> bool {
    copy_values() && delivery_values() && owners()
}

const fn copy_values() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdateCarrierStamp,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdatePayloadClaim,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdateContract,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::update::RepresentationGapCause,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdateLineage,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdateStampError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdateIdentityStamp,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdateStateStamp,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdateError,
    >()
}

const fn delivery_values() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationUpdateRequestError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationUpdateRequest,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationUpdateDeliveryError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationUpdateUnavailable,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationUpdateDeliveryStatus,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationUpdateApplication,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationUpdateAcknowledgementError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationUpdateAcknowledgement,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationUpdateAcknowledgementResult,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationResynchronizationRequestError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationResynchronizationResultError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationResynchronizationFailure,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationResynchronizationReceipt,
    >()
}

const fn owners() -> bool {
    crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdateStamp,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdatePreparation,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdateBindingOutcome,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdateRejection,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::update::
            RepresentationUpdateCarrier,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::update::RepresentationUpdate,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationUpdateDelivery,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationResynchronizationRequest,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::update::
            RepresentationResynchronizationAdmission,
    >()
}

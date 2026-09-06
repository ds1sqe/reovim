pub const fn all() -> bool {
    identities()
        && stamps()
        && bounds()
        && descriptors()
        && delivery_values()
        && delivery_owners()
}

const fn identities() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::interaction::identity::ClientIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::identity::
            InteractionSessionIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::identity::
            RepresentationAttachmentIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::identity::
            InteractionRequestIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::identity::
            InteractionCancellationIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::identity::
            InteractionCorrelationIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::identity::
            InteractionDeadlineIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::identity::
            InteractionReconcileIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::identity::
            RepresentationUpdateStreamIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::identity::
            RepresentationUpdateIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::identity::
            RepresentationResynchronizationIdentity,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::identity::
            RepresentationUpdateCursor,
    >()
}

const fn stamps() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::interaction::session::
            InteractionClientStamp,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::session::
            InteractionSessionStamp,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::session::
            InteractionSessionHandle,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::session::
            InteractionResourceScope,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::session::
            RepresentationAttachmentHandle,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::session::
            RepresentationUpdateStreamStamp,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::session::
            RepresentationUpdateStreamHandle,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::session::InteractionDeadline,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::session::
            InteractionSemanticBinding,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::session::InteractionStampError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::session::
            InteractionRequestStamp,
    >()
}

const fn bounds() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::interaction::bounds::InteractionBoundField,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::bounds::InteractionBoundsError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::bounds::
            InteractionBoundViolation,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::bounds::InteractionIntentCharge,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::bounds::
            InteractionPendingTotals,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::interaction::bounds::
            InteractionRequestBounds,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::bounds::
            RepresentationUpdateShape,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::bounds::
            RepresentationUpdateTotals,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::bounds::
            RepresentationUpdateShapeBounds,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::bounds::
            RepresentationUpdateFlowBounds,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::bounds::
            RepresentationResynchronizationBounds,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::bounds::
            RepresentationResynchronizationCharge,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::bounds::
            RepresentationResynchronizationTotals,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::interaction::bounds::
            RepresentationUpdateBounds,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::interaction::bounds::
            InteractionSessionBounds,
    >()
}

const fn descriptors() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::interaction::descriptor::
            InteractionCapabilitySupport,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::descriptor::
            InteractionSessionCapabilities,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::descriptor::
            InteractionCapabilityError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::descriptor::InteractionFairness,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::descriptor::
            InteractionOutcomeRetention,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::descriptor::
            RepresentationUpdateRetention,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::descriptor::
            InteractionSessionOperations,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::descriptor::
            InteractionDescriptorError,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::interaction::descriptor::
            InteractionSessionDescriptor,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::descriptor::
            RepresentationAttachmentError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::descriptor::
            RepresentationAttachmentSelection,
    >() && crate::clone_debug_eq_witness::<
        ::reovim_kernel_interface::interaction::descriptor::
            RepresentationAttachmentDescriptor,
    >()
}

const fn delivery_values() -> bool {
    crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            InteractionNegotiationRequestError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            InteractionNegotiationRequest,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            InteractionNegotiationFailure,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            InteractionNegotiationStatus,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            InteractionNegotiationOutcome,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentRequestError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentFailure,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentReadyError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentStatus,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            InteractionCloseRequest,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            InteractionCloseError,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            InteractionCloseRetainedState,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            InteractionIncompleteClose,
    >() && crate::derive_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            InteractionCloseOutcome,
    >()
}

const fn delivery_owners() -> bool {
    crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentRequest,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentRequestBindingOutcome,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentRequestRejection,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentReady,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentReadyBindingOutcome,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentReadyRejection,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentResynchronizing,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentRejection,
    >() && crate::send_sync_witness::<
        ::reovim_kernel_interface::interaction::delivery::session::
            RepresentationAttachmentOutcome,
    >()
}

use alloc::boxed::Box;

use reovim_kernel_interface::resource::operation::{
    ResourceCancellationIdentity, ResourceCloseRequest,
    ResourceCorrelationIdentity, ResourceDeadline, ResourceObservationRequest,
    ResourceObservationTarget, ResourceOpenRequest, ResourceOperationKind,
    ResourceOperationStamp, ResourceRequestIdentity,
};

use super::MemoryResource;

impl MemoryResource {
    /// Builds an exact open request for this source; callers own request IDs.
    ///
    /// # Panics
    /// Only if the self-owned descriptor rejects its own adapter generation.
    #[must_use]
    pub fn open_request(&self, id: u128) -> ResourceOpenRequest {
        ResourceOpenRequest::new(
            &self.descriptor,
            self.operation(ResourceOperationKind::Open, id),
            self.snapshot.access().resource(),
        )
        .expect("source-owned open stamp")
    }

    /// Builds the selected native whole-resource observation request.
    ///
    /// # Panics
    /// Only if the fixed single-region selector violates the reference bounds.
    #[must_use]
    pub fn observation_request(&self, id: u128) -> ResourceObservationRequest {
        let prepared = self
            .descriptor
            .prepare_region_carrier(
                self.descriptor.carriers().regions().selected_stamp(),
                1,
                1,
            )
            .expect("one whole-resource selector");
        let regions = self
            .descriptor
            .bind_region_carrier(prepared, Box::new([0]))
            .expect("whole-resource selector encoding");
        ResourceObservationRequest::new(
            &self.descriptor,
            self.operation(ResourceOperationKind::Observation, id),
            ResourceObservationTarget::Snapshot(self.snapshot),
            regions,
        )
        .expect("source-owned observation stamp")
    }

    /// Builds the idempotent exact-generation shutdown request.
    ///
    /// # Panics
    /// Only if the self-owned descriptor rejects its own adapter generation.
    #[must_use]
    pub fn close_request(&self) -> ResourceCloseRequest {
        ResourceCloseRequest::new(
            &self.descriptor,
            self.operation(ResourceOperationKind::Close, 0),
        )
        .expect("source-owned close stamp")
    }

    const fn operation(
        &self,
        kind: ResourceOperationKind,
        id: u128,
    ) -> ResourceOperationStamp {
        ResourceOperationStamp::new(
            self.descriptor.adapter(),
            kind,
            ResourceRequestIdentity::new(id),
            ResourceCancellationIdentity::new(id),
            ResourceCorrelationIdentity::new(id),
            ResourceDeadline::None,
        )
    }
}

//! Host-side gRPC service descriptor passed to the driver.
//!
//! Each descriptor identifies one gRPC service the driver should
//! route. The host (`server/lib/server/`) builds a `Vec<ServiceDescriptor>`
//! from its 12 `<Service>ServiceImpl` instances; the driver's `serve`
//! iterates the vec to wire its tonic Router.
//!
//! This is the in-process Rust shape; the FFI image is
//! [`crate::abi::FfiServiceDescriptor`]. The cdylib loader
//! (`server/lib/subsys/driver-loader/`) converts between them.
//!
//! See SP02 Phase 1 §C: the proto service-name strings are stable
//! `reovim.v3.<X>Service` identifiers compiled into the driver-side
//! typed wrappers (one per known service); the descriptor's name is
//! informational/diagnostic.

use {std::convert::Infallible, tonic::body::BoxBody, tower::util::BoxCloneService};

/// gRPC service descriptor handed to a [`crate::GrpcServerDriver`]'s
/// `serve` call.
///
/// `inner` is the type-erased tower service; `name` is the proto
/// service identifier (`reovim.v3.BufferService`, etc.) for
/// diagnostics and FFI marshalling.
pub struct ServiceDescriptor {
    /// Proto service name: `reovim.v3.<X>Service`. Stable per the
    /// proto contract under `uapi/protocol/`.
    pub name: &'static str,
    /// Type-erased tower service. tonic-compatible
    /// (`Error = Infallible`) — errors are mapped to `tonic::Status`
    /// in the response body before reaching this layer.
    pub inner: BoxCloneService<http::Request<BoxBody>, http::Response<BoxBody>, Infallible>,
}

impl ServiceDescriptor {
    /// Build a descriptor from a name and a tonic-compatible tower
    /// service.
    #[must_use]
    pub const fn new(
        name: &'static str,
        inner: BoxCloneService<http::Request<BoxBody>, http::Response<BoxBody>, Infallible>,
    ) -> Self {
        Self { name, inner }
    }
}

impl std::fmt::Debug for ServiceDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceDescriptor")
            .field("name", &self.name)
            .field("inner", &"<BoxCloneService>")
            .finish()
    }
}

#[cfg(test)]
#[path = "service_descriptor_tests.rs"]
mod tests;

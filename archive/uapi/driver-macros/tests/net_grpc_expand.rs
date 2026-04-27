//! Expansion smoke test for `declare_net_grpc_driver!`.
//!
//! Verifies the macro expansion compiles cleanly with a minimal user
//! driver type and that the generated vtable + per-service proxy
//! wrappers carry the canonical proto names.

#![allow(unsafe_code)]

use {
    async_trait::async_trait,
    reovim_driver_macros::declare_net_grpc_driver,
    reovim_subsys_net::{
        GrpcServerDriver, NetError, ServiceDescriptor, TransportConfig,
        abi::{NetGrpcDriverProbe, ShutdownFd},
    },
    std::sync::atomic::AtomicU16,
    tokio::runtime::{Builder, Handle, Runtime},
};

pub struct HygieneNetGrpcDriver {
    rt: Runtime,
}

impl HygieneNetGrpcDriver {
    #[must_use]
    pub fn probe() -> NetGrpcDriverProbe {
        NetGrpcDriverProbe::new("net_grpc", "hygiene-net-grpc")
    }

    /// # Errors
    /// Propagates `tokio::runtime::Builder::build` failures.
    pub fn construct() -> Result<Self, NetError> {
        let rt = Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| NetError::Io(format!("runtime: {e}")))?;
        Ok(Self { rt })
    }

    pub fn runtime_handle(&self) -> Handle {
        self.rt.handle().clone()
    }
}

#[async_trait]
impl GrpcServerDriver for HygieneNetGrpcDriver {
    async fn serve(
        &self,
        _config: TransportConfig,
        _descriptors: Vec<ServiceDescriptor>,
        _shutdown_fd: ShutdownFd,
        _bind_ready_fd: ShutdownFd,
        _port_writeback: &'static AtomicU16,
    ) -> Result<(), NetError> {
        Ok(())
    }
}

declare_net_grpc_driver!(HygieneNetGrpcDriver);

#[test]
fn vtable_symbol_exported() {
    // SAFETY: REOVIM_NET_GRPC_DRIVER_VTABLE is a `pub static` emitted
    // by the macro at module scope; reading constants on it is safe.
    assert_eq!(
        REOVIM_NET_GRPC_DRIVER_VTABLE.abi_version,
        reovim_subsys_net::abi::REOVIM_NET_GRPC_DRIVER_ABI_VERSION
    );
    assert_eq!(REOVIM_NET_GRPC_DRIVER_VTABLE.api_version.major, 1);
    assert_eq!(REOVIM_NET_GRPC_DRIVER_VTABLE.api_version.minor, 0);
}

#[test]
fn proxy_wrappers_have_named_service_impls() {
    use tonic::server::NamedService;
    assert_eq!(<__ReovimBufferServiceProxy as NamedService>::NAME, "reovim.v3.BufferService");
    assert_eq!(<__ReovimEditorServiceProxy as NamedService>::NAME, "reovim.v3.EditorService");
    assert_eq!(
        <__ReovimClientDebugServiceProxy as NamedService>::NAME,
        "reovim.v3.ClientDebugService"
    );
    assert_eq!(<__ReovimCommandServiceProxy as NamedService>::NAME, "reovim.v3.CommandService");
}

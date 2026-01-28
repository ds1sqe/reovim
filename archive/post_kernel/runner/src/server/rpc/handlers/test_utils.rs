//! Shared test utilities for RPC handler tests.

use std::sync::Arc;

use {
    reovim_arch::sync::RwLock,
    reovim_driver_buffer::TestBufferManager,
    reovim_driver_vfs::{MockVfs, VfsDriver},
    reovim_kernel::api::v1::{
        EventBus, KernelContext, MarkBank, ModeId, ModuleId, MotionEngine, OptionRegistry,
        RegisterBank, ServiceRegistry, TextObjectEngine,
    },
    tokio::sync::watch,
};

use crate::{
    server::{client::Client, rpc::RpcContext, transport::TransportWriter},
    session::{ClientId, Session, SessionId},
};

/// Create a mock VFS for testing.
#[must_use]
pub fn test_vfs() -> Arc<dyn VfsDriver> {
    Arc::new(MockVfs::new())
}

/// Create a test session with default configuration.
///
/// Uses `KernelContext::default()` which has stub implementations.
/// For tests that need working buffers, use [`test_session_with_buffers`].
#[must_use]
pub fn test_session() -> Arc<Session> {
    Session::new(
        SessionId::new("test"),
        KernelContext::default(),
        ModeId::new(ModuleId::new("test"), "normal"),
        test_vfs(),
    )
}

/// Create a test session with working buffer management.
///
/// Unlike [`test_session`] which uses stubs, this creates a session
/// with a real `TestBufferManager` that can create and store buffers.
#[must_use]
pub fn test_session_with_buffers() -> Arc<Session> {
    Session::new(
        SessionId::new("test"),
        real_kernel_context(),
        ModeId::new(ModuleId::new("test"), "normal"),
        test_vfs(),
    )
}

/// Create a real `KernelContext` with working buffer management.
///
/// Epic #417 Part 2: Uses `TestBufferManager` from driver layer.
/// Runner has zero knowledge of module types.
fn real_kernel_context() -> KernelContext {
    KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(RwLock::new(RegisterBank::new())),
        Arc::new(RwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    )
}

/// Create a test client with a stdio writer (no-op for tests).
#[must_use]
pub fn test_client(session_id: SessionId, client_id: ClientId) -> Arc<Client> {
    Client::new(client_id, session_id, TransportWriter::from_stdio())
}

/// Create a complete [`RpcContext`] for testing.
///
/// This helper creates a session, client, and context with all required fields.
#[must_use]
pub fn test_ctx() -> RpcContext {
    let session = test_session();
    let client_id = ClientId::new(1);
    let client = test_client(session.id().clone(), client_id);
    RpcContext {
        session,
        client_id,
        client,
        shutdown_tx: watch::channel(false).0,
    }
}

/// Create a test context with a custom session.
#[must_use]
pub fn test_ctx_with_session(session: Arc<Session>) -> RpcContext {
    let client_id = ClientId::new(1);
    let client = test_client(session.id().clone(), client_id);
    RpcContext {
        session,
        client_id,
        client,
        shutdown_tx: watch::channel(false).0,
    }
}

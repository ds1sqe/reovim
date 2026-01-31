//! `ModuleService` gRPC implementation.
//!
//! Provides module management operations for v2 protocol clients.
//!
//! Note: Actual module loading/unloading is an application concern and belongs
//! in the runner crate. This service provides stubs that return unimplemented.
//! The runner can replace this with a full implementation.

// `Status` is tonic's standard error type - size is inherent to the library
#![allow(clippy::result_large_err)]

use {
    reovim_protocol::v2::{
        ListModulesRequest, ListModulesResponse, LoadModuleRequest, LoadModuleResponse,
        ReloadModuleRequest, ReloadModuleResponse, UnloadModuleRequest, UnloadModuleResponse,
        module_service_server::ModuleService,
    },
    tonic::{Request, Response, Status},
};

/// gRPC `ModuleService` implementation (stub).
///
/// Module loading is an application concern - the actual implementation
/// lives in the runner crate. This stub returns unimplemented for all
/// operations. The runner can provide its own `ModuleService` implementation
/// with full FFI module loading capabilities.
#[derive(Default)]
pub struct ModuleServiceImpl;

impl ModuleServiceImpl {
    /// Create a new `ModuleService` stub.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[tonic::async_trait]
impl ModuleService for ModuleServiceImpl {
    /// List loaded modules.
    ///
    /// Stub: Returns empty list. Full implementation in runner.
    async fn list(
        &self,
        _request: Request<ListModulesRequest>,
    ) -> Result<Response<ListModulesResponse>, Status> {
        // lib/server doesn't load modules - that's the runner's job
        // Return empty list as a safe default
        Ok(Response::new(ListModulesResponse { modules: vec![] }))
    }

    /// Load a module.
    ///
    /// Stub: Returns unimplemented. Full implementation in runner.
    async fn load(
        &self,
        _request: Request<LoadModuleRequest>,
    ) -> Result<Response<LoadModuleResponse>, Status> {
        Err(Status::unimplemented(
            "Module loading is not available in lib/server - use runner for full module support",
        ))
    }

    /// Unload a module.
    ///
    /// Stub: Returns unimplemented. Full implementation in runner.
    async fn unload(
        &self,
        _request: Request<UnloadModuleRequest>,
    ) -> Result<Response<UnloadModuleResponse>, Status> {
        Err(Status::unimplemented(
            "Module unloading is not available in lib/server - use runner for full module support",
        ))
    }

    /// Reload a module.
    ///
    /// Stub: Returns unimplemented. Full implementation in runner.
    async fn reload(
        &self,
        _request: Request<ReloadModuleRequest>,
    ) -> Result<Response<ReloadModuleResponse>, Status> {
        Err(Status::unimplemented(
            "Module reloading is not available in lib/server - use runner for full module support",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_returns_empty() {
        let service = ModuleServiceImpl::new();

        let request = Request::new(ListModulesRequest {});
        let response = service.list(request).await;

        assert!(response.is_ok());
        let resp = response.unwrap().into_inner();
        assert!(resp.modules.is_empty());
    }

    #[tokio::test]
    async fn test_load_unimplemented() {
        let service = ModuleServiceImpl::new();

        let request = Request::new(LoadModuleRequest {
            name: "test".to_string(),
            path: None,
        });
        let response = service.load(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
    }

    #[tokio::test]
    async fn test_unload_unimplemented() {
        let service = ModuleServiceImpl::new();

        let request = Request::new(UnloadModuleRequest {
            name: "test".to_string(),
        });
        let response = service.unload(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
    }

    #[tokio::test]
    async fn test_reload_unimplemented() {
        let service = ModuleServiceImpl::new();

        let request = Request::new(ReloadModuleRequest {
            name: "test".to_string(),
        });
        let response = service.reload(request).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::Unimplemented);
    }
}

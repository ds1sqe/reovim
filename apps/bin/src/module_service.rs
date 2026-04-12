//! Runner-owned gRPC `ModuleService` implementation.

#![allow(clippy::result_large_err)]

use std::{path::Path, sync::Arc};

use {
    reovim_driver_module_loader::registry::ModuleRegistry,
    reovim_kernel::api::v1::{ModuleContext, ModuleError, ModuleId, ModuleState},
    reovim_protocol::v2::{
        ListModulesRequest, ListModulesResponse, LoadModuleRequest, LoadModuleResponse, ModuleInfo,
        ReloadModuleRequest, ReloadModuleResponse, UnloadModuleRequest, UnloadModuleResponse,
        module_service_server::ModuleService,
    },
    tonic::{Request, Response, Status},
};

/// Full module-service implementation for the runner.
pub struct RunnerModuleService {
    registry: Arc<ModuleRegistry>,
    ctx: Arc<ModuleContext>,
}

impl RunnerModuleService {
    /// Create a new runner module service.
    #[must_use]
    pub const fn new(registry: Arc<ModuleRegistry>, ctx: Arc<ModuleContext>) -> Self {
        Self { registry, ctx }
    }

    fn list_modules(&self) -> ListModulesResponse {
        let mut modules = self.registry.list_modules();
        modules.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

        ListModulesResponse {
            modules: modules
                .into_iter()
                .map(|module| ModuleInfo {
                    id: module.id.as_str().to_string(),
                    name: module.name,
                    version: module.version.to_string(),
                    path: module
                        .path
                        .map(|path| path.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    loaded: matches!(module.state, ModuleState::Running),
                })
                .collect(),
        }
    }

    #[allow(unsafe_code)]
    #[allow(clippy::option_if_let_else)]
    fn load_module(&self, request: &LoadModuleRequest) -> LoadModuleResponse {
        let requested_path = request.path.as_deref();
        let result = match request.path.as_deref() {
            Some(path) => {
                // SAFETY: The runner only exposes this on trusted local `.so` paths.
                unsafe { self.registry.load_dynamic(Path::new(&path)) }
            }
            None => {
                // SAFETY: The runner only searches trusted local module paths.
                unsafe { self.registry.load_by_name(&request.name) }
            }
        };

        match result {
            Ok(id) => match self.registry.init_module(&id, self.ctx.as_ref()) {
                Ok(()) => LoadModuleResponse {
                    ok: true,
                    error: None,
                },
                Err(error) => load_init_failed(&error),
            },
            Err(error) => LoadModuleResponse {
                ok: false,
                error: Some(format_load_error(&request.name, requested_path, &error)),
            },
        }
    }

    fn unload_module(&self, request: &UnloadModuleRequest) -> UnloadModuleResponse {
        let id = ModuleId::from_string(request.name.clone());

        match self.registry.unload(&id) {
            Ok(()) => UnloadModuleResponse {
                ok: true,
                error: None,
            },
            Err(ModuleError::InUse { module, by }) => UnloadModuleResponse {
                ok: false,
                error: Some(format!("module '{module}' is in use by '{by}'")),
            },
            Err(ModuleError::NotLoaded(_)) => UnloadModuleResponse {
                ok: false,
                error: Some(format!("module '{}' is not loaded", request.name)),
            },
            Err(error) => unload_unexpected(&error),
        }
    }

    #[allow(unsafe_code)]
    fn reload_module(&self, request: &ReloadModuleRequest) -> ReloadModuleResponse {
        let id = ModuleId::from_string(request.name.clone());

        // SAFETY: The runner only hot-reloads modules built for the active workspace ABI.
        match unsafe { self.registry.reload_atomic(&id, self.ctx.as_ref()) } {
            Ok(()) => ReloadModuleResponse {
                ok: true,
                error: None,
            },
            Err(error) => format_reload_error(&request.name, &error),
        }
    }
}

// Catch-all error handlers for gRPC methods. Each requires a specific .so
// failure mode (init panic, exit error, dlopen corruption) that is untestable
// without purpose-built dynamic modules.

#[cfg_attr(coverage_nightly, coverage(off))]
fn load_init_failed(error: &ModuleError) -> LoadModuleResponse {
    LoadModuleResponse {
        ok: false,
        error: Some(format_init_error(error)),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn unload_unexpected(error: &ModuleError) -> UnloadModuleResponse {
    UnloadModuleResponse {
        ok: false,
        error: Some(error.to_string()),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn reload_unexpected(error: &ModuleError) -> ReloadModuleResponse {
    ReloadModuleResponse {
        ok: false,
        error: Some(error.to_string()),
    }
}

fn format_reload_error(module_name: &str, error: &ModuleError) -> ReloadModuleResponse {
    match error {
        ModuleError::InUse { module, by } => ReloadModuleResponse {
            ok: false,
            error: Some(format!("cannot reload '{module}': in use by '{by}'")),
        },
        ModuleError::LoadFailed(message)
            if message.contains("cannot reload static module or missing path") =>
        {
            ReloadModuleResponse {
                ok: false,
                error: Some(format!(
                    "module '{module_name}' is a static builtin and cannot be hot-reloaded"
                )),
            }
        }
        _ => reload_unexpected(error),
    }
}

/// Concrete tonic-facing wrapper stored on `Server`.
#[derive(Clone)]
pub struct RunnerGrpcModuleService {
    inner: Arc<RunnerModuleService>,
}

impl RunnerGrpcModuleService {
    /// Create a new gRPC module-service wrapper.
    #[must_use]
    pub fn new(registry: Arc<ModuleRegistry>, ctx: Arc<ModuleContext>) -> Self {
        Self {
            inner: Arc::new(RunnerModuleService::new(registry, ctx)),
        }
    }
}

#[tonic::async_trait]
impl ModuleService for RunnerGrpcModuleService {
    async fn list(
        &self,
        _request: Request<ListModulesRequest>,
    ) -> Result<Response<ListModulesResponse>, Status> {
        Ok(Response::new(self.inner.list_modules()))
    }

    async fn load(
        &self,
        request: Request<LoadModuleRequest>,
    ) -> Result<Response<LoadModuleResponse>, Status> {
        let request = request.into_inner();
        Ok(Response::new(self.inner.load_module(&request)))
    }

    async fn unload(
        &self,
        request: Request<UnloadModuleRequest>,
    ) -> Result<Response<UnloadModuleResponse>, Status> {
        let request = request.into_inner();
        Ok(Response::new(self.inner.unload_module(&request)))
    }

    async fn reload(
        &self,
        request: Request<ReloadModuleRequest>,
    ) -> Result<Response<ReloadModuleResponse>, Status> {
        let request = request.into_inner();
        Ok(Response::new(self.inner.reload_module(&request)))
    }
}

fn format_load_error(
    requested_name: &str,
    requested_path: Option<&str>,
    error: &ModuleError,
) -> String {
    match error {
        ModuleError::IncompatibleVersion { module, kernel } => format!(
            "API version mismatch: module requires {}.{}, kernel provides {}.{}",
            module.0, module.1, kernel.0, kernel.1
        ),
        ModuleError::LoadFailed(message) if message.contains("already loaded") => {
            format!("module '{requested_name}' is already loaded")
        }
        ModuleError::LoadFailed(message) => requested_path.map_or_else(
            || message.clone(),
            |path| format!("failed to load module from '{path}': {message}"),
        ),
        ModuleError::NotFound(name) => format!("module '{name}' not found in search paths"),
        ModuleError::NoEntryPoint(message) => {
            format!("module is missing required symbol: {message}")
        }
        ModuleError::InitFailed(message) => format!("module init failed: {message}"),
        other => other.to_string(),
    }
}

fn format_init_error(error: &ModuleError) -> String {
    match error {
        ModuleError::InitFailed(message) => format!("module init failed: {message}"),
        other => other.to_string(),
    }
}

#[cfg(test)]
#[path = "module_service_tests.rs"]
mod tests;

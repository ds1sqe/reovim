use std::{path::PathBuf, sync::Arc, time::Duration};

use {
    super::*,
    reovim_driver_module_loader::registry::ModuleRegistry,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    reovim_protocol::v2::{
        ListModulesRequest, LoadModuleRequest, ReloadModuleRequest, UnloadModuleRequest,
        module_service_client::ModuleServiceClient,
    },
    reovim_server::{Server, ServerConfig, SessionFactory, SessionState},
    tonic::Request,
};

struct StaticTestModule;

struct ProviderTestModule;

struct ConsumerTestModule;

impl Module for StaticTestModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("runner-static")
    }

    fn name(&self) -> &'static str {
        "Runner Static Module"
    }

    fn version(&self) -> Version {
        Version::new(1, 2, 3)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

impl Module for ProviderTestModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("provider")
    }

    fn name(&self) -> &'static str {
        "Provider Module"
    }

    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

impl Module for ConsumerTestModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("consumer")
    }

    fn name(&self) -> &'static str {
        "Consumer Module"
    }

    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }

    fn dependencies(&self) -> Vec<ModuleId> {
        vec![ModuleId::new("provider")]
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

fn single_session_factory() -> SessionFactory {
    let state = Arc::new(parking_lot::Mutex::new(Some(SessionState::default())));
    Box::new(move || state.lock().take().expect("session state already consumed"))
}

fn test_dynamic_module_path() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent()?.parent()?;
    let so_path = workspace_root
        .join("target")
        .join("debug")
        .join("libreovim_test_dynamic_module.so");

    so_path.exists().then_some(so_path)
}

macro_rules! require_so {
    () => {
        match test_dynamic_module_path() {
            Some(path) => path,
            None => {
                eprintln!(
                    "SKIP: test module .so not found. Run: cargo build -p reovim-test-dynamic-module"
                );
                return;
            }
        }
    };
}

async fn wait_for_module_client(
    port_rx: tokio::sync::oneshot::Receiver<u16>,
) -> ModuleServiceClient<tonic::transport::Channel> {
    let port = port_rx.await.expect("should receive port");
    let addr = format!("http://127.0.0.1:{port}");

    for _ in 0..20 {
        match ModuleServiceClient::connect(addr.clone()).await {
            Ok(client) => return client,
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }

    panic!("module-service client failed to connect");
}

#[tokio::test]
async fn list_on_empty_registry_returns_empty() {
    let registry = ModuleRegistry::new().into_arc();
    let service =
        RunnerGrpcModuleService::new(Arc::clone(&registry), Arc::new(ModuleContext::default()));

    let response = service
        .list(Request::new(ListModulesRequest {}))
        .await
        .expect("list should succeed")
        .into_inner();

    assert!(response.modules.is_empty());
}

#[tokio::test]
async fn load_with_nonexistent_path_returns_error_message() {
    let registry = ModuleRegistry::new().into_arc();
    let service =
        RunnerGrpcModuleService::new(Arc::clone(&registry), Arc::new(ModuleContext::default()));

    let response = service
        .load(Request::new(LoadModuleRequest {
            name: "missing".into(),
            path: Some("/definitely/missing/module.so".into()),
        }))
        .await
        .expect("load should return response")
        .into_inner();

    assert!(!response.ok);
    assert!(
        response
            .error
            .expect("error expected")
            .contains("/definitely/missing/module.so")
    );
}

#[tokio::test]
async fn list_marks_loaded_but_uninitialized_modules_as_not_loaded() {
    let registry = ModuleRegistry::new().into_arc();
    registry
        .register(StaticTestModule)
        .expect("static register should succeed");
    let service =
        RunnerGrpcModuleService::new(Arc::clone(&registry), Arc::new(ModuleContext::default()));

    let response = service
        .list(Request::new(ListModulesRequest {}))
        .await
        .expect("list should succeed")
        .into_inner();

    assert_eq!(response.modules.len(), 1);
    assert!(!response.modules[0].loaded);
}

#[tokio::test]
async fn unload_nonexistent_module_returns_not_loaded() {
    let registry = ModuleRegistry::new().into_arc();
    let service =
        RunnerGrpcModuleService::new(Arc::clone(&registry), Arc::new(ModuleContext::default()));

    let response = service
        .unload(Request::new(UnloadModuleRequest {
            name: "missing".into(),
        }))
        .await
        .expect("unload should return response")
        .into_inner();

    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("module 'missing' is not loaded"));
}

#[tokio::test]
async fn reload_static_module_returns_hot_reload_error() {
    let registry = ModuleRegistry::new().into_arc();
    registry
        .register(StaticTestModule)
        .expect("static register should succeed");
    registry
        .init_all(&ModuleContext::default())
        .expect("init_all should succeed");
    let service =
        RunnerGrpcModuleService::new(Arc::clone(&registry), Arc::new(ModuleContext::default()));

    let response = service
        .reload(Request::new(ReloadModuleRequest {
            name: "runner-static".into(),
        }))
        .await
        .expect("reload should return response")
        .into_inner();

    assert!(!response.ok);
    assert_eq!(
        response.error.as_deref(),
        Some("module 'runner-static' is a static builtin and cannot be hot-reloaded")
    );
}

#[tokio::test]
async fn unload_in_use_dependency_returns_blocker_name() {
    let registry = ModuleRegistry::new().into_arc();
    registry
        .register(ProviderTestModule)
        .expect("provider register should succeed");
    registry
        .register(ConsumerTestModule)
        .expect("consumer register should succeed");
    registry
        .init_all(&ModuleContext::default())
        .expect("init_all should succeed");
    let service =
        RunnerGrpcModuleService::new(Arc::clone(&registry), Arc::new(ModuleContext::default()));

    let response = service
        .unload(Request::new(UnloadModuleRequest {
            name: "provider".into(),
        }))
        .await
        .expect("unload should return response")
        .into_inner();

    assert!(!response.ok);
    assert_eq!(response.error.as_deref(), Some("module 'provider' is in use by 'consumer'"));
}

#[tokio::test]
async fn load_already_loaded_module_returns_specific_error() {
    let so_path = require_so!();

    let registry = ModuleRegistry::new().into_arc();
    let service =
        RunnerGrpcModuleService::new(Arc::clone(&registry), Arc::new(ModuleContext::default()));

    let first = service
        .load(Request::new(LoadModuleRequest {
            name: "test-dynamic".into(),
            path: Some(so_path.to_string_lossy().into_owned()),
        }))
        .await
        .expect("first load should return response")
        .into_inner();
    assert!(first.ok, "first load failed: {:?}", first.error);

    let second = service
        .load(Request::new(LoadModuleRequest {
            name: "test-dynamic".into(),
            path: Some(so_path.to_string_lossy().into_owned()),
        }))
        .await
        .expect("second load should return response")
        .into_inner();

    assert!(!second.ok);
    assert_eq!(second.error.as_deref(), Some("module 'test-dynamic' is already loaded"));
}

#[tokio::test]
async fn injected_server_uses_runner_module_service() {
    let registry = ModuleRegistry::new().into_arc();
    registry
        .register(StaticTestModule)
        .expect("static register should succeed");
    registry
        .init_all(&ModuleContext::default())
        .expect("init_all should succeed");

    let service =
        RunnerGrpcModuleService::new(Arc::clone(&registry), Arc::new(ModuleContext::default()));
    let server = Arc::new(
        Server::with_session_factory(ServerConfig::grpc(0), single_session_factory())
            .with_module_service(service),
    );

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let (port_tx, port_rx) = tokio::sync::oneshot::channel::<u16>();

    let server_task = {
        let server = Arc::clone(&server);
        tokio::spawn(async move {
            server
                .run_until(
                    async {
                        let _ = shutdown_rx.await;
                    },
                    Some(port_tx),
                )
                .await
        })
    };

    let mut client = wait_for_module_client(port_rx).await;
    let response = client
        .list(ListModulesRequest {})
        .await
        .expect("list RPC should succeed")
        .into_inner();

    assert_eq!(response.modules.len(), 1);
    assert_eq!(response.modules[0].name, "Runner Static Module");
    assert_eq!(response.modules[0].version, "1.2.3");
    assert!(response.modules[0].loaded);
    drop(client);

    shutdown_tx.send(()).expect("shutdown send should succeed");
    server_task
        .await
        .expect("server task should join")
        .expect("server should shut down cleanly");
}

#[tokio::test]
async fn real_dynamic_module_flow_loads_lists_reloads_and_unloads() {
    let so_path = require_so!();

    let registry = ModuleRegistry::new().into_arc();
    let service =
        RunnerGrpcModuleService::new(Arc::clone(&registry), Arc::new(ModuleContext::default()));
    let server = Arc::new(
        Server::with_session_factory(ServerConfig::grpc(0), single_session_factory())
            .with_module_service(service),
    );

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let (port_tx, port_rx) = tokio::sync::oneshot::channel::<u16>();

    let server_task = {
        let server = Arc::clone(&server);
        tokio::spawn(async move {
            server
                .run_until(
                    async {
                        let _ = shutdown_rx.await;
                    },
                    Some(port_tx),
                )
                .await
        })
    };

    let mut client = wait_for_module_client(port_rx).await;

    let load = client
        .load(LoadModuleRequest {
            name: "test-dynamic".into(),
            path: Some(so_path.to_string_lossy().into_owned()),
        })
        .await
        .expect("load RPC should succeed")
        .into_inner();
    assert!(load.ok, "load failed: {:?}", load.error);

    let list = client
        .list(ListModulesRequest {})
        .await
        .expect("list RPC should succeed")
        .into_inner();
    assert_eq!(list.modules.len(), 1);
    assert_eq!(list.modules[0].name, "Test Dynamic Module");
    assert!(list.modules[0].loaded);
    assert!(
        list.modules[0]
            .path
            .ends_with("libreovim_test_dynamic_module.so")
    );

    let reload = client
        .reload(ReloadModuleRequest {
            name: "test-dynamic".into(),
        })
        .await
        .expect("reload RPC should succeed")
        .into_inner();
    assert!(reload.ok, "reload failed: {:?}", reload.error);

    let unload = client
        .unload(UnloadModuleRequest {
            name: "test-dynamic".into(),
        })
        .await
        .expect("unload RPC should succeed")
        .into_inner();
    assert!(unload.ok, "unload failed: {:?}", unload.error);

    let list_after = client
        .list(ListModulesRequest {})
        .await
        .expect("list RPC should succeed")
        .into_inner();
    assert!(list_after.modules.is_empty());
    drop(client);

    shutdown_tx.send(()).expect("shutdown send should succeed");
    server_task
        .await
        .expect("server task should join")
        .expect("server should shut down cleanly");
}

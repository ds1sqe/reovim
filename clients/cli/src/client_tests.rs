use {
    super::*,
    reovim_protocol::v2::{
        ListModulesRequest, ListModulesResponse, LoadModuleRequest, LoadModuleResponse, ModuleInfo,
        ReloadModuleRequest, ReloadModuleResponse, UnloadModuleRequest, UnloadModuleResponse,
        module_service_server::{ModuleService, ModuleServiceServer},
    },
    tonic::{Request, Response, Status, transport::Server},
};

#[derive(Default)]
struct TestModuleService;

#[tonic::async_trait]
impl ModuleService for TestModuleService {
    async fn list(
        &self,
        _request: Request<ListModulesRequest>,
    ) -> Result<Response<ListModulesResponse>, Status> {
        Ok(Response::new(ListModulesResponse {
            modules: vec![ModuleInfo {
                id: "sample".to_string(),
                name: "Sample Module".to_string(),
                version: "1.0.0".to_string(),
                path: "/tmp/sample.so".to_string(),
                loaded: true,
            }],
        }))
    }

    async fn load(
        &self,
        _request: Request<LoadModuleRequest>,
    ) -> Result<Response<LoadModuleResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn unload(
        &self,
        _request: Request<UnloadModuleRequest>,
    ) -> Result<Response<UnloadModuleResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }

    async fn reload(
        &self,
        _request: Request<ReloadModuleRequest>,
    ) -> Result<Response<ReloadModuleResponse>, Status> {
        Err(Status::unimplemented("not used in test"))
    }
}

#[test]
fn test_error_display() {
    let err = GrpcClientError::ConnectionFailed("test".to_string());
    assert!(err.to_string().contains("Connection failed"));

    let status = tonic::Status::not_found("test");
    let err = GrpcClientError::GrpcError(status);
    assert!(err.to_string().contains("gRPC error"));

    let err = GrpcClientError::InvalidArgument("bad arg".to_string());
    assert!(err.to_string().contains("Invalid argument"));
    assert!(err.to_string().contains("bad arg"));

    let err = GrpcClientError::OperationFailed("broken registry".to_string());
    assert!(err.to_string().contains("Operation failed"));
    assert!(err.to_string().contains("broken registry"));

    let err = GrpcClientError::CaptureError("script failed".to_string());
    assert!(err.to_string().contains("Capture error"));
    assert!(err.to_string().contains("script failed"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_module_list_round_trips_response() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    drop(listener);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let server_task = tokio::spawn(async move {
        Server::builder()
            .add_service(ModuleServiceServer::new(TestModuleService))
            .serve_with_shutdown(addr, async {
                let _ = shutdown_rx.await;
            })
            .await
    });

    let mut client = loop {
        match GrpcClient::connect(&addr.to_string()).await {
            Ok(client) => break client,
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(25)).await,
        }
    };

    let response = client
        .module_list()
        .await
        .expect("module_list should succeed");
    assert_eq!(response.modules.len(), 1);
    assert_eq!(response.modules[0].id, "sample");
    assert_eq!(response.modules[0].name, "Sample Module");
    assert!(response.modules[0].loaded);
    drop(client);

    shutdown_tx.send(()).expect("shutdown should send");
    server_task
        .await
        .expect("server task should join")
        .expect("server should shut down cleanly");
}

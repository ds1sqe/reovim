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

use super::*;

// ── require_client_id tests ──────────────────────────────────────

#[test]
fn require_returns_token_client_id() {
    let ext = Some(ClientId::new(42));
    assert_eq!(require_client_id(ext).unwrap(), ClientId::new(42));
}

#[test]
fn require_rejects_missing_token() {
    let err = require_client_id(None).unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

// ── resolve_target_client_id tests ───────────────────────────────

#[test]
fn target_zero_returns_caller() {
    let ext = Some(ClientId::new(42));
    assert_eq!(resolve_target_client_id(ext, 0).unwrap(), ClientId::new(42));
}

#[test]
fn target_nonzero_returns_target() {
    let ext = Some(ClientId::new(42));
    assert_eq!(resolve_target_client_id(ext, 99).unwrap(), ClientId::new(99));
}

#[test]
fn target_rejects_missing_token() {
    let err = resolve_target_client_id(None, 7).unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

#[test]
fn target_zero_rejects_missing_token() {
    let err = resolve_target_client_id(None, 0).unwrap_err();
    assert_eq!(err.code(), tonic::Code::Unauthenticated);
}

// ── AuthInterceptor tests ────────────────────────────────────

#[test]
fn interceptor_call_with_valid_token() {
    use tonic::metadata::MetadataValue;

    let registry = Arc::new(TokenRegistry::new());
    let client_id = ClientId::new(42);
    let token = registry.register(client_id);

    let mut interceptor = AuthInterceptor::new(Arc::clone(&registry));

    let mut request = Request::new(());
    request
        .metadata_mut()
        .insert("x-reovim-token", MetadataValue::try_from(token.as_str()).unwrap());

    let result = interceptor.call(request);
    assert!(result.is_ok());

    let request = result.unwrap();
    let extracted_id = request.extensions().get::<ClientId>().copied();
    assert_eq!(extracted_id, Some(client_id));
}

#[test]
fn interceptor_call_without_token() {
    let registry = Arc::new(TokenRegistry::new());
    let mut interceptor = AuthInterceptor::new(registry);

    let request = Request::new(());
    let result = interceptor.call(request);

    assert!(result.is_ok());
    let request = result.unwrap();
    let extracted_id = request.extensions().get::<ClientId>().copied();
    assert_eq!(extracted_id, None);
}

#[test]
fn interceptor_call_with_invalid_token() {
    let registry = Arc::new(TokenRegistry::new());
    let mut interceptor = AuthInterceptor::new(registry);

    let mut request = Request::new(());
    request.metadata_mut().insert(
        "x-reovim-token",
        tonic::metadata::MetadataValue::try_from("invalid-token").unwrap(),
    );

    let result = interceptor.call(request);
    assert!(result.is_ok());

    let request = result.unwrap();
    let extracted_id = request.extensions().get::<ClientId>().copied();
    assert_eq!(extracted_id, None);
}

#[test]
fn interceptor_call_with_malformed_metadata() {
    let registry = Arc::new(TokenRegistry::new());
    let mut interceptor = AuthInterceptor::new(registry);

    let mut request = Request::new(());
    // Insert binary metadata that won't convert to str
    request.metadata_mut().insert_bin(
        "x-reovim-token-bin",
        tonic::metadata::MetadataValue::from_bytes(&[0xFF, 0xFE]),
    );

    let result = interceptor.call(request);
    assert!(result.is_ok());

    let request = result.unwrap();
    let extracted_id = request.extensions().get::<ClientId>().copied();
    assert_eq!(extracted_id, None);
}

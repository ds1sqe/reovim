use super::*;

#[test]
fn test_error_display() {
    let err = TuiGrpcError::ConnectionFailed("test".to_string());
    assert!(err.to_string().contains("Connection failed"));

    let status = tonic::Status::not_found("test");
    let err = TuiGrpcError::GrpcError(status);
    assert!(err.to_string().contains("gRPC error"));
}

#[test]
fn test_error_from_status() {
    let status = tonic::Status::internal("internal error");
    let err: TuiGrpcError = status.into();
    matches!(err, TuiGrpcError::GrpcError(_));
}

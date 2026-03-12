use super::*;

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

    let err = GrpcClientError::CaptureError("script failed".to_string());
    assert!(err.to_string().contains("Capture error"));
    assert!(err.to_string().contains("script failed"));
}

//! Shape and Debug tests for [`ServiceDescriptor`].

use {
    super::ServiceDescriptor,
    std::{convert::Infallible, task::Poll},
    tonic::body::BoxBody,
    tower::util::BoxCloneService,
};

/// Trivial tower service that always responds with empty body.
#[derive(Clone)]
struct NoopSvc;

impl tower::Service<http::Request<BoxBody>> for NoopSvc {
    type Response = http::Response<BoxBody>;
    type Error = Infallible;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Infallible>> + Send>,
    >;

    fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: http::Request<BoxBody>) -> Self::Future {
        Box::pin(async { Ok(http::Response::new(empty_body())) })
    }
}

fn empty_body() -> BoxBody {
    use http_body_util::Empty;
    let empty = Empty::<bytes::Bytes>::new();
    let mapped = http_body_util::BodyExt::map_err(empty, |never| match never {});
    BoxBody::new(mapped)
}

#[test]
fn descriptor_carries_name_and_service() {
    let svc = BoxCloneService::new(NoopSvc);
    let desc = ServiceDescriptor::new("reovim.v3.BufferService", svc);
    assert_eq!(desc.name, "reovim.v3.BufferService");
}

#[test]
fn descriptor_debug_redacts_inner() {
    let svc = BoxCloneService::new(NoopSvc);
    let desc = ServiceDescriptor::new("reovim.v3.EditorService", svc);
    let s = format!("{desc:?}");
    assert!(s.contains("reovim.v3.EditorService"));
    assert!(s.contains("BoxCloneService"));
}

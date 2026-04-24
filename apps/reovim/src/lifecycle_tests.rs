use super::*;

#[tokio::test]
async fn notify_server_wakes_subscriber() {
    let coord = ShutdownCoord::new();
    let mut rx = coord.subscribe();
    coord.notify_server().expect("at least one receiver alive");
    rx.recv().await.expect("signal should be delivered");
}

#[tokio::test]
async fn notify_server_errors_when_no_subscribers() {
    let coord = ShutdownCoord::new();
    let err = coord
        .notify_server()
        .expect_err("no subscribers means no observers");
    assert!(err.to_string().contains("all subscribers dropped"));
}

#[test]
fn default_is_equivalent_to_new() {
    let default = ShutdownCoord::default();
    let _rx = default.subscribe();
}

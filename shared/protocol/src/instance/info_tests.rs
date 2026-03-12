use super::*;

#[test]
fn test_instance_info_serialization() {
    let info = InstanceInfo::new(
        "test-instance".to_string(),
        12345,
        TransportInfo::tcp("127.0.0.1", 12521),
    );

    let json = serde_json::to_string(&info).unwrap();
    let deserialized: InstanceInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(info.name, deserialized.name);
    assert_eq!(info.pid, deserialized.pid);
    assert_eq!(info.transport, deserialized.transport);
}

#[test]
fn test_transport_info_tcp() {
    let transport = TransportInfo::tcp("localhost", 9000);
    assert_eq!(transport.display(), "localhost:9000");
}

#[test]
fn test_transport_info_local() {
    let transport = TransportInfo::local("/tmp/reovim.sock");
    assert_eq!(transport.display(), "/tmp/reovim.sock");
}

#[test]
fn test_transport_info_display_trait() {
    let tcp = TransportInfo::tcp("10.0.0.1", 8080);
    assert_eq!(format!("{tcp}"), "10.0.0.1:8080");

    let local = TransportInfo::local("/var/run/reovim.sock");
    assert_eq!(format!("{local}"), "/var/run/reovim.sock");
}

#[test]
fn test_instance_info_new_fields() {
    let info =
        InstanceInfo::new("field-test".to_string(), 42, TransportInfo::local("/tmp/test.sock"));
    assert_eq!(info.name, "field-test");
    assert_eq!(info.pid, 42);
    assert!(matches!(info.transport, TransportInfo::Local { .. }));
    // started_at should be a reasonable timestamp (> year 2020)
    assert!(info.started_at > 1_577_836_800);
    // cwd should be set in test environment
    assert!(info.cwd.is_some());
}

#[test]
fn test_instance_info_equality() {
    let info1 = InstanceInfo {
        name: "test".to_string(),
        pid: 1,
        transport: TransportInfo::tcp("127.0.0.1", 9000),
        started_at: 100,
        cwd: None,
    };
    let info2 = info1.clone();
    assert_eq!(info1, info2);
}

#[test]
fn test_transport_info_equality() {
    let tcp1 = TransportInfo::tcp("localhost", 9000);
    let tcp2 = TransportInfo::tcp("localhost", 9000);
    assert_eq!(tcp1, tcp2);

    let tcp3 = TransportInfo::tcp("localhost", 9001);
    assert_ne!(tcp1, tcp3);

    let local1 = TransportInfo::local("/a");
    let local2 = TransportInfo::local("/b");
    assert_ne!(local1, local2);
}

#[test]
fn test_transport_info_roundtrip_tcp() {
    let transport = TransportInfo::tcp("192.168.1.1", 12345);
    let json = serde_json::to_string(&transport).unwrap();
    let decoded: TransportInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(transport, decoded);
}

#[test]
fn test_transport_info_roundtrip_local() {
    let transport = TransportInfo::local("/tmp/test.sock");
    let json = serde_json::to_string(&transport).unwrap();
    let decoded: TransportInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(transport, decoded);
}

#[test]
fn test_instance_info_debug() {
    let info =
        InstanceInfo::new("debug-test".to_string(), 1, TransportInfo::tcp("localhost", 9000));
    let debug_str = format!("{info:?}");
    assert!(debug_str.contains("debug-test"));
    assert!(debug_str.contains("localhost"));
}

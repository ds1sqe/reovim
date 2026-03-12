use super::*;

#[test]
fn test_version_result_serialization() {
    let result = VersionResult {
        version: "0.9.0".to_string(),
        git_hash: Some("abc1234".to_string()),
        git_commit: Some("abc1234567890abcdef1234567890abcdef123456".to_string()),
        git_dirty: Some(false),
        build_date: Some("2025-01-14".to_string()),
        build_timestamp: Some("2025-01-14T10:30:00+0000".to_string()),
        rust_version: "1.92.0".to_string(),
        target: Some("x86_64-unknown-linux-gnu".to_string()),
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"version\":\"0.9.0\""));
    assert!(json.contains("\"git_hash\":\"abc1234\""));
    assert!(json.contains("\"git_commit\":"));
    assert!(json.contains("\"git_dirty\":false"));
    assert!(json.contains("\"rust_version\":\"1.92.0\""));
    assert!(json.contains("\"target\":\"x86_64-unknown-linux-gnu\""));
}

#[test]
fn test_version_result_without_optional() {
    let result = VersionResult {
        version: "0.9.0".to_string(),
        git_hash: None,
        git_commit: None,
        git_dirty: None,
        build_date: None,
        build_timestamp: None,
        rust_version: "1.92.0".to_string(),
        target: None,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("git_hash"));
    assert!(!json.contains("git_commit"));
    assert!(!json.contains("git_dirty"));
    assert!(!json.contains("build_date"));
    assert!(!json.contains("build_timestamp"));
    assert!(!json.contains("target"));
}

#[test]
fn test_uptime_result_serialization() {
    let result = UptimeResult {
        uptime_seconds: 3661.5,
        uptime_human: "1h 1m 1s".to_string(),
        start_time: "2025-01-14T10:00:00Z".to_string(),
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"uptime_seconds\":3661.5"));
    assert!(json.contains("\"uptime_human\":\"1h 1m 1s\""));
}

#[test]
fn test_kernel_state_result_serialization() {
    let result = KernelStateResult {
        buffer_count: 3,
        active_buffer: Some(1),
        buffer_ids: vec![1, 2, 3],
        event_handlers: 10,
        event_queue_len: 0,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"buffer_count\":3"));
    assert!(json.contains("\"active_buffer\":1"));
}

#[test]
fn test_yank_type_serialization() {
    assert_eq!(serde_json::to_string(&YankType::Characterwise).unwrap(), "\"characterwise\"");
    assert_eq!(serde_json::to_string(&YankType::Linewise).unwrap(), "\"linewise\"");
}

#[test]
fn test_register_entry_serialization() {
    let entry = RegisterEntry {
        name: "\"".to_string(),
        content: "hello".to_string(),
        content_length: 5,
        yank_type: YankType::Characterwise,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"name\":\"\\\"\""));
    assert!(json.contains("\"yank_type\":\"characterwise\""));
}

#[test]
fn test_mark_entry_serialization() {
    let entry = MarkEntry {
        name: "a".to_string(),
        position: Position::new(10, 5),
        buffer_id: None,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"name\":\"a\""));
    assert!(!json.contains("buffer_id"));
}

#[test]
fn test_log_tail_params_default() {
    let params = LogTailParams::default();
    assert_eq!(params.count, 50);
    assert!(params.level.is_none());
    assert!(params.target.is_none());
    assert!(params.grep.is_none());
}

#[test]
fn test_log_tail_params_with_filters() {
    let params = LogTailParams {
        count: 100,
        level: Some("warn".to_string()),
        target: Some("runner::server".to_string()),
        grep: Some("error".to_string()),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"count\":100"));
    assert!(json.contains("\"level\":\"warn\""));
    assert!(json.contains("\"target\":\"runner::server\""));
    assert!(json.contains("\"grep\":\"error\""));
}

#[test]
fn test_log_tail_params_partial_filters() {
    // Only level filter
    let params = LogTailParams {
        count: 50,
        level: Some("info".to_string()),
        target: None,
        grep: None,
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"level\":\"info\""));
    assert!(!json.contains("\"target\""));
    assert!(!json.contains("\"grep\""));
}

#[test]
fn test_log_tail_params_deserialization() {
    // With all filters
    let json = r#"{"count": 25, "level": "debug", "target": "mymod", "grep": "test"}"#;
    let params: LogTailParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.count, 25);
    assert_eq!(params.level.as_deref(), Some("debug"));
    assert_eq!(params.target.as_deref(), Some("mymod"));
    assert_eq!(params.grep.as_deref(), Some("test"));

    // With default count
    let json = r#"{"level": "warn"}"#;
    let params: LogTailParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.count, 50);
    assert_eq!(params.level.as_deref(), Some("warn"));
}

#[test]
fn test_handler_stats_serialization() {
    let stats = HandlerStats {
        method: "input/keys".to_string(),
        call_count: 100,
        total_micros: 5000,
        avg_micros: 50.0,
    };
    let json = serde_json::to_string(&stats).unwrap();
    assert!(json.contains("\"method\":\"input/keys\""));
    assert!(json.contains("\"avg_micros\":50.0"));
}

#[test]
fn test_log_level_params_serialization() {
    // Get current level (no level specified)
    let params = LogLevelParams::default();
    let json = serde_json::to_string(&params).unwrap();
    assert_eq!(json, "{}");

    // Set new level
    let params = LogLevelParams {
        level: Some("debug".to_string()),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"level\":\"debug\""));
}

// Phase 1 tests for #332 - subscription types

#[test]
fn test_log_subscribe_params_serialization() {
    // Default (no level filter)
    let params = LogSubscribeParams::default();
    let json = serde_json::to_string(&params).unwrap();
    assert_eq!(json, "{}");

    // With level filter
    let params = LogSubscribeParams {
        level: Some("warn".to_string()),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"level\":\"warn\""));
}

#[test]
fn test_log_subscribe_result_serialization() {
    let result = LogSubscribeResult {
        subscription_id: 42,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"subscription_id\":42"));
}

#[test]
fn test_log_unsubscribe_params_serialization() {
    let params = LogUnsubscribeParams {
        subscription_id: 123,
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"subscription_id\":123"));
}

#[test]
fn test_log_unsubscribe_result_serialization() {
    let result = LogUnsubscribeResult { success: true };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"success\":true"));

    let result = LogUnsubscribeResult { success: false };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"success\":false"));
}

// Additional coverage tests

#[test]
fn test_version_result_roundtrip() {
    let result = VersionResult {
        version: "0.9.0".to_string(),
        git_hash: Some("abc1234".to_string()),
        git_commit: None,
        git_dirty: Some(true),
        build_date: None,
        build_timestamp: Some("2025-01-14T10:30:00+0000".to_string()),
        rust_version: "1.92.0".to_string(),
        target: Some("x86_64-unknown-linux-gnu".to_string()),
    };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: VersionResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.version, "0.9.0");
    assert_eq!(decoded.git_hash, Some("abc1234".to_string()));
    assert!(decoded.git_commit.is_none());
    assert_eq!(decoded.git_dirty, Some(true));
}

#[test]
fn test_uptime_result_roundtrip() {
    let result = UptimeResult {
        uptime_seconds: 90061.5,
        uptime_human: "25h 1m 1s".to_string(),
        start_time: "2025-01-01T00:00:00Z".to_string(),
    };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: UptimeResult = serde_json::from_str(&json).unwrap();
    assert!((decoded.uptime_seconds - 90061.5).abs() < f64::EPSILON);
    assert_eq!(decoded.uptime_human, "25h 1m 1s");
}

#[test]
fn test_kernel_state_result_without_active_buffer() {
    let result = KernelStateResult {
        buffer_count: 0,
        active_buffer: None,
        buffer_ids: vec![],
        event_handlers: 0,
        event_queue_len: 0,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"buffer_count\":0"));
    assert!(!json.contains("\"active_buffer\":null"));
}

#[test]
fn test_yank_type_deserialization() {
    let yt: YankType = serde_json::from_str("\"characterwise\"").unwrap();
    assert_eq!(yt, YankType::Characterwise);
    let yt: YankType = serde_json::from_str("\"linewise\"").unwrap();
    assert_eq!(yt, YankType::Linewise);
}

#[test]
fn test_registers_result_serialization() {
    let result = RegistersResult {
        unnamed: RegisterEntry {
            name: "\"".to_string(),
            content: "hello world".to_string(),
            content_length: 11,
            yank_type: YankType::Linewise,
        },
        named: vec![RegisterEntry {
            name: "a".to_string(),
            content: "register a".to_string(),
            content_length: 10,
            yank_type: YankType::Characterwise,
        }],
    };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: RegistersResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.unnamed.content, "hello world");
    assert_eq!(decoded.named.len(), 1);
    assert_eq!(decoded.named[0].name, "a");
}

#[test]
fn test_marks_result_serialization() {
    let result = MarksResult {
        local: vec![MarkEntry {
            name: "a".to_string(),
            position: Position::new(5, 10),
            buffer_id: None,
        }],
        global: vec![MarkEntry {
            name: "A".to_string(),
            position: Position::new(100, 0),
            buffer_id: Some(1),
        }],
        special: vec![],
    };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: MarksResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.local.len(), 1);
    assert_eq!(decoded.global.len(), 1);
    assert_eq!(decoded.global[0].buffer_id, Some(1));
    assert!(decoded.special.is_empty());
}

#[test]
fn test_mark_entry_with_buffer_id() {
    let entry = MarkEntry {
        name: "A".to_string(),
        position: Position::new(0, 0),
        buffer_id: Some(42),
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"buffer_id\":42"));
}

#[test]
fn test_mode_stack_result_serialization() {
    let result = ModeStackResult {
        current: "Normal".to_string(),
        stack: vec!["Normal".to_string()],
        depth: 1,
    };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: ModeStackResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.current, "Normal");
    assert_eq!(decoded.depth, 1);
    assert_eq!(decoded.stack.len(), 1);
}

#[test]
fn test_metric_entry_serialization() {
    let entry = MetricEntry {
        name: "rpc_requests".to_string(),
        value: 12345,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"name\":\"rpc_requests\""));
    assert!(json.contains("\"value\":12345"));
}

#[test]
fn test_metrics_result_serialization() {
    let result = MetricsResult {
        uptime_seconds: 100.5,
        total_requests: 500,
        counters: vec![MetricEntry {
            name: "keys_processed".to_string(),
            value: 1000,
        }],
    };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: MetricsResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.total_requests, 500);
    assert_eq!(decoded.counters.len(), 1);
}

#[test]
fn test_handlers_result_serialization() {
    let result = HandlersResult {
        handlers: vec![
            HandlerStats {
                method: "input/keys".to_string(),
                call_count: 100,
                total_micros: 5000,
                avg_micros: 50.0,
            },
            HandlerStats {
                method: "state/cursor".to_string(),
                call_count: 200,
                total_micros: 2000,
                avg_micros: 10.0,
            },
        ],
    };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: HandlersResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.handlers.len(), 2);
    assert_eq!(decoded.handlers[0].method, "input/keys");
    assert_eq!(decoded.handlers[1].call_count, 200);
}

#[test]
fn test_log_level_result_serialization() {
    let result = LogLevelResult {
        level: "info".to_string(),
        previous: Some("debug".to_string()),
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"level\":\"info\""));
    assert!(json.contains("\"previous\":\"debug\""));
}

#[test]
fn test_log_level_result_without_previous() {
    let result = LogLevelResult {
        level: "warn".to_string(),
        previous: None,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("\"previous\""));
}

#[test]
fn test_log_entry_result_serialization() {
    let entry = LogEntryResult {
        timestamp: "2026-01-01T00:00:00Z".to_string(),
        level: "error".to_string(),
        target: "reovim::kernel".to_string(),
        message: "panic!".to_string(),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let decoded: LogEntryResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.level, "error");
    assert_eq!(decoded.message, "panic!");
}

#[test]
fn test_log_tail_result_serialization() {
    let result = LogTailResult {
        entries: vec![LogEntryResult {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            level: "info".to_string(),
            target: "test".to_string(),
            message: "test message".to_string(),
        }],
        overflow_count: 5,
    };
    let json = serde_json::to_string(&result).unwrap();
    let decoded: LogTailResult = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.entries.len(), 1);
    assert_eq!(decoded.overflow_count, 5);
}

#[test]
fn test_log_subscribe_params_roundtrip() {
    let params = LogSubscribeParams {
        level: Some("debug".to_string()),
    };
    let json = serde_json::to_string(&params).unwrap();
    let decoded: LogSubscribeParams = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.level.as_deref(), Some("debug"));
}

#[test]
fn test_log_level_params_default() {
    let params = LogLevelParams::default();
    assert!(params.level.is_none());
}

use {
    reovim_driver_lsp::DiagnosticSeverity,
    reovim_driver_session::{
        ExtensionMap,
        bridges::{ExtensionScope, ExtensionStateBridge},
    },
};

use {
    super::*,
    crate::items::{PanelItem, PanelMode, SortOrder},
};

fn make_extensions_with_state() -> ExtensionMap {
    let mut exts = ExtensionMap::new();
    exts.get_or_insert::<DiagnosticsState>();
    exts
}

fn make_item(severity: DiagnosticSeverity, file: &str, line: u32) -> PanelItem {
    PanelItem {
        file_path: file.to_owned(),
        line,
        col: 0,
        severity,
        message: "test error".to_owned(),
        source: None,
        buffer_id: None,
    }
}

#[test]
fn kind() {
    assert_eq!(DiagnosticsPanelBridge.kind(), "diagnostics-panel");
}

#[test]
fn scope_is_client() {
    assert_eq!(DiagnosticsPanelBridge.scope(), ExtensionScope::Client);
}

#[test]
fn snapshot_no_state() {
    let exts = ExtensionMap::new();
    assert!(DiagnosticsPanelBridge.snapshot(&exts).is_none());
}

#[test]
fn snapshot_inactive() {
    let exts = make_extensions_with_state();

    let json = DiagnosticsPanelBridge.snapshot(&exts).unwrap();
    assert_eq!(json["active"], false);
    assert!(json.get("items").is_none());
}

#[test]
fn snapshot_active_with_items() {
    let mut exts = ExtensionMap::new();
    {
        let state = exts.get_or_insert::<DiagnosticsState>();
        state.active = true;
        state.mode = PanelMode::Diagnostics;
        state.items = vec![
            PanelItem {
                file_path: "/src/main.rs".to_owned(),
                line: 5,
                col: 10,
                severity: DiagnosticSeverity::Error,
                message: "type error".to_owned(),
                source: Some("rustc".to_owned()),
                buffer_id: Some(1),
            },
            make_item(DiagnosticSeverity::Warning, "/src/lib.rs", 3),
        ];
    }

    let json = DiagnosticsPanelBridge.snapshot(&exts).unwrap();
    assert_eq!(json["active"], true);
    assert_eq!(json["mode"], "Diagnostics");
    assert_eq!(json["panelTitle"], "Diagnostics");
    assert_eq!(json["totalCount"], 2);
    assert_eq!(json["errorCount"], 1);
    assert_eq!(json["warningCount"], 1);
    assert_eq!(json["infoCount"], 0);
    assert_eq!(json["hintCount"], 0);
    assert_eq!(json["selected"], 0);
    assert_eq!(json["sortOrder"], "severity");

    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["file"], "/src/main.rs");
    assert_eq!(items[0]["line"], 5);
    assert_eq!(items[0]["col"], 10);
    assert_eq!(items[0]["severity"], "error");
    assert_eq!(items[0]["message"], "type error");
    assert_eq!(items[0]["source"], "rustc");
    assert_eq!(items[0]["bufferId"], 1);

    assert!(items[1].get("source").is_none());
    assert!(items[1].get("bufferId").is_none());
}

#[test]
fn snapshot_active_empty() {
    let mut exts = ExtensionMap::new();
    {
        let state = exts.get_or_insert::<DiagnosticsState>();
        state.active = true;
    }

    let json = DiagnosticsPanelBridge.snapshot(&exts).unwrap();
    assert_eq!(json["active"], true);
    assert_eq!(json["totalCount"], 0);
    let items = json["items"].as_array().unwrap();
    assert!(items.is_empty());
}

#[test]
fn snapshot_all_modes() {
    for mode in [
        PanelMode::Diagnostics,
        PanelMode::Quickfix,
        PanelMode::References,
        PanelMode::Todo,
    ] {
        let mut exts = ExtensionMap::new();
        {
            let state = exts.get_or_insert::<DiagnosticsState>();
            state.active = true;
            state.mode = mode;
        }
        let json = DiagnosticsPanelBridge.snapshot(&exts).unwrap();
        assert_eq!(json["panelTitle"], mode.title());
    }
}

#[test]
fn snapshot_sort_order_strings() {
    for (order, expected) in [
        (SortOrder::BySeverity, "severity"),
        (SortOrder::ByFile, "file"),
        (SortOrder::ByLine, "line"),
    ] {
        let mut exts = ExtensionMap::new();
        {
            let state = exts.get_or_insert::<DiagnosticsState>();
            state.active = true;
            state.sort_order = order;
        }
        let json = DiagnosticsPanelBridge.snapshot(&exts).unwrap();
        assert_eq!(json["sortOrder"], expected);
    }
}

#[test]
fn is_active_false_when_no_state() {
    let exts = ExtensionMap::new();
    assert!(!DiagnosticsPanelBridge.is_active(&exts));
}

#[test]
fn is_active_false_when_inactive() {
    let exts = make_extensions_with_state();
    assert!(!DiagnosticsPanelBridge.is_active(&exts));
}

#[test]
fn is_active_true() {
    let mut exts = ExtensionMap::new();
    {
        let state = exts.get_or_insert::<DiagnosticsState>();
        state.active = true;
    }
    assert!(DiagnosticsPanelBridge.is_active(&exts));
}

#[test]
fn severity_str_values() {
    assert_eq!(severity_str(DiagnosticSeverity::Error), "error");
    assert_eq!(severity_str(DiagnosticSeverity::Warning), "warning");
    assert_eq!(severity_str(DiagnosticSeverity::Information), "info");
    assert_eq!(severity_str(DiagnosticSeverity::Hint), "hint");
}

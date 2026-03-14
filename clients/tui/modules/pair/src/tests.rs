use reovim_client_driver::{Attributes, ClientModule};

use super::*;

// =========================================================================
// Construction and identity
// =========================================================================

#[test]
fn new_inactive() {
    let m = PairModule::new();
    assert!(!m.has_buffer_contrib());
    assert_eq!(m.id(), "pair");
    assert_eq!(m.kind(), "pair");
    assert_eq!(m.name(), "Bracket Pair");
}

#[test]
fn default_inactive() {
    let m = PairModule::default();
    assert!(!m.has_buffer_contrib());
}

#[test]
fn version() {
    let m = PairModule::new();
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

// =========================================================================
// on_notification
// =========================================================================

#[test]
fn notification_activates() {
    let mut m = PairModule::new();
    m.on_notification(
        r#"{"active":true,"brackets":[{"line":0,"col":5,"depth":0,"char":"(","unmatched":false}]}"#,
    );
    assert!(m.has_buffer_contrib());
    assert_eq!(m.brackets.len(), 1);
}

#[test]
fn notification_deactivates() {
    let mut m = PairModule::new();
    m.on_notification(
        r#"{"active":true,"brackets":[{"line":0,"col":0,"depth":0,"char":"(","unmatched":false}]}"#,
    );
    assert!(m.has_buffer_contrib());

    m.on_notification(r#"{"active":false}"#);
    assert!(!m.has_buffer_contrib());
}

#[test]
fn notification_invalid_json() {
    let mut m = PairModule::new();
    m.on_notification("not json{{{");
    assert!(!m.has_buffer_contrib());
}

#[test]
fn notification_with_matched_pair() {
    let mut m = PairModule::new();
    m.on_notification(
        r#"{"active":true,"brackets":[],"matched":{"open":{"line":0,"col":3},"close":{"line":5,"col":0}}}"#,
    );
    assert!(m.has_buffer_contrib());
    assert!(m.matched.is_some());
}

#[test]
fn notification_rainbow_default_true() {
    let mut m = PairModule::new();
    m.on_notification(r#"{"active":true,"brackets":[]}"#);
    assert!(m.rainbow_enabled);
}

#[test]
fn notification_rainbow_disabled() {
    let mut m = PairModule::new();
    m.on_notification(r#"{"active":true,"rainbow":false,"brackets":[]}"#);
    assert!(!m.rainbow_enabled);
}

#[test]
fn notification_matchpair_disabled() {
    let mut m = PairModule::new();
    m.on_notification(r#"{"active":true,"matchpair":false,"brackets":[]}"#);
    assert!(!m.matchpair_enabled);
}

// =========================================================================
// inline_decorations
// =========================================================================

#[test]
fn decorations_rainbow_brackets() {
    let mut m = PairModule::new();
    m.on_notification(
        r#"{"active":true,"brackets":[
            {"line":0,"col":5,"depth":0,"char":"(","unmatched":false},
            {"line":0,"col":10,"depth":1,"char":"(","unmatched":false},
            {"line":2,"col":3,"depth":0,"char":")","unmatched":false}
        ]}"#,
    );

    let line0_decs = m.inline_decorations(0);
    assert_eq!(line0_decs.len(), 2);
    assert_eq!(line0_decs[0].col_start, 5);
    assert_eq!(line0_decs[0].col_end, 6);
    // Depth 0 = first rainbow color (Gold)
    assert_eq!(
        line0_decs[0].style.fg,
        Some(Color::Rgb {
            r: 255,
            g: 215,
            b: 0
        })
    );
    // Depth 1 = second rainbow color (Orchid)
    assert_eq!(
        line0_decs[1].style.fg,
        Some(Color::Rgb {
            r: 218,
            g: 112,
            b: 214
        })
    );

    let line2_decs = m.inline_decorations(2);
    assert_eq!(line2_decs.len(), 1);
    assert_eq!(line2_decs[0].col_start, 3);
}

#[test]
fn decorations_unmatched_bracket() {
    let mut m = PairModule::new();
    m.on_notification(
        r#"{"active":true,"brackets":[{"line":0,"col":5,"depth":0,"char":"(","unmatched":true}]}"#,
    );

    let decs = m.inline_decorations(0);
    assert_eq!(decs.len(), 1);
    assert_eq!(decs[0].style.fg, Some(UNMATCHED_COLOR));
    assert!(decs[0].style.attributes.contains(Attributes::UNDERLINE));
}

#[test]
fn decorations_matched_pair() {
    let mut m = PairModule::new();
    m.on_notification(
        r#"{"active":true,"brackets":[],"matched":{"open":{"line":0,"col":3},"close":{"line":5,"col":0}}}"#,
    );

    let open_decs = m.inline_decorations(0);
    assert_eq!(open_decs.len(), 1);
    assert_eq!(open_decs[0].col_start, 3);
    assert_eq!(open_decs[0].col_end, 4);
    assert!(
        open_decs[0]
            .style
            .attributes
            .contains(Attributes::BOLD | Attributes::UNDERLINE)
    );

    let close_decs = m.inline_decorations(5);
    assert_eq!(close_decs.len(), 1);
    assert_eq!(close_decs[0].col_start, 0);
    assert_eq!(close_decs[0].col_end, 1);
}

#[test]
fn decorations_empty_for_other_line() {
    let mut m = PairModule::new();
    m.on_notification(
        r#"{"active":true,"brackets":[{"line":0,"col":5,"depth":0,"char":"(","unmatched":false}]}"#,
    );

    assert!(m.inline_decorations(1).is_empty());
    assert!(m.inline_decorations(99).is_empty());
}

#[test]
fn decorations_inactive() {
    let m = PairModule::new();
    assert!(m.inline_decorations(0).is_empty());
}

#[test]
fn decorations_rainbow_disabled() {
    let mut m = PairModule::new();
    m.on_notification(
        r#"{"active":true,"rainbow":false,"brackets":[{"line":0,"col":5,"depth":0,"char":"(","unmatched":false}]}"#,
    );

    // Rainbow disabled, so no bracket decorations
    assert!(m.inline_decorations(0).is_empty());
}

#[test]
fn decorations_matchpair_disabled() {
    let mut m = PairModule::new();
    m.on_notification(
        r#"{"active":true,"matchpair":false,"brackets":[],"matched":{"open":{"line":0,"col":3},"close":{"line":5,"col":0}}}"#,
    );

    // Matchpair disabled, so no match decorations
    assert!(m.inline_decorations(0).is_empty());
    assert!(m.inline_decorations(5).is_empty());
}

#[test]
fn decorations_rainbow_depth_cycling() {
    let mut m = PairModule::new();
    // Depth 6 should cycle back to first color (same as depth 0)
    m.on_notification(
        r#"{"active":true,"brackets":[{"line":0,"col":0,"depth":6,"char":"(","unmatched":false}]}"#,
    );

    let decs = m.inline_decorations(0);
    assert_eq!(decs.len(), 1);
    assert_eq!(
        decs[0].style.fg,
        Some(Color::Rgb {
            r: 255,
            g: 215,
            b: 0
        })
    ); // Gold, same as depth 0
}

#[test]
fn decorations_cleared_on_deactivation() {
    let mut m = PairModule::new();
    m.on_notification(
        r#"{"active":true,"brackets":[{"line":0,"col":5,"depth":0,"char":"(","unmatched":false}]}"#,
    );
    assert!(!m.inline_decorations(0).is_empty());

    m.on_notification(r#"{"active":false}"#);
    assert!(m.inline_decorations(0).is_empty());
}

// =========================================================================
// Lifecycle defaults
// =========================================================================

#[test]
fn tick_returns_false() {
    let mut m = PairModule::new();
    assert!(!m.tick());
}

#[test]
fn cursor_position_none() {
    let m = PairModule::new();
    assert!(m.cursor_position(80, 24).is_none());
}

#[test]
fn has_annotations_false() {
    let m = PairModule::new();
    assert!(!m.has_annotations());
}

#[test]
fn has_chrome_false() {
    let m = PairModule::new();
    assert!(!m.has_chrome());
}

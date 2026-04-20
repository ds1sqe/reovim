use reovim_driver_text_session::SessionExtension;

use super::*;

#[test]
fn default_options() {
    let opts = PairOptions::default();
    assert!(opts.rainbow);
    assert!(opts.autopair);
    assert!(opts.matchpair);
}

#[test]
fn options_debug_clone() {
    let opts = PairOptions::default();
    let cloned = opts.clone();
    assert_eq!(cloned.rainbow, opts.rainbow);
    let debug = format!("{opts:?}");
    assert!(debug.contains("PairOptions"));
}

#[test]
fn state_create_defaults() {
    let state = PairState::create();
    assert!(state.brackets.is_empty());
    assert!(state.matched.is_none());
    assert!(state.options.rainbow);
    assert!(state.options.autopair);
    assert!(state.options.matchpair);
}

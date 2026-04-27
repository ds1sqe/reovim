//! Tests for `key_sequence_to_input_sequence` and `InputSequenceBridgeError`.

use {
    reovim_subsys_input::{DefaultInputCodecRegistry, InputCodecRegistry},
    std::sync::Arc,
};

use crate::{
    InputSequenceBridgeError, KeySequence, input_sequence_bridge::key_sequence_to_input_sequence,
};

fn make_registry_with_tui_codecs() -> DefaultInputCodecRegistry {
    use reovim_codec_tui_input::codecs::{tui_key_codec, tui_mouse_codec, tui_scroll_codec};
    let reg = DefaultInputCodecRegistry::new();
    reg.register(tui_key_codec());
    reg.register(tui_mouse_codec());
    reg.register(tui_scroll_codec());
    reg
}

#[test]
fn bridge_simple_tokens_to_input_sequence() {
    let reg = make_registry_with_tui_codecs();
    let ks = KeySequence::parse("gg").unwrap();
    let seq = key_sequence_to_input_sequence(&ks, &reg).unwrap();
    assert_eq!(seq.len(), 2);
}

#[test]
fn bridge_special_token_esc() {
    let reg = make_registry_with_tui_codecs();
    let ks = KeySequence::parse("<Esc>").unwrap();
    let seq = key_sequence_to_input_sequence(&ks, &reg).unwrap();
    assert_eq!(seq.len(), 1);
}

#[test]
fn bridge_ctrl_w_sequence() {
    let reg = make_registry_with_tui_codecs();
    let ks = KeySequence::parse("<C-w>h").unwrap();
    let seq = key_sequence_to_input_sequence(&ks, &reg).unwrap();
    assert_eq!(seq.len(), 2);
}

#[test]
fn bridge_without_codec_returns_not_registered() {
    let reg = DefaultInputCodecRegistry::new(); // empty registry
    let ks = KeySequence::parse("g").unwrap();
    let err = key_sequence_to_input_sequence(&ks, &reg).unwrap_err();
    assert_eq!(err, InputSequenceBridgeError::TuiKeyCodecNotRegistered);
}

#[test]
fn bridge_error_display() {
    let not_registered = InputSequenceBridgeError::TuiKeyCodecNotRegistered;
    let s = format!("{not_registered}");
    assert!(s.contains("0x0001"));

    let unknown = InputSequenceBridgeError::UnknownToken("<Foo>".to_owned());
    let s = format!("{unknown}");
    assert!(s.contains("Foo"));
}

#[test]
fn bridge_arc_dyn_registry() {
    let reg: Arc<dyn InputCodecRegistry> = Arc::new(make_registry_with_tui_codecs());
    let ks = KeySequence::parse("j").unwrap();
    let seq = key_sequence_to_input_sequence(&ks, reg.as_ref()).unwrap();
    assert_eq!(seq.len(), 1);
}

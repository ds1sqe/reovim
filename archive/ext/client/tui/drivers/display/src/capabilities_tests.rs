use {super::*, std::sync::Mutex};

// Env var tests must be serialized since env vars are global state.
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_detect_returns_valid_capabilities() {
    let caps = DisplayCapabilities::detect();
    let _ = caps.color_mode;
    let _ = caps.supports_underline_color;
    let _ = caps.supports_extended_underlines;
    let _ = caps.supports_mouse;
    let _ = caps.supports_kitty_graphics;
    let _ = caps.supports_sixel;
}

#[test]
fn test_default_matches_detect() {
    let detected = DisplayCapabilities::detect();
    let default = DisplayCapabilities::default();
    assert_eq!(detected.color_mode, default.color_mode);
    assert_eq!(detected.supports_mouse, default.supports_mouse);
    assert_eq!(detected.supports_sixel, default.supports_sixel);
}

#[test]
fn test_mouse_always_supported() {
    let caps = DisplayCapabilities::detect();
    assert!(caps.supports_mouse);
}

#[test]
fn test_sixel_not_supported_by_default() {
    let caps = DisplayCapabilities::detect();
    assert!(!caps.supports_sixel);
}

#[test]
fn test_capabilities_is_clone() {
    let caps = DisplayCapabilities::detect();
    let cloned = caps.clone();
    assert_eq!(caps.color_mode, cloned.color_mode);
    assert_eq!(caps.supports_mouse, cloned.supports_mouse);
}

#[test]
fn test_capabilities_is_debug() {
    let caps = DisplayCapabilities::detect();
    let debug = format!("{caps:?}");
    assert!(debug.contains("DisplayCapabilities"));
}

// --- MC/DC coverage for env-var-dependent detection functions ---

/// Save and restore env vars around a closure.
///
/// # Safety rationale
/// `set_var`/`remove_var` are unsafe in Rust 2024 due to potential
/// data races with other threads reading env vars concurrently.
/// We serialize all env-var tests via `ENV_LOCK` and restore
/// original values after the closure, so this is safe in practice.
#[allow(unsafe_code)]
fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let saved: Vec<(&str, Option<String>)> = vars
        .iter()
        .map(|(k, _)| (*k, std::env::var(k).ok()))
        .collect();
    for &(k, v) in vars {
        match v {
            Some(val) => unsafe { std::env::set_var(k, val) },
            None => unsafe { std::env::remove_var(k) },
        }
    }
    f();
    for (k, v) in &saved {
        match v {
            Some(val) => unsafe { std::env::set_var(k, val) },
            None => unsafe { std::env::remove_var(k) },
        }
    }
}

#[test]
fn test_detect_kitty_with_kitty_term() {
    with_env(&[("TERM", Some("xterm-kitty"))], || {
        assert!(DisplayCapabilities::detect_kitty());
    });
}

#[test]
fn test_detect_kitty_without_kitty_term() {
    with_env(&[("TERM", Some("xterm-256color"))], || {
        assert!(!DisplayCapabilities::detect_kitty());
    });
}

#[test]
fn test_detect_kitty_no_term() {
    with_env(&[("TERM", None)], || {
        assert!(!DisplayCapabilities::detect_kitty());
    });
}

#[test]
fn test_detect_extended_underlines_kitty() {
    with_env(&[("TERM", Some("xterm-kitty")), ("VTE_VERSION", None)], || {
        assert!(DisplayCapabilities::detect_extended_underlines());
    });
}

#[test]
fn test_detect_extended_underlines_vte() {
    with_env(
        &[
            ("TERM", Some("xterm-256color")),
            ("VTE_VERSION", Some("6800")),
        ],
        || {
            assert!(DisplayCapabilities::detect_extended_underlines());
        },
    );
}

#[test]
fn test_detect_extended_underlines_neither() {
    with_env(&[("TERM", Some("dumb")), ("VTE_VERSION", None)], || {
        assert!(!DisplayCapabilities::detect_extended_underlines());
    });
}

#[test]
fn test_detect_underline_color_kitty() {
    with_env(
        &[
            ("TERM", Some("xterm-kitty")),
            ("TERM_PROGRAM", None),
            ("WT_SESSION", None),
        ],
        || {
            assert!(DisplayCapabilities::detect_underline_color());
        },
    );
}

#[test]
fn test_detect_underline_color_xterm256() {
    with_env(
        &[
            ("TERM", Some("xterm-256color")),
            ("TERM_PROGRAM", None),
            ("WT_SESSION", None),
        ],
        || {
            assert!(DisplayCapabilities::detect_underline_color());
        },
    );
}

#[test]
fn test_detect_underline_color_alacritty() {
    with_env(
        &[
            ("TERM", Some("alacritty")),
            ("TERM_PROGRAM", None),
            ("WT_SESSION", None),
        ],
        || {
            assert!(DisplayCapabilities::detect_underline_color());
        },
    );
}

#[test]
fn test_detect_underline_color_iterm() {
    with_env(
        &[
            ("TERM", Some("dumb")),
            ("TERM_PROGRAM", Some("iTerm.app")),
            ("WT_SESSION", None),
        ],
        || {
            assert!(DisplayCapabilities::detect_underline_color());
        },
    );
}

#[test]
fn test_detect_underline_color_windows_terminal() {
    with_env(
        &[
            ("TERM", Some("dumb")),
            ("TERM_PROGRAM", None),
            ("WT_SESSION", Some("abc-123")),
        ],
        || {
            assert!(DisplayCapabilities::detect_underline_color());
        },
    );
}

#[test]
fn test_detect_underline_color_none() {
    with_env(
        &[
            ("TERM", Some("dumb")),
            ("TERM_PROGRAM", None),
            ("WT_SESSION", None),
        ],
        || {
            assert!(!DisplayCapabilities::detect_underline_color());
        },
    );
}

#[test]
fn test_detect_underline_color_no_term_at_all() {
    with_env(&[("TERM", None), ("TERM_PROGRAM", None), ("WT_SESSION", None)], || {
        assert!(!DisplayCapabilities::detect_underline_color());
    });
}

#[test]
fn test_detect_underline_color_wrong_term_program() {
    with_env(
        &[
            ("TERM", Some("dumb")),
            ("TERM_PROGRAM", Some("Terminal.app")),
            ("WT_SESSION", None),
        ],
        || {
            assert!(!DisplayCapabilities::detect_underline_color());
        },
    );
}

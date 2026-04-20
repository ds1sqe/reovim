use super::*;

#[test]
fn test_indent_guide_new() {
    let guide = IndentGuide::new(5, 2, true);
    assert_eq!(guide.line, 5);
    assert_eq!(guide.level, 2);
    assert!(guide.active);
}

#[test]
fn test_indent_guide_eq() {
    let a = IndentGuide::new(1, 0, false);
    let b = IndentGuide::new(1, 0, false);
    assert_eq!(a, b);
}

#[test]
fn test_indent_guide_ne() {
    let a = IndentGuide::new(1, 0, false);
    let b = IndentGuide::new(1, 0, true);
    assert_ne!(a, b);
}

#[test]
fn test_indent_guide_clone() {
    let guide = IndentGuide::new(3, 1, true);
    let cloned = guide.clone();
    assert_eq!(guide, cloned);
}

#[test]
fn test_indent_guide_debug() {
    let guide = IndentGuide::new(0, 0, false);
    let debug = format!("{guide:?}");
    assert!(debug.contains("IndentGuide"));
}

#[test]
fn test_options_default() {
    let opts = IndentGuideOptions::default();
    assert!(!opts.enabled);
    assert_eq!(opts.guide_char, '\u{2502}');
    assert_eq!(opts.tab_size, 4);
}

#[test]
fn test_options_clone() {
    let opts = IndentGuideOptions::default();
    let cloned = opts.clone();
    assert_eq!(cloned.tab_size, opts.tab_size);
}

#[test]
fn test_options_debug() {
    let opts = IndentGuideOptions::default();
    let debug = format!("{opts:?}");
    assert!(debug.contains("IndentGuideOptions"));
}

#[test]
fn test_state_create() {
    let state = IndentGuideState::create();
    assert!(state.guides.is_empty());
    assert!(!state.options.enabled);
}

#[test]
fn test_compute_guides_disabled() {
    let mut state = IndentGuideState::create();
    state.compute_guides(&["    hello"], 0, 4, 0);
    assert!(state.guides.is_empty());
}

#[test]
fn test_compute_guides_enabled_no_indent() {
    let mut state = IndentGuideState::create();
    state.options.enabled = true;
    state.compute_guides(&["hello"], 0, 4, 0);
    assert!(state.guides.is_empty());
}

#[test]
fn test_compute_guides_one_level() {
    let mut state = IndentGuideState::create();
    state.options.enabled = true;
    state.compute_guides(&["    hello"], 0, 4, 0);
    assert_eq!(state.guides.len(), 1);
    assert_eq!(state.guides[0].line, 0);
    assert_eq!(state.guides[0].level, 0);
    assert!(state.guides[0].active);
}

#[test]
fn test_compute_guides_two_levels() {
    let mut state = IndentGuideState::create();
    state.options.enabled = true;
    state.compute_guides(&["        hello"], 0, 4, 1);
    assert_eq!(state.guides.len(), 2);
    assert_eq!(state.guides[0].level, 0);
    assert!(!state.guides[0].active);
    assert_eq!(state.guides[1].level, 1);
    assert!(state.guides[1].active);
}

#[test]
fn test_compute_guides_multiple_lines() {
    let mut state = IndentGuideState::create();
    state.options.enabled = true;
    state.compute_guides(&["    a", "        b", "c"], 10, 4, 0);
    // Line 10: 1 level (level 0 active)
    // Line 11: 2 levels (level 0 active, level 1 not)
    // Line 12: 0 levels
    assert_eq!(state.guides.len(), 3);
    assert_eq!(state.guides[0], IndentGuide::new(10, 0, true));
    assert_eq!(state.guides[1], IndentGuide::new(11, 0, true));
    assert_eq!(state.guides[2], IndentGuide::new(11, 1, false));
}

#[test]
fn test_compute_guides_tabs() {
    let mut state = IndentGuideState::create();
    state.options.enabled = true;
    state.compute_guides(&["\thello"], 0, 4, 0);
    assert_eq!(state.guides.len(), 1);
    assert_eq!(state.guides[0].level, 0);
}

#[test]
fn test_compute_guides_zero_tab_size() {
    let mut state = IndentGuideState::create();
    state.options.enabled = true;
    state.compute_guides(&["    hello"], 0, 0, 0);
    assert!(state.guides.is_empty());
}

#[test]
fn test_compute_guides_clears_previous() {
    let mut state = IndentGuideState::create();
    state.options.enabled = true;
    state.compute_guides(&["    a"], 0, 4, 0);
    assert_eq!(state.guides.len(), 1);
    state.compute_guides(&["b"], 0, 4, 0);
    assert!(state.guides.is_empty());
}

#[test]
fn test_count_leading_whitespace_spaces() {
    assert_eq!(count_leading_whitespace("    hello", 4), 4);
    assert_eq!(count_leading_whitespace("  hello", 4), 2);
    assert_eq!(count_leading_whitespace("hello", 4), 0);
}

#[test]
fn test_count_leading_whitespace_tabs() {
    assert_eq!(count_leading_whitespace("\thello", 4), 4);
    assert_eq!(count_leading_whitespace("\t\thello", 4), 8);
}

#[test]
fn test_count_leading_whitespace_mixed() {
    assert_eq!(count_leading_whitespace("  \thello", 4), 4);
    assert_eq!(count_leading_whitespace("\t  hello", 4), 6);
}

#[test]
fn test_count_leading_whitespace_empty() {
    assert_eq!(count_leading_whitespace("", 4), 0);
}

#[test]
fn test_count_leading_whitespace_all_spaces() {
    assert_eq!(count_leading_whitespace("        ", 4), 8);
}

#[test]
fn test_compute_guides_tab_size_2() {
    let mut state = IndentGuideState::create();
    state.options.enabled = true;
    state.compute_guides(&["    hello"], 0, 2, 0);
    // 4 spaces / 2 = 2 levels
    assert_eq!(state.guides.len(), 2);
    assert_eq!(state.guides[0].level, 0);
    assert_eq!(state.guides[1].level, 1);
}

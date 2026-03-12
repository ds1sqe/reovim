use {super::*, crate::annotation::AnnotationKind};

// ========================================================================
// VisibilityMode tests
// ========================================================================

#[test]
fn test_visibility_mode_auto() {
    let mode = VisibilityMode::Auto;
    assert!(mode.should_show(true));
    assert!(!mode.should_show(false));
}

#[test]
fn test_visibility_mode_always() {
    let mode = VisibilityMode::Always;
    assert!(mode.should_show(true));
    assert!(mode.should_show(false));
}

#[test]
fn test_visibility_mode_never() {
    let mode = VisibilityMode::Never;
    assert!(!mode.should_show(true));
    assert!(!mode.should_show(false));
}

#[test]
fn test_visibility_mode_default() {
    let mode = VisibilityMode::default();
    assert_eq!(mode, VisibilityMode::Auto);
}

// ========================================================================
// ColumnConfig tests
// ========================================================================

#[test]
fn test_column_config_new() {
    let col = ColumnConfig::new(KindPattern::exact("line_number"));
    assert_eq!(col.visibility, VisibilityMode::Auto);
    assert!(col.width.is_none());
}

#[test]
fn test_column_config_builder() {
    let col = ColumnConfig::new(KindPattern::exact("sign"))
        .visibility(VisibilityMode::Always)
        .width(2);

    assert_eq!(col.visibility, VisibilityMode::Always);
    assert_eq!(col.width, Some(2));
}

#[test]
fn test_column_config_matches() {
    let col = ColumnConfig::new(KindPattern::prefix("diagnostic"));

    assert!(col.matches(&AnnotationKind::new("diagnostic.error")));
    assert!(col.matches(&AnnotationKind::new("diagnostic.warning")));
    assert!(!col.matches(&AnnotationKind::new("line_number")));
}

// ========================================================================
// GutterConfig tests
// ========================================================================

#[test]
fn test_gutter_config_new() {
    let config = GutterConfig::new(vec![ColumnConfig::new(KindPattern::exact("line_number"))]);

    assert_eq!(config.column_count(), 1);
    assert!(config.show_separator);
}

#[test]
fn test_gutter_config_default_line_numbers() {
    let config = GutterConfig::default_line_numbers();

    assert_eq!(config.column_count(), 1);
    assert!(config.show_separator);

    // Should match line_number
    assert!(
        config
            .column_for_kind(&AnnotationKind::new("line_number"))
            .is_some()
    );
}

#[test]
fn test_gutter_config_full() {
    let config = GutterConfig::full();

    // Should have 5 columns: sign, git, diagnostic, fold, line_number
    assert_eq!(config.column_count(), 5);
    assert!(config.show_separator);
}

#[test]
fn test_gutter_config_none() {
    let config = GutterConfig::none();

    assert!(config.is_empty());
    assert!(!config.show_separator);
}

#[test]
fn test_gutter_config_with_separator() {
    let config = GutterConfig::default_line_numbers().with_separator(false);
    assert!(!config.show_separator);
}

#[test]
fn test_gutter_config_add_column() {
    let config = GutterConfig::default_line_numbers()
        .add_column(ColumnConfig::new(KindPattern::prefix("sign")));

    assert_eq!(config.column_count(), 2);
}

#[test]
fn test_gutter_config_column_for_kind() {
    let config = GutterConfig::new(vec![
        ColumnConfig::new(KindPattern::prefix("diagnostic")),
        ColumnConfig::new(KindPattern::exact("line_number")),
    ]);

    // diagnostic.error should match first column
    let (idx, _) = config
        .column_for_kind(&AnnotationKind::new("diagnostic.error"))
        .unwrap();
    assert_eq!(idx, 0);

    // line_number should match second column
    let (idx, _) = config
        .column_for_kind(&AnnotationKind::new("line_number"))
        .unwrap();
    assert_eq!(idx, 1);

    // unknown should not match
    assert!(
        config
            .column_for_kind(&AnnotationKind::new("unknown"))
            .is_none()
    );
}

#[test]
fn test_gutter_config_first_match_wins() {
    // If multiple columns could match, first one wins
    let config = GutterConfig::new(vec![
        ColumnConfig::new(KindPattern::exact("diagnostic.error")),
        ColumnConfig::new(KindPattern::prefix("diagnostic")),
    ]);

    // diagnostic.error matches first column (exact)
    let (idx, _) = config
        .column_for_kind(&AnnotationKind::new("diagnostic.error"))
        .unwrap();
    assert_eq!(idx, 0);

    // diagnostic.warning matches second column (prefix)
    let (idx, _) = config
        .column_for_kind(&AnnotationKind::new("diagnostic.warning"))
        .unwrap();
    assert_eq!(idx, 1);
}

#[test]
fn test_gutter_config_default() {
    let config = GutterConfig::default();
    assert!(config.is_empty());
}

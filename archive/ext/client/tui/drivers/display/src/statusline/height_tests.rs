use {super::*, crate::Style};

fn make_section(id: SectionId, text: &str) -> Section {
    Section::new(id, text, Style::default())
}

#[test]
fn test_content_metrics_from_sections() {
    let sections = vec![
        make_section(SectionId::A, " NORMAL "),      // 8
        make_section(SectionId::B, ""),              // 0
        make_section(SectionId::C, " main.rs [+] "), // 13
        make_section(SectionId::X, ""),              // 0
        make_section(SectionId::Y, " rust "),        // 6
        make_section(SectionId::Z, " 42:15 Top "),   // 11
    ];

    let metrics = ContentMetrics::from_sections(&sections);

    assert_eq!(metrics.left_width, 8 + 13); // A(8) + B(0) + C(13) = 21
    assert_eq!(metrics.right_width, 6 + 11); // X(0) + Y(6) + Z(11) = 17
    assert_eq!(metrics.total_width, 38);
    assert_eq!(metrics.section_widths[0], 8); // A
    assert_eq!(metrics.section_widths[2], 13); // C
}

#[test]
fn test_content_metrics_overflows() {
    let metrics = ContentMetrics {
        total_width: 100,
        left_width: 50,
        right_width: 50,
        section_widths: [10, 20, 20, 20, 20, 10],
    };

    assert!(metrics.overflows(80));
    assert!(metrics.overflows(99));
    assert!(!metrics.overflows(100));
    assert!(!metrics.overflows(120));
}

#[test]
fn test_content_metrics_gap_width() {
    let metrics = ContentMetrics {
        total_width: 60,
        left_width: 30,
        right_width: 30,
        section_widths: [10, 10, 10, 10, 10, 10],
    };

    assert_eq!(metrics.gap_width(100), 40); // 100 - 60 = 40
    assert_eq!(metrics.gap_width(60), 0); // exactly fits
    assert_eq!(metrics.gap_width(50), 0); // overflows
}

#[test]
fn test_content_metrics_rows_for_wrap() {
    let metrics = ContentMetrics {
        total_width: 160,
        ..Default::default()
    };

    assert_eq!(metrics.rows_for_wrap(80), 2); // 160 / 80 = 2
    assert_eq!(metrics.rows_for_wrap(100), 2); // 160 / 100 = 1.6 → 2
    assert_eq!(metrics.rows_for_wrap(160), 1); // exact fit
    assert_eq!(metrics.rows_for_wrap(200), 1); // fits with room
}

#[test]
fn test_content_metrics_rows_for_redistribute() {
    // All fits in one row
    let metrics = ContentMetrics {
        total_width: 60,
        left_width: 30,
        right_width: 30,
        section_widths: [10, 10, 10, 10, 10, 10],
    };
    assert_eq!(metrics.rows_for_redistribute(80), 1);

    // Needs 2 rows (left and right can fit separately)
    let metrics = ContentMetrics {
        total_width: 120,
        left_width: 60,
        right_width: 60,
        section_widths: [20, 20, 20, 20, 20, 20],
    };
    assert_eq!(metrics.rows_for_redistribute(80), 2);

    // Needs 3 rows (A+B on row1, C on row2, X+Y+Z on row3)
    let metrics = ContentMetrics {
        total_width: 180,
        left_width: 100, // A+B=40, C=60
        right_width: 80,
        section_widths: [20, 20, 60, 30, 30, 20],
    };
    assert_eq!(metrics.rows_for_redistribute(80), 3);
}

#[test]
fn test_calculate_height_from_sections_no_overflow() {
    let sections = vec![
        make_section(SectionId::A, " NORMAL "),
        make_section(SectionId::Z, " 1:1 "),
    ];
    let config = HeightConfig::default();

    let result = calculate_height_from_sections(30, &sections, 80, &config);

    assert_eq!(result.height, 1);
    assert!(!result.has_overflow);
}

#[test]
fn test_calculate_height_from_sections_with_overflow() {
    // Create sections that total more than 80 width
    let sections = vec![
        make_section(SectionId::A, " NORMAL "),
        make_section(SectionId::C, " very-long-filename-that-causes-overflow.rs [+] "),
        make_section(SectionId::Z, " 42:15 25% "),
    ];
    let config = HeightConfig::default().with_overflow_strategy(OverflowStrategy::Redistribute);

    let result = calculate_height_from_sections(50, &sections, 60, &config);

    assert!(result.has_overflow);
    assert!(result.height >= 1);
}

#[test]
fn test_height_config_default() {
    let config = HeightConfig::default();
    assert_eq!(config.min_height, 1);
    assert_eq!(config.max_height, Some(3));
    assert_eq!(config.overflow_strategy, OverflowStrategy::Truncate);
    assert_eq!(config.screen_thresholds.len(), 2);
}

#[test]
fn test_height_config_fixed() {
    let config = HeightConfig::fixed(2);
    assert_eq!(config.min_height, 2);
    assert_eq!(config.max_height, Some(2));
}

#[test]
fn test_height_config_single_row() {
    let config = HeightConfig::single_row();
    assert_eq!(config.min_height, 1);
    assert_eq!(config.max_height, Some(1));
}

#[test]
fn test_overflow_strategy_may_use_multiple_rows() {
    assert!(!OverflowStrategy::Truncate.may_use_multiple_rows());
    assert!(OverflowStrategy::Wrap.may_use_multiple_rows());
    assert!(OverflowStrategy::Redistribute.may_use_multiple_rows());
}

#[test]
fn test_calculate_height_no_overflow() {
    let config = HeightConfig::default();
    let result = calculate_height(30, 50, 80, &config);

    assert_eq!(result.height, 1);
    assert!(!result.has_overflow);
}

#[test]
fn test_calculate_height_with_overflow_truncate() {
    let config = HeightConfig::default(); // Truncate by default
    let result = calculate_height(30, 100, 80, &config);

    // Truncate doesn't add rows
    assert_eq!(result.height, 1);
    assert!(result.has_overflow);
    assert_eq!(result.strategy, OverflowStrategy::Truncate);
}

#[test]
fn test_calculate_height_with_overflow_wrap() {
    let config = HeightConfig::default().with_overflow_strategy(OverflowStrategy::Wrap);
    let result = calculate_height(50, 160, 80, &config);

    // 160 width needs 2 rows of 80
    assert_eq!(result.height, 2);
    assert!(result.has_overflow);
    assert_eq!(result.strategy, OverflowStrategy::Wrap);
}

#[test]
fn test_calculate_height_respects_max() {
    let config = HeightConfig::default()
        .with_max_height(2)
        .with_overflow_strategy(OverflowStrategy::Wrap);
    let result = calculate_height(50, 300, 80, &config);

    // Would need 4 rows but capped at 2
    assert_eq!(result.height, 2);
    assert!(result.has_overflow);
}

#[test]
fn test_calculate_height_screen_thresholds() {
    let config = HeightConfig::default().with_overflow_strategy(OverflowStrategy::Redistribute);

    // Small screen (30 rows) - no extra rows from thresholds
    let result = calculate_height(30, 200, 80, &config);
    assert_eq!(result.height, 1); // min_height, below first threshold

    // Medium screen (45 rows) - first threshold applies (+1 row allowed)
    let result = calculate_height(45, 200, 80, &config);
    assert_eq!(result.height, 2); // allowed up to 2, needs at least 3, capped

    // Large screen (70 rows) - both thresholds apply (+3 rows allowed)
    let result = calculate_height(70, 200, 80, &config);
    assert_eq!(result.height, 3); // allowed up to 4, needs 3, takes 3
}

#[test]
fn test_height_result_single_row() {
    let result = HeightResult::single_row();
    assert_eq!(result.height, 1);
    assert!(!result.has_overflow);
    assert_eq!(result.strategy, OverflowStrategy::Truncate);
}

#[test]
fn test_height_config_unlimited() {
    let config = HeightConfig::default().unlimited();
    assert!(config.max_height.is_none());
    assert_eq!(config.min_height, 1);
}

#[test]
fn test_rows_for_wrap_zero_width() {
    let metrics = ContentMetrics {
        total_width: 100,
        ..Default::default()
    };
    assert_eq!(metrics.rows_for_wrap(0), 1);
}

#[test]
fn test_rows_for_redistribute_zero_width() {
    let metrics = ContentMetrics {
        total_width: 100,
        left_width: 50,
        right_width: 50,
        section_widths: [10, 20, 20, 20, 20, 10],
    };
    assert_eq!(metrics.rows_for_redistribute(0), 1);
}

#[test]
fn test_rows_for_redistribute_3_rows_exact() {
    // A+B fits, C fits, X+Y+Z fits -> exactly 3 rows
    let metrics = ContentMetrics {
        total_width: 150,
        left_width: 90,
        right_width: 60,
        section_widths: [20, 20, 50, 20, 20, 20],
    };
    // width = 55: left(90) > 55, right(60) > 55 -> skip 2-row
    // A+B = 40 <= 55, C = 50 <= 55, X+Y+Z = 60 > 55 -> 3-row condition false
    // Falls through to rows_for_wrap(55) -> 150/55 = 3
    assert_eq!(metrics.rows_for_redistribute(55), 3);

    // Now test where the 3-row condition IS true (line 296-298)
    let metrics2 = ContentMetrics {
        total_width: 140,
        left_width: 80,
        right_width: 60,
        section_widths: [20, 20, 40, 15, 15, 30],
    };
    // width = 65: left(80) > 65, right(60) <= 65 -> skip 2-row (left doesn't fit)
    // A+B = 40 <= 65, C = 40 <= 65, X+Y+Z = 60 <= 65 -> 3-row condition TRUE
    assert_eq!(metrics2.rows_for_redistribute(65), 3);
}

#[test]
fn test_rows_for_redistribute_fallback_to_wrap() {
    // None of the groups fit individually -> fallback to wrap calculation
    let metrics = ContentMetrics {
        total_width: 300,
        left_width: 200,
        right_width: 100,
        section_widths: [80, 80, 40, 40, 40, 20],
    };
    // width = 30: left(200) > 30, right(100) > 30 -> skip 2-row
    // A+B = 160 > 30, C = 40 > 30, X+Y+Z = 100 > 30 -> 3-row false
    // Fallback to rows_for_wrap(30) -> 300/30 = 10
    assert_eq!(metrics.rows_for_redistribute(30), 10);
}

#[test]
fn test_calculate_height_with_metrics_wrap_strategy() {
    let config = HeightConfig {
        min_height: 1,
        max_height: Some(5),
        screen_thresholds: vec![ScreenThreshold {
            min_screen_height: 20,
            additional_rows: 4,
        }],
        overflow_strategy: OverflowStrategy::Wrap,
    };

    let metrics = ContentMetrics {
        total_width: 200,
        left_width: 100,
        right_width: 100,
        section_widths: [30, 30, 40, 30, 30, 40],
    };

    // Screen height 30 >= threshold 20, so allowed_height = 1 + 4 = 5
    // Overflow: 200 > 80 -> true
    // Wrap strategy: rows_for_wrap(80) = 200/80 = 3
    // Final: max(1, 3).min(5) = 3
    let result = calculate_height_with_metrics(30, &metrics, 80, &config);
    assert_eq!(result.height, 3);
    assert!(result.has_overflow);
    assert_eq!(result.strategy, OverflowStrategy::Wrap);
}

#[test]
fn test_calculate_height_with_metrics_redistribute_strategy() {
    let config = HeightConfig {
        min_height: 1,
        max_height: Some(5),
        screen_thresholds: vec![ScreenThreshold {
            min_screen_height: 20,
            additional_rows: 4,
        }],
        overflow_strategy: OverflowStrategy::Redistribute,
    };

    let metrics = ContentMetrics {
        total_width: 140,
        left_width: 80,
        right_width: 60,
        section_widths: [20, 20, 40, 15, 15, 30],
    };

    // Overflow: 140 > 65 -> true
    // Redistribute strategy: rows_for_redistribute(65)
    // left(80) > 65 -> skip 2-row
    // A+B=40 <= 65, C=40 <= 65, X+Y+Z=60 <= 65 -> 3 rows
    let result = calculate_height_with_metrics(30, &metrics, 65, &config);
    assert_eq!(result.height, 3);
    assert!(result.has_overflow);
    assert_eq!(result.strategy, OverflowStrategy::Redistribute);
}

#[test]
fn test_calculate_height_with_metrics_truncate_strategy() {
    // Covers line 423: OverflowStrategy::Truncate arm in calculate_height_with_metrics
    let config = HeightConfig {
        min_height: 1,
        max_height: Some(3),
        screen_thresholds: vec![ScreenThreshold {
            min_screen_height: 20,
            additional_rows: 2,
        }],
        overflow_strategy: OverflowStrategy::Truncate,
    };

    let metrics = ContentMetrics {
        total_width: 200,
        left_width: 100,
        right_width: 100,
        section_widths: [30, 30, 40, 30, 30, 40],
    };

    // Overflow: 200 > 80 -> true
    // Truncate strategy: returns config.min_height = 1
    let result = calculate_height_with_metrics(30, &metrics, 80, &config);
    assert_eq!(result.height, 1);
    assert!(result.has_overflow);
    assert_eq!(result.strategy, OverflowStrategy::Truncate);
}

#[test]
fn test_rows_for_redistribute_left_exceeds_right_fits() {
    // Line 286: left_fits && right_fits where left_fits is false (left exceeds width)
    let metrics = ContentMetrics {
        total_width: 100,
        left_width: 60,  // Exceeds width=50
        right_width: 40, // Fits in 50
        section_widths: [20, 20, 20, 15, 15, 10],
    };
    // total > width -> not 1 row; left_fits=false -> skip 2-row
    // A+B=40<=50, C=20<=50, X+Y+Z=40<=50 -> 3-row condition true
    assert_eq!(metrics.rows_for_redistribute(50), 3);
}

#[test]
fn test_rows_for_redistribute_both_exceed() {
    // Line 296: multi-term AND where xyz_width > width
    let metrics = ContentMetrics {
        total_width: 200,
        left_width: 80,
        right_width: 120,
        section_widths: [20, 20, 40, 40, 40, 40],
    };
    // left(80)>50, right(120)>50 -> skip 2-row
    // A+B=40<=50, C=40<=50, X+Y+Z=120>50 -> condition false
    // Falls through to rows_for_wrap
    let rows = metrics.rows_for_redistribute(50);
    assert!(rows >= 3);
}

#[test]
fn test_calculate_height_no_max() {
    // Line 333: max_height is None (no cap applied)
    let config = HeightConfig {
        min_height: 1,
        max_height: None,
        screen_thresholds: vec![],
        overflow_strategy: OverflowStrategy::Truncate,
    };

    let result = calculate_height(30, 100, 80, &config);
    assert_eq!(result.height, 1);
}

#[test]
fn test_height_config_with_overflow_strategy() {
    let config = HeightConfig::default().with_overflow_strategy(OverflowStrategy::Redistribute);
    assert_eq!(config.overflow_strategy, OverflowStrategy::Redistribute);
}

#[test]
fn test_calculate_height_with_metrics_unlimited_config() {
    let config = HeightConfig::default()
        .unlimited()
        .with_overflow_strategy(OverflowStrategy::Wrap);

    let metrics = ContentMetrics {
        total_width: 400,
        left_width: 200,
        right_width: 200,
        section_widths: [60, 60, 80, 60, 60, 80],
    };

    // No max_height cap, large screen triggers both thresholds
    // allowed_height = 1 + 1 + 2 = 4 (from default thresholds)
    // Wrap: 400/80 = 5, but capped at allowed_height = 4
    let result = calculate_height_with_metrics(70, &metrics, 80, &config);
    assert_eq!(result.height, 4);
    assert!(result.has_overflow);
}

#[test]
fn test_rows_for_redistribute_right_exceeds_left_fits() {
    // Line 286: left_fits (true) && right_fits (false)
    let metrics = ContentMetrics {
        total_width: 100,
        left_width: 40,  // Fits in 50
        right_width: 60, // Exceeds width=50
        section_widths: [10, 10, 20, 20, 20, 20],
    };
    // total > width -> not 1 row
    // left_fits=true, right_fits=false -> skip 2-row
    // A+B=20<=50, C=20<=50, X+Y+Z=60>50 -> 3-row condition false
    // Falls through to rows_for_wrap
    let rows = metrics.rows_for_redistribute(50);
    assert!(rows >= 2);
}

#[test]
fn test_rows_for_redistribute_ab_exceeds_but_c_xyz_fit() {
    // Line 296: ab_width > width (first term false), c and xyz fit
    let metrics = ContentMetrics {
        total_width: 120,
        left_width: 80,
        right_width: 40,
        section_widths: [30, 30, 20, 15, 15, 10],
    };
    // total(120) > 50 -> not 1 row
    // left(80) > 50 -> skip 2-row
    // A+B=60 > 50 (false), so the 3-term AND is false at first term
    // Falls through to rows_for_wrap
    let rows = metrics.rows_for_redistribute(50);
    assert!(rows >= 2);
}

#[test]
fn test_rows_for_redistribute_ab_fits_c_exceeds() {
    // Line 296 MC/DC: ab_width <= width (true), c_width > width (false)
    // Middle sub-condition independently flips the 3-term AND
    let metrics = ContentMetrics {
        total_width: 150,
        left_width: 90,
        right_width: 60,
        section_widths: [10, 10, 70, 20, 20, 20],
    };
    // total(150) > 50 -> not 1 row
    // left(90) > 50 -> skip 2-row
    // A+B=20<=50 (true), C=70>50 (false) -> 3-row AND fails at second term
    let rows = metrics.rows_for_redistribute(50);
    assert!(rows >= 2);
}

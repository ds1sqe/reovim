use super::*;

#[test]
fn test_single_row_layout() {
    let layout = MultiRowLayout::single_row();
    assert_eq!(layout.row_count(), 1);
    assert_eq!(layout.sections_for_row(0), SectionId::ALL);
}

#[test]
fn test_two_row_layout() {
    let layout = MultiRowLayout::two_rows();
    assert_eq!(layout.row_count(), 2);
    assert_eq!(layout.sections_for_row(0), SectionId::LEFT);
    assert_eq!(layout.sections_for_row(1), SectionId::RIGHT);
}

#[test]
fn test_three_row_layout() {
    let layout = MultiRowLayout::three_rows();
    assert_eq!(layout.row_count(), 3);
    assert_eq!(layout.sections_for_row(0), &[SectionId::A, SectionId::B]);
    assert_eq!(layout.sections_for_row(1), &[SectionId::C]);
    assert_eq!(layout.sections_for_row(2), &[SectionId::X, SectionId::Y, SectionId::Z]);
}

#[test]
fn test_row_for_section() {
    let layout = MultiRowLayout::three_rows();
    assert_eq!(layout.row_for_section(SectionId::A), Some(0));
    assert_eq!(layout.row_for_section(SectionId::B), Some(0));
    assert_eq!(layout.row_for_section(SectionId::C), Some(1));
    assert_eq!(layout.row_for_section(SectionId::X), Some(2));
    assert_eq!(layout.row_for_section(SectionId::Z), Some(2));
}

#[test]
fn test_layout_contains() {
    let layout = MultiRowLayout::two_rows();
    assert!(layout.contains(SectionId::A));
    assert!(layout.contains(SectionId::Z));
}

#[test]
fn test_all_sections() {
    let layout = MultiRowLayout::single_row();
    assert_eq!(layout.all_sections(), SectionId::ALL.to_vec());
}

#[test]
fn test_layout_calculator_single_row() {
    let calc = LayoutCalculator::new(100);
    // All sections fit in 100 width
    let widths = [10, 10, 20, 10, 15, 15]; // total = 80
    let layout = calc.calculate(&widths, 3);
    assert_eq!(layout.row_count(), 1);
}

#[test]
fn test_layout_calculator_two_rows() {
    let calc = LayoutCalculator::new(60);
    // Total = 90, doesn't fit in 60, but left (40) and right (50) fit separately
    let widths = [10, 10, 20, 15, 15, 20];
    let layout = calc.calculate(&widths, 3);
    assert_eq!(layout.row_count(), 2);
}

#[test]
fn test_layout_calculator_three_rows() {
    let calc = LayoutCalculator::new(50);
    // Left=60, right=55, neither fits in 50
    // But A+B=20, C=40, X+Y+Z=55 - C doesn't fit perfectly but 3-row is best
    let widths = [10, 10, 40, 20, 20, 15];
    let layout = calc.calculate(&widths, 3);
    assert_eq!(layout.row_count(), 3);
}

#[test]
fn test_layout_calculator_respects_max() {
    let calc = LayoutCalculator::new(30);
    // Would need 3 rows but max is 2
    let widths = [10, 10, 40, 20, 20, 15];
    let layout = calc.calculate(&widths, 2);
    assert_eq!(layout.row_count(), 2);
}

#[test]
fn test_row_content_from_layout() {
    let layout = MultiRowLayout::two_rows();
    let rows = RowContent::from_layout(&layout);

    assert_eq!(rows.len(), 2);
    assert!(rows[0].is_left_only);
    assert!(!rows[0].is_right_only);
    assert!(!rows[1].is_left_only);
    assert!(rows[1].is_right_only);
}

#[test]
fn test_multi_row_layout_default() {
    let layout = MultiRowLayout::default();
    assert_eq!(layout.row_count(), 1);
    assert_eq!(layout.sections_for_row(0), SectionId::ALL);
}

#[test]
fn test_custom_layout() {
    let layout = MultiRowLayout::custom(vec![
        vec![SectionId::A],
        vec![SectionId::B, SectionId::C],
        vec![SectionId::X, SectionId::Y, SectionId::Z],
    ]);
    assert_eq!(layout.row_count(), 3);
    assert_eq!(layout.sections_for_row(0), &[SectionId::A]);
    assert_eq!(layout.sections_for_row(1), &[SectionId::B, SectionId::C]);
    assert_eq!(layout.sections_for_row(2), &[SectionId::X, SectionId::Y, SectionId::Z]);
}

#[test]
fn test_layout_calculator_two_row_via_max_rows() {
    // Line 169: condition false but max_rows == 2 forces 2-row layout
    let calc = LayoutCalculator::new(20);
    // total=120>20, left=60>20, right=60>20 -> condition false
    // But max_rows == 2 -> returns two_rows()
    let widths = [20, 20, 20, 20, 20, 20];
    let layout = calc.calculate(&widths, 2);
    assert_eq!(layout.row_count(), 2);
}

#[test]
fn test_layout_calculator_three_row_via_max_rows() {
    // Line 178: condition false but max_rows == 3 forces 3-row layout
    let calc = LayoutCalculator::new(10);
    // total=180>10, left=90>10, right=90>10 -> skip 2-row
    // A+B=60>10 -> 3-row condition false
    // But max_rows == 3 -> returns three_rows()
    let widths = [30, 30, 30, 30, 30, 30];
    let layout = calc.calculate(&widths, 3);
    assert_eq!(layout.row_count(), 3);
}

#[test]
fn test_layout_calculator_fallback_three_rows() {
    // Force the path where max_rows > 3 but nothing fits,
    // reaching the fallback `MultiRowLayout::three_rows()` at line 183.
    let calc = LayoutCalculator::new(10);
    // Total = 360, nothing fits in 10 width.
    // left(180) > 10, right(180) > 10 -> not 2-row
    // A+B(60) > 10, C(120) > 10, X+Y+Z(180) > 10 -> not 3-row condition
    // max_rows = 5, so none of the `max_rows == N` guards fire
    // Falls through to line 183: `MultiRowLayout::three_rows()`
    let widths = [30, 30, 120, 60, 60, 60];
    let layout = calc.calculate(&widths, 5);
    assert_eq!(layout.row_count(), 3);
}

#[test]
fn test_layout_calculator_three_row_condition_line_180() {
    // Test the exact condition at line 178-180:
    // ab_width <= width && c_width <= width && xyz_width <= width
    let calc = LayoutCalculator::new(50);
    // Total = 100, doesn't fit in 50 (line 161)
    // left = 10+10+30 = 50, right = 20+15+15 = 50
    // Both fit in 50, so two_rows() returns at line 170
    // We need left OR right to NOT fit to skip line 170,
    // but A+B, C, and X+Y+Z each fit in 50.
    let widths = [20, 20, 40, 20, 20, 20];
    // total = 140, doesn't fit
    // left = 80 > 50, right = 60 > 50 -> skip 2-row
    // A+B = 40 <= 50, C = 40 <= 50, X+Y+Z = 60 > 50 -> condition false
    // max_rows != 3 (using 5), so fallback at line 183
    let layout = calc.calculate(&widths, 5);
    assert_eq!(layout.row_count(), 3);

    // Now test where the 3-row condition IS true (line 178 true -> 179-180):
    // widths [10, 10, 30, 15, 15, 15] total=95 but left=50<=50, right=45<=50
    // -> takes 2-row path. Adjust so left doesn't fit in 50:
    let widths3 = [10, 10, 40, 15, 15, 15];
    // total = 105, doesn't fit in 50
    // left = 60 > 50 -> skip 2-row
    // A+B = 20 <= 50, C = 40 <= 50, X+Y+Z = 45 <= 50 -> 3-row condition TRUE
    let layout2 = calc.calculate(&widths3, 5);
    assert_eq!(layout2.row_count(), 3);
}

#[test]
fn test_calculate_from_metrics() {
    use super::super::height::ContentMetrics;

    let calc = LayoutCalculator::new(100);
    let metrics = ContentMetrics {
        total_width: 80,
        left_width: 40,
        right_width: 40,
        section_widths: [10, 10, 20, 10, 15, 15],
    };
    let layout = calc.calculate_from_metrics(&metrics, 3);
    assert_eq!(layout.row_count(), 1);

    // Also test overflow case
    let metrics_overflow = ContentMetrics {
        total_width: 200,
        left_width: 100,
        right_width: 100,
        section_widths: [30, 30, 40, 30, 30, 40],
    };
    let layout2 = calc.calculate_from_metrics(&metrics_overflow, 3);
    assert!(layout2.row_count() >= 2);
}

#[test]
fn test_sections_for_row_out_of_bounds() {
    let layout = MultiRowLayout::single_row();
    assert_eq!(layout.sections_for_row(5), &[] as &[SectionId]);
}

#[test]
fn test_layout_calculator_max_rows_1() {
    let calc = LayoutCalculator::new(10);
    let widths = [30, 30, 30, 30, 30, 30];
    let layout = calc.calculate(&widths, 1);
    assert_eq!(layout.row_count(), 1);
}

#[test]
fn test_layout_calculator_two_row_left_fits_right_does_not() {
    // Line 169: left_width <= width (true) && right_width <= width (false), max_rows != 2
    // Forces fall-through to 3-row check
    let calc = LayoutCalculator::new(50);
    // left = 10+10+20 = 40 <= 50 (true), right = 20+20+20 = 60 > 50 (false)
    let widths = [10, 10, 20, 20, 20, 20];
    let layout = calc.calculate(&widths, 5);
    // Should NOT take 2-row path. Instead checks 3-row.
    // A+B=20<=50, C=20<=50, X+Y+Z=60>50 -> 3-row false, fallback
    assert_eq!(layout.row_count(), 3);
}

#[test]
fn test_layout_calculator_three_row_ab_exceeds() {
    // Line 178: ab_width > width (first term false), skip 3-row condition
    let calc = LayoutCalculator::new(30);
    // total=120>30, left=70>30, right=50>30 -> skip 2-row
    // A+B=40>30 -> first term of 3-row AND is false
    // max_rows=5 -> fallback to three_rows
    let widths = [20, 20, 30, 20, 15, 15];
    let layout = calc.calculate(&widths, 5);
    assert_eq!(layout.row_count(), 3);
}

#[test]
fn test_layout_calculator_three_row_ab_fits_c_exceeds() {
    // Line 178 MC/DC: ab_width <= width (true), c_width > width (false)
    // Middle sub-condition independently flips the 3-term AND
    let calc = LayoutCalculator::new(40);
    // left=40+70=110>40, right=60>40 -> skip 2-row
    // A+B=20<=40 (true), C=70>40 (false) -> 3-row AND fails at second term
    // max_rows=5 -> fallback to three_rows
    let widths = [10, 10, 70, 20, 20, 20];
    let layout = calc.calculate(&widths, 5);
    assert_eq!(layout.row_count(), 3);
}

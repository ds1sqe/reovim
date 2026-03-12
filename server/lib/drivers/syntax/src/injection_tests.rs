use super::*;

#[test]
fn test_injection_new() {
    let inj = Injection::new("rust", 100..200, 5, 3, 10, 3);

    assert_eq!(inj.language_id, "rust");
    assert_eq!(inj.byte_range, 100..200);
    assert_eq!(inj.start_row, 5);
    assert_eq!(inj.start_col, 3);
    assert_eq!(inj.end_row, 10);
    assert_eq!(inj.end_col, 3);
}

#[test]
fn test_injection_from_bytes() {
    let inj = Injection::from_bytes("python", 50..150);

    assert_eq!(inj.language_id, "python");
    assert_eq!(inj.byte_range, 50..150);
    assert_eq!(inj.start_row, 0);
    assert_eq!(inj.start_col, 0);
    assert_eq!(inj.end_row, 0);
    assert_eq!(inj.end_col, 0);
}

#[test]
fn test_injection_overlaps_lines() {
    let inj = Injection::new("rust", 100..200, 5, 0, 10, 0);

    // Overlapping line ranges
    assert!(inj.overlaps_lines(5, 10)); // Exact match
    assert!(inj.overlaps_lines(0, 7)); // Overlaps start
    assert!(inj.overlaps_lines(8, 15)); // Overlaps end
    assert!(inj.overlaps_lines(6, 8)); // Fully inside
    assert!(inj.overlaps_lines(0, 20)); // Fully contains

    // Non-overlapping line ranges
    assert!(!inj.overlaps_lines(0, 4));
    assert!(!inj.overlaps_lines(11, 20));
}

#[test]
fn test_injection_contains_line() {
    let inj = Injection::new("rust", 100..200, 5, 0, 10, 0);

    assert!(inj.contains_line(5));
    assert!(inj.contains_line(7));
    assert!(inj.contains_line(10));
    assert!(!inj.contains_line(4));
    assert!(!inj.contains_line(11));
}

#[test]
fn test_injection_overlaps_bytes() {
    let inj = Injection::new("rust", 100..200, 0, 0, 0, 0);

    assert!(inj.overlaps_bytes(&(50..150)));
    assert!(inj.overlaps_bytes(&(150..250)));
    assert!(inj.overlaps_bytes(&(120..180)));
    assert!(inj.overlaps_bytes(&(50..250)));

    assert!(!inj.overlaps_bytes(&(0..100)));
    assert!(!inj.overlaps_bytes(&(200..300)));
}

#[test]
fn test_injection_byte_len() {
    let inj = Injection::new("rust", 100..200, 0, 0, 0, 0);
    assert_eq!(inj.byte_len(), 100);

    let empty = Injection::new("rust", 100..100, 0, 0, 0, 0);
    assert_eq!(empty.byte_len(), 0);
}

#[test]
fn test_injection_is_multiline() {
    let multiline = Injection::new("rust", 100..200, 5, 0, 10, 0);
    assert!(multiline.is_multiline());

    let single = Injection::new("rust", 100..200, 5, 0, 5, 10);
    assert!(!single.is_multiline());
}

#[test]
fn test_injection_line_count() {
    let inj = Injection::new("rust", 100..200, 5, 0, 10, 0);
    assert_eq!(inj.line_count(), 6); // Lines 5, 6, 7, 8, 9, 10

    let single = Injection::new("rust", 100..200, 5, 0, 5, 10);
    assert_eq!(single.line_count(), 1);
}

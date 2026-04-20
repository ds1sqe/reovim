use super::*;

#[test]
fn test_color_assignment_deterministic() {
    // Same client ID should always get the same color
    assert_eq!(color_for_client(0), color_for_client(0));
    assert_eq!(color_for_client(42), color_for_client(42));
}

#[test]
fn test_color_wraps_at_8() {
    // Colors wrap around after 8 clients
    assert_eq!(color_for_client(0), color_for_client(8));
    assert_eq!(color_for_client(1), color_for_client(9));
    assert_eq!(color_for_client(7), color_for_client(15));
}

#[test]
fn test_all_colors_distinct() {
    // All 8 base colors should be different
    for i in 0..8 {
        for j in (i + 1)..8 {
            assert_ne!(
                color_for_client(i),
                color_for_client(j),
                "Colors at index {i} and {j} should be different"
            );
        }
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_dimmed_color() {
    // Dimmed should be 1/4 intensity
    match (color_for_client(0), dimmed_color_for_client(0)) {
        (
            Color::Rgb { r, g, b },
            Color::Rgb {
                r: dr,
                g: dg,
                b: db,
            },
        ) => {
            assert_eq!(dr, r / 4);
            assert_eq!(dg, g / 4);
            assert_eq!(db, b / 4);
        }
        _ => panic!("Expected RGB colors"),
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_dark_color() {
    // Dark should be 1/3 intensity
    match (color_for_client(0), dark_color_for_client(0)) {
        (
            Color::Rgb { r, g, b },
            Color::Rgb {
                r: dr,
                g: dg,
                b: db,
            },
        ) => {
            assert_eq!(dr, r / 3);
            assert_eq!(dg, g / 3);
            assert_eq!(db, b / 3);
        }
        _ => panic!("Expected RGB colors"),
    }
}

#[test]
fn test_palette_has_8_colors() {
    assert_eq!(REMOTE_CLIENT_COLORS.len(), 8);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_all_palette_colors_are_rgb() {
    for (i, color) in REMOTE_CLIENT_COLORS.iter().enumerate() {
        assert!(
            matches!(color, Color::Rgb { .. }),
            "Palette color at index {i} should be RGB, got: {color:?}"
        );
    }
}

#[test]
fn test_color_for_client_large_ids() {
    // Very large client IDs should wrap correctly
    assert_eq!(color_for_client(u64::MAX), color_for_client(u64::MAX % 8));
    assert_eq!(color_for_client(1_000_000), color_for_client(1_000_000 % 8));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_dimmed_color_all_clients() {
    for id in 0..8 {
        match (color_for_client(id), dimmed_color_for_client(id)) {
            (
                Color::Rgb { r, g, b },
                Color::Rgb {
                    r: dr,
                    g: dg,
                    b: db,
                },
            ) => {
                assert_eq!(dr, r / 4, "Dimmed red mismatch for client {id}");
                assert_eq!(dg, g / 4, "Dimmed green mismatch for client {id}");
                assert_eq!(db, b / 4, "Dimmed blue mismatch for client {id}");
            }
            _ => panic!("Expected RGB colors for client {id}"),
        }
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_dark_color_all_clients() {
    for id in 0..8 {
        match (color_for_client(id), dark_color_for_client(id)) {
            (
                Color::Rgb { r, g, b },
                Color::Rgb {
                    r: dr,
                    g: dg,
                    b: db,
                },
            ) => {
                assert_eq!(dr, r / 3, "Dark red mismatch for client {id}");
                assert_eq!(dg, g / 3, "Dark green mismatch for client {id}");
                assert_eq!(db, b / 3, "Dark blue mismatch for client {id}");
            }
            _ => panic!("Expected RGB colors for client {id}"),
        }
    }
}

#[test]
fn test_dimmed_wraps_same_as_color() {
    assert_eq!(dimmed_color_for_client(0), dimmed_color_for_client(8));
    assert_eq!(dimmed_color_for_client(3), dimmed_color_for_client(11));
}

#[test]
fn test_dark_wraps_same_as_color() {
    assert_eq!(dark_color_for_client(0), dark_color_for_client(8));
    assert_eq!(dark_color_for_client(5), dark_color_for_client(13));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_dimmed_is_darker_than_original() {
    for id in 0..8 {
        match (color_for_client(id), dimmed_color_for_client(id)) {
            (
                Color::Rgb { r, g, b },
                Color::Rgb {
                    r: dr,
                    g: dg,
                    b: db,
                },
            ) => {
                assert!(dr <= r, "Dimmed red should be <= original for client {id}");
                assert!(dg <= g, "Dimmed green should be <= original for client {id}");
                assert!(db <= b, "Dimmed blue should be <= original for client {id}");
            }
            _ => panic!("Expected RGB colors for client {id}"),
        }
    }
}

#[test]
fn test_first_color_is_orange() {
    // First palette color should be orange (230, 159, 0) per CBF-8
    assert_eq!(
        color_for_client(0),
        Color::Rgb {
            r: 230,
            g: 159,
            b: 0
        }
    );
}

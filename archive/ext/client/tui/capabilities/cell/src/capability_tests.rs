use crate::{
    capability::{Cell, CellCapability, WriteCellError},
    style::{CellAttrs, CellColor, CellStyle},
};

#[test]
fn default_cell_is_space_with_default_style() {
    let c = Cell::default();
    assert_eq!(c.ch, ' ');
    assert_eq!(c.style, CellStyle::default());
}

#[test]
fn cell_new_stores_char_and_style() {
    let s = CellStyle::plain().with_attrs(CellAttrs::BOLD);
    let c = Cell::new('X', s);
    assert_eq!(c.ch, 'X');
    assert_eq!(c.style, s);
}

#[test]
fn new_grid_has_expected_dimensions_and_default_cells() {
    let g = CellCapability::new(3, 2);
    assert_eq!(g.width(), 3);
    assert_eq!(g.height(), 2);
    assert_eq!(g.len(), 6);
    assert!(!g.is_empty());
    for y in 0..2 {
        for x in 0..3 {
            assert_eq!(g.get_cell(x, y), Some(&Cell::default()));
        }
    }
}

#[test]
fn zero_width_grid_is_empty() {
    let g = CellCapability::new(0, 5);
    assert_eq!(g.len(), 0);
    assert!(g.is_empty());
}

#[test]
fn zero_height_grid_is_empty() {
    let g = CellCapability::new(5, 0);
    assert_eq!(g.len(), 0);
    assert!(g.is_empty());
}

#[test]
fn write_cell_in_bounds_stores_value() {
    let mut g = CellCapability::new(2, 2);
    let new_cell = Cell::new('Q', CellStyle::plain().with_attrs(CellAttrs::BOLD));
    g.write_cell(1, 0, new_cell.clone()).expect("in bounds");
    assert_eq!(g.get_cell(1, 0), Some(&new_cell));
    assert_eq!(g.get_cell(0, 0), Some(&Cell::default()));
}

#[test]
fn write_cell_out_of_bounds_x_errors() {
    let mut g = CellCapability::new(2, 2);
    let err = g.write_cell(2, 0, Cell::default()).unwrap_err();
    assert_eq!(
        err,
        WriteCellError {
            x: 2,
            y: 0,
            width: 2,
            height: 2
        }
    );
    assert_eq!(g.get_cell(0, 0), Some(&Cell::default()));
}

#[test]
fn write_cell_out_of_bounds_y_errors() {
    let mut g = CellCapability::new(2, 2);
    let err = g.write_cell(0, 2, Cell::default()).unwrap_err();
    assert_eq!(
        err,
        WriteCellError {
            x: 0,
            y: 2,
            width: 2,
            height: 2
        }
    );
}

#[test]
fn get_cell_out_of_bounds_returns_none() {
    let g = CellCapability::new(2, 2);
    assert!(g.get_cell(2, 0).is_none());
    assert!(g.get_cell(0, 2).is_none());
    assert!(g.get_cell(100, 100).is_none());
}

#[test]
fn fill_overwrites_every_cell() {
    let mut g = CellCapability::new(2, 2);
    let style = CellStyle::plain().with_fg(CellColor::Ansi256(9));
    g.fill('*', style);
    for y in 0..2 {
        for x in 0..2 {
            let c = g.get_cell(x, y).unwrap();
            assert_eq!(c.ch, '*');
            assert_eq!(c.style, style);
        }
    }
}

#[test]
fn clear_resets_to_default() {
    let mut g = CellCapability::new(2, 1);
    g.fill('x', CellStyle::plain().with_attrs(CellAttrs::BOLD));
    g.clear();
    assert_eq!(g.get_cell(0, 0), Some(&Cell::default()));
    assert_eq!(g.get_cell(1, 0), Some(&Cell::default()));
}

#[test]
fn resize_changes_dimensions_and_resets_content() {
    let mut g = CellCapability::new(2, 2);
    g.fill('x', CellStyle::plain());
    g.resize(3, 1);
    assert_eq!(g.width(), 3);
    assert_eq!(g.height(), 1);
    assert_eq!(g.len(), 3);
    for x in 0..3 {
        assert_eq!(g.get_cell(x, 0), Some(&Cell::default()));
    }
}

#[test]
fn resize_to_zero_empties_the_grid() {
    let mut g = CellCapability::new(2, 2);
    g.resize(0, 0);
    assert!(g.is_empty());
    assert!(g.get_cell(0, 0).is_none());
}

#[test]
fn iter_yields_row_major_coordinates() {
    let mut g = CellCapability::new(3, 2);
    g.write_cell(2, 0, Cell::new('a', CellStyle::default()))
        .unwrap();
    g.write_cell(0, 1, Cell::new('b', CellStyle::default()))
        .unwrap();

    let collected: Vec<_> = g.iter().collect();
    assert_eq!(collected.len(), 6);
    assert_eq!(collected[0].0, (0, 0));
    assert_eq!(collected[2].0, (2, 0));
    assert_eq!(collected[3].0, (0, 1));
    assert_eq!(collected[5].0, (2, 1));
    assert_eq!(collected[2].1.ch, 'a');
    assert_eq!(collected[3].1.ch, 'b');
}

#[test]
fn write_cell_error_formats_coordinates_and_grid_size() {
    let err = WriteCellError {
        x: 7,
        y: 2,
        width: 4,
        height: 3,
    };
    let rendered = format!("{err}");
    assert!(rendered.contains("(7, 2)"));
    assert!(rendered.contains("4x3"));
}

#[test]
fn write_cell_error_is_std_error() {
    let err = WriteCellError {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };
    let e: &dyn std::error::Error = &err;
    assert!(e.source().is_none());
}

#[test]
fn write_cell_error_is_copy_clone_eq() {
    let a = WriteCellError {
        x: 1,
        y: 2,
        width: 3,
        height: 4,
    };
    let b = a;
    let c = a;
    assert_eq!(a, b);
    assert_eq!(a, c);
}

#[test]
fn large_grid_preserves_row_major_ordering() {
    let mut g = CellCapability::new(100, 50);
    g.write_cell(99, 49, Cell::new('Z', CellStyle::default()))
        .expect("corner in bounds");
    assert_eq!(g.get_cell(99, 49).map(|c| c.ch), Some('Z'));
    assert!(g.write_cell(100, 0, Cell::default()).is_err());
    assert!(g.write_cell(0, 50, Cell::default()).is_err());
}

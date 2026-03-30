//! Column mapping for cursor positioning inside table rows.
//!
//! Maps buffer column positions to visual column positions in expanded
//! table rows (accounting for padding and box-drawing borders).

/// Build column mapping from buffer positions to visual positions in an expanded row.
#[allow(clippy::cast_possible_truncation)]
pub fn build_column_mapping(original: &str, max_widths: &[usize]) -> Vec<u16> {
    let orig_len = original.len();
    if orig_len == 0 || max_widths.is_empty() {
        return Vec::new();
    }
    let orig_pipes: Vec<usize> = original
        .bytes()
        .enumerate()
        .filter(|(_, b)| *b == b'|')
        .map(|(i, _)| i)
        .collect();
    if orig_pipes.len() < 2 {
        #[allow(clippy::cast_possible_truncation)]
        return (0..orig_len).map(|i| i as u16).collect();
    }
    let mut visual_pipes = vec![0u16];
    for &width in max_widths {
        let prev = *visual_pipes.last().unwrap();
        visual_pipes.push(prev + (width as u16) + 3);
    }
    (0..orig_len)
        .map(|buf_col| map_to_visual(buf_col, &orig_pipes, &visual_pipes))
        .collect()
}

/// Map a buffer column to visual column using pipe positions.
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::cast_possible_truncation)]
pub fn map_to_visual(buf_col: usize, orig_pipes: &[usize], visual_pipes: &[u16]) -> u16 {
    if let Some(idx) = orig_pipes.iter().position(|&p| p == buf_col) {
        return visual_pipes.get(idx).copied().unwrap_or(buf_col as u16);
    }
    let seg = orig_pipes
        .iter()
        .position(|&p| p > buf_col)
        .unwrap_or(orig_pipes.len());
    if seg == 0 || seg >= orig_pipes.len() {
        return buf_col as u16;
    }
    let buf_cell_start = orig_pipes[seg - 1] + 1;
    let buf_cell_end = orig_pipes[seg];
    let buf_cell_len = buf_cell_end - buf_cell_start;
    let vis_cell_start = visual_pipes[seg - 1] + 1;
    let vis_cell_end = visual_pipes[seg];
    let pos_in_buf_cell = buf_col - buf_cell_start;
    if pos_in_buf_cell == 0 {
        vis_cell_start
    } else if pos_in_buf_cell >= buf_cell_len.saturating_sub(1) {
        vis_cell_end.saturating_sub(1)
    } else {
        let max_content_pos = vis_cell_end.saturating_sub(2);
        (vis_cell_start + pos_in_buf_cell as u16).min(max_content_pos)
    }
}

#[cfg(test)]
#[path = "mapping_tests.rs"]
mod tests;

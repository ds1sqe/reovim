use super::*;

#[test]
fn test_build_column_mapping_empty() {
    assert!(build_column_mapping("", &[3]).is_empty());
    assert!(build_column_mapping("| a |", &[]).is_empty());
}

#[test]
fn test_build_column_mapping_single_pipe() {
    let mapping = build_column_mapping("abc|def", &[3]);
    assert_eq!(mapping.len(), 7);
    #[allow(clippy::cast_possible_truncation)]
    for (i, &v) in mapping.iter().enumerate() {
        assert_eq!(v, i as u16);
    }
}

#[test]
fn test_build_column_mapping_basic() {
    let mapping = build_column_mapping("| a | b |", &[3, 3]);
    assert_eq!(mapping.len(), 9);
    assert_eq!(mapping[0], 0);
}

#[test]
fn test_map_to_visual_pipe_match() {
    let orig_pipes = vec![0, 4, 8];
    let visual_pipes = vec![0, 6, 12];
    assert_eq!(map_to_visual(0, &orig_pipes, &visual_pipes), 0);
    assert_eq!(map_to_visual(4, &orig_pipes, &visual_pipes), 6);
    assert_eq!(map_to_visual(8, &orig_pipes, &visual_pipes), 12);
}

#[test]
fn test_map_to_visual_content() {
    let orig_pipes = vec![0, 4, 8];
    let visual_pipes = vec![0, 6, 12];
    assert_eq!(map_to_visual(1, &orig_pipes, &visual_pipes), 1);
}

#[test]
fn test_map_to_visual_before_first_pipe() {
    let orig_pipes = vec![2, 6];
    let visual_pipes = vec![0, 8];
    assert_eq!(map_to_visual(0, &orig_pipes, &visual_pipes), 0);
}

use super::*;

#[test]
fn test_scaling_constants() {
    assert_eq!(scaling::SIZES_SMALL.len(), 3);
    assert_eq!(scaling::SIZES_MEDIUM.len(), 3);
    assert_eq!(scaling::SIZES_LARGE.len(), 3);

    // Each set should be strictly increasing
    for sizes in [
        scaling::SIZES_SMALL,
        scaling::SIZES_MEDIUM,
        scaling::SIZES_LARGE,
    ] {
        for window in sizes.windows(2) {
            assert!(window[0] < window[1]);
        }
    }
}

#[test]
fn test_slow_bench_config() {
    let config = slow_bench_config();
    // Criterion doesn't expose getters, but we can verify it builds without panic.
    // The real validation is that benchmarks using this config run correctly.
    drop(config);
}

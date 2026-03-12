use super::*;

use std::collections::HashMap;

#[test]
fn test_line_cache_new() {
    let cache: LineCache<String> = LineCache::new();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
}

#[test]
fn test_line_cache_has_with_valid_hash() {
    let cache: LineCache<i32> = LineCache::new();
    let mut entries = HashMap::new();
    entries.insert(0, (12345u64, 100));
    cache.store(entries);

    assert!(cache.has(0, 12345));
}

#[test]
fn test_line_cache_has_with_invalid_hash() {
    let cache: LineCache<i32> = LineCache::new();
    let mut entries = HashMap::new();
    entries.insert(0, (12345u64, 100));
    cache.store(entries);

    assert!(!cache.has(0, 99999)); // Wrong hash
    assert!(!cache.has(1, 12345)); // Wrong line
}

#[test]
fn test_line_cache_get_if_valid() {
    let cache: LineCache<String> = LineCache::new();
    let mut entries = HashMap::new();
    entries.insert(0, (100u64, "hello".to_string()));
    entries.insert(1, (200u64, "world".to_string()));
    cache.store(entries);

    assert_eq!(cache.get_if_valid(0, 100), Some("hello".to_string()));
    assert_eq!(cache.get_if_valid(1, 200), Some("world".to_string()));
    assert_eq!(cache.get_if_valid(0, 999), None); // Wrong hash
    assert_eq!(cache.get_if_valid(2, 100), None); // Wrong line
}

#[test]
fn test_line_cache_store() {
    let cache: LineCache<i32> = LineCache::new();

    let mut entries1 = HashMap::new();
    entries1.insert(0, (1u64, 10));
    cache.store(entries1);
    assert_eq!(cache.len(), 1);

    // Store replaces all content
    let mut entries2 = HashMap::new();
    entries2.insert(1, (2u64, 20));
    entries2.insert(2, (3u64, 30));
    cache.store(entries2);
    assert_eq!(cache.len(), 2);
    assert!(!cache.has(0, 1)); // Line 0 was replaced
}

#[test]
fn test_line_cache_update() {
    let cache: LineCache<i32> = LineCache::new();

    let mut entries1 = HashMap::new();
    entries1.insert(0, (1u64, 10));
    cache.store(entries1);

    // Update merges, doesn't replace
    let mut entries2 = HashMap::new();
    entries2.insert(1, (2u64, 20));
    cache.update(&entries2);

    assert_eq!(cache.len(), 2);
    assert!(cache.has(0, 1)); // Line 0 preserved
    assert!(cache.has(1, 2)); // Line 1 added
}

#[test]
fn test_line_cache_clone_entries() {
    let cache: LineCache<i32> = LineCache::new();
    let mut entries = HashMap::new();
    entries.insert(0, (1u64, 10));
    entries.insert(1, (2u64, 20));
    cache.store(entries.clone());

    let cloned = cache.clone_entries();
    assert_eq!(cloned, entries);
}

#[test]
fn test_line_cache_invalidate_from() {
    let cache: LineCache<i32> = LineCache::new();
    let mut entries = HashMap::new();
    entries.insert(0, (1u64, 10));
    entries.insert(1, (2u64, 20));
    entries.insert(2, (3u64, 30));
    entries.insert(3, (4u64, 40));
    cache.store(entries);

    cache.invalidate_from(2);

    assert!(cache.has(0, 1));
    assert!(cache.has(1, 2));
    assert!(!cache.has(2, 3)); // Invalidated
    assert!(!cache.has(3, 4)); // Invalidated
    assert_eq!(cache.len(), 2);
}

#[test]
fn test_line_cache_invalidate_line() {
    let cache: LineCache<i32> = LineCache::new();
    let mut entries = HashMap::new();
    entries.insert(0, (1u64, 10));
    entries.insert(1, (2u64, 20));
    cache.store(entries);

    cache.invalidate_line(0);

    assert!(!cache.has(0, 1));
    assert!(cache.has(1, 2));
    assert_eq!(cache.len(), 1);
}

#[test]
fn test_line_cache_clear() {
    let cache: LineCache<i32> = LineCache::new();
    let mut entries = HashMap::new();
    entries.insert(0, (1u64, 10));
    cache.store(entries);

    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn test_line_cache_concurrent_access() {
    use std::{sync::Arc, thread};

    let cache = Arc::new(LineCache::<i32>::new());

    // Spawn readers
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let cache = Arc::clone(&cache);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = cache.has(i, i as u64);
                    let _ = cache.get_if_valid(i, i as u64);
                }
            })
        })
        .collect();

    // Concurrent writer
    let writer_cache = Arc::clone(&cache);
    let writer = thread::spawn(move || {
        for j in 0_usize..100 {
            let mut entries = HashMap::new();
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            entries.insert(j % 10, (j as u64, j as i32));
            writer_cache.update(&entries);
        }
    });

    for handle in handles {
        handle.join().unwrap();
    }
    writer.join().unwrap();
}

#[test]
fn test_line_cache_get_without_validation() {
    let cache: LineCache<String> = LineCache::new();
    let mut entries = HashMap::new();
    entries.insert(5, (999u64, "data".to_string()));
    cache.store(entries);

    let result = cache.get(5);
    assert!(result.is_some());
    let (hash, value) = result.unwrap();
    assert_eq!(hash, 999);
    assert_eq!(value, "data");

    assert!(cache.get(99).is_none());
}

#[test]
fn test_line_cache_cached_lines() {
    let cache: LineCache<i32> = LineCache::new();
    let mut entries = HashMap::new();
    entries.insert(1, (1u64, 10));
    entries.insert(5, (5u64, 50));
    entries.insert(10, (10u64, 100));
    cache.store(entries);

    let mut lines = cache.cached_lines();
    lines.sort_unstable();
    assert_eq!(lines, vec![1, 5, 10]);
}

#[test]
fn test_line_cache_default() {
    let cache: LineCache<i32> = LineCache::default();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
}

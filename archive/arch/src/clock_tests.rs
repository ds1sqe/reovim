use {super::*, std::sync::Arc};

#[test]
fn system_clock_advances() {
    let clock = SystemClock;
    let t1 = clock.now();
    let t2 = clock.now();
    assert!(t2 >= t1);
}

#[test]
fn test_clock_stable_without_advance() {
    let clock = TestClock::new();
    let t1 = clock.now();
    let t2 = clock.now();
    assert_eq!(t1, t2);
}

#[test]
fn test_clock_advances_by_duration() {
    let clock = TestClock::new();
    let t1 = clock.now();
    clock.advance(Duration::from_millis(500));
    let t2 = clock.now();
    assert_eq!(t2 - t1, Duration::from_millis(500));
}

#[test]
fn test_clock_cumulative_advance() {
    let clock = TestClock::new();
    let t1 = clock.now();
    clock.advance(Duration::from_millis(100));
    clock.advance(Duration::from_millis(200));
    let t2 = clock.now();
    assert_eq!(t2 - t1, Duration::from_millis(300));
}

#[test]
fn test_clock_default() {
    let clock = TestClock::default();
    let t = clock.now();
    assert!(t <= Instant::now());
}

#[test]
fn test_clock_is_send_sync() {
    let clock = Arc::new(TestClock::new());
    let clone = Arc::clone(&clock);
    let handle = std::thread::spawn(move || {
        clone.advance(Duration::from_millis(100));
        clone.now()
    });
    let t_thread = handle.join().unwrap();
    let t_main = clock.now();
    assert_eq!(t_thread, t_main);
}

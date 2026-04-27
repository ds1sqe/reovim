use super::*;

struct CountingObserver {
    remaining: u32,
}

impl DebugObserver for CountingObserver {
    fn next_frame(&mut self) -> Result<Option<Vec<u8>>, DebugError> {
        if self.remaining == 0 {
            Ok(None)
        } else {
            self.remaining -= 1;
            Ok(Some(format!("frame-{}", self.remaining).into_bytes()))
        }
    }
}

struct FailingObserver;

impl DebugObserver for FailingObserver {
    fn next_frame(&mut self) -> Result<Option<Vec<u8>>, DebugError> {
        Err(DebugError("stream broken".into()))
    }
}

#[test]
fn observer_pumps_frames_then_signals_end_of_stream() {
    let mut obs = CountingObserver { remaining: 2 };
    assert_eq!(obs.next_frame().unwrap().unwrap(), b"frame-1");
    assert_eq!(obs.next_frame().unwrap().unwrap(), b"frame-0");
    assert!(obs.next_frame().unwrap().is_none());
}

#[test]
fn observer_surfaces_driver_error() {
    let mut obs = FailingObserver;
    let err = obs.next_frame().unwrap_err();
    assert_eq!(err.to_string(), "stream broken");
}

#[test]
fn observer_is_object_safe() {
    let mut obs: Box<dyn DebugObserver> = Box::new(CountingObserver { remaining: 1 });
    assert!(obs.next_frame().unwrap().is_some());
    assert!(obs.next_frame().unwrap().is_none());
}

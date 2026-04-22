use super::*;

struct RecordingTarget {
    submissions: Vec<Vec<u8>>,
}

impl RecordingTarget {
    fn new() -> Self {
        Self {
            submissions: Vec::new(),
        }
    }
}

impl RenderTarget for RecordingTarget {
    fn submit(&mut self, data: &[u8]) -> Result<(), RenderError> {
        self.submissions.push(data.to_vec());
        Ok(())
    }
}

struct FailingTarget;

impl RenderTarget for FailingTarget {
    fn submit(&mut self, _data: &[u8]) -> Result<(), RenderError> {
        Err(RenderError::NotReady)
    }
}

#[test]
fn render_target_submit_records_data() {
    let mut target = RecordingTarget::new();
    assert!(target.submit(&[1, 2, 3]).is_ok());
    assert!(target.submit(&[4, 5]).is_ok());
    assert_eq!(target.submissions.len(), 2);
    assert_eq!(target.submissions[0], vec![1, 2, 3]);
    assert_eq!(target.submissions[1], vec![4, 5]);
}

#[test]
fn render_target_submit_error() {
    let mut target = FailingTarget;
    let err = target.submit(&[1]).unwrap_err();
    assert_eq!(err, RenderError::NotReady);
}

#[test]
fn render_error_display() {
    let err = RenderError::InvalidData("bad header".into());
    assert_eq!(err.to_string(), "invalid render data: bad header");

    let err = RenderError::NotReady;
    assert_eq!(err.to_string(), "render target not ready");
}

#[test]
fn render_target_object_safety() {
    let mut target: Box<dyn RenderTarget> = Box::new(RecordingTarget::new());
    assert!(target.submit(&[0xFF]).is_ok());
}

#[test]
fn render_target_empty_submission() {
    let mut target = RecordingTarget::new();
    assert!(target.submit(&[]).is_ok());
    assert_eq!(target.submissions.len(), 1);
    assert!(target.submissions[0].is_empty());
}

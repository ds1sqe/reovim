//! Opaque render target for domain content submission.
//!
//! [`RenderTarget`] defines a platform-agnostic byte-submission interface.
//! Domain view modules submit encoded render commands; platform adapters
//! decode them using `render-codec`.

/// Error returned when a render submission fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// The submitted data could not be decoded by the platform adapter.
    InvalidData(String),
    /// The render target is not ready (e.g., surface not yet initialized).
    NotReady,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidData(msg) => write!(f, "invalid render data: {msg}"),
            Self::NotReady => write!(f, "render target not ready"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Opaque render target for domain content submission.
///
/// Domain view modules call [`submit`](RenderTarget::submit) with encoded
/// render command bytes. Platform adapters implement this trait and decode
/// the bytes using the appropriate `render-codec` surface module.
///
/// No coordinates or surface dimensions appear in this trait — those are
/// encoded in the command buffer by the domain driver and decoded by the
/// platform adapter.
pub trait RenderTarget: Send + Sync {
    /// Submit encoded render commands.
    ///
    /// The byte slice contains a `render-codec` `CommandBuffer` payload.
    /// The platform adapter decodes and executes the commands.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the data cannot be processed.
    fn submit(&mut self, data: &[u8]) -> Result<(), RenderError>;
}

#[cfg(test)]
mod tests {
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
}

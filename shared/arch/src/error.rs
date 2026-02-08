//! Error types for architecture layer.

use std::fmt;

/// Architecture layer error.
#[derive(Debug)]
pub enum ArchError {
    /// I/O error from platform.
    Io(std::io::Error),
    /// Feature not supported on this platform.
    NotSupported(&'static str),
}

impl fmt::Display for ArchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::NotSupported(feature) => write!(f, "Not supported: {feature}"),
        }
    }
}

impl std::error::Error for ArchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::NotSupported(_) => None,
        }
    }
}

impl From<std::io::Error> for ArchError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_error_io_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let arch_err = ArchError::Io(io_err);
        let display = format!("{arch_err}");
        assert!(
            display.starts_with("I/O error:"),
            "Expected 'I/O error:' prefix, got: {display}"
        );
        assert!(
            display.contains("file not found"),
            "Expected 'file not found' in message, got: {display}"
        );
    }

    #[test]
    fn test_arch_error_not_supported_display() {
        let arch_err = ArchError::NotSupported("clipboard");
        let display = format!("{arch_err}");
        assert_eq!(display, "Not supported: clipboard");
    }

    #[test]
    fn test_arch_error_io_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let arch_err = ArchError::Io(io_err);
        let source = std::error::Error::source(&arch_err);
        assert!(source.is_some(), "Io variant should have a source error");
        let source_display = format!("{}", source.unwrap());
        assert!(
            source_display.contains("access denied"),
            "Source should contain original message, got: {source_display}"
        );
    }

    #[test]
    fn test_arch_error_not_supported_source() {
        let arch_err = ArchError::NotSupported("signals");
        let source = std::error::Error::source(&arch_err);
        assert!(source.is_none(), "NotSupported variant should have no source error");
    }

    #[test]
    fn test_arch_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broken");
        let arch_err: ArchError = io_err.into();
        match &arch_err {
            ArchError::Io(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe);
            }
            ArchError::NotSupported(_) => panic!("Expected Io variant"),
        }
    }

    #[test]
    fn test_arch_error_debug() {
        let io_err = std::io::Error::other("test");
        let arch_err = ArchError::Io(io_err);
        let debug = format!("{arch_err:?}");
        assert!(debug.contains("Io"), "Debug should contain variant name, got: {debug}");

        let arch_err = ArchError::NotSupported("feature_x");
        let debug = format!("{arch_err:?}");
        assert!(
            debug.contains("NotSupported"),
            "Debug should contain variant name, got: {debug}"
        );
        assert!(debug.contains("feature_x"), "Debug should contain feature name, got: {debug}");
    }

    #[test]
    fn test_arch_error_is_std_error() {
        // Verify ArchError implements std::error::Error by using it as a trait object
        let io_err = std::io::Error::other("test");
        let arch_err = ArchError::Io(io_err);
        let _: &dyn std::error::Error = &arch_err;

        let arch_err = ArchError::NotSupported("test");
        let _: &dyn std::error::Error = &arch_err;
    }

    #[test]
    fn test_arch_error_various_io_error_kinds() {
        let kinds = [
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::ConnectionRefused,
            std::io::ErrorKind::ConnectionReset,
            std::io::ErrorKind::ConnectionAborted,
            std::io::ErrorKind::NotConnected,
            std::io::ErrorKind::AddrInUse,
            std::io::ErrorKind::BrokenPipe,
            std::io::ErrorKind::AlreadyExists,
            std::io::ErrorKind::WouldBlock,
            std::io::ErrorKind::InvalidInput,
            std::io::ErrorKind::InvalidData,
            std::io::ErrorKind::TimedOut,
            std::io::ErrorKind::WriteZero,
            std::io::ErrorKind::Interrupted,
            std::io::ErrorKind::UnexpectedEof,
        ];
        for kind in kinds {
            let io_err = std::io::Error::new(kind, "test error");
            let arch_err: ArchError = io_err.into();
            // Verify Display works for each kind
            let display = format!("{arch_err}");
            assert!(display.starts_with("I/O error:"));
            // Verify source is present
            assert!(std::error::Error::source(&arch_err).is_some());
        }
    }
}

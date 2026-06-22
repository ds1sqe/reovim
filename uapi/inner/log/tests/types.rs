//! Behavioral tests for `uapi/log` wrappers and function-table dispatch.

use {
    reovim_uapi_log::{LogSinkControl, LogSinkError, LogSinkHandle},
    reovim_uapi_panic::PanicConfigError,
};

#[test]
fn log_sink_handle_is_opaque_and_round_trips() {
    let handle = LogSinkHandle::new(42);
    assert_eq!(handle.raw(), 42);
}

#[test]
fn log_sink_error_carries_bridge_code() {
    let error = LogSinkError::new(13);
    assert_eq!(error.code(), 13);
}

#[test]
fn log_sink_control_dispatches_function_pointers() {
    fn open(path: &[u8]) -> Result<LogSinkHandle, LogSinkError> {
        assert_eq!(path, b"/tmp/reovim.log\0");
        Ok(LogSinkHandle::new(5))
    }
    fn write(handle: LogSinkHandle, bytes: &[u8]) -> Result<(), LogSinkError> {
        assert_eq!(handle.raw(), 5);
        assert_eq!(bytes, b"line\n");
        Ok(())
    }
    fn close(handle: LogSinkHandle) -> Result<(), LogSinkError> {
        assert_eq!(handle.raw(), 5);
        Ok(())
    }
    fn register(handle: LogSinkHandle) -> Result<(), PanicConfigError> {
        assert_eq!(handle.raw(), 5);
        Ok(())
    }
    fn diagnostic(bytes: &[u8]) -> Result<(), LogSinkError> {
        assert_eq!(bytes, b"diag\n");
        Ok(())
    }

    let log = LogSinkControl::new(open, write, close, register, diagnostic);
    let handle = log.open_file_sink(b"/tmp/reovim.log\0").unwrap();
    assert_eq!(handle, LogSinkHandle::new(5));
    assert_eq!(log.write_sink(handle, b"line\n"), Ok(()));
    assert_eq!(log.register_panic_flush(handle), Ok(()));
    assert_eq!(log.write_diagnostic(b"diag\n"), Ok(()));
    assert_eq!(log.close_sink(handle), Ok(()));
}

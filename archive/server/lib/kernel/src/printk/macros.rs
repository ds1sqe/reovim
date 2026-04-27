//! Logging macros for kernel messages.
//!
//! Linux equivalent: `pr_err()`, `pr_warn()`, `pr_info()`, `pr_debug()` in `include/linux/printk.h`
//!
//! These macros provide a convenient way to log messages at different severity levels.
//! They check the log level before formatting to avoid allocation overhead when
//! logging is disabled.

/// Logs a message at the Error level.
///
/// Use for critical errors that require immediate attention and may prevent
/// normal operation.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::pr_err;
///
/// let buffer_id = 42;
/// pr_err!("failed to save buffer {}", buffer_id);
/// pr_err!("critical error occurred");
/// ```
#[macro_export]
macro_rules! pr_err {
    ($($arg:tt)*) => {
        if $crate::api::v1::logger().enabled($crate::api::v1::Level::Error) {
            $crate::api::v1::__log(
                $crate::api::v1::Level::Error,
                module_path!(),
                file!(),
                line!(),
                format_args!($($arg)*)
            )
        }
    };
}

/// Logs a message at the Warn level.
///
/// Use for warning conditions that don't prevent operation but indicate
/// potential problems.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::pr_warn;
///
/// let path = "/tmp/file.txt";
/// pr_warn!("file {} not found, using default", path);
/// pr_warn!("deprecated configuration option used");
/// ```
#[macro_export]
macro_rules! pr_warn {
    ($($arg:tt)*) => {
        if $crate::api::v1::logger().enabled($crate::api::v1::Level::Warn) {
            $crate::api::v1::__log(
                $crate::api::v1::Level::Warn,
                module_path!(),
                file!(),
                line!(),
                format_args!($($arg)*)
            )
        }
    };
}

/// Logs a message at the Info level.
///
/// Use for general informational messages about normal operation.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::pr_info;
///
/// pr_info!("editor started");
/// pr_info!("opened {} buffers", 3);
/// ```
#[macro_export]
macro_rules! pr_info {
    ($($arg:tt)*) => {
        if $crate::api::v1::logger().enabled($crate::api::v1::Level::Info) {
            $crate::api::v1::__log(
                $crate::api::v1::Level::Info,
                module_path!(),
                file!(),
                line!(),
                format_args!($($arg)*)
            )
        }
    };
}

/// Logs a message at the Debug level.
///
/// Use for detailed diagnostic information useful during development.
/// These messages are typically disabled in release builds.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::pr_debug;
///
/// let cursor_pos = (10, 5);
/// pr_debug!("cursor moved to {:?}", cursor_pos);
/// pr_debug!("entering function");
/// ```
#[macro_export]
macro_rules! pr_debug {
    ($($arg:tt)*) => {
        if $crate::api::v1::logger().enabled($crate::api::v1::Level::Debug) {
            $crate::api::v1::__log(
                $crate::api::v1::Level::Debug,
                module_path!(),
                file!(),
                line!(),
                format_args!($($arg)*)
            )
        }
    };
}

/// Logs a message at the Trace level.
///
/// Use for very detailed tracing of execution flow. This is the most
/// verbose level and should be used sparingly.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::pr_trace;
///
/// pr_trace!("processing event");
/// pr_trace!("loop iteration {}", 42);
/// ```
#[macro_export]
macro_rules! pr_trace {
    ($($arg:tt)*) => {
        if $crate::api::v1::logger().enabled($crate::api::v1::Level::Trace) {
            $crate::api::v1::__log(
                $crate::api::v1::Level::Trace,
                module_path!(),
                file!(),
                line!(),
                format_args!($($arg)*)
            )
        }
    };
}

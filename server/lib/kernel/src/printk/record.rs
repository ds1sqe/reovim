//! Log record containing message and metadata.
//!
//! Linux equivalent: `struct printk_log` in `kernel/printk/printk_ringbuffer.h`
//!
//! A `Record` contains all the information about a single log entry:
//! the log level, message, and source location metadata.

use super::level::Level;

/// A log record containing message and source location metadata.
///
/// This struct is lifetime-bound to the message string to avoid
/// allocations during logging. The source location fields (`module_path`,
/// `file`, `line`) are captured at the call site using compiler intrinsics.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let record = Record::builder(Level::Info)
///     .message("buffer opened")
///     .module_path("reovim_kernel::mm")
///     .file("buffer.rs")
///     .line(42)
///     .build();
///
/// assert_eq!(record.level(), Level::Info);
/// assert_eq!(record.message(), "buffer opened");
/// assert_eq!(record.line(), 42);
/// ```
#[derive(Debug, Clone)]
pub struct Record<'a> {
    level: Level,
    message: &'a str,
    module_path: &'static str,
    file: &'static str,
    line: u32,
}

impl<'a> Record<'a> {
    /// Creates a new record builder with the given level.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::*;
    ///
    /// let record = Record::builder(Level::Error)
    ///     .message("something went wrong")
    ///     .build();
    /// ```
    #[must_use]
    pub const fn builder(level: Level) -> RecordBuilder<'a> {
        RecordBuilder::new(level)
    }

    /// Returns the log level.
    #[must_use]
    pub const fn level(&self) -> Level {
        self.level
    }

    /// Returns the log message.
    #[must_use]
    pub const fn message(&self) -> &str {
        self.message
    }

    /// Returns the module path where the log was emitted.
    #[must_use]
    pub const fn module_path(&self) -> &'static str {
        self.module_path
    }

    /// Returns the file where the log was emitted.
    #[must_use]
    pub const fn file(&self) -> &'static str {
        self.file
    }

    /// Returns the line number where the log was emitted.
    #[must_use]
    pub const fn line(&self) -> u32 {
        self.line
    }
}

/// Builder for constructing log records.
///
/// Use [`Record::builder`] to create an instance.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let record = Record::builder(Level::Debug)
///     .message("entering function")
///     .module_path("reovim_kernel::core")
///     .file("motion.rs")
///     .line(100)
///     .build();
/// ```
#[derive(Debug)]
pub struct RecordBuilder<'a> {
    level: Level,
    message: &'a str,
    module_path: &'static str,
    file: &'static str,
    line: u32,
}

impl<'a> RecordBuilder<'a> {
    /// Creates a new builder with the given level.
    #[must_use]
    const fn new(level: Level) -> Self {
        Self {
            level,
            message: "",
            module_path: "",
            file: "",
            line: 0,
        }
    }

    /// Sets the log message.
    #[must_use]
    pub const fn message(mut self, message: &'a str) -> Self {
        self.message = message;
        self
    }

    /// Sets the module path.
    #[must_use]
    pub const fn module_path(mut self, module_path: &'static str) -> Self {
        self.module_path = module_path;
        self
    }

    /// Sets the file name.
    #[must_use]
    pub const fn file(mut self, file: &'static str) -> Self {
        self.file = file;
        self
    }

    /// Sets the line number.
    #[must_use]
    pub const fn line(mut self, line: u32) -> Self {
        self.line = line;
        self
    }

    /// Builds the record.
    #[must_use]
    pub const fn build(self) -> Record<'a> {
        Record {
            level: self.level,
            message: self.message,
            module_path: self.module_path,
            file: self.file,
            line: self.line,
        }
    }
}


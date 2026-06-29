//! Product-facing service lifecycle syscall vocabulary.
//!
//! This leaf owns service-domain semantics such as lifecycle control reports.
//! The raw syscall leaf remains only the transport spine.

#![no_std]

use {
    core::mem::size_of,
    reovim_uapi_syscall::{RawSyscall, SyscallArgs, SyscallError, SyscallNr},
};

/// Maximum service name bytes retained in a service-control report.
pub const SERVICE_REPORT_NAME_BYTES: usize = 32;
/// Maximum service target bytes retained in a service-control report.
pub const SERVICE_REPORT_TARGET_BYTES: usize = 64;

/// Service lifecycle control operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ServiceControlOp(usize);

impl ServiceControlOp {
    /// Request that rootd start a retained boot service target.
    pub const REQUEST: Self = Self(3);
    /// Stop a retained resident service by service name.
    pub const STOP: Self = Self(0);
    /// Start a retained payload service by service name.
    pub const START: Self = Self(1);
    /// Restart a retained payload service by service name.
    pub const RESTART: Self = Self(2);

    /// Builds a control op from a raw transport scalar.
    #[must_use]
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Returns the raw transport scalar.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }
}

/// Stable retained-service state code used by structured service reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ServiceStateCode(usize);

impl ServiceStateCode {
    /// Empty or absent service slot.
    pub const EMPTY: Self = Self(0);
    /// Service requested but not started.
    pub const REQUESTED: Self = Self(1);
    /// Service has a retained started process.
    pub const STARTED: Self = Self(2);
    /// Service process exited normally.
    pub const EXITED: Self = Self(3);
    /// Service was stopped by an operator-facing control.
    pub const STOPPED: Self = Self(4);
    /// Service failed during runtime or startup.
    pub const FAILED: Self = Self(5);

    /// Builds a service state code from a stable scalar.
    #[must_use]
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Returns the stable scalar.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }
}

/// Stable retained-service reason code used by structured service reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ServiceReasonCode(usize);

impl ServiceReasonCode {
    /// Empty or absent service reason.
    pub const NONE: Self = Self(0);
    /// Service was requested by a process.
    pub const REQUESTED: Self = Self(1);
    /// Service is running.
    pub const RUNNING: Self = Self(2);
    /// Service process exited normally.
    pub const PROCESS_EXITED: Self = Self(3);
    /// Service process failed.
    pub const PROCESS_FAILED: Self = Self(4);
    /// Service process was killed.
    pub const PROCESS_KILLED: Self = Self(5);
    /// Service was stopped by an operator-facing control.
    pub const OPERATOR_STOP: Self = Self(6);
    /// Service target could not be loaded.
    pub const EXEC_LOAD_ERROR: Self = Self(7);
    /// Service target halted the system.
    pub const HALT: Self = Self(8);
    /// Service failed during startup.
    pub const START_ERROR: Self = Self(9);

    /// Builds a service reason code from a stable scalar.
    #[must_use]
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Returns the stable scalar.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }
}

/// Structured result of a service lifecycle control operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ServiceControlReport {
    service_pid: usize,
    service_task_id: usize,
    state: usize,
    reason: usize,
    result: usize,
    exit_code: i32,
    name_len: usize,
    name_truncated: u8,
    target_len: usize,
    target_truncated: u8,
    name: [u8; SERVICE_REPORT_NAME_BYTES],
    target: [u8; SERVICE_REPORT_TARGET_BYTES],
}

impl ServiceControlReport {
    /// Empty report value used before a service-control request is issued.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            service_pid: 0,
            service_task_id: 0,
            state: ServiceStateCode::EMPTY.raw(),
            reason: ServiceReasonCode::NONE.raw(),
            result: ServiceControlResultCode::NONE.raw(),
            exit_code: 0,
            name_len: 0,
            name_truncated: 0,
            target_len: 0,
            target_truncated: 0,
            name: [0; SERVICE_REPORT_NAME_BYTES],
            target: [0; SERVICE_REPORT_TARGET_BYTES],
        }
    }

    /// Clears this report to the empty value.
    pub fn clear(&mut self) {
        *self = Self::empty();
    }

    /// Records the retained service name.
    pub fn set_name(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(self.name.len());
        self.name[..len].copy_from_slice(&bytes[..len]);
        self.name_len = len;
        self.name_truncated = u8::from(len < bytes.len());
    }

    /// Records the retained service target.
    pub fn set_target(&mut self, target: &str) {
        let bytes = target.as_bytes();
        let len = bytes.len().min(self.target.len());
        self.target[..len].copy_from_slice(&bytes[..len]);
        self.target_len = len;
        self.target_truncated = u8::from(len < bytes.len());
    }

    /// Records the service process id.
    pub fn set_service_pid(&mut self, pid: usize) {
        self.service_pid = pid;
    }

    /// Records the service task id.
    pub fn set_service_task_id(&mut self, task_id: usize) {
        self.service_task_id = task_id;
    }

    /// Records the retained service state.
    pub fn set_state(&mut self, state: ServiceStateCode) {
        self.state = state.raw();
    }

    /// Records the retained service reason.
    pub fn set_reason(&mut self, reason: ServiceReasonCode) {
        self.reason = reason.raw();
    }

    /// Records the payload/control result code.
    pub fn set_result(&mut self, result: ServiceControlResultCode) {
        self.result = result.raw();
    }

    /// Records the payload/control exit code.
    pub fn set_exit_code(&mut self, exit_code: i32) {
        self.exit_code = exit_code;
    }

    /// Returns the service process id.
    #[must_use]
    pub const fn service_pid(self) -> usize {
        self.service_pid
    }

    /// Returns the service task id.
    #[must_use]
    pub const fn service_task_id(self) -> usize {
        self.service_task_id
    }

    /// Returns the retained service state.
    #[must_use]
    pub const fn state(self) -> ServiceStateCode {
        ServiceStateCode::new(self.state)
    }

    /// Returns the retained service reason.
    #[must_use]
    pub const fn reason(self) -> ServiceReasonCode {
        ServiceReasonCode::new(self.reason)
    }

    /// Returns the payload/control result code.
    #[must_use]
    pub const fn result(self) -> ServiceControlResultCode {
        ServiceControlResultCode::new(self.result)
    }

    /// Returns the payload/control exit code.
    #[must_use]
    pub const fn exit_code(self) -> i32 {
        self.exit_code
    }

    /// Returns the retained service name bytes.
    #[must_use]
    pub fn name_bytes(&self) -> &[u8] {
        &self.name[..self.name_len]
    }

    /// Returns whether the service name was truncated.
    #[must_use]
    pub const fn name_truncated(self) -> bool {
        self.name_truncated != 0
    }

    /// Returns the retained service target bytes.
    #[must_use]
    pub fn target_bytes(&self) -> &[u8] {
        &self.target[..self.target_len]
    }

    /// Returns whether the service target was truncated.
    #[must_use]
    pub const fn target_truncated(self) -> bool {
        self.target_truncated != 0
    }
}

/// Stable result code for service-control operations that launch payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ServiceControlResultCode(usize);

impl ServiceControlResultCode {
    /// No payload launch result applies to this control operation.
    pub const NONE: Self = Self(0);
    /// Payload launch completed and returned ready.
    pub const PAYLOAD_READY: Self = Self(1);
    /// Payload published readiness and stayed resident.
    pub const PAYLOAD_RESIDENT: Self = Self(2);
    /// Payload launch was unavailable for the current profile/catalog.
    pub const PAYLOAD_NOT_CONFIGURED: Self = Self(3);
    /// Payload launch failed.
    pub const PAYLOAD_FAILED: Self = Self(4);
    /// Payload returned an explicit exit code; inspect `exit_code`.
    pub const PAYLOAD_EXIT_CODE: Self = Self(5);

    /// Builds a service-control result code from a stable scalar.
    #[must_use]
    pub const fn new(raw: usize) -> Self {
        Self(raw)
    }

    /// Returns the stable scalar.
    #[must_use]
    pub const fn raw(self) -> usize {
        self.0
    }
}

/// Product-facing service syscall error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ServiceError(i32);

impl ServiceError {
    /// The requested arguments are not valid for this operation.
    pub const INVALID_ARGUMENT: Self = Self(SyscallError::INVALID_ARGUMENT.code());
    /// The current profile or backend does not support the requested operation.
    pub const UNSUPPORTED: Self = Self(SyscallError::UNSUPPORTED.code());
    /// The requested service was not found.
    pub const NOT_FOUND: Self = Self(SyscallError::NOT_FOUND.code());
    /// The syscall requires an attached current process context.
    pub const NO_CURRENT_PROCESS: Self = Self(SyscallError::NO_CURRENT_PROCESS.code());
    /// The requested service process is protected from this operation.
    pub const PROTECTED_SERVICE: Self = Self(SyscallError::PROTECTED_PROCESS.code());
    /// The retained service process is missing from the process table.
    pub const PROCESS_NOT_FOUND: Self = Self(SyscallError::PROCESS_NOT_FOUND.code());
    /// The retained service is not currently stoppable.
    pub const NOT_STOPPABLE: Self = Self(SyscallError::BUSY.code());

    /// Builds a service error from a bridge/provider code.
    #[must_use]
    pub const fn new(code: i32) -> Self {
        Self(code)
    }

    /// Returns the diagnostic bridge/provider code.
    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }
}

/// Service control backed by the raw Reovim syscall transport.
#[derive(Debug, Clone, Copy)]
pub struct SyscallServiceControl {
    raw: RawSyscall,
}

impl SyscallServiceControl {
    /// Creates a service control adapter over the raw syscall transport.
    #[must_use]
    pub const fn new(raw: RawSyscall) -> Self {
        Self { raw }
    }

    /// Stops a retained resident service by service name.
    ///
    /// This is service lifecycle control, not raw process kill. The returned
    /// report carries service state/reason fields so `/bin` programs can format
    /// operator output without source-image-only helper operations.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError`] when there is no current process, the service is
    /// missing, protected, not stoppable, or the raw transport rejects the
    /// request.
    pub fn stop_report(self, name: &str) -> Result<ServiceControlReport, ServiceError> {
        self.control_report(ServiceControlOp::STOP, name)
    }

    /// Starts a retained payload service by service name.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError`] when there is no current process, the service is
    /// missing, protected, already started, has an unsupported target, or the
    /// raw transport rejects the request.
    pub fn start_report(self, name: &str) -> Result<ServiceControlReport, ServiceError> {
        self.control_report(ServiceControlOp::START, name)
    }

    /// Restarts a retained payload service by service name.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError`] when there is no current process, the service is
    /// missing, protected, has an unsupported target, a live instance cannot be
    /// stopped, or the raw transport rejects the request.
    pub fn restart_report(self, name: &str) -> Result<ServiceControlReport, ServiceError> {
        self.control_report(ServiceControlOp::RESTART, name)
    }

    /// Requests that rootd retain a service target for boot/session startup.
    ///
    /// This is a service-domain request, not process spawn and not a shell
    /// shortcut. The current system-kernel dispatcher supports the boot
    /// service request used by `/bin/init`: `shell -> /bin/sh`.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError`] when there is no current process, either string
    /// is empty, or the kernel does not support the requested service target.
    pub fn request(self, name: &str, target: &str) -> Result<(), ServiceError> {
        if name.is_empty() || target.is_empty() {
            return Err(ServiceError::INVALID_ARGUMENT);
        }
        self.raw
            .invoke(
                SyscallNr::SERVICE_CONTROL,
                SyscallArgs::new([
                    ServiceControlOp::REQUEST.raw(),
                    name.as_ptr() as usize,
                    name.len(),
                    target.as_ptr() as usize,
                    target.len(),
                    0,
                ]),
            )
            .decode()
            .map(|_| ())
            .map_err(service_error_from_syscall)
    }

    fn control_report(
        self,
        op: ServiceControlOp,
        name: &str,
    ) -> Result<ServiceControlReport, ServiceError> {
        if name.is_empty() {
            return Err(ServiceError::INVALID_ARGUMENT);
        }
        let mut report = ServiceControlReport::empty();
        let service_pid = self
            .raw
            .invoke(
                SyscallNr::SERVICE_CONTROL,
                SyscallArgs::new([
                    op.raw(),
                    name.as_ptr() as usize,
                    name.len(),
                    (&mut report as *mut ServiceControlReport) as usize,
                    size_of::<ServiceControlReport>(),
                    0,
                ]),
            )
            .decode()
            .map_err(service_error_from_syscall)?;
        if report.service_pid() == service_pid {
            Ok(report)
        } else {
            Err(ServiceError::INVALID_ARGUMENT)
        }
    }
}

fn service_error_from_syscall(error: SyscallError) -> ServiceError {
    ServiceError::new(error.code())
}

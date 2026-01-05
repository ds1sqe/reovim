use {reovim_core::event_bus::Event, std::sync::Arc};

/// Trait for implementing health checks
pub trait HealthCheck: Send + Sync + std::fmt::Debug + 'static {
    fn name(&self) -> &'static str;
    fn category(&self) -> &'static str; // "Core", "Plugin", "Language"
    fn run(&self) -> HealthCheckResult;
}

#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub status: CheckStatus,
    pub messages: Vec<String>,
    pub details: Option<String>,
}

impl HealthCheckResult {
    pub fn ok(msg: impl Into<String>) -> Self {
        Self {
            status: CheckStatus::Ok,
            messages: vec![msg.into()],
            details: None,
        }
    }

    pub fn warning(msg: impl Into<String>) -> Self {
        Self {
            status: CheckStatus::Warning,
            messages: vec![msg.into()],
            details: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            status: CheckStatus::Error,
            messages: vec![msg.into()],
            details: None,
        }
    }

    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,      // ✓ (green)
    Warning, // ⚠ (yellow)
    Error,   // ✗ (red)
}

impl CheckStatus {
    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Ok => "✓",
            Self::Warning => "⚠",
            Self::Error => "✗",
        }
    }
}

/// Metadata wrapper for check results
#[derive(Debug, Clone)]
pub struct HealthCheckResultWithMeta {
    pub name: String,
    pub category: String,
    pub result: HealthCheckResult,
}

/// Event for registering health checks
#[derive(Debug, Clone)]
pub struct RegisterHealthCheck {
    pub check: Arc<dyn HealthCheck>,
}

impl RegisterHealthCheck {
    pub fn new(check: Arc<dyn HealthCheck>) -> Self {
        Self { check }
    }
}

impl Event for RegisterHealthCheck {
    fn priority(&self) -> u32 {
        50 // Normal priority
    }
}

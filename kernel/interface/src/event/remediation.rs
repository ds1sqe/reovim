use reovim_foundation::contract::ContractId;

use crate::lifecycle::ReadinessOutcome;

use super::{
    Actionability, RemediationCode, RemediationRef, Severity,
    StartupEventDescriptor,
};

impl crate::event::RemediationRef {
    const fn new(
        catalog: u128,
        catalog_version: u64,
        code: RemediationCode,
    ) -> Self {
        Self { catalog: ContractId::new(catalog), catalog_version, code }
    }

    /// Returns the remediation catalog identity scalar.
    #[must_use]
    pub const fn catalog(self) -> u128 {
        self.catalog.value()
    }

    /// Returns the remediation catalog version.
    #[must_use]
    pub const fn catalog_version(self) -> u64 {
        self.catalog_version
    }

    /// Returns the closed remediation code.
    #[must_use]
    pub const fn code(self) -> RemediationCode {
        self.code
    }
}

pub(super) const fn classify(
    descriptor: &StartupEventDescriptor,
    readiness: ReadinessOutcome,
) -> (Severity, Actionability) {
    let identities = descriptor.identities();
    let severity = severity(readiness);
    let actionability = actionability(
        readiness,
        identities.remediation_catalog(),
        descriptor.remediation_catalog_version(),
    );
    (severity, actionability)
}

pub(super) const fn classification_matches(
    descriptor: &StartupEventDescriptor,
    readiness: ReadinessOutcome,
    actual_severity: Severity,
    actual_actionability: Actionability,
) -> bool {
    let (expected_severity, expected_actionability) =
        classify(descriptor, readiness);
    severity_code(actual_severity) == severity_code(expected_severity)
        && actionability_matches(actual_actionability, expected_actionability)
}

const fn severity(readiness: ReadinessOutcome) -> Severity {
    match readiness {
        ReadinessOutcome::Ready => Severity::Info,
        ReadinessOutcome::DiagnosticsDegraded => Severity::Warning,
        ReadinessOutcome::ActivationFailed(_) => Severity::Error,
    }
}

const fn actionability(
    readiness: ReadinessOutcome,
    catalog: u128,
    catalog_version: u64,
) -> Actionability {
    match readiness {
        ReadinessOutcome::Ready => Actionability::None,
        ReadinessOutcome::DiagnosticsDegraded => {
            Actionability::Operator(RemediationRef::new(
                catalog,
                catalog_version,
                RemediationCode::DiagnosticPathDegraded,
            ))
        }
        ReadinessOutcome::ActivationFailed(_) => {
            Actionability::Operator(RemediationRef::new(
                catalog,
                catalog_version,
                RemediationCode::ActivationFailed,
            ))
        }
    }
}

const fn severity_code(value: Severity) -> u8 {
    match value {
        Severity::Info => 0,
        Severity::Warning => 1,
        Severity::Error => 2,
    }
}

const fn actionability_matches(
    actual: Actionability,
    expected: Actionability,
) -> bool {
    match (actual, expected) {
        (Actionability::None, Actionability::None) => true,
        (Actionability::Operator(left), Actionability::Operator(right)) => {
            left.catalog() == right.catalog()
                && left.catalog_version() == right.catalog_version()
                && remediation_code(left.code())
                    == remediation_code(right.code())
        }
        (Actionability::None, Actionability::Operator(_))
        | (Actionability::Operator(_), Actionability::None) => false,
    }
}

const fn remediation_code(value: RemediationCode) -> u8 {
    match value {
        RemediationCode::DiagnosticPathDegraded => 0,
        RemediationCode::ActivationFailed => 1,
    }
}

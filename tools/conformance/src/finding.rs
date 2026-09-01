//! Deterministic, bounded conformance findings.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub stage: &'static str,
    pub code: &'static str,
    pub path: Option<String>,
    pub message: String,
}

impl Finding {
    #[must_use]
    pub fn new(
        stage: &'static str,
        code: &'static str,
        path: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self { stage, code, path, message: message.into() }
    }
}

#[derive(Debug)]
pub struct Findings {
    limit: usize,
    values: Vec<Finding>,
    saturated: bool,
}

impl Findings {
    #[must_use]
    pub const fn new(limit: usize) -> Self {
        Self { limit, values: Vec::new(), saturated: false }
    }

    pub fn push(&mut self, finding: Finding) {
        if self.saturated {
            return;
        }
        if self.values.len() + 1 >= self.limit {
            self.values.push(Finding::new(
                "limits",
                "FINDING_LIMIT",
                None,
                format!("finding count reached committed limit {}", self.limit),
            ));
            self.saturated = true;
        } else {
            self.values.push(finding);
        }
    }

    pub fn extend(&mut self, values: impl IntoIterator<Item = Finding>) {
        for value in values {
            self.push(value);
        }
    }

    #[must_use]
    pub fn finish(mut self) -> Vec<Finding> {
        self.values.sort();
        self.values.dedup();
        self.values
    }
}

//! Requests the CLI refuses for a known reason, so the error an agent reads
//! carries a stable code and `details.reason` instead of a generic failure.

use serde_json::Value;

/// How the CLI reports a refusal: its error code and exit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalKind {
    /// The caller has to change the request.
    InvalidInput,
    /// Nothing matched the request.
    NotFound,
    /// The target exists but cannot serve this yet; retrying later can work.
    Unavailable,
    /// The target refuses in its current state.
    Conflict,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Refusal {
    pub kind: RefusalKind,
    /// Stable snake_case reason, reported as `details.reason`.
    pub reason: &'static str,
    pub message: String,
    pub details: Option<Value>,
}

impl Refusal {
    pub fn new(kind: RefusalKind, reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            reason,
            message: message.into(),
            details: None,
        }
    }

    #[must_use]
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }
}

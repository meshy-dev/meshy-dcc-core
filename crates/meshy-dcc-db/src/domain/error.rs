//! Single error surface for `meshy-dcc-db` (extend variants as infra grows).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    /// Invalid configuration (e.g. retention window).
    #[error("invalid database configuration: {0}")]
    InvalidConfig(String),
    /// Backend-specific failure (wrapped for `From` impls in infra).
    #[error("database backend error: {0}")]
    Backend(String),
}

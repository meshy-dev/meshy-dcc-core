//! Server configuration compatibility facade.
//!
//! The value-type surface, including `McpHttpConfig`, lives in
//! `meshy-dcc-http-types::config` (issue #852) so Rust tooling can validate and
//! round-trip HTTP configuration without depending on axum / tokio / reqwest /
//! pyo3. This module preserves the historical `meshy_dcc_http::config::*` path.

#[allow(deprecated)]
pub use meshy_dcc_http_types::config::GatewayConfig;
pub use meshy_dcc_http_types::config::{
    FeatureFlags, GatewaySettings, InstanceConfig, JobConfig, JobRecoveryPolicy, McpHttpConfig,
    QueueConfig, ServerConfig, ServerSpawnMode, SessionConfig, TelemetryConfig, WorkflowConfig,
};

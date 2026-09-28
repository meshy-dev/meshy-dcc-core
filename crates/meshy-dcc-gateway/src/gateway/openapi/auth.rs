//! Auth configuration compatibility facade for OpenAPI-to-MCP forwarding.
//!
//! `AuthKind` and `AuthConfig` live in `meshy-dcc-gateway-core::openapi`
//! (issue #845). The gateway runtime re-exports them here so existing
//! `meshy_dcc_gateway::gateway::openapi::auth::*` call sites keep compiling.

pub use meshy_dcc_gateway_core::openapi::{AuthConfig, AuthKind};

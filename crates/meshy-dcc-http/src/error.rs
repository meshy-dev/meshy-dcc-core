//! Error types for the MCP HTTP server.
//!
//! # Relocation notice (issue #852)
//!
//! [`HttpError`] / [`HttpResult`] were migrated to the dedicated
//! [`meshy_dcc_http_types::error`] module as part of the `meshy-dcc-http`
//! Clean-Architecture split. This module now re-exports them so existing
//! call sites (`crate::error::HttpError`, `crate::error::HttpResult`)
//! keep compiling under their historical paths; the canonical
//! definitions live in `meshy-dcc-http-types`.
//!
//! New code should depend on `meshy-dcc-http-types` directly when it only
//! needs the error surface.

pub use meshy_dcc_http_types::error::{HttpError, HttpResult};

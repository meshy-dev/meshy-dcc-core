//! Capability record — the unit of the gateway index.
//!
//! # Relocation notice (issue #845)
//!
//! All types and helpers in this module were migrated to the dedicated
//! [`meshy_dcc_gateway_core::capability`] module as part of the
//! `meshy-dcc-gateway` Clean-Architecture split (issue #845). This module
//! now re-exports them so existing call sites
//! (`crate::gateway::capability::record::*`,
//! `crate::gateway::capability::CapabilityRecord`, …) keep compiling
//! under their historical paths; the domain-level definitions live in
//! `meshy-dcc-gateway-core`.
//!
//! New code should depend on `meshy-dcc-gateway-core` directly when it
//! only needs the wire types (`POST /v1/search` deserialisation, etc.).
//!
//! See `meshy_dcc_gateway_core::capability` for the original documentation.

pub use meshy_dcc_gateway_core::capability::{
    CapabilityAnnotations, CapabilityGroupInfo, CapabilityMetadata, CapabilityRecord,
    SCHEMA_AVAILABLE, parse_slug, tool_slug,
};

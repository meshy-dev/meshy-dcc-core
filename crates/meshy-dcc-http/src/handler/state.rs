//! Shared [`AppState`] owned by the server.
//!
//! The lower-level runtime state (tool registry, sessions, jobs, executor,
//! in-flight requests, and cache generation) lives in
//! [`meshy_dcc_http_server::ServerState`].  This crate-level state keeps the
//! application-layer objects that belong to `meshy-dcc-http`: bridge,
//! resources, prompts, and readiness.

use std::sync::Arc;

use crate::{
    bridge_registry::BridgeRegistry, prompts::PromptRegistry, resources::ResourceRegistry,
};
use meshy_dcc_http_server::ServerState;
use meshy_dcc_skill_rest::{ReadinessProbe, StaticReadiness};

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Runtime server state extracted into `meshy-dcc-http-server`.
    pub server: ServerState,
    /// Python / host bridge registry owned by the embedding HTTP crate.
    pub bridge_registry: BridgeRegistry,
    /// MCP Resources primitive registry.
    pub resources: ResourceRegistry,
    /// MCP Prompts primitive registry.
    pub prompts: PromptRegistry,
    /// Shared readiness probe gating DCC-touching `tools/call` dispatches.
    pub readiness: Arc<dyn ReadinessProbe>,
}

impl AppState {
    /// Build the default [`ReadinessProbe`] — a `StaticReadiness`
    /// locked to the fully-ready state (issue #714).
    #[must_use]
    pub fn default_readiness() -> Arc<dyn ReadinessProbe> {
        Arc::new(StaticReadiness::fully_ready())
    }
}

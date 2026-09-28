//! Compatibility paths for domain types owned by `meshy-dcc-gateway-admin`.

pub mod agent_context {
    pub use meshy_dcc_gateway_admin::domain::agent_context::*;
}

pub mod trace {
    pub use meshy_dcc_gateway_admin::domain::trace::*;
}

pub use trace::*;

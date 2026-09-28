//! Compatibility facade for the pure capability builder.
//!
//! The implementation lives in `meshy-dcc-gateway-core`; this module keeps
//! historical gateway paths stable for downstream callers.

pub use meshy_dcc_gateway_core::capability::builder::{
    BuildInput, BuildOutcome, backend_job_status_tool, build_records_from_backend,
    is_backend_job_tool,
};

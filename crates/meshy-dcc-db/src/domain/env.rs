//! Canonical environment variable names for on-disk Meshy DCC state.

/// Shared file-backed service registry directory (`MESHY_DCC_REGISTRY_DIR`).
pub const ENV_REGISTRY_DIR: &str = "MESHY_DCC_REGISTRY_DIR";

/// Explicit override for the gateway admin SQLite file (traces, audits, custom skill paths).
pub const ENV_GATEWAY_ADMIN_DB: &str = "MESHY_DCC_GATEWAY_ADMIN_DB";

/// Default filename inside the persistent application-state directory.
pub const GATEWAY_ADMIN_SQLITE_FILENAME: &str = "gateway_admin.sqlite";

/// Rolling file log directory for `meshy_dcc_logging` (`MESHY_DCC_LOG_DIR`).
pub const ENV_MESHY_DCC_LOG_DIR: &str = "MESHY_DCC_LOG_DIR";

/// Repeat threshold at which a repeated escape-hatch script becomes a
/// skill-promotion candidate (#2297-A3). Unparsable values fall back to the
/// built-in default of 3.
pub const ENV_SCRIPT_PROMOTION_MIN_REPEATS: &str = "MESHY_DCC_SCRIPT_PROMOTION_MIN_REPEATS";

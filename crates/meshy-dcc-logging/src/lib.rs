//! meshy-dcc-logging: tracing-subscriber bootstrap + rolling file-logging layer.
//!
//! Extracted from the former `meshy-dcc-utils::file_logging` /
//! `meshy-dcc-utils::log_config` modules so that pure data crates no longer
//! transitively depend on `tracing-appender`, `tracing-subscriber`, etc.
//! See [issue #496](https://github.com/meshy-dev/meshy-dcc-core/issues/496).

pub mod config;
pub mod constants;
pub mod file_logging;

pub use config::{
    BoxedLayer, FileLayerInstallError, init_logging, install_file_layer_boxed, reload_handle,
};
pub use file_logging::{
    FileLoggingConfig, FileLoggingError, RollingFileWriter, RotationPolicy, flush_logs,
    init_file_logging, shutdown_file_logging,
};

/// Re-export the Python entry points so callers can use
/// `meshy_dcc_logging::python::py_init_file_logging` etc., mirroring the
/// previous `meshy_dcc_utils::file_logging::python` path.
#[cfg(feature = "python-bindings")]
pub mod python {
    pub use crate::file_logging::python::{
        PyFileLoggingConfig, py_default_settings, py_flush_logs, py_init_file_logging,
        py_shutdown_file_logging,
    };
}

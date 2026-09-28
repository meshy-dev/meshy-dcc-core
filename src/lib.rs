//! meshy-dcc-core: Python bindings entry point.
//!
//! This root crate serves as the `meshy_dcc_core._core` Python extension module.
//! All logic lives in workspace sub-crates; this crate only re-exports and registers.

#[cfg(feature = "python-bindings")]
mod scene_digest_custody;
#[cfg(feature = "python-bindings")]
mod skill_http;

#[cfg(feature = "python-bindings")]
use pyo3::prelude::*;

// Re-export sub-crates for Rust consumers
pub use meshy_dcc_actions as actions;
pub use meshy_dcc_artefact as artefact;
pub use meshy_dcc_capture as capture;
pub use meshy_dcc_host as host;
pub use meshy_dcc_http as http;
pub use meshy_dcc_models as models;
pub use meshy_dcc_naming as naming;
pub use meshy_dcc_paths as paths;
pub use meshy_dcc_process as process;
pub use meshy_dcc_protocols as protocols;
pub use meshy_dcc_sandbox as sandbox;
pub use meshy_dcc_shm as shm;
pub use meshy_dcc_skills as skills;
pub use meshy_dcc_telemetry as telemetry;
pub use meshy_dcc_transport as transport;
pub use meshy_dcc_usd as usd;
pub use meshy_dcc_wire as wire;

#[cfg(feature = "workflow")]
pub use meshy_dcc_workflow as workflow;

#[cfg(feature = "scheduler")]
pub use meshy_dcc_scheduler as scheduler;

// ── Helper macros (defined before use for readability) ──

/// Batch-register `#[pyfunction]`s on a module.
#[cfg(feature = "python-bindings")]
macro_rules! add_functions {
    ($m:expr, $($func:path),+ $(,)?) => {
        $( $m.add_function(wrap_pyfunction!($func, $m)?)?; )+
    };
}

/// Batch-register `#[pyclass]` types on a module.
#[cfg(feature = "python-bindings")]
macro_rules! add_classes {
    ($m:expr, $($cls:path),+ $(,)?) => {
        $( $m.add_class::<$cls>()?; )+
    };
}

/// Batch-register string constants on a module.
#[cfg(feature = "python-bindings")]
macro_rules! add_constants {
    ($m:expr, $($name:literal => $val:expr),+ $(,)?) => {
        $( $m.add($name, $val)?; )+
    };
}

// ── Module initialization ──

/// Python module initialization — `meshy_dcc_core._core`
#[cfg(feature = "python-bindings")]
#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_logging::init_logging();

    register_models(m)?;
    register_actions(m)?;
    register_protocols(m)?;
    register_skills(m)?;
    register_transport(m)?;
    register_process(m)?;
    register_telemetry(m)?;
    register_sandbox(m)?;
    register_shm(m)?;
    register_capture(m)?;
    register_usd(m)?;
    register_utils(m)?;
    register_http(m)?;
    register_artefact(m)?;
    register_host(m)?;
    register_wire(m)?;
    register_naming(m)?;
    scene_digest_custody::register(m)?;
    register_constants(m)?;
    #[cfg(feature = "workflow")]
    register_workflow(m)?;
    #[cfg(feature = "scheduler")]
    register_scheduler(m)?;

    // ── Metadata ──
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("__author__", env!("CARGO_PKG_AUTHORS"))?;

    Ok(())
}

// ── Type-stub generation (opt-in, PoC) ──
//
// Enabled by the `stub-gen` feature — powers the `stub_gen` binary target
// which regenerates `python/meshy_dcc_core/_core.pyi` from annotated Rust code.
#[cfg(feature = "stub-gen")]
pyo3_stub_gen::define_stub_info_gatherer!(stub_info);

#[cfg(feature = "python-bindings")]
fn register_models(m: &Bound<'_, PyModule>) -> PyResult<()> {
    add_classes!(
        m,
        meshy_dcc_models::ActionResultModel,
        meshy_dcc_models::SerializeFormat,
        meshy_dcc_models::SkillGroup,
        meshy_dcc_models::SkillMetadata,
        meshy_dcc_models::SkillRuntimeDescriptor,
        meshy_dcc_models::SkillRuntimeKind,
        meshy_dcc_models::SkillRuntimeReport,
        meshy_dcc_models::SkillRuntimeState,
        meshy_dcc_models::SkillRuntimeSummary,
        meshy_dcc_models::SkillScope,
    );
    add_functions!(
        m,
        meshy_dcc_models::py_success_result,
        meshy_dcc_models::py_error_result,
        meshy_dcc_models::py_from_exception,
        meshy_dcc_models::py_validate_action_result,
        meshy_dcc_models::py_serialize_result,
        meshy_dcc_models::py_deserialize_result,
    );
    meshy_dcc_models::register_schema_validation(m)?;
    Ok(())
}

#[cfg(feature = "python-bindings")]
fn register_actions(m: &Bound<'_, PyModule>) -> PyResult<()> {
    add_classes!(
        m,
        meshy_dcc_actions::ToolRegistry,
        meshy_dcc_actions::EventBus,
        meshy_dcc_actions::SemVer,
        meshy_dcc_actions::versioned::PyVersionConstraint,
        meshy_dcc_actions::VersionedRegistry,
    );
    meshy_dcc_actions::python::register_classes(m)?;
    meshy_dcc_actions::pipeline::python::register_classes(m)?;
    Ok(())
}

#[cfg(feature = "python-bindings")]
fn register_protocols(m: &Bound<'_, PyModule>) -> PyResult<()> {
    add_classes!(
        m,
        // MCP protocol types
        meshy_dcc_protocols::ToolDefinition,
        meshy_dcc_protocols::ToolAnnotations,
        meshy_dcc_protocols::ResourceAnnotations,
        meshy_dcc_protocols::ResourceDefinition,
        meshy_dcc_protocols::ResourceTemplateDefinition,
        meshy_dcc_protocols::PromptArgument,
        meshy_dcc_protocols::PromptDefinition,
        // DCC adapter types
        meshy_dcc_protocols::PyDccInfo,
        meshy_dcc_protocols::PyScriptResult,
        meshy_dcc_protocols::PyScriptLanguage,
        meshy_dcc_protocols::PySceneInfo,
        meshy_dcc_protocols::PySceneStatistics,
        meshy_dcc_protocols::PyDccCapabilities,
        meshy_dcc_protocols::PyDccError,
        meshy_dcc_protocols::PyDccErrorCode,
        meshy_dcc_protocols::PyCaptureResult,
        // Cross-DCC protocol data models
        meshy_dcc_protocols::PyObjectTransform,
        meshy_dcc_protocols::PyBoundingBox,
        meshy_dcc_protocols::PySceneObject,
        meshy_dcc_protocols::PySceneNode,
        meshy_dcc_protocols::PyFrameRange,
        meshy_dcc_protocols::PyRenderOutput,
    );
    Ok(())
}

#[cfg(feature = "python-bindings")]
fn register_skills(m: &Bound<'_, PyModule>) -> PyResult<()> {
    add_classes!(
        m,
        meshy_dcc_skills::SkillScanner,
        meshy_dcc_skills::PySkillWatcher,
        meshy_dcc_skills::SkillCatalog,
        meshy_dcc_skills::SkillSummary,
        meshy_dcc_skills::PySkillValidationIssue,
        meshy_dcc_skills::PySkillValidationReport,
        meshy_dcc_skills::PyGuiExecutableHint,
        meshy_dcc_models::ToolDeclaration,
    );
    add_functions!(
        m,
        meshy_dcc_skills::py_parse_skill_md,
        meshy_dcc_skills::py_scan_skill_paths,
        meshy_dcc_skills::py_resolve_dependencies,
        meshy_dcc_skills::py_validate_dependencies,
        meshy_dcc_skills::py_expand_transitive_dependencies,
        meshy_dcc_skills::py_scan_and_load,
        meshy_dcc_skills::py_scan_and_load_lenient,
        meshy_dcc_skills::py_scan_and_load_strict,
        meshy_dcc_skills::py_scan_and_load_user,
        meshy_dcc_skills::py_scan_and_load_team,
        meshy_dcc_skills::py_scan_and_load_user_lenient,
        meshy_dcc_skills::py_scan_and_load_team_lenient,
        meshy_dcc_skills::py_validate_skill,
        meshy_dcc_skills::py_is_gui_executable,
        meshy_dcc_skills::py_correct_python_executable,
    );
    Ok(())
}

#[cfg(feature = "python-bindings")]
fn register_transport(m: &Bound<'_, PyModule>) -> PyResult<()> {
    add_classes!(
        m,
        meshy_dcc_transport::PyServiceEntry,
        meshy_dcc_transport::PyServiceStatus,
        meshy_dcc_transport::PyTransportAddress,
        meshy_dcc_transport::PyTransportScheme,
        meshy_dcc_transport::PyDccLinkFrame,
        meshy_dcc_transport::PyIpcChannelAdapter,
        meshy_dcc_transport::PyGracefulIpcChannelAdapter,
        meshy_dcc_transport::PySocketServerAdapter,
    );
    Ok(())
}

#[cfg(feature = "python-bindings")]
fn register_utils(m: &Bound<'_, PyModule>) -> PyResult<()> {
    add_functions!(
        m,
        meshy_dcc_paths::py_get_platform_dir,
        meshy_dcc_paths::py_get_config_dir,
        meshy_dcc_paths::py_get_data_dir,
        meshy_dcc_paths::py_get_log_dir,
        meshy_dcc_paths::py_get_tools_dir,
        meshy_dcc_skills::paths::py_get_skills_dir,
        meshy_dcc_skills::paths::py_get_local_skills_dir,
        meshy_dcc_skills::paths::py_get_skill_paths_from_env,
        meshy_dcc_skills::paths::py_get_app_skill_paths_from_env,
        meshy_dcc_skills::paths::py_get_user_skill_paths_from_env,
        meshy_dcc_skills::paths::py_get_team_skill_paths_from_env,
        meshy_dcc_skills::paths::py_get_app_user_skill_paths_from_env,
        meshy_dcc_skills::paths::py_get_app_team_skill_paths_from_env,
        meshy_dcc_skills::paths::py_get_user_skills_dir,
        meshy_dcc_skills::paths::py_get_team_skills_dir,
        meshy_dcc_skills::paths::py_copy_skill_to_user_dir,
        meshy_dcc_skills::paths::py_copy_skill_to_team_dir,
        meshy_dcc_skills::feedback::py_record_skill_feedback,
        meshy_dcc_skills::feedback::py_get_skill_feedback,
        meshy_dcc_skills::versioning::py_get_skill_version_manifest,
        meshy_dcc_pybridge::python::py_unwrap_value,
        meshy_dcc_pybridge::python::py_unwrap_parameters,
        meshy_dcc_pybridge::python::py_wrap_value,
        meshy_dcc_logging::python::py_init_file_logging,
        meshy_dcc_logging::python::py_shutdown_file_logging,
        meshy_dcc_logging::python::py_flush_logs,
        meshy_dcc_logging::python::py_default_settings,
        meshy_dcc_pybridge::python::json_dumps,
        meshy_dcc_pybridge::python::json_loads,
        meshy_dcc_pybridge::python::yaml_loads,
        meshy_dcc_pybridge::python::yaml_dumps,
        crate::skill_http::py_skill_http_request,
    );
    add_classes!(
        m,
        meshy_dcc_pybridge::type_wrappers::BooleanWrapper,
        meshy_dcc_pybridge::type_wrappers::IntWrapper,
        meshy_dcc_pybridge::type_wrappers::FloatWrapper,
        meshy_dcc_pybridge::type_wrappers::StringWrapper,
        meshy_dcc_skills::feedback::SkillFeedback,
        meshy_dcc_skills::versioning::SkillVersionManifest,
        meshy_dcc_skills::versioning::SkillVersionEntry,
        meshy_dcc_logging::python::PyFileLoggingConfig,
    );
    Ok(())
}

#[cfg(feature = "python-bindings")]
fn register_process(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_process::python::register_classes(m)
}

#[cfg(feature = "python-bindings")]
fn register_telemetry(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_telemetry::python::register_classes(m)
}

#[cfg(feature = "python-bindings")]
fn register_sandbox(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_sandbox::python::register_classes(m)
}

#[cfg(feature = "python-bindings")]
fn register_shm(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_shm::python::register_classes(m)
}

#[cfg(feature = "python-bindings")]
fn register_capture(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_capture::python::register_classes(m)
}

#[cfg(feature = "python-bindings")]
fn register_usd(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_usd::python::register_classes(m)
}

#[cfg(feature = "python-bindings")]
fn register_http(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_http_py::register_classes(m)
}

#[cfg(feature = "python-bindings")]
fn register_naming(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_naming::python::register(m)
}

#[cfg(feature = "python-bindings")]
fn register_artefact(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_artefact::python::register_classes(m)
}

#[cfg(feature = "python-bindings")]
fn register_host(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_host::python::register(m)
}

#[cfg(feature = "python-bindings")]
fn register_wire(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_wire::python::register(m)
}

#[cfg(all(feature = "python-bindings", feature = "workflow"))]
fn register_workflow(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_workflow::python::register_classes(m)
}

#[cfg(all(feature = "python-bindings", feature = "scheduler"))]
fn register_scheduler(m: &Bound<'_, PyModule>) -> PyResult<()> {
    meshy_dcc_scheduler::python::register_classes(m)
}

#[cfg(feature = "python-bindings")]
fn register_constants(m: &Bound<'_, PyModule>) -> PyResult<()> {
    use meshy_dcc_logging::constants as log_constants;
    use meshy_dcc_skills::constants as skill_constants;
    add_constants!(
        m,
        "APP_NAME"           => meshy_dcc_paths::APP_NAME,
        "APP_AUTHOR"         => meshy_dcc_paths::APP_AUTHOR,
        "DEFAULT_DCC"        => meshy_dcc_naming::DEFAULT_DCC,
        "DEFAULT_VERSION"    => meshy_dcc_naming::DEFAULT_VERSION,
        "DEFAULT_MIME_TYPE"  => meshy_dcc_protocols::DEFAULT_MIME_TYPE,
        "SKILL_METADATA_FILE"=> skill_constants::SKILL_METADATA_FILE,
        "SKILL_SCRIPTS_DIR"  => skill_constants::SKILL_SCRIPTS_DIR,
        "SKILL_METADATA_DIR" => skill_constants::SKILL_METADATA_DIR,
        "ENV_SKILL_PATHS"    => skill_constants::ENV_SKILL_PATHS,
        "ENV_USER_SKILL_PATHS" => skill_constants::ENV_USER_SKILL_PATHS,
        "ENV_TEAM_SKILL_PATHS" => skill_constants::ENV_TEAM_SKILL_PATHS,
        "ENV_DISABLE_ACCUMULATED_SKILLS" => skill_constants::ENV_DISABLE_ACCUMULATED_SKILLS,
        "ENV_DISABLE_DEFAULT_SKILL_PATHS" => skill_constants::ENV_DISABLE_DEFAULT_SKILL_PATHS,
        "ENV_LOG_LEVEL"      => log_constants::ENV_LOG_LEVEL,
        "DEFAULT_LOG_LEVEL"  => log_constants::DEFAULT_LOG_LEVEL,
        "ENV_LOG_FILE"       => log_constants::ENV_LOG_FILE,
        "ENV_LOG_DIR"        => log_constants::ENV_LOG_DIR,
        "ENV_LOG_MAX_SIZE"   => log_constants::ENV_LOG_MAX_SIZE,
        "ENV_LOG_MAX_FILES"  => log_constants::ENV_LOG_MAX_FILES,
        "ENV_LOG_ROTATION"   => log_constants::ENV_LOG_ROTATION,
        "ENV_LOG_FILE_PREFIX"=> log_constants::ENV_LOG_FILE_PREFIX,
        "DEFAULT_LOG_FILE_PREFIX" => log_constants::DEFAULT_LOG_FILE_PREFIX,
        "DEFAULT_LOG_ROTATION"   => log_constants::DEFAULT_LOG_ROTATION,
    );
    m.add("DEFAULT_LOG_MAX_SIZE", log_constants::DEFAULT_LOG_MAX_SIZE)?;
    m.add(
        "DEFAULT_LOG_MAX_FILES",
        log_constants::DEFAULT_LOG_MAX_FILES,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_workspace_crates_accessible() {
        let _ = meshy_dcc_models::ActionResultModelData::default();
        let reg = meshy_dcc_actions::ToolRegistry::new();
        assert!(reg.is_empty());
        let monitor = meshy_dcc_process::ProcessMonitor::new();
        assert_eq!(monitor.tracked_count(), 0);
        // Verify telemetry types are accessible
        let cfg = meshy_dcc_telemetry::TelemetryConfig::builder("test").build();
        assert_eq!(cfg.service_name, "test");
        // Verify sandbox types are accessible
        let policy = meshy_dcc_sandbox::SandboxPolicy::default();
        assert!(policy.check_action("anything").is_ok());
        // Verify shm types are accessible
        let buf = meshy_dcc_shm::SharedBuffer::create(256).unwrap();
        assert_eq!(buf.capacity(), 256);
        // Verify capture types are accessible
        let capturer = meshy_dcc_capture::Capturer::new_auto();
        let (count, _, _) = capturer.stats().snapshot();
        assert_eq!(count, 0);
        // Verify USD types are accessible
        let mut stage = meshy_dcc_usd::UsdStage::new("test");
        stage.define_prim(meshy_dcc_usd::SdfPath::new("/World").unwrap(), "Xform");
        assert!(stage.has_prim("/World"));
        let m = stage.metrics();
        assert_eq!(m.xform_count, 1);
    }
}

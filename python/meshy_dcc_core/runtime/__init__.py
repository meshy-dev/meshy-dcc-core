"""Public runtime configuration and lightweight fallback contracts."""

from __future__ import annotations

from meshy_dcc_core._lite_fallback import DccCapabilities
from meshy_dcc_core._lite_fallback import GuiExecutableHint
from meshy_dcc_core._lite_fallback import PyPumpedDispatcher
from meshy_dcc_core._lite_fallback import ReadinessProbe
from meshy_dcc_core._lite_fallback import correct_python_executable
from meshy_dcc_core._lite_fallback import is_gui_executable
from meshy_dcc_core._lite_fallback import parse_skill_md
from meshy_dcc_core._lite_fallback import scan_and_load_strict
from meshy_dcc_core._runtime.config_bridge import resolve_mcp_http_config_class
from meshy_dcc_core.runtime.capture_contract import CaptureReturnMode
from meshy_dcc_core.runtime.capture_contract import CaptureTargetSpec
from meshy_dcc_core.runtime.capture_contract import build_capture_response
from meshy_dcc_core.runtime.capture_contract import capture_response
from meshy_dcc_core.runtime.scene_digest import SCENE_DIGEST_SCHEMA_VERSION
from meshy_dcc_core.runtime.scene_digest import SceneDigestError
from meshy_dcc_core.runtime.scene_digest import SceneDigestExecution
from meshy_dcc_core.runtime.scene_digest import SceneDigestExecutionError
from meshy_dcc_core.runtime.scene_digest import SceneDigestSnapshot
from meshy_dcc_core.runtime.scene_digest import StateDigestProvider

McpHttpConfig = resolve_mcp_http_config_class()

__all__ = [
    "SCENE_DIGEST_SCHEMA_VERSION",
    "CaptureReturnMode",
    "CaptureTargetSpec",
    "DccCapabilities",
    "GuiExecutableHint",
    "McpHttpConfig",
    "PyPumpedDispatcher",
    "ReadinessProbe",
    "SceneDigestError",
    "SceneDigestExecution",
    "SceneDigestExecutionError",
    "SceneDigestSnapshot",
    "StateDigestProvider",
    "build_capture_response",
    "capture_response",
    "correct_python_executable",
    "is_gui_executable",
    "parse_skill_md",
    "scan_and_load_strict",
]

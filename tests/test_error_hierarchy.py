"""Tests for the shared Python exception hierarchy."""

import json
import os
from pathlib import Path
import subprocess
import sys

from meshy_dcc_core import MeshyDccError
from meshy_dcc_core.asset_import import AssetImportValidationError
from meshy_dcc_core.asset_sync import AssetSyncConflictError
from meshy_dcc_core.asset_sync import AssetSyncValidationError
from meshy_dcc_core.auth import TokenValidationError
from meshy_dcc_core.bridge import BridgeConnectionError
from meshy_dcc_core.bridge import BridgeError
from meshy_dcc_core.bridge import BridgeRpcError
from meshy_dcc_core.bridge import BridgeTimeoutError
from meshy_dcc_core.cancellation import MeshyDccCancelledError
from meshy_dcc_core.cua_cli import CuaCliError
from meshy_dcc_core.guardrails import DccGuardrailError
from meshy_dcc_core.host._fallback import DispatchError
from meshy_dcc_core.lifecycle_hooks import HookDeny
from meshy_dcc_core.script_execution import ScriptExecutionSerializationError
from meshy_dcc_core.skills_helper import HttpStatusError
from meshy_dcc_core.skills_helper import SkillCodecError
from meshy_dcc_core.skills_helper import SkillFileError
from meshy_dcc_core.skills_helper import SkillHelperError
from meshy_dcc_core.skills_helper import SkillHttpError
from meshy_dcc_core.vector_embedder import EmbedderError

REPO_ROOT = Path(__file__).resolve().parent.parent


def test_error_module_does_not_load_core_in_fresh_process() -> None:
    env = os.environ.copy()
    env["PYTHONPATH"] = str(REPO_ROOT / "python")
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            "import json, sys; from meshy_dcc_core import MeshyDccError; "
            'print(json.dumps({"core_loaded": "meshy_dcc_core._core" in sys.modules, '
            '"name": MeshyDccError.__name__}))',
        ],
        cwd=str(REPO_ROOT),
        env=env,
        check=True,
        capture_output=True,
        text=True,
    )
    assert json.loads(result.stdout) == {"core_loaded": False, "name": "MeshyDccError"}


def test_public_errors_share_one_root() -> None:
    error_types = (
        AssetImportValidationError,
        AssetSyncConflictError,
        AssetSyncValidationError,
        BridgeConnectionError,
        BridgeError,
        BridgeRpcError,
        BridgeTimeoutError,
        MeshyDccCancelledError,
        CuaCliError,
        DccGuardrailError,
        DispatchError,
        EmbedderError,
        HookDeny,
        HttpStatusError,
        ScriptExecutionSerializationError,
        SkillCodecError,
        SkillFileError,
        SkillHelperError,
        SkillHttpError,
        TokenValidationError,
    )
    assert all(issubclass(error_type, MeshyDccError) for error_type in error_types)


def test_builtin_exception_contracts_are_preserved() -> None:
    assert issubclass(AssetImportValidationError, ValueError)
    assert issubclass(AssetSyncValidationError, ValueError)
    assert issubclass(AssetSyncConflictError, RuntimeError)
    assert issubclass(CuaCliError, RuntimeError)
    assert issubclass(DccGuardrailError, RuntimeError)
    assert issubclass(DispatchError, RuntimeError)
    assert issubclass(EmbedderError, RuntimeError)
    assert issubclass(ScriptExecutionSerializationError, TypeError)

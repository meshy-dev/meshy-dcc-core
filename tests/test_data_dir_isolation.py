"""``MESHY_DCC_DATA_DIR`` keeps everything an adapter server writes out of $HOME.

Unset, the documented defaults (``~/.meshy-dcc``, the platform log dir) stay.
"""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys
import textwrap

import pytest

from meshy_dcc_core import checkpoint
from meshy_dcc_core import host_errors
from meshy_dcc_core import loaded_state_store
from meshy_dcc_core import script_materialization
from meshy_dcc_core import vector_embedder
from meshy_dcc_core._runtime import skill_paths


@pytest.fixture
def data_dir(tmp_path, monkeypatch):
    for name in (
        "MESHY_DCC_LOG_DIR",
        "MESHY_DCC_CHECKPOINT_DIR",
        "MESHY_DCC_SCRIPT_MATERIALIZATION_ROOT",
        "MESHY_DCC_EMBEDDING_CACHE_DIR",
    ):
        monkeypatch.delenv(name, raising=False)
    monkeypatch.setenv("HOME", str(tmp_path / "home"))
    root = tmp_path / "data"
    monkeypatch.setenv("MESHY_DCC_DATA_DIR", str(root))
    return root


def test_state_paths_follow_data_dir(data_dir, monkeypatch):
    assert host_errors._default_log_dir() == data_dir / "log"
    assert checkpoint.default_checkpoint_dir() == data_dir
    assert loaded_state_store.default_loaded_state_path("Maya") == data_dir / "maya" / "loaded.json"
    assert script_materialization.default_script_materialization_root() == data_dir
    assert vector_embedder.default_embedding_cache_path("maya") == data_dir / "maya" / "skill-embeddings.json"
    monkeypatch.setattr(skill_paths, "is_core_extension_available", lambda: False)
    assert skill_paths.get_local_skills_dir("Maya") == str(data_dir / "maya" / "skills")

    monkeypatch.setenv("MESHY_DCC_LOG_DIR", str(data_dir / "explicit"))
    assert host_errors._default_log_dir() == data_dir / "explicit"


def test_state_paths_unchanged_without_data_dir(data_dir, monkeypatch):
    monkeypatch.delenv("MESHY_DCC_DATA_DIR")
    home = Path(os.environ["HOME"])
    assert checkpoint.default_checkpoint_dir() == home / ".meshy-dcc"
    assert loaded_state_store.default_loaded_state_path("maya") == home / ".meshy-dcc" / "maya" / "loaded.json"
    monkeypatch.setattr(skill_paths, "is_core_extension_available", lambda: False)
    assert skill_paths.get_local_skills_dir("maya") == str(home / ".meshy-dcc" / "maya" / "skills")


def test_native_paths_follow_data_dir(data_dir):
    core = pytest.importorskip("meshy_dcc_core._core")
    assert Path(core.get_log_dir()) == data_dir / "log"
    assert Path(core.get_data_dir()) == data_dir
    assert Path(core.get_local_skills_dir("maya")) == data_dir / "maya" / "skills"


_SERVER = textwrap.dedent(
    """
    import logging, pathlib, sys
    from meshy_dcc_core._server.options import DccServerOptions
    from meshy_dcc_core.server_base import DccServerBase

    options = DccServerOptions.from_env("probe", pathlib.Path(sys.argv[1]), port=0, gateway_port=0)
    server = DccServerBase(options)
    server.register_builtin_actions()
    server.start()
    logging.getLogger("probe").error("an error the host-error log captures")
    server.stop()
    """
)


def test_adapter_server_writes_nothing_under_home(tmp_path):
    pytest.importorskip("meshy_dcc_core._core")
    home, data, skills = tmp_path / "home", tmp_path / "data", tmp_path / "skills"
    home.mkdir()
    skills.mkdir()
    env = {k: v for k, v in os.environ.items() if not k.startswith("MESHY_DCC_")}
    env.update(
        HOME=str(home),
        CFFIXED_USER_HOME=str(home),
        MESHY_DCC_DATA_DIR=str(data),
        MESHY_DCC_REGISTRY_DIR=str(tmp_path / "registry"),
        MESHY_DCC_GATEWAY_PORT="0",
    )
    subprocess.run([sys.executable, "-c", _SERVER, str(skills)], env=env, check=True, timeout=120)

    assert [p for p in home.rglob("*")] == []
    logs = sorted(p.name for p in (data / "log").iterdir())
    assert any(name.startswith("meshy-dcc-probe.") and name.endswith(".log") for name in logs), logs
    assert any(name.endswith("-jobs.db") for name in logs), logs
    assert any(name.endswith(".host-errors.log") for name in logs), logs
    assert (data / "probe" / "skills").is_dir()

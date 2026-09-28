"""Metadata projection tests for the extensionless Python 3.7 wheel."""

from __future__ import annotations

from pathlib import Path

from scripts.build_py37_pure_wheel import _is_lite_payload_file
from scripts.build_py37_pure_wheel import _read_console_scripts
from scripts.build_py37_pure_wheel import _read_runtime_requirements


def test_lite_wheel_inherits_project_runtime_dependencies() -> None:
    requirements = _read_runtime_requirements()
    assert "meshy-dcc-server>=0.18.17,<1.0.0" in requirements
    assert not any(requirement.lower().startswith("typing-extensions") for requirement in requirements)


def test_lite_wheel_inherits_project_console_scripts() -> None:
    assert _read_console_scripts() == {
        "meshy-dcc-install-lifecycle": "meshy_dcc_core.install_lifecycle_cli:main",
        "meshy-dcc-ui-control-server": "meshy_dcc_core.ui_control_server:cli",
    }


def test_lite_wheel_excludes_native_build_residue(tmp_path: Path) -> None:
    package = tmp_path / "meshy_dcc_core"
    package.mkdir()
    source = package / "_typing.py"
    source.write_text("", encoding="utf-8")
    assert _is_lite_payload_file(source)
    for name in ("_core.cp312-win_amd64.pyd", "_core.pdb", "helper.dll", "helper.so", "helper.dylib"):
        residue = package / name
        residue.write_bytes(b"")
        assert not _is_lite_payload_file(residue)

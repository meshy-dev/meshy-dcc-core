"""Import smoke tests for wheel and embedded Python compatibility."""

# Import future modules
from __future__ import annotations

# Import built-in modules
import importlib
import pkgutil
import subprocess
import sys

# Import third-party modules
import pytest

# Import local modules
import meshy_dcc_core
from meshy_dcc_core._core import SkillScanner
from meshy_dcc_core._core import ToolRegistry
from meshy_dcc_core._core import ToolResult


def collect_import_failures() -> list[tuple[str, str]]:
    """Import every package module and return failures as ``(name, error)``."""
    failures: list[tuple[str, str]] = []
    for module_info in pkgutil.walk_packages(meshy_dcc_core.__path__, prefix=f"{meshy_dcc_core.__name__}."):
        try:
            importlib.import_module(module_info.name)
        except Exception as exc:
            failures.append((module_info.name, f"{type(exc).__name__}: {exc}"))
    return failures


def assert_import_smoke() -> None:
    """Run the same import smoke test used by wheel CI and pytest."""
    print(f"Version: {meshy_dcc_core.__version__}")
    result = ToolResult(success=True, message="Wheel test passed")
    print(f"Result: {result}")
    reg = ToolRegistry()
    print(f"Registry: {reg}")
    print(f"Scanner: {SkillScanner}")

    failures = collect_import_failures()
    if failures:
        for name, error in failures:
            print(f"Import failed: {name}: {error}")
    assert failures == []
    print("All imports OK!")


def test_meshy_dcc_core_import_smoke() -> None:
    """Every importable package module should load without side effects."""
    assert_import_smoke()


@pytest.mark.parametrize(
    "module_name",
    ["meshy_dcc_core.dcc_server", "meshy_dcc_core._server", "meshy_dcc_core.skills.builtin"],
)
def test_module_imports_cleanly_as_first_import(module_name: str) -> None:
    """A fresh interpreter must import each entry point without an import cycle."""
    result = subprocess.run([sys.executable, "-c", f"import {module_name}"], capture_output=True, text=True)
    assert result.returncode == 0, result.stderr


if __name__ == "__main__":
    assert_import_smoke()

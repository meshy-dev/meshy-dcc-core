"""Scheduled uv.lock refresh guard contract tests."""

from __future__ import annotations

import importlib.util
from pathlib import Path

from conftest import REPO_ROOT

SCRIPT_PATH = REPO_ROOT / "scripts" / "ci" / "uv_lock_refresh.py"


def _load_module():
    spec = importlib.util.spec_from_file_location("uv_lock_refresh", SCRIPT_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _wheel(package: str, version: str, tag: str) -> str:
    return f"https://files.pythonhosted.org/packages/ab/cd/{package}-{version}-{tag}.whl"


def _semantic_wheels(version: str, *, cp37: bool = True) -> list:
    wheels = []
    if cp37:
        wheels.append(_wheel("meshy_dcc_core_semantic", version, "cp37-cp37m-manylinux_2_28_x86_64"))
    wheels.append(_wheel("meshy_dcc_core_semantic", version, "cp38-abi3-manylinux_2_28_x86_64"))
    return wheels


def _server_wheels(version: str) -> list:
    return [_wheel("meshy_dcc_server", version, "py3-none-manylinux2014_x86_64")]


def _parse(text: str) -> dict:
    try:
        import tomllib
    except ModuleNotFoundError:  # pragma: no cover - exercised by the Python 3.7 CI lane
        import tomli as tomllib

    return tomllib.loads(text)


def _lock_text(*, requires_python: str = ">=3.7", packages: list) -> str:
    lines = ["version = 1", "revision = 3", f'requires-python = "{requires_python}"', ""]
    for name, version, wheels in packages:
        lines.append("[[package]]")
        lines.append(f'name = "{name}"')
        lines.append(f'version = "{version}"')
        if wheels:
            lines.append("wheels = [")
            for url in wheels:
                lines.append(f'    {{ url = "{url}", hash = "sha256:deadbeef" }},')
            lines.append("]")
        lines.append("")
    return "\n".join(lines)


def _baseline():
    return [
        ("meshy-dcc-core-semantic", "0.20.33", _semantic_wheels("0.20.33")),
        ("meshy-dcc-server", "0.20.33", _server_wheels("0.20.33")),
        ("zipp", "3.19.1", [_wheel("zipp", "3.19.1", "py3-none-any")]),
    ]


def _refreshed():
    return [
        ("meshy-dcc-core-semantic", "0.20.34", _semantic_wheels("0.20.34")),
        ("meshy-dcc-server", "0.20.34", _server_wheels("0.20.34")),
        ("zipp", "3.19.1", [_wheel("zipp", "3.19.1", "py3-none-any")]),
    ]


def test_unchanged_lock_passes() -> None:
    module = _load_module()
    lock = _parse(_lock_text(packages=_baseline()))

    assert module.verify_refresh(lock, lock) == []


def test_allowed_package_bump_passes() -> None:
    module = _load_module()

    assert (
        module.verify_refresh(_parse(_lock_text(packages=_baseline())), _parse(_lock_text(packages=_refreshed()))) == []
    )


def test_unmanaged_package_bump_fails() -> None:
    module = _load_module()
    drifted = _refreshed()
    drifted[2] = ("zipp", "3.21.0", [_wheel("zipp", "3.21.0", "py3-none-any")])

    errors = module.verify_refresh(_parse(_lock_text(packages=_baseline())), _parse(_lock_text(packages=drifted)))

    assert len(errors) == 1
    assert "zipp" in errors[0]
    assert "outside the allowed refresh set" in errors[0]


def test_dropped_cp37_wheel_fails() -> None:
    module = _load_module()
    without_cp37 = _refreshed()
    without_cp37[0] = ("meshy-dcc-core-semantic", "0.20.34", _semantic_wheels("0.20.34", cp37=False))

    errors = module.verify_refresh(_parse(_lock_text(packages=_baseline())), _parse(_lock_text(packages=without_cp37)))

    assert any("cp37" in error and "meshy-dcc-core-semantic" in error for error in errors)


def test_narrowed_requires_python_fails() -> None:
    module = _load_module()
    before = _parse(_lock_text(packages=_baseline()))
    after = _parse(_lock_text(requires_python=">=3.9", packages=_baseline()))

    errors = module.verify_refresh(before, after)

    assert any("requires-python" in error for error in errors)


def test_removed_allowed_pin_fails() -> None:
    module = _load_module()
    trimmed = [entry for entry in _refreshed() if entry[0] != "meshy-dcc-server"]

    errors = module.verify_refresh(_parse(_lock_text(packages=_baseline())), _parse(_lock_text(packages=trimmed)))

    assert any("meshy-dcc-server" in error and "removed" in error for error in errors)


def test_downgraded_allowed_pin_fails() -> None:
    module = _load_module()
    # `0.20.34` got yanked, so the resolver falls back to an older release.
    rolled_back = _refreshed()
    rolled_back[0] = ("meshy-dcc-core-semantic", "0.20.32", _semantic_wheels("0.20.32"))

    errors = module.verify_refresh(_parse(_lock_text(packages=_baseline())), _parse(_lock_text(packages=rolled_back)))

    assert any("backwards" in error and "meshy-dcc-core-semantic" in error for error in errors)


def test_cli_verify_accepts_a_scoped_refresh(tmp_path: Path) -> None:
    module = _load_module()
    before = tmp_path / "before.lock"
    after = tmp_path / "after.lock"
    before.write_text(_lock_text(packages=_baseline()), encoding="utf-8")
    after.write_text(_lock_text(packages=_refreshed()), encoding="utf-8")

    assert module.main(["verify", str(before), str(after)]) == 0


def test_cli_verify_rejects_an_unscoped_refresh(tmp_path: Path) -> None:
    module = _load_module()
    before = tmp_path / "before.lock"
    after = tmp_path / "after.lock"
    before.write_text(_lock_text(packages=_baseline()), encoding="utf-8")
    after.write_text(_lock_text(requires_python=">=3.10", packages=_baseline()), encoding="utf-8")

    assert module.main(["verify", str(before), str(after)]) == 1


def test_cli_verify_honours_a_package_override(tmp_path: Path) -> None:
    module = _load_module()
    before = tmp_path / "before.lock"
    after = tmp_path / "after.lock"
    moved = _baseline()
    moved[2] = ("zipp", "3.21.0", [_wheel("zipp", "3.21.0", "py3-none-any")])
    before.write_text(_lock_text(packages=_baseline()), encoding="utf-8")
    after.write_text(_lock_text(packages=moved), encoding="utf-8")

    assert module.main(["verify", str(before), str(after), "--package", "zipp"]) == 0
    assert module.main(["verify", str(before), str(after)]) == 1

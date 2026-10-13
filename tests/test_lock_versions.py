"""Lockfile version drift gate contract tests."""

from __future__ import annotations

import importlib.util
from pathlib import Path

from conftest import REPO_ROOT

SCRIPT_PATH = REPO_ROOT / "scripts" / "ci" / "check_lock_versions.py"


def _load_checker_module():
    spec = importlib.util.spec_from_file_location("check_lock_versions", SCRIPT_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _write_cargo_workspace(
    root: Path,
    *,
    members: list[str],
    workspace_version: str,
    member_versions: dict,
) -> None:
    (root / "Cargo.toml").write_text(
        f'[workspace]\nmembers = {members!r}\nresolver = "2"\n\n[workspace.package]\nversion = "{workspace_version}"\n',
        encoding="utf-8",
    )
    for member, version in member_versions.items():
        declared = "version.workspace = true" if version is None else f'version = "{version}"'
        (root / member).mkdir(parents=True, exist_ok=True)
        (root / member / "Cargo.toml").write_text(
            f'[package]\nname = "{Path(member).name}"\n{declared}\n',
            encoding="utf-8",
        )


def _write_cargo_lock(root: Path, pinned: dict) -> None:
    entries = [f'[[package]]\nname = "{name}"\nversion = "{version}"\n' for name, version in pinned.items()]
    (root / "Cargo.lock").write_text("version = 4\n\n" + "\n".join(entries), encoding="utf-8")


def test_cargo_lock_matching_versions_passes(tmp_path: Path) -> None:
    checker = _load_checker_module()
    _write_cargo_workspace(
        tmp_path,
        members=["crates/a", "crates/hack"],
        workspace_version="0.20.34",
        member_versions={"crates/a": None, "crates/hack": "0.1.0"},
    )
    _write_cargo_lock(tmp_path, {"a": "0.20.34", "hack": "0.1.0"})

    assert checker.check_cargo_lock_versions(tmp_path) == []


def test_cargo_lock_drifted_crate_is_reported(tmp_path: Path) -> None:
    checker = _load_checker_module()
    _write_cargo_workspace(
        tmp_path,
        members=["crates/a"],
        workspace_version="0.20.34",
        member_versions={"crates/a": None},
    )
    _write_cargo_lock(tmp_path, {"a": "0.20.33"})

    errors = checker.check_cargo_lock_versions(tmp_path)

    assert len(errors) == 1
    assert "0.20.33" in errors[0]
    assert "0.20.34" in errors[0]


def test_cargo_lock_missing_member_is_reported(tmp_path: Path) -> None:
    checker = _load_checker_module()
    _write_cargo_workspace(
        tmp_path,
        members=["crates/a"],
        workspace_version="0.20.34",
        member_versions={"crates/a": None},
    )
    _write_cargo_lock(tmp_path, {})

    assert any("missing workspace member" in error for error in checker.check_cargo_lock_versions(tmp_path))


def test_main_reports_errors_and_returns_failure(tmp_path: Path, capsys) -> None:
    checker = _load_checker_module()
    _write_cargo_workspace(
        tmp_path, members=["crates/a"], workspace_version="0.2.0", member_versions={"crates/a": None}
    )
    _write_cargo_lock(tmp_path, {"a": "0.1.0"})

    assert checker.main([str(tmp_path)]) == 1
    assert "::error::" in capsys.readouterr().err


def test_main_succeeds_on_clean_repository() -> None:
    checker = _load_checker_module()

    assert checker.main([str(REPO_ROOT)]) == 0

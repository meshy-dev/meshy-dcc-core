"""Release lock projection and consistency contract tests."""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

from conftest import REPO_ROOT
from meshy_dcc_core import yaml_loads

SCRIPT_PATH = REPO_ROOT / "scripts" / "ci" / "check_uv_lock.py"
EXPECTED_ROOT_PACKAGE = "meshy-dcc-core"


def _load_checker_module():
    spec = importlib.util.spec_from_file_location("check_uv_lock", SCRIPT_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _write_version_files(
    root: Path,
    *,
    project_version: str,
    lock_version: str,
    project_name: str = EXPECTED_ROOT_PACKAGE,
    lock_name: str = EXPECTED_ROOT_PACKAGE,
    extra_lock_package: str = "",
) -> None:
    (root / "Cargo.toml").write_text(
        f'[workspace.package]\nversion = "{project_version}"\n',
        encoding="utf-8",
    )
    (root / "pyproject.toml").write_text(
        f'[project]\nname = "{project_name}"\nversion = "{project_version}"\n',
        encoding="utf-8",
    )
    (root / "uv.lock").write_text(
        f'version = 1\n\n[[package]]\nname = "{lock_name}"\n'
        f'version = "{lock_version}"\nsource = {{ editable = "." }}\n{extra_lock_package}',
        encoding="utf-8",
    )


def test_stale_editable_root_version_is_rejected(tmp_path: Path) -> None:
    checker = _load_checker_module()
    _write_version_files(tmp_path, project_version="0.20.15", lock_version="0.20.14")

    errors = checker.check_uv_lock_consistency(tmp_path)

    assert errors == ["uv.lock editable root meshy-dcc-core version '0.20.14' != expected '0.20.15'"]


def test_matching_editable_root_version_is_accepted(tmp_path: Path) -> None:
    checker = _load_checker_module()
    _write_version_files(tmp_path, project_version="0.20.15", lock_version="0.20.15")

    assert checker.check_uv_lock_consistency(tmp_path) == []


def test_synchronized_root_package_rename_is_rejected(tmp_path: Path) -> None:
    checker = _load_checker_module()
    _write_version_files(
        tmp_path,
        project_version="0.20.15",
        lock_version="0.20.15",
        project_name="renamed-core",
        lock_name="renamed-core",
    )

    assert checker.check_uv_lock_consistency(tmp_path) == [
        "pyproject.toml project.name 'renamed-core' != fixed identity 'meshy-dcc-core'"
    ]


def test_second_editable_shadow_root_is_rejected(tmp_path: Path) -> None:
    checker = _load_checker_module()
    _write_version_files(
        tmp_path,
        project_version="0.20.15",
        lock_version="0.20.15",
        extra_lock_package=('\n[[package]]\nname = "shadow-root"\nversion = "0.20.15"\nsource = { editable = "." }\n'),
    )

    assert checker.check_uv_lock_consistency(tmp_path) == [
        "uv.lock must contain exactly one source.editable='.' root package; found 2"
    ]


def test_invalid_project_version_is_rejected(tmp_path: Path) -> None:
    checker = _load_checker_module()
    _write_version_files(tmp_path, project_version="latest", lock_version="latest")

    assert checker.check_uv_lock_consistency(tmp_path) == [
        "Cargo.toml workspace.package.version 'latest' is not a valid project version"
    ]


def test_uv_lock_symlink_is_rejected_even_when_bytes_match(tmp_path: Path) -> None:
    checker = _load_checker_module()
    _write_version_files(tmp_path, project_version="0.20.15", lock_version="0.20.15")
    lock_path = tmp_path / "uv.lock"
    target_path = tmp_path / "uv.lock.target"
    lock_path.replace(target_path)
    lock_path.symlink_to(target_path.name)

    assert checker.check_uv_lock_consistency(tmp_path) == [
        "uv.lock must be a regular file and not a symlink or reparse point"
    ]


def test_uv_lock_directory_is_rejected_as_non_regular(tmp_path: Path) -> None:
    checker = _load_checker_module()
    _write_version_files(tmp_path, project_version="0.20.15", lock_version="0.20.15")
    lock_path = tmp_path / "uv.lock"
    lock_path.unlink()
    lock_path.mkdir()

    assert checker.check_uv_lock_consistency(tmp_path) == [
        "uv.lock must be a regular file and not a symlink or reparse point"
    ]


@pytest.mark.parametrize(
    ("path", "content", "expected"),
    [
        (
            "Cargo.toml",
            'workspace = "invalid"\n',
            "Cargo.toml workspace.package must be a mapping",
        ),
        (
            "pyproject.toml",
            'project = "invalid"\n',
            "pyproject.toml project must be a mapping",
        ),
        (
            "uv.lock",
            'version = 1\npackage = "invalid"\n',
            "uv.lock package must be a list",
        ),
        (
            "uv.lock",
            'version = 1\npackage = ["invalid"]\n',
            "uv.lock package entries must be mappings",
        ),
    ],
)
def test_malformed_shapes_return_stable_errors(
    tmp_path: Path,
    path: str,
    content: str,
    expected: str,
) -> None:
    checker = _load_checker_module()
    _write_version_files(tmp_path, project_version="0.20.15", lock_version="0.20.15")
    (tmp_path / path).write_text(content, encoding="utf-8")

    assert checker.check_uv_lock_consistency(tmp_path) == [expected]


def test_workflow_contract_parser_uses_wheel_owned_yaml_codec() -> None:
    workflow = yaml_loads("jobs:\n  validate:\n    steps:\n      - name: Run check\n        run: vx uv lock --check\n")

    assert workflow["jobs"]["validate"]["steps"][0]["run"] == "vx uv lock --check"

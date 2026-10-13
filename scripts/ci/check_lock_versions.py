#!/usr/bin/env python3
"""Fail when ``Cargo.lock`` drifts from the versions declared in the repo.

Every workspace member crate must be pinned to the version its ``Cargo.toml``
resolves to. Crates inherit the workspace version through
``version.workspace = true``, so a version bump moves the declaration and the
lock together; any difference means one of the two was regenerated or
hand-edited alone.

The check is read-only and never rewrites the lockfile.
"""

from __future__ import annotations

from pathlib import Path
import sys

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - exercised by the Python 3.7 CI lane
    import tomli as tomllib


def _load_toml(path: Path) -> dict:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def _locked_versions(lock: dict) -> dict:
    """Map package name -> list of versions pinned in a Cargo lockfile."""
    versions: dict = {}
    packages = lock.get("package")
    if not isinstance(packages, list):
        return versions
    for package in packages:
        if not isinstance(package, dict):
            continue
        name = package.get("name")
        version = package.get("version")
        if isinstance(name, str) and isinstance(version, str):
            versions.setdefault(name, []).append(version)
    return versions


def check_cargo_lock_versions(root: Path) -> list[str]:
    """Return errors for workspace crates whose ``Cargo.lock`` pin drifted."""
    cargo_toml_path = root / "Cargo.toml"
    cargo_lock_path = root / "Cargo.lock"
    if not cargo_toml_path.exists() or not cargo_lock_path.exists():
        return []

    workspace = _load_toml(cargo_toml_path).get("workspace")
    if not isinstance(workspace, dict):
        return ["Cargo.toml must declare a [workspace] table"]
    workspace_version = workspace.get("package", {}).get("version")
    members = workspace.get("members")
    if not isinstance(workspace_version, str) or not isinstance(members, list):
        return ["Cargo.toml must declare workspace.package.version and workspace.members"]

    locked = _locked_versions(_load_toml(cargo_lock_path))

    errors = []
    for member in members:
        if not isinstance(member, str):
            continue
        member_toml_path = root / member / "Cargo.toml"
        if not member_toml_path.exists():
            errors.append(f"Cargo.toml member {member!r} has no Cargo.toml")
            continue
        package = _load_toml(member_toml_path).get("package")
        if not isinstance(package, dict):
            continue
        name = package.get("name")
        declared = package.get("version")
        if isinstance(declared, dict):
            # ``version.workspace = true`` inherits the workspace version; any
            # other table form is an inheritance we do not model, so skip it
            # rather than reporting a false drift.
            if declared.get("workspace") is True:
                declared = workspace_version
            else:
                continue
        if not isinstance(name, str) or not isinstance(declared, str):
            continue

        versions = locked.get(name)
        if not versions:
            errors.append(f"Cargo.lock is missing workspace member {name!r}")
        elif declared not in versions:
            errors.append(f"Cargo.lock {name} version {versions[0]!r} != declared {declared!r} ({member}/Cargo.toml)")
    return errors


def main(argv: list[str] | None = None) -> int:
    """Run the lockfile version check against the repository root."""
    args = list(sys.argv[1:] if argv is None else argv)
    root = Path(args[0]) if args else Path.cwd()

    errors = check_cargo_lock_versions(root)
    if errors:
        for error in errors:
            print(f"::error::{error}", file=sys.stderr)
        print(
            "::error::Lockfile versions drifted from the declared versions. "
            "See the diff lines above for each offending package.",
            file=sys.stderr,
        )
        return 1
    print("Lockfile versions match the declared versions.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

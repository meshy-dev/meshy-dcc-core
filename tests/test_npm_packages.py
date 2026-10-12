"""npm packaging: a launcher package plus one binary package per platform."""

from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "release" / "build_npm_packages.py"


def _members(tarball: Path) -> dict[str, tarfile.TarInfo]:
    with tarfile.open(tarball) as archive:
        return {member.name: member for member in archive.getmembers()}


def _json(tarball: Path, name: str) -> dict:
    with tarfile.open(tarball) as archive:
        return json.load(archive.extractfile(f"package/{name}"))


@pytest.mark.skipif(shutil.which("npm") is None, reason="npm is unavailable")
def test_packages_pin_platform_binaries_and_leave_upgrades_to_npm(tmp_path: Path) -> None:
    mac, win = tmp_path / "mac", tmp_path / "win"
    mac.mkdir()
    win.mkdir()
    for name in ("meshy-dcc", "meshy-dcc-server"):
        (mac / name).write_bytes(b"mac")
        (win / f"{name}.exe").write_bytes(b"win")
    out = tmp_path / "out"
    subprocess.run(
        [sys.executable, str(SCRIPT), "--version", "1.2.3", "--out", str(out),
         "--macos-arm64", str(mac), "--windows-x86_64", str(win)],
        check=True,
        capture_output=True,
    )

    main = out / "meshy-dcc-1.2.3.tgz"
    manifest = _json(main, "package.json")
    assert manifest["bin"] == {"meshy-dcc": "bin/meshy-dcc.js"}
    assert manifest["optionalDependencies"] == {
        "@meshy-ai/meshy-dcc-darwin-arm64": "1.2.3",
        "@meshy-ai/meshy-dcc-win32-x64": "1.2.3",
    }

    darwin = out / "meshy-ai-meshy-dcc-darwin-arm64-1.2.3.tgz"
    assert _json(darwin, "package.json")["os"] == ["darwin"]
    assert _json(darwin, "package.json")["cpu"] == ["arm64"]
    assert _json(darwin, "bin/meshy-dcc.package-manager.json")["manager"] == "npm"
    members = _members(darwin)
    assert members["package/bin/meshy-dcc"].mode & 0o111
    assert members["package/bin/meshy-dcc-server"].mode & 0o111
    assert "package/NOTICE" in members

    win32 = out / "meshy-ai-meshy-dcc-win32-x64-1.2.3.tgz"
    assert "package/bin/meshy-dcc-server.exe" in _members(win32)

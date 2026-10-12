"""Release update-manifest component contracts."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "release" / "generate_update_manifest.py"


def _run(tmp_path: Path, platform: str) -> subprocess.CompletedProcess[str]:
    (tmp_path / "cli").write_bytes(b"cli")
    (tmp_path / "server").write_bytes(b"server")
    command = [
        sys.executable,
        str(SCRIPT),
        "--version",
        "1.2.3",
        "--platform",
        platform,
        "--cli-bin",
        str(tmp_path / "cli"),
        "--server-bin",
        str(tmp_path / "server"),
        "--out-dir",
        str(tmp_path / "dist"),
    ]
    return subprocess.run(command, capture_output=True, text=True, check=False)


@pytest.mark.parametrize(("platform", "ext"), [("macos-arm64", ""), ("windows-x86_64", ".exe")])
def test_manifest_names_cdn_assets_that_sit_beside_it(tmp_path: Path, platform: str, ext: str) -> None:
    result = _run(tmp_path, platform)
    assert result.returncode == 0, result.stderr
    dist = tmp_path / "dist"
    manifest = json.loads((dist / f"meshy-dcc-update-manifest-{platform}.json").read_text())
    assert set(manifest) == {"meshy-dcc", "meshy-dcc-server"}
    for name, content in (("meshy-dcc", b"cli"), ("meshy-dcc-server", b"server")):
        asset = f"{name}-{platform}{ext}"
        assert manifest[name]["url"] == f"https://cdn.meshy.ai/meshy-dcc-core/v1.2.3/{asset}"
        assert manifest[name]["sha256"] == hashlib.sha256(content).hexdigest()
        assert (dist / asset).read_bytes() == content


def test_unpublished_platforms_are_refused(tmp_path: Path) -> None:
    result = _run(tmp_path, "macos-universal2")
    assert result.returncode != 0

"""Release update-manifest component contracts."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path
import subprocess
import sys

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "release" / "generate_update_manifest.py"


def _run(tmp_path: Path, platform: str, *extra: str) -> subprocess.CompletedProcess[str]:
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
        *extra,
    ]
    return subprocess.run(command, capture_output=True, text=True, check=False)


@pytest.mark.parametrize(
    ("platform", "label", "ext"),
    [("macos-arm64", "macos-arm64", ""), ("windows-x86_64", "windows-x64", ".exe")],
)
def test_cdn_manifest_uses_cdn_names_in_a_version_dir(tmp_path: Path, platform: str, label: str, ext: str) -> None:
    result = _run(tmp_path, platform)
    assert result.returncode == 0, result.stderr
    dist = tmp_path / "dist"
    manifest = json.loads((dist / f"meshy-dcc-update-manifest-{label}.json").read_text())
    assert set(manifest) == {"meshy-dcc", "meshy-dcc-server"}
    for name, content in (("meshy-dcc", b"cli"), ("meshy-dcc-server", b"server")):
        asset = f"{name}-{label}{ext}"
        assert manifest[name]["url"] == f"https://cdn.meshy.ai/meshy-dcc-core/v1-2-3/{asset}"
        assert manifest[name]["mirrors"] == [
            f"https://cdn.meshy.cn/meshy-dcc-core/v1-2-3/{asset}",
            f"https://github.com/meshy-dev/meshy-dcc-core/releases/download/v1.2.3/{name}-{platform}{ext}",
        ]
        assert re.fullmatch(r"[a-z0-9-]+(\.[a-z0-9]+)?", asset)
        assert manifest[name]["sha256"] == hashlib.sha256(content).hexdigest()
        assert (dist / asset).read_bytes() == content


def test_unpublished_platforms_are_refused(tmp_path: Path) -> None:
    result = _run(tmp_path, "macos-universal2")
    assert result.returncode != 0


def test_github_manifest_points_at_the_github_release(tmp_path: Path) -> None:
    result = _run(tmp_path, "macos-arm64", "--source", "github")
    assert result.returncode == 0, result.stderr
    manifest = json.loads((tmp_path / "dist" / "meshy-dcc-update-manifest-macos-arm64.json").read_text())
    assert manifest["meshy-dcc"]["url"] == (
        "https://github.com/meshy-dev/meshy-dcc-core/releases/download/v1.2.3/meshy-dcc-macos-arm64"
    )


def test_windows_on_github_keeps_the_release_platform_name(tmp_path: Path) -> None:
    result = _run(tmp_path, "windows-x86_64", "--source", "github")
    assert result.returncode == 0, result.stderr
    manifest = json.loads((tmp_path / "dist" / "meshy-dcc-update-manifest-windows-x86_64.json").read_text())
    assert manifest["meshy-dcc-server"]["url"].endswith("/v1.2.3/meshy-dcc-server-windows-x86_64.exe")

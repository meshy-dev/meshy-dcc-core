"""Release update-manifest component contracts."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "release" / "generate_update_manifest.py"


def _run(tmp_path: Path, platform: str) -> subprocess.CompletedProcess[str]:
    server = tmp_path / ("meshy-dcc-server.exe" if platform.startswith("windows-") else "meshy-dcc-server")
    cli = tmp_path / ("meshy-dcc-cli.exe" if platform.startswith("windows-") else "meshy-dcc-cli")
    server.write_bytes(b"server")
    cli.write_bytes(b"cli")
    command = [
        sys.executable,
        str(SCRIPT),
        "--version",
        "1.2.3",
        "--platform",
        platform,
        "--release-tag",
        "v1.2.3",
        "--repo",
        "meshy-dev/meshy-dcc-core",
        "--server-bin",
        str(server),
        "--cli-bin",
        str(cli),
        "--out-dir",
        str(tmp_path / "dist"),
    ]
    return subprocess.run(command, capture_output=True, text=True, check=False)


def test_platform_manifests_publish_only_core_binaries(tmp_path: Path) -> None:
    result = _run(tmp_path, "windows-x86_64")
    assert result.returncode == 0, result.stderr
    manifest = json.loads((tmp_path / "dist" / "meshy-dcc-update-manifest-windows-x86_64.json").read_text())
    assert set(manifest) == {"meshy-dcc-server", "meshy-dcc-cli"}
    assert manifest["meshy-dcc-server"]["sha256"] == hashlib.sha256(b"server").hexdigest()
    assert manifest["meshy-dcc-cli"]["sha256"] == hashlib.sha256(b"cli").hexdigest()

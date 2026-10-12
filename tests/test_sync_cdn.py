"""CDN upload: names are checked before anything is sent."""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "release" / "sync_cdn.py"


def test_names_that_break_the_cdn_rules_stop_before_any_request(tmp_path: Path) -> None:
    (tmp_path / "meshy-dcc-windows-x64.exe").write_bytes(b"ok")
    (tmp_path / "meshy_dcc_windows_x86_64.zip").write_bytes(b"bad")
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--version", "0.2.0", "--release-dir", str(tmp_path)],
        capture_output=True,
        text=True,
        env={"CDN_API_KEY": "cdnk_test", "PATH": "/usr/bin:/bin"},
        timeout=30,
    )
    assert result.returncode != 0
    assert "meshy_dcc_windows_x86_64.zip does not follow the CDN naming rules" in result.stderr

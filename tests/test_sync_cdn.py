"""CDN copy of a release: names checked first, release files never overwritten,
latest.txt only moves forward."""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "release" / "sync_cdn.py"

FAKE_AWS = """#!/bin/sh
echo "$*" >> "$AWS_LOG"
if [ "$*" = "s3 cp s3://meshy-cdn/meshy-dcc-core/latest.txt -" ]; then
    [ -n "$CURRENT_LATEST" ] || exit 1
    echo "$CURRENT_LATEST"
fi
exit 0
"""


def _run(tmp_path: Path, version: str, current_latest: str = "") -> tuple[subprocess.CompletedProcess[str], list[str]]:
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir(exist_ok=True)
    (bin_dir / "aws").write_text(FAKE_AWS)
    (bin_dir / "aws").chmod(0o755)
    log = tmp_path / "aws.log"
    log.write_text("")
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--version", version, "--release-dir", str(tmp_path / "release"),
         "--install-script", str(tmp_path / "install.sh")],
        capture_output=True,
        text=True,
        env={"PATH": f"{bin_dir}:/usr/bin:/bin", "AWS_LOG": str(log), "CURRENT_LATEST": current_latest},
        timeout=30,
    )
    return result, log.read_text().splitlines()


@pytest.fixture
def release(tmp_path: Path) -> Path:
    (tmp_path / "release").mkdir()
    (tmp_path / "release" / "meshy-dcc-macos-arm64").write_bytes(b"cli")
    (tmp_path / "release" / "meshy-dcc-update-manifest-macos-arm64.json").write_text("{}")
    (tmp_path / "install.sh").write_text("#!/bin/sh\n")
    return tmp_path


def test_names_that_break_the_cdn_rules_stop_before_any_upload(release: Path) -> None:
    (release / "release" / "meshy_dcc_windows_x86_64.zip").write_bytes(b"bad")
    result, calls = _run(release, "0.2.0")
    assert result.returncode != 0
    assert "meshy_dcc_windows_x86_64.zip does not follow the CDN naming rules" in result.stderr
    assert calls == []


def test_release_files_are_written_once_and_latest_moves_forward(release: Path) -> None:
    result, calls = _run(release, "0.2.0", current_latest="0.1.0")
    assert result.returncode == 0, result.stderr
    puts = [call for call in calls if call.startswith("s3api put-object")]
    assert len(puts) == 2
    assert all("--if-none-match *" in call and "--key meshy-dcc-core/v0-2-0/" in call for call in puts)
    assert any("s3://meshy-cdn/meshy-dcc-core/install.sh" in call and "no-cache" in call for call in calls)
    assert "s3://meshy-cdn/meshy-dcc-core/latest.txt --content-type text/plain --cache-control no-cache" in calls[-1]


@pytest.mark.parametrize(("version", "current"), [("0.2.0", "0.3.0"), ("0.3.0-rc.1", "0.2.0")])
def test_latest_never_goes_back_or_to_a_prerelease(release: Path, version: str, current: str) -> None:
    result, calls = _run(release, version, current_latest=current)
    assert result.returncode == 0, result.stderr
    assert not any("latest.txt --content-type" in call for call in calls)

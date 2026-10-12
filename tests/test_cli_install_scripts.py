from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[1]
SH_INSTALLER = ROOT / "scripts" / "install-cli.sh"
PS_INSTALLER = ROOT / "scripts" / "install-cli.ps1"
OFFICIAL_RELEASE = "https://cdn.meshy.ai/meshy-dcc-core"
GITHUB_RELEASES = "https://github.com/meshy-dev/meshy-dcc-core/releases"
INSTALL_DOCS = (ROOT / "README.md",)


def _shell() -> str:
    executable = shutil.which("sh")
    if executable:
        return executable
    git_sh = Path(r"C:\Program Files\Git\bin\sh.exe")
    if not git_sh.is_file():
        pytest.skip("sh is unavailable")
    return str(git_sh)


def _shell_path(path: Path) -> str:
    value = path.resolve().as_posix()
    if os.name == "nt" and len(value) > 2 and value[1] == ":":
        return f"/{value[0].lower()}{value[2:]}"
    return value


def _write_fake_unix_tools(bin_dir: Path, os_name: str = "Darwin", arch: str = "arm64") -> None:
    bin_dir.mkdir()
    curl = bin_dir / "curl"
    with curl.open("w", encoding="utf-8", newline="\n") as stream:
        stream.write(
            """#!/bin/sh
url=""
out=""
format=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o) out="$2"; shift 2 ;;
        -w) format="$2"; shift 2 ;;
        http*) url="$1"; shift ;;
        *) shift ;;
    esac
done
printf '%s\n' "$url" >> "$REQUEST_LOG"
for prefix in ${UNREACHABLE:-}; do
    case "$url" in "$prefix"*) exit 7 ;; esac
done
if [ -n "$format" ]; then
    printf '%s' "$LATEST_REDIRECT"
    exit 0
fi
case "$url" in
    *meshy-dcc-update-manifest-*) cp "$FIXTURE_MANIFEST" "$out" ;;
    *meshy-dcc-*) cp "$FIXTURE_BINARY" "$out" ;;
    *) exit 22 ;;
esac
"""
        )
    uname = bin_dir / "uname"
    with uname.open("w", encoding="utf-8", newline="\n") as stream:
        stream.write(
            """#!/bin/sh
case "$1" in
    -s) echo %OS% ;;
    -m) echo %ARCH% ;;
    *) exit 1 ;;
esac
"""
            .replace("%OS%", os_name)
            .replace("%ARCH%", arch)
        )
    curl.chmod(0o755)
    uname.chmod(0o755)


def _run_sh_installer(
    tmp_path: Path,
    manifest: dict,
    binary: bytes,
    os_name: str = "Darwin",
    arch: str = "arm64",
    unreachable: str = "",
) -> subprocess.CompletedProcess[str]:
    fake_bin = tmp_path / "bin"
    _write_fake_unix_tools(fake_bin, os_name, arch)
    manifest_path = tmp_path / "manifest.json"
    binary_path = tmp_path / "cli.bin"
    request_log = tmp_path / "requests.log"
    install_dir = tmp_path / "install"
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    binary_path.write_bytes(binary)
    env = os.environ.copy()
    env.update(
        {
            "MESHY_DCC_VERSION": "latest",
            "MESHY_DCC_INSTALL_DIR": _shell_path(install_dir),
            "FIXTURE_MANIFEST": _shell_path(manifest_path),
            "FIXTURE_BINARY": _shell_path(binary_path),
            "REQUEST_LOG": _shell_path(request_log),
            "ARG_LOG": _shell_path(tmp_path / "args.log"),
            "UNREACHABLE": unreachable,
            "LATEST_REDIRECT": f"{GITHUB_RELEASES}/tag/v0.2.0",
        }
    )
    return subprocess.run(
        [
            _shell(),
            "-c",
            'PATH="$1:/usr/bin:/bin"; export PATH; exec "$2"',
            "installer-test",
            _shell_path(fake_bin),
            _shell_path(SH_INSTALLER),
        ],
        capture_output=True,
        env=env,
        text=True,
        timeout=30,
        check=False,
    )


def _powershell() -> str:
    executable = shutil.which("pwsh") or shutil.which("powershell")
    if not executable:
        pytest.skip("PowerShell is unavailable")
    return executable


def _run_ps_installer(tmp_path: Path, manifest: dict, binary: bytes) -> subprocess.CompletedProcess[str]:
    manifest_path = tmp_path / "manifest.json"
    binary_path = tmp_path / "cli.exe"
    request_log = tmp_path / "requests.log"
    install_dir = tmp_path / "install"
    wrapper = tmp_path / "invoke-installer.ps1"
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    binary_path.write_bytes(binary)
    wrapper.write_text(
        """$ErrorActionPreference = "Stop"
$originalUserPath = [Environment]::GetEnvironmentVariable("Path", "User")
function Invoke-WebRequest {
    param([string] $Uri, [string] $OutFile, [int] $TimeoutSec)
    Add-Content -LiteralPath $env:REQUEST_LOG -Value $Uri
    if ($Uri -like "*meshy-dcc-update-manifest-*") {
        Copy-Item -LiteralPath $env:FIXTURE_MANIFEST -Destination $OutFile
    } elseif ($Uri -like "*meshy-dcc-*") {
        Copy-Item -LiteralPath $env:FIXTURE_BINARY -Destination $OutFile
    } else {
        throw "Unexpected URL: $Uri"
    }
}
function Start-Process {
    param([string] $FilePath, [object[]] $ArgumentList, [switch] $Wait, [switch] $PassThru, [switch] $NoNewWindow)
    [pscustomobject]@{ ExitCode = 0 }
}
try {
    & $env:INSTALL_SCRIPT -Version v0.2.0 -InstallDir $env:TEST_INSTALL_DIR
} finally {
    $currentUserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($currentUserPath -cne $originalUserPath) {
        [Environment]::SetEnvironmentVariable("Path", $originalUserPath, "User")
    }
}
""",
        encoding="utf-8",
    )
    env = os.environ.copy()
    env.update(
        {
            "INSTALL_SCRIPT": str(PS_INSTALLER),
            "TEST_INSTALL_DIR": str(install_dir),
            "FIXTURE_MANIFEST": str(manifest_path),
            "FIXTURE_BINARY": str(binary_path),
            "REQUEST_LOG": str(request_log),
        }
    )
    return subprocess.run(
        [_powershell(), "-NoLogo", "-NoProfile", "-File", str(wrapper)],
        capture_output=True,
        env=env,
        text=True,
        timeout=30,
        check=False,
    )


def _manifest(*, platform: str, binary: bytes, url=None, sha256=None, source: str = "cdn") -> dict:
    ext = ".exe" if platform.startswith("windows") else ""
    if source == "cdn":
        base, label = f"{OFFICIAL_RELEASE}/v0-2-0", platform.replace("x86_64", "x64")
    else:
        base, label = f"{GITHUB_RELEASES}/download/v0.2.0", platform
    return {
        name: {
            "version": "0.2.0",
            "url": url or f"{base}/{name}-{label}{ext}",
            "sha256": sha256 or hashlib.sha256(binary).hexdigest(),
        }
        for name in ("meshy-dcc", "meshy-dcc-server")
    }


def test_installers_do_not_recommend_remote_pipe_execution() -> None:
    forbidden = re.compile(
        r"(?:curl|irm|invoke-restmethod)[^\r\n|]*\|[^\r\n]*(?:sh|bash|iex|invoke-expression)|ExecutionPolicy\s+Bypass",
        re.IGNORECASE,
    )
    for path in (SH_INSTALLER, PS_INSTALLER):
        assert forbidden.search(path.read_text(encoding="utf-8")) is None, path


def test_install_docs_pipe_only_official_installers_and_never_bypass_policy() -> None:
    pipe = re.compile(
        r"(?:curl|irm|invoke-restmethod)\s[^\r\n|]*\|[^\r\n]*(?:sh|bash|iex|invoke-expression)", re.IGNORECASE
    )
    official = (
        "https://cdn.meshy.ai/meshy-dcc-core/install.",
        "https://cdn.meshy.cn/meshy-dcc-core/install.",
        "https://github.com/meshy-dev/meshy-dcc-core/releases/latest/download/install.",
    )
    for path in INSTALL_DOCS:
        text = path.read_text(encoding="utf-8")
        assert re.search(r"ExecutionPolicy\s+Bypass", text, re.IGNORECASE) is None, path
        for match in pipe.finditer(text):
            assert any(url in match.group(0) for url in official), match.group(0)


def test_installers_use_fixed_official_update_manifests() -> None:
    sh_source = SH_INSTALLER.read_text(encoding="utf-8")
    ps_source = PS_INSTALLER.read_text(encoding="utf-8")
    for source in (sh_source, ps_source):
        assert "MESHY_DCC_REPO" not in source
        assert "api.github.com" not in source
        assert "https://cdn.meshy.ai/meshy-dcc-core" in source
        assert "https://github.com/meshy-dev/meshy-dcc-core/releases" in source
        assert "meshy-dcc-update-manifest" in source
        assert "sha256" in source.lower()
    assert "MESHY_DCC_RELEASE_FALLBACK" not in sh_source
    assert "[string] $Repo" not in ps_source


def test_sh_latest_uses_manifest_pinned_urls_and_installs_verified_binaries(tmp_path: Path) -> None:
    binary = b'#!/bin/sh\necho "$*" >> "$ARG_LOG"\n'
    result = _run_sh_installer(tmp_path, _manifest(platform="macos-arm64", binary=binary), binary)

    assert result.returncode == 0, result.stderr
    assert (tmp_path / "install" / "meshy-dcc").read_bytes() == binary
    assert (tmp_path / "install" / "meshy-dcc-server").read_bytes() == binary
    requests = (tmp_path / "requests.log").read_text(encoding="utf-8").splitlines()
    assert requests == [
        f"{GITHUB_RELEASES}/latest",
        f"{OFFICIAL_RELEASE}/v0-2-0/meshy-dcc-update-manifest-macos-arm64.json",
        f"{OFFICIAL_RELEASE}/v0-2-0/meshy-dcc-macos-arm64",
        f"{OFFICIAL_RELEASE}/v0-2-0/meshy-dcc-server-macos-arm64",
    ]
    # The Skill plan is shown before it is written.
    assert (tmp_path / "args.log").read_text(encoding="utf-8").splitlines() == [
        "--output human agent-skill",
        "--output human agent-skill --execute",
    ]


@pytest.mark.parametrize(("os_name", "arch"), [("Darwin", "x86_64"), ("Linux", "x86_64")])
def test_sh_refuses_unpublished_platforms_before_download(tmp_path: Path, os_name: str, arch: str) -> None:
    binary = b"never downloaded"
    result = _run_sh_installer(
        tmp_path, _manifest(platform="macos-arm64", binary=binary), binary, os_name, arch
    )

    assert result.returncode != 0
    assert "Apple Silicon" in result.stderr
    assert not (tmp_path / "requests.log").exists()


def test_sh_uses_cdn_meshy_cn_when_cdn_meshy_ai_is_unreachable(tmp_path: Path) -> None:
    binary = b'#!/bin/sh\necho "$*" >> "$ARG_LOG"\n'
    result = _run_sh_installer(
        tmp_path, _manifest(platform="macos-arm64", binary=binary), binary, unreachable="https://cdn.meshy.ai/"
    )

    assert result.returncode == 0, result.stderr
    requests = (tmp_path / "requests.log").read_text(encoding="utf-8").splitlines()
    cn = "https://cdn.meshy.cn/meshy-dcc-core/v0-2-0"
    assert requests == [
        f"{GITHUB_RELEASES}/latest",
        f"{OFFICIAL_RELEASE}/v0-2-0/meshy-dcc-update-manifest-macos-arm64.json",
        f"{cn}/meshy-dcc-update-manifest-macos-arm64.json",
        f"{cn}/meshy-dcc-macos-arm64",
        f"{cn}/meshy-dcc-server-macos-arm64",
    ]


def test_sh_falls_back_to_the_github_release_when_both_cdn_hosts_are_unreachable(tmp_path: Path) -> None:
    binary = b'#!/bin/sh\necho "$*" >> "$ARG_LOG"\n'
    result = _run_sh_installer(
        tmp_path,
        _manifest(platform="macos-arm64", binary=binary, source="github"),
        binary,
        unreachable="https://cdn.meshy.ai/ https://cdn.meshy.cn/",
    )

    assert result.returncode == 0, result.stderr
    assert (tmp_path / "install" / "meshy-dcc").read_bytes() == binary
    requests = (tmp_path / "requests.log").read_text(encoding="utf-8").splitlines()
    assert requests == [
        f"{GITHUB_RELEASES}/latest",
        f"{OFFICIAL_RELEASE}/v0-2-0/meshy-dcc-update-manifest-macos-arm64.json",
        "https://cdn.meshy.cn/meshy-dcc-core/v0-2-0/meshy-dcc-update-manifest-macos-arm64.json",
        f"{GITHUB_RELEASES}/download/v0.2.0/meshy-dcc-update-manifest-macos-arm64.json",
        f"{GITHUB_RELEASES}/download/v0.2.0/meshy-dcc-macos-arm64",
        f"{GITHUB_RELEASES}/download/v0.2.0/meshy-dcc-server-macos-arm64",
    ]


def test_sh_a_failed_check_does_not_move_to_another_source(tmp_path: Path) -> None:
    binary = b"tampered"
    result = _run_sh_installer(
        tmp_path, _manifest(platform="macos-arm64", binary=binary, sha256="0" * 64), binary
    )

    assert result.returncode != 0
    requests = (tmp_path / "requests.log").read_text(encoding="utf-8").splitlines()
    assert not any(url.startswith(f"{GITHUB_RELEASES}/download") for url in requests)


def test_sh_checksum_mismatch_preserves_existing_binary(tmp_path: Path) -> None:
    install_dir = tmp_path / "install"
    install_dir.mkdir()
    target = install_dir / "meshy-dcc"
    target.write_bytes(b"known good")
    binary = b"tampered"
    result = _run_sh_installer(
        tmp_path,
        _manifest(platform="macos-arm64", binary=binary, sha256="0" * 64),
        binary,
    )

    assert result.returncode != 0
    assert "SHA-256" in result.stderr
    assert target.read_bytes() == b"known good"


def test_sh_rejects_non_official_asset_before_download(tmp_path: Path) -> None:
    binary = b"must not download"
    result = _run_sh_installer(
        tmp_path,
        _manifest(
            platform="macos-arm64",
            binary=binary,
            url="https://example.invalid/meshy-dcc-macos-arm64",
        ),
        binary,
    )

    assert result.returncode != 0
    assert "official" in result.stderr.lower()
    requests = (tmp_path / "requests.log").read_text(encoding="utf-8").splitlines()
    assert requests == [
        f"{GITHUB_RELEASES}/latest",
        f"{OFFICIAL_RELEASE}/v0-2-0/meshy-dcc-update-manifest-macos-arm64.json",
    ]
    assert not (tmp_path / "install" / "meshy-dcc").exists()


def test_powershell_latest_uses_manifest_pinned_url_and_installs_verified_binary(
    tmp_path: Path,
) -> None:
    binary = b"verified windows cli"
    result = _run_ps_installer(
        tmp_path,
        _manifest(platform="windows-x86_64", binary=binary),
        binary,
    )

    assert result.returncode == 0, result.stderr
    assert (tmp_path / "install" / "meshy-dcc.exe").read_bytes() == binary
    assert (tmp_path / "install" / "meshy-dcc-server.exe").read_bytes() == binary
    requests = (tmp_path / "requests.log").read_text(encoding="utf-8").splitlines()
    assert requests == [
        f"{OFFICIAL_RELEASE}/v0-2-0/meshy-dcc-update-manifest-windows-x64.json",
        f"{OFFICIAL_RELEASE}/v0-2-0/meshy-dcc-windows-x64.exe",
        f"{OFFICIAL_RELEASE}/v0-2-0/meshy-dcc-server-windows-x64.exe",
    ]


def test_powershell_checksum_mismatch_preserves_existing_binary(tmp_path: Path) -> None:
    install_dir = tmp_path / "install"
    install_dir.mkdir()
    target = install_dir / "meshy-dcc.exe"
    target.write_bytes(b"known good")
    binary = b"tampered"
    result = _run_ps_installer(
        tmp_path,
        _manifest(platform="windows-x86_64", binary=binary, sha256="0" * 64),
        binary,
    )

    assert result.returncode != 0
    assert "SHA-256" in result.stderr
    assert target.read_bytes() == b"known good"


def test_powershell_rejects_non_official_asset_before_download(tmp_path: Path) -> None:
    binary = b"must not download"
    result = _run_ps_installer(
        tmp_path,
        _manifest(
            platform="windows-x86_64",
            binary=binary,
            url="https://example.invalid/meshy-dcc-windows-x86_64.exe",
        ),
        binary,
    )

    assert result.returncode != 0
    assert "official release" in result.stderr.lower()
    requests = (tmp_path / "requests.log").read_text(encoding="utf-8").splitlines()
    assert requests == [
        f"{OFFICIAL_RELEASE}/v0-2-0/meshy-dcc-update-manifest-windows-x64.json",
    ]
    assert not (tmp_path / "install" / "meshy-dcc.exe").exists()

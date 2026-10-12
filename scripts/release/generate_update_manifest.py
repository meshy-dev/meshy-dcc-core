#!/usr/bin/env python3
"""Generate one platform's release files for the Meshy release channel.

Writes, into ``--out-dir``:

- ``meshy-dcc-<platform>[.exe]`` and ``meshy-dcc-server-<platform>[.exe]``,
  copies of the two binaries under their published names;
- ``meshy-dcc-update-manifest-<platform>.json``, mapping each binary to its
  version, download URL and SHA-256.

The same binaries go to two places, each with its own manifest (``--source``):

- the GitHub Release, under the names above;
- the Meshy CDN, under ``meshy-dcc-core/<version dir>/`` with names that follow
  the CDN naming rules (lowercase letters, digits and hyphens): the version dir
  is ``v0-2-0`` for 0.2.0 and ``windows-x86_64`` becomes ``windows-x64``.
  Nothing on the CDN is ever overwritten, so there is no ``latest/``; the
  installers ask the GitHub Release which version is latest.

The installers try the CDN first and the GitHub Release when the CDN is
unreachable, and accept only the URLs written here for that source.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil

CDN = "https://cdn.meshy.ai/meshy-dcc-core"
GITHUB = "https://github.com/meshy-dev/meshy-dcc-core/releases/download"
PLATFORMS = ("macos-arm64", "windows-x86_64")
SOURCES = ("cdn", "github")
CDN_PLATFORM = {"macos-arm64": "macos-arm64", "windows-x86_64": "windows-x64"}


def cdn_version_dir(version: str) -> str:
    """``0.2.0`` -> ``v0-2-0``: CDN keys allow only lowercase letters, digits and hyphens."""
    return "v" + re.sub(r"[^0-9a-z]+", "-", version.lower())


def generate(
    *, version: str, platform: str, binaries: dict[str, Path], out_dir: Path, source: str = "cdn"
) -> Path:
    if platform not in PLATFORMS:
        raise SystemExit(f"unsupported platform {platform!r}; expected one of {', '.join(PLATFORMS)}")
    ext = ".exe" if platform.startswith("windows") else ""
    if source == "cdn":
        label, base = CDN_PLATFORM[platform], f"{CDN}/{cdn_version_dir(version)}"
    else:
        label, base = platform, f"{GITHUB}/v{version}"
    out_dir.mkdir(parents=True, exist_ok=True)
    manifest = {}
    for name, path in binaries.items():
        if not path.is_file():
            raise SystemExit(f"binary not found for {name}: {path}")
        asset = f"{name}-{label}{ext}"
        shutil.copyfile(path, out_dir / asset)
        manifest[name] = {
            "version": version,
            "url": f"{base}/{asset}",
            "sha256": hashlib.sha256((out_dir / asset).read_bytes()).hexdigest(),
            "release_notes": None,
        }
    manifest_path = out_dir / f"meshy-dcc-update-manifest-{label}.json"
    # Bytes, not text: Path.write_text() would write CRLF on Windows, and the
    # manifest is signed and compared as published bytes.
    manifest_path.write_bytes((json.dumps(manifest, indent=2) + "\n").encode("utf-8"))
    return manifest_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", required=True)
    parser.add_argument("--platform", required=True, choices=PLATFORMS)
    parser.add_argument("--cli-bin", type=Path, required=True)
    parser.add_argument("--server-bin", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--source", choices=SOURCES, default="cdn")
    args = parser.parse_args()
    path = generate(
        version=args.version,
        platform=args.platform,
        binaries={"meshy-dcc": args.cli_bin, "meshy-dcc-server": args.server_bin},
        out_dir=args.out_dir,
        source=args.source,
    )
    print(path.as_posix())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

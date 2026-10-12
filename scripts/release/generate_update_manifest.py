#!/usr/bin/env python3
"""Generate one platform's release files for the Meshy release channel.

Writes, into ``--out-dir``:

- ``meshy-dcc-<platform>[.exe]`` and ``meshy-dcc-server-<platform>[.exe]``,
  copies of the two binaries under their published names;
- ``meshy-dcc-update-manifest-<platform>.json``, mapping each binary to its
  version, download URL and SHA-256.

The same files go to two places, each with its own manifest (``--source``):
the GitHub Release (``.../releases/download/v<version>/``) and the Meshy CDN
(``cdn.meshy.ai/meshy-dcc-core/v<version>/``, manifest also under
``latest/``). The installers try the CDN first and the GitHub Release when the
CDN is unreachable, and accept only the URLs written here for that source.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil

SOURCES = {
    "cdn": "https://cdn.meshy.ai/meshy-dcc-core/v{version}",
    "github": "https://github.com/meshy-dev/meshy-dcc-core/releases/download/v{version}",
}
PLATFORMS = ("macos-arm64", "windows-x86_64")


def generate(
    *, version: str, platform: str, binaries: dict[str, Path], out_dir: Path, source: str = "cdn"
) -> Path:
    if platform not in PLATFORMS:
        raise SystemExit(f"unsupported platform {platform!r}; expected one of {', '.join(PLATFORMS)}")
    ext = ".exe" if platform.startswith("windows") else ""
    out_dir.mkdir(parents=True, exist_ok=True)
    manifest = {}
    for name, path in binaries.items():
        if not path.is_file():
            raise SystemExit(f"binary not found for {name}: {path}")
        asset = f"{name}-{platform}{ext}"
        shutil.copyfile(path, out_dir / asset)
        manifest[name] = {
            "version": version,
            "url": f"{SOURCES[source].format(version=version)}/{asset}",
            "sha256": hashlib.sha256((out_dir / asset).read_bytes()).hexdigest(),
            "release_notes": None,
        }
    manifest_path = out_dir / f"meshy-dcc-update-manifest-{platform}.json"
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
    parser.add_argument("--source", choices=sorted(SOURCES), default="cdn")
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

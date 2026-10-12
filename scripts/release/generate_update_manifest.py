#!/usr/bin/env python3
"""Generate one platform's release files for the Meshy release channel.

Writes, into ``--out-dir``:

- ``meshy-dcc-<platform>[.exe]`` and ``meshy-dcc-server-<platform>[.exe]``,
  copies of the two binaries under their published names;
- ``meshy-dcc-update-manifest-<platform>.json``, mapping each binary to its
  version, download URL and SHA-256.

The directory is uploaded to ``cdn.meshy.ai/meshy-dcc-core/v<version>/``, and
the manifest also to ``latest/``. The install scripts and the gateway's
update check read the manifest; the installers accept only the URLs written
here.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil

BASE_URL = "https://cdn.meshy.ai/meshy-dcc-core"
PLATFORMS = ("macos-arm64", "windows-x86_64")


def generate(*, version: str, platform: str, binaries: dict[str, Path], out_dir: Path) -> Path:
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
            "url": f"{BASE_URL}/v{version}/{asset}",
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
    args = parser.parse_args()
    path = generate(
        version=args.version,
        platform=args.platform,
        binaries={"meshy-dcc": args.cli_bin, "meshy-dcc-server": args.server_bin},
        out_dir=args.out_dir,
    )
    print(path.as_posix())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

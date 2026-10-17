#!/usr/bin/env python3
"""Make an install catalog that serves locally built adapter packages.

  local_test_catalog.py --builds <dir> --base-url http://127.0.0.1:<port>/ --out <file>
                        [--catalog meshy-dcc-catalog.yml] [--platform macos-arm64|windows-x86_64]

<dir> holds adapter zips under their release names
(meshy-dcc-<host>-<macos-arm64|windows-x64>.zip, as scripts/build_release.py in
meshy-dcc-adapters writes them). Every entry with a zip for the platform gets
that zip's URL under --base-url and its SHA-256, no mirrors, and is made
installable; the rest stay as they are. Serve <dir> on loopback
(python -m http.server <port> --bind 127.0.0.1 --directory <dir>) and pass the
output to `meshy-dcc install --catalog`. For tests only: never publish it.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import platform as host_platform

import yaml

CDN_PLATFORM = {"macos-arm64": "macos-arm64", "windows-x86_64": "windows-x64"}


def this_platform() -> str:
    return "windows-x86_64" if host_platform.system() == "Windows" else "macos-arm64"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--builds", type=Path, required=True)
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--catalog", type=Path, default=Path(__file__).resolve().parents[2] / "meshy-dcc-catalog.yml")
    parser.add_argument("--platform", choices=sorted(CDN_PLATFORM), default=this_platform())
    args = parser.parse_args()

    catalog = yaml.safe_load(args.catalog.read_text(encoding="utf-8"))
    base = args.base_url.rstrip("/") + "/"
    served = []
    for entry in catalog["entries"]:
        build = ((entry.get("install") or {}).get("platforms") or {}).get(args.platform)
        host = entry["name"].removeprefix("meshy-dcc-")
        package = args.builds / f"meshy-dcc-{host}-{CDN_PLATFORM[args.platform]}.zip"
        if build is None or not package.is_file():
            continue
        build["url"] = base + package.name
        build["sha256"] = hashlib.sha256(package.read_bytes()).hexdigest()
        build.pop("mirrors", None)
        entry["policy"] = {"installation": "available"}
        served.append(host)
    args.out.write_text(json.dumps(catalog, indent=2), encoding="utf-8")
    print(f"{args.out}: {len(served)} installable ({', '.join(served) or 'none'})")


if __name__ == "__main__":
    main()

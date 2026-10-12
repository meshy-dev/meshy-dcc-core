#!/usr/bin/env python3
"""Build the npm packages: `meshy-dcc` plus one binary package per platform.

  build_npm_packages.py --version 0.2.0 --out <dir> \\
      [--macos-arm64 <dir with meshy-dcc, meshy-dcc-server>] \\
      [--windows-x86_64 <dir with meshy-dcc.exe, meshy-dcc-server.exe>]

`meshy-dcc` holds only the launcher (npm/meshy-dcc/bin/meshy-dcc.js) and lists
the platform packages as optional dependencies, so npm installs just the one
for the machine. Each platform package carries both binaries and the
package-manager marker that makes the CLI leave upgrades to npm. Writes one
tarball per package into --out with `npm pack`.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
SCOPE = "@meshy-ai"
# (release platform, npm os, npm cpu, executable suffix)
PLATFORMS = {
    "macos-arm64": ("darwin", "arm64", ""),
    "windows-x86_64": ("win32", "x64", ".exe"),
}
COMMON = {
    "license": "MIT",
    "homepage": "https://www.meshy.ai",
    "repository": {"type": "git", "url": "git+https://github.com/meshy-dev/meshy-dcc-core.git"},
}


def platform_package(version: str, platform: str, bin_dir: Path, stage: Path) -> Path:
    os_name, cpu, ext = PLATFORMS[platform]
    name = f"{SCOPE}/meshy-dcc-{os_name}-{cpu}"
    pkg = stage / f"meshy-dcc-{os_name}-{cpu}"
    (pkg / "bin").mkdir(parents=True)
    for binary in ("meshy-dcc", "meshy-dcc-server"):
        source = bin_dir / f"{binary}{ext}"
        if not source.is_file():
            raise SystemExit(f"missing {source}")
        shutil.copy2(source, pkg / "bin" / source.name)
        (pkg / "bin" / source.name).chmod(0o755)
    marker = {
        "schema_version": 1,
        "distribution": "meshy-dcc",
        "manager": "npm",
        "version": version,
        "platform": platform,
        "binary": f"meshy-dcc{ext}",
    }
    (pkg / "bin" / "meshy-dcc.package-manager.json").write_text(json.dumps(marker, indent=2) + "\n")
    manifest = {
        "name": name,
        "version": version,
        "description": f"meshy-dcc binaries for {platform}",
        **COMMON,
        "os": [os_name],
        "cpu": [cpu],
        "files": ["bin/", "LICENSE", "NOTICE"],
        "preferUnplugged": True,
    }
    (pkg / "package.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return pkg


def main_package(version: str, platforms: list[str], stage: Path) -> Path:
    pkg = stage / "meshy-dcc"
    shutil.copytree(ROOT / "npm" / "meshy-dcc", pkg)
    (pkg / "bin" / "meshy-dcc.js").chmod(0o755)
    manifest = {
        "name": "meshy-dcc",
        "version": version,
        "description": "Let AI agents work inside running Blender, Unity, Godot and Unreal.",
        **COMMON,
        "bin": {"meshy-dcc": "bin/meshy-dcc.js"},
        "files": ["bin/", "README.md", "LICENSE", "NOTICE"],
        "engines": {"node": ">=18"},
        "optionalDependencies": {
            f"{SCOPE}/meshy-dcc-{PLATFORMS[p][0]}-{PLATFORMS[p][1]}": version for p in platforms
        },
    }
    (pkg / "package.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return pkg


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", required=True)
    parser.add_argument("--out", type=Path, required=True)
    for platform in PLATFORMS:
        parser.add_argument(f"--{platform}", type=Path, metavar="DIR")
    args = parser.parse_args()
    given = {p: getattr(args, p.replace("-", "_")) for p in PLATFORMS}
    platforms = [p for p, d in given.items() if d]
    if not platforms:
        raise SystemExit("pass at least one platform binary directory")

    args.out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temp:
        stage = Path(temp)
        packages = [platform_package(args.version, p, given[p], stage) for p in platforms]
        packages.append(main_package(args.version, platforms, stage))
        for pkg in packages:
            for legal in ("LICENSE", "NOTICE"):
                shutil.copy2(ROOT / legal, pkg / legal)
            subprocess.run(
                ["npm", "pack", "--pack-destination", str(args.out.resolve())],
                cwd=pkg,
                check=True,
                stdout=subprocess.DEVNULL,
            )
    for tarball in sorted(args.out.glob("*.tgz")):
        print(tarball.as_posix())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

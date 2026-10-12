#!/usr/bin/env python3
"""Copy a release to the Meshy CDN bucket (s3://meshy-cdn/meshy-dcc-core/).

  sync_cdn.py --version 0.2.0 --release-dir <dir> [--install-script <file> ...]

Uses the AWS CLI with the credentials of the release role (the workflow
assumes it through GitHub OIDC). cdn.meshy.ai and cdn.meshy.cn both serve
this bucket.

- Files in --release-dir go to ``<version dir>/`` (``v0-2-0`` for 0.2.0) with
  a conditional write, so a release file is never overwritten; a rerun that
  finds the same bytes already there moves on.
- Install scripts go to ``meshy-dcc-core/<name>`` with ``no-cache``.
- ``latest.txt`` is written last, ``no-cache``, and only for a stable version
  at least as new as the one it names.

Every name is checked against the CDN naming rules (lowercase letters,
digits, hyphens, one extension) before anything is uploaded.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import sys
import tempfile

BUCKET = "meshy-cdn"
ROOT = "meshy-dcc-core"
KEY_NAME = re.compile(r"^[a-z0-9-]+(\.[a-z0-9]+)?$")
CONTENT_TYPES = {".json": "application/json", ".sh": "text/plain", ".ps1": "text/plain", ".txt": "text/plain"}
STABLE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")


def cdn_version_dir(version: str) -> str:
    return "v" + re.sub(r"[^0-9a-z]+", "-", version.lower())


def aws(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["aws", *args], capture_output=True, text=True, check=check)


def content_type(path: Path) -> str:
    return CONTENT_TYPES.get(path.suffix, "application/octet-stream")


def put_once(path: Path, key: str) -> None:
    """Upload unless the key exists; an existing key must hold the same bytes."""
    result = aws(
        "s3api", "put-object", "--bucket", BUCKET, "--key", key, "--body", str(path),
        "--content-type", content_type(path), "--if-none-match", "*", check=False,
    )
    if result.returncode == 0:
        print(f"uploaded s3://{BUCKET}/{key}")
        return
    if "PreconditionFailed" not in result.stderr:
        sys.exit(f"upload of {key} failed: {result.stderr.strip()}")
    with tempfile.TemporaryDirectory() as temp:
        existing = Path(temp) / "existing"
        aws("s3api", "get-object", "--bucket", BUCKET, "--key", key, str(existing))
        if hashlib.sha256(existing.read_bytes()).digest() != hashlib.sha256(path.read_bytes()).digest():
            sys.exit(f"s3://{BUCKET}/{key} already exists with different bytes; release files are never overwritten")
    print(f"already there s3://{BUCKET}/{key}")


def put_uncached(path: Path, key: str) -> None:
    aws("s3", "cp", str(path), f"s3://{BUCKET}/{key}", "--content-type", content_type(path),
        "--cache-control", "no-cache")
    print(f"uploaded s3://{BUCKET}/{key} (no-cache)")


def current_latest() -> tuple[int, ...] | None:
    result = aws("s3", "cp", f"s3://{BUCKET}/{ROOT}/latest.txt", "-", check=False)
    match = STABLE.match(result.stdout.strip()) if result.returncode == 0 else None
    return tuple(map(int, match.groups())) if match else None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", required=True)
    parser.add_argument("--release-dir", type=Path, required=True)
    parser.add_argument("--install-script", type=Path, action="append", default=[])
    args = parser.parse_args()

    files = sorted(p for p in args.release_dir.iterdir() if p.is_file())
    for path in files + args.install_script:
        if not KEY_NAME.match(path.name):
            sys.exit(f"{path.name} does not follow the CDN naming rules (lowercase letters, digits, hyphens)")

    prefix = f"{ROOT}/{cdn_version_dir(args.version)}"
    for path in files:
        put_once(path, f"{prefix}/{path.name}")
    for path in args.install_script:
        put_uncached(path, f"{ROOT}/{path.name}")

    stable = STABLE.match(args.version)
    if not stable:
        print(f"{args.version} is a prerelease; latest.txt is left as it is")
        return 0
    version = tuple(map(int, stable.groups()))
    latest = current_latest()
    if latest is not None and latest > version:
        print(f"latest.txt names a newer release ({'.'.join(map(str, latest))}); left as it is")
        return 0
    with tempfile.TemporaryDirectory() as temp:
        latest_file = Path(temp) / "latest.txt"
        latest_file.write_text(args.version + "\n")
        put_uncached(latest_file, f"{ROOT}/latest.txt")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Upload release files to cdn.meshy.ai through cdnmgr presigned URLs.

  sync_cdn.py --version 0.2.0 --release-dir <dir> [--install-script <file> ...]

Needs CDN_API_KEY (a cdnk_ key; upload-only). Files in --release-dir go to
``meshy-dcc-core/<version dir>/`` (``v0-2-0`` for 0.2.0) and are never
overwritten: an existing key with different bytes fails the run. Install
scripts go to ``meshy-dcc-core/<name>`` and are replaced only when their bytes
changed; CloudFront keeps serving the old copy until someone invalidates the
path in the cdnmgr web UI (upload keys cannot), so the run says which paths
to invalidate.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import urllib.error
import urllib.request

CDNMGR = "https://cdn.meshy.team"
PUBLIC = "https://cdn.meshy.ai"
ROOT = "meshy-dcc-core"
KEY_NAME = re.compile(r"^[a-z0-9-]+(\.[a-z0-9]+)?$")
CONTENT_TYPES = {".json": "application/json", ".sh": "text/plain", ".ps1": "text/plain"}


def cdn_version_dir(version: str) -> str:
    return "v" + re.sub(r"[^0-9a-z]+", "-", version.lower())


def api(path: str, payload: dict, key: str) -> dict:
    request = urllib.request.Request(
        f"{CDNMGR}{path}",
        data=json.dumps(payload).encode(),
        headers={"Authorization": f"Bearer {key}", "content-type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def published_sha256(s3_key: str) -> str | None:
    try:
        with urllib.request.urlopen(f"{PUBLIC}/{s3_key}", timeout=60) as response:
            return hashlib.sha256(response.read()).hexdigest()
    except urllib.error.HTTPError as error:
        if error.code in (403, 404):  # a missing object is a 403 on this CDN
            return None
        raise


def upload(path: Path, s3_key: str, key: str) -> None:
    content_type = CONTENT_TYPES.get(path.suffix, "application/octet-stream")
    url = api("/api/s3/upload-url", {"key": s3_key, "contentType": content_type}, key)["url"]
    request = urllib.request.Request(
        url, data=path.read_bytes(), headers={"Content-Type": content_type}, method="PUT"
    )
    with urllib.request.urlopen(request, timeout=600):
        pass
    print(f"uploaded {PUBLIC}/{s3_key}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", required=True)
    parser.add_argument("--release-dir", type=Path, required=True)
    parser.add_argument("--install-script", type=Path, action="append", default=[])
    args = parser.parse_args()
    key = os.environ.get("CDN_API_KEY", "")
    if not key:
        sys.exit("CDN_API_KEY is not set")

    files = sorted(p for p in args.release_dir.iterdir() if p.is_file())
    for path in files + args.install_script:
        if not KEY_NAME.match(path.name):
            sys.exit(f"{path.name} does not follow the CDN naming rules (lowercase letters, digits, hyphens)")

    prefix = f"{ROOT}/{cdn_version_dir(args.version)}"
    keys = [f"{prefix}/{p.name}" for p in files]
    existing = set(api("/api/s3/check-exists", {"keys": keys}, key).get("existingKeys", []))
    for path, s3_key in zip(files, keys):
        if s3_key in existing:
            if published_sha256(s3_key) != hashlib.sha256(path.read_bytes()).hexdigest():
                sys.exit(f"{s3_key} already exists with different bytes; release files are never overwritten")
            print(f"already published {PUBLIC}/{s3_key}")
            continue
        upload(path, s3_key, key)

    stale = []
    for path in args.install_script:
        s3_key = f"{ROOT}/{path.name}"
        if published_sha256(s3_key) == hashlib.sha256(path.read_bytes()).hexdigest():
            print(f"unchanged {PUBLIC}/{s3_key}")
            continue
        upload(path, s3_key, key)
        stale.append(f"/{s3_key}")
    if stale:
        print(
            "::warning::Invalidate in the cdnmgr web UI (https://cdn.meshy.team, Invalidations), "
            "or the old copy keeps being served: " + " ".join(stale)
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

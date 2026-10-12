#!/usr/bin/env python3
"""Remove build-machine paths from release artifacts, then prove none are left.

  scrub_artifacts.py <repo_root> <out_dir>

The wheel's CycloneDX SBOM names workspace crates by their cargo package ID,
``path+file://<checkout>/crates/...``; those are rewritten to
``/meshy-dcc-core`` and the wheel's RECORD is updated to match. Every file in
<out_dir>, and every member of each wheel or zip there, is then searched for
the checkout path and $HOME; any hit fails the build.
"""
import base64
import hashlib
import os
import sys
import zipfile
from pathlib import Path

NEUTRAL = b"/meshy-dcc-core"


def record_hash(data):
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=").decode()
    return f"sha256={digest}"


def scrub_wheel(wheel, repo):
    with zipfile.ZipFile(wheel) as zin:
        infos = zin.infolist()
        members = {info.filename: zin.read(info) for info in infos}
    changed = {}
    for name, data in members.items():
        if "/sboms/" in name and repo in data:
            changed[name] = data.replace(repo, NEUTRAL)
    if not changed:
        return 0
    record_name = next(name for name in members if name.endswith(".dist-info/RECORD"))
    lines = []
    for line in members[record_name].decode().splitlines():
        name = line.split(",", 1)[0]
        if name in changed:
            data = changed[name]
            line = f"{name},{record_hash(data)},{len(data)}"
        lines.append(line)
    changed[record_name] = ("\n".join(lines) + "\n").encode()
    tmp = wheel.with_suffix(".tmp")
    with zipfile.ZipFile(tmp, "w") as zout:
        for info in infos:
            zout.writestr(info, changed.get(info.filename, members[info.filename]))
    tmp.replace(wheel)
    return len(changed) - 1


def leaks(path, needles):
    found = []
    data = path.read_bytes()
    found += [f"{path.name}: {n.decode()}" for n in needles if n in data]
    if path.suffix in (".whl", ".zip"):
        with zipfile.ZipFile(path) as z:
            for info in z.infolist():
                member = z.read(info)
                found += [f"{path.name}!{info.filename}: {n.decode()}" for n in needles if n in member]
    return found


def main(repo_root, out_dir):
    repo = str(Path(repo_root).resolve()).encode()
    out = Path(out_dir)
    for wheel in sorted(out.glob("*.whl")):
        print(f"{wheel.name}: rewrote {scrub_wheel(wheel, repo)} SBOM file(s)")
    needles = [repo, os.environ["HOME"].encode()]
    found = [hit for path in sorted(out.iterdir()) if path.is_file() for hit in leaks(path, needles)]
    if found:
        sys.exit("build-machine paths left in release artifacts:\n  " + "\n  ".join(found))
    print(f"no build-machine paths in {out}")


if __name__ == "__main__":
    main(*sys.argv[1:3])

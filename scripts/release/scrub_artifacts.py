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
import csv
import hashlib
import io
import os
import subprocess
import sys
import tempfile
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
    replace_wheel_members(wheel, infos, members, changed)
    return len(changed)


def replace_wheel_members(wheel, infos, members, changed):
    record_name = next(name for name in members if name.endswith(".dist-info/RECORD"))
    records = list(csv.reader(io.StringIO(members[record_name].decode())))
    for row in records:
        name = row[0]
        if name in changed:
            data = changed[name]
            row[:] = [name, record_hash(data), str(len(data))]
    output = io.StringIO()
    csv.writer(output, lineterminator="\n").writerows(records)
    updates = dict(changed)
    updates[record_name] = output.getvalue().encode()
    tmp = wheel.with_suffix(".tmp")
    with zipfile.ZipFile(tmp, "w") as zout:
        for info in infos:
            zout.writestr(info, updates.get(info.filename, members[info.filename]))
    tmp.replace(wheel)


def normalize_macos_symbols(wheel, needles):
    """Apple strip removes local/debug paths without LLVM's malformed layout."""
    if sys.platform != "darwin":
        return 0
    with zipfile.ZipFile(wheel) as archive:
        infos = archive.infolist()
        members = {info.filename: archive.read(info) for info in infos}
    changed = {}
    for name, data in members.items():
        if not name.endswith((".so", ".dylib")) or not any(n in data for n in needles.values()):
            continue
        with tempfile.TemporaryDirectory(prefix="meshy-wheel-symbols-") as scratch:
            native = Path(scratch) / Path(name).name
            native.write_bytes(data)
            subprocess.run(["/usr/bin/strip", "-x", str(native)], check=True)
            subprocess.run(["/usr/bin/codesign", "--force", "--sign", "-", str(native)], check=True)
            changed[name] = native.read_bytes()
    if changed:
        replace_wheel_members(wheel, infos, members, changed)
    return len(changed)


def leaks(path, needles):
    found = []
    data = path.read_bytes()
    found += [f"{path.name}: {label}" for label, n in needles.items() if n in data]
    if path.suffix in (".whl", ".zip"):
        with zipfile.ZipFile(path) as z:
            for info in z.infolist():
                member = z.read(info)
                found += [f"{path.name}!{info.filename}: {label}" for label, n in needles.items() if n in member]
    return found


def path_needles(*paths):
    """Each path as UTF-8; on Windows also its 8.3 short form (C code built
    from short paths keeps them) and UTF-16 (wide strings in binaries)."""
    forms = set(paths)
    if os.name == "nt":
        import ctypes

        for path in paths:
            buf = ctypes.create_unicode_buffer(32768)
            if ctypes.windll.kernel32.GetShortPathNameW(path, buf, len(buf)):
                forms.add(buf.value)
    needles = {form: form.encode() for form in forms}
    if os.name == "nt":
        needles.update({f"{form} (UTF-16)": form.encode("utf-16-le") for form in forms})
    return needles


def verify_native_wheel(wheel):
    """Install the exact wheel offline and exercise native init, without hosts."""
    with tempfile.TemporaryDirectory(prefix="meshy-wheel-load-") as scratch:
        root = Path(scratch)
        venv = root / "venv"
        subprocess.run([sys.executable, "-m", "venv", str(venv)], check=True)
        python = venv / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        env = dict(os.environ, PIP_DISABLE_PIP_VERSION_CHECK="1", PYTHONDONTWRITEBYTECODE="1",
                   MESHY_DCC_DATA_DIR=str(root / "data"), MESHY_DCC_REGISTRY_DIR=str(root / "registry"),
                   MESHY_DCC_LOG_DIR=str(root / "logs"), MESHY_DCC_LOG_LEVEL="off", MESHY_DCC_TELEMETRY="0",
                   MESHY_DCC_DISABLE_UPDATE_CHECK="1")
        subprocess.run([str(python), "-m", "pip", "--isolated", "--disable-pip-version-check", "--no-cache-dir",
                        "install", "--no-index", "--no-deps", str(wheel.resolve())], env=env, check=True)
        code = ("import importlib.machinery; import meshy_dcc_core._core as core; "
                "assert isinstance(core.__loader__, importlib.machinery.ExtensionFileLoader), "
                "'wheel did not load a native extension'; print('native wheel import passed')")
        subprocess.run([str(python), "-I", "-B", "-c", code], env=env, check=True)


def main(repo_root, out_dir):
    repo = str(Path(repo_root).resolve()).encode()
    out = Path(out_dir)
    needles = path_needles(repo.decode(), os.environ["HOME"])
    for wheel in sorted(out.glob("*.whl")):
        print(f"{wheel.name}: normalized {normalize_macos_symbols(wheel, needles)} native file(s)")
        print(f"{wheel.name}: rewrote {scrub_wheel(wheel, repo)} SBOM file(s)")
    found = [hit for path in sorted(out.iterdir()) if path.is_file() for hit in leaks(path, needles)]
    if found:
        sys.exit("build-machine paths left in release artifacts:\n  " + "\n  ".join(found))
    print(f"no build-machine paths in {out}")
    for wheel in sorted(out.glob("*.whl")):
        verify_native_wheel(wheel)


if __name__ == "__main__":
    main(*sys.argv[1:3])

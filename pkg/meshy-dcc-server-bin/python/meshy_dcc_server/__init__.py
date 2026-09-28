"""Locator helpers for the bundled ``meshy-dcc-server`` Rust binary.

Usage::

    from meshy_dcc_server import binary_path
    import subprocess
    subprocess.Popen([str(binary_path()), "sidecar", "--dcc", "maya", ...])

Most users do not need this — after ``pip install meshy-dcc-server`` the
binary is on ``PATH`` and can be invoked directly. This helper exists
for DCC plugins / addons that want a stable absolute path to
``subprocess.Popen``.
"""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import sys

__all__ = ["__version__", "binary_path"]

# Kept in sync with pyproject.toml / Rust crate via release-please.
__version__ = "0.2.0"

_BINARY_NAME = "meshy-dcc-server.exe" if os.name == "nt" else "meshy-dcc-server"


def binary_path() -> Path:
    """Return the filesystem path of the bundled ``meshy-dcc-server`` binary.

    Resolution order:

    1. ``MESHY_DCC_SERVER_BIN`` env var — operator override.
    2. ``scripts/`` directory next to this package's installation (where
       maturin places ``bindings = "bin"`` artefacts inside the wheel).
    3. ``sys.prefix/Scripts`` (Windows) or ``sys.prefix/bin`` (POSIX) —
       the scripts directory for the *current* interpreter environment.
    4. ``shutil.which("meshy-dcc-server")`` — a final system PATH fallback.

    Raises:
        FileNotFoundError: if no installation of the binary is found.

    """
    override = os.environ.get("MESHY_DCC_SERVER_BIN")
    if override:
        p = Path(override).expanduser()
        if p.is_file():
            return p

    package_dir = Path(__file__).resolve().parent
    candidate = package_dir.parent / "scripts" / _BINARY_NAME
    if candidate.is_file():
        return candidate

    scripts_dir = Path(sys.prefix) / ("Scripts" if os.name == "nt" else "bin")
    candidate = scripts_dir / _BINARY_NAME
    if candidate.is_file():
        return candidate

    on_path = shutil.which("meshy-dcc-server")
    if on_path:
        return Path(on_path)

    raise FileNotFoundError(
        "meshy-dcc-server binary not found. Did you `pip install meshy-dcc-server`? "
        "You can also set the MESHY_DCC_SERVER_BIN env var to an explicit path."
    )

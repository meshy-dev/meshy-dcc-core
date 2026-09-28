from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys


def test_native_debug_logs_do_not_pollute_stdout() -> None:
    root = Path(__file__).resolve().parent.parent
    env = os.environ.copy()
    env["MESHY_DCC_LOG_LEVEL"] = "debug"

    result = subprocess.run(
        [
            sys.executable,
            "-c",
            "from meshy_dcc_core import PySharedBuffer; PySharedBuffer.create(capacity=1024)",
        ],
        cwd=root,
        env=env,
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )

    assert result.stdout == ""
    assert "SharedBuffer created" in result.stderr

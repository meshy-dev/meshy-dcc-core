# meshy-dcc-server-bin — PyPI distribution blueprint

> **Status:** blueprint — packaging files are wired but no PyPI release has
> shipped yet. Tracked in issue **#1002** (deliverable 3 of RFC #998
> Addendum A.7).

This directory packages the `crates/meshy-dcc-server` Rust binary as a
platform-specific **binary-only** PyPI wheel, following the same pattern as
`ruff`, `uv`, `cmake`, and `pyright`. The result is a single
`pip install meshy-dcc-server` that drops the gateway / sidecar / translate
CLI onto `PATH` for Python 3.7+ regardless of which DCC the user runs.

## Why a separate PyPI package?

`meshy-dcc-core` is a PyO3 wheel — its `_core.so` is loaded into the host
Python interpreter (mayapy / blender-python / hython). The sidecar binary,
by contrast, is meant to run as its **own** OS process; bundling it into
`meshy-dcc-core` would couple two artefacts with very different release
cadences and ABI matrices. Splitting them is the standard pattern.

| Package | Distributes | Audience |
|---|---|---|
| `meshy-dcc-core` (existing) | PyO3 wheel (`_core.so` + Python facade) | Skill authors, plugin/addon code running *inside* a DCC interpreter |
| `meshy-dcc-server` (this dir) | platform-specific Python 3.7+ binary wheels | Operators, sidecar spawners, anyone who wants a standalone gateway |
| `meshy-dcc-<dcc>` (each repo) | pure-Python plugin/addon glue | DCC plugin loaders (`userSetup.py`, addon `register()`, …) |

## Layout

```
pkg/meshy-dcc-server-bin/
├── pyproject.toml              ← maturin config, bindings = "bin"
├── python/
│   └── meshy_dcc_server/
│       └── __init__.py         ← binary_path() helper for subprocess spawn
└── README.md                   ← this file
```

The Rust source is **not duplicated** here — `pyproject.toml` sets
`manifest-path = "../../crates/meshy-dcc-server/Cargo.toml"` so maturin
builds the existing workspace crate.

## Local build

```bash
# Build a wheel for the current platform / Python
cd pkg/meshy-dcc-server-bin/
vx pip install maturin
vx maturin build --release

# Resulting wheel lands in ../../target/wheels/meshy_dcc_server-*.whl
vx pip install ../../target/wheels/meshy_dcc_server-*.whl
meshy-dcc-server --help
```

The wheel uses maturin `bindings = "bin"`, so it does not load a Python
extension module and has no CPython ABI dependency. Its metadata deliberately
declares `Requires-Python: >=3.7` so embedded Python 3.7 hosts such as Maya
2022 can install it directly.

## Cross-platform CI release

The new `.github/workflows/release-server-binary.yml` workflow (also part
of this PR) builds wheels for:

| OS | Arch | Tag |
|---|---|---|
| manylinux | x86_64 | `manylinux_2_28_x86_64` |
| manylinux | aarch64 | `manylinux_2_28_aarch64` |
| Windows | x86_64 | `win_amd64` |
| Windows | arm64 | `win_arm64` |
| macOS | x86_64 | `macosx_11_0_x86_64` |
| macOS | arm64 | `macosx_11_0_arm64` |

The workflow triggers on tags matching `meshy-dcc-server-v*` so it stays
independent of the existing `meshy-dcc-core` release cycle.

## Usage from a DCC plugin

```python
# In a Maya plugin / Blender addon, after `pip install meshy-dcc-server`:
import os, subprocess
from meshy_dcc_server import binary_path

_proc = subprocess.Popen([
    str(binary_path()),
    "sidecar",
    "--dcc", "maya",
    "--host-rpc", "commandport://127.0.0.1:6000",
    "--watch-pid", str(os.getpid()),
])
```

That's the entire plugin → sidecar wiring. Per-DCC `HostRpcClient`
implementations land in their respective adapter repos.

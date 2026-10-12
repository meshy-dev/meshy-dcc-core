#!/usr/bin/env bash
# Release build for this machine's platform: the CLI, the gateway and the core
# wheel, with the `dist` profile and no paths of the build machine inside.
#
#   scripts/release/build_release.sh [out_dir]      (default: dist/release)
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"
out="${1:-dist/release}"
mkdir -p "$out"

# Panic locations and file!() keep absolute paths of dependencies and of this
# checkout; map them to fixed prefixes.
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
export RUSTFLAGS="--remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$repo=/meshy-dcc-core ${RUSTFLAGS:-}"
# Git Bash on Windows: rustc sees C:\... paths, not /c/....
if command -v cygpath >/dev/null 2>&1; then
    RUSTFLAGS="--remap-path-prefix=$(cygpath -w "$cargo_home")=/cargo --remap-path-prefix=$(cygpath -w "$repo")=/meshy-dcc-core $RUSTFLAGS"
fi

cargo build --profile dist -p meshy-dcc -p meshy-dcc-server
exe=""
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) exe=".exe" ;; esac
cp "target/dist/meshy-dcc$exe" "target/dist/meshy-dcc-server$exe" "$out/"

# Features come from [tool.maturin] in pyproject.toml.
uvx --from 'maturin>=1.0,<2.0' maturin build --profile dist --out "$out"

python3 scripts/release/scrub_artifacts.py "$repo" "$out"

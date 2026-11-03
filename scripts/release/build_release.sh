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
# checkout; map them to fixed prefixes. With the rust-src component installed,
# std's own paths point into the toolchain directory too.
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
sysroot="$(rustc --print sysroot)"
export RUSTFLAGS="--remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$sysroot=/rust --remap-path-prefix=$repo=/meshy-dcc-core ${RUSTFLAGS:-}"
# Git Bash on Windows: rustc sees C:\... paths, not /c/....
if command -v cygpath >/dev/null 2>&1; then
    RUSTFLAGS="--remap-path-prefix=$(cygpath -w "$cargo_home")=/cargo --remap-path-prefix=$(cygpath -w "$repo")=/meshy-dcc-core $RUSTFLAGS"
    # C dependencies (aws-lc's asserts) keep __FILE__ as the absolute source
    # path; aws-lc-sys maps it only for gcc and clang, and MSVC trims a prefix
    # with /d1trimfile. aws-lc-sys compiles from the 8.3 short form of its
    # directory, so trim that form too. Shell-escaped so spaces survive.
    export CC_SHELL_ESCAPED_FLAGS=1
    export CFLAGS="\"/d1trimfile:$(cygpath -w "$cargo_home")\" \"/d1trimfile:$(cygpath -d "$cargo_home")\" ${CFLAGS:-}"
fi

exe=""
python=python3
# Windows Pythons (python.org, setup-python) have no python3 command.
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) exe=".exe"; python=python ;; esac
# Features come from [tool.maturin] in pyproject.toml.
# Build the wheel first: maturin adds macOS dynamic-lookup rustflags, so doing
# native builds first invalidates the warm wheel dependency fingerprints.
if [[ "$(uname -s)" == "Darwin" ]]; then
    # Rust 1.95's Darwin cdylib stripping can misalign LINKEDIT string tables,
    # rejected by macOS 27 dyld (rust-lang/rust#158410). Keep the wheel unstripped
    # until the pinned toolchain includes the fix; executable profiles stay intact.
    CARGO_PROFILE_DIST_STRIP=none uvx --from 'maturin>=1.0,<2.0' maturin build --profile dist --out "$out"
else
    uvx --from 'maturin>=1.0,<2.0' maturin build --profile dist --out "$out"
fi

cargo build --profile dist -p meshy-dcc -p meshy-dcc-server
cp "target/dist/meshy-dcc$exe" "target/dist/meshy-dcc-server$exe" "$out/"

"$python" scripts/release/scrub_artifacts.py "$repo" "$out"

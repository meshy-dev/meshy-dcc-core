#!/usr/bin/env sh
set -eu

OFFICIAL_RELEASES="https://github.com/meshy-dev/meshy-dcc-core/releases"
VERSION="${MESHY_DCC_VERSION:-latest}"
INSTALL_DIR="${MESHY_DCC_INSTALL_DIR:-$HOME/.local/bin}"

usage() {
    cat <<'EOF'
Install meshy-dcc from its official GitHub release manifest.

Usage:
  install-cli.sh [--version v0.19.63] [--install-dir ~/.local/bin]

Download this script first, inspect it, then run the local file.
The installer verifies the release-manifest URL and SHA-256 before replacing
an existing meshy-dcc binary.

Environment:
  MESHY_DCC_VERSION      Release version, default latest
  MESHY_DCC_INSTALL_DIR  Install directory, default ~/.local/bin
EOF
}

fail() {
    echo "Error: $*" >&2
    exit 1
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            [ "$#" -ge 2 ] || fail "--version requires a value"
            VERSION="$2"
            shift 2
            ;;
        --install-dir)
            [ "$#" -ge 2 ] || fail "--install-dir requires a value"
            INSTALL_DIR="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS" in
    Linux)
        if [ "$ARCH" != "x86_64" ] && [ "$ARCH" != "amd64" ]; then
            fail "unsupported Linux architecture: $ARCH"
        fi
        PLATFORM="linux-x86_64"
        ASSET="meshy-dcc-linux-x86_64"
        ;;
    Darwin)
        PLATFORM="macos-universal2"
        ASSET="meshy-dcc-macos-universal2"
        ;;
    *)
        fail "unsupported OS: $OS"
        ;;
esac

REQUESTED_VERSION="${VERSION#v}"
if [ "$VERSION" = "latest" ]; then
    MANIFEST_URL="$OFFICIAL_RELEASES/latest/download/meshy-dcc-update-manifest-$PLATFORM.json"
else
    printf '%s' "$REQUESTED_VERSION" \
        | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?$' \
        || fail "invalid release version: $VERSION"
    MANIFEST_URL="$OFFICIAL_RELEASES/download/v$REQUESTED_VERSION/meshy-dcc-update-manifest-$PLATFORM.json"
fi

mkdir -p "$INSTALL_DIR"
MANIFEST_TMP="$(mktemp "$INSTALL_DIR/.meshy-dcc-manifest.XXXXXX")"
BINARY_TMP="$(mktemp "$INSTALL_DIR/.meshy-dcc.XXXXXX")"
cleanup() {
    rm -f "$MANIFEST_TMP" "$BINARY_TMP"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

download_to() {
    url="$1"
    output="$2"
    echo "Downloading $url"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$output"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O "$output" "$url"
    else
        fail "curl or wget is required to download release assets"
    fi
}

manifest_field() {
    field="$1"
    awk -v field="$field" '
        /^[[:space:]]*"meshy-dcc"[[:space:]]*:[[:space:]]*\{[[:space:]]*$/ {
            in_cli = 1
            next
        }
        in_cli && /^[[:space:]]*\}[,]?[[:space:]]*$/ { exit }
        in_cli && $0 ~ "^[[:space:]]*\"" field "\"[[:space:]]*:" {
            value = $0
            sub("^[^:]*:[[:space:]]*\"", "", value)
            sub("\"[[:space:]]*,?[[:space:]]*$", "", value)
            print value
        }
    ' "$MANIFEST_TMP"
}

calculate_sha256() {
    path="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$path" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$path" | awk '{print $1}'
    elif command -v openssl >/dev/null 2>&1; then
        openssl dgst -sha256 "$path" | awk '{print $NF}'
    else
        fail "sha256sum, shasum, or openssl is required to verify meshy-dcc"
    fi
}

download_to "$MANIFEST_URL" "$MANIFEST_TMP" \
    || fail "official release manifest download failed"

ENTRY_COUNT="$(grep -Ec '^[[:space:]]*"meshy-dcc"[[:space:]]*:[[:space:]]*\{' "$MANIFEST_TMP" || true)"
[ "$ENTRY_COUNT" = "1" ] || fail "official release manifest must contain one meshy-dcc entry"

MANIFEST_VERSION="$(manifest_field version)"
ASSET_URL="$(manifest_field url)"
EXPECTED_SHA256="$(manifest_field sha256)"
printf '%s' "$MANIFEST_VERSION" \
    | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?$' \
    || fail "official release manifest contains an invalid meshy-dcc version"
if [ "$VERSION" != "latest" ] && [ "$MANIFEST_VERSION" != "$REQUESTED_VERSION" ]; then
    fail "official release manifest version does not match requested version $REQUESTED_VERSION"
fi

EXPECTED_URL="$OFFICIAL_RELEASES/download/v$MANIFEST_VERSION/$ASSET"
[ "$ASSET_URL" = "$EXPECTED_URL" ] \
    || fail "official release manifest contains a non-official meshy-dcc URL"
[ "${#EXPECTED_SHA256}" -eq 64 ] \
    && ! printf '%s' "$EXPECTED_SHA256" | grep -Eq '[^0-9A-Fa-f]' \
    || fail "official release manifest contains an invalid SHA-256"

download_to "$ASSET_URL" "$BINARY_TMP" || fail "meshy-dcc download failed"
ACTUAL_SHA256="$(calculate_sha256 "$BINARY_TMP")"
[ "$(printf '%s' "$ACTUAL_SHA256" | tr 'A-F' 'a-f')" = "$(printf '%s' "$EXPECTED_SHA256" | tr 'A-F' 'a-f')" ] \
    || fail "meshy-dcc SHA-256 does not match the official release manifest"

chmod 0755 "$BINARY_TMP"
TARGET="$INSTALL_DIR/meshy-dcc"
mv -f "$BINARY_TMP" "$TARGET"

# The CLI and dcc-cua are independently released. Reconcile the verified
# companion only after the CLI replacement succeeds. This is convergent, but
# not crash-atomic across the two independently owned executables.
"$TARGET" components ensure dcc-cua --yes --output json \
    || fail "meshy-dcc installed, but verified dcc-cua component installation failed"

echo "Installed verified meshy-dcc $MANIFEST_VERSION to $TARGET"
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) echo "Add $INSTALL_DIR to PATH to run meshy-dcc from any shell." ;;
esac

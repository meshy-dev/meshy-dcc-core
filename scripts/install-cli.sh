#!/usr/bin/env sh
set -eu

OFFICIAL_RELEASES="https://cdn.meshy.ai/meshy-dcc-core"
VERSION="${MESHY_DCC_VERSION:-latest}"
INSTALL_DIR="${MESHY_DCC_INSTALL_DIR:-$HOME/.local/bin}"

usage() {
    cat <<'EOF'
Install meshy-dcc and its gateway from the Meshy release channel.

Usage:
  install-cli.sh [--version 0.2.0] [--install-dir ~/.local/bin]

Download this script first, inspect it, then run the local file.
The installer checks the release manifest's URLs and SHA-256 digests before
replacing an existing meshy-dcc, then installs the meshy-dcc Skill for the
agents it finds (Claude Code, Codex).

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
case "$OS/$ARCH" in
    Darwin/arm64) PLATFORM="macos-arm64" ;;
    Darwin/*) fail "meshy-dcc supports Apple Silicon Macs only (this Mac is $ARCH)" ;;
    *) fail "meshy-dcc is published for macOS (Apple Silicon) and Windows; not for $OS/$ARCH yet" ;;
esac

REQUESTED_VERSION="${VERSION#v}"
if [ "$VERSION" = "latest" ]; then
    MANIFEST_URL="$OFFICIAL_RELEASES/latest/meshy-dcc-update-manifest-$PLATFORM.json"
else
    printf '%s' "$REQUESTED_VERSION" \
        | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?$' \
        || fail "invalid release version: $VERSION"
    MANIFEST_URL="$OFFICIAL_RELEASES/v$REQUESTED_VERSION/meshy-dcc-update-manifest-$PLATFORM.json"
fi

mkdir -p "$INSTALL_DIR"
MANIFEST_TMP="$(mktemp "$INSTALL_DIR/.meshy-dcc-manifest.XXXXXX")"
CLI_TMP="$(mktemp "$INSTALL_DIR/.meshy-dcc.XXXXXX")"
SERVER_TMP="$(mktemp "$INSTALL_DIR/.meshy-dcc-server.XXXXXX")"
cleanup() {
    rm -f "$MANIFEST_TMP" "$CLI_TMP" "$SERVER_TMP"
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

# manifest_field <binary> <field>: one string field of one binary's entry.
manifest_field() {
    awk -v binary="$1" -v field="$2" '
        $0 ~ "^[[:space:]]*\"" binary "\"[[:space:]]*:[[:space:]]*\\{[[:space:]]*$" {
            in_entry = 1
            next
        }
        in_entry && /^[[:space:]]*\}[,]?[[:space:]]*$/ { exit }
        in_entry && $0 ~ "^[[:space:]]*\"" field "\"[[:space:]]*:" {
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

# fetch_verified <binary> <output>: download one binary the manifest names and
# check its version, URL and SHA-256. Sets MANIFEST_VERSION.
fetch_verified() {
    binary="$1"
    output="$2"
    count="$(grep -Ec "^[[:space:]]*\"$binary\"[[:space:]]*:[[:space:]]*\\{" "$MANIFEST_TMP" || true)"
    [ "$count" = "1" ] || fail "official release manifest must contain one $binary entry"

    version="$(manifest_field "$binary" version)"
    url="$(manifest_field "$binary" url)"
    expected="$(manifest_field "$binary" sha256)"
    printf '%s' "$version" \
        | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?$' \
        || fail "official release manifest contains an invalid $binary version"
    if [ "$VERSION" != "latest" ] && [ "$version" != "$REQUESTED_VERSION" ]; then
        fail "official release manifest version does not match requested version $REQUESTED_VERSION"
    fi
    [ "$url" = "$OFFICIAL_RELEASES/v$version/$binary-$PLATFORM" ] \
        || fail "official release manifest contains a non-official $binary URL"
    [ "${#expected}" -eq 64 ] \
        && ! printf '%s' "$expected" | grep -Eq '[^0-9A-Fa-f]' \
        || fail "official release manifest contains an invalid SHA-256"

    download_to "$url" "$output" || fail "$binary download failed"
    actual="$(calculate_sha256 "$output")"
    [ "$(printf '%s' "$actual" | tr 'A-F' 'a-f')" = "$(printf '%s' "$expected" | tr 'A-F' 'a-f')" ] \
        || fail "$binary SHA-256 does not match the official release manifest"
    chmod 0755 "$output"
    MANIFEST_VERSION="$version"
}

# Both are verified before either replaces an installed file.
fetch_verified meshy-dcc "$CLI_TMP"
fetch_verified meshy-dcc-server "$SERVER_TMP"
mv -f "$SERVER_TMP" "$INSTALL_DIR/meshy-dcc-server"
TARGET="$INSTALL_DIR/meshy-dcc"
mv -f "$CLI_TMP" "$TARGET"
echo "Installed verified meshy-dcc $MANIFEST_VERSION to $INSTALL_DIR"

echo "Installing the meshy-dcc Skill for the agents on this machine:"
"$TARGET" --output human agent-skill || true
"$TARGET" --output human agent-skill --execute \
    || echo "The meshy-dcc Skill was not installed for every agent; run: meshy-dcc agent-skill" >&2

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) echo "Add $INSTALL_DIR to PATH to run meshy-dcc from any shell." ;;
esac

#!/usr/bin/env sh
set -eu

# Tried in order: the Meshy CDN, its mainland-China host (cdn.meshy.cn serves
# the same bucket; cdn.meshy.ai does not resolve there), then the GitHub
# Release. A download that fails moves on to the next source; a manifest or
# file that fails its checks stops the install.
CDN="https://cdn.meshy.ai/meshy-dcc-core"
CDN_CN="https://cdn.meshy.cn/meshy-dcc-core"
GITHUB="https://github.com/meshy-dev/meshy-dcc-core/releases"
VERSION="${MESHY_DCC_VERSION:-latest}"
INSTALL_DIR="${MESHY_DCC_INSTALL_DIR:-$HOME/.local/bin}"

usage() {
    cat <<'EOF'
Install meshy-dcc and its gateway from the Meshy CDN, or from the GitHub
Release when the CDN cannot be reached.

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

download_to() {
    url="$1"
    output="$2"
    echo "Downloading $url"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --connect-timeout 15 "$url" -o "$output"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -T 15 -O "$output" "$url"
    else
        fail "curl or wget is required to download release assets"
    fi
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

# The latest version: latest.txt on either CDN host (served uncached), else the
# GitHub Release redirect releases/latest -> releases/tag/v<version>.
if [ "$VERSION" = "latest" ]; then
    LATEST_TMP="$(mktemp "${TMPDIR:-/tmp}/meshy-dcc-latest.XXXXXX")"
    for host in "$CDN" "$CDN_CN"; do
        if download_to "$host/latest.txt" "$LATEST_TMP" 2>/dev/null; then
            VERSION="$(tr -d ' \r\n' < "$LATEST_TMP")"
            break
        fi
    done
    rm -f "$LATEST_TMP"
fi
if [ "$VERSION" = "latest" ]; then
    if command -v curl >/dev/null 2>&1; then
        LATEST_URL="$(curl -fsSIL --connect-timeout 15 -o /dev/null -w '%{url_effective}' "$GITHUB/latest" || true)"
    else
        LATEST_URL="$(wget -q -S -T 15 --spider "$GITHUB/latest" 2>&1 | awk '/^ *Location: /{print $2}' | tail -1)"
    fi
    case "$LATEST_URL" in
        */releases/tag/v*) VERSION="${LATEST_URL##*/releases/tag/}" ;;
        *) fail "could not find the latest release on the Meshy CDN or GitHub; set MESHY_DCC_VERSION to install a given version" ;;
    esac
fi
REQUESTED_VERSION="${VERSION#v}"
printf '%s' "$REQUESTED_VERSION" \
    | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?$' \
    || fail "invalid release version: $VERSION"

# CDN names follow the CDN rules (lowercase letters, digits, hyphens): the
# version dir for 0.2.0 is v0-2-0. GitHub keeps the release names.
CDN_DIR="$CDN/v$(printf '%s' "$REQUESTED_VERSION" | tr 'A-Z' 'a-z' | sed 's/[^0-9a-z][^0-9a-z]*/-/g')"
GITHUB_DIR="$GITHUB/download/v$REQUESTED_VERSION"

# release_url <source> <name without platform>: where <name>-<platform> lives,
# as the manifest names it. Both CDN hosts use the cdn.meshy.ai URLs.
release_url() {
    case "$1" in
        cdn | cdn-cn) echo "$CDN_DIR/$2-$PLATFORM" ;;
        github) echo "$GITHUB_DIR/$2-$PLATFORM" ;;
    esac
}

# fetch_url <source> <url>: the same object on the host that source uses.
fetch_url() {
    case "$1" in
        cdn-cn) echo "$CDN_CN${2#"$CDN"}" ;;
        *) echo "$2" ;;
    esac
}

mkdir -p "$INSTALL_DIR"
MANIFEST_TMP="$(mktemp "$INSTALL_DIR/.meshy-dcc-manifest.XXXXXX")"
CLI_TMP="$(mktemp "$INSTALL_DIR/.meshy-dcc.XXXXXX")"
SERVER_TMP="$(mktemp "$INSTALL_DIR/.meshy-dcc-server.XXXXXX")"
cleanup() {
    rm -f "$MANIFEST_TMP" "$CLI_TMP" "$SERVER_TMP"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM


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

# fetch_verified <source> <binary> <output>: download one binary the manifest
# names and check its version, URL and SHA-256. Returns 1 when the download
# fails; exits when a check fails.
fetch_verified() {
    source="$1"
    binary="$2"
    output="$3"
    count="$(grep -Ec "^[[:space:]]*\"$binary\"[[:space:]]*:[[:space:]]*\\{" "$MANIFEST_TMP" || true)"
    [ "$count" = "1" ] || fail "official release manifest must contain one $binary entry"

    version="$(manifest_field "$binary" version)"
    url="$(manifest_field "$binary" url)"
    expected="$(manifest_field "$binary" sha256)"
    printf '%s' "$version" \
        | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?$' \
        || fail "official release manifest contains an invalid $binary version"
    [ "$version" = "$REQUESTED_VERSION" ] \
        || fail "official release manifest version does not match requested version $REQUESTED_VERSION"
    [ "$url" = "$(release_url "$source" "$binary")" ] \
        || fail "official release manifest contains a non-official $binary URL"
    [ "${#expected}" -eq 64 ] \
        && ! printf '%s' "$expected" | grep -Eq '[^0-9A-Fa-f]' \
        || fail "official release manifest contains an invalid SHA-256"

    download_to "$(fetch_url "$source" "$url")" "$output" || return 1
    actual="$(calculate_sha256 "$output")"
    [ "$(printf '%s' "$actual" | tr 'A-F' 'a-f')" = "$(printf '%s' "$expected" | tr 'A-F' 'a-f')" ] \
        || fail "$binary SHA-256 does not match the official release manifest"
    chmod 0755 "$output"
}

# Both are verified before either replaces an installed file.
FETCHED=""
for SOURCE in cdn cdn-cn github; do
    if download_to "$(fetch_url "$SOURCE" "$(release_url "$SOURCE" meshy-dcc-update-manifest).json")" "$MANIFEST_TMP" \
        && fetch_verified "$SOURCE" meshy-dcc "$CLI_TMP" \
        && fetch_verified "$SOURCE" meshy-dcc-server "$SERVER_TMP"; then
        FETCHED="$SOURCE"
        break
    fi
    echo "Could not download from $SOURCE; trying the next source." >&2
done
[ -n "$FETCHED" ] || fail "meshy-dcc could not be downloaded from the Meshy CDN (cdn.meshy.ai, cdn.meshy.cn) or GitHub"
mv -f "$SERVER_TMP" "$INSTALL_DIR/meshy-dcc-server"
TARGET="$INSTALL_DIR/meshy-dcc"
mv -f "$CLI_TMP" "$TARGET"
echo "Installed verified meshy-dcc $REQUESTED_VERSION to $INSTALL_DIR"

echo "Installing the meshy-dcc Skill for the agents on this computer:"
"$TARGET" --output human agent-skill --execute \
    || echo "The meshy-dcc Skill was not installed for every agent; run: meshy-dcc agent-skill" >&2

RUN="meshy-dcc"
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        # Like the Windows installer, which adds it to the user PATH.
        case "${SHELL:-}" in
            */zsh) PROFILE="$HOME/.zshrc" ;;
            */bash) PROFILE="$HOME/.bash_profile" ;;
            *) PROFILE="" ;;
        esac
        LINE="export PATH=\"$INSTALL_DIR:\$PATH\""
        RUN="$TARGET"
        echo
        if [ -n "$PROFILE" ]; then
            if ! grep -qsF "$LINE" "$PROFILE"; then
                printf '\n# meshy-dcc\n%s\n' "$LINE" >> "$PROFILE"
                echo "Added $INSTALL_DIR to PATH in $PROFILE."
            fi
            echo "Open a new terminal to run meshy-dcc by name."
        else
            echo "Add $INSTALL_DIR to PATH to run meshy-dcc from any terminal:"
            echo "  $LINE"
        fi
        ;;
esac
echo
echo "Next: run '$RUN ui' to pick the apps to connect."

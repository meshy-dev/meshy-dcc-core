#!/usr/bin/env bash
# Upgrade smoke test: from the installed release to the one a manifest names.
#
#   upgrade_smoke.sh --bin-dir DIR --manifest-url URL --version NEW
#                    [--install-previous] [--port 19865]
#
# --install-previous first installs the release latest.txt names into DIR with
# the published install script (the release workflow runs this before it
# writes the new latest.txt). Without it, DIR must already hold an older
# meshy-dcc and meshy-dcc-server.
#
# Then, with every Meshy DCC path in a temporary directory:
#   1. start the gateway, with MESHY_DCC_UPDATE_MANIFEST_URL set;
#   2. `meshy-dcc update apply --yes` must succeed and stage the server;
#   3. the first command after it must print its own JSON and exit 0;
#   4. both binaries and the running gateway must report NEW.
# A release before 0.2.2 does not restart the gateway in `update apply`; then
# this script runs `meshy-dcc gateway restart` itself and says so.
#
# Needs bash, curl and jq (all on GitHub's macOS and Windows runners).
set -euo pipefail

bin_dir="" manifest_url="" version="" install_previous=0 port=19865
while [ "$#" -gt 0 ]; do
    case "$1" in
        --bin-dir) bin_dir="$2"; shift 2 ;;
        --manifest-url) manifest_url="$2"; shift 2 ;;
        --version) version="$2"; shift 2 ;;
        --install-previous) install_previous=1; shift ;;
        --port) port="$2"; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done
[ -n "$bin_dir" ] && [ -n "$manifest_url" ] && [ -n "$version" ] || {
    echo "usage: $0 --bin-dir DIR --manifest-url URL --version NEW [--install-previous] [--port N]" >&2
    exit 2
}

fail() { echo "::error::upgrade smoke: $*" >&2; exit 1; }
step() { echo "--- $*"; }

case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) exe=".exe" ;; *) exe="" ;; esac
mkdir -p "$bin_dir"
bin_dir="$(cd "$bin_dir" && pwd)"

if [ "$install_previous" = 1 ]; then
    step "install the current latest release into $bin_dir"
    if [ -n "$exe" ]; then
        MESHY_DCC_INSTALL_DIR="$(cygpath -w "$bin_dir")" pwsh -NoProfile -Command \
            'irm https://cdn.meshy.ai/meshy-dcc-core/install.ps1 | iex'
    else
        curl -fsSL https://cdn.meshy.ai/meshy-dcc-core/install.sh | MESHY_DCC_INSTALL_DIR="$bin_dir" sh
    fi
fi

cli="$bin_dir/meshy-dcc$exe"
server="$bin_dir/meshy-dcc-server$exe"
[ -x "$cli" ] && [ -x "$server" ] || fail "no meshy-dcc and meshy-dcc-server in $bin_dir"

# Every path the CLI and gateway write goes to a temporary directory.
root="$(mktemp -d)"
export HOME="$root/home" USERPROFILE="$root/home"
export MESHY_DCC_DATA_DIR="$root/data" MESHY_DCC_REGISTRY_DIR="$root/registry"
export MESHY_DCC_UPDATE_CACHE_DIR="$root/cache" MESHY_DCC_GATEWAY_ADMIN_DB="$root/admin.sqlite"
export MESHY_DCC_MARKETPLACE_INSTALL_ROOT="$root/marketplace"
export MESHY_DCC_GATEWAY_PORT="$port" MESHY_DCC_UPDATE_MANIFEST_URL="$manifest_url"
export MESHY_DCC_NON_INTERACTIVE=true MESHY_DCC_DISABLE_UPDATE_CHECK=1
mkdir -p "$HOME" "$MESHY_DCC_DATA_DIR" "$MESHY_DCC_REGISTRY_DIR" "$MESHY_DCC_UPDATE_CACHE_DIR"

stop_gateway() { "$cli" --output json gateway stop > /dev/null 2>&1 || true; }
trap stop_gateway EXIT

previous="$("$cli" --version | awk '{print $NF}')"
step "installed $previous, upgrading to $version"
if [ "$previous" = "$version" ]; then
    echo "::warning::latest.txt already names $version; nothing to upgrade from"
    exit 0
fi

step "start the gateway on port $port"
"$cli" --output json gateway start --port "$port" | tee "$root/start.json"

step "update apply --yes"
set +e
"$cli" --output json update apply --yes > "$root/apply.json"
apply_code=$?
set -e
jq . "$root/apply.json" || cat "$root/apply.json"
[ "$apply_code" = 0 ] || fail "update apply exited $apply_code"
server_status="$(jq -r '.server_update.status // empty' "$root/apply.json")"
[ "$server_status" = staged ] || fail "server_update.status is '$server_status', not staged"
restart_status="$(jq -r '.gateway_restart.status // empty' "$root/apply.json")"

step "the first command after the update prints its own output and exit status"
set +e
"$cli" --output json gateway status > "$root/first.json" 2> "$root/first.err"
first_code=$?
set -e
cat "$root/first.err"
[ "$first_code" = 0 ] || fail "first command exited $first_code"
jq -e '.port' "$root/first.json" > /dev/null || fail "first command printed no JSON of its own: $(cat "$root/first.json")"

if [ -z "$restart_status" ]; then
    echo "::notice::$previous does not restart the gateway in update apply; restarting it here"
    "$cli" --output json gateway restart --port "$port" > /dev/null
else
    [ "$restart_status" = restarted ] || fail "gateway_restart.status is '$restart_status'"
    restarted_as="$(jq -r '.gateway_restart.new_version' "$root/apply.json")"
    [ "$restarted_as" = "$version" ] || fail "gateway restarted as $restarted_as"
fi

step "both binaries and the gateway report $version"
cli_version="$("$cli" --version | awk '{print $NF}')"
server_version="$("$server" --version | awk '{print $NF}')"
gateway_version="$(curl -fsS "http://127.0.0.1:$port/admin/api/health" | jq -r .version)"
echo "meshy-dcc $cli_version, meshy-dcc-server $server_version, gateway $gateway_version"
[ "$cli_version" = "$version" ] || fail "meshy-dcc is $cli_version"
[ "$server_version" = "$version" ] || fail "meshy-dcc-server is $server_version"
[ "$gateway_version" = "$version" ] || fail "the gateway runs $gateway_version"
echo "upgrade smoke passed: $previous -> $version"

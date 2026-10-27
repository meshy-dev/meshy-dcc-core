# Changelog

Meshy DCC Core release notes. Crates, the Python wheel and the product share
one version line.

## Unreleased

### Added

- Anonymous usage telemetry, on by default. Once a day the gateway sends one
  aggregate event to Meshy's PostHog: versions, OS, installed connectors, live
  instances per app and major version, tool-call counts per app, skill and
  outcome with latency buckets, and a few counts (searches, bug reports sent,
  agent kinds, window opened). Skills you wrote are sent as `custom`. It never
  sends tool arguments or outputs, paths, scene or project names, IDs or
  prompts, and asks PostHog to store no IP, location or person profile. The
  first CLI command prints a one-time notice and the window shows a banner.
  Turn it off with `meshy-dcc telemetry off`, the Settings switch,
  `DO_NOT_TRACK=1` or `MESHY_DCC_TELEMETRY=0`; it is off in CI.
  `meshy-dcc telemetry preview` prints the exact next request. Every field is
  listed in `docs/telemetry.md`.
- The Skill tells agents to set `MESHY_CLI_CLIENT="meshy-dcc/<version> (<dcc_type>)"`
  on each `meshy` call, so Meshy can tell those calls apart (meshy-cli adds it
  to its User-Agent; older versions ignore it).
- `meshy-dcc update apply --yes` restarts the local gateway after it updates
  the CLI and the server, so the new server runs at once. It restarts only the
  standalone gateway on this computer, never one an app started, and checks
  that the gateway comes back with the new version. The result reports
  `server_update` and `gateway_restart` (`restarted`, `not_needed`, `skipped`,
  `not_running`, `failed` or `unknown`, with the old and new version).
  `--no-restart` leaves the gateway running. The update from 0.2.1 itself is
  done by the 0.2.1 CLI, which does not restart; run
  `meshy-dcc gateway restart` once after it.
- `meshy-dcc gateway restart`, the same as `gateway daemon restart`.
- **Update now** in the window's Overview, when an update is available. It
  shows "Updating…" while Meshy DCC Core restarts, then reloads on the new
  version, or says why the update failed.
- Pull requests and pushes to main run the Rust, admin UI and Python tests.

### Changed

- `call`, `call-batch`, `search`, `describe`, `list` and `load-skill` no longer
  wait up to 750 ms for the marketplace update check. Other commands still
  show its notice.
- The server download that `update apply` starts stops after 10 minutes and
  is reported as `unknown`, instead of waiting forever.
- Releases: the workflow loads the built wheel and binaries on the newest
  macOS runner, and upgrades from the previous release on macOS and Windows
  before it writes `latest.txt`. Every action is pinned by commit.

## 0.2.1 (2026-10-27)

### Upgrading from 0.2.0

The 0.2.0 gateway cannot apply its own update, so `meshy-dcc update apply`
leaves it on 0.2.0. Install 0.2.1 again instead, which replaces both the CLI
and the gateway:

- macOS: `curl -fsSL https://cdn.meshy.ai/meshy-dcc-core/install.sh | bash`
- Windows: run `meshy-dcc gateway daemon stop` first, then
  `irm https://cdn.meshy.ai/meshy-dcc-core/install.ps1 | iex`
- npm: `npm install -g meshy-dcc@latest`

Then run `meshy-dcc gateway daemon restart` so the gateway runs 0.2.1.

### Added

- `meshy-dcc report --summary "..."` sends a bug report, and **Report a problem**
  in the window does the same from a form. Both show the exact report first and
  send nothing without `--yes` or a click on **Send**. A report holds the
  description, the versions and OS, a diagnostic summary without paths or IDs,
  and the agent error reports of the last 24 hours. A bot turns each report into
  an issue labelled `user-report` in meshy-dev/meshy-dcc-core; the raw report is
  kept privately for 90 days. The Skill tells agents to offer a report when Meshy
  DCC Core itself misbehaves, and to send it only after the user agrees.

### Changed

- The window shows real logos for Houdini, Substance 3D Painter, FreeCAD,
  OpenSCAD and Material Maker, and for the Pi, Amp and Kimi Code CLI agents.

### Fixed

- The first command after a staged update now returns its own output and exit
  status. It used to exit at once with no output and exit status 0, while the
  updated CLI ran on unattended.
- A gateway that applies a staged update keeps its process. Before, the first
  start after a server update failed with "gateway process exited before it
  became healthy".
- `meshy-dcc update apply --yes` also updates the `meshy-dcc-server` installed
  beside the CLI, and the gateway applies a staged update when it starts. Until
  now the gateway kept running the version that was first installed.
- `meshy-dcc-server` no longer updates itself when a package manager owns the
  installation, just as the CLI already did not.
- A package-managed CLI's update notice names the upgrade command for its
  package manager, not `update apply`, which it refuses.
- `MESHY_DCC_DISABLE_UPDATE_CHECK`, and `update_check: false` in settings, now
  also turn off the marketplace update check that ran on every command.

## 0.2.0 (2026-10-19)

Meshy DCC Core diverges completely from the project it was forked from. It no
longer tracks that project or takes patches from it, shares no names with it,
and is not runtime-compatible with it. Attribution stays in `LICENSE` and
`NOTICE`.

### Changed

- Everything is Meshy-named:
  - the CLI is `meshy-dcc` everywhere (binary, crate, install scripts, release
    assets and the PyPI wrapper); the other crates are `meshy-dcc-*`;
  - the Python package is `meshy_dcc_core`;
  - environment variables are `MESHY_DCC_*`;
  - registry and state directories are `meshy-dcc`;
  - the MCP identity, request headers and registry metadata keys use the
    Meshy names;
  - the default gateway port is 19765.
- One version line, 0.2.0. The separate internal compatibility version is
  gone, and gateway election compares Meshy components only.
- The bundled install catalog lists only Meshy adapters, 23 hosts in all. An
  entry stays `not_available` until its adapter is published; the signed
  catalog makes the published ones installable.
- The catalog is refreshed from `cdn.meshy.ai/meshy-dcc-core/catalog/`, or
  the same object on cdn.meshy.cn when cdn.meshy.ai cannot be reached, and
  must carry a Meshy signature. While the feed is unpublished, the CDN's 403
  counts as unavailable and the bundled catalog is used.
- The marketplace's default source is
  `cdn.meshy.ai/meshy-dcc-core/marketplace/` (signature
  `marketplace-sigstore.json`), with cdn.meshy.cn as the fallback host. Both
  feeds are replaced in place and published uncached.
- The gateway answers browsers only from its own origin. CORS used to allow
  every origin, so any web page could call the gateway, including its admin
  endpoints. A request that carries `Origin` must now come from the
  gateway's own address, a loopback page talking to a loopback gateway, or
  `MESHY_DCC_GATEWAY_ALLOWED_ORIGINS`; browser requests must also name the
  gateway by IP address or `localhost` (or a name in
  `MESHY_DCC_GATEWAY_ALLOWED_HOSTS`), which stops DNS rebinding. The CLI,
  adapters and other non-browser clients are not affected.
- The gateway no longer opens its remote (LAN) listener by default. Adapter
  processes used to bind `0.0.0.0:59765` with no authentication, so anyone on
  the network could list instances and call DCC tools. Set
  `MESHY_DCC_GATEWAY_REMOTE_PORT` (and `MESHY_DCC_GATEWAY_REMOTE_HOST`) to
  enable it; only loopback `127.0.0.1:19765` is served otherwise.
- Admin endpoints that put code on the computer or write its configuration
  answer loopback peers only, like the connector and agent installs: adding
  or removing a skill path, the marketplace's install, uninstall, source and
  update endpoints and its WebSocket, and saving or testing an integration.
  A remote viewer reads them and runs the CLI on the gateway's computer. A
  new skill path must be an existing folder given in full.
- A local `call` must name its instance, either with an instance slug or
  with `--instance-id`, even when only one instance is live. The only
  exception is a tool the host marks `readOnlyHint: true`. The CLI rejects
  the call before sending anything to the host.
- Agents reach hosts through the gateway. It is the CLI's default route, and
  the gateway routes each call to the instance the agent names. Only an
  explicit `--gateway local` talks to hosts directly, for diagnostics;
  `--require-gateway` is still accepted and changes nothing. The gateway is one resident process that the CLI starts on
  demand: `meshy-dcc-server` when it is installed next to the CLI, otherwise
  the CLI's own `meshy-dcc gateway`. `start-instance` starts it before it
  launches a host, so the host registers behind it. The CLI's direct-to-host
  local mode stays for diagnostics.
- Hosts no longer promote themselves to gateway after they start. The
  embedded election did that by restarting the host's server, which
  registered a new instance id and cut off the launch operation.
  `DccGatewayElection` is still importable; core no longer starts it.
- A gateway that has not seen a host yet waits 10 minutes before it leaves;
  once its last host is gone it still leaves after 30 seconds.
- Errors carry a code and `details.reason` instead of `INTERNAL_ERROR`:
  `NOT_FOUND`, `INVALID_INPUT`, `UNAVAILABLE` or `CONFLICT`, with reasons such
  as `instance_not_found`, `instance_required`, `skill_not_loaded` or
  `invalid_params`. The error envelope follows `--output`. The gateway passes
  a host's own refusal (bad arguments, skill not loaded) through with its kind,
  message and status instead of a 502 backend error, and the CLI reads the
  gateway's nested error into the envelope.
- Search returns the best match first: hosts score whole-word tool matches
  and mark exact ones, and the CLI keeps their order.
- A `next_action` command repeats the caller's global flags, and a retried
  `start-instance` keeps its `--launch-plan` and other options.
- `stop-instance` on a local host always uses the local lifecycle path, where
  the launch operation records ownership. It waits up to 10 seconds for a
  just-started host to publish its stop hook, and follows a host that
  registered again under the same launch operation.
- `meshy_dcc_server_version` and gateway election use the core version
  rather than the adapter's.
- Background update checks never start the gateway.

### Added

- Connectors for 13 hosts in the bundled catalog, installable once their
  builds are published: Blender, Unity, Godot, Unreal Engine, OpenSCAD,
  Material Maker, FreeCAD, Roblox Studio, Houdini, Maya, 3ds Max (Windows
  only), Substance 3D Painter and ZBrush. Cinema 4D is listed but not
  installable in this release.
- The release workflow publishes the signed install catalog
  (`catalog/install-catalog.json` on the CDN): run without a tag to publish
  it alone, and daily once `INSTALL_CATALOG_REFRESH` is on. Every installable
  build is downloaded from each of its URLs and checked first.
- A catalog entry tagged `experimental` is reported as
  `"experimental": true` by `dcc-types` and marked on the Connectors page.
- `skills/meshy-dcc`: the agent Skill for the `meshy-dcc` CLI. It covers
  choosing an instance, finding and loading tools, calling them, reading the
  scene back and handing off Meshy models.
- `type: host-package` install catalog entries for adapters whose payload is
  host-native. Their host commands run as argv without a shell, start with
  the operator's `--dcc-path`, and can write the start-instance launch plan.
- `host-package` entries can set `host.extract` to unpack the zip into the
  Meshy DCC data directory (`{package_dir}`). Commands may then run a file
  inside the package as `{package_dir}/<relative path>`.
- `host.project_steps` and `install --project <root>` set up one project, for
  hosts whose plugins live in each project. The steps may use `{project}`.
- A failed `host-package` step now reports why it failed in the JSON report
  (`steps[].message`) and on stderr. Other step types still keep their errors
  out of the report, because those errors can carry credentials.
- `MESHY_DCC_DATA_DIR` overrides the data directory that holds extracted
  adapter packages.
- Meshy Unity adapter 0.1.0 in the bundled catalog (still `not_available`):
  it installs per project, and the Skill covers it
  (`skills/meshy-dcc/references/unity.md`).
- Meshy Godot adapter 0.1.0 in the bundled catalog (still `not_available`):
  it installs per project, and the Skill covers it
  (`skills/meshy-dcc/references/godot.md`).
- Meshy Unreal adapter 0.1.0 in the bundled catalog (still `not_available`):
  it installs per project and builds its C++ editor module there, and the
  Skill covers it (`skills/meshy-dcc/references/unreal.md`).
- `stop-instance --discard-unsaved` tells the host it may close even though
  work might be unsaved. Send it only after the user has agreed.
- `meshy_dcc_core.host.safe_stop.serve_safe_stop` serves a loopback endpoint that
  an adapter can advertise as `safe_stop_url`. It accepts JSON bodies only.
- The Skill covers `call --batch` (up to 25 calls in order, one result each),
  the gateway's result shapes, waited job results and the Unity source-write
  gate.
- Tool arguments are checked against JSON Schema `pattern`.
- A `not_available` catalog entry's error carries the catalog's reason.
- `scripts/install-cli.sh` (macOS, Apple Silicon) and `scripts/install-cli.ps1`
  (Windows x86_64) install `meshy-dcc` and `meshy-dcc-server` from
  `cdn.meshy.ai/meshy-dcc-core`: both are checked against the release
  manifest's URLs and SHA-256 digests before either replaces an installed
  file, then the installer shows and applies `meshy-dcc agent-skill`. Other
  platforms are refused before anything is downloaded. Both default to
  `~/.local/bin`. They no longer install dcc-cua. When cdn.meshy.ai cannot
  be reached they try cdn.meshy.cn (the same bucket, reachable in mainland
  China), then the GitHub Release. The
  latest version comes from `latest.txt` on the CDN (served uncached), or the
  GitHub Release when neither CDN host answers; the CDN keeps each release
  under its own version dir (`v0-2-0`) and never overwrites it, with names
  that follow the CDN rules (`windows-x64`). The README shows the one-line
  installs (`curl … | bash`, `irm … | iex`).
  `scripts/release/generate_update_manifest.py` writes the matching manifest
  and the binaries under their published names.
- The gateway self-update uses the same layout. It reads the latest version
  from `latest.txt` (cdn.meshy.ai, then cdn.meshy.cn, then the GitHub
  Release) and fetches that release's manifest and Sigstore bundle from its
  version dir on either CDN host, or from the GitHub Release: a source that
  cannot be reached moves on, a manifest that fails verification stops.
  GitHub Release manifests must carry the signature too. Each CDN manifest
  entry lists `mirrors` (cdn.meshy.cn, the GitHub Release) that
  `update apply` tries when a download fails; bytes that fail the SHA-256
  check stop the update.
- A platform build in the catalog can list `mirrors`: URLs serving the same
  bytes, tried in order when a download fails. A download whose bytes fail
  the SHA-256 check stops the install instead. The bundled catalog points
  each adapter build at its CDN copy (CDN names, version dirs) with
  cdn.meshy.cn and the GitHub Release of meshy-dcc-adapters (tagged
  `<host>-v<version>`, one repository for all adapters) as mirrors.
- Catalog `host-package` entries can list one build per platform under
  `install.platforms` (`macos-arm64`, `windows-x86_64`); `meshy-dcc` uses the
  running machine's build, and an entry with none for it is reported as
  `not_available` with the platforms it does have. The bundled catalog does
  this for the four adapters, each for macOS and Windows; a platform's build
  can bring its own host commands (the Windows launchers are `.cmd` files,
  and Godot's installer runs the package's Python directly).
- `.github/workflows/release.yml` publishes a release by hand from main for a
  given tag: it builds the CLI, the gateway and the core wheel natively on
  macOS (arm64) and Windows (MSVC), signs the release manifests, uploads
  everything to the GitHub Release, publishes the three npm packages
  (skipping versions already there) and copies the release to the CDN
  bucket (`scripts/release/sync_cdn.py`: release files written once,
  `latest.txt` last and only forward). No credentials live in GitHub: the
  workflow assumes an AWS role through OIDC and reads the npm token from
  AWS Secrets Manager (`scripts/release/aws/`).
- npm packaging: `meshy-dcc` holds a small launcher and lists one binary
  package per platform (`@meshy-ai/meshy-dcc-darwin-arm64`,
  `@meshy-ai/meshy-dcc-win32-x64`) as optional dependencies, so npm installs
  only the machine's own. Each carries both binaries and the package-manager
  marker, so `update apply` leaves upgrades to npm.
  `scripts/release/build_npm_packages.py` builds the tarballs.
- `meshy-dcc agent-skill` installs the `meshy-dcc` Skill, which is embedded in
  the CLI, into agents' skill directories: every agent in the skills.sh table
  that is installed on the computer (Claude Code, Codex, Cursor, Gemini CLI
  and about 70 more, each detected the way skills.sh does), the agents named
  with `--agent <id>` (`*` for all), or any `--dir`. Agents that read one
  folder (Codex, Cline, Warp, Zed and others share `~/.agents/skills`) are one
  target. It prints the plan unless `--execute` is given, writes only into
  `<skills dir>/meshy-dcc/`, records what it wrote, never overwrites a
  directory it did not write, and `--remove` deletes only its own files.
  Release builds update an installed copy when the CLI changes.
- `meshy-dcc ui` opens the Meshy DCC Core window in the browser (the gateway's
  `/admin` page), starting the local gateway first; `--no-open` only prints
  the address, and a remote gateway profile opens that gateway. The page is
  the user-facing product: Overview (core service, connected apps by
  instance, installed connectors and Skill), Connectors (install, or add to
  a project, and what is coming), Skills (what each connected instance has
  loaded, every skill it offers and whether it ships with Meshy, is the
  user's own or came from the marketplace, the user's skill folders with the
  state of each skill in them and a reload into the open apps, a prompt that
  has an agent write a skill, and marketplace packages when the marketplace
  lists any), Agents (turn the Skill on per agent),
  Settings, and a first-run checklist that goes away once a connector is
  installed, an agent has the Skill and a call has succeeded. Operations
  pages stay under a Developer section that Settings turns on. English,
  Simplified Chinese, Japanese and Korean. An open page keeps an idle
  standalone gateway running.
- New admin endpoints behind those pages run the `meshy-dcc` CLI installed
  next to the gateway: `GET /admin/api/connectors` (`dcc-types`),
  `POST /admin/api/connectors/install` (`install --execute`),
  `GET /admin/api/agents` and `POST /admin/api/agents/skill`
  (`agent-skill`). The two that change the computer answer loopback peers
  only. `GET /admin/api/skill-library` and `POST
  /admin/api/skill-library/reload` (loopback only) serve the Skills page. `/admin/api/health` adds `service`: who started the gateway, the LAN
  listener, whether authentication is on and the idle timeout.
- The agent Skill has an authoring guide (`references/authoring.md`): where a
  user's own Blender skill goes, its `SKILL.md`, `tools.yaml` and script, and
  `lint`, `reload-skills`, `load-skill` and `call` to check it.
- A successful `install --execute` writes a receipt under the Meshy DCC data
  directory (`receipt_path` in the report); `dcc-types` lists them as
  `installed` and marks adapters that install per project (`per_project`).
- `dcc-types` reports an adapter the catalog lists but cannot install with
  `next_action.id: report_unavailable` and the catalog's reason, and no
  command. It used to point back at `dcc-types`, which sent agents in a loop.

### Fixed

- Each adapter version is extracted into its own directory. CDN file names
  carry no version, so an update used to replace the files of the running
  version, and on Windows failed on a loaded `_core.pyd`.
- An instance's heartbeat no longer cancels the gateway's status write, which
  could leave a ready instance `booting` (invisible to `list` and `call`).
  A request that reaches a `booting` instance probes it at once, so calls
  right after `start-instance` succeed.
- The gateway index keeps the tools of a DCC type with a hyphen
  (`material-maker`): its job status tool and preloaded skills used to drop
  out, so `call --wait` timed out.
- `call --wait` reports a stopped instance instead of retrying until its
  timeout; image detection reads MCP content blocks only; `--gateway local`
  reads the local registry even with `MESHY_DCC_BASE_URL` set.
- Stopping a server ends the skill scripts it started, with their child
  processes; their calls return `CANCELLED`.
- `gateway start`, `stop`, `status` and `doctor` find the gateway the other
  commands use (`MESHY_DCC_BASE_URL`, `MESHY_DCC_GATEWAY_PORT`) instead of
  assuming 19765.
- `start-instance` right after a stop waits for the host to exit and starts a
  new one; it used to reuse the closing host and report it ready.
- A tool that reports `success: false` is a tool result (`GENERAL_ERROR`,
  exit 1) instead of a backend failure (`UNAVAILABLE`, exit 3).
- Windows: the release build runs natively (admin UI `npm.cmd`, `python`,
  no build-machine paths from aws-lc's asserts), and the CLI and the gateway
  it starts no longer hand their standard handles to child processes, which
  kept an agent reading `meshy-dcc list` waiting for minutes.
- Adding or removing a skill path, or installing, updating or uninstalling a
  marketplace package, now makes the open apps re-scan their skill folders.
  The standalone gateway only refreshed its own index, so the apps saw the
  change after a restart or `meshy-dcc reload-skills`. Loaded skills stay
  loaded, and one whose files changed is reloaded.
- The admin Sessions page read a summary and row fields the gateway does not
  send and blanked the whole page; Back stayed blank until a reload. It now
  follows `GET /admin/api/sessions`, and a page that fails shows its error in
  place.
- The OTLP integration showed as active whenever `otlp.json` held an
  endpoint, though neither the dev nor the release build exports OTLP. It now
  reads "not in this build" unless `meshy-dcc-server` is built with
  `otlp-exporter`, which now also enables `telemetry`.
- The admin Skills tab inlined a 45 KB PNG icon in every row (about 2.8 MB of
  page for Blender's skills); hosts with a product logo use its small SVG.
- A host launched by `start-instance` now records its project and operation in
  its registry row, so `start-instance` can find and reuse that instance.
- When a `tools.yaml` entry sets `read_only`, `destructive` or `idempotent` to
  `true` and declares no `*_hint` key, the matching MCP annotation is now
  published.
- `stop-instance` with `--base-url` (or `MESHY_DCC_BASE_URL`) pointing at a
  gateway on this computer stops a `start-instance` host from its local
  operation record again. Any `--base-url` used to send the stop to the
  gateway, whose guarded stop needs `--expected-owner` and
  `--expected-session` and so refused. Passing those two still uses the
  gateway.
- `stop-instance --instance-id` right after `start-instance` waits for the
  host to publish its stop hook, as `--operation-id` already did; it used to
  refuse with `safe_stop_unavailable` when the host had not published it yet.
- When an adapter installer fails, `install` reports what it said: its
  stderr, or its JSON report on stdout when stderr is empty. The installer's
  stdout used to be discarded, so a Godot `ownership_conflict` showed only as
  `exit status: 30:`.
- A gateway started right after another one stopped no longer gives up: the
  port stays unbindable while TCP TIME_WAIT lasts (about 30 s on macOS, 60 s
  on Linux), and the new `meshy-dcc-server` used to exit at once, leaving the
  next host to run the gateway inside its own process. It now keeps
  contesting the port for up to 75 s, and the CLI waits up to 90 s for it,
  stopping early if the gateway process exits. The behavior is the same on
  every platform.
- `components ensure dcc-cua` downloads from the upstream `dcc-mcp/dcc-cua`
  releases again; the rename had pointed it at a repository that does not
  exist.
- Stopping a server now ends the skill scripts it is running as child
  processes, together with every process they started, and the waiting call
  returns a `CANCELLED: …` error. They used to run on after the stop and
  could write files after it.
- `gateway start|stop|status|ensure` and `gateway daemon …` without
  `--host`/`--port` act on the gateway the other commands use (`--base-url`,
  `MESHY_DCC_BASE_URL`), not always 127.0.0.1:19765. All commands also take
  the local gateway's port from `MESHY_DCC_GATEWAY_PORT` when neither a base
  URL nor a remote profile is chosen.
- With `MESHY_DCC_DATA_DIR` set, an adapter server writes everything under
  it: logs, the jobs database and host-error log in `log/` (unless
  `MESHY_DCC_LOG_DIR` is set), the gateway's admin database (unless
  `MESHY_DCC_GATEWAY_ADMIN_DB` is set), and the local skills folder, checkpoints,
  loaded-skill state, materialized scripts and embedding cache in
  `<dcc>/…` instead of `~/.meshy-dcc`. Unset, nothing moves.

### Removed

- The inherited documentation site, agent guides, release tooling
  configuration and patch workflow.
- The `asset-source` demo skill, which returned placeholder assets.
- The release tooling inherited from upstream for PyPI and release-please: the
  PyPI wrapper packages for the CLI and the gateway (`pkg/meshy-dcc-bin`,
  `pkg/meshy-dcc-server-bin`), the wheel retagging, PyPI cleanup and
  size-budget scripts, the GitHub release bundle builders, and the
  release-please, release-notes, release-asset and distribution-set checks,
  with their tests. Releases go out as a GitHub Release plus npm.
- The CLI no longer downloads `meshy-dcc-server` from the release channel or
  runs a cached copy: it ran that binary without verifying it. With no
  `meshy-dcc-server` beside it, the CLI runs the gateway itself, as it already
  did whenever the download failed.

## 0.1.0 (2026-09-28)

First Meshy release line.

### Changed

- Binaries are `meshy-dcc` (CLI) and `meshy-dcc-server` (gateway/sidecar).
- Self-update and gateway download use the Meshy release channel at
  `cdn.meshy.ai/meshy-dcc-core`. Official manifests require a Sigstore bundle
  from the Meshy release workflow and fail closed until one is published.
- `--version` reports the Meshy product version.

### Removed

- `feedback` is disabled; nothing is submitted off the machine.
- GitHub workflows inherited from the fork source are not carried over.

# Changelog

Meshy DCC Core release notes. Crates, the Python wheel and the product share
one version line.

## 0.2.0 (unreleased)

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
  entry stays `not_available` until its adapter is published.
- The catalog is refreshed from `cdn.meshy.ai/meshy-dcc-core/catalog/` and
  must carry a Meshy signature. While the feed is unpublished, the CDN's 403
  counts as unavailable and the bundled catalog is used.
- The marketplace's default source is
  `cdn.meshy.ai/meshy-dcc-core/marketplace/`.
- The gateway no longer opens its remote (LAN) listener by default. Adapter
  processes used to bind `0.0.0.0:59765` with no authentication, so anyone on
  the network could list instances and call DCC tools. Set
  `MESHY_DCC_GATEWAY_REMOTE_PORT` (and `MESHY_DCC_GATEWAY_REMOTE_HOST`) to
  enable it; only loopback `127.0.0.1:19765` is served otherwise.
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
  `~/.local/bin`. They no longer install dcc-cua. When the CDN cannot be
  reached (meshy.ai is blocked in some regions) they fall back to the GitHub
  Release; a file that fails its checks stops the install instead. The
  latest version comes from the GitHub Release; the CDN keeps each release
  under its own version dir (`v0-2-0`) and never overwrites it, with names
  that follow the CDN rules (`windows-x64`). The README shows the one-line
  installs (`curl … | bash`, `irm … | iex`).
  `scripts/release/generate_update_manifest.py` writes the matching manifest
  and the binaries under their published names.
- A platform build in the catalog can list `mirrors`: URLs serving the same
  bytes, tried in order when a download fails. A download whose bytes fail
  the SHA-256 check stops the install instead. The bundled catalog points
  each adapter build at its CDN copy (CDN names, version dirs) with the
  adapter repository's GitHub Release as the mirror.
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
  (`scripts/release/sync_cdn.py`, which never overwrites a release file).
- npm packaging: `meshy-dcc` holds a small launcher and lists one binary
  package per platform (`@meshy-ai/meshy-dcc-darwin-arm64`,
  `@meshy-ai/meshy-dcc-win32-x64`) as optional dependencies, so npm installs
  only the machine's own. Each carries both binaries and the package-manager
  marker, so `update apply` leaves upgrades to npm.
  `scripts/release/build_npm_packages.py` builds the tarballs.
- `meshy-dcc agent-skill` installs the `meshy-dcc` Skill, which is embedded in
  the CLI, into agents' skill directories: Claude Code (`~/.claude/skills`)
  and Codex (`$CODEX_HOME/skills`) when they are present, or any `--dir`. It
  prints the plan unless `--execute` is given, writes only into
  `<skills dir>/meshy-dcc/`, records what it wrote, never overwrites a
  directory it did not write, and `--remove` deletes only its own files.
  Release builds update an installed copy when the CLI changes.
- `dcc-types` reports an adapter the catalog lists but cannot install with
  `next_action.id: report_unavailable` and the catalog's reason, and no
  command. It used to point back at `dcc-types`, which sent agents in a loop.

### Fixed

- A host launched by `start-instance` now records its project and operation in
  its registry row, so `start-instance` can find and reuse that instance.
- When a `tools.yaml` entry sets `read_only`, `destructive` or `idempotent` to
  `true` and declares no `*_hint` key, the matching MCP annotation is now
  published.
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

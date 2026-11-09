# Usage telemetry

Meshy DCC Core sends anonymous, aggregate usage counts to Meshy once a day, so
we can see which apps, connectors and skills people use and where tool calls
fail. It never sends what you work on.

## Turn it off

Any one of these turns it off:

| How | Scope |
|---|---|
| `meshy-dcc telemetry off` | Saved in `~/.meshy-dcc/settings.json` (`"telemetry": false`) |
| Settings → **Send anonymous usage statistics** in the window | Same setting |
| `DO_NOT_TRACK=1` | Environment of the CLI or gateway |
| `MESHY_DCC_TELEMETRY=0` | Environment of the CLI or gateway |
| A CI environment (`CI`, `GITHUB_ACTIONS`, `GITLAB_CI`, `BUILDKITE`, `CIRCLECI`, `TRAVIS`, `JENKINS_URL`, `TEAMCITY_VERSION`, `TF_BUILD`, `BITBUCKET_BUILD_NUMBER`, `CODEBUILD_BUILD_ID`, `DRONE`) | Detected automatically |

Turning it off also deletes the counts and any unsent days. The gateway reads
the environment it was started with, so after setting `DO_NOT_TRACK`, run
`meshy-dcc gateway daemon restart` (the saved setting needs no restart).
Development (debug) builds send nothing unless `MESHY_DCC_TELEMETRY=1`.

Other commands:

- `meshy-dcc telemetry status`: whether it is on, and what turned it off.
- `meshy-dcc telemetry preview`: the exact requests (URL, headers, body) the
  next send would post to each destination (only the timestamps differ).
  Nothing is sent.
- `meshy-dcc telemetry on`
- `meshy-dcc telemetry reset`: a new random install ID; unsent data is deleted.

## First run

The first CLI command prints a one-time notice on stderr. The window shows a
banner until you click **OK** or **Turn off**. Both say what is collected, how
to turn it off, and link here.

## What is sent

Once a day the local gateway posts the same events to two destinations, one
request each:

- PostHog (`https://us.i.posthog.com/batch/`, Meshy's project), in the shape
  below.
- Statsig (`https://events.statsigapi.net/v1/log_event`, Meshy's project), in
  Statsig's shape: see [Statsig](#statsig).

Every event name starts with `meshy_dcc_`.

### Every event (PostHog)

| Field | Example | Notes |
|---|---|---|
| `distinct_id` | `ff812f9c-…` | Random install ID (UUID v4) in `~/.meshy-dcc/telemetry/install_id`. Not tied to a Meshy account; never in bug reports or the `MESHY_CLI_CLIENT` tag. |
| `timestamp` | `2026-10-27T08:42:45.412Z` | Time of the rollup |
| `$process_person_profile` | `false` | PostHog creates no person profile |
| `$geoip_disable` | `true` | PostHog does not look up a location |
| `$ip` | `0.0.0.0` | Placeholder stored instead of your address (see below) |
| `$lib`, `$lib_version` | `meshy-dcc`, `0.2.2` | |

### `meshy_dcc_daily_usage`

| Field | Meaning |
|---|---|
| `schema` | Payload version (`1`) |
| `window_hours` | Hours since the last rollup (24 on the first) |
| `cli_version`, `server_version` | Versions of the CLI that last ran and of the gateway |
| `os`, `arch` | For example `macos`, `aarch64` |
| `connectors` | Installed connectors: `dcc_type` and `version`, from the install receipts |
| `instances` | Live app instances at rollup time: `dcc_type`, `host_major` (for example `4` for Blender 4.5), `count` |
| `tool_calls_total` | Tool calls in the window |
| `tool_calls` | Per `dcc_type` × `skill` × `outcome`: `count`, and the `p50` / `p95` latency as a bucket (`<100ms`, `100-500ms`, `0.5-1s`, `1-5s`, `5-30s`, `30s-2m`, `>2m`). `outcome` is `ok`, `error`, `timeout` or `unknown`. At most 200 rows. |
| `searches` | `meshy-dcc search` runs |
| `bug_reports_sent` | Bug reports sent (the count only) |
| `agents` | Agent kinds the agent Skill was installed into (for example `claude-code`) |
| `window_opened` | Whether the window was opened |

`dcc_type` is one of the host types in the bundled catalog, otherwise `other`.
Versions that do not look like version numbers are sent as `other`.

#### When a skill name is sent

`skill` carries a skill's name only when the skill came with a connector or
from the marketplace; every other skill is sent as `custom`.

- **Connector skill.** The skill's folder lies inside the package folder an
  installed connector was extracted to. The install receipt records that
  folder (`package_dir`); the gateway asks the running app which folder it
  loaded each skill from, resolves symbolic links, `..`, Windows `\\?\` and
  short (8.3) names on both sides, ignores case on Windows and macOS (not on
  Linux), and requires the skill folder to be strictly inside the package
  folder, compared folder by folder (`pkg-old` is not inside `pkg`).
- **Marketplace skill.** Installed from the Meshy marketplace.
- **Everything else is `custom`**, whatever registered it: your own skill
  folders, project folders, `~/.meshy-dcc/<app>/skills`, a folder added in the
  window, or an explicit path that is not under a connector package. A folder
  that cannot be resolved counts as `custom`.

Only the names that qualify are kept (in `state.json`, so a name still counts
when the app is closed at rollup time). Folder paths are never kept or sent.

Not recognized, so their skills are `custom`:

- Blender. Its connector is a Blender extension that Blender itself unpacks
  into its own extensions folder, so the install receipt has no package folder
  to compare with.
- Connectors installed before the receipt recorded `package_dir`, until they
  are installed again (`meshy-dcc install`).
- An app that opens and closes between two hourly checks, since the gateway
  has to ask it while it runs.

### Lifecycle events

- `meshy_dcc_installed`: the first rollup on this computer (also the first
  after upgrading from a version without telemetry). `server_version`, `os`, `arch`.
- `meshy_dcc_updated`: the gateway version changed since the last rollup.
  `from_version`, `server_version`, `os`, `arch`.

### Statsig

The request carries the headers `statsig-api-key` (Meshy's public client key,
the one its web app ships to browsers) and `statsig-client-time` (time of the
rollup, in milliseconds), and the body `{"events": [...]}`. Each event:

| Field | Example | Notes |
|---|---|---|
| `eventName` | `meshy_dcc_daily_usage` | The PostHog event name, or `meshy_dcc_tool_calls` (below) |
| `time` | `1800000000000` | Time of the rollup, in milliseconds |
| `value` | `12` | Only on `meshy_dcc_tool_calls`: the row's `count` |
| `user.customIDs.dcc_install_id` | `ff812f9c-…` | The same random install ID as PostHog's `distinct_id`. No `userID`, `email`, `ip` or `userAgent` is set. |
| `user.appVersion` | `0.2.3` | Gateway version |
| `user.statsigEnvironment.tier` | `production` | |
| `metadata` | | The PostHog event's properties (without the `$` ones), every value as a string; lists such as `connectors` as JSON text, empty values left out |
| `metadata.client_type` | `meshy_dcc` | |
| `metadata.meta` | `{"event_id":"…","schema_version":1}` | JSON text. `event_id` is a UUID v5 of the install ID, UTC day, event name and row, so a day sent again keeps its IDs and duplicates can be dropped. |

Statsig limits each event's metadata, so `meshy_dcc_daily_usage` goes without
`tool_calls`, and each `tool_calls` row is its own `meshy_dcc_tool_calls`
event: metadata `dcc_type`, `skill`, `outcome`, `p50`, `p95` (plus
`client_type` and `meta`), `value` the count. Still at most 200 rows a day.

### Never sent

Tool arguments or outputs, file paths, project, scene or instance names,
instance or request IDs, prompts, search queries, host or computer names, user
names, Meshy account data.

## Where the numbers come from

- Tool calls: the call history the gateway already keeps for `meshy-dcc stats`
  (its SQLite store, plus the in-memory buffer).
- Instances: the gateway's live instance list. Connectors: the install receipts.
- Skill names: the folder each running app loaded a skill from (its own
  `get_skill_info`), checked against the package folders in the install
  receipts.
- Searches, bug reports, agent installs, window opened: one line each in
  `~/.meshy-dcc/telemetry/counters.log`, appended by the CLI or gateway. The CLI
  never touches the network for telemetry.

## Cadence and failures

The gateway rolls up 10 minutes after it starts, then checks every hour and
rolls up once per UTC day. The two requests go out side by side, each with a
10-second timeout, so one destination that does not answer never holds up the
other. Each destination keeps its own queue in
`~/.meshy-dcc/telemetry/state.json`: if its request fails (no answer, a server
error, 408 or 429), the day stays queued there and goes with that
destination's next request, and is never sent again to the destination that
accepted it. At most 3 days are kept per destination, the oldest is dropped. A
request the destination rejects (any other 4xx) is not retried. Failures are
silent. Nothing happens while no gateway runs. A gateway started without the
window (`--no-admin`) keeps no call history, so its tool-call counts are zero.

## IP address and location

### PostHog

The PostHog project that receives these events does not discard client IPs
(it also holds Meshy web app events), so Meshy DCC Core sets these per event:

- `$ip: "0.0.0.0"`. PostHog documents that it uses the request's IP address
  only "if an `$ip` isn't passed through `event.properties`"
  ([how to redact event data](https://posthog.com/tutorials/web-redact-properties#hiding-customer-ip-address)),
  so the stored `$ip` is the placeholder.
- `$geoip_disable: true`, the property PostHog's server SDKs set to skip GeoIP
  enrichment ([Python SDK, GeoIP properties](https://posthog.com/docs/libraries/python)).
  With the placeholder IP there is nothing to look up either.
- `$process_person_profile: false`, PostHog's documented switch for anonymous
  events without a person profile
  ([capture API](https://posthog.com/docs/api/capture)).

What this does not cover: your IP address still reaches PostHog's servers as
the source of the HTTPS request, as with any web request, and their logs are
outside our control. The only documented way to drop it on PostHog's side is
the project's **Discard client IP data** setting.

### Statsig

Statsig receives your IP address as the source of the request and records a
city and country derived from it, along with the `meshy-dcc/<version>` user
agent. Its event API has no per-event switch that turns this off, and Meshy DCC
Core does not set `user.ip`. Everything else in the event stays as described
above: no account, no names, no paths.

## Testing

`MESHY_DCC_TELEMETRY_HOST` (send every destination to a local stub, at the
same paths) and
`MESHY_DCC_TELEMETRY_DELAY_SECS` (seconds before the first rollup) are for
tests only.

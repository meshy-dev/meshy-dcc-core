# Software preparation and project lifecycle

Core separates software observations, adapter preparation and project-bound
execution. These contracts are shared by the CLI and the local MCP gateway;
clients do not implement host-specific installation or launch commands.

## Software and adapter preparation

`meshy-dcc software --json [--catalog <file>]` returns `schema_version: 1` and a
`software` array. Each row contains `dcc_type`, human application `name`, executable
`candidates` (`path`), `detection`, and `adapter` (`available`, `version`,
`prepared`, `per_project`). A catalog may supply `homepage` through
`host.software_homepage`; this is separate from the adapter's website.

Without an explicit catalog, observations use Core's authenticated, unexpired
installation-catalog cache, or the bundled catalog when no cache exists. Polling
does not access the network or write the cache. Invalid or expired cache entries
fail rather than silently restoring older bundled metadata. An explicit
`meshy-dcc dcc-types` refreshes this same signed catalog without installing an
adapter, so later observations can show newly published software independently of
the Core binary release.

Detection means `found` (one catalog path matched), `multiple`, `not_observed`
(no known path matched), or `unknown` (no platform discovery paths). It never
starts a host or infers absence from an empty registry. Versions and custom
installation paths remain unverified until the adapter checks them. There are
no projects, instance bindings or project-readiness fields in these rows.

Unity and Unreal observations cover their usual Hub/Launcher install locations.
They do not select a project editor: the adapters still resolve Unity's project
version and Unreal's `EngineAssociation`, including launcher metadata and explicit
custom locations. A missing observation does not block an existing project launch
plan or establish that an adapter cannot connect.

`meshy-dcc prepare-adapter --dcc-type <type> [--dcc-path <executable>]
[--catalog <file>] --json` returns a plan. Add `--execute` to authorize the
catalog-pinned download, checksum verification and machine/user-level setup.
The terminal status is `prepared` or `failed`; a successful response includes
`adapter_version` and the optional diagnostic `package_dir`.

Planning reads the same local catalog as observations. With `--execute`, Core
refreshes its existing signed installation feed before planning and acquisition,
using the same rollback, expiry and withdrawal checks as `install`. An unavailable
official feed does not authorize acquisition from cached or bundled metadata.
Explicit `--catalog` files retain their existing local behavior and do not refresh
or replace the official cache.

Preparation does not run project steps, create a native project or open a project
session. Global installer commands may invoke a host's installation CLI.
Its receipt records the verified package and adapter-owned project commands in
Core's existing installation state. Later project configuration uses the
prepared package; the download server need not remain available. `prepared`
records successful preparation and the presence of its required package files;
it does not prove a running host's readiness or continuous integrity of every
user-level hook. The package manager still owns component updates.
Older project installation receipts are separate from this preparation receipt;
`prepared: false` does not invalidate their project configuration or launch plans.

## MCP lifecycle

The gateway retains `search`, `describe`, `load_skill` and `call` for actual
backend execution, and advertises one additional `lifecycle` tool. Its actions:

| Action | Inputs | Result |
| --- | --- | --- |
| `software` | none | The observations above, using the server's configured catalog |
| `configure_project` | `dcc_type`, absolute existing native `project`, stable `request_id`; optional `dcc_path` | Accepted operation; executes only the prepared adapter's project steps |
| `start` | `dcc_type`, existing `project`, stable `request_id`; optional exact `instance_id` | Accepted operation; opens or reuses the exact project and observes readiness |
| `operation` | `operation_id`, or the original `request_id` | Original caller-scoped operation |
| `wait` | same handle, optional `timeout_ms` | Bounded observation of the same operation; at most 30 seconds per request |
| `stop` | original owned launch `operation_id`, new stable `request_id`; optional exact `instance_id` | Accepted safe-stop operation, without discarding unsaved work |
| `active_work` | optional `agent_name`, `session_id`, `turn_id` filters | Activity evidence described below |

Mutating operations return promptly after their admission is persisted:

```json
{"schema_version":1,"operation_id":"<uuid>","request_id":"<stable-id>",
 "action":"start","state":"accepted","dcc_type":"godot",
 "project":"/absolute/native-project","agent_context":{"session_id":"<session>"}}
```

States are `accepted`, `running`, `completed`, `failed` and `outcome_unknown`.
A terminal observation adds `result` or `error`. Start results retain the CLI's
separate `ready`, `launched`, `reused`, `owned`, full `instance_id`, PID and
project evidence. A completed stop means its callback returned; inspect the
instance/process again to establish exit. Host readback still establishes
scene changes. Configuration success is not registration or readiness.

`dcc_path` is a `configure_project` selector, never a `start` override. Project
commands must declare that they consume it; otherwise Core refuses the selector
before admission. A global adapter with no project commands can retain the
same verified prepared/published launch target. Other choices require the
adapter's native project/launch configuration or preparation of a supported
target; start then consumes the resulting launch plan.

Keep one `request_id` for one mutation and repeat it only with identical
arguments to reconcile a lost response. Core returns the original operation
without dispatching again; changed arguments are refused. The original caller
is derived from canonical protocol context, never from tool arguments. Provide
stable `agent_context.session_id` for reconnect recovery; a legacy MCP session
without it cannot acquire a different session's ownership by reconnecting.
`agent_context.turn_id` and release metadata are preserved as origin evidence.
These correlation fields are not authentication or a filesystem sandbox.

The request's `meshy-dcc/timeout_ms` budget bounds admission and status waits.
An accepted asynchronous operation has its own `timeout_ms` (default ten
minutes, maximum fifteen). An expired admission never dispatches new work.
Queued operations check their deadline before beginning another effect.
Expiry or cancellation of a wait does not guarantee stopping an installer or
host; Core retains the original handle and reports uncertainty. No client
should automatically replay an uncertain mutation.

The MCP surface does not accept model-supplied consent to lose work.
`discard_unsaved: true` is refused before dispatch. After explicit operator
confirmation, use the existing CLI `stop-instance --operation-id <original>
--discard-unsaved` workflow. Merely allowing a connection is not discard
consent. Ordinary MCP stop preserves the adapter's unsaved-work refusal.

## Project creation and reuse

Use ordinary authorized file/code operations to inspect and create native
projects. Reuse an existing compatible project when the workflow calls for it;
otherwise create a new native project in the authorized working folder without
overwriting existing files. Core does not invent a universal project format.
For example a Godot project needs `project.godot`; Unity needs its native
project/version files. Follow the host's creation contract before asking Core
to configure its adapter.

For project-bound work, use `lifecycle.configure_project`, wait for its result,
then `lifecycle.start` to open or reuse that exact project. Keep the returned
operation and instance identity, then discover real host tools and read back
the native project/scene. This also records a recoverable, actual lifecycle
association. Reusing an external or another caller's process never grants
permission to dispose of it. The gateway does not inherit its process-wide
environment as a caller's ownership or approval context.

Gateway and host launches remove inherited UI target selectors (process ID,
window handle, process name, window title and UI application type). Each new
host establishes its own target; an adapter's current-process validation remains
in force. Backend preferences, registry/data/catalog settings and workspace
context remain available, and host launch stamps its own operation/project.

Local `list` preserves the verified owned `launch` projection and adds optional
`launch.agent_context`. It may also include `associations` with
`{operation_id, source: "lifecycle", agent_context}` from completed real lifecycle
records. Each association must still match the live full instance ID, PID,
DCC type and canonical project. These are derived views, not another binding
registry. Absence does not mean a host is unusable, and association is not
ownership. Remote rows cannot forge local ownership or associations.

Explicit contradictory project metadata defeats an old launch operation or PID
during reuse and registration. Conflicting project aliases are not ownership
evidence. Legacy rows with no project metadata retain operation/PID fallback;
that fallback cannot prove the host's current native project, so native readback
still establishes the working document before edits.

## Activity and mode changes

`meshy-dcc active-work --json [--agent-name <name>] [--session-id <id>]
[--turn-id <run>]` is read-only and never starts a gateway. It uses
`GET /v1/activity` independently of the MCP client. The same projection is
available through `lifecycle.active_work`.

The response has `schema_version: 1`, `state: idle | busy | unknown`, the
requested `scope`, `lifecycle_operations`, `pending_calls`, `jobs.pending` and
`jobs.running`. It observes accepted/running lifecycle operations, actual
gateway calls and the backend Core JobManager, including queued jobs. Job
origin is recorded before admission and persists in the existing job store;
children inherit it when they have no separate origin.

`unknown_instances`, `unobserved_instances` and unattributed counts expose
coverage gaps. In scoped queries an unrelated legacy backend with no attribution
support is not automatically assigned to the caller. A relevant associated
backend whose work cannot be observed remains unknown. A stopped/unreachable
gateway cannot establish idleness. Legacy jobs without origin are not silently
assigned to a new client. Adapter-owned job handoffs remain unknown where Core
has no terminal evidence in a tracked Core job; a completed Core dispatcher is
not completion proof for its external job. A synchronous adapter-owned handoff
without a Core job row is outside JobManager coverage after its call returns.
This observes Core work, not arbitrary UI actions or commands that bypass Core.

Changing a client's routing mode does not uninstall adapters, stop the shared
gateway, close native hosts or cancel their jobs. Use the existing operation
and job handles to reconcile relevant unknown work before changing modes.

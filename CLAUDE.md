# Project guidance

## Scope and current stage

`meshy-dcc-core` is a private, independent product repository, developed as a
submodule of `meshy-dev/meshy-integrations`. Read `README.md` and `docs/plan.md`
before proposing changes. The repository currently contains planning documents
only; do not imply that a runtime or adapter has been implemented or validated.

The confirmed objective is one managed DCC integration for the user. Build the
common connection, adapter lifecycle, instance routing, and capability system
before choosing a specific Meshy-to-game-engine workflow.

Start new work by inspecting context and stating the intended change and checks.
A planning request authorizes a plan; implement when the user requests it.

## Design constraints

- Reuse existing public runtime, adapter, and CLI contracts before adding code.
  Upstream `dcc-mcp-core` is a candidate dependency, not an accepted dependency yet.
- Keep host-specific installation, main-thread dispatch, and API behavior in
  adapters. Do not add application-name branches to common routing for each host.
- Keep the agent-facing tool set bounded; discover and load capabilities on demand.
- Route by a concrete live instance and project/scene context. Never silently pick
  the first process or retry a mutation against another instance.
- A process or endpoint being alive does not establish dispatch readiness, and an
  accepted command does not establish that the scene was changed successfully.
- Preserve user scenes and files. Respect explicit workspaces, protect credentials,
  and report unknown outcomes rather than blindly replaying mutating operations.
- Do not promise universal cancellation, rollback, or lossless asset conversion.
  Expose the guarantees the selected host actually provides.
- Keep Meshy authentication, billed task creation, and downloads in `meshy-cli`.
  Integrate through its public machine-output contract, not sibling source imports.
- Use public upstream releases for dependencies. Research archives are read-only
  background; do not copy internal source trees, documents, or environment data.

## Verification and Git

There is no build system yet. Validate documentation links and Git/submodule state
for planning changes. Runtime work needs focused runnable checks and real-host
evidence; mock registration is not proof that a DCC can execute calls.

Use the authenticated `gh` CLI for GitHub operations. Keep this repository's
commits separate from the superproject. Push a submodule commit before recording
its pointer in the superproject; preserve unrelated sibling changes. Do not
publish packages or change repository visibility without user authorization.

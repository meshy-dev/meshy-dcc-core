# Project guidance

## Scope and current stage

`meshy-dcc-core` is a private, independent Meshy product repository, developed
as a submodule of `meshy-dev/meshy-integrations`. Its Rust source was imported
from `dcc-mcp/dcc-mcp-core` (MIT) `v0.20.36` without upstream git history; see
`UPSTREAM.md`. Product version 0.1.0; crates keep 0.20.36 as the internal
wire/compat version (gateway election and adapter checks compare it).

Read `docs/plan.md` before proposing changes; it lists the confirmed direction,
DCC scope and phases. Inherited upstream docs (`docs/`, `AI_AGENT_GUIDE.md`,
`README_zh.md`) describe the imported code, not the Meshy product.

Start new work by inspecting context and stating the intended change and checks.
A planning request authorizes a plan; implement when the user requests it.

The agent-facing entry is the `meshy-dcc` CLI (built from `crates/dcc-mcp-cli`)
plus `skills/meshy-dcc/SKILL.md`. No MCP connector is exposed to the agent.
Meshy API calls, authentication, billing, polling and downloads belong to the
published `meshy-cli`; core only consumes its machine-output contract.

The Node.js prototype 0.3.30 lives on the local branch
`archive/js-prototype-0.3.30` as a behavior reference only; do not revive the JS
code. Some of its constraints came from test-environment problems, so re-check
each one on a real host before porting it as a Rust test.

## Fork rules

- Rename only the product surface: binary name, Meshy-owned state and config,
  Skill, docs. Keep internal crate names, the `dcc_mcp_core` Python module,
  `DCC_MCP_*` host protocol variables and the registry layout, so upstream
  adapters keep working and upstream patches still apply.
- Put Meshy-specific behavior in new modules or Meshy skill packs rather than
  scattering edits through upstream files. Keep upstream `LICENSE` and notices.
- Take upstream fixes as reviewed patches from a separate upstream clone (see
  `UPSTREAM.md`); never add an upstream remote or merge upstream history.
- Scope is the nine DCCs with a Meshy Send To bridge (Blender, Godot, Unity,
  Unreal, Maya, 3ds Max, Roblox, Omniverse, ZBrush). Don't add other hosts.
- Host adapters are Meshy-owned: imported from upstream `dcc-mcp-<host>`
  source (new for Roblox and Omniverse), each bundling this repo's core wheel.
  The existing Send To plugins stay separate; take only their import rules
  (PBR channels, color space, host pitfalls), not their code.

## Design constraints

- Keep host-specific installation, launch, main-thread dispatch and API behavior
  in adapters. Do not add per-host branches to common routing.
- Route every call to a concrete live instance and project/scene context.
  Never pick the first process or fail over to another instance.
- A live process or endpoint does not establish readiness, and an accepted
  command does not establish that the scene changed. Read back after writes.
- Preserve user scenes and files, reject path and symlink escapes, protect
  credentials, and report unknown outcomes instead of replaying mutations.
- Do not add Meshy API calls, OAuth, download logic or a second auth store.
- Only list OS × host × adapter combinations in the support matrix after a real
  host check passes; a mock or loopback fixture is not host support.

## Verification and Git

Toolchain: Rust `1.95.0` (edition 2024). Focused checks:

```sh
cargo build -p dcc-mcp-cli
cargo test -p dcc-mcp-cli
```

Real-host checks start only test-owned projects and processes and never touch an
existing user project. Private development records live in the ignored
`docs/local/`; never force-add them, and keep local absolute paths, instance
UUIDs and private task logs out of tracked files.

Use the authenticated `gh` CLI for GitHub operations. Local commits are allowed;
do not push, publish, open PRs or change repository visibility unless the user
explicitly asks for that task. Keep this repository's commits separate from the
superproject.

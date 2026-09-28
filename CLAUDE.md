# Project guidance

## Scope and current stage

`meshy-dcc-core` is a private Meshy product repository, developed as a submodule
of `meshy-dev/meshy-integrations`. It is a Rust fork of `dcc-mcp/dcc-mcp-core`
(MIT) that keeps upstream history; the fork base is tag `v0.20.36`. Stage: R0
(fork skeleton). Read `docs/plan.md` before proposing changes; it lists the
confirmed direction, DCC scope and phases. Upstream's own docs (`README.md`
of upstream, `docs/`, `AI_AGENT_GUIDE.md`) describe the inherited code, not the
Meshy product.

Start new work by inspecting context and stating the intended change and checks.
A planning request authorizes a plan; implement when the user requests it.

The agent-facing entry is the `meshy-dcc` CLI (built from `crates/dcc-mcp-cli`)
plus `skills/meshy-dcc/SKILL.md`. No MCP connector is exposed to the agent.
Meshy API calls, authentication, billing, polling and downloads belong to the
published `meshy-cli`; core only consumes its machine-output contract.

The Node.js prototype 0.3.30 lives on the local branch
`archive/js-prototype-0.3.30` as a behavior reference only. Port its verified
constraints (listed in `docs/plan.md`) as Rust tests; do not revive the JS code.

## Fork rules

- Rename only the product surface: binary name, Meshy-owned state and config,
  Skill, docs. Keep internal crate names, the `dcc_mcp_core` Python module,
  `DCC_MCP_*` host protocol variables and the registry layout, so upstream
  adapters keep working and upstream fixes can be cherry-picked.
- Put Meshy-specific behavior in new modules or Meshy skill packs rather than
  scattering edits through upstream files. Keep upstream `LICENSE` and notices.
- Track upstream with the `upstream` remote; merge or cherry-pick by tag.
- Scope is the nine DCCs with a Meshy Send To bridge (Blender, Godot, Unity,
  Unreal, Maya, 3ds Max, Roblox, Omniverse, ZBrush). Don't add other hosts.

## Design constraints

- Keep host-specific installation, launch, main-thread dispatch and API behavior
  in adapters. Do not add per-host branches to common routing.
- Route every call to a concrete live instance by full UUID and project/scene
  context. Never pick the first process or fail over to another instance.
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

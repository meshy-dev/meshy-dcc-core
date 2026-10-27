# Project guidance

## Scope and current stage

`meshy-dcc-core` is a private, independent Meshy product repository, developed
as a submodule of `meshy-dev/meshy-integrations`. Version 0.2.1: one version
line for crates, the Python wheel and the product.

The code began as a fork of an MIT-licensed project and has fully diverged
from it. There is no upstream to track: no patches, no shared names, no
compatibility with other runtimes. Keep `LICENSE` and `NOTICE` intact; they are
the only required attribution.

Read `docs/plan.md` before proposing changes; it lists the confirmed direction,
DCC scope and phases.

On a Windows machine, start from `docs/windows-validation.md`: the Windows test
plan, and the results of the first Windows run at its end.

Start new work by inspecting context and stating the intended change and checks.
A planning request authorizes a plan; implement when the user requests it.

The agent-facing entry is the `meshy-dcc` CLI (built from `crates/meshy-dcc`)
plus `skills/meshy-dcc/SKILL.md`. No MCP connector is exposed to the agent.
Meshy API calls, authentication, billing, polling and downloads belong to the
published `meshy-cli`; core only consumes its machine-output contract.

The Node.js prototype 0.3.30 lives on the local branch
`archive/js-prototype-0.3.30` as a behavior reference only; do not revive the JS
code. Some of its constraints came from test-environment problems, so re-check
each one on a real host before porting it as a Rust test.

## Naming and scope

- Everything is Meshy-named: the CLI crate `meshy-dcc` and the other crates
  `meshy-dcc-*`, Python package
  `meshy_dcc_core`, environment variables `MESHY_DCC_*`, registry and state
  under `meshy-dcc`, gateway port 19765.
- Host adapters are Meshy-owned and live together in one repository,
  `meshy-dev/meshy-dcc-adapters` (a directory per host, its own version line
  and releases tagged `<host>-v<version>`), each built against this repo's
  `meshy_dcc_core` wheel. Never release an adapter from this repository: its
  GitHub `releases/latest` must stay core's. The bundled catalog
  (`meshy-dcc-catalog.yml`) lists only Meshy adapters; an entry stays
  `not_available` until its build is published.
- Other hosts reach users as opt-in Meshy marketplace packages, built the same
  way when someone needs them; add hosts to the bundled catalog only when the
  user decides.
- The Meshy Send To plugins stay separate; take only their import rules and
  host-side processing logic (as Meshy skill packs), not their transport code.

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
- Besides installs and updates, the only data that leaves the machine is a bug
  report the user sends with `meshy-dcc report` (or **Send** in the window) after
  seeing it. Its intake (`scripts/reports/`) holds no GitHub credentials.
- Only list OS × host × adapter combinations in the support matrix after a real
  host check passes; a mock or loopback fixture is not host support.

## Verification and Git

Toolchain: Rust `1.95.0` (edition 2024). Focused checks:

```sh
cargo build -p meshy-dcc
cargo test -p meshy-dcc
```

Real-host checks start only test-owned projects and processes and never touch an
existing user project. Private development records live in the ignored
`docs/local/`; never force-add them, and keep local absolute paths, instance
UUIDs and private task logs out of tracked files.

Use the authenticated `gh` CLI for GitHub operations. Local commits are allowed;
do not push, publish, open PRs or change repository visibility unless the user
explicitly asks for that task. Keep this repository's commits separate from the
superproject.

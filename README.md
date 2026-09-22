# meshy-dcc-core

One local entry point for agents to work across a creator's DCC applications.

Meshy DCC Core aims to manage application adapters, discover running instances,
load capabilities on demand, and route work to the right scene or project.
Users should install and configure one Meshy integration instead of finding,
installing, and configuring a separate MCP server for every application.

**Status: planning. No runtime, installer, or supported-DCC release exists yet.**

## Product direction

- One installation and one agent-facing connection, with adapters managed through it.
- Explicit application, process, scene, and project identity for every operation.
- A small discovery/dispatch surface; application capabilities load when needed.
- Structured results, readback, diagnostics, and recoverable installation changes.
- New DCCs join through adapters; creative workflows build on the shared core.

Host plugins are still necessary where an application requires them. Core owns
their setup experience; users should not have to assemble the underlying MCP stack.

## Plan

Read [the initial design and milestones](docs/plan.md). It records the confirmed
scope, proposed architecture, upstream reuse evaluation, and acceptance criteria.
The first implementation milestone validates the core across multiple instances
and two different host applications. A particular Meshy-to-engine workflow is a
later consumer of that core.

Prefer reusing the public `dcc-mcp` runtime and adapters where they pass our
installation, compatibility, and execution checks. That dependency decision is
pending a runnable evaluation; this repository is not currently an upstream fork.

## Workspace

This private repository is a submodule of `meshy-dev/meshy-integrations` at
`meshy-dcc-core/`. It owns the local DCC integration product. Meshy API calls,
authentication, generation, and downloads remain owned by `meshy-cli`; agent
guidance remains coordinated with `meshy-3d-agent`.

Development guidance lives in [CLAUDE.md](CLAUDE.md), with `AGENTS.md` pointing to
the same file. There is no build or test command until implementation begins.

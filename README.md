# meshy-dcc-core

One local entry point for agents to work across a creator's DCC applications
and game engines: install adapters once, discover running instances, load
capabilities on demand, and route every call to the right scene or project.

**Status: 0.1.0, early development.** Verified so far: macOS arm64 with
Blender 5.0.1 (list, search, load-skill, describe, call, readback). Other
hosts are not in the support matrix yet.

## Components

| Binary | Role |
|---|---|
| `meshy-dcc` | Agent-facing CLI: instances, capability search, calls, adapter install, marketplace |
| `meshy-dcc-server` | Local gateway/sidecar daemon that routes to host instances |

Agents use the CLI together with the `meshy-dcc` Skill; no MCP connector has to
be configured. Meshy generation, authentication and downloads stay in the
published [`meshy-cli`](https://www.npmjs.com/package/meshy-cli).

Meshy-owned adapters cover the DCCs with a Meshy Send To bridge: Blender,
Godot, Unity, Unreal, Maya, 3ds Max, Roblox, Omniverse and ZBrush. The bundled
install catalog also offers a curated set of upstream dcc-mcp adapters (Houdini,
Cinema 4D, Substance 3D, Photoshop and others) at upstream versions. See
[docs/plan.md](docs/plan.md) for the roadmap and support status.

## Build

Requires Rust 1.95.

```sh
cargo build -p dcc-mcp-cli -p dcc-mcp-server   # produces meshy-dcc and meshy-dcc-server
cargo test  -p dcc-mcp-cli
```

## Origin and license

Built from the MIT-licensed [dcc-mcp-core](https://github.com/dcc-mcp/dcc-mcp-core)
v0.20.36 source; see [UPSTREAM.md](UPSTREAM.md) and [LICENSE](LICENSE).
Meshy releases use their own version numbers.

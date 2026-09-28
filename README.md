# meshy-dcc-core

One local entry point for agents to work across a creator's DCC applications
and game engines: install adapters once, discover running instances, load
capabilities on demand, and route every call to the right scene or project.

**Status: 0.2.0, early development.** Verified so far: macOS arm64 with
Blender 5.0.1: list, search, load-skill, describe, call and readback, plus
host-package install, start-instance and consent-gated stop. Other hosts are
not in the support matrix yet.

## Components

| Component | Role |
|---|---|
| `meshy-dcc` | Agent-facing CLI: instances, capability search, calls, adapter install, marketplace |
| `meshy-dcc-server` | Local gateway/sidecar daemon that routes to host instances |
| `meshy_dcc_core` | Python wheel that Meshy host adapters embed |

Agents use the CLI together with the `meshy-dcc` Skill; no MCP connector has to
be configured. Meshy generation, authentication and downloads stay in the
published [`meshy-cli`](https://www.npmjs.com/package/meshy-cli).

Host adapters are Meshy-owned, one repository per host. The bundled install
catalog (`meshy-dcc-catalog.yml`) lists the Meshy adapters for these hosts:
- Blender, Unity, Godot, Unreal, ZBrush, Maya, 3ds Max, Roblox and Omniverse;
- Houdini, Cinema 4D, Marmoset, Substance 3D Painter and Designer, Mari,
  OpenUSD, Katana and MotionBuilder;
- SketchUp, FreeCAD, OpenSCAD, Material Maker and Photoshop.

See [docs/plan.md](docs/plan.md) for the roadmap and support status.

## Build

Requires Rust 1.95.

```sh
cargo build -p meshy-dcc-cli -p meshy-dcc-server   # produces meshy-dcc and meshy-dcc-server
cargo test  -p meshy-dcc-cli
```

## License

MIT; see [LICENSE](LICENSE) and [NOTICE](NOTICE).

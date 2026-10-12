# meshy-dcc-core

One local entry point for agents to work across a creator's DCC applications
and game engines: install adapters once, discover running instances, load
capabilities on demand, and route every call to the right scene or project.

**Status: 0.2.0, early development.** Verified so far on macOS arm64:

- Blender 5.0.1: list, search, load-skill, describe, call and readback, plus
  host-package install, start-instance and consent-gated stop.
- Unity 6000.4.10f1: per-project install, start-instance, FBX import and
  instantiate with readback, a PBR material built from a Meshy texture set
  (Built-in Standard), save and reopen, and stop (refused while scenes are
  unsaved unless the user agrees to discard them).
- Godot 4.7.2: per-project install, start-instance, import of a textured
  Meshy GLB with mesh and PBR material readback, save and reopen, and stop
  (refused while scenes or scripts are unsaved unless the user agrees to
  discard them).
- Unreal Engine 5.7.2: per-project install that builds the plugin's C++
  editor module, start-instance, import of a Meshy GLB and of a Meshy FBX
  whose texture set becomes a PBR material, readback, save and reopen, a
  rendered capture, native tools, and stop (refused while maps or assets are
  unsaved unless the user agrees to discard them). The editor needs Xcode and
  its Metal toolchain on macOS, and the build an Xcode the engine accepts.

Other hosts are not in the support matrix yet.

## Install

macOS (Apple Silicon):

```sh
curl -fsSL https://cdn.meshy.ai/meshy-dcc-core/install.sh | bash
```

Windows (PowerShell):

```powershell
irm https://cdn.meshy.ai/meshy-dcc-core/install.ps1 | iex
```

Where meshy.ai cannot be reached, use the copies on the GitHub Release:
`https://github.com/meshy-dev/meshy-dcc-core/releases/latest/download/install.sh`
and `…/install.ps1`. The installers themselves fall back to the GitHub Release
when the CDN is unreachable, check every file against the release manifest's
SHA-256, install `meshy-dcc` and `meshy-dcc-server` into `~/.local/bin`, and
then show and install the agent Skill (`meshy-dcc agent-skill`).

With npm (the CLI only; adapters always install through `meshy-dcc install`):

```sh
npm install -g meshy-dcc
meshy-dcc agent-skill --execute
```

## Components

| Component | Role |
|---|---|
| `meshy-dcc` | Agent-facing CLI: instances, capability search, calls, adapter install, marketplace |
| `meshy-dcc-server` | Local gateway daemon that routes every agent call to the named host instance; `meshy-dcc` starts it on demand, or runs the same gateway itself when it is not installed alongside |
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
cargo build -p meshy-dcc -p meshy-dcc-server   # produces meshy-dcc and meshy-dcc-server
cargo test  -p meshy-dcc
```

## License

MIT; see [LICENSE](LICENSE) and [NOTICE](NOTICE).

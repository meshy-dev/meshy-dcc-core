# meshy-dcc-core

Turn your DCC instances into one creation pipeline, powered by Meshy and your
agent.

One local entry point for agents across a creator's DCC applications and game
engines: install connectors once, route every call to the exact running
instance, and load capabilities as skills on demand, so one agent can carry a
Meshy model from generation through every app it touches.

**Status: 0.2.0, first release.** Every connector below ran end to end in the
app on the platforms shown: install, start-instance, a Meshy model brought in
and read back, save and reopen, and stop (refused while work is unsaved unless
the user agrees to discard it).

| App | macOS arm64 | Windows x64 |
|---|---|---|
| Blender | 5.0.1 | 5.1 |
| Unity | 6000.4.10f1 | 2022.3.55f1c1 |
| Godot | 4.7.2 | 4.7.2 |
| Unreal Engine | 5.7.2 | 5.7.4 |
| OpenSCAD | 2026.10.15 snapshot | 2021.01 |
| Material Maker | 1.7 | 1.7 |
| FreeCAD | 1.1.4 | 1.1.4 |
| Roblox Studio | 0.743 | 0.743 |
| Houdini | not run yet | 21.0.440 |
| Maya | not run yet | 2027.2 |
| 3ds Max | no macOS version | 2027 |
| Substance 3D Painter | not run yet | 12.1.5 |
| ZBrush | not run yet | 2026.2.1 |

- Roblox Studio: imported models last for the Studio session only (Roblox
  does not save them with a place yet).
- Unreal Engine builds the plugin's C++ module at install: Xcode and its Metal
  toolchain on macOS, Visual Studio 2022 on Windows.
- Material Maker: the connector works on the graph files and runs Material
  Maker for exports; it does not drive an open Material Maker window.
- Cinema 4D is not supported in this release.
- The Windows programs are not code-signed yet, so SmartScreen or Smart App
  Control may stop them.

## Install

macOS (Apple Silicon):

```sh
curl -fsSL https://cdn.meshy.ai/meshy-dcc-core/install.sh | bash
```

Windows (PowerShell):

```powershell
irm https://cdn.meshy.ai/meshy-dcc-core/install.ps1 | iex
```

In mainland China, where cdn.meshy.ai does not resolve, use cdn.meshy.cn
(`https://cdn.meshy.cn/meshy-dcc-core/install.sh`, `…/install.ps1`); the
GitHub Release carries copies too
(`https://github.com/meshy-dev/meshy-dcc-core/releases/latest/download/install.sh`).
The installers themselves try cdn.meshy.ai, cdn.meshy.cn and then the GitHub
Release, check every file against the release manifest's
SHA-256, install `meshy-dcc` and `meshy-dcc-server` into `~/.local/bin`, and
then show and install the agent Skill (`meshy-dcc agent-skill`).

With npm (the CLI only; adapters always install through `meshy-dcc install`):

```sh
npm install -g meshy-dcc
meshy-dcc agent-skill --execute
```

`meshy-dcc ui` opens the Meshy DCC Core window in the browser: connected apps,
connectors to install, the skills each app offers (your own ones included), and
the agents that have the Skill.

## Components

| Component | Role |
|---|---|
| `meshy-dcc` | Agent-facing CLI: instances, capability search, calls, adapter install, marketplace |
| `meshy-dcc-server` | Local gateway daemon that routes every agent call to the named host instance; `meshy-dcc` starts it on demand, or runs the same gateway itself when it is not installed alongside |
| `meshy_dcc_core` | Python wheel that Meshy host adapters embed |

Agents use the CLI together with the `meshy-dcc` Skill; no MCP connector has to
be configured. Meshy generation, authentication and downloads stay in the
published [`meshy-cli`](https://www.npmjs.com/package/meshy-cli).

Host adapters are Meshy-owned and live together in
[meshy-dcc-adapters](https://github.com/meshy-dev/meshy-dcc-adapters). The bundled install
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

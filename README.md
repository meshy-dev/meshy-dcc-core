# Meshy DCC Core

Turn your DCC instances into one creation pipeline, powered by Meshy and your
agent.

Meshy DCC Core gives an AI agent one local entry point to the 3D apps and game
engines you have open. Install a connector once per app; every call goes to
the exact running instance, and each app's tools load as skills when they are
needed, so one agent can carry a Meshy model through every app it touches.

Documentation: [docs.meshy.ai](https://docs.meshy.ai/en/webapp/plugins/meshy-dcc/introduction)

## Supported apps

| App | Versions |
|---|---|
| Blender | 4.5 – 5.1 |
| Unity | 2018.4 – 6000.4 |
| Godot | 4.4 – 4.7 |
| Unreal Engine | 5.0 – 5.7 |
| Maya | 2025 – 2027 |
| 3ds Max | 2025 – 2027 |
| Houdini | 20.5 – 21.0 |
| Substance 3D Painter | 8.1 – 12.1 |
| ZBrush | 2026.1 – 2026.2 |
| Roblox Studio | current release |
| FreeCAD | 1.0 – 1.1 |
| OpenSCAD | 2021.01 – 2026.10 |
| Material Maker | 1.7 |

On macOS (Apple Silicon) and Windows x64; 3ds Max on Windows only.

## Install

macOS:

```sh
curl -fsSL https://cdn.meshy.ai/meshy-dcc-core/install.sh | bash
```

Windows (PowerShell):

```powershell
irm https://cdn.meshy.ai/meshy-dcc-core/install.ps1 | iex
```

Or with npm:

```sh
npm install -g meshy-dcc
meshy-dcc agent-skill --execute
```

In mainland China, use `cdn.meshy.cn` in place of `cdn.meshy.ai`.

Then add a connector for each app, from `meshy-dcc ui` or with
`meshy-dcc install --dcc-type <app>`.

## Components

| Component | Role |
|---|---|
| `meshy-dcc` | The CLI agents use, with the `meshy-dcc` agent Skill |
| `meshy-dcc-server` | The local gateway that routes each call to its app instance |
| `meshy_dcc_core` | The Python runtime the connectors are built on |

Connectors live in [meshy-dcc-adapters](https://github.com/meshy-dev/meshy-dcc-adapters).
Meshy generation and your Meshy account stay with
[`meshy-cli`](https://www.npmjs.com/package/meshy-cli).

## Build

Requires Rust 1.95.

```sh
cargo build -p meshy-dcc -p meshy-dcc-server
cargo test -p meshy-dcc
```

## License

MIT; see [LICENSE](LICENSE) and [NOTICE](NOTICE).

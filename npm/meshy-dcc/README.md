# meshy-dcc

Turn your DCC instances into one creation pipeline, powered by Meshy and your
agent. Meshy DCC Core gives your agent one entry point to every Blender, Unity,
Godot and Unreal instance you have open: each call goes to the exact instance,
tools load as skills on demand, and every change is read back.

```sh
npm install -g meshy-dcc
meshy-dcc agent-skill            # shows where the agent Skill would go
meshy-dcc agent-skill --execute  # installs it for the agents on this computer
meshy-dcc ui                     # opens the Meshy DCC Core window
```

Published for macOS on Apple Silicon and Windows x64. Upgrade with
`npm install -g meshy-dcc@latest`; the agent Skill follows the CLI version.

MIT licensed; see `LICENSE` and `NOTICE`.

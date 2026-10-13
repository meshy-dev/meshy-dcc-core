# meshy-dcc

Meshy DCC Core lets an AI agent work inside the Blender, Unity, Godot or Unreal
you have open: find the running instance, load its tools, call them and read the
scene back, including importing Meshy-generated models.

```sh
npm install -g meshy-dcc
meshy-dcc agent-skill            # shows where the agent Skill would go
meshy-dcc agent-skill --execute  # installs it for the agents on this computer
meshy-dcc ui                     # opens the Meshy DCC Core window
```

Published for macOS on Apple Silicon and Windows x64. Upgrade with
`npm install -g meshy-dcc@latest`; the agent Skill follows the CLI version.

MIT licensed; see `LICENSE` and `NOTICE`.

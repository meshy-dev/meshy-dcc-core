# Installing and removing connectors

A connector is the Meshy adapter for one host. Install or remove one only when the user
asks, and with the host (or that project) closed.

## Install

```sh
meshy-dcc install --dcc-type blender --execute --json
meshy-dcc install --dcc-type unity --project /absolute/project --execute --json
```

Unity, Godot and Unreal install into each project (`--project`); see their references.
Blender installs once per computer.

- `status` `ok` or `partial` is a normal install: the host registers when it starts.
- `error.code` `DCC_PATH_REQUIRED`: the host was not found in its usual places, or more
  than one copy was. `error.message` lists any copies found. Ask the user which app to
  use and run again with `--dcc-path`, for example
  `--dcc-path "/Applications/Blender 4.5.app"`.
- Any other failure explains itself in `steps[].message`; pass it on.

## Remove

There is no uninstall command. With the user's go-ahead and the host closed:

| Host | Remove |
|---|---|
| Blender | Run `<Blender executable> --command extension remove user_default.meshy_dcc_blender` (on macOS the executable is `Blender.app/Contents/MacOS/Blender`) |
| Unity | The project's `Packages/ai.meshy.dcc` and `.meshy-dcc` folders |
| Godot | The project's `addons/meshy_dcc_godot` and `.meshy-dcc` folders, and `res://addons/meshy_dcc_godot/plugin.cfg` from `[editor_plugins] enabled` in `project.godot` |
| Unreal | The project's `Plugins/MeshyDccUnreal` and `.meshy-dcc` folders |

Then delete the install record so the Meshy DCC Core window stops listing it: the file
in the `installs` folder of the data directory (`MESHY_DCC_DATA_DIR`, otherwise
`~/Library/Application Support/meshy-dcc` on macOS and `%APPDATA%\meshy-dcc` on
Windows) named `<host>.json`, or for a project `<host>-<hash>.json` whose `project` field
is that project. Delete only the paths listed here, and list them to the user first.

To remove Meshy DCC Core itself: `meshy-dcc agent-skill --remove --execute`, then
delete `meshy-dcc` and `meshy-dcc-server` from `~/.local/bin` (or
`npm uninstall -g meshy-dcc`). `~/.meshy-dcc` holds the user's settings and own skills;
delete it only if they say so.

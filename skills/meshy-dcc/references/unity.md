# Unity

Verified on Unity 6000.4.10f1, macOS arm64, with adapter `meshy-dcc-unity` 0.1.0.

- The adapter has two parts:
  - the Editor package `ai.meshy.dcc`, copied into each project's `Packages/`;
  - a sidecar process that serves the tools and talks to the Editor.
- The Editor starts the sidecar itself. `list` shows the sidecar's PID as the instance
  `pid`; `meshy_unity__session` reports the Editor's `host_pid`.
- Each Editor gets its own bridge port, so several projects can be open at once.

## Set up a project

Installing writes the package and `.meshy-dcc/` (launch plan, sidecar config, receipt)
into the project. It picks the Editor that matches `ProjectSettings/ProjectVersion.txt`,
so that Editor version must be installed. With the user's go-ahead, and with that
project's Editor closed:

```sh
meshy-dcc install --dcc-type unity --project /absolute/project --execute --json
```

A failed step explains itself in `steps[].message`. Afterwards `dcc-types --project`
recommends `start_instance`.

## Tools used most

| Skill | Tools |
|---|---|
| `meshy-unity` | `session`, `inspect_object`, `inspect_renderers` (read-only); `inspect_model`, `instantiate_model`, `save_scene_as`, `open_scene` |
| `unity-scene` | `inspect_scene` (read-only); `create_game_object`, `set_transform`, `save_scene` |
| `unity-project` | `inspect_project`, `refresh_assets`, `refresh_and_compile` |

## Import a Meshy model

1. Copy the FBX or OBJ to a new folder below `Assets/`. Unity cannot import GLB.
2. `meshy_unity__inspect_model` with the `Assets/...` path imports it and returns the mesh,
   vertex, material and animation counts.
3. `meshy_unity__instantiate_model` adds it to the active scene with Undo and returns its
   `instance_id`. Without `material_textures` it reports `geometry_only`.
4. Read it back with `meshy_unity__inspect_object` and `meshy_unity__inspect_renderers`.
5. Save with `meshy_unity__save_scene_as` (`Assets/.../Name.unity`) only when asked. A
   project that was never saved has no scene path; `session.scene` is empty until then.

Unity's `dirty` flag is reliable. `open_scene` refuses while a scene is dirty, and so does
`stop-instance`, with `unsaved_changes`, unless the user agreed to `--discard-unsaved`.

A keyed texture set (`material_textures` with each PNG's `bytes` and `sha256`, all in the
FBX's folder) maps to one Standard (Built-in) or Lit (URP) material. The tool does not
install URP or switch pipelines.

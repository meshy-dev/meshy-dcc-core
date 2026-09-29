# Unreal

Verified on Unreal Engine 5.7.2, macOS arm64, with adapter `meshy-dcc-unreal` 0.1.0.

- The adapter is a project plugin, `Plugins/MeshyDccUnreal`: a C++ editor module plus a
  Python layer on the editor's own Python. `list` shows the editor's PID as the instance `pid`.
- On macOS the editor needs Xcode with its license accepted and the Metal toolchain; without
  them it quits at startup with "Xcode Not Found". Tell the user; installing Xcode needs
  their Apple ID and an administrator password.
- A first launch of a project compiles shaders and can take several minutes.

## Set up a project

Installing copies the plugin into the project, builds its C++ module with the engine's
build script (a few seconds to minutes), and writes `.meshy-dcc/` (launch plan, build log,
receipt) next to the `.uproject`; the `.uproject` itself is not changed. The engine comes
from the project's `EngineAssociation` (for example `5.7`) and the Epic Games Launcher's
install list; for a source build or another location, set `MESHY_DCC_UNREAL_ENGINE` to the
engine root. With the user's go-ahead, and with that project closed in Unreal:

```sh
meshy-dcc install --dcc-type unreal --project /absolute/project --execute --json
```

A failed step explains itself in `steps[].message`. The installer refuses, and changes
nothing, while `Plugins/MeshyDccUnreal` holds files it did not write; pass the message on.
When the module does not build (installer exit 20), the message quotes the build log: the
machine needs Visual Studio on Windows, or on macOS an Xcode inside the engine's range
(Unreal 5.7 accepts up to 26.9 and refuses Xcode 27). Tell the user what is missing; do not
edit the engine's configuration to get around it.

## Tools used most

| Skill | Tools |
|---|---|
| `meshy-unreal` | `session`, `inspect_model`, `capture_model` (read-only); `import_model` |
| `unreal-level` | `get_level_info` (read-only); `create_level`, `load_level`, `save_level` |
| `unreal-actors` | `list_actors`, `get_actor_transform` (read-only); `set_actor_transform` |

## Import a Meshy model

1. `meshy_unreal__session`. If `level_in_project` is false (the editor opened an engine
   template map), create a project level first:
   `unreal_level__create_level` with `{"level_path": "/Game/Maps/<name>", "template_path":
   "/Engine/Maps/Templates/Template_Default"}` gives sky, lights and a floor. Never save an
   engine map.
2. Download the model to disk. A GLB carries its textures. For an FBX, keep Meshy's PNGs
   next to it: `<name>.png`, `<name>_normal.png`, `<name>_metallic.png`,
   `<name>_roughness.png`.
3. `meshy_unreal__import_model` with the absolute `source_path` and a `name` imports into
   `/Game/Meshy/<name>`, saves those assets and places the mesh with its base at `location`
   (default the origin). An FBX gets one PBR material built from its texture set. It
   refuses a folder that already holds assets; pick another name rather than retrying.
4. Read it back with `meshy_unreal__inspect_model` and `actor` set to the name: texture
   roles, sRGB, compression and green-channel flip show whether the material is right.
5. `meshy_unreal__capture_model` with the actor and a `file_name` renders
   `<project>/Saved/MeshyDcc/<file_name>.png`, also while the editor is in the background.
   Meshy models face +Y after import; `view: front` looks at the face.
6. Save with `unreal_level__save_level` only when asked, then check that `session.dirty`
   is false.

Unreal may build Nanite for a dense Meshy mesh (`nanite: true`); after a reload, vertex and
triangle counts are the fallback mesh's.

`session.dirty` covers the project's unsaved maps and assets (`session.unsaved` lists
them). `session.engine_unsaved` lists engine content that got dirty on its own; it is not
the user's work and must never be saved. `stop-instance` refuses with `unsaved_changes`
while the project has unsaved work, unless the user agreed to `--discard-unsaved`.

# Unity

Verified on Unity 6000.4.10f1 (macOS arm64) and 2022.3.55f1c1 (Windows x64), with adapter
`meshy-dcc-unity` 0.1.0.

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
| `unity-project` | `inspect_project`, `read_text_asset` (read-only); `refresh_assets`, `upsert_text_asset`, `refresh_and_compile`, `set_play_mode` |

The scene tools create empty GameObjects and set transforms. No tool adds components,
creates primitives or deletes objects; that needs scripts (below).

## Scripts and other source files

Writing C# and other text assets below `Assets/` is off unless the operator started Unity
with `MESHY_DCC_UNITY_ALLOW_SOURCE_WRITES=1`; tool arguments cannot turn it on.
`meshy_unity__session` reports it as `source_writes`. If it is `false` and the task needs
scripts, ask the user to restart Unity with the setting, or report that the scene has no
scripts. Don't put files into `Assets/` some other way to get around it.

When it is on:

1. `unity_project__upsert_text_asset` writes one file: a new UUID `request_id`, an
   `Assets/...` path ending in `.cs`, `.json`, `.txt`, `.shader`, `.cginc`, `.asmdef`,
   `.xml`, `.uss` or `.uxml`, at most 256 KiB of UTF-8, and `expected_sha256` `absent` for a
   new file or the digest from `read_text_asset` to replace one. It is an async job; call it
   with `--wait`.
2. `unity_project__refresh_and_compile` (async, `--wait`) imports and compiles, and fails
   with the compile errors; `unity_diagnostics__read_console` shows them in full.
3. Use current Unity 6 APIs. An obsolete API can make Unity stop and ask to update the
   scripts, which blocks the Editor until someone answers.

If a Unity job ends with `context.reason: host_unresponsive`, the Editor's main thread
stopped answering for longer than the threshold (default 120 s): Unity may be showing a
dialog or be stuck in a long import. Do not submit the job again. Tell the user, and once
`unity_diagnostics__ping` succeeds, call `unity_diagnostics__inspect_job` with the same
`request_id`: the job may have finished.

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

## Materials from a Meshy texture set

A keyed texture set maps to one Standard (Built-in) or Lit (URP) material. The tool does
not install URP or switch pipelines. Put the FBX and its PNGs in the same new folder below
`Assets/` and pass that folder as `material_root`.

A Meshy "FBX with textures" download holds `<name>.fbx` and these PNGs:

| File | Role and key (set 0) |
|---|---|
| `<name>.png` | `base_color`, `texture_0_base_color` |
| `<name>_normal.png` | `normal`, `texture_0_normal` |
| `<name>_metallic.png` | `metallic`, `texture_0_metallic` |
| `<name>_roughness.png` | `roughness`, `texture_0_roughness` |

Each entry of `material_textures` needs `key`, `path` (`Assets/...`), `bytes` and `sha256`
of the file as copied; the tool re-checks them. It writes `MeshyMaterials/` next to the
FBX: sRGB base color, a NormalMap, and one linear map with metallic in red and
1 − roughness in alpha. `material_delivery.status` is `verified` only after readback.

## Looking at the result

`unity_diagnostics__capture_game_view` needs Play Mode and a rendering Game View. An
Editor in the background does not render, and the capture times out, unless the project
has Run In Background on (`session.run_in_background`). Ask the user to bring Unity to
the front or to enable that setting; do not change project settings yourself. Meshy
characters face +Z, so Unity's default camera sees their back.

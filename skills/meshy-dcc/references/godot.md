# Godot

Verified on Godot 4.7.2 on macOS arm64 and Windows x64, with adapter `meshy-dcc-godot` 0.1.0.
On Windows the editor executable is never named `godot`: set `MESHY_DCC_GODOT_EXECUTABLE`
to it (or put it on `PATH` under that name) before install and start.

- The adapter has two parts:
  - the EditorPlugin, copied into each project's `addons/meshy_dcc_godot` and enabled in
    `project.godot`;
  - a sidecar process that serves the tools and talks to the editor.
- The editor starts the sidecar itself and the sidecar exits with it. `list` shows the
  sidecar's PID as the instance `pid`; `meshy_godot__session` reports the editor's
  `host_pid`.
- Each editor gets its own bridge port, so several projects can be open at once.

## Set up a project

Installing writes the addon, enables it, and writes `.meshy-dcc/` (launch plan, sidecar
config, receipt) into the project. It finds Godot on `PATH` (`godot4`, `godot`) or as
`Godot.app` in `/Applications` or `~/Applications`; for another location, set
`MESHY_DCC_GODOT_EXECUTABLE` to the editor binary. With the user's go-ahead, and with
that project closed in Godot:

```sh
meshy-dcc install --dcc-type godot --project /absolute/project --execute --json
```

A failed step explains itself in `steps[].message`. Running it again for the same
adapter version changes nothing.

## Tools used most

| Skill | Tools |
|---|---|
| `meshy-godot` | `session`, `inspect_model` (read-only); `prepare_asset`, `import_model` |
| `godot-scene-management` | `get_scene_tree` (read-only); `create_scene`, `open_scene`, `save_scene` |
| `godot-editor` | `get_editor_screenshot` |

## Import a Meshy model

1. Download the GLB (textures are embedded) and copy it below the project's
   `assets/meshy/`. Godot imports GLB natively; `meshy-godot` accepts nothing else.
2. `meshy_godot__prepare_asset` with the `res://assets/meshy/<name>.glb` path starts a
   scan. Poll `meshy_godot__session` with `asset_path` until `asset_ready` is true; a
   textured GLB of a few MB takes tens of seconds. Godot extracts the textures next to
   the GLB as it imports.
3. Open or create a 3D scene: `godot_scene_management__create_scene` with
   `{"path": "res://scenes/<name>.tscn", "root_type": "Node3D", "open": true}`.
4. `meshy_godot__import_model` with `path` and a node `name` instances the GLB once, with
   undo. It returns the readback: meshes, vertex and triangle counts, bounds, and each
   surface's `StandardMaterial3D` with its albedo, metallic, roughness, normal and
   emission textures.
5. Read it back later with `meshy_godot__inspect_model` and `node_path` set to that name.
6. Save with `godot_scene_management__save_scene` only when asked, then check that
   `session.dirty` is false.

`session.dirty` covers unsaved scenes and scripts (`session.unsaved` lists them).
`stop-instance` refuses with `unsaved_changes` while anything is unsaved, unless the
user agreed to `--discard-unsaved`.

## Looking at the result

`godot_editor__get_editor_screenshot` with `{"path": "res://.meshy-dcc/<name>.png",
"mode": "3d"}` writes a PNG of the editor's 3D viewport and works with the editor in the
background. Meshy characters face +Z, which is toward Godot's default editor view.

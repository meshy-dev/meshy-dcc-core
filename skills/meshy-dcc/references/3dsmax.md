# 3ds Max

Adapter `meshy-dcc-3dsmax`, for 3ds Max 2025 and later; 3ds Max runs on Windows only.

- The adapter starts with 3ds Max from a startup script, and each 3ds Max process is one
  `3dsmax` instance. Tools run on 3ds Max's main thread; a long render or a modal dialog
  makes calls wait.
- If no `3dsmax` instance is listed while 3ds Max is open, the MAXScript Listener shows why
  (a line starting `meshy-dcc-3dsmax did not start`). Ask the user to open it.

## Set up

Installs once per Windows user and 3ds Max version. With the user's go-ahead and 3ds Max
closed:

```sh
meshy-dcc install --dcc-type 3dsmax --execute --json
```

- `error.code` `DCC_PATH_REQUIRED`: several 3ds Max versions are installed, or none in
  `C:\Program Files\Autodesk`. Ask which one and pass
  `--dcc-path "C:\Program Files\Autodesk\3ds Max 2026\3dsmax.exe"`.
- The installer's own codes arrive in `steps[].message`:
  - `host_never_started`: that 3ds Max was never opened by this user. Ask them to start it
    once and close it, then install again.
  - `dcc_version_unknown`: 3ds Max is not in a folder named `3ds Max <year>`; not supported.
  - `startup_script_foreign`: a file of the same name that the installer did not write;
    pass the path on to the user.
- The adapter starts the next time 3ds Max starts, or with
  `meshy-dcc start-instance --dcc-type 3dsmax --project <folder> --wait-ready --yes`
  (`--project` is required; 3ds Max opens no scene from it).

## Tools used most

| Skill | Tools |
|---|---|
| `3dsmax-scene` | `get_scene_status`, `get_scene_info`, `list_scene_nodes` (read-only); `save_scene`, `save_scene_as`, `open_scene`, `new_scene` |
| `3dsmax-geometry-io` | `import_fbx` (`file_path`, optional `units`, `up_axis`, `include_animation`), `import_geometry` (FBX, OBJ, 3DS) |
| `3dsmax-materials` | `create_physical_material`, `assign_bitmap_texture`, `apply_material`, `inspect_material`, `list_bitmap_connections` |

The adapter has no glTF/GLB import. Use Meshy's FBX download. `import_fbx` returns
`created_nodes` and `created_count`; read back with `list_scene_nodes`.

A Meshy FBX download puts `<name>.png` (base color), `<name>_normal.png`,
`<name>_metallic.png` and `<name>_roughness.png` next to `<name>.fbx`.
`create_material_from_textures` sorts textures by filename keywords, so it usually leaves
the plain `<name>.png` unassigned. Wire the texture set slot by slot instead:

1. `create_physical_material` with a `name`.
2. `assign_bitmap_texture` once for each `slot`: `base_color`, `normal`, `roughness`,
   `metalness`. Pass `texture_path` as an absolute path. A normal map is wrapped in a Normal
   Bump map.
3. `apply_material` with `node_names` from `created_nodes`, then `inspect_material`.

Each write is checked by reading it back. A slot the material does not expose fails the
call instead of reporting success.

- Normal, roughness, metallic and other data maps are opened as data (OCIO's data colour
  space; gamma 1.0 in the older gamma workflow). Colour maps follow 3ds Max's input rules,
  sRGB for PNG and JPEG. `assign_bitmap_texture` sets this only when it opens the file, so
  wire a map again with the tool rather than changing it by hand.
- A normal map on a Physical material gets the full bump amount (`bump_map_amt` 1.0), as
  the Send To plugin does.
- Meshy normal maps are OpenGL style (green up). Whether 3ds Max expects green down is
  untested.

## Saving and unsaved work

- `get_scene_status` returns `current_file_path` (empty when never saved) and `dirty`
  (3ds Max's own flag). Every tool that is not read-only runs as one undo step, which is
  what sets that flag; Python run in the session some other way may not set it.
- `save_scene` saves to the current file and fails with `current_scene_has_no_path` if
  there is none. `save_scene_as` takes an absolute `.max` `file_path`, and also `overwrite`
  for a file that already exists. Both read the saved path back.
- `open_scene` and `new_scene` refuse while `dirty` unless `force` is true. Save first, or
  get the user's OK before passing `force`.
- `stop-instance` on a 3ds Max that `start-instance` launched is refused unless the user
  agrees to lose unsaved work (`--discard-unsaved`), because 3ds Max then quits without
  asking to save.

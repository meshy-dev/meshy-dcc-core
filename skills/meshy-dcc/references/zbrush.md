# ZBrush

Adapter `meshy-dcc-zbrush`, for ZBrush 2026.1 and later on macOS and Windows. Install checks
that ZBrush is signed by Maxon and refuses one that is not. The first start after install can
restart ZBrush through Explorer and lose the script; if `start-instance` times out, run it
again.

- The adapter installs per project folder: the folder that holds the work (models to import,
  saved `.zpr` and `.ztl` files). Nothing goes into the ZBrush installation or its settings.
- Only a ZBrush started by `meshy-dcc start-instance` has the adapter. A ZBrush the user
  opened by hand is not listed; ask before starting a new one.
- Each ZBrush process is one `zbrush` instance. ZBrush runs one request at a time; a second
  call waits briefly and is retried for you.

## Set up and start

With the user's go-ahead:

```sh
meshy-dcc install --dcc-type zbrush --project <folder> --execute --json
meshy-dcc start-instance --dcc-type zbrush --project <folder> --wait-ready --yes
```

- `install` finds ZBrush in `/Applications/Maxon ZBrush 20*` or
  `C:\Program Files\Maxon ZBrush 20*`. With several versions or another location, ask which
  one and pass `--dcc-path` (the `ZBrush.app` or `ZBrush.exe`). It refuses a ZBrush older
  than 2026.1 or not signed by Maxon.
- ZBrush may ask the user to sign in to Maxon when it starts; `start-instance` then waits.

## Tools used most

| Skill | Tools |
|---|---|
| `zbrush-scene` | `get_scene_info` (`active_tool_path`, `subtool_count`, `active_subtool_index`), `list_subtools` (index and visible/locked flags, no names) |
| `zbrush-import-to-scene` | `import_to_scene` (`asset_id`, `variants: [{local_path, format: "obj"}]`) |
| `zbrush-file` | `save_project` (`.zpr`), `save_tool` (`.ztl`), `open_project` |
| `zbrush-subtool` | `select_subtool`, `inspect_active_mesh`, `remesh_active_subtool`, `refine_active_subtool`, `bake_active_subtool_map` |
| `zbrush-interchange` | `export_active_subtool_obj` |
| `zbrush-scripting` | `get_session_info`; `execute_python` (last resort, `zbc` is `zbrush.commands`) |

Use Meshy's OBJ download. ZBrush imports OBJ without a dialog; FBX opens an interactive
importer and is refused, and ZBrush has no GLB importer. The import brings geometry only:
`unit_hint`, `up_axis` and `material_mode` are accepted but not applied, and textures are not
loaded. An OBJ face with more than four vertices is refused before ZBrush sees it (it would
open a blocking Note). When the active tool already has a mesh, the import adds a subtool and
keeps the existing ones. It returns `imported_nodes` (the new subtool's name); read back with
`get_scene_info` (`subtool_count`, `active_tool_path`).

## Saving and unsaved work

- ZBrush cannot report unsaved changes. Treat everything since the last save as unsaved.
- `save_project` and `save_tool` take an absolute `path` with the right extension and an
  existing folder. An existing file is refused (`FILE_EXISTS`) unless `overwrite: true`. Both
  return the written `path` and `bytes`.
- `open_project` replaces what ZBrush has open without asking; save first or get the user's
  OK. It returns the loaded `active_tool_path` and `subtool_count`.
- `stop-instance` is refused (`unsaved_changes`) unless the user agreed to lose unsaved work
  (`--discard-unsaved`); it then ends the ZBrush process that `start-instance` launched.

## When ZBrush does not answer

- `ZBRUSH_BLOCKED`: ZBrush is inside a native call, possibly a dialog waiting for a person.
  Do not retry; tell the user.
- `ZBRUSH_TIMEOUT_STILL_RUNNING`: the request is still running in ZBrush. Do not send it
  again; poll `get_session_info` until it answers.

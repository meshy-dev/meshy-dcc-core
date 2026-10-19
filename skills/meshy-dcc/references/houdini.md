# Houdini

Verified on Houdini 21.0.440 on Windows 11, with adapter `meshy-dcc-houdini` 0.1.0, built for
Houdini 20.5 and later (the macOS build has not run in Houdini). Checked: install, start, a
Meshy GLB with a MaterialX PBR material, save, stop, reopen, uninstall; the rest is read from
the adapter source.

- Houdini looks for its preferences under `HOME` first. When the agent's environment sets
  `HOME` (common on Windows), install writes into `$HOME/houdini<X.Y>`: a Houdini started
  with `start-instance` reads it, one started from the Start menu does not. Tell the user
  when they plan to open Houdini themselves.

- `meshy-dcc install --dcc-type houdini` writes one package file,
  `meshy_dcc_houdini.json`, into that Houdini version's user preferences
  (`~/Library/Preferences/houdini/<X.Y>/packages` on macOS, `$HOME/houdini<X.Y>/packages`
  on Windows). With several versions installed, pass `--dcc-path`; each version is
  installed separately.
- The server starts when the UI is ready (`uiready.py`), in interactive sessions only.
  Each Houdini process is one instance. Headless `hython` and the adapter's own
  background render and cook processes never register.
- At startup only `houdini-scripting` and `houdini-scene` are loaded; load other skills
  as needed.
- `houdini_scripting__get_session_info` reports `houdini_version`, `hip_file` (`null`
  for a scene that was never saved) and `ui_available`.

## Tools used most

| Skill | Tools |
|---|---|
| `houdini-scene` | `get_scene_info`, `list_obj_nodes`, `list_child_nodes`, `get_node_info`, `inspect_selection` (read-only) |
| `houdini-scene-edit` | `save_scene`, `open_scene`, `new_scene`, `find_nodes`, `get_bounding_box` |
| `houdini-interchange` | `probe_file`, `import_geometry` (bgeo, OBJ, FBX, Alembic, USD; glTF/GLB through the GLTF SOP; a file it cannot read fails the call), `export_fbx`, `export_usd` |
| `houdini-import-to-scene` | `import_to_scene` (takes an asset descriptor, `{"asset_id", "variants": [{"local_path"}]}`; GLB/glTF, OBJ, USD materials become `principledshader` nodes in `/mat`) |
| `houdini-materials` | `build_materialx_pbr` (base color, roughness, metallic, normal textures; color maps `srgb_texture`, data maps `Raw`), `assign_material` |
| `houdini-nodes` | `create_node`, `set_node_parms`, `connect_nodes`, `cook_node` |

## Meshy models

- No tool wires texture maps on import. `import_to_scene` rebuilds only scalar PBR
  values (base color, metallic, roughness, emissive, opacity) from a GLB, and reports
  FBX materials as `unsupported_format` with a `material_fallback` warning. Treat
  `success: true` as "geometry arrived" and check `warnings` and
  `extra.materials_imported`.
- For a textured look: create a `geo` node under `/obj` (`houdini_nodes__create_node`),
  import the FBX into it with `import_geometry` (`parent_path` `/obj/<geo>`), build the material with `build_materialx_pbr` from
  Meshy's texture files, then `assign_material` to the geo node.
- Read back with `houdini_scene__get_node_info` or `houdini_geometry__get_geometry_info`.

## Saving and unsaved work

- `save_scene` takes an optional `file_path`. Without one it saves to the current hip
  file and fails if the scene was never saved. It returns the path Houdini wrote.
- `new_scene` and `open_scene` refuse while the scene has unsaved changes unless
  `force` is true. Save first, or get the user's OK to discard.
- `stop-instance` on a Houdini started by `start-instance` refuses while
  `hou.hipFile.hasUnsavedChanges()` is true, unless the user agreed to
  `--discard-unsaved`; it then exits without the save prompt.

## Pitfalls

- On Windows the preferences follow Houdini's `$HOME`: the `HOME` variable when set,
  else the Documents folder. A `HOME` set only in a terminal (Git Bash sets one) puts
  the package file where a Houdini started from the Start menu does not look.
- A Houdini whose path names no version (`Houdini21.0.440`, `Versions/21.0`) needs
  `MESHY_DCC_HOUDINI_VERSION=<X.Y>` for the install.
- `houdini_render__render_rop` runs as a background job by default, and it and
  `houdini_nodes__start_cook_job` refuse a scene with unsaved changes; save first.

# Maya

Adapter `meshy-dcc-maya`, for Maya 2023 and later on macOS and Windows.

- `meshy-dcc install --dcc-type maya` writes one file, `meshy_dcc_maya.mod`, into the user's
  Maya modules folder. Every Maya the user starts afterwards loads the adapter, and each Maya
  process is one instance (dcc type `maya`). Pass `--dcc-path` when several Maya versions are
  installed.
- Maya on Windows keeps the module's files open: close Maya before a reinstall.

## Tools used most

| Skill | Tools |
|---|---|
| `meshy-maya` | `import_model` (Meshy FBX or OBJ, with a PBR `standardSurface` built from the texture set), `inspect_model` |
| `maya-scene` | `get_session_info`, `list_objects`, `get_bounding_box` (read-only); `save_scene`, `new_scene`, `open_scene` |
| `maya-geometry` | `import_file`, `import_fbx`, `export_fbx`, `export_obj` |
| `maya-primitives` | `create_sphere`, `create_cube` and other primitives |

## Meshy models

- Maya has no built-in glTF importer: download Meshy's FBX. Keep Meshy's maps next to it
  (`<name>.png`, `<name>_normal.png`, `<name>_metallic.png`, `<name>_roughness.png`, and
  `<name>_emission.png` when there is one) or pass them as `textures`. The material reads the
  maps where they are, so keep the files.
- `import_model` puts everything the import added under a new group `name` and refuses a name
  that already exists. It is one undo step. Don't repeat it when the outcome is unknown; call
  `inspect_model` with the group name instead.
- The material is one `standardSurface` `<name>_mat` on every imported mesh: colour maps sRGB,
  metallic, roughness and normal maps Raw, normal through a `bump2d` set to tangent-space
  normals. A packed `metallic_roughness` map is read as glTF: green roughness, blue metalness.
  `inspect_model` reports, per input, the file, whether it exists, its colour space and the
  channel used; read those back instead of assuming the wiring.
- `inspect_model` reports bounds in the scene's linear unit (Maya's default is cm). Check the
  size after an import; the FBX importer's unit conversion has not been checked on a Meshy file.

## Saving and unsaved work

- `get_session_info` reports `scene_file` (`<unsaved>` if never saved) and `scene_modified`.
- `save_scene` saves in place, or to `file_path` (`.ma` or `.mb`) first. Without `file_path`
  it fails on a scene that was never saved, instead of opening a Save As dialog.
- `new_scene` and `open_scene` return an error on a modified scene unless `force` is set.
  Save first or get the user's OK to discard.
- `meshy-dcc stop-instance` on a Maya started by `start-instance` refuses while
  `scene_modified` is true, or when Maya does not answer within 30 s, unless the user agreed
  to discard (`--discard-unsaved`). Maya then quits without a save prompt.

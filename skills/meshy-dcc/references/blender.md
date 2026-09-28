# Blender

Verified on Blender 5.0.1, macOS arm64, with adapter `dcc-mcp-blender` 0.2.11.

- The adapter is a Blender extension.
- Its server starts with Blender, and each Blender process is one instance.
- `list` showed `scene: null` for an unsaved file. `get_session_info` reports the open
  file as `scene_file`, or `<unsaved>` if it was never saved.

## Tools used most

| Skill | Tools |
|---|---|
| `blender-interchange` | `import_file` (FBX, OBJ, glTF/GLB, Alembic; optional `collection_name`), `import_fbx`, `import_obj`, `import_usd`, `export_gltf` |
| `blender-scene` | `list_objects`, `get_scene_info`, `get_session_info` (read-only); `save_scene`, `new_scene`, `open_scene` |
| `blender-mesh-ops` | `create_primitive`, `get_poly_count` |

A GLB import returns `context.imported_object_names`. Read back with `list_objects`, which
returns each object's name, type, location and visibility.

## Saving and unsaved work

- `save_scene` takes an optional `filepath`, an absolute `.blend` path. Without one it
  saves to the current file, and fails if the file was never saved.
- After saving, read back `get_session_info`: `scene_file` holds the saved path, or
  `<unsaved>` if nothing was saved.
- Blender's `scene_modified` flag is not reliable. It stays `false` right after an import,
  so don't use it to decide whether work is unsaved.
- `new_scene` and `open_scene` replace the current scene. Before calling either, save the
  scene or get the user's OK to discard it.

## Tools with an empty schema

Some tools, `save_scene` among them, publish only `{"type": "object"}`. Their arguments
are the keyword parameters of the tool's script, which lives inside the extension:

```text
<Blender user resources>/extensions/user_default/dcc_mcp_blender/skills/<skill>/tools.yaml
<Blender user resources>/extensions/user_default/dcc_mcp_blender/skills/<skill>/scripts/<tool>.py
```

On macOS the user resources directory is `~/Library/Application Support/Blender/<major.minor>/`
unless `BLENDER_USER_RESOURCES` is set. Read the script only to learn its arguments; call
the tool through `meshy-dcc`, never by running the script.

## Render settings

Blender 5.0 names the EEVEE engine `BLENDER_EEVEE`, not `BLENDER_EEVEE_NEXT`. After
changing render settings, read back the engine, resolution and samples.

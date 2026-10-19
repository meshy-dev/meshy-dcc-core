# FreeCAD

Adapter `meshy-dcc-freecad`, on macOS and Windows. It accepts FreeCAD 1.0.x and 1.1.x and
refuses other versions.

- The adapter is a headless service, not a FreeCAD plugin. No FreeCAD window opens, and
  nothing is installed into FreeCAD or its preferences.
- It installs per project: `install --dcc-type freecad --project <dir>`, then
  `start-instance` with the same `--project`. The instance serves that project.
- Every call runs in a fresh FreeCADCmd process and works on `.FCStd` files. Documents,
  and models to import, must be inside the project directory. Paths outside it are
  refused.
- `list` shows `scene: null`; the service has no open document.

## Tools used most

Most tools run as jobs: pass `--wait` to `call`.

| Skill | Tools |
|---|---|
| `freecad-session` | `list_documents`, `inspect_document`, `validate_document`, `get_status` (read-only); `create_document`, `save_copy`, `remove_object`, `create_snapshot`, `restore_snapshot` |
| `freecad-modeling` | `import_geometry` (STEP, IGES, BREP, STL, OBJ, GLB), `export_geometry` (STEP, IGES, BREP, STL, OBJ, 3MF), `add_primitive`, `transform_object`, `boolean_operation` |
| `freecad-sketch`, `freecad-feature`, `freecad-modify` | Sketches, PartDesign features, fillets, chamfers, patterns |

To bring in a Meshy model, put the GLB inside the project, then call `import_geometry`
with `document_path`, `input_path` and a new `object_name` (letters, digits and
underscores). Create the document first with `create_document` if it does not exist.

- A GLB lands as a `Mesh::Feature`, with materials and textures dropped.
- Metres become millimetres, and glTF's Y-up becomes FreeCAD's Z-up.
- The result lists `verified_checks`, including `source.triangles`.
- Only `.glb` is accepted, not `.gltf`.
- Read back with `inspect_document`, which returns each object's type, label, and mesh
  points, facets and bounding box.

## Saving and unsaved work

- Every mutating call saves the document before it returns. There is no unsaved state
  and no separate save tool.
- A call that fails leaves the document unchanged.
- `save_copy` writes a copy to a new path and leaves the source unchanged. It refuses an
  existing destination unless `overwrite` is true.
- `stop-instance` is refused while a FreeCAD call is running. With `--discard-unsaved`,
  the stop cancels the running call. That call returns no result, and its document stays
  as it was before the call.

## Gotchas

- `render_view` does not work on macOS: Qt has no offscreen OpenGL there. The tool
  refuses with `render_host_limited`, and `get_capabilities` reports it as
  `host_limited`.
- The adapter edits the file on disk and does not talk to a running FreeCAD. A document
  the user already has open in FreeCAD is not updated; ask them to reopen it.

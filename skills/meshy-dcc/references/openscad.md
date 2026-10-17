# OpenSCAD

Verified with the OpenSCAD 2026.10.15 snapshot, macOS arm64, with adapter
`meshy-dcc-openscad` 0.1.0. The Windows package has not run on Windows yet.

- OpenSCAD has no embedded Python. The instance is the adapter process, which
  runs the `openscad` command line once per call; no OpenSCAD window opens.
- It installs per project, a directory of `.scad` files:
  `meshy-dcc install --dcc-type openscad --project <dir>`, then
  `start-instance` with the same `--project`. That directory is the
  workspace: relative paths resolve there, and paths outside it are refused.
- `list` shows the OpenSCAD version (`openscad@2026.10.15-<short>`) and
  `scene: null`.
- Supported OpenSCAD: `2021.01.x`, `2026.09.x`, `2026.10.x`. Install and every
  modelling call refuse other versions with an `openscad_host_version_*`
  error. 2021.01 is Intel-only on macOS; Apple Silicon needs a snapshot.

## Tools used most

| Skill | Tools |
|---|---|
| `openscad-pipeline` | `get_status`, `inspect_source` (read-only); `write_model_source`; `validate_model`, `export_model`, `render_preview` |
| `meshy-openscad` | `write_print_base` |

`validate_model`, `export_model` and `render_preview` return a job. Poll
`jobs_get_status` with its `job_id` until `status` is `completed` or `failed`.

## Writing and changing models

- `write_model_source` only creates files. It refuses an existing path, and
  any `include`, `use`, `import` or `surface`, even in a comment.
  `output_path` must be absolute.
- To change a design, pass `parameters` (top-level variables, as JSON values)
  to `validate_model`, `export_model` or `render_preview`, or write a new
  file. Overriding `width` and `height` on a box changed the exported STL's
  bounds from 40×40×60 to 60×60×30.
- An export refuses an existing output unless `overwrite: true`. A refused
  export leaves the file unchanged.
- Each export reads its file back before it returns: `context.bytes`,
  `context.sha256` and `context.verification_checks`, with
  `postcondition.verified`. `inspect_source` lists a file's variables,
  modules and `include`/`use` references without running it.

## Saving and unsaved work

Every write is a complete file on disk when its call returns; nothing is held
in the instance. `stop-instance` always proceeds. After a restart,
`inspect_source` read the same source, and exporting it again gave a
byte-identical STL.

## Meshy models for 3D printing

1. Copy the Meshy model as STL into the project, for example `meshy/`.
2. `write_print_base` with an absolute `output_path` and `mesh_path` writes a
   new `.scad` that imports the mesh, scales it to `target_height` mm, centres
   it and stands it on a `round` or `square` base (`base_shape: "none"` leaves
   the base off). Pass `up_axis: "y"` for a mesh converted from Meshy's GLB,
   or the model lies on its back.
3. Export `.stl` or `.3mf` with `export_model` and look at a `render_preview`
   (`full_render: true` for final geometry). Pass `target_height`,
   `base_shape`, `base_height`, `base_margin` or `sink` as `parameters` to
   resize without writing another file.

A 50 mm Meshy diorama on a round base exported as a manifold STL (52.5 mm
tall with the base) under the default strict mode. Imported meshes that
OpenSCAD only warns about need `strict: false`.

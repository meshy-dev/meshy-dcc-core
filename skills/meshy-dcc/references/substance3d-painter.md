# Substance 3D Painter

Adapter `meshy-dcc-substance3d-painter`, for Substance 3D Painter 8.1 and later on macOS and
Windows.

- The adapter is a startup plugin in the user's Painter profile
  (`Documents/Adobe/Adobe Substance 3D Painter/python/startup/`). Its server starts with Painter,
  and each Painter process is one instance. Painter 8.1 or later (Python 3.9+).
- Install once per machine, with no `--project`. A Painter that was already running loads the
  adapter only after a restart. `start-instance` still needs `--project` (any folder; Painter
  opens no project from it).
- Every tool runs on Painter's main thread. The `painter-project` tools are jobs: call them with
  `--wait`, or poll `jobs_get_status`.
- `painter_diagnostics__ping` is loaded at startup and proves the main thread answers.

## Tools used most

| Skill | Tools |
|---|---|
| `painter-project` | `create_project`, `open_project`, `save_project`, `save_project_as`, `close_project`, `reload_mesh`; `inspect_project`, `list_texture_sets`, `list_layer_stack` (read-only); `import_resource`, `create_textured_pbr_layer`, `create_pbr_fill_layer`, `apply_smart_material`; `bake_mesh_maps`; `create_export_preset`, `export_textures` |
| `painter-material` | `inspect_material_channels`, `set_layer_channels`, `set_layer_channel_value` |
| `painter-compositing` | `inspect_layer_compositing`, `set_layer_visibility`, `set_layer_opacity`, `set_layer_blending_mode` |
| `painter-lighting` | `inspect_environment`, `set_environment_map`, `set_environment_exposure`, `set_environment_rotation` |

Load `painter-project` with `load-skill` before calling its tools.

## A Meshy model into Painter

1. `create_project` with `mesh_path` (FBX, OBJ, DAE, PLY, USD or glTF/GLB) and `project_path`, an
   absolute `.spp` path used as the default save path. It refuses while another project is open.
   `resolution` is one of 128 to 4096 (default 1024); `normal_map_format` is `DirectX` (default)
   or `OpenGL`.
2. `list_texture_sets` gives the exact texture-set names the other tools take.
3. `create_textured_pbr_layer` needs all five maps as files: base color, normal, roughness,
   metallic and ambient occlusion. It works on the active texture-set stack only. Meshy's GLB
   packs metallic and roughness in one map (green roughness, blue metallic): split it into two
   grayscale images, and pass a white image as AO when there is none. Painter 12 projects use
   OpenPBR channels; Painter routes the layer's roughness and metallic into
   `SpecularRoughness` and `BaseMetalness`. Create the project with
   `normal_map_format: OpenGL` for a GLB.
4. `bake_mesh_maps` takes one `texture_set` and a list of `maps` (`ambient_occlusion`,
   `curvature`, `id`, `normal`, `position`, `thickness`, `world_space_normal`). It returns a job
   at once; the finished job carries the baked resource URLs and the texture-set resolution. It
   bakes from the project's own mesh: there is no high-poly source.
5. `create_export_preset` builds a PNG preset from explicit channel routing; pass its result as
   `preset` to `export_textures` with `export_path` and `texture_sets`. The export succeeds only
   when every file Painter reports exists, and with an inline preset each PNG has the texture
   set's resolution.

## Saving and unsaved work

- `save_project_as` takes an absolute `.spp` path; `save_project` saves to the current one. Both
  succeed only when Painter then reports that file, it exists, and `needs_saving` is false.
  `create_project` does not save; whether the first `save_project` uses its
  `project_path` is unchecked, so save a new project with `save_project_as`.
- `inspect_project` reports `dirty` (Painter's `needs_saving`), `busy` and `file_path`.
- `close_project` refuses while the project has unsaved changes. Save first, or get the user's
  OK to discard them.
- A stop refuses while the open project needs saving, or when Painter cannot tell, unless
  `--discard-unsaved` is passed after the user agreed; it then closes Painter. A Painter the user
  opened themselves is never stopped.

## Uninstall

There is no `meshy-dcc uninstall` yet. Run
`<data dir>/meshy-dcc/adapters/meshy-dcc-substance3d-painter/<package>/bin/meshy-dcc-substance3d-painter uninstall --yes`
(`.cmd` on Windows; the data dir is `~/Library/Application Support` on macOS and `%APPDATA%` on
Windows), or delete `Documents/Adobe/Adobe Substance 3D Painter/python/startup/meshy_dcc_substance3d_painter_plugin.py`,
then delete the package folder. The installer never removes a file of that name it did not write.
Painter 12 caches the bytecode of every module it loads under `<profile>/pycache`; the uninstall
command also removes the adapter's part of it, deleting the loader by hand does not.

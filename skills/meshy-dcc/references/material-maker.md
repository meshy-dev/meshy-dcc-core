# Material Maker

Adapter `meshy-dcc-material-maker`, for Material Maker 1.7 and later on macOS and Windows.

- Material Maker has no plug-in interface. The adapter is a sidecar bound to one
  project folder of material graphs (`.ptex`); `start-instance` starts the
  sidecar, not the Material Maker window. `list` shows the folder as `scene`.
- The tools edit `.ptex` files directly, not an open Material Maker window (see
  below). Material Maker runs only for an export, which briefly opens it and
  takes focus while it renders (a few seconds; the first export after installing
  Material Maker takes longer), and for `open_in_material_maker`.
- Paths are relative to the project folder; nothing outside it is read or written.

## Set up a folder

With the user's go-ahead (a separate folder, not one another adapter set up):

```sh
meshy-dcc install --dcc-type material-maker --project /absolute/folder --execute --json
meshy-dcc start-instance --dcc-type material-maker --project /absolute/folder --wait-ready --yes
meshy-dcc load-skill material-maker-materials --dcc-type material-maker
```

Install finds `Material Maker.app` in `/Applications` or `~/Applications`; on
Windows, or anywhere else, pass `--dcc-path` (an `.app` is accepted). It writes
only `.meshy-dcc/` in the folder.

## Tools used most

| Skill | Tools |
|---|---|
| `material-maker-materials` | `get_status`, `get_graph`, `list_node_types`, `inspect_project` (read-only); `create_material`, `edit_graph`, `export_material`; `open_in_material_maker` (shows the user) |

- `get_status` lists the bundled examples (`bricks`, `rusted_metal`, `wood`, ...),
  the export targets and the Material Maker version.
- `create_material` with `template: "blank"` gives a graph with only the Material
  node; with an example name, a copy of that example. It never replaces a file.
- `list_node_types` with `query` finds types; with `node_type` it returns the
  type's inputs, outputs and parameters (defaults, ranges, enum values).
- `edit_graph` applies `add_node`, `remove_node`, `set_parameters`, `connect`,
  `disconnect` all or nothing and saves. Material inputs: 0 albedo, 1 metallic,
  2 roughness, 3 emission, 4 normal, 5 ambient occlusion, 6 height, 7 opacity,
  8 subsurface. Colors are `{r, g, b, a}` from 0 to 1. Unknown node types,
  parameters and ports are refused, and so are `webcam` nodes. Pass the `sha256`
  from `get_graph` as `expected_sha256`.
- Read back with `get_graph` (nodes, parameters, connections, `sha256`).

## Export

`export_material` with `project_path`, a new `output_directory` and a `target`:
a Material Maker profile from `get_status` (`Godot/Godot 4 Standard`, `Unity/URP`,
`Blender`, `GLTF/Plane`, ...) or `Godot`, `Unity`, `Unreal`, `Blender`, `GLTF`.
Give the call `--timeout-secs 300`; the CLI default of 30 s can be too short.

- Textures are 2048x2048. Godot gives `_albedo`, `_normal`, `_orm` PNGs and a
  `.tres`; Blender gives an albedo PNG and EXR roughness and metallic maps. Only
  the channels connected to the Material node are written.
- The result lists every file with its size, SHA-256 and PNG width and height;
  look at the albedo PNG to judge the material. `warnings` carries Material
  Maker's engine errors; an export with warnings still wrote its files.
- The folder must be new. After a timeout, check whether it exists before
  exporting again.
- Exporting the same saved graph again gives byte-identical files.

## The user's Material Maker window

A Material Maker window does not see the agent's edits: it does not reload a
`.ptex` changed on disk and keeps showing what it loaded, and saving there
overwrites the agent's change.

- Before changing a graph, ask the user whether it is open in Material Maker.
  If it is, ask them to save what they want to keep and close it, and wait.
- Never edit a graph while the user is editing it.
- After exporting, `open_in_material_maker` with `path` opens the graph in a new
  Material Maker window for the user and returns at once with its `pid`. It
  never touches a Material Maker the user already has open, and the window stays
  open after `stop-instance`. On macOS it can open behind the app in front (with
  Stage Manager, in the side strip): tell the user it is there.
- To change that graph again, ask the user to close the window first, then open
  it again afterwards.

## Meshy textures

An `image` node brings a project image into a graph:
`{"op": "add_node", "name": "Base", "type": "image", "parameters": {"image": "textures/base.jpg"}}`.
Connecting it to the Material's albedo and, through a `normal_map` node, to its
normal input exports the Meshy base color with a normal map derived from it.

## Saving and unsaved work

Every edit is saved when it is made, so there is never unsaved work and
`stop-instance` always stops the sidecar. It does not close windows opened with
`open_in_material_maker`; those belong to the user.

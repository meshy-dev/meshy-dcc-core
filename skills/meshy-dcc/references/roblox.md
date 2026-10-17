# Roblox Studio

Not verified on a real host yet; experimental. Adapter `meshy-dcc-roblox` 0.1.0.

- The adapter is a Studio plugin plus a sidecar. `meshy-dcc install --dcc-type roblox
  --project <dir>` needs a directory holding exactly one place file (`.rbxl` or
  `.rbxlx`); it also writes the plugin into Studio's local plugins folder.
- `start-instance` opens that place in Studio. Each instance is one place; the instance
  only talks to the Studio window of its own place, never to another one.
- `session` reports the place name, PlaceId, Studio version and whether Luau is allowed.

## Tools used most

| Skill | Tools |
|---|---|
| `roblox-scene` | `session`, `get_scene_tree`, `inspect_instance` (read-only); `create_instance`, `set_properties`, `move_instance`, `delete_instance`; `execute_luau` (off unless the user allows it) |
| `meshy-roblox` | `import_model` (absolute `.glb` path), `inspect_model` |

- Instances are addressed by path, such as `Workspace/Castle/Gate`. A path with two
  siblings of the same name is refused; names you create must be free among siblings.
- Positions are studs, `[x, y, z]`, +Y up. Property values: `[x, y, z]` for Vector3,
  `[r, g, b]` (0-1) or `"#rrggbb"` for Color3, the item name for an Enum (`"Neon"`).
- Each change is one Studio undo step and returns a readback; read the tree again
  after a change before reporting it.
- `import_model` builds a Model of MeshParts (split above 20,000 triangles) with a
  SurfaceAppearance (color, normal, metalness, roughness, at most 1024 px). glTF metres
  become studs (3.571 per metre). Read it back with `inspect_model`.

## Saving and unsaved work

- A plugin cannot save a place or tell whether it has unsaved changes; `dirty` is always
  `null`. Ask the user to save in Studio (File > Save to File, or Publish).
- An imported model lasts only for the Studio session (`persistent: false`): Roblox does
  not save editable meshes with a place. `publish: true` uploads the meshes and images to
  the logged-in user's Roblox inventory first and builds from those assets, which saves
  with the place. Ask the user before every upload; uploads that finished stay in their
  inventory even if the import fails later.
- `stop-instance` always answers `unsaved_changes`. Stop with `--discard-unsaved` only
  after the user saved or agreed to lose the changes; it then ends the Studio process that
  `start-instance` launched.

## Luau

`execute_luau` is refused until the user clicks **Allow Luau** in Studio's Meshy DCC
toolbar. Prefer the typed tools; do not ask the user to allow Luau to work around a tool
that exists.

# Cinema 4D

Not verified on a real host yet; experimental. Adapter `meshy-dcc-c4d` 0.1.0 was run
end to end on macOS arm64 only against a stand-in `c4dpy` (not Cinema 4D), so the
lifecycle below is checked, but what Cinema 4D itself does is not.

- The adapter never runs inside the Cinema 4D GUI. Its service runs in the package's own
  Python and starts Cinema 4D's licensed headless `c4dpy` once for every operation.
- Tools work on `.c4d` files on disk, not on a scene open in Cinema 4D. A document open
  in the GUI does not see the change until it is reopened there, and saving it from the
  GUI overwrites the change. Ask the user to close the document first.
- An instance is one service bound to one project folder. Every path a tool takes must
  be inside that folder; anything else fails with
  `Path is outside MESHY_DCC_C4D_ALLOWED_ROOTS`.
- `list` shows `scene: null`; `version` is the Cinema 4D release read from `c4dpy`.

## Set up a project folder

A project is any folder for the user's `.c4d` files. The installer finds `c4dpy` in
`/Applications/Maxon Cinema 4D */c4dpy.app` (macOS) or
`C:/Program Files/Maxon Cinema 4D */c4dpy.exe` (Windows); with several releases
installed, or none in those places, pass `--dcc-path` with the `c4dpy` to use. With the
user's go-ahead:

```sh
meshy-dcc install --dcc-type c4d --project /absolute/folder --execute --json
meshy-dcc start-instance --dcc-type c4d --project /absolute/folder --wait-ready --yes
```

Install writes only `.meshy-dcc/launch-plan.json` and `.meshy-dcc/c4d.json` into the
folder, and refuses a folder whose launch plan belongs to another adapter. It writes
nothing into Cinema 4D or its preferences.

## Tools used most

| Skill | Tools |
|---|---|
| `cinema4d-session` | `get_status`, `get_capabilities`, `inspect_document`, `validate_document` (read-only); `create_document`, `save_copy` |
| `cinema4d-modeling` | `add_primitive`, `transform_object`, `remove_object`, `import_geometry`, `export_document`, `render_document` |

- Start with `get_status`: it runs `c4dpy` and reports `cinema4d_version`,
  `python_version` and `ready`. Without `c4dpy` it reports `ready: false`; a `c4dpy`
  that cannot run fails the call.
- Every tool except `get_status` and `get_capabilities` is a job: call it with `--wait`.
- `import_geometry` merges a file into a document and returns `imported_paths`.
  Formats depend on the importers the Cinema 4D install has; OBJ is the baseline.
  Read back with `inspect_document`: each object's name, path, transform (translation,
  HPB rotation in degrees, scale) and bounds, plus point and polygon counts for
  polygon objects.
- `add_primitive` takes a unique `name` and the dimensions of its type: cube `size`
  [x, y, z]; sphere `radius`; cylinder `radius`, `height`; cone `bottom_radius`,
  `top_radius`, `height`; torus `ring_radius`, `pipe_radius`; plane `width`, `height`.
- `transform_object` and `remove_object` need a name no other object has.
  `remove_object` refuses an object with children unless `cascade` is true.

## Saving and unsaved work

- There is no unsaved state. Each mutating tool edits a staged copy, replaces the
  document only after `c4dpy` succeeded, then reopens it and compares; the result says
  `read_back: "compared"` and carries `document_sha256`.
- `create_document`, `save_copy`, `export_document` and `render_document` refuse an
  existing output unless `overwrite` is true.
- `export_document` and `render_document` check the written file (format signature,
  image size). When that check fails the file is already written; inspect it before
  trying again.
- `stop-instance` always stops the service. A call still running then ends with an
  unknown outcome: inspect the document before repeating it.

## Pitfalls

- Each call starts `c4dpy`, and a mutation starts it twice (change, then reopen). How
  long that takes, and whether an unlicensed or never-signed-in Cinema 4D blocks
  `c4dpy` without a window, has not been measured.
- Cinema 4D R21 and R22 ship `c4dpy` with Python 2.7 and cannot run the adapter; R23 is
  the floor.

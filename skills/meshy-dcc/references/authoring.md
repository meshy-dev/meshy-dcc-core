# Writing a skill for the user

When the user wants a tool of their own in a host (a routine they repeat, a house rule
for imports, a report), write it as a skill in their skills folder. The app picks it up
on reload, and every agent on the machine can then load and call it like a built-in one.

Supported for **Blender** only. For other hosts, tell the user custom skills are not
supported there yet.

## Where it goes

`~/.meshy-dcc/blender/skills/<skill-name>/`. Blender reads only its own folder.

- Use a name of lowercase words joined by hyphens, the same as the folder name, and
  start it with `my-` (or the studio's prefix). Never reuse a built-in name such as
  `blender-scene`: the built-in copy wins and the user's copy is ignored without an
  error. Check with `meshy-dcc search --instance-id ID --query NAME`.
- One skill can hold several tools. Add a tool to an existing skill of the user's when it
  belongs there.

## Files

```text
my-scene-report/
├── SKILL.md
├── tools.yaml
└── scripts/
    └── count_objects.py
```

`SKILL.md`: the description is what agents see when they choose a skill, so say what it
does and when to use it.

```markdown
---
name: my-scene-report
description: "Count the objects in the open Blender scene by type"
metadata:
  meshy-dcc:
    dcc: blender
    version: "0.1.0"
    stage: scene
    tags: [blender, scene, report]
    search-hint: "count objects, scene report, object types"
    tools: tools.yaml
---

# my-scene-report

Counts the objects in the current Blender scene by type.
```

`stage` is one of `scene`, `authoring`, `lookdev`, `animation`, `render`, `import`,
`pipeline`.

`tools.yaml`: one entry per tool. The call name is the skill name with `_` for `-`, two
underscores, then the tool name: `my_scene_report__count_objects`.

```yaml
tools:
- name: count_objects
  description: Count the objects in the current scene by type
  source_file: scripts/count_objects.py
  execution: sync
  affinity: main
  read_only: true
  destructive: false
  idempotent: true
  input_schema:
    type: object
    additionalProperties: false
    properties: {}
```

- `affinity: main` runs the tool on Blender's main thread; `bpy` needs it.
- Set `read_only`, `destructive` and `idempotent` truthfully; agents rely on them.
- Describe every argument in `input_schema` (types, ranges, `description`), with
  `additionalProperties: false`. Arguments arrive as keyword arguments.

`scripts/count_objects.py`:

```python
"""Count the objects in the current Blender scene by type."""

from __future__ import annotations

from meshy_dcc_core.skill import skill_entry, skill_exception, skill_success


def count_objects() -> dict:
    try:
        import bpy

        counts: dict[str, int] = {}
        for obj in bpy.context.scene.objects:
            counts[obj.type] = counts.get(obj.type, 0) + 1
        return skill_success(f"{sum(counts.values())} objects", counts=counts)
    except Exception as exc:
        return skill_exception(exc, message="Failed to count objects")


@skill_entry
def main(**kwargs) -> dict:
    return count_objects(**kwargs)
```

Return `skill_success(message, **context)`, or `skill_error(message, error)` for a
request the tool refuses. The script runs inside Blender with the user's permissions:
change only what the tool is for, never delete or overwrite the user's files, and make no
network calls unless the user asked for that.

## Check, reload, try

```sh
meshy-dcc lint ~/.meshy-dcc/blender/skills/my-scene-report
meshy-dcc reload-skills
meshy-dcc load-skill my-scene-report --instance-id ID
meshy-dcc call blender.<instance_short>.my_scene_report__count_objects --json '{}'
```

1. `lint` must report `"errors": 0`. Fix what it lists before going on.
2. `reload-skills` makes the open apps re-scan their folders. Skills already loaded stay
   loaded, and a loaded skill whose files changed is reloaded, so the same two commands
   apply after an edit.
3. Call the tool once on the user's scene, and for a tool that changes the scene, read
   the result back. If the skill does not appear after the reload, run `lint` again.

Tell the user the skill's name and folder. They can see it under **My skills** on the
Skills page of `meshy-dcc ui`.

---
name: meshy-dcc
description: Work inside a creator's running DCC application through the local meshy-dcc CLI - find the live instance, discover and load its tools, call them, and read the scene back. Use for scene edits and for importing Meshy-generated models into a DCC project. Supported today - Blender, Unity, Godot, Unreal, OpenSCAD, Material Maker and FreeCAD; experimental - 3ds Max, Cinema 4D, Houdini, Maya, Roblox Studio, Substance 3D Painter and ZBrush.
---

# Meshy DCC

`meshy-dcc` is a local CLI. DCC hosts running a Meshy DCC adapter register themselves on
the machine; the CLI lists them, searches their tools and runs a tool inside the instance
you choose. No MCP connector is involved.

**Supported now**, on macOS (arm64):

- Blender 5.0. Read [the Blender reference](references/blender.md) before Blender work.
- Unity 6 (6000.4). Read [the Unity reference](references/unity.md) before Unity work.
- Godot 4.7. Read [the Godot reference](references/godot.md) before Godot work.
- Unreal Engine 5.7. Read [the Unreal reference](references/unreal.md) before Unreal work.
- OpenSCAD (2026.10 snapshot). Read [the OpenSCAD reference](references/openscad.md) before OpenSCAD work.
- Material Maker 1.7. Read [the Material Maker reference](references/material-maker.md) before Material Maker work.
- FreeCAD 1.1. Read [the FreeCAD reference](references/freecad.md) before FreeCAD work.

**Experimental**, built and tested without the app itself. Tell the user that before relying
on one, and read back every change:

- 3ds Max 2025+, Windows only. Read [the 3ds Max reference](references/3dsmax.md).
- Cinema 4D 2023+, headless: it edits `.c4d` files in a project folder, not the open app.
  Read [the Cinema 4D reference](references/c4d.md).
- Houdini 20.5+. Read [the Houdini reference](references/houdini.md).
- Maya 2025+. Read [the Maya reference](references/maya.md).
- Roblox Studio. Read [the Roblox reference](references/roblox.md).
- Substance 3D Painter 8.1+. Read [the Substance 3D Painter reference](references/substance3d-painter.md).
- ZBrush 2026.1+, only when started with `start-instance`. Read [the ZBrush reference](references/zbrush.md).

For any other host, tell the user it is not supported yet instead of improvising.

Meshy generation, sign-in, credits and downloads belong to the published `meshy` CLI and
its `meshy-3d-generation` skill. This Skill takes over once the model file is on disk.

## Invocation

Run every command in this form:

```sh
meshy-dcc --non-interactive --output json <command> ...
```

It is shortened to `meshy-dcc <command>` below. Every command goes through the local
Meshy gateway, which routes each call to the instance you name; the CLI starts the gateway
when it is not running (it says so on stderr). `start-instance`, `stop-instance`,
`install` and `dcc-types` manage the local machine and run without it.

- Read the JSON on stdout and the exit status.
- Errors go to stderr as `{"error": {"code", "message", "exit_code", "retryable",
  "details"}}`, and the exit status is non-zero. `details.reason` names the cause, for
  example `skill_not_loaded`, `invalid_params`, `unknown_slug`, `instance_offline`,
  `instance_not_found` or a host's `unsaved_changes`. Codes: `INVALID_INPUT` and
  `NOT_FOUND` (exit 2, change the request), `UNAVAILABLE` (exit 3, may work later),
  `TIMEOUT` (exit 4), `CONFLICT` (exit 7, the target refuses in its current state, for
  example a skill that is not loaded yet), `GENERAL_ERROR` (exit 1).
- A `next_action.command` in a result is complete; run it as it is.
- If `meshy-dcc` is not on PATH, or the gateway cannot start, stop and tell the user.

## 1. Choose the instance

```sh
meshy-dcc list
```

Each entry in `instances` has:

- `dcc_type`, `instance_id`, `instance_short`, `pid` and host `version`;
- `scene`, which can be `null` (the host reference says how to read the open file), and
  `metadata.meshy_dcc_project` for a host started for a project;
- `status`, `available` when the instance takes calls.

Choose by host, scene or project, and PID. If more than one fits, ask the user. Never take
the first entry.

If no instance is listed, check whether the host can be launched for the
project:

```sh
meshy-dcc dcc-types --dcc-type blender --project /absolute/project
```

- If `next_action.id` is `start_instance`, the adapter is installed and left a
  launch plan. With the user's go-ahead to open the application, run
  `meshy-dcc start-instance --dcc-type blender --project /absolute/project --wait-ready --yes`.
  Then use the instance it reports.
- If `next_action.id` is `report_unavailable`, there is nothing to run. Tell the user
  `next_action.reason`, for example that the adapter for that host is not published yet.
- Unity, Godot and Unreal install into each project. If such a project has no
  launch plan yet, see the host reference ([Unity](references/unity.md#set-up-a-project),
  [Godot](references/godot.md#set-up-a-project), [Unreal](references/unreal.md#set-up-a-project)).
- Otherwise, ask the user to open the host with the Meshy adapter enabled.

Name that instance explicitly in every later command:

- search and load-skill take `--instance-id <instance_id>`;
- call and describe take the full slug `<dcc_type>.<instance_short>.<tool>`. The gateway
  refuses a bare tool name.

A restarted host gets a new ID and new slugs. Run `list` again, load the skills again and
use the new slugs. Never reroute a call to another instance.

## 2. Find and load the tool

```sh
meshy-dcc search --instance-id ID --query "import gltf" --limit 20
meshy-dcc load-skill SKILL_NAME --instance-id ID
```

- **Hits come best match first.** Each hit has `tool_slug`, `skill_name`, `summary` and
  `loaded`. When `loaded` is false, `next_step.action` is `load_skill`: load that
  `skill_name` on the same instance before calling the tool.
- **What load-skill returns:** `tools` lists every tool in the skill with its `name`,
  `inputSchema` and `annotations`. The call slug is
  `<dcc_type>.<instance_short>.<tools[].name>`, for example
  `blender.1a2b3c4d.blender_interchange__import_file`.
- **One tool's schema:** `meshy-dcc describe SLUG`.

## 3. Call, then read back

```sh
meshy-dcc call SLUG --json '{"path": "/absolute/model.glb"}'
meshy-dcc call SLUG --json-file args.json
```

- **Where the outcome is:** `output` has the tool's `success`, `message`, `error` and
  `context` (for example `imported_object_names`). A tool that reports failure also
  exits non-zero.
- **Delivered is not changed:** after a write, read the scene back with a read-only tool
  and compare it with what you intended.
- **Unknown outcomes:** keep `request_id`. If a call times out, is interrupted or returns
  something malformed, the outcome is unknown. Read the scene back and do not repeat the
  write.
- **Long and async tools:** raise `--timeout-secs` for long sync tools. An async tool
  returns a job; `--wait` polls that same job until it finishes, so don't resubmit it.
  With `--wait`, `structuredContent.status` is the job's final state and
  `structuredContent.result` holds the tool's `success`, `message`, `error` and `context`.
- **Many similar edits:** send up to 25 calls in one batch instead of one command each.
  The calls run in order; each result says whether it succeeded, and a failed call does not
  stop the rest unless `stop_on_error` is true.

  ```sh
  meshy-dcc call --batch --json-file batch.json
  ```

  ```json
  {"calls": [
    {"tool_slug": "blender.1a2b3c4d.blender_objects__move_object",
     "arguments": {"object_name": "Tree.001", "location": [4, 0, 0]}},
    {"tool_slug": "blender.1a2b3c4d.blender_objects__move_object",
     "arguments": {"object_name": "Tree.002", "location": [8, 0, 0]}}
  ]}
  ```

  `results[]` holds `index`, `ok`, and `result.output` or `error` for each call. Check
  every entry, and read the scene back once at the end.
- **Paths:** use absolute paths, and save only where the user asked.
- **Prefer typed tools.** Use script-execution tools (`execute_python`,
  `dcc_introspect__eval` and similar) only when the user asked for scripting.

## 4. Stopping a host

`stop-instance` works only for an instance that `start-instance` launched and that
advertises a safe-stop hook. It refuses for a host the user opened, and you should
never kill a host process.

```sh
meshy-dcc stop-instance --operation-id OPERATION_ID
```

A host with unsaved scenes refuses with `CONFLICT` and `details.reason`
`unsaved_changes` (Unity, Godot, Unreal; Unreal lists them in `details.unsaved`). A host
that cannot tell whether work is unsaved, such as Blender, refuses with
`unsaved_state_unknown`. When either happens:

1. Save what the user wants kept.
2. Ask the user whether the rest may be lost.
3. Add `--discard-unsaved` only after they agree.

If a host needs to close and you cannot stop it this way, ask the user to save and close
it.

## Installing or removing a connector

When the user asks to set up or remove a host's connector, follow
[the connector reference](references/connectors.md).

## The user's own skills

When the user wants a tool that no skill has (a routine they repeat, a house rule for
imports), write it as a skill in their skills folder. Read
[the authoring guide](references/authoring.md) first. Blender only for now.

## Meshy models

1. Get the model with the `meshy` CLI; its skill covers generation, task recovery and
   download. If a wait fails, recover the same task ID rather than submitting a new paid
   task.
2. Download a format the host imports well (GLB for Blender; FBX or OBJ for Unity, below
   the project's `Assets/`; GLB for Godot, below `assets/meshy/`; GLB, or FBX with its
   PNGs, for Unreal, anywhere on disk) into the user's project or a folder they named.
3. Import it with the host's import tool, using the absolute path.
4. Read back what arrived (object names, count, materials) and tell the user where it is.
   Save only when asked.

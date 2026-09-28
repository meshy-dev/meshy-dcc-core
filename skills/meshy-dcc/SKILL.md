---
name: meshy-dcc
description: Work inside a creator's running DCC application through the local meshy-dcc CLI - find the live instance, discover and load its tools, call them, and read the scene back. Use for scene edits and for importing Meshy-generated models into a DCC project. Supported today - Blender.
---

# Meshy DCC

`meshy-dcc` is a local CLI. DCC hosts running a Meshy DCC adapter register themselves on
the machine; the CLI lists them, searches their tools and runs a tool inside the instance
you choose. No MCP connector is involved.

**Supported now:** Blender 5.0 on macOS (arm64). Read [the Blender reference](references/blender.md)
before Blender work. For any other host, tell the user it is not supported yet instead of
improvising.

Meshy generation, sign-in, credits and downloads belong to the published `meshy` CLI and
its `meshy-3d-generation` skill. This Skill takes over once the model file is on disk.

## Invocation

Run every command in this form:

```sh
meshy-dcc --gateway local --no-auto-gateway --non-interactive --output json <command> ...
```

It is shortened to `meshy-dcc <command>` below. In this mode the CLI reads the local
registry and calls hosts directly, so no gateway daemon is needed.

- Read the JSON on stdout and the exit status.
- Errors go to stderr with a `[CODE]`, and the exit status is non-zero.
- If `meshy-dcc` is not on PATH, stop and tell the user.

## 1. Choose the instance

```sh
meshy-dcc list
```

Each entry in `instances` has:

- `dcc_type`, `instance_id`, `instance_short`, `pid` and host `version`;
- `scene`, which can be `null` (the host reference says how to read the open file);
- `direct_control.ready`.

Choose by host, scene or project, and PID. If more than one fits, ask the user. Never take
the first entry.

If no instance is listed, ask the user to open the host with the adapter enabled.
`start-instance` and `install` do not work for the supported hosts yet.

Name that instance explicitly in every later command:

- search and load-skill take `--instance-id <instance_id>`;
- call and describe take a slug that contains its `instance_short`.

A call that doesn't name its instance is refused unless the host marks the tool read-only,
even when only one instance is live.

A restarted host gets a new ID and new slugs. Run `list` again, load the skills again and
use the new slugs. Never reroute a call to another instance.

## 2. Find and load the tool

```sh
meshy-dcc search --instance-id ID --query "import gltf" --limit 20
meshy-dcc load-skill SKILL_NAME --instance-id ID
```

- **Search ranking is weak**, and the right hit is often last. Read all `hits`, and prefer
  one whose `slug` or `target_tool_slug` is set. `requires_load_skill: true` means you
  must load its `skill_name` first.
- **What load-skill returns:** `tools` lists every tool in the skill with its name,
  `inputSchema` and `annotations`, and `compact_schema` summarizes the best match.
- **Building the call slug:** it is `<dcc_type>.<instance_short>.<tools[].name>`, for
  example `blender.1a2b3c4d.blender_interchange__import_file`. Use names from this
  result, not the slug a search showed before loading.
- **Empty schemas:** some tools publish only `{"type": "object"}`, and `describe` cannot
  fill in the arguments. Find them as the host reference explains; don't guess argument
  names.
- **Loaded tools:** `search --instance-id ID` without a query lists what is already
  loaded.

## 3. Call, then read back

```sh
meshy-dcc call SLUG --json '{"path": "/absolute/model.glb"}'
meshy-dcc call SLUG --json-file args.json
```

- **Where the outcome is:** `result.structuredContent` has `success`, `message`, `error`
  and `context` (for example `imported_object_names`). Also check `result.isError`. The
  top-level `success` only means the call was delivered.
- **Delivered is not changed:** after a write, read the scene back with a read-only tool
  and compare it with what you intended.
- **Unknown outcomes:** keep `request_id`. If a call times out, is interrupted or returns
  something malformed, the outcome is unknown. Read the scene back and do not repeat the
  write.
- **Long and async tools:** raise `--timeout-secs` for long sync tools. An async tool
  returns a job; `--wait` polls that same job until it finishes, so don't resubmit it.
- **Paths:** use absolute paths, and save only where the user asked.
- **Prefer typed tools.** Use script-execution tools (`execute_python`,
  `dcc_introspect__eval` and similar) only when the user asked for scripting.

## 4. Stopping a host

`stop-instance` works only for instances that advertise a safe-stop hook. The supported
hosts don't yet, so it refuses. Never kill a host process. If a host needs to close, ask
the user to save and close it.

## Meshy models

1. Get the model with the `meshy` CLI; its skill covers generation, task recovery and
   download. If a wait fails, recover the same task ID rather than submitting a new paid
   task.
2. Download a format the host imports well (GLB for Blender) into the user's project or a
   folder they named.
3. Import it with the host's import tool, using the absolute path.
4. Read back what arrived (object names, count, materials) and tell the user where it is.
   Save only when asked.

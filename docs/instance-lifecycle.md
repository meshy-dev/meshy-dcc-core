# Instance launch ownership

`meshy-dcc start-instance` launches or reuses one exact project-bound instance.
Its JSON result separates `ready`, `launched`, `reused` and `owned`, and includes
the operation ID, instance ID, host PID and project. Readiness is separate from
process liveness; installation success does not establish either.

For local inventory, including a loopback gateway, `meshy-dcc list` adds an
optional `launch` object to each instance row:

```json
{"operation_id":"<original-operation-uuid>","owned":true,"launched":true}
```

Core derives this projection from its original launch receipt and a live local
registry snapshot. The full instance ID, host PID, DCC type and bound project
must match, and the original receipt must record an owned launch. Missing,
unreadable, stale or mismatched evidence omits `launch`; absence does not mean
that a host cannot be used. Remote inventory cannot grant local ownership.
Clients can rebuild this view after restarting without copying Core's receipt
store or maintaining a second instance registry.

To request a safe stop, pass the returned operation ID to
`meshy-dcc stop-instance --operation-id <id>` and retain the exact instance ID.
Core revalidates ownership and invokes the adapter's advertised stop hook.
An accepted stop is not proof of process exit: read the inventory again.
Unsaved work may cause the adapter to refuse; save it in the host before
retrying. `--discard-unsaved` requires authorization to lose unsaved changes.

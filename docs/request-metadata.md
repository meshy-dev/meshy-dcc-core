# Request metadata and deadlines

This contract applies to Core MCP clients, the REST adapter surface and native
transports. The CLI and other clients retain their own entry points.

## Caller context

MCP clients can attach caller attribution in `params._meta.agent_context` on
`tools/call`; REST callers use `meta.agent_context`. Context includes optional
agent, actor, session, task and trace identifiers. The gateway normalizes it
at ingress, records the trust source of individual fields, and derives network
fields from the server's request context. Self-reported identifiers are
attribution, not authorization or proof of identity.

The gateway's `call` wrapper merges per-call `arguments.meta` with protocol
`params._meta`. Batch items use the same rule with each item's `meta`. Per-call
hints such as `lease_owner` may supplement protocol metadata. They cannot
replace or erase ingress caller context through `agent_context` or its legacy
aliases. The gateway checks instance leases before forwarding. Lease ownership
is not a tool argument and does not grant permission to a different instance.

Outbound metadata is bounded: the gateway forwards `credential_profile`,
`permission_hint`, `project_scope` and `search_id`, then inserts canonical
`agent_context` and the remaining timeout. Unknown fields, including inline
credential fields, are dropped. The REST adapter receives it in `meta`; MCP
sidecar and job endpoints receive it in `params._meta`, beside `arguments`.
Tool arguments, request identifiers and adapter-owned job identifiers retain
their existing shape.

## Remaining timeout

`meshy-dcc/timeout_ms` is an unsigned integer representing how many milliseconds
the caller still waits. It is a relative budget, not a wall-clock timestamp,
lease expiry or promise that the host can interrupt a running operation.
An absent or non-integer value does not add a caller budget; the gateway's
configured backend timeout still applies. Zero is expired and never means
unlimited time.

When both protocol and per-call metadata contain valid budgets, the smaller
one wins. A batch's outer budget covers the whole batch, while an item's budget
may further restrict that item. Metadata hints, malformed overrides and route
refreshes cannot reset or extend an existing budget. Elapsed gateway and
readiness-probe time is subtracted before dispatch. Each forwarding hop reserves
500 ms so a downstream endpoint can answer before its caller stops waiting.
A call whose remaining dispatch budget is exhausted is not sent.

The native sidecar bounds the wait for its RPC lock, rejects expired work before
dispatch, and narrows `_meta` again before calling the host. Its bundled
commandPort, Qt/TCP and WebSocket clients preserve `_meta` as an optional field
in their existing RPC envelope, outside `args`. Existing host bridges may ignore
that field; custom `HostRpcClient` implementations may use the compatible
fallback that omits metadata. Such bridges do not acquire host-side deadline
enforcement merely by receiving the new Core version.

If the native sidecar stops waiting after dispatch, it retains the in-flight RPC
and its lock until the correlated response is consumed. The response deadline
does not cancel the host operation, start a new job, replay a mutation or release
the channel for a later call to consume a stale response.

## Outcomes and recovery

- `not-applied` means the call was refused before host dispatch or the host
  confirms it never started. It is safe to retry with a fresh budget.
- `outcome-unknown` means the host may have started or may still be running.
  Read back the scene or query the existing job before deciding to repeat it.

The main-thread dispatcher preserves this distinction for queued versus started
work. A transport timeout after sending is also an unknown outcome. Budget
propagation alone does not guarantee host cancellation, rollback or a stopped
physical effect.

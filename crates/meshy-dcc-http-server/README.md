# meshy-dcc-http-server

Runtime support layer for the Meshy DCC HTTP server — no axum, tower,
PyO3, or top-level `meshy-dcc-http` dependency.

This crate is the reusable server-runtime layer of the Clean Architecture
split called for in issue [#852]. The dependency direction is strictly
inward:

```text
meshy-dcc-http  (axum server + Python bindings + compatibility facade)
        │
        ▼
meshy-dcc-http-server  (runtime support — this crate)
        │
        ▼
meshy-dcc-http-types  (wire/config/value types)
```

## What lives here

- fixed core-tool descriptor builders (`build_core_tools`),
- host/main-thread execution bridges (`DccExecutorHandle`, `DeferredExecutor`)
  with their transport-neutral `ExecutorError` contract,
- session state and connection-scoped `tools/list` cache,
- in-flight request cancellation and progress routing,
- job/workflow notifications,
- workspace-root resolution helpers.

The public `meshy-dcc-http` crate re-exports this surface from historical
module paths for source compatibility while the split continues.

## Stability

Semver follows the workspace version. Runtime support moves here from
`meshy-dcc-http` one self-contained subsystem at a time so each step can be
reviewed independently and the dependency direction stays easy to audit.

[#852]: https://github.com/meshy-dev/meshy-dcc-core/issues/852

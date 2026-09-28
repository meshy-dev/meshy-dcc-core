# meshy-dcc-http-types

Wire-level value types for the Meshy DCC HTTP server — no axum, tokio,
reqwest, or pyo3.

This crate is the innermost layer of the `meshy-dcc-http` Clean-Architecture
split called for in issue [#852]. The dependency direction is strictly
inward:

```text
meshy-dcc-http  (server + pyo3 bindings)
        │
        ▼
meshy-dcc-http-types  (types — this crate)
```

Consumers in `meshy-dcc-http` re-export the types under their historical
paths so existing call sites keep compiling while the migration is in
flight.

Application UI policy intentionally lives in the Python `ui-control` skill,
while the standalone `dcc-cua` CLI owns automation and Host IPC. That keeps
the HTTP wire/config layer independent from any platform automation backend.

## Stability

Semver follows the workspace version. Types move here from
`meshy-dcc-http` one at a time so each move can be reviewed in isolation
and the dependency direction verified.

[#852]: https://github.com/meshy-dev/meshy-dcc-core/issues/852

# meshy-dcc-gateway-core

Domain layer for the Meshy DCC gateway — pure logic and types with no direct HTTP or
async-runtime dependency, and no dependency on `meshy-dcc-gateway`.

This crate is the innermost layer of the Clean-Architecture split called
for in issue [#845]. The dependency direction is strictly inward:

```text
meshy-dcc-gateway  (application + infrastructure)
        │
        ▼
meshy-dcc-gateway-core  (domain — this crate)
```

Consumers in `meshy-dcc-gateway` re-export the types under stable paths so
existing call sites keep compiling while the migration is in flight.
The lock-free `CapabilityIndexState` owns per-instance slices, unloaded
records, snapshots, fingerprints, and bounded lifecycle tombstones. The
gateway wrapper supplies synchronization, cache generations, clocks, and
async refresh gates without leaking those runtime concerns inward.
The deterministic capability builder translates transport-neutral
`meshy-dcc-jsonrpc` `McpTool` values into indexed domain records; backend HTTP
fetching and lifecycle orchestration remain in `meshy-dcc-gateway`.
Refresh assembly combines those live records with instance-scoped unloaded
skill hints, then sorts and fingerprints the complete slice. The gateway
runtime only performs I/O, observability, synchronization, and the final upsert.

## Stability

Semver follows the workspace version. Types move here from
`meshy-dcc-gateway` one at a time so each move can be reviewed in isolation
and the dependency direction verified.

[#845]: https://github.com/meshy-dev/meshy-dcc-core/issues/845

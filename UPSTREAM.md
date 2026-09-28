# Upstream source

meshy-dcc-core is an independent Meshy product built from the source of
[dcc-mcp-core](https://github.com/dcc-mcp/dcc-mcp-core) (MIT). This repository
does not carry upstream git history.

| Item | Value |
|---|---|
| Imported release | `v0.20.36` |
| Upstream commit | `b6154464382d072fc6c6fd24f17c80a454ff3c86` |
| License | MIT, upstream `LICENSE` retained unchanged |

Crate versions stay at the imported release as the internal wire/compat
version, because gateway election and adapter minimum-core checks compare it
with hosts that embed the same runtime. The Meshy product version is separate
(`PRODUCT_VERSION` in `crates/dcc-mcp-updater/src/meshy_release.rs`).

## Taking an upstream fix

Upstream changes arrive as patches, not merges. With a separate local clone of
dcc-mcp-core, diff the two upstream tags and apply the result with three-way
fallback, then review and test before committing:

```sh
git -C <upstream-clone> diff v0.20.36 <newer-tag> -- <paths> > upstream.patch
git apply -3 upstream.patch
```

Record the new upstream base here when a whole release is taken. Re-check the
license and notices before redistributing any upstream adapter or dependency.

# systemd — meshy-dcc-gateway

A hardened systemd unit for the standalone `meshy-dcc-server` binary.
See [`docs/guide/production-deployment.md`](../../docs/guide/production-deployment.md)
§3 for the full operational context.

## Install

```bash
# 1. Install the binary (built via `cargo build --release --bin meshy-dcc-server`)
sudo install -m0755 target/release/meshy-dcc-server /usr/local/bin/meshy-dcc-server

# 2. Drop in the unit
sudo install -m0644 examples/systemd/meshy-dcc-gateway.service \
  /etc/systemd/system/meshy-dcc-gateway.service

# 3. Enable & start
sudo systemctl daemon-reload
sudo systemctl enable --now meshy-dcc-gateway.service
```

## Override per host

```bash
sudo systemctl edit meshy-dcc-gateway.service
```

```ini
[Service]
# Put registry on shared storage for HA deployments
Environment=MESHY_DCC_REGISTRY_DIR=/mnt/shared/meshy-dcc/registry
# Bind publicly (only when behind a TLS-terminating proxy)
ExecStart=
ExecStart=/usr/local/bin/meshy-dcc-server --host 0.0.0.0 --app generic
```

Reload afterwards:

```bash
sudo systemctl daemon-reload
sudo systemctl restart meshy-dcc-gateway.service
```

## Smoke test

```bash
systemctl status meshy-dcc-gateway.service
curl -sf http://127.0.0.1:19765/health
# → {"ok":true}
curl -s http://127.0.0.1:19765/instances | jq
journalctl -u meshy-dcc-gateway.service -n 50 --no-pager
```

## Hardening summary

The unit applies `DynamicUser`, `ProtectSystem=strict`, `NoNewPrivileges`,
full `CapabilityBoundingSet=` (empty), syscall filtering, and read-only
root — the service cannot write outside `/var/lib/meshy-dcc` and
`/var/log/meshy-dcc`.

If you need to relax a restriction (e.g. to mount shared storage from an
NFS helper), copy the setting into your `systemctl edit` override rather
than modifying the shipped unit.

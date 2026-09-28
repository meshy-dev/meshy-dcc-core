# syntax=docker/dockerfile:1.23
#
# Multi-stage image for `meshy-dcc-server`.
#
# Build context: the meshy-dcc-core repo root (i.e. run `docker build` with
# `-f examples/compose/gateway-ha/Dockerfile .` from the repo root).

# ─── Stage 1: build ──────────────────────────────────────────────────────────
FROM rust:1.95-slim AS builder

RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --bin meshy-dcc-server \
 && cp /src/target/release/meshy-dcc-server /usr/local/bin/meshy-dcc-server

# ─── Stage 2: runtime ────────────────────────────────────────────────────────
FROM debian:13-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /var/lib/meshy-dcc dcc \
 && mkdir -p /var/lib/meshy-dcc /var/log/meshy-dcc \
 && chown -R dcc:dcc /var/lib/meshy-dcc /var/log/meshy-dcc

COPY --from=builder /usr/local/bin/meshy-dcc-server /usr/local/bin/meshy-dcc-server

USER dcc
ENV MESHY_DCC_REGISTRY_DIR=/var/lib/meshy-dcc/registry \
    MESHY_DCC_LOG_DIR=/var/log/meshy-dcc \
    MESHY_DCC_GATEWAY_PORT=19765

EXPOSE 19765

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -sf http://127.0.0.1:19765/health || exit 1

ENTRYPOINT ["/usr/local/bin/meshy-dcc-server"]
CMD ["--host", "0.0.0.0", "--dcc", "generic"]

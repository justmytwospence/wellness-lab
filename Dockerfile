# syntax=docker/dockerfile:1.7
# Wellness Lab server image: the Rust server and the browser engine (wasm), the built app, a slim runtime.
# Build: docker build -t wellness-lab:dev .   Run: see compose.yaml and docs/self-hosting.md.

# 1. The browser engine (wasm) and the app. Both are the same on every platform, so they are built once on
#    the build machine's own platform, never under emulation.
FROM --platform=$BUILDPLATFORM rust:1.99.1-bookworm AS wasm
RUN rustup target add wasm32-unknown-unknown
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=wl-cargo-registry \
    cargo install wasm-pack --locked --version 0.13.1
WORKDIR /w
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=wl-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,id=wl-cargo-git \
    --mount=type=cache,target=/w/target,id=wl-target-wasm \
    scripts/build-wasm.sh

FROM --platform=$BUILDPLATFORM node:24-bookworm AS web
WORKDIR /w
COPY . .
COPY --from=wasm /w/packages/engine/pkg packages/engine/pkg
RUN --mount=type=cache,target=/root/.npm,id=wl-npm \
    npm ci && npm run build

# 2. The server binary, for the target platform. The toolchain version matches rust-toolchain.toml.
FROM rust:1.99.1-bookworm AS server
WORKDIR /w
COPY . .
# The target directory is a cache mount, so the binary is copied out in the same step.
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=wl-cargo-registry \
    --mount=type=cache,target=/usr/local/cargo/git,id=wl-cargo-git \
    --mount=type=cache,target=/w/target,id=wl-target-server \
    cargo build --release --locked -p wellness-lab-server \
    && mkdir -p /out && cp target/release/wellness-lab-server /out/

# 3. The runtime.
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /data && chown 10001:10001 /data
COPY --from=server /out/wellness-lab-server /usr/local/bin/wellness-lab-server
COPY --from=web /w/app/dist /app/dist
USER 10001
ENV LAB_DATA_DIR=/data LAB_STATIC_DIR=/app/dist LAB_PORT=8080
VOLUME /data
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 CMD ["wellness-lab-server","healthcheck"]
ENTRYPOINT ["wellness-lab-server"]
CMD ["serve"]

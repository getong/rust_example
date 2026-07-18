# syntax=docker/dockerfile:1
# Build context is the crate root (openraft_libp2p_cluster/). The crate has no
# path dependencies on the surrounding cargo workspace, so it builds standalone
# inside the container.
#
#   ./script/build-docker.sh            # -> openraft-libp2p-cluster:latest
#
# Container network mode: nodes listen on plain-websocket multiaddrs
# (/ip4/0.0.0.0/tcp/4001/ws) and advertise /dns4/<container-or-pod-dns>/...,
# so no wss certificates are needed (wss + --ws-tls-key/--ws-tls-cert still
# work if you pass them via EXTRA_ARGS).

FROM rust:1-trixie AS builder

# Use the 163 mirror for apt (trixie ships deb822 debian.sources; keep a
# fallback for classic sources.list layouts).
RUN set -eux; \
    for f in /etc/apt/sources.list.d/debian.sources /etc/apt/sources.list; do \
        [ -f "$f" ] || continue; \
        sed -i \
            -e 's|http://deb.debian.org|https://mirrors.163.com|g' \
            -e 's|https://deb.debian.org|https://mirrors.163.com|g' \
            -e 's|http://security.debian.org/debian-security|https://mirrors.163.com/debian-security|g' \
            -e 's|http://security.debian.org|https://mirrors.163.com/debian-security|g' \
            "$f"; \
    done

# clang/libclang: rocksdb bindgen; protobuf-compiler: prost-build (build.rs);
# cmake: transitive C deps.
RUN apt-get update && apt-get install -y --no-install-recommends \
        clang libclang-dev cmake pkg-config protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY . .

# Cap cargo's build parallelism. The Docker Desktop VM typically has far less
# memory than the host (8GB default); an unbounded `cargo build` spawns one
# rustc per vCPU (16 here) and the heavy crates (wasmtime/cranelift, rocksdb,
# libp2p) then blow past the VM's RAM, which makes the whole build crawl in
# swap. 4 jobs stays comfortably inside 8GB; raise it via
# CARGO_JOBS=8 ./script/build-docker.sh once the VM has more memory.
ARG CARGO_JOBS=4

# Cache mounts keep the cargo registry/git and target dir across image builds.
# Binaries are copied out of the cache mount within the same RUN.
#
# The /host-cargo binds are named build contexts pointing into the host's
# ~/.cargo (wired up by build-docker.sh). On first build they seed the cache
# mounts, so crates.io packages and the rust-libp2p GitHub clone are copied
# from the host instead of re-downloaded (GitHub access is the slow part).
# Seeding runs once per cache mount; delete the .host-seeded marker files (or
# `docker builder prune`) to re-seed after the host cache gains new deps.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    --mount=type=bind,from=cargo-registry-index,target=/host-cargo/registry/index \
    --mount=type=bind,from=cargo-registry-cache,target=/host-cargo/registry/cache \
    --mount=type=bind,from=cargo-git-db,target=/host-cargo/git/db \
    --mount=type=bind,from=cargo-lock,target=/host-cargo/lock \
    set -eux; \
    # With the workspace Cargo.lock (and the seeded caches) cargo resolves
    # fully offline; without it every build re-resolves against the crates.io
    # index over the network, which stalls/fails behind slow egress.
    if [ -f /host-cargo/lock/Cargo.lock ] && [ ! -f Cargo.lock ]; then \
        cp /host-cargo/lock/Cargo.lock Cargo.lock; \
    fi; \
    if [ ! -f /usr/local/cargo/registry/.host-seeded ]; then \
        for d in registry/index registry/cache; do \
            mkdir -p "/usr/local/cargo/$d"; \
            cp -Rn "/host-cargo/$d/." "/usr/local/cargo/$d/" || true; \
        done; \
        touch /usr/local/cargo/registry/.host-seeded; \
    fi; \
    if [ ! -f /usr/local/cargo/git/.host-seeded ]; then \
        mkdir -p /usr/local/cargo/git/db; \
        cp -Rn /host-cargo/git/db/. /usr/local/cargo/git/db/ || true; \
        touch /usr/local/cargo/git/.host-seeded; \
    fi; \
    cargo build --release --bins -j "${CARGO_JOBS}" \
    && mkdir -p /out \
    && cp target/release/openraft_libp2p_cluster \
          target/release/olpc-peer-id \
          target/release/olpc-kv \
          target/release/olpc-status \
          target/release/olpc-task \
          /out/

FROM debian:trixie-slim

# Keep the runtime stage's mirror dependence minimal: the CA bundle is copied
# from the builder (same trixie release) instead of apt-installing
# ca-certificates, and the healthcheck uses bash's built-in /dev/tcp instead
# of curl. apt is used for ONE package: graphviz — the /graph endpoint shells
# out to `dot` to render the cluster graph SVG. 163 mirror over plain http
# (no CA bundle exists at this point in the stage); once this layer succeeds
# it stays cached.
RUN set -eux; \
    for f in /etc/apt/sources.list.d/debian.sources /etc/apt/sources.list; do \
        [ -f "$f" ] || continue; \
        sed -i \
            -e 's|http://deb.debian.org|http://mirrors.163.com|g' \
            -e 's|https://deb.debian.org|http://mirrors.163.com|g' \
            -e 's|http://security.debian.org/debian-security|http://mirrors.163.com/debian-security|g' \
            -e 's|http://security.debian.org|http://mirrors.163.com/debian-security|g' \
            "$f"; \
    done; \
    apt-get update -o Acquire::Retries=3; \
    apt-get install -y --no-install-recommends -o Acquire::Retries=3 graphviz; \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /etc/ssl/certs /etc/ssl/certs
COPY --from=builder /usr/share/ca-certificates /usr/share/ca-certificates

COPY --from=builder /out/openraft_libp2p_cluster /out/olpc-peer-id /out/olpc-kv /out/olpc-status /out/olpc-task /usr/local/bin/
COPY docker/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

ENV DB_DIR=/data \
    P2P_PORT=4001 \
    HTTP_PORT=3000
VOLUME /data
EXPOSE 3000 4001

# Generous start period: 100 nodes joining at once converge slowly.
# Probe via bash /dev/tcp: the image ships no curl/wget (see the no-apt note).
HEALTHCHECK --interval=15s --timeout=5s --start-period=180s --retries=5 \
    CMD bash -c 'exec 3<>"/dev/tcp/127.0.0.1/${HTTP_PORT}" && printf "GET /cluster HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n" >&3 && head -n1 <&3 | grep -q " 200 "' || exit 1

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]

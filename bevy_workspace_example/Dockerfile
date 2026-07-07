FROM rust:1-trixie AS builder

RUN apt-get update \
  && apt-get install -y --no-install-recommends libprotobuf-dev protobuf-compiler \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
COPY . .

RUN cargo build --release -p game_server

FROM debian:trixie-slim AS runtime

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && useradd --system --uid 10001 --home-dir /nonexistent --shell /usr/sbin/nologin game-server

COPY --from=builder /workspace/target/release/game_server /usr/local/bin/game_server

USER 10001:10001
ENV GAME_SERVER_BIND=0.0.0.0:6000
EXPOSE 6000/udp

ENTRYPOINT ["/usr/local/bin/game_server"]

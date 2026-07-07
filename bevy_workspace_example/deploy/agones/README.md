# Agones Local Deployment

This setup runs `game_server` as an Agones Fleet and runs `game_client` from the host with Cargo.

## Quick Start

```bash
./scripts/dev-agones-up.sh
./scripts/run-game-client-local.sh
```

`dev-agones-up.sh` builds a local Docker image, updates the Fleet manifest image tag, deploys the Agones Fleet and local UDP Service, then starts a local UDP proxy.

## Step By Step

Install or verify Agones:

```bash
./scripts/ensure-agones.sh
kubectl get pods -n agones-system
```

Build the local image and update the Fleet manifest:

```bash
./scripts/build-game-server-image.sh
```

Use a fixed image tag when needed:

```bash
GAME_SERVER_IMAGE=game-server:dev-manual ./scripts/build-game-server-image.sh
```

Deploy the Fleet and local NodePort Service:

```bash
./scripts/deploy-game-server-agones.sh
kubectl get fleet,gameserver,pods
kubectl get svc/bevy-game-server-local endpoints/bevy-game-server-local
```

Start the UDP proxy for Docker Desktop/kind local networking:

```bash
./scripts/start-game-udp-proxy.sh
```

Run a headless connection probe:

```bash
GAME_SERVER_ADDR=127.0.0.1:30600 cargo run -p game_client --bin net_probe
```

Run the game client:

```bash
GAME_SERVER_ADDR=127.0.0.1:30600 cargo run -p game_client
```

Or use the wrapper:

```bash
./scripts/run-game-client-local.sh
```

## Why Use The UDP Proxy

Agones reports GameServer addresses like `172.21.0.4:7455`. In this Docker Desktop/kind environment those are internal Docker network addresses, and UDP `hostPort` or `NodePort` is not reliably reachable directly from macOS.

The local proxy publishes:

```text
127.0.0.1:30600/udp -> Kubernetes node NodePort 30600/udp -> game_server Pod 6000/udp
```

So the host client should use `127.0.0.1:30600`.

## Useful Environment Variables

```bash
AGONES_VERSION=1.59.0
GAME_SERVER_IMAGE=game-server:dev-custom
GAME_SERVER_WAIT_SECONDS=180
GAME_UDP_PROXY_PORT=30600
GAME_UDP_PROXY_TARGET=172.21.0.2:30600
START_GAME_UDP_PROXY=true
RUN_GAME_CLIENT=false
GAME_CLIENT_PROBE=true
GAME_SERVER_ADDR=127.0.0.1:30600
```

## Cleanup

```bash
./scripts/stop-game-udp-proxy.sh
kubectl delete -f deploy/agones
```

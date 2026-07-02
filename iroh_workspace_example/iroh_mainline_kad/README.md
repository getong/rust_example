# iroh_mainline_kad

This example combines [`iroh`](https://crates.io/crates/iroh) with
[`mainline`](https://crates.io/crates/mainline) to build a small Kademlia-style discovery layer for
iroh endpoints.

It is meant to feel similar to a libp2p Kademlia server/client flow:

- servers join the DHT and publish their dialable iroh endpoint address
- clients query the DHT for cluster members
- clients dial a discovered member over iroh QUIC with a fixed ALPN
- the server replies on a bidirectional iroh stream

## Design

Mainline DHT and iroh solve different parts of the problem:

- `mainline` provides the Kademlia DHT. This project uses its BEP44 mutable value support as the
  cluster discovery record.
- `iroh` provides authenticated endpoint IDs, NAT traversal, relay support, and QUIC streams.

The DHT record is a JSON value stored as a BEP44 mutable item. It is addressed by:

- `cluster_secret`: a 32-byte Ed25519 signing key, passed as 64 hex characters
- `cluster_salt`: a namespace string, defaulting to `iroh-mainline-kad/v0`

Every server updates the same mutable record with its current iroh endpoint data:

- endpoint id
- node name
- direct socket addresses
- relay URLs
- update timestamp

The record is kept under Mainline's BEP44 value limit of 1000 bytes. If needed, older members are
trimmed. Writes use CAS and retry a few times to reduce conflicts when multiple servers publish
around the same time.

The iroh application protocol uses this ALPN:

```text
iroh-mainline-kad/cluster/0
```

## Commands

Run everything locally in one process:

```bash
cargo run -p iroh_mainline_kad -- local-demo
```

With explicit local demo sizing:

```bash
cargo run -p iroh_mainline_kad -- local-demo \
  --dht-nodes 5 \
  --servers 2 \
  --message "hello from local demo" \
  --discover-timeout-secs 10
```

Run a server using the public Mainline DHT bootstrap nodes:

```bash
cargo run -p iroh_mainline_kad -- server \
  --name node-a \
  --cluster-secret 0000000000000000000000000000000000000000000000000000000000000001
```

Run a client for the same cluster:

```bash
cargo run -p iroh_mainline_kad -- client \
  --cluster-secret 0000000000000000000000000000000000000000000000000000000000000001 \
  --message "ping from client"
```

Use a private or local Mainline bootstrap list:

```bash
cargo run -p iroh_mainline_kad -- server \
  --name node-a \
  --cluster-secret 0000000000000000000000000000000000000000000000000000000000000001 \
  --bootstrap 127.0.0.1:6881,127.0.0.1:6882
```

```bash
cargo run -p iroh_mainline_kad -- client \
  --cluster-secret 0000000000000000000000000000000000000000000000000000000000000001 \
  --bootstrap 127.0.0.1:6881,127.0.0.1:6882
```

Disable iroh relay usage for same-host or LAN testing:

```bash
cargo run -p iroh_mainline_kad -- server \
  --name node-a \
  --relay false \
  --iroh-bind 127.0.0.1:0
```

```bash
cargo run -p iroh_mainline_kad -- client \
  --relay false \
  --iroh-bind 127.0.0.1:0
```

## Example: One KAD Bootstrap Network, One Server, Two Clients

This scenario uses four terminals on one machine.

Important: a single Mainline node is not enough to store a BEP44 mutable record. Mainline PUT needs
closest DHT nodes with valid write tokens. The `kad-server` command below starts a small local
Mainline KAD network and prints a bootstrap list. The iroh server publishes its endpoint into that
network, and the two clients discover it through the same bootstrap list.

Use the same cluster secret in all terminals:

```bash
export CLUSTER_SECRET=0000000000000000000000000000000000000000000000000000000000000001
```

Terminal 1: start the local Mainline KAD bootstrap network.

```bash
cargo run -p iroh_mainline_kad -- kad-server \
  --nodes 5 \
  --bind 127.0.0.1
```

This prints a line like:

```text
bootstrap: 127.0.0.1:6881,127.0.0.1:53622,127.0.0.1:54873,127.0.0.1:61210,127.0.0.1:62001
```

Copy the full comma-separated value into the next terminals:

```bash
export KAD_BOOTSTRAP=127.0.0.1:6881,127.0.0.1:53622,127.0.0.1:54873,127.0.0.1:61210,127.0.0.1:62001
```

Terminal 2: start the iroh server that publishes itself into the KAD network.

```bash
cargo run -p iroh_mainline_kad -- server \
  --name kad-server-1 \
  --cluster-secret "$CLUSTER_SECRET" \
  --bootstrap "$KAD_BOOTSTRAP" \
  --dht-bind 127.0.0.1 \
  --iroh-bind 127.0.0.1:0 \
  --relay false \
  --wait-online-secs 0
```

Terminal 3: start the first KAD client.

```bash
cargo run -p iroh_mainline_kad -- client \
  --cluster-secret "$CLUSTER_SECRET" \
  --bootstrap "$KAD_BOOTSTRAP" \
  --dht-bind 127.0.0.1 \
  --iroh-bind 127.0.0.1:0 \
  --relay false \
  --wait-online-secs 0 \
  --message "hello from kad client 1"
```

Terminal 4: start the second KAD client.

```bash
cargo run -p iroh_mainline_kad -- client \
  --cluster-secret "$CLUSTER_SECRET" \
  --bootstrap "$KAD_BOOTSTRAP" \
  --dht-bind 127.0.0.1 \
  --iroh-bind 127.0.0.1:0 \
  --relay false \
  --wait-online-secs 0 \
  --message "hello from kad client 2"
```

Expected behavior:

- the server prints its iroh endpoint id and publishes a Mainline mutable record
- each client discovers one member from the KAD record
- each client dials the discovered iroh endpoint and prints a response
- the server prints the messages received from both clients

For multi-host testing, keep `--relay true` or omit the `--relay` option, bind to reachable
interfaces, and use a bootstrap address reachable by the client machines.

## Useful Options

Server options:

- `--name`: name stored in the cluster record
- `--cluster-secret`: 64-hex Ed25519 seed for the shared cluster record
- `--cluster-salt`: optional DHT namespace
- `--bootstrap`: comma-separated Mainline bootstrap nodes
- `--dht-bind`: IPv4 bind address for the Mainline node
- `--dht-port`: Mainline UDP port; `0` means random
- `--iroh-bind`: iroh socket bind address
- `--relay`: enable or disable iroh relay support
- `--republish-secs`: interval for refreshing this server's DHT member record

Client options:

- `--cluster-secret`: must match the server cluster secret
- `--cluster-salt`: must match the server salt if set
- `--bootstrap`: comma-separated Mainline bootstrap nodes
- `--message`: payload sent over the iroh stream
- `--discover-timeout-secs`: how long to poll the DHT for members
- `--connect-timeout-secs`: per-member iroh dial/request timeout

## Notes

The default cluster secret is fixed for easy local demos. Use `--cluster-secret` for real runs so
unrelated processes do not publish into the same record.

Public Mainline DHT publication depends on UDP reachability and DHT convergence. For deterministic
testing, use `local-demo` or pass your own bootstrap nodes.

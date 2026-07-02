# iroh_mainline_kad

This example combines [`iroh`](https://crates.io/crates/iroh) with
[`mainline`](https://crates.io/crates/mainline) to build a small Kademlia-style discovery layer for
iroh endpoints.

It is meant to feel similar to a libp2p Kademlia server/client flow:

- servers join the DHT and publish their dialable iroh endpoint address
- clients query the DHT for cluster members
- clients dial a discovered member over iroh QUIC with a fixed ALPN
- the server replies on a bidirectional iroh stream
- gossip peers query the same DHT record, join an `iroh-gossip` topic, and broadcast pubsub-style
  messages to all peers on that topic
- blob seed peers import files into `iroh-blobs`, publish provider metadata into the DHT, and
  download peers discover multiple providers for the same hash

## Design

Mainline DHT and iroh solve different parts of the problem:

- `mainline` provides the Kademlia DHT. This project uses its BEP44 mutable value support as the
  cluster discovery record.
- `iroh` provides authenticated endpoint IDs, NAT traversal, relay support, and QUIC streams.
- `iroh-gossip` provides the topic membership and broadcast tree, similar in effect to libp2p
  gossipsub.
- `iroh-blobs` provides BLAKE3 verified blob transfer. Its downloader can split blob requests into
  ranges and fetch them from multiple providers concurrently.

The DHT record is stored as a BEP44 mutable item. New records use a compact `postcard` binary
payload with a small format marker; older JSON records are still accepted while nodes migrate. It
is addressed by:

- `cluster_secret`: a 32-byte Ed25519 signing key, passed as 64 hex characters
- `cluster_salt`: a namespace string, defaulting to `iroh-mainline-kad/v0`

Every server updates the same mutable record with its current iroh endpoint data:

- endpoint id
- node name
- supported protocol labels, currently `request`, `gossip`, or `blob`
- provided blob metadata for blob seed nodes
- direct socket addresses
- relay URLs
- update timestamp
- record nonce used with the timestamp to reject stale record regressions

The record is kept under Mainline's BEP44 value limit of 1000 bytes. If needed, older members are
trimmed with a bounded-size search instead of repeated one-by-one JSON serialization. Writes use
CAS and retry a few times to reduce conflicts when multiple servers publish around the same time.

The iroh application protocol uses this ALPN:

```text
iroh-mainline-kad/cluster/0
```

The gossip command uses the standard `iroh-gossip` ALPN and a 32-byte `--topic` hex value. The
default topic is the same as `TopicId::from_bytes([23u8; 32])`.

The blob commands use the standard `iroh-blobs` ALPN. A `blob-seed` node imports a local file into
an `FsStore`, pins it with a named tag, and publishes the BLAKE3 hash into Mainline. A `blob-get`
node finds providers for that hash, injects their `EndpointAddr` values into an iroh memory address
lookup, then uses `iroh-blobs` downloader with `SplitStrategy::Split` to fetch verified ranges from
multiple sources and export the restored file.

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
export KAD_BOOTSTRAP=127.0.0.1:6881,127.0.0.1:53622,127.0.0.1:54873,127.0.0.1:61210,127.0.0.1:62001
```

Copy or eval the full export line in the next terminals:

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

## Example: Gossipsub-style Gossip

This scenario uses one local Mainline KAD bootstrap network and three iroh-gossip peers. The first
peer stays online as a seed/listener; the second and third peers discover it through Mainline and
broadcast messages to the shared topic.

Terminal 1: start the local Mainline KAD bootstrap network and export the printed bootstrap list.

```bash
cargo run -p iroh_mainline_kad -- kad-server \
  --nodes 5 \
  --bind 127.0.0.1
```

```bash
export CLUSTER_SECRET=0000000000000000000000000000000000000000000000000000000000000001
export KAD_BOOTSTRAP=127.0.0.1:6881,127.0.0.1:53622,127.0.0.1:54873,127.0.0.1:61210,127.0.0.1:62001
```

Terminal 2: start the first gossip peer.

```bash
cargo run -p iroh_mainline_kad -- gossip \
  --name gossip-peer-1 \
  --cluster-secret "$CLUSTER_SECRET" \
  --bootstrap "$KAD_BOOTSTRAP" \
  --dht-bind 127.0.0.1 \
  --iroh-bind 127.0.0.1:0 \
  --relay false \
  --wait-online-secs 0
```

Terminal 3: start the second peer and broadcast a message.

```bash
cargo run -p iroh_mainline_kad -- gossip \
  --name gossip-peer-2 \
  --cluster-secret "$CLUSTER_SECRET" \
  --bootstrap "$KAD_BOOTSTRAP" \
  --dht-bind 127.0.0.1 \
  --iroh-bind 127.0.0.1:0 \
  --relay false \
  --wait-online-secs 0 \
  --message "hello from gossip peer 2"
```

Terminal 4: start the third peer and broadcast another message.

```bash
cargo run -p iroh_mainline_kad -- gossip \
  --name gossip-peer-3 \
  --cluster-secret "$CLUSTER_SECRET" \
  --bootstrap "$KAD_BOOTSTRAP" \
  --dht-bind 127.0.0.1 \
  --iroh-bind 127.0.0.1:0 \
  --relay false \
  --wait-online-secs 0 \
  --message "hello from gossip peer 3"
```

Expected behavior:

- each gossip peer publishes its endpoint address into the Mainline record
- later peers print discovered gossip bootstrap peers and join the shared topic
- peers print `gossip neighbor up` when the topic connects
- peers print `gossip received from ...` when messages arrive

## Example: BitTorrent-style Blob Transfer

This scenario uses one local Mainline KAD bootstrap network, two blob seed peers, and one downloader.
Both seed peers import the same file. The downloader discovers both providers for the hash and lets
`iroh-blobs` split the transfer across available sources.

Create a sample file:

```bash
mkdir -p /tmp/iroh-blob-demo
dd if=/dev/urandom of=/tmp/iroh-blob-demo/input.bin bs=1024 count=256
```

Terminal 1: start the local Mainline KAD bootstrap network.

```bash
cargo run -p iroh_mainline_kad -- kad-server \
  --nodes 5 \
  --bind 127.0.0.1
```

Use the printed `export KAD_BOOTSTRAP=...` line and the same cluster secret in the seed/get
terminals:

```bash
export CLUSTER_SECRET=0000000000000000000000000000000000000000000000000000000000000001
export KAD_BOOTSTRAP=127.0.0.1:6881,127.0.0.1:53622,127.0.0.1:54873,127.0.0.1:61210,127.0.0.1:62001
```

Terminal 2: start the first blob seed.

```bash
cargo run -p iroh_mainline_kad -- blob-seed \
  --name blob-seed-1 \
  --cluster-secret "$CLUSTER_SECRET" \
  --bootstrap "$KAD_BOOTSTRAP" \
  --dht-bind 127.0.0.1 \
  --iroh-bind 127.0.0.1:0 \
  --relay false \
  --wait-online-secs 0 \
  --file /tmp/iroh-blob-demo/input.bin \
  --store-path /tmp/iroh-blob-demo/seed-1-store
```

Terminal 3: start the second blob seed with the same file.

```bash
cargo run -p iroh_mainline_kad -- blob-seed \
  --name blob-seed-2 \
  --cluster-secret "$CLUSTER_SECRET" \
  --bootstrap "$KAD_BOOTSTRAP" \
  --dht-bind 127.0.0.1 \
  --iroh-bind 127.0.0.1:0 \
  --relay false \
  --wait-online-secs 0 \
  --file /tmp/iroh-blob-demo/input.bin \
  --store-path /tmp/iroh-blob-demo/seed-2-store
```

Both seed commands print the same `blob hash`. Copy it into:

```bash
export BLOB_HASH=<printed-blob-hash>
```

Terminal 4: download and restore the file.

```bash
cargo run -p iroh_mainline_kad -- blob-get \
  --cluster-secret "$CLUSTER_SECRET" \
  --bootstrap "$KAD_BOOTSTRAP" \
  --dht-bind 127.0.0.1 \
  --iroh-bind 127.0.0.1:0 \
  --relay false \
  --wait-online-secs 0 \
  --hash "$BLOB_HASH" \
  --output /tmp/iroh-blob-demo/output.bin \
  --store-path /tmp/iroh-blob-demo/get-store
```

Verify the restored file:

```bash
cmp /tmp/iroh-blob-demo/input.bin /tmp/iroh-blob-demo/output.bin
```

Expected behavior:

- each seed prints its endpoint id, direct address, blob hash, and stores a Mainline provider record
- `blob-get` prints all discovered providers for the hash
- `iroh-blobs` validates chunks by BLAKE3 while downloading
- the final export recreates the original file at `--output`

## Useful Options

Server options:

- `--name`: name stored in the cluster record
- `--cluster-secret`: 64-hex Ed25519 seed for the shared cluster record
- `--cluster-salt`: optional DHT namespace
- `--bootstrap`: comma-separated Mainline bootstrap nodes
- `--dht-bind`: IPv4 bind address for the Mainline node
- `--dht-port`: Mainline UDP port; `0` means random
- `--iroh-bind`: iroh socket bind address
- `--iroh-secret-path`: optional hex secret-key file for a stable iroh endpoint id across restarts
- `--relay`: enable or disable iroh relay support
- `--republish-secs`: interval for refreshing this server's DHT member record

Client options:

- `--cluster-secret`: must match the server cluster secret
- `--cluster-salt`: must match the server salt if set
- `--bootstrap`: comma-separated Mainline bootstrap nodes
- `--message`: payload sent over the iroh stream
- `--iroh-secret-path`: optional hex secret-key file for a stable iroh endpoint id
- `--discover-timeout-secs`: how long to poll the DHT for members
- `--connect-timeout-secs`: per-member iroh dial/request timeout

Gossip options:

- `--name`: name stored in the cluster record
- `--cluster-secret`: shared DHT record key; use the same value for all peers
- `--cluster-salt`: optional DHT namespace
- `--bootstrap`: comma-separated Mainline bootstrap nodes, usually `$KAD_BOOTSTRAP`
- `--topic`: 64-hex topic id; peers only gossip with the same topic
- `--message`: optional message to broadcast after joining
- `--wait-joined-secs`: how long to wait for at least one gossip neighbor before broadcasting
- `--exit-after-broadcast`: send `--message` and then shut down instead of staying subscribed
- `--iroh-secret-path`: optional hex secret-key file for a stable iroh endpoint id across restarts
- `--relay`: enable or disable iroh relay support

Blob seed options:

- `--file`: local file to import and serve through `iroh-blobs`
- `--store-path`: filesystem store used by `iroh-blobs`; keep it stable if restarting a seed
- `--name`: provider name stored in the DHT record
- `--cluster-secret`: shared DHT record key; use the same value for seeds and downloaders
- `--bootstrap`: comma-separated Mainline bootstrap nodes
- `--iroh-secret-path`: optional hex secret-key file for a stable provider endpoint id
- `--republish-secs`: interval for refreshing this provider in the DHT record

Blob get options:

- `--hash`: BLAKE3 blob hash printed by `blob-seed`
- `--output`: restored file path
- `--store-path`: local `iroh-blobs` store used for downloaded verified chunks
- `--cluster-secret`: must match the seed cluster secret
- `--bootstrap`: comma-separated Mainline bootstrap nodes
- `--iroh-secret-path`: optional hex secret-key file for a stable iroh endpoint id
- `--discover-timeout-secs`: how long to poll the DHT for providers
- `--request-timeout-secs`: timeout while waiting for downloader progress

## Notes

The default cluster secret is fixed for easy local demos. Use `--cluster-secret` for real runs so
unrelated processes do not publish into the same record.

By default iroh endpoints use an ephemeral identity. Long-running servers, gossip peers, and blob
seeds should pass `--iroh-secret-path`; the file is read on restart or created with a new 32-byte
secret if missing, keeping the endpoint id stable.

Gossip subscribes to its topic immediately and discovers peers in the background. Discovered relay
addresses are accepted by default; private, loopback, link-local, and other non-public direct IP
addresses are ignored unless the local iroh bind address is loopback for local testing.

Public Mainline DHT publication depends on UDP reachability and DHT convergence. For deterministic
testing, use `local-demo` or pass your own bootstrap nodes.

For local gossip tests with `--relay false`, keep all peers on the same host or LAN and pass the
complete `KAD_BOOTSTRAP` list printed by `kad-server`.

For blob transfers, the current CLI treats `--hash` as a raw single-file blob. `iroh-blobs` also
supports hash sequences/collections; the DHT record stores the format so collection support can be
added without changing the discovery record shape.

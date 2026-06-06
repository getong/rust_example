# kameo_game_sync

This example starts one logical game node per process. Each process can run both
`MapActor` and the `PlayerActor`s assigned to that node.

Map state is authoritative in `MapActor`; `PlayerActor` keeps only a local mirror
and TCP/session state.

## Run One Node

From the workspace root:

```bash
cargo run -p kameo_game_sync -- --node-id node-a
```

By default the node stays alive and waits for `Ctrl-C`.

## Run Two Nodes

Open two terminals from the workspace root.

Terminal 1:

```bash
cargo run -p kameo_game_sync -- --node-id node-a --nodes node-a,node-b --seed 42
```

Terminal 2:

```bash
cargo run -p kameo_game_sync -- --node-id node-b --nodes node-a,node-b --seed 42
```

Both nodes use the same `--nodes` list and `--seed`, so player assignment is
deterministic across processes. With seed `42`, the demo assigns `player:1004`
to `node-a`, and the other players to `node-b`.

When nodes discover each other, the logs should include lines like:

```text
[node:node-a] connected to peer map "map:node-b" (4 player(s))
[node:node-b] connected to peer map "map:node-a" (1 player(s))
```

Stop each node with `Ctrl-C`.

## Run Once

Use `--run-once` when you only want to run the startup demo and exit:

```bash
cargo run -p kameo_game_sync -- --node-id node-a --nodes node-a,node-b --seed 42 --run-once
```

## CLI Options

```text
--node-id <NODE_ID>  Current logical node id, e.g. node-a
--nodes <NODES>      Comma-separated logical node ids participating in player assignment
--seed <SEED>        Deterministic RNG seed used to assign players to nodes
--run-once           Run the startup demo once and exit instead of keeping the node alive
```


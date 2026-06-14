# apalis + libp2p + RocksDB task scheduler

This is a small distributed task scheduling demo.

- `libp2p request-response` sends tasks from a scheduler node to worker nodes.
- `apalis` runs received tasks through a local worker backend.
- `RocksDB` stores task state in column families, following the same style as the referenced `raft_kv_rocksdb` example.

## Architecture

```text
scheduler
  accepts submitted tasks, or creates optional demo tasks
  stores Created/Assigned/Completed in ./data/scheduler
  sends run requests to registered workers over libp2p

worker
  registers with scheduler
  receives run requests over libp2p
  pushes WorkerJob into a local Apalis backend
  stores Received/Running/Completed in ./data/worker
  returns TaskResponse to scheduler
```

The RocksDB schema uses two column families:

- `tasks`: keyed by task id, value is a JSON `TaskRecord`.
- `meta`: node lifecycle events.

## Run

Terminal 1:

```sh
cargo run -- scheduler --listen /ip4/127.0.0.1/tcp/7000
```

Copy the printed bootnode value:

```text
<scheduler-peer-id>@/ip4/127.0.0.1/tcp/7000
```

Terminal 2:

```sh
cargo run -- worker \
  --name worker-a \
  --listen /ip4/127.0.0.1/tcp/0 \
  --scheduler '<scheduler-peer-id>@/ip4/127.0.0.1/tcp/7000'
```

The scheduler queues tasks until a worker connects, then dispatches queued tasks round-robin across connected workers.

## Add Tasks

Submit one task to a running scheduler:

```sh
cargo run -- submit \
  --scheduler '<scheduler-peer-id>@/ip4/127.0.0.1/tcp/7000' \
  --payload 'send-email user=42'
```

Use a stable id when you want to query the task later by a known name:

```sh
cargo run -- submit \
  --scheduler '<scheduler-peer-id>@/ip4/127.0.0.1/tcp/7000' \
  --task-id task-email-42 \
  --payload 'send-email user=42'
```

The command prints a scheduler acknowledgement. It means the task was accepted and queued or dispatched; the final execution result is stored in RocksDB and can be inspected with `show` or `list`.

You can also create demo tasks automatically from the scheduler:

```sh
cargo run -- scheduler \
  --listen /ip4/127.0.0.1/tcp/7000 \
  --tasks 10 \
  --interval-ms 1000
```

## Inspect

Use real task ids from the scheduler log. `task-...` below is only a placeholder.

```sh
cargo run -- show --db ./data/scheduler --task-id task-1781453167239-1
cargo run -- show --db ./data/worker --task-id task-1781453167239-1
```

List all stored task records:

```sh
cargo run -- list --db ./data/scheduler
cargo run -- list --db ./data/worker
```

Filter by status:

```sh
cargo run -- list --db ./data/scheduler --status completed
cargo run -- list --db ./data/worker --status running
```

List active task records:

```sh
cargo run -- list-running --db ./data/scheduler
cargo run -- list-running --db ./data/worker
```

`list-running` is intentionally narrow: scheduler records are active while `Assigned`; worker records are active while `Running`. This demo task sleeps for only about 750 ms, so `list-running` often returns `[]` after the task has already completed. Use `list --status completed` or plain `list` to see completed history.

`show`, `list`, and `list-running` open RocksDB in read-only mode, so they can inspect a database while the scheduler or worker process is still running.

## Command Reference

```sh
cargo run -- scheduler \
  --db ./data/scheduler \
  --listen /ip4/127.0.0.1/tcp/7000 \
  --tasks 0 \
  --interval-ms 1000
```

```sh
cargo run -- worker \
  --db ./data/worker \
  --name worker-a \
  --listen /ip4/127.0.0.1/tcp/0 \
  --scheduler '<scheduler-peer-id>@<scheduler-multiaddr>'
```

```sh
cargo run -- submit \
  --scheduler '<scheduler-peer-id>@<scheduler-multiaddr>' \
  --payload '<payload>'

cargo run -- submit \
  --scheduler '<scheduler-peer-id>@<scheduler-multiaddr>' \
  --task-id <task-id> \
  --payload '<payload>'
```

```sh
cargo run -- show --db ./data/scheduler --task-id <task-id>
cargo run -- list --db ./data/scheduler
cargo run -- list --db ./data/scheduler --status completed
cargo run -- list-running --db ./data/worker
```

## Notes

This demo models at-least-once delivery. Failed outbound requests are queued again, and task IDs are stable so a production version can add idempotency checks before execution.

On macOS, `rocksdb` may need `libclang` during the native build. This repo includes `.cargo/config.toml` pointing at the Command Line Tools `libclang.dylib`; adjust `LIBCLANG_PATH` there if your Xcode/LLVM installation is elsewhere.

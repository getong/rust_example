# RocksDB WAL Example

This example shows how to write data through RocksDB's write-ahead log (WAL),
sync the WAL file, inspect retained WAL batches, and recover after an abnormal
process crash.

## Run the normal demo

```sh
cargo run -p rocksdb_wal_example -- demo
```

The demo writes a `WriteBatch` with:

- `WriteOptions::disable_wal(false)` to keep WAL enabled.
- `WriteOptions::set_sync(true)` to fsync the WAL before the write returns.
- `DB::flush_wal(true)` to flush and sync the live WAL file.

The code deliberately does not call `db.flush()`, so the memtable is not forced
to an SST file. On reopen, RocksDB can replay the WAL file and recover the keys.

## Simulate a crash and recover

Start from a clean directory:

```sh
cargo run -p rocksdb_wal_example -- clean
```

Write a synced WAL batch and abort the process without dropping the DB:

```sh
cargo run -p rocksdb_wal_example -- crash
```

The command exits abnormally on purpose. Then reopen the same DB path:

```sh
cargo run -p rocksdb_wal_example -- recover
```

RocksDB automatically reads the WAL/log files under `rocksdb_wal_example_db`
while opening the database. The recovered values are printed by `recover`.

## Inspect retained WAL records

```sh
cargo run -p rocksdb_wal_example -- show-wal
```

This uses `DB::get_updates_since(0)` and `WriteBatch::iterate()` to print the
operations retained in WAL batches. The example sets `wal_ttl_seconds` and
`wal_size_limit_mb` so recent archived logs remain available long enough for
inspection.

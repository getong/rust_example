# Redis Cluster Distributed Lock Example

A Rust implementation of distributed locks using Redis Cluster, demonstrating lock acquisition, release, extension, and automatic cleanup with multiple concurrent clients.

## Features

- **Distributed Lock Implementation**: Thread-safe distributed locks across Redis cluster
- **Atomic Operations**: Uses Lua scripts for atomic lock operations
- **Lock Extension**: Ability to extend lock TTL while holding it
- **Auto-release Guard**: RAII pattern for automatic lock release
- **Retry Mechanism**: Configurable retry logic with exponential backoff
- **Lock Information**: Query lock status and ownership
- **Multiple Demonstrations**: Shows various usage patterns and scenarios

## Prerequisites

- Rust 1.70+
- Docker and Docker Compose
- Redis cluster (provided via docker-compose)

## Setup

1. Start the Redis cluster:
```bash
docker-compose up -d
```

Wait a few seconds for the cluster to initialize.

2. Build the project:
```bash
cargo build --release
```

3. Run the example:
```bash
cargo run
```

## Project Structure

```
cluster_distributed_lock_example/
├── src/
│   ├── main.rs                 # Example demonstrations
│   └── distributed_lock.rs     # Core lock implementation
├── redis-conf/                 # Redis node configurations
│   ├── redis-7000.conf
│   ├── redis-7001.conf
│   ├── redis-7002.conf
│   ├── redis-7003.conf
│   ├── redis-7004.conf
│   └── redis-7005.conf
├── docker-compose.yml          # Redis cluster setup
├── Cargo.toml                  # Project dependencies
└── README.md                   # This file
```

## Key Components

### RedisDistributedLock
Main lock implementation with methods:
- `acquire()`: Try to acquire lock once
- `acquire_with_retry()`: Acquire with configurable retries
- `release()`: Release the lock
- `extend()`: Extend lock TTL
- `is_locked()`: Check if resource is locked
- `get_lock_info()`: Get detailed lock information

### LockGuard
RAII wrapper for automatic lock release on drop.

### Demonstrations

1. **Lock Contention**: Multiple clients competing for the same lock
2. **Lock Guard**: Automatic release using RAII pattern
3. **Concurrent Workers**: Multiple workers processing tasks with shared locks

## Usage Example

```rust
use distributed_lock::RedisDistributedLock;
use std::time::Duration;

let lock = RedisDistributedLock::new(
    vec!["redis://127.0.0.1:7000".to_string()],
    "my_resource".to_string(),
    Duration::from_secs(10),
)?;

if lock.acquire().await? {
    // Critical section
    println!("Lock acquired!");

    // Extend if needed
    lock.extend(Duration::from_secs(5)).await?;

    // Work...

    lock.release().await?;
}
```

## Redis Cluster Configuration

The cluster consists of 6 Redis nodes (3 masters + 3 replicas):
- Ports: 7000-7005
- Replication factor: 1
- Node timeout: 5000ms
- Persistence: AOF enabled

## Stop the Cluster

```bash
docker-compose down
```

To completely remove volumes:
```bash
docker-compose down -v
```
# Chitchat + OpenRaft Distributed System

A distributed key-value store inspired by [Stract's](https://github.com/StractOrg/stract) architecture, combining **Chitchat** for service discovery with **OpenRaft** for consensus.

## Architecture Overview

This project implements a distributed system using the same architectural pattern as Stract:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   🔍 Chitchat    │    │   ⚡ OpenRaft    │    │   🌐 Poem API   │
│ Service Discovery│◄──►│   Consensus     │◄──►│   REST/OpenAPI  │
│                 │    │                 │    │                 │
│ • Gossip Protocol│    │ • Leader Election│    │ • Read/Write Ops│
│ • Node Discovery │    │ • Log Replication│    │ • Cluster Status│
│ • Health Checks  │    │ • Linearizable   │    │ • Swagger UI    │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────┐
                    │   💾 RocksDB    │
                    │    Storage      │
                    │                 │
                    │ • Persistent    │
                    │ • Raft Log      │
                    │ • State Machine │
                    └─────────────────┘
```

## Key Components

### 🔍 **Chitchat** (Service Discovery)
- **Purpose**: Gossip-based service discovery and cluster membership
- **Features**: 
  - Automatic node discovery
  - Failure detection
  - Service registration
- **Separation of Concerns**: Handles "who's in the cluster" separately from consensus

### ⚡ **OpenRaft** (Consensus)
- **Purpose**: Raft consensus for data consistency
- **Features**:
  - Leader election
  - Log replication  
  - Linearizable reads
- **Storage**: RocksDB backend with specialized raft log store

### 🌐 **Poem + OpenAPI** (Web Layer)
- **Purpose**: REST API with automatic documentation
- **Features**:
  - Swagger UI at `/`
  - API endpoints at `/api/*`
  - OpenAPI specification at `/spec`

## Quick Start

### 1. Build the Project
```bash
cargo build --release
```

### 2. Start a 3-Node Cluster
```bash
./test_distributed.sh
```

### 3. Test the System
```bash
# View cluster status
curl http://localhost:3001/api/cluster

# Write data (to any node)
curl http://localhost:3001/api/write -X POST \
  -H 'Content-Type: application/json' \
  -d '{"key":"hello","value":"world"}'

# Read data (from any node) 
curl http://localhost:3002/api/read/hello
```

## Manual Node Startup

Start individual nodes with different configurations:

```bash
# Node 1 (seed node)
./target/release/chitchat_poem_tarpc_rocksdb_example \
  --id 1 \
  --rpc-addr 127.0.0.1:21001 \
  --api-addr 127.0.0.1:3001 \
  --gossip-addr 127.0.0.1:20001

# Node 2 (joins via gossip)
./target/release/chitchat_poem_tarpc_rocksdb_example \
  --id 2 \
  --rpc-addr 127.0.0.1:21002 \
  --api-addr 127.0.0.1:3002 \
  --gossip-addr 127.0.0.1:20002 \
  --seed-gossip-addrs 127.0.0.1:20001
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/` | GET | Swagger UI |
| `/spec` | GET | OpenAPI specification |
| `/api/cluster` | GET | Cluster status and membership |
| `/api/write` | POST | Write key-value data |
| `/api/read/{key}` | GET | Read value for key |
| `/api/consistent_read/{key}` | GET | Linearizable read |

## Configuration Options

```bash
Options:
  --id <ID>                             Node ID (required)
  --rpc-addr <RPC_ADDR>                 RPC address for raft communication (required)
  --api-addr <API_ADDR>                 API server address [default: 127.0.0.1:8080]
  --gossip-addr <GOSSIP_ADDR>           Chitchat gossip address [default: 127.0.0.1:9000]
  --seed-gossip-addrs <SEED_GOSSIP_ADDRS>  Seed gossip addresses for joining cluster
  --node_id <NODE_ID>                   Chitchat node ID (optional, auto-generated)
```

## Why This Architecture?

This design follows **Stract's proven pattern** of separating concerns:

1. **🔍 Chitchat handles membership**: "Who's in the cluster?"
2. **⚡ OpenRaft handles consensus**: "What's the agreed state?"
3. **🌐 Clean API layer**: Standard REST endpoints with documentation

### Benefits:
- ✅ **Resilient**: Gossip protocol handles network partitions gracefully
- ✅ **Scalable**: Chitchat scales to hundreds of nodes
- ✅ **Consistent**: Raft ensures strong consistency for critical data
- ✅ **Observable**: Built-in API documentation and cluster introspection
- ✅ **Proven**: Architecture battle-tested in Stract search engine

## Dependencies

- **openraft**: Raft consensus implementation
- **chitchat**: Gossip-based cluster membership
- **poem**: Async web framework
- **poem-openapi**: OpenAPI/Swagger integration  
- **rocksdb**: Persistent storage
- **tokio**: Async runtime

## Inspired By

This project is inspired by [Stract](https://github.com/StractOrg/stract), an open-source search engine that uses this exact architectural pattern for building resilient distributed systems.

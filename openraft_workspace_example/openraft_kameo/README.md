# openraft_kameo — 架构说明

## 这个项目做什么？

本项目实现了一个**分布式键值存储（KV Store）**，核心思路是：

> 用 **OpenRaft** 保证多节点之间的数据一致性，用 **Kameo Actor** 管理本地状态，用 **Actix-Web** 对外暴露 HTTP API。

### 一句话概括

客户端通过 HTTP 提交 `key → value` 写入请求，OpenRaft 把这个操作作为日志条目复制到集群所有节点，只有日志被多数节点确认后，才会真正通过 Kameo Actor 应用到每个节点本地的内存 Map 里。

---

## 架构分层

```
┌───────────────────────────────────────────────────────┐
│                  HTTP 客户端                           │
│   POST /write  POST /read  POST /init  …              │
└────────────────────────┬──────────────────────────────┘
                         │  Actix-Web handlers
┌────────────────────────▼──────────────────────────────┐
│                   OpenRaft 层                          │
│   • Raft 共识协议（选主 / 日志复制 / 快照）            │
│   • 网络层：HTTP RPC (/vote /append /snapshot)         │
│   • 日志存储：mem-log（内存实现）                      │
└────────────────────────┬──────────────────────────────┘
                         │  apply() 回调
┌────────────────────────▼──────────────────────────────┐
│            RaftStateMachineStore                       │
│   • 实现 RaftStateMachine trait                        │
│   • 维护 last_applied_log / last_membership / snapshot │
│   • 把命令转发给 Kameo Actor                           │
└────────────────────────┬──────────────────────────────┘
                         │  ask(SetCommand)
┌────────────────────────▼──────────────────────────────┐
│                KvStoreActor（Kameo）                   │
│   • 持有真正的 BTreeMap<String, String>                │
│   • 处理 SetCommand / DumpState / InstallState 消息    │
└───────────────────────────────────────────────────────┘
```

---

## 关键组件说明

| 组件 | 类型 | 职责 |
|------|------|------|
| `TypeConfig` | openraft 类型配置 | 绑定命令类型 `SetCommand`、回复类型 `Option<String>` |
| `SetCommand` | Raft 日志条目 | 携带 `key`/`value`，可序列化，在节点间复制 |
| `KvStoreActor` | Kameo Actor | 线程安全地持有内存状态，通过 `ask()` 消息驱动 |
| `RaftStateMachineStore` | Raft 状态机 | 将 Raft 应用日志的回调桥接到 Kameo Actor |
| `LogStore` | Raft 日志存储 | 基于内存的日志存储（`mem-log` crate） |
| `network-v1-http` | Raft 网络层 | 用 HTTP 实现节点间 RPC（投票、追加日志、快照） |

---

## HTTP API

| 路径 | 方法 | 说明 |
|------|------|------|
| `/init` | POST | 初始化集群（传入节点列表，或单节点自初始化） |
| `/add-learner` | POST | 添加学习者节点 |
| `/change-membership` | POST | 变更集群成员 |
| `/write` | POST | 写入 KV（自动转发到 Leader） |
| `/write-local` | POST | 直接在本节点写入（由 `/write` 转发调用） |
| `/read` | POST | 读取 KV（最终一致，直接读本地状态） |
| `/linearizable-read` | POST | 线性一致读（通过 ReadIndex 协议保证） |
| `/leader` | GET | 查询当前 Leader 信息 |
| `/metrics` | GET | 查看 Raft 内部指标 |
| `/vote` | POST | Raft RPC：投票 |
| `/append` | POST | Raft RPC：追加日志 |
| `/snapshot` | POST | Raft RPC：安装快照 |

---

## 为什么必须用 OpenRaft？

### 问题根源：单节点不可靠

如果只有一个进程持有 `BTreeMap`，那么进程崩溃 → 数据全部丢失，无法做到高可用。

### 多节点直写的困境

如果让客户端同时写多个节点，在网络分区、节点崩溃等场景下，各节点的数据很快就会出现**不一致**（有的节点写入成功、有的没有），再也无法确定谁的数据是"权威"的。

### OpenRaft 解决的核心问题

OpenRaft 实现了 **Raft 共识算法**，提供以下保证：

1. **单一 Leader 写入**：任何时刻只有一个 Leader 接受客户端写请求，杜绝"脑裂"写冲突。
2. **多数派确认（Quorum）**：日志必须被 `N/2 + 1` 个节点写入磁盘（或内存），才算提交。即使部分节点宕机，数据也不会丢失。
3. **有序、幂等地应用日志**：`apply()` 保证每个节点按相同顺序执行相同的命令，最终状态一致。
4. **自动选主**：Leader 宕机后，剩余节点自动选出新 Leader，通常在秒级内完成，对外透明。
5. **快照与日志压缩**：支持将状态机快照传输给落后节点，避免无限增长的日志。
6. **线性一致读**：通过 `ReadIndex` 协议，可以保证读取到最新的已提交数据。

### 为什么不用其他方案？

| 方案 | 问题 |
|------|------|
| 单节点内存 Map | 无高可用，进程崩溃即丢数据 |
| 主从复制（自己实现） | 脑裂风险，主节点宕机时需人工介入 |
| 2PC（两阶段提交） | 阻塞协议，协调者宕机会导致整个系统挂起 |
| etcd/ZooKeeper（外部服务） | 引入额外运维依赖，本项目目标是内嵌式一致性 |
| **OpenRaft** ✓ | Rust 原生、类型安全、可插拔存储、异步友好，直接嵌入应用进程 |

---

## 数据流：一次完整的写操作

```
客户端
  │
  │  POST /write {"key":"color","value":"blue"}
  ▼
Actix-Web handler (write)
  │
  │  raft.client_write(SetCommand)
  ▼
OpenRaft Leader
  │  1. 追加日志到本地 LogStore
  │  2. 并发 HTTP 复制到其他节点 (/append)
  │  3. 等待多数节点确认
  │  4. 标记日志为已提交
  │
  │  apply() 回调
  ▼
RaftStateMachineStore
  │
  │  actor_ref.ask(SetCommand).send().await
  ▼
KvStoreActor
  │  self.state.insert("color", "blue")
  │  返回 old_value (None)
  ▼
响应链路逆序返回给客户端
```

---

## 快速启动（3 节点集群）

```bash
# 构建并启动 3 个节点
./scripts/start-3nodes.sh

# 查看节点状态
./scripts/status-nodes.sh

# 停止所有节点
./scripts/stop-nodes.sh
```

节点默认监听端口：`21001`、`21002`、`21003`。

### 手动操作示例

```bash
# 1. 初始化集群（在 node 1 上执行）
curl -X POST http://127.0.0.1:21001/init \
  -H 'Content-Type: application/json' \
  -d '[[1,"127.0.0.1:21001"],[2,"127.0.0.1:21002"],[3,"127.0.0.1:21003"]]'

# 2. 写入数据（自动转发到 Leader）
curl -X POST http://127.0.0.1:21001/write \
  -H 'Content-Type: application/json' \
  -d '{"key":"hello","value":"world"}'

# 3. 读取数据
curl -X POST http://127.0.0.1:21001/read \
  -H 'Content-Type: application/json' \
  -d '"hello"'

# 4. 线性一致读
curl -X POST http://127.0.0.1:21001/linearizable-read \
  -H 'Content-Type: application/json' \
  -d '"hello"'

# 5. 查看 Leader 信息
curl http://127.0.0.1:21001/leader
```

---

## 依赖版本

| crate | 版本 |
|-------|------|
| `openraft` | `0.10.0-alpha.24` |
| `kameo` | `0.21.0` |
| `actix-web` | `4.14.0` |
| `tokio` | `1.52.3` |
| `reqwest` | `0.13.4` |
| `serde` / `serde_json` | `1.0` |

# kameo_tarpc

一个最小但可运行的分布式示例：

- `actor-node` 运行远端 `kameo` actor
- `rpc-server` 通过 `tarpc` 暴露过程调用
- `rpc-client` 调用 `rpc-server`
- `rpc-server` 再把请求转发给远端 `kameo` actor
- `kameo` actor 内部不再直接维护内存 counter，而是把真实状态写入 `openraft + rocksdb`

调用链是：

```text
rpc-client -> tarpc rpc-server -> remote kameo actor -> openraft state machine -> rocksdb
```

## 参考

- `kameo` 分布式 actor 参考：`/Users/gerald/test/rust/kameo`
- `tarpc` TCP/JSON RPC 参考：`/Users/gerald/test/rust/tarpc`

## 运行

### 一键本地演示

```bash
./demo_distributed.sh
```

脚本会：

1. 启动一个 `actor-node`
2. 启动一个 `rpc-server`
3. 由 `rpc-client` 连续发起两次 RPC
4. 打印最终累加值

成功时会输出类似结果：

```text
rpc_client caller=demo-run-1 amount=7 total=7 ...
rpc_client caller=demo-run-2 amount=7 total=14 ...

final accumulated total: 14
```

脚本每次运行前会清空 `./data/actor-node`，保证演示从 0 开始。

## 手动运行

### 1. 启动 actor 节点

```bash
cargo run -- actor-node \
  --actor-name distributed-counter \
  --swarm-listen-addr /ip4/127.0.0.1/tcp/47011 \
  --raft-db-path ./data/actor-node
```

记下日志中的 `peer_id`，拼出 seed 地址：

```text
/ip4/127.0.0.1/tcp/47011/p2p/<ACTOR_PEER_ID>
```

### 2. 启动 rpc-server

```bash
cargo run -- rpc-server \
  --actor-name distributed-counter \
  --swarm-listen-addr /ip4/127.0.0.1/tcp/47012 \
  --rpc-listen-addr 127.0.0.1:47013 \
  --seed /ip4/127.0.0.1/tcp/47011/p2p/<ACTOR_PEER_ID>
```

### 3. 启动 rpc-client

```bash
cargo run -- rpc-client \
  --server-addr 127.0.0.1:47013 \
  --amount 7 \
  --caller demo-run
```

## 设计说明

- `kameo` 部分使用 custom swarm，而不是 `bootstrap_on`
- `--seed` 会显式注入 swarm 地址簿，并主动 `dial` 到已知节点
- 远端 actor 仍使用 `register/lookup` 方式发现
- `tarpc` 负责对外 RPC 接口，`rpc-server` 本身不承载业务状态
- `openraft 0.10.0-alpha.20` + `rocksdb` 负责 counter 的真实持久化状态
- 当前实现是单节点 raft：目的不是演示 raft 复制，而是先把 `kameo counter` 从内存迁到 raft 状态机

这样更接近真实分布式部署，而不是只依赖局域网自动发现

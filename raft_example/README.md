# 三节点 Raft 示例

参考本机 `raft-rs/examples/single_mem_node` 的 `RawNode` 用法，以及 Qdrant
`src/consensus.rs` 的 Ready / LightReady 处理顺序，实现一个简单的键值状态机。

## 运行

需要 Rust（支持 edition 2024）、Cargo、Git 和 `protoc`（macOS 可用
`brew install protobuf` 安装）。首次运行需要联网下载依赖。

```sh
cargo run --locked
cargo test --locked
```

程序自动完成以下步骤后退出：

1. 创建三个 voter，通过 `tick()` 触发选举。
2. Leader 提交 `name=raft`，复制并应用到三个节点。
3. 暂停 Leader，剩下两个节点选出新 Leader。
4. 提交 `name=raft-after-failover`。
5. 恢复旧节点，让它追赶日志，检查三个副本数据一致。

终端会显示 Leader ID、提交日志的 index，以及各节点的角色、term、applied
和键值数据。选举超时带随机性，每次选出的 Leader 可能不同。最后应输出：

```text
all three replicas agree
```

## GitHub 依赖

`Cargo.toml` 沿用参考 Qdrant 项目固定的 raft-rs 提交，避免跟随分支漂移：

```toml
raft = { git = "https://github.com/tikv/raft-rs", rev = "aafb07c7bab439c6139926a77dfafc5b10e9bc84", default-features = false, features = ["prost-codec"] }
```

Raft 从 GitHub 下载，没有使用本地 `path` 依赖；日志库 `slog` 及传递依赖
由 Cargo 从 crates.io 下载。`Cargo.lock` 固定完整依赖版本。

## 核心代码

所有示例逻辑在 `src/main.rs` 中：

- `Node::on_ready`：保存日志和 HardState，再处理需要持久化后才能发送的消息、
  应用已提交日志，最后处理 `advance()` 返回的 LightReady。
- `Node::apply`：仅对已提交日志执行 `key=value`，并推进 applied index。
- `Cluster::drain`：通过内存消息队列将 Raft 消息送给对应节点的 `step()`。
- `Cluster::tick`：推进逻辑时钟，无需实际等待。
- `Cluster::set`：提交提案后确认其已应用；`propose()` 返回成功本身不代表提交成功。

测试覆盖故障转移和恢复追赶，以及仅剩一个节点时无法提交写入。

## 示例边界

这是单进程、固定三个成员的内存演示。`MemStorage` 不落盘；“下线/恢复”表示暂停
和继续同一个节点，保留其内存，并非进程崩溃后的磁盘恢复。离线期间消息直接丢弃。
没有 TCP/gRPC、成员变更、日志压缩或快照恢复；不支持的日志类型和快照会明确报错。
打印状态只用于观察副本，不是线性一致读 API。写入未确认时返回错误，并不撤销提案，
如果之后恢复多数派，该提案仍可能提交。

参考源码：

- [raft-rs 单节点示例](https://github.com/tikv/raft-rs/tree/aafb07c7bab439c6139926a77dfafc5b10e9bc84/examples/single_mem_node)
- 本机 `/Users/gerald/test/rust/qdrant/src/consensus.rs` 的 `process_ready` / `process_light_ready`

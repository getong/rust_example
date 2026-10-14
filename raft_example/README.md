# Raft + Tokio + libp2p + RocksDB 三节点示例

每个进程运行一个 Raft 节点，三个进程通过真正的 libp2p TCP 连接复制键值数据。
Raft 日志、HardState、ConfState、状态机数据及 applied index 都持久化到 RocksDB。
原先的单进程内存模拟保留在 `examples/in_memory.rs`。

## 快速运行

需要 Rust（支持 edition 2024）、Cargo、Git 和 `protoc`。
macOS 可使用 `brew install protobuf` 安装 protoc。RocksDB 原生构建还需要 C++ 编译器
和 libclang（macOS 安装 Xcode Command Line Tools；Linux 安装 clang/libclang 开发包）。
首次构建需联网，RocksDB C++ 编译可能较慢。

```sh
cargo build --locked
./target/debug/raft_example init
```

`init` 在 `cluster/` 生成三个独立的 Ed25519 身份和配置（已有配置不会覆盖）。
配置固定 `Raft ID ↔ PeerId ↔ 地址`，所有节点拥有同一份 voter 列表。
配置含有该节点自己的私钥；在 Unix 上以 0600 权限创建，目录已加入 `.gitignore`。

分别在三个终端运行：

```sh
# 终端 1
./target/debug/raft_example node cluster/node1.json
# 终端 2
./target/debug/raft_example node cluster/node2.json
# 终端 3
./target/debug/raft_example node cluster/node3.json
```

监听地址分别是 `127.0.0.1:7001`、`:7002`、`:7003`。启动后数秒内自动选主。
任意终端输入 `status` 查看该节点的角色、Leader ID、term、commit、applied 和本地键值。
在 **Leader 所在终端** 输入：

```text
set name raft over libp2p
status
```

`set` 的第一个单词是 key，其余内容是 value。Follower 上写入会返回错误，附带其已知
Leader ID（0 表示未知）；本例不自动转发客户端请求。

输出为 JSON 行。例如：

```json
{"event":"proposed","id":1,"term":1,"index":2}
{"event":"committed","id":1,"term":1,"index":2,"key":"name","value":"raft over libp2p"}
```

`proposed` 只是接收提案；只有 `committed` 表示日志经多数派提交并已应用到该节点。
其他节点也会输出同一条日志的 `committed` 事件。无多数派时可能已经输出 `proposed`，
但不会出现对应 `committed`；该提案在恢复多数派后仍可能提交，不能将未确认视作撤销。
`status` 只是本地观察，不是线性一致读 API。

输入 `quit` 或按 Ctrl-C 退出。stdin 关闭时节点继续运行，便于后台启动。

## 跨机器运行

先生成包含三台机器实际可达地址的配置：

```sh
./target/debug/raft_example init cluster-lan \
  /ip4/192.168.1.11/tcp/7001 \
  /ip4/192.168.1.12/tcp/7002 \
  /ip4/192.168.1.13/tcp/7003
```

将 `node1.json`、`node2.json`、`node3.json` 分别放到对应机器，用同一构建版本启动。
只分发每台机器自己的配置文件。各机器需允许相互连接对应 TCP 端口。

配置中的 `listen` 是本机绑定地址，可按需改为 `/ip4/0.0.0.0/tcp/7001` 等；
`peers[].address` 必须保持为其他节点可达的地址，不能写 `0.0.0.0`。
地址支持 IP 或 DNS；本例没有 mDNS、DHT、NAT 穿透或动态成员发现。
生成配置一次后分发，不要在每台机器上独立执行 `init`，否则 PeerId 不匹配。

## 故障转移与测试

macOS/Linux 上可暂停 Leader 进程，保留它的内存：

```sh
kill -STOP <leader-pid>
# 等另外两个节点选出新 Leader，在新 Leader 终端输入 set 命令
kill -CONT <old-leader-pid>
```

旧 Leader 恢复后会退回 Follower，并通过网络追赶日志。仅剩一个节点时无法提交写入。

也可以直接 `kill -KILL <leader-pid>`，在另两个节点完成选主和写入后，使用原来的
`node cluster/nodeN.json` 命令重启旧节点。它会读取自己的 RocksDB，并追赶缺失日志。
整个集群停止后也可以用原配置和原数据库重新启动，已提交数据会保留。

自动验证：

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
python3 scripts/smoke.py
```

`smoke.py` 需要 Python 3 和 POSIX 信号，使用临时配置及空闲本机 TCP 端口，启动三个
真实子进程，验证选主、拒绝 Follower 写入、复制、SIGKILL Leader 后的故障转移、
重启旧 Leader 后追赶、少数派不提交、恢复多数派后继续写入、整个集群 SIGKILL 后的
数据恢复和继续写入，以及 stdin EOF / Ctrl-C 行为。测试结束清理子进程和临时数据库。
可传入自定义可执行文件路径：`python3 scripts/smoke.py /path/to/raft_example`。

原来的内存示例仍可运行：

```sh
cargo run --locked --example in_memory
```

## 代码结构与处理顺序

- `src/main.rs`：Tokio 多线程运行时；`select!` 同时处理 libp2p Swarm 事件、
  100ms Raft tick、终端命令和 Ctrl-C。`RawNode` 只由这个事件循环持有，无需 Mutex。
  stdin 使用独立阻塞读取线程和有界通道，避免阻塞网络循环或退出流程。
- `src/config.rs`：生成身份、验证固定三节点成员配置及 PeerId 对应关系。
- `src/node.rs`：Raft Ready/LightReady 流程、日志应用和键值状态机。
- `src/storage.rs`：直接实现 `raft::Storage` 的 RocksDB 存储，日志按大端 index 排序；
  处理未提交冲突日志后缀的原子替换，并校验 applied ≤ commit ≤ last。
- `src/network.rs`：TCP + Noise + Yamux；通过 libp2p request-response 传输
  Protobuf 编码的 `raft_proto::eraftpb::Message`，外层使用 CBOR framing。
  Noise 验证 PeerId，应用层进一步校验 PeerId 与消息 `from` 的映射和 `to`。
  本地 tick/campaign/propose 等控制消息不能从网络注入。

```text
Tokio tick / 网络入站 step / 终端 propose
                    ↓
               RawNode::ready
                    ↓
       原子保存 entries + HardState，sync RocksDB WAL
                    ↓
       收集 persisted_messages，原子保存状态机 + applied
                    ↓
          advance → LightReady → advance_apply
                    ↓
       Protobuf encode → libp2p → 远端 decode → step
```

libp2p 返回的 bool ACK 只说明收到并交给 `step()`，不是 Raft 的日志复制确认。
真正的 AppendResponse / VoteResponse 等 Raft 消息由 Ready 流程独立发出。
每个远端最多保留 32 个在途请求，超时 2 秒；失败会 `report_unreachable`，
由 Raft 后续心跳和日志复制重试，慢节点不阻塞事件循环。
连接管理由 tick 独立驱动，显式使用配置中的 PeerId 和地址拨号，每个远端只保留一个拨号尝试。
失败后按 250ms、500ms、1s 等退避，最大 5s；节点后启动或重启后会自动重连。
出站连接显式分配新的临时端口，不复用监听端口，避免同时拨号时的端口冲突和握手失败。

## 依赖

沿用你当前选定的 GitHub 提交，`raft` 与 `raft-proto` 使用完全相同的来源、rev 和 codec：

```toml
raft = { git = "https://github.com/tikv/raft-rs", rev = "ad13f3d90780f53aea2488c6a4b76c0d334bf136", default-features = false, features = ["prost-codec"] }
raft-proto = { git = "https://github.com/tikv/raft-rs", rev = "ad13f3d90780f53aea2488c6a4b76c0d334bf136", default-features = false, features = ["prost-codec"] }
prost = "=0.11.9"
```

`raft` 原本已重新导出 `eraftpb`，所以直接声明 `raft-proto` 并非必须；这里显式声明以体现
网络消息类型的来源。Cargo 将两者使用的 `raft-proto` 合并为同一份依赖。
`prost` 必须匹配该版本 `raft-proto` 使用的 0.11，否则 `Message` trait 不能互通。

保留 libp2p 0.57.0、Tokio 1.53.3，将 `full` 收窄为实际使用的 features。
它们及辅助库从 crates.io 下载，Raft 两个包从 GitHub 下载，完整版本由 `Cargo.lock` 锁定。

## RocksDB 数据目录与恢复

生成的配置包含 `"data_dir": "node1.rocksdb"`（各节点数字不同）。相对路径以配置文件
所在目录为基准，因此默认写入 `cluster/node1.rocksdb`、`node2.rocksdb`、`node3.rocksdb`。
可以改成绝对路径。旧配置没有 `data_dir` 时，也使用该默认规则；启动时会打印实际路径。

每个节点必须使用自己的数据库目录，RocksDB 文件锁防止两个进程同时打开同一数据库。
数据库还保存节点 ID、固定 voter 集合及 PeerId 映射，配置身份不符时拒绝启动。
移动配置时注意一起移动数据库，或把 `data_dir` 改成原数据库的绝对路径。

| RocksDB key | 内容 |
| --- | --- |
| `meta/owner-v1` | 节点身份和固定成员信息 |
| `meta/conf` | Protobuf ConfState |
| `meta/hard` | Protobuf HardState：term、vote、commit |
| `meta/last` | 最后日志 index |
| `meta/applied` | 状态机已应用的 index |
| `log/` + 8 字节大端 index | Protobuf Entry |
| `state/` + key | 状态机 value |

日志追加、冲突后缀删除和 HardState 更新使用一个 `WriteBatch`，开启 WAL 并设置
`sync=true`；只有写入成功后才发出需要持久化保证的 Raft 响应。
状态机更新与 applied index 使用另一个原子同步批次；成功后才更新内存缓存和输出
`committed` 事件。重启先恢复 applied 位置及键值缓存，再补放已提交但尚未应用的日志。
日志不会保存到自定义 JSON 文件；JSON 配置只负责身份、地址和数据库路径。

存储单元测试覆盖冲突后缀替换、大小限制、拒绝覆盖已提交日志、重开数据库，以及
“commit 已持久化但尚未应用”时的恢复。原有内存示例仅用于对照，主程序不再使用 MemStorage。

## 范围

支持同一身份和同一数据库的进程崩溃/重启恢复。没有快照、Raft 日志压缩、动态成员变更
或客户端请求去重，日志会持续增长；RocksDB 自身的 SST compaction 不等于 Raft 日志截断。
存储操作为同步实现，磁盘延迟会延迟该节点的事件循环，适用于学习和小规模演示。
如需进一步扩展，可引入异步持久化调度，但必须保留 Ready 的持久化/响应顺序。

不要删除单个运行中集群成员的数据库后用原身份重新加入；丢失投票/日志会破坏一致性前提。
从旧的内存版本升级时，应先停止所有旧节点，再一起启动新版本；已经丢失的内存数据无法迁移。
完整重置需要先停止所有节点，再明确清理所有节点的数据目录。

参考：

- 本机 `raft-rs/examples/single_mem_node` 和 Qdrant `src/consensus.rs` 的 Ready 顺序
- [raft-rs 源码](https://github.com/tikv/raft-rs/tree/ad13f3d90780f53aea2488c6a4b76c0d334bf136)
- [libp2p request-response 文档](https://docs.rs/libp2p/0.57.0/libp2p/request_response/)

- [RocksDB 同步写入选项](https://docs.rs/rocksdb/0.25.0/rocksdb/struct.WriteOptions.html)

## 拨号错误排查

节点尚未启动时，其他节点暂时拨号失败是正常的。三个节点无需同时启动。
日志会输出 `dial node N (PeerId) at 地址 failed: 具体错误; retry in ...`，成功后输出
`connected to node N`。不再只打印 `Failed to dial the requested peer`。

- `Connection refused`：目标端口没有监听，检查节点进程和 `listen`。
- `WrongPeerId`：地址对应的节点身份与配置不符，检查三份配置是否来自同一次 `init`。
- 超时：检查地址可达性及防火墙；跨机器不能使用 `127.0.0.1` 作为对端地址。

不要为了修复暂时的拨号失败重新生成身份或删除 RocksDB。升级网络代码后，应使用原配置和
原数据库重启节点；已经运行的旧进程不会自动加载新编译的二进制。

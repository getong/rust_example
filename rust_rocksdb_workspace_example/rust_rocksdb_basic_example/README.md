# rust-rocksdb + Tokio 演示

这是一个小而完整的 `rust-rocksdb 0.52` 示例，展示 RocksDB 在 Rust 服务中的典型作用，以及如何安全地把它放进 Tokio 异步程序。

RocksDB 是嵌入进程的持久化有序键值数据库。它适合本地状态、缓存、索引、事件或元数据等需要低延迟读写的数据；应用直接链接数据库库文件，不需要另起数据库服务。`rust-rocksdb` 是 RocksDB C++ API 的 Rust 同步绑定，键和值本质上都是字节数组。

## 示例覆盖的功能

| 功能 | 示例中的用途 |
| --- | --- |
| `put/get/delete` | 基本键值 CRUD |
| Column Family | 在同一个数据库中隔离不同逻辑数据集 |
| `WriteBatch` | 原子地提交多个写入和删除 |
| `multi_get` | 一次读取多个键，并保持输入顺序 |
| 有序前缀扫描 | 利用 RocksDB 的字节序排列查询一组键 |
| WAL flush + sync | 明确演示持久化边界 |
| 属性与序列号 | 获取估算键数和最新写入序列号 |
| 多 Tokio task 并发 | 共享线程安全的 RocksDB 句柄 |

## 为什么需要 `spawn_blocking`

RocksDB 内部会使用后台线程执行 flush 和 compaction，但它暴露给 Rust 的 `get`、`put`、迭代器等 API 仍然是同步调用。直接在 Tokio worker 上调用它们，磁盘抖动或压缩压力较大时会阻塞同一 worker 上的其他异步任务。

本项目的调用路径是：

```text
async task
    -> 异步 Semaphore（限制排队和并发量）
    -> tokio::task::spawn_blocking
    -> rust-rocksdb 同步 API
    -> RocksDB / WAL / memtable / SST 文件
```

[`AsyncRocksDb`](src/async_db.rs) 会在进入阻塞线程池前复制输入参数，也会把迭代结果复制为拥有所有权的 `Vec<u8>`，因此 RocksDB 借用和迭代器不会跨过 `.await`。默认最多并发提交 16 个阻塞操作，也可通过 `open_with_max_concurrency` 调整。

## 运行

构建 `rust-rocksdb` 需要可用的 C++ 工具链、Clang 和 libclang。macOS 安装 Xcode Command Line Tools 后通常即可构建。

```bash
cargo run
```

默认数据库位于 `target/rocksdb-demo`，可传入其他目录：

```bash
cargo run -- /tmp/my-rocksdb-demo
```

运行测试：

```bash
cargo test
```

## 代码结构

- [`src/async_db.rs`](src/async_db.rs)：Tokio 友好的有界阻塞封装。
- [`src/demo.rs`](src/demo.rs)：CRUD、列族、批写、扫描和并发演示。
- [`src/main.rs`](src/main.rs)：解析数据库路径并启动异步演示。
- [`tests/async_db.rs`](tests/async_db.rs)：使用临时数据库验证行为。

## 生产环境注意事项

- `spawn_blocking` 只是不阻塞 Tokio worker，并不会让 RocksDB 变成原生异步数据库，也不能取消已经开始的同步调用。
- 键和值没有模式和序列化协议；JSON、Protobuf 或自定义二进制格式需要由应用决定并进行版本管理。
- `WriteBatch` 提供单数据库内的原子写，但不等同于带冲突检测和回滚语义的事务；需要事务时应评估 `TransactionDB` 或 `OptimisticTransactionDB`。
- 前缀扫描返回拥有所有权的数据，结果集很大时应分页或流式分批处理，避免一次占用过多内存。
- 默认写入 WAL，但写入成功不代表已经 `fsync`。示例用 `flush_wal(true)` 明确同步；生产中应按延迟和持久性要求配置 `WriteOptions`。
- RocksDB 性能依赖 workload。block cache、压缩、memtable、后台任务和 compaction 参数应先测量再调优。

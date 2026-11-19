# Agones Rust SDK 示例

这是一个使用 Rust 和 Tokio 调用 Agones SDK 的示例，演示健康上报、GameServer 更新监听、元数据设置、就绪、预留和关闭操作，并提供可选的分配、计数器及列表演示。

## 文档作用

本 README 用于帮助阅读者快速了解项目用途、代码结构、运行条件和执行流程，尤其说明 SDK 服务依赖与关闭时序，便于学习代码和排查运行问题。

本项目主要用于 SDK 接入演示，不包含实际游戏逻辑、玩家连接处理或对外监听的游戏端口，也不是完整的游戏服务器实现。

## 项目结构与依赖

```text
.
├── Cargo.toml       # 项目配置与依赖声明
├── Cargo.lock       # 锁定依赖版本
├── README.md        # 项目使用说明
└── src/
    ├── main.rs      # 命令行入口、后台任务、生命周期和服务器信息输出
    └── demos.rs     # 可选的 Beta 计数器与列表演示
```

当前依赖声明为：

| 依赖 | 版本要求 | 用途 |
| --- | --- | --- |
| `agones` | `1.62.0` | 创建 SDK 客户端并调用 GameServer 相关接口 |
| `tokio` | `1.54.1`，启用 `full` | 提供异步运行时、后台任务、定时器和通道 |

项目使用 Rust 2024 edition，入口通过 `#[tokio::main]` 创建包含 4 个工作线程的多线程运行时。

## 代码执行流程

`main()` 解析命令行选项后调用 `run()`，根据执行结果打印日志并返回退出码 `0`（成功）或 `1`（错误）。`--help` 无需连接 SDK 即可查看用法，未知参数会在连接前报错。

`run()` 按以下顺序执行：

1. **连接 SDK 服务**：调用 `agones::Sdk::new_with_host(None, None, None)`，允许通过环境变量指定 SDK 主机和端口。
2. **启动健康检查任务**：使用 `health_check()` 获取发送通道，通过 Tokio 定时器每 2 秒发送一次健康信号。
3. **启动状态监听任务**：克隆 SDK 客户端，调用 `watch_gameserver()`，打印收到的名称、命名空间、UID、标签、注解、状态、地址及端口。
4. **设置元数据**：依次设置标签 `test-label = test-value` 和注解 `test-annotation = test value`。
5. **标记就绪**：调用 `ready()`，通知 Agones 当前服务器已准备好接收连接。
6. **临时预留**：调用 `reserve(Duration::from_secs(5))`，请求预留 5 秒；预留到期后由 SDK 服务端处理恢复就绪。
7. **读取服务器信息**：等待 6 秒，再调用 `get_gameserver()` 并打印服务器详情。缺少元信息或状态时记录提示，避免 `unwrap()` 导致 panic。
8. **执行可选演示**：指定 `--allocate` 时调用 `allocate()`；指定 `--counters-and-lists` 时运行 Beta 计数器与列表演示。两个选项可同时使用，分配先执行。
9. **模拟运行并请求关闭**：每次等待 10 秒，在第 6 次等待结束后调用 `shutdown()`，成功后退出循环。

### 功能覆盖与本次补齐

对照当前锁定的 `agones 1.62.0` 源码，原示例遗漏了 `allocate()` 和全部 13 个 Beta 计数器／列表接口，也没有展示远程主机配置及完整的服务器连接信息。本次增加的覆盖如下：

| 功能 | 调用接口 | 启用方式 |
| --- | --- | --- |
| 主机与端口配置 | `new_with_host()` | 默认启用，读取环境变量 |
| 健康、监听和生命周期 | `health_check()`、`watch_gameserver()`、`ready()`、`reserve()`、`shutdown()` | 默认执行 |
| 元数据与服务器详情 | `set_label()`、`set_annotation()`、`get_gameserver()` | 默认执行 |
| 自分配 | `allocate()` | `--allocate` |
| 计数器查询 | `get_counter_count()`、`get_counter_capacity()` | `--counters-and-lists` |
| 计数器修改 | `set_counter_count()`、`set_counter_capacity()`、`increment_counter()`、`decrement_counter()` | `--counters-and-lists` |
| 列表查询 | `get_list_capacity()`、`get_list_length()`、`get_list_values()`、`list_contains()` | `--counters-and-lists` |
| 列表修改 | `set_list_capacity()`、`append_list_value()`、`delete_list_value()` | `--counters-and-lists` |

`allocate()` 标记当前 GameServer 为已分配，不会帮客户端选择其他服务器，也不演示 Kubernetes 的 GameServerAllocation 流程。调用成功表示请求成功，最终状态通过监听更新观察。

虽然 SDK 暴露了 `alpha()`，当前版本的 `Alpha` 没有公开的玩家跟踪方法，因此本项目没有添加 `player_connect()` 等不可用调用。列表中的玩家 ID 只是字符串数据，示例不会处理真实玩家连接。

### 健康检查与监听任务如何停止

两个后台任务各自使用一个 `tokio::sync::oneshot` 通道接收停止信号，发送端分别由 `run()` 中的 `_health` 和 `_watch` 持有。`run()` 返回或因错误提前退出时，发送端被释放，后台任务可通过接收端完成来感知取消。

Tokio 的定时器首次 tick 立即就绪，因此健康任务会立即尝试发送第一次健康信号，随后每 2 秒发送一次。健康信号发送失败时，健康检查任务会退出。监听流被服务端关闭、建立监听失败或读取流出错时，监听任务会记录原因并退出，没有自动重连机制。这些后台任务的错误不会直接作为 `run()` 的返回错误传播。

程序没有等待两个后台任务完成后再退出，因此取消日志不保证一定打印。

### 关闭时序说明

日志中的 `Shutting down after 60 seconds...` 指进入模拟运行循环后累计等待约 60 秒，**不是从进程启动起计算**；此前还有 SDK 初始化、接口调用和 6 秒等待。

`shutdown()` 向 Agones 请求关闭，调用成功后立即跳出模拟运行循环。整个主流程的显式等待时间约为 66 秒，另加连接、接口调用和可选演示耗时；实际部署中进程可能由外部环境终止。旧代码在关闭请求后仍会继续循环约 40 秒，本次已修正。

## 运行方式

### 环境准备

- 安装支持 Rust 2024 edition 的 Rust 工具链和 Cargo。
- 准备可访问的 Agones SDK 服务：本地调试使用本地模式的 SDK Server；集群中则需要对应的 SDK sidecar。
- 当前仓库未提供 SDK Server 启动脚本、Dockerfile 或 Kubernetes 部署清单，需要自行准备运行环境。

**本程序是 SDK 客户端，不会自行启动 Agones SDK 服务。**

### 编译与启动

在项目目录中执行：

```bash
cargo build --locked
cargo run --locked
```

按当前使用的 `agones` 依赖实现，`Sdk::new_with_host(None, None, None)` 优先读取 `AGONES_SDK_GRPC_HOST` 和 `AGONES_SDK_GRPC_PORT`，默认使用 `http://localhost` 和 `9357`。端口值无法解析时 SDK 会回退到 `9357`。

如果 SDK 服务使用其他端口，可以执行：

```bash
AGONES_SDK_GRPC_PORT=9358 cargo run --locked
```

这里的端口是 **SDK 的 gRPC 端口**，不是游戏业务端口。连接远程调试环境时，主机值包含 `http://` 协议前缀，不包含端口，例如：

```bash
AGONES_SDK_GRPC_HOST=http://192.168.1.20 AGONES_SDK_GRPC_PORT=9357 cargo run --locked
```

### 可选的分配与计数器／列表演示

```bash
cargo run --locked -- --help
cargo run --locked -- --allocate
cargo run --locked -- --counters-and-lists
cargo run --locked -- --allocate --counters-and-lists
```

计数器／列表演示要求 SDK Server 支持并启用 Counts and Lists，并在 GameServer 创建时预先声明以下键。将下面的配置合并到用于演示的 GameServer 的 `spec`，或本地 SDK Server 所加载的 GameServer 文件中；这是配置片段，不能直接作为完整 Kubernetes 清单提交。

```yaml
spec:
  counters:
    demo-rooms:
      count: 0
      capacity: 10
  lists:
    demo-players:
      values: []
      capacity: 10
```

程序先读取两个键，要求初始值与上面的配置一致，再执行修改：

1. 计数器容量设为 20，数量设为 1，增加 1 后查询，再减少 1，最后将数量恢复为 0、容量恢复为 10。
2. 列表容量设为 20，添加 `example-player`，查询是否包含该值、列表长度和全部值，再删除该值并将容量恢复为 10。

正常完成后恢复初始数据。接口失败会返回包含具体方法和键名的错误，并以退出码 `1` 结束；中途失败不会自动回滚已完成的修改。请使用独立的演示 GameServer，避免其他程序同时修改这两个演示键；初始值检查及后续调用不构成事务。未指定该选项时不会调用 Beta 接口，也无需声明这些键。

### 运行日志

正常执行时可看到以下关键日志；健康检查与状态监听属于并发任务，相关日志顺序可能交错：

```text
Rust Game Server has started!
Creating SDK instance
Setting a label
Setting an annotation
Marking server as ready...
...marked Ready
Setting as Reserved for 5 seconds
...Reserved
Getting GameServer details...
GameServer name: ...
Running for 0 seconds
...
Shutting down after 60 seconds...
...marked for Shutdown
```

如果主流程正常完成，还会打印 `Rust Game Server finished.`。

## 常见问题与边界

- **停留在 `Creating SDK instance` 或提示 `unable to create sdk client`**：检查 SDK 服务是否启动，以及本机 gRPC 端口是否匹配。当前依赖在初始化时会尝试获取 GameServer 信息，等待超时约为 30 秒。
- **标签、就绪、预留或关闭操作失败**：主流程会打印对应操作的错误并以退出码 `1` 结束；检查 SDK 服务和 GameServer 的运行状态。
- **监听任务报错但程序继续运行**：监听任务是独立的后台任务，没有将错误传回主流程，也没有自动重连逻辑。
- **计数器或列表报错**：检查 SDK Server 的功能支持及演示键是否预先声明；初始数据不匹配时按上面的配置重置演示环境。部分修改已成功后再发生错误，需要自行恢复数据。
- **GameServer 数据缺少元信息或状态**：记录缺失字段提示，继续处理存在的数据。
- **发出关闭请求后仍有监听日志**：关闭请求成功后主循环会退出，但并发监听可能在退出前输出最后的状态更新。

当前项目没有提供自动化测试；验证完整 SDK 交互流程需要配合可用的 SDK 服务运行。本 README 描述的是当前代码行为，不代表已经完成本地 SDK 或集群环境的运行验证。

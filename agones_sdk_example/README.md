# Agones Rust SDK 示例

这是一个使用 Rust 和 Tokio 调用 Agones SDK 的最小示例，演示游戏服务器如何上报健康状态、监听 GameServer 更新，以及执行就绪、预留和关闭操作。

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
    └── main.rs      # 程序入口、后台任务和 SDK 调用流程
```

当前依赖声明为：

| 依赖 | 版本要求 | 用途 |
| --- | --- | --- |
| `agones` | `1.62.0` | 创建 SDK 客户端并调用 GameServer 相关接口 |
| `tokio` | `1.54.1`，启用 `full` | 提供异步运行时、后台任务、定时器和通道 |

项目使用 Rust 2024 edition，入口通过 `#[tokio::main]` 创建包含 4 个工作线程的多线程运行时。

## 代码执行流程

`main()` 调用 `run()`，根据执行结果打印日志并以退出码 `0`（成功）或 `1`（返回错误）结束进程。

`run()` 按以下顺序执行：

1. **连接 SDK 服务**：调用 `agones::Sdk::new(None, None)`，使用默认连接配置。
2. **启动健康检查任务**：使用 `health_check()` 获取发送通道，通过 Tokio 定时器每 2 秒发送一次健康信号。
3. **启动状态监听任务**：克隆 SDK 客户端，调用 `watch_gameserver()`，打印收到的 GameServer 名称和状态。
4. **设置元数据**：依次设置标签 `test-label = test-value` 和注解 `test-annotation = test value`。
5. **标记就绪**：调用 `ready()`，通知 Agones 当前服务器已准备好接收连接。
6. **临时预留**：调用 `reserve(Duration::from_secs(5))`，请求预留 5 秒；预留到期后由 SDK 服务端处理恢复就绪。
7. **读取服务器信息**：等待 6 秒，再调用 `get_gameserver()` 并打印 GameServer 名称。
8. **模拟运行并请求关闭**：进入 10 次循环，每次等待 10 秒，在第 6 次等待结束后调用 `shutdown()`。

### 健康检查与监听任务如何停止

两个后台任务各自使用一个 `tokio::sync::oneshot` 通道接收停止信号，发送端分别由 `run()` 中的 `_health` 和 `_watch` 持有。`run()` 返回或因错误提前退出时，发送端被释放，后台任务可通过接收端完成来感知取消。

健康信号发送失败时，健康检查任务会退出。监听流被服务端关闭时，监听任务会退出；建立监听失败或读取流出错时会打印日志，代码没有实现重新建立监听连接的机制。这些后台任务的错误不会直接作为 `run()` 的返回错误传播。

程序没有等待两个后台任务完成后再退出，因此取消日志不保证一定打印。

### 关闭时序说明

日志中的 `Shutting down after 60 seconds...` 指进入模拟运行循环后累计等待约 60 秒，**不是从进程启动起计算**；此前还有 SDK 初始化、接口调用和 6 秒等待。

`shutdown()` 只是向 Agones 请求关闭，当前代码在调用成功后不会立即跳出循环。如果进程未被外部环境终止，循环仍会继续到累计等待约 100 秒，整个主流程的显式等待时间约为 106 秒，另加连接和接口调用耗时。在实际部署环境中，进程可能在关闭请求后被提前终止。

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

按当前使用的 `agones` 依赖实现，`Sdk::new(None, None)` 连接本机 SDK 服务，优先读取 `AGONES_SDK_GRPC_PORT`，未提供可用端口值时默认使用 `9357`。

如果 SDK 服务使用其他端口，可以执行：

```bash
AGONES_SDK_GRPC_PORT=9358 cargo run --locked
```

这里的端口是 **SDK 的 gRPC 端口**，不是游戏业务端口。当前代码调用的是 `Sdk::new()`，没有提供远程 SDK 主机地址的配置入口。

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
- **GameServer 数据缺少元信息或状态时 panic**：当前示例对部分字段直接调用 `unwrap()`，没有处理这些字段缺失的情况。
- **发出关闭请求后仍有运行日志**：这是当前循环逻辑的行为，`shutdown()` 后没有立即返回或退出循环。

当前项目没有提供自动化测试；验证完整 SDK 交互流程需要配合可用的 SDK 服务运行。本 README 描述的是当前代码行为，不代表已经完成本地 SDK 或集群环境的运行验证。

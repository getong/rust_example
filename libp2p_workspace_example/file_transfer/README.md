# Awesome Chat libp2p

基于 `libp2p` gossipsub partial message 的文件传输示例。

一个节点用 `/send <path>` 发布文件后，文件会被切成多个分片。其它节点通过 gossipsub partial message 逐步接收自己缺失的分片，并把已经拥有的分片继续广播给其它节点。文件完整后会在接收端合并写入 `received/` 目录。

## 环境变量

程序启动前必须设置 `CHAT_MDNS_ENABLED`，否则直接 `cargo run` 会报：

```text
Error: environment variable not found
```

`export` 只对当前终端生效，所以每个终端都要分别执行。

## 三个终端运行

打开终端 1：

```sh
export CHAT_MDNS_ENABLED=true
cargo run
```

打开终端 2：

```sh
export CHAT_MDNS_ENABLED=true
cargo run
```

打开终端 3：

```sh
export CHAT_MDNS_ENABLED=true
cargo run
```

看到 `Discovered peer`、`Connected to peer` 或 `Gossipsub subscribed` 后，在终端 1 输入：

```text
/send ./Cargo.toml
```

终端 2 和终端 3 会逐步收到文件分片，输出类似：

```text
Received file partial from <peer>: Cargo.toml group=<id> parts=4/8
Completed file from <peer>: Cargo.toml (...) -> received/<blake3-hex>
```

接收完成后的文件路径在：

```text
received/<blake3-hex>
```

接收端完成合并后会计算文件内容的 BLAKE3 hash，并把 hash 作为文件名。写入使用 create-new 语义，如果相同内容的文件已经存在，会跳过写入并保留已有文件，不会覆盖。

## 验证文件

例如发送 `Cargo.toml` 后，可以对比：

```sh
diff Cargo.toml received/<blake3-hex>
```

没有输出表示文件一致。

## 命令

程序运行后支持：

```text
/send <path>  通过 gossipsub partial message 发送文件
/peers        打印当前连接的 peer
/help         打印命令
```

普通文本不会作为聊天发送；现在 stdin 只接受命令。

## Gossipsub partial message

当前代码对 `file-transfer` topic 启用了 partial message：

- 启动时调用 `enable_partials_for_topic(..., true)`。
- `/send <path>` 会读取文件并调用 `publish_partial`。
- metadata 中包含文件名、文件大小、分片总数和本节点已拥有分片的 bitmap。
- body 中只携带对端缺失的分片，每次最多发送 4 个分片。
- 接收端处理 `gossipsub::Event::Partial`，合并分片，继续 `publish_partial` 广播已有分片。
- 文件完整后以 BLAKE3 内容 hash 作为文件名写入 `received/`，相同内容的文件会去重且不会覆盖已有文件；同时保留在内存中，后加入的第三个节点订阅 topic 后也可以继续从已有节点同步。

## 使用 bootstrap peer

如果不想使用 mDNS，可以关闭 mDNS，并手动指定 bootstrap peer。

终端 1：

```sh
export CHAT_MDNS_ENABLED=false
cargo run
```

记录终端 1 输出的 `Peer ID` 和 `Listening on` 地址。

终端 2：

```sh
export CHAT_MDNS_ENABLED=false
export CHAT_BOOTSTRAP_PEERS="/ip4/127.0.0.1/tcp/<PORT>/p2p/<PEER_ID>"
export CHAT_BOOTSTRAP_PEERS="/ip4/127.0.0.1/tcp/51726/p2p/12D3KooWSJeWVHGF7d4Pki8yJVzpMVw39EVv3rMHHKqmzUhNJjjZ"
cargo run
```

把 `<PORT>` 替换成终端 1 监听地址里的端口，把 `<PEER_ID>` 替换成终端 1 输出的 Peer ID。

多个 bootstrap peer 用英文逗号分隔：

```sh
export CHAT_BOOTSTRAP_PEERS="/ip4/127.0.0.1/tcp/<PORT1>/p2p/<PEER1>,/ip4/127.0.0.1/tcp/<PORT2>/p2p/<PEER2>"
```

## 依赖版本

`libp2p` 使用 GitHub 指定 revision：

```text
https://github.com/libp2p/rust-libp2p
6a6814bb698b01c40d6ce08eb0f61281c909124f
```

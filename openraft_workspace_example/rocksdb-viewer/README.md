# RocksDB 网络数据浏览器

GPUI Kit 桌面客户端通过节点的 HTTP JSON 接口查看 RocksDB 数据。客户端没有 RocksDB 依赖，不打开本地数据库；服务端使用已经打开的节点数据库句柄。

## 启动

先使用更新后的代码重新构建、启动集群节点，确保其 HTTP 服务可访问（例如 `http://127.0.0.1:3001`）。浏览器位于与 `openraft_libp2p_cluster` 同级的 `rocksdb-viewer` 目录，在浏览器目录执行：

```sh
cargo run -- \
  --url http://127.0.0.1:3001 --group users
```

可追加 `--cf sm_data --prefix 'user:'`。不传参数时 URL 默认是 `http://127.0.0.1:3001`，group 留空使用节点默认组。旧的 `--db` 参数已移除。

- 窗口中修改 URL、group、UTF-8 key 前缀后点击「连接 / 刷新」应用条件并返回第一页。
- URL 是节点基础地址，支持 HTTP/HTTPS，以及反向代理路径前缀；不要填写 `/rocksdb/entries` 或查询参数。
- 默认展示业务列族 `sm_data`，也可切换节点实际提供的 `default`、`meta`、`sm_meta`、`logs`。
- 每页最多 100 条，按 key 字节顺序排列。「下一页」传递服务端游标，「首页」从头读取。
- JSON 对象按字段拆成表格列，每条记录一行。嵌套对象和数组按路径展开，字符串中嵌入的 JSON 也会展开。常用标识、状态字段优先显示；表格最多展示 24 个业务字段，可横向滚动。
- 上方保留字段表格（展开最多 128 个字段、16 层嵌套）；点击记录后，下方显示该记录原始结构的缩进 JSON，支持滚动和复制。布尔值、null、数组及嵌入的 JSON 字符串保持原有类型，不使用表格的扁平化字段重新拼装。普通文本记录直接显示原文。
- JSON 格式化在后台完成；这里展示的是服务端返回值的格式化版本，不保证与磁盘原文的空白、字段顺序完全一致。
- 表格保持字段化展示，下方详情显示 JSON；二进制内容不显示为 JSON。服务端仅解析不超过 4096 字节的值；超限值、无效 JSON 和二进制记录显示明确状态，避免把不完整数据误当成业务字段。
- 每个浏览器实例复用同一个 HTTP 客户端及其连接池，空闲连接保留最多 90 秒。连接超时 5 秒、请求超时 15 秒，响应上限 4 MiB。
- 网络请求、响应 JSON 解码（sonic-rs）、嵌套字段展开和列整理均在后台执行；UI 线程只接收完成后的数据并渲染。连接失败、HTTP 错误或无效响应会显示错误并保留上次成功结果。

## 服务端 API

复用节点现有 Axum HTTP 服务，无需单独部署 gRPC 服务或增加端口：

```sh
curl 'http://127.0.0.1:3001/rocksdb/entries?group_id=users&cf=sm_data&prefix=user%3A'
```

参数：`group_id`（也接受 `group`，缺省使用节点默认组）、`cf`（缺省 `sm_data`）、`prefix`（UTF-8 key 前缀）、`cursor`（上页返回的 `next`）。

响应示例：

```json
{
  "group_id": "users",
  "column_families": ["default", "sm_data", "sm_meta", "meta", "logs"],
  "column_family": "sm_data",
  "rows": [{"key": "user:1", "value": {"kind": "text", "data": "Alice"}, "key_bytes": 6, "value_bytes": 5}],
  "next": null
}
```

`next` 非空时原样作为下一页的 `cursor`，不要从预览文本生成游标。翻页必须保持同一节点、group、列族和前缀。

`value` 使用类型标记：`{"kind":"json","data":{"name":"Alice"}}` 直接携带 JSON 对象；`text` 携带普通文本；`binary`、`truncated`、`invalid_json` 只携带状态。服务端在读取线程中解析业务 JSON，客户端随 HTTP 响应一次解码，不再解析被转义的根 value 字符串。业务字段里本身嵌入的 JSON 字符串仍在后台按需展开。

此响应格式取代旧版的字符串 value，升级时需要同时重新构建并重启节点和浏览器；旧节点会被新客户端报告为响应格式不兼容。

无效列族/游标返回 400，节点未托管该 group 返回 404，数据库读取失败返回 500。请求只读取 URL 指定节点的本地数据，不自动转发到 leader。每页使用主库快照，分页之间可以看到新写入；不提供跨页快照或 Raft 线性一致读保证。

## 工程结构与验证

`../openraft_libp2p_cluster/src/browser_protocol.rs` 定义服务端和客户端共享的 JSON 数据结构（编译时通过相对路径引用）；`../openraft_libp2p_cluster/src/http/browser.rs` 实现服务端查询；本工具的 `src/lib.rs` 只实现 HTTP 客户端。

目录布局：

```text
openraft_workspace_example/
├── openraft_libp2p_cluster/
└── rocksdb-viewer/
```

本工具继续保留独立 Cargo 工作区，隔离 GPUI 与集群 libp2p 的 Wasm 依赖约束。不要把 `gpui-kit` 添加到集群主包。

```sh
cargo check --manifest-path ../Cargo.toml -p openraft_libp2p_cluster --lib --locked
cargo test --manifest-path ../Cargo.toml -p openraft_libp2p_cluster --lib http::browser::tests --locked
cargo check
cargo test --lib
```

服务端测试覆盖真实 RocksDB 的分页、二进制游标、后续写入、列族和参数校验；客户端通过本地 HTTP 测试服务器验证 URL 参数编码、结构化 JSON 读取、错误处理，以及连续两次请求复用同一 TCP 连接。

JSON 编解码统一使用 `sonic_rs`；`serde_json::Value` 仅作为共享结构化值的数据表示保留，保持字段匹配逻辑和接口格式兼容。具体性能收益取决于数据及构建配置，尚未做基准测量。

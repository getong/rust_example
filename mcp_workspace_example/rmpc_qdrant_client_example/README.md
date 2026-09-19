# Qdrant RAG MCP 客户端

独立的 Rust MCP client，对接 `rmpc_qdrant_example` 的 stdio server。
客户端启动 `rmpc_qdrant_example serve` 子进程，完成 MCP 初始化、工具发现和工具调用。
查询和 embedding 由 server 执行，client 不依赖 Qdrant SDK。

## 构建和运行

在 workspace 根目录执行：

```sh
cargo build -p rmpc_qdrant_example -p rmpc_qdrant_client_example

# 默认列出工具，不需要已启动 Qdrant 或 embedding 服务
cargo run -p rmpc_qdrant_client_example -- tools

# 通过 similar_ue 查询已有点的相似内容，需要 Qdrant 和已导入集合
cargo run -p rmpc_qdrant_client_example -- point docs 0 5
cargo run -p rmpc_qdrant_client_example -- point engine 0 3

# 通过 search_ue 执行自然语言检索，还需已就绪的 embedding 服务
export EMBEDDING_URL=http://127.0.0.1:8000/v1/embeddings
cargo run -p rmpc_qdrant_client_example -- text docs 'How do I use delegates in Unreal Engine?' 5
```

每次运行自动启动并关闭 server，无需先在另一个终端运行 `serve`。
stdio 通过子进程管道通信，不能附着到另一个终端已运行的 server；此例不提供 HTTP 连接。
默认寻找与 client 二进制同目录的 server，因此 debug/release 构建模式应保持一致。
也可指定路径（路径含空格时加引号；变量仅为可执行文件路径，不包含参数）：

```sh
export MCP_SERVER_BIN=/absolute/path/to/rmpc_qdrant_example
cargo run -p rmpc_qdrant_client_example -- tools
```

server 继承当前环境的 `QDRANT_URL`（默认 `http://localhost:6334`）、`QDRANT_API_KEY`、
`EMBEDDING_URL`、`EMBEDDING_MODEL`、`EMBEDDING_API_KEY`、`EMBEDDING_INSTRUCTION`。
数据导入和模型启动说明见 [server README](../rmpc_qdrant_example/README.md)。

## 命令和结果

- 无参数或 `tools`：输出工具名称、说明及输入 schema。
- `text docs|engine '问题' [TOP_K]`：调用 `search_ue`。
- `point docs|engine POINT_ID [TOP_K]`：调用 `similar_ue`。
- `--help`：显示帮助，不启动 server。

TOP_K 默认 5，范围 1–100。POINT_ID 为非负整数，是 Qdrant 点 ID（导入行号）。
查询命令将完整 MCP CallToolResult 作为 JSON 输出到 stdout：
`structuredContent` 包含 matches、context 等检索结果，`content` 保留兼容文本，
`isError` 表示服务端工具执行是否失败。日志和诊断写入 stderr。

协议错误、工具执行错误、启动失败、超时均返回非零退出码。
初始化超时 15 秒，工具发现及调用总超时 300 秒；关闭管道后等待 server 退出，
超过 5 秒则终止并回收子进程。

## 验证

```sh
cargo test -p rmpc_qdrant_client_example
cargo clippy -p rmpc_qdrant_client_example --all-targets -- -D warnings
```

不依赖外部服务的跨进程检查：`tools` 应发现 search_ue、similar_ue；
`text docs ''` 应收到 server 返回的 `isError: true` 和“搜索问题不能为空”，退出码为 1。

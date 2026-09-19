# MCP + Qdrant UE RAG

参考 `../rmcp_example` 的 MCP 通信和
`../../qdrant_workspace_example/qdrant_search_ue_example` 的检索逻辑。
保留现有包名 `rmpc_qdrant_example`。

调用链：MCP 调用者 → Rust MCP server → 文本 embedding（仅文本查询）
→ Qdrant 向量检索 → MCP 结构化结果 → 调用者。
服务只查询已导入的数据，不创建集合、不写入数据；回答生成由调用者的 LLM 完成。

## MCP server feature

需要 `server`。rmcp 3.4.0 默认 features 已包含 `server` 和 `macros`；这里显式声明
`features = ["server", "client", "transport-io"]`，其中 `client` 用于内置调用演示，
`transport-io` 用于 stdio 通信。外部客户端通过 tools/list 发现工具、tools/call 调用工具。

## 准备

Qdrant 使用 gRPC 端口，默认 `http://localhost:6334`。
需要已有 `ue58_rag_docs` 或 `ue58_rag_engine` 集合，与参考示例共用数据。

文本搜索还需启动参考示例的 embedding 服务：

```sh
cd ../qdrant_workspace_example/qdrant_search_ue_example
uv run --script embedding_server.py
```

等待模型就绪。另开终端，在本 workspace 目录执行：

```sh
export QDRANT_URL=http://localhost:6334
export EMBEDDING_URL=http://127.0.0.1:8000/v1/embeddings
cargo run -p rmpc_qdrant_example -- demo docs text 'How do I use delegates in Unreal Engine?' 5
cargo run -p rmpc_qdrant_example -- demo engine text 'How does FIntMargin work?' 5
```

无 embedding 服务时，可使用已有点的向量：

```sh
cargo run -p rmpc_qdrant_example
cargo run -p rmpc_qdrant_example -- demo engine point 0 5
```

demo 真实执行 MCP 初始化、工具发现、工具调用，再查询真实 Qdrant；不使用伪造数据。
查询失败返回非零退出码。point ID 是导入行号，不是 payload 中的 chunk ID。

可选环境变量：

- `QDRANT_API_KEY`：Qdrant 认证。
- `EMBEDDING_MODEL`：默认 `Qwen/Qwen3-Embedding-0.6B`，必须与入库一致。
- `EMBEDDING_API_KEY`：embedding 服务认证。
- `EMBEDDING_INSTRUCTION`：默认 UE 文档与源码检索任务描述；设为空可关闭前缀。

embedding 返回值必须为 1024 维、非零、有限数值向量。仅设置 URL 不会启动服务。
本地 embedding 地址绕过代理，远程地址保留代理配置。连接超时 5 秒，请求超时 120 秒。

## 外部 MCP 调用者

```sh
cargo build -p rmpc_qdrant_example
cargo run -p rmpc_qdrant_example -- serve
```

stdio 模式由 MCP 客户端启动进程，stdout 专用于协议消息，日志写入 stderr。
下面是通用 MCP 配置示例；将二进制路径改成 `cargo build` 实际生成的绝对路径
（若配置了 CARGO_TARGET_DIR，以该目录为准）：

```json
{
  "mcpServers": {
    "ue-rag": {
      "command": "/absolute/path/to/target/debug/rmpc_qdrant_example",
      "args": ["serve"],
      "env": {
        "QDRANT_URL": "http://localhost:6334",
        "EMBEDDING_URL": "http://127.0.0.1:8000/v1/embeddings"
      }
    }
  }
}
```

工具调用参数：

```json
{"name":"search_ue","arguments":{"query":"How do delegates work?","split":"docs","top_k":5}}
```

```json
{"name":"similar_ue","arguments":{"point_id":0,"split":"engine","top_k":5}}
```

`split` 默认 docs，可选 engine；`top_k` 默认 5，范围 1–100。
成功结果的 `structuredContent` 包含：

- `collection`、`input`、`count`、`qdrant_time_seconds`。
- `matches`：按 Qdrant 排序返回 `rank`、`point_id`、`score`、完整 `payload`。
- `context`：含编号、标题、文件、URL、完整正文的 RAG 上下文。

payload 保留原始 `id`、`title`、`file_path`、`symbol`、`metadata`、`content` 等全部字段。
文本 content 也包含同一 JSON，兼容不读取 structuredContent 的调用者。
无命中返回空 matches/context；查询或 embedding 失败返回 `isError: true` 和 error 信息，
MCP 服务继续接收后续调用。参数类型/枚举不合法由 MCP 参数解析返回协议错误。

## 验证

```sh
cargo test -p rmpc_qdrant_example
cargo clippy -p rmpc_qdrant_example --all-targets -- -D warnings
# 需要已导入的本地或 QDRANT_URL 指定的 Qdrant：
cargo test -p rmpc_qdrant_example mcp_returns_real_qdrant_results -- --ignored
```

普通测试覆盖 embedding 校验和通过真实 MCP 通信返回参数错误。
忽略的集成测试验证 MCP → Qdrant → MCP 的实际查询结果。

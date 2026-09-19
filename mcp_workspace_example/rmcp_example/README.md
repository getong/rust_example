# rmcp MCP 示例

这个目录演示 `rmcp` 的功能和作用：

- `rmcp` 是 Model Context Protocol 的 Rust SDK。
- MCP 让 AI 客户端用统一协议发现并调用外部能力。
- Rust 侧可以把函数注册成 MCP tools，再通过 stdio、HTTP 等传输暴露给客户端。

运行自带演示：

```bash
cargo run
```

它会在同一个进程里启动 MCP server 和 MCP client，完成初始化、列出 tools、调用 tools，并打印结果。

运行真实 stdio MCP server：

```bash
cargo run -- serve
```

这个模式适合配置给支持 MCP 的客户端。示例 server 暴露两个工具：

- `mcp_overview`：说明 rmcp/MCP 的作用。
- `analyze_text`：统计输入文本的字符数、词数、行数，并返回结构化 JSON。

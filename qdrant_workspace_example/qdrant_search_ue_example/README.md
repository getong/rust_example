# UE58 RAG 搜索示例

在 `qdrant_workspace_example` 目录执行。只读取已导入的集合，不创建或删除数据。

## 无需模型服务的相似内容搜索

```sh
cargo run -p qdrant_search_ue_example
cargo run -p qdrant_search_ue_example -- engine point 0 5
```

默认从 `ue58_rag_docs` 的点 0（Programming with C++）复用已有向量，检索最近的 5 个其他点。
engine 示例从点 0（FIntMargin）查找相似源码。点 ID 是导入时的行号；原始 chunk ID 保留在 payload 中。
这验证向量检索链路，不会将任意文本转换成向量。

## 自然语言搜索

需要先启动 embedding 模型服务。**仅 export EMBEDDING_URL 不会启动服务**；
Qdrant 的 6333/6334 端口也不负责把文本转成向量。

### 终端 1：启动本地模型服务

安装好 `uv` 后，在 `qdrant_search_ue_example` 目录执行：

```sh
uv run --script embedding_server.py
```

脚本自动安装隔离的 Python 依赖，并在首次运行时从 Hugging Face 下载
`Qwen/Qwen3-Embedding-0.6B` 权重，需要网络、磁盘和模型运行内存。
默认使用 CPU，也可使用 `--device mps`（Apple Silicon）或 `--device cuda`。
等待输出 `Ready: http://127.0.0.1:8000/health`；模型加载前不能搜索。
这是绑定本机地址、串行处理请求的演示服务。查询使用模型自带的池化方式并归一化为 1024 维。
实现参考 [Qwen 官方模型说明](https://huggingface.co/Qwen/Qwen3-Embedding-0.6B)。

### 终端 2：检查服务并搜索

```sh
curl --noproxy '*' --fail http://127.0.0.1:8000/health
export EMBEDDING_URL=http://127.0.0.1:8000/v1/embeddings
export EMBEDDING_MODEL=Qwen/Qwen3-Embedding-0.6B
cargo run -p qdrant_search_ue_example -- docs text 'How do I use delegates in Unreal Engine?' 5
cargo run -p qdrant_search_ue_example -- engine text 'How does FIntMargin calculate total horizontal margin?' 5
```

也可以使用已有的 OpenAI 兼容 embedding 服务，修改 `EMBEDDING_URL` 为其完整端点，
必要时设置 `EMBEDDING_API_KEY`。必须加载相同模型并返回 1024 维向量。

### 连接错误排查

- `Connection refused`：服务没有启动、已退出，或端口配置不一致。
- HTTP 502：通常来自代理/网关，上游模型服务不可达。先直连检查 `/health`，再查看网关与模型日志。
- 本示例自动绕过 localhost、127.0.0.0/8、::1 的 HTTP 代理；远程端点保持正常代理行为。
- 8000 被其他程序占用时，可启动 `uv run --script embedding_server.py --port 8001`，并同步修改 `EMBEDDING_URL`。
- 模型下载失败时查看终端 1 的错误；不要把未就绪的地址当作可用服务。

查询默认添加 `Instruct: <任务描述>\nQuery: <问题>`。
可通过 `EMBEDDING_INSTRUCTION` 自定义任务描述；如果服务已自动加前缀，设置为空避免重复。
输入文本会发送到指定 embedding 服务；搜索返回的数据不会发送给该服务。
模型仅维度相同不够，必须使用与入库相同的模型和兼容的池化方式。

完整参数：`[docs|engine] [point|text] [ID|问题] [TOP_K]`。
`TOP_K` 范围为 1–100，默认 5；文本问题请使用引号。
`QDRANT_URL` 默认 `http://localhost:6334`，使用 gRPC 端口。
结果包含 Cosine 分数、标题、chunk ID、文件路径、符号、文档 URL 和前 600 个字符。
分数是相似度，不是正确率；导入未完成时只能检索已入库内容。

```sh
cargo test -p qdrant_search_ue_example
cargo clippy -p qdrant_search_ue_example --all-targets -- -D warnings
```

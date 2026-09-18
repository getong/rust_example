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

需要一个 OpenAI 兼容的 `/v1/embeddings` 服务，加载与入库相同的
`Qwen/Qwen3-Embedding-0.6B` 模型，返回 1024 维向量。
以下端口仅为示例，需要替换为你的服务地址；本程序不会启动或下载模型。

```sh
export EMBEDDING_URL=http://localhost:8000/v1/embeddings
export EMBEDDING_MODEL=Qwen/Qwen3-Embedding-0.6B
# 如服务要求鉴权，设置 EMBEDDING_API_KEY
cargo run -p qdrant_search_ue_example -- docs text 'How do I use delegates in Unreal Engine?' 5
cargo run -p qdrant_search_ue_example -- engine text 'How does FIntMargin calculate total horizontal margin?' 5
```

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

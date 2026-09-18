# 导入 Jatpeng/ue58-rag-embeddings

先运行 `./qdrant-docker.sh`，然后导入已下载的 Hugging Face 数据。

```sh
# 自动从 HF_HUB_CACHE / HF_HOME / ~/.cache/huggingface/hub 查找 main snapshot
# 默认导入 engine
cargo run --release -p qdrant_load_hf_file

# 指定 snapshot，导入 docs 和 engine
cargo run --release -p qdrant_load_hf_file -- \
  "$HOME/.cache/huggingface/hub/datasets--Jatpeng--ue58-rag-embeddings/snapshots/bfd4c56d05411ef079a0fab5974f9b5437288b5a" all

# 每个分区只导入前 501 条，验证完整批次和尾批次
cargo run -p qdrant_load_hf_file -- /path/to/snapshot all 501
```

参数：`[SNAPSHOT_DIR] [engine|docs|all] [LIMIT]`。
`QDRANT_URL` 可覆盖默认 gRPC 地址 `http://localhost:6334`。

- engine 写入 `ue58_rag_engine`，docs 写入 `ue58_rag_docs`。
- 读取 C-order float32 的 1024 维 NPY，使用 Cosine 距离；新集合将向量和 payload 存储在磁盘。
- 每批 500 条，流式读取向量和 JSONL，不将整个向量矩阵加载到内存。
- 校验 sidecar 行号和 chunk ID；payload 保留全部原始字段，包括 `id`、`content` 和 `metadata`。
- Qdrant 点 ID 使用从 0 开始的行号，原始 `chunk-…` ID 保留在 payload.id。
- 每批等待 Qdrant 确认写入后才增加进度。失败后可重跑同一 snapshot，已写入行会被覆盖。
- 行号仅在同一 snapshot 内稳定。更换数据集版本前应使用独立集合或显式处理旧集合；程序不会自动删除现有数据。LIMIT 不会删除之前导入的其他行。
- 本数据集向量使用 `Qwen/Qwen3-Embedding-0.6B` 生成，查询时应使用同一模型。

验证：`cargo test -p qdrant_load_hf_file` 和
`cargo clippy -p qdrant_load_hf_file --all-targets -- -D warnings`。

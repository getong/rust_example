# LanceDB Rust 示例

一个可离线运行的完整示例，演示 [LanceDB](https://github.com/lancedb/lancedb)
的核心能力，尤其是它在 AI 应用中的角色。

## LanceDB 是什么

LanceDB 是用 Rust 编写的**嵌入式（serverless）向量数据库**：

- **无需服务进程**：像 SQLite 一样，`connect("某个目录")` 就能用；同一套 API
  也支持 `s3://`、`gs://`、`az://` 对象存储。
- **底层是 Lance 列式格式**：专为 AI 工作负载设计的存储格式，随机访问比
  Parquet 快约 100 倍，数据可直接被 Python（pandas/polars/PyTorch DataLoader）
  零拷贝读取。
- **数据交换基于 Apache Arrow**：与整个数据/ML 生态无缝衔接。

## 它在 AI 方面的作用

| 能力 | AI 场景 |
|---|---|
| ANN 向量检索（IVF_PQ / HNSW） | RAG 检索、语义搜索、推荐、图片/音频相似搜索 |
| 向量 + SQL 元数据过滤 | 多租户/带权限的企业级 RAG |
| BM25 全文检索 + 混合检索（RRF） | 生产级 RAG 的标准两路召回 |
| 原始数据、张量、向量同表存储 | 多模态数据湖：图像字节和它的 embedding 放一行 |
| 自动版本化 / 时间旅行 | 训练数据集快照，实验可复现，误操作可回滚 |
| Embedding Registry | 开启 `openai`/`sentence-transformers` feature 后，写入和查询自动向量化 |

## 本示例演示的内容（`src/main.rs`）

1. 连接嵌入式数据库（一个本地目录）
2. 用 Arrow RecordBatch 建表：文本 + 元数据 + 向量同表
3. **语义检索**：自然语言问题 → 向量 → 最近邻文档（RAG 的核心一步）
4. **向量检索 + SQL 过滤**，并为标量列建 BTree 索引
5. **全文检索**：倒排索引 + BM25
6. **混合检索**：向量与全文两路召回，RRF 融合排序
7. 2000 条 128 维向量上建 **IVF_PQ 向量索引** 并用 `nprobes` 调节速度/召回
8. **版本化与时间旅行**：误删数据后 `checkout` 历史版本并 `restore` 回滚

示例用确定性的"词袋哈希"函数模拟 embedding 模型，所以完全离线可跑；
生产中把 `embed()` 换成真实模型（或用 LanceDB 的 embedding registry）即可。

## 运行

```bash
cargo run
```

数据会写入 `./data/ai_knowledge_base` 目录，每次运行前自动清空。

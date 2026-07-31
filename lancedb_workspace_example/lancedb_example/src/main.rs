//! LanceDB Rust 综合示例
//!
//! LanceDB 是一个用 Rust 编写的 **嵌入式(serverless)向量数据库**，底层是为 AI
//! 工作负载设计的 Lance 列式存储格式。它在 AI 应用中的典型角色：
//!
//! 1. **RAG 检索层**: 存储文档的 embedding 向量，通过近似最近邻(ANN)搜索 为 LLM
//!    找到语义相关的上下文；
//! 2. **语义搜索 / 推荐**: 向量相似度检索 + SQL 元数据过滤；
//! 3. **混合检索(Hybrid Search)**: 向量语义召回 + BM25 全文关键词召回， 用 RRF 融合排序，是生产级
//!    RAG 的标准做法；
//! 4. **多模态数据湖**: Lance 格式可以把原始图像/音频字节、张量和向量
//!    存在同一张表里（不像传统向量库只存向量）；
//! 5. **训练数据管理**: 数据自动版本化，支持时间旅行(time travel)， 保证模型训练/评测的可复现性。
//!
//! 本例完全离线可跑：用一个确定性的"词袋哈希" embedding 模拟向量化。
//! 生产环境中把 `embed()` 换成真实模型即可（LanceDB 内置 embedding registry，
//! 开启 `openai` / `sentence-transformers` feature 后可让表在写入时自动算向量）。

use std::sync::Arc;

use lancedb::arrow::arrow_array::{
  types::Float32Type, FixedSizeListArray, Float32Array, Int32Array, RecordBatch, StringArray,
};
use lancedb::arrow::arrow_schema::{DataType, Field, Schema};
use futures::TryStreamExt;
use lance_index::scalar::FullTextSearchQuery;
use lancedb::{
  connect,
  connection::Connection,
  index::{
    scalar::{BTreeIndexBuilder, FtsIndexBuilder},
    vector::IvfPqIndexBuilder,
    Index,
  },
  query::{ExecutableQuery, QueryBase, QueryExecutionOptions},
  DistanceType, Result, Table,
};

/// 向量维度。真实场景由 embedding 模型决定（如 OpenAI text-embedding-3-small 是 1536 维）
const DIM: usize = 64;

#[tokio::main]
async fn main() -> Result<()> {
  // 每次运行从干净状态开始
  if std::path::Path::new("data").exists() {
    std::fs::remove_dir_all("data").unwrap();
  }

  // ─── 1. 连接数据库 ───────────────────────────────────────────────
  // LanceDB 是嵌入式的：不需要启动任何服务进程，"连接"就是打开一个本地目录
  // （也支持 s3:// gs:// az:// 等对象存储 URI，云端规模同样 serverless）。
  let db = connect("data/ai_knowledge_base").execute().await?;
  println!(
    "已连接嵌入式数据库, 现有表: {:?}",
    db.table_names().execute().await?
  );

  // ─── 2. 建立 RAG 知识库表 (文本 + 元数据 + 向量同表存储) ─────────
  let tbl = create_knowledge_base(&db).await?;
  println!(
    "知识库 'documents' 建表完成, 共 {} 条文档",
    tbl.count_rows(None).await?
  );

  // ─── 3. 语义检索: RAG 的核心 ────────────────────────────────────
  // 把自然语言问题向量化，找语义最近的文档 —— 这些文档就是喂给 LLM 的上下文。
  let question = "how do neural networks learn deep representations";
  let hits = tbl
    .query()
    .nearest_to(embed(question))? // ANN 向量检索
    .distance_type(DistanceType::Cosine) // embedding 通常用余弦距离
    .limit(3)
    .execute()
    .await?
    .try_collect::<Vec<_>>()
    .await?;
  print_hits(&format!("语义检索: \"{question}\""), &hits, "_distance");

  // ─── 4. 向量检索 + SQL 元数据过滤 ───────────────────────────────
  // 生产 RAG 几乎都要按租户/权限/类目过滤。LanceDB 支持 SQL 谓词下推，
  // 还可以给标量列建 BTree 索引加速过滤。
  tbl
    .create_index(&["category"], Index::BTree(BTreeIndexBuilder::default()))
    .execute()
    .await?;
  let hits = tbl
    .query()
    .only_if("category = 'science'") // SQL 过滤条件
    .nearest_to(embed(question))?
    .distance_type(DistanceType::Cosine)
    .limit(3)
    .execute()
    .await?
    .try_collect::<Vec<_>>()
    .await?;
  print_hits("同一问题, 但限定 category = 'science'", &hits, "_distance");

  // ─── 5. 全文检索 (BM25) ─────────────────────────────────────────
  // 向量检索擅长"语义相近"，但对专有名词/精确关键词不敏感，
  // 所以 LanceDB 内置了倒排索引 + BM25 打分。
  tbl
    .create_index(&["text"], Index::FTS(FtsIndexBuilder::default()))
    .execute()
    .await?;
  let hits = tbl
    .query()
    .full_text_search(FullTextSearchQuery::new("speed of light".to_owned()))
    .limit(3)
    .execute()
    .await?
    .try_collect::<Vec<_>>()
    .await?;
  print_hits("全文检索(BM25): \"speed of light\"", &hits, "_score");

  // ─── 6. 混合检索: 向量 + 全文, RRF 融合 ─────────────────────────
  // 一次查询同时走两条召回路径，默认用 Reciprocal Rank Fusion 重排，
  // 兼顾语义泛化和关键词精确性 —— 生产级 RAG 的标准配置。
  let q = "machine learning";
  let hits = tbl
    .query()
    .full_text_search(FullTextSearchQuery::new(q.to_owned()))
    .nearest_to(embed(q))?
    .limit(3)
    .execute_hybrid(QueryExecutionOptions::default())
    .await?
    .try_collect::<Vec<_>>()
    .await?;
  print_hits(&format!("混合检索: \"{q}\""), &hits, "_relevance_score");

  // ─── 7. 向量索引 (IVF_PQ): 让百万/亿级向量毫秒响应 ──────────────
  // 小表可以暴力扫描, 数据大了就需要 ANN 索引。IVF_PQ = 倒排分区 + 乘积量化,
  // 在召回率、速度、内存之间取得平衡（这也是 LanceDB 磁盘友好的关键:
  // 索引按需从磁盘加载, 不必全量驻留内存）。
  demo_vector_index(&db).await?;

  // ─── 8. 数据版本化与时间旅行: AI 数据集的可复现性 ───────────────
  demo_versioning(&tbl).await?;

  println!("\n数据以 Lance 列式格式存放在 ./data 目录, 可直接被 pandas/polars/PyTorch 读取。");
  Ok(())
}

/// 一条知识库文档
struct Doc {
  category: &'static str,
  text: &'static str,
}

/// 建表：id + 原文 + 类目 + embedding 向量放在同一张表。
/// Lance 格式同样可以再加一列存原始图片字节/张量，实现多模态数据湖。
async fn create_knowledge_base(db: &Connection) -> Result<Table> {
  let docs = [
    Doc {
      category: "ai",
      text: "Artificial neural networks are inspired by the human brain.",
    },
    Doc {
      category: "ai",
      text: "Deep learning is a subset of machine learning that uses neural networks with many \
             layers.",
    },
    Doc {
      category: "ai",
      text: "Large language models learn representations of text by predicting the next token.",
    },
    Doc {
      category: "ai",
      text: "Retrieval augmented generation grounds a language model with documents from a vector \
             database.",
    },
    Doc {
      category: "ai",
      text: "The transformer architecture relies on the attention mechanism.",
    },
    Doc {
      category: "science",
      text: "The speed of light is approximately 299,792 kilometers per second.",
    },
    Doc {
      category: "science",
      text: "Photosynthesis is the process by which plants make their food.",
    },
    Doc {
      category: "science",
      text: "The human brain contains about 86 billion neurons that learn by strengthening \
             connections.",
    },
    Doc {
      category: "science",
      text: "Water boils at 100 degrees Celsius at sea level.",
    },
    Doc {
      category: "geo",
      text: "Mount Everest is the highest mountain in the world.",
    },
    Doc {
      category: "geo",
      text: "The Pacific Ocean is the largest ocean on Earth.",
    },
    Doc {
      category: "history",
      text: "Leonardo da Vinci painted the Mona Lisa.",
    },
  ];

  let schema = Arc::new(Schema::new(vec![
    Field::new("id", DataType::Int32, false),
    Field::new("category", DataType::Utf8, false),
    Field::new("text", DataType::Utf8, false),
    Field::new(
      "vector",
      DataType::FixedSizeList(
        Arc::new(Field::new("item", DataType::Float32, true)),
        DIM as i32,
      ),
      true,
    ),
  ]));

  let batch = RecordBatch::try_new(
    schema.clone(),
    vec![
      Arc::new(Int32Array::from_iter_values(0 .. docs.len() as i32)),
      Arc::new(StringArray::from_iter_values(
        docs.iter().map(|d| d.category),
      )),
      Arc::new(StringArray::from_iter_values(docs.iter().map(|d| d.text))),
      Arc::new(
        FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
          docs
            .iter()
            .map(|d| Some(embed(d.text).into_iter().map(Some).collect::<Vec<_>>())),
          DIM as i32,
        ),
      ),
    ],
  )?;

  // create_table 直接接受 Arrow RecordBatch (也接受 RecordBatchReader / Stream)
  db.create_table("documents", batch).execute().await
}

/// 大规模向量 + IVF_PQ 索引演示
async fn demo_vector_index(db: &Connection) -> Result<()> {
  const TOTAL: usize = 2000;
  const BIG_DIM: usize = 128;

  let schema = Arc::new(Schema::new(vec![
    Field::new("id", DataType::Int32, false),
    Field::new(
      "vector",
      DataType::FixedSizeList(
        Arc::new(Field::new("item", DataType::Float32, true)),
        BIG_DIM as i32,
      ),
      true,
    ),
  ]));

  // 确定性伪随机向量 (LCG)，模拟真实 embedding 分布
  let mut state = 42u64;
  let mut next = move || {
    state = state
      .wrapping_mul(6364136223846793005)
      .wrapping_add(1442695040888963407);
    ((state >> 40) as f32) / (1u64 << 24) as f32
  };
  let batch = RecordBatch::try_new(
    schema.clone(),
    vec![
      Arc::new(Int32Array::from_iter_values(0 .. TOTAL as i32)),
      Arc::new(
        FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
          (0 .. TOTAL).map(|_| Some((0 .. BIG_DIM).map(|_| Some(next())).collect::<Vec<_>>())),
          BIG_DIM as i32,
        ),
      ),
    ],
  )?;
  let tbl = db.create_table("embeddings_2k", batch).execute().await?;

  // IVF_PQ: 16 个分区, 每 8 维压缩为 1 字节码 (128 维 -> 16 字节)
  tbl
    .create_index(
      &["vector"],
      Index::IvfPq(
        IvfPqIndexBuilder::default()
          .num_partitions(16)
          .num_sub_vectors(16),
      ),
    )
    .execute()
    .await?;

  let query: Vec<f32> = (0 .. BIG_DIM).map(|i| i as f32 / BIG_DIM as f32).collect();
  let hits = tbl
    .query()
    .nearest_to(query)?
    .nprobes(4) // 只探测 16 个分区中的 4 个: 用少量召回率换速度
    .limit(3)
    .execute()
    .await?
    .try_collect::<Vec<_>>()
    .await?;
  let n: usize = hits.iter().map(|b| b.num_rows()).sum();
  println!(
    "\n== 向量索引 (IVF_PQ) ==\n  2000 条 128 维向量建索引后 ANN 查询返回 top-{n} (nprobes=4)"
  );
  Ok(())
}

/// 版本化与时间旅行：每次写操作自动产生新版本，可回看/回滚。
/// 对 AI 而言这意味着"数据集快照"——训练与评测可精确复现。
async fn demo_versioning(tbl: &Table) -> Result<()> {
  println!("\n== 数据版本化与时间旅行 ==");
  let v_before = tbl.version().await?;
  let rows_before = tbl.count_rows(None).await?;

  // 模拟数据清洗事故: 误删了一批文档
  tbl.delete("category = 'ai'").await?;
  println!(
    "  误删 ai 类文档后: {} -> {} 行",
    rows_before,
    tbl.count_rows(None).await?
  );

  // 时间旅行: 只读地回看历史版本
  tbl.checkout(v_before).await?;
  println!(
    "  checkout 到版本 {}: 仍能看到 {} 行",
    v_before,
    tbl.count_rows(None).await?
  );

  // 回滚: 把历史版本恢复为最新版本, 数据找回
  tbl.restore().await?;
  println!(
    "  restore 后恢复为 {} 行, 训练数据可精确复现",
    tbl.count_rows(None).await?
  );
  Ok(())
}

/// 模拟 embedding 模型：把文本映射为定长归一化向量。
///
/// 这里用"词袋 + FNV 哈希"实现，保证例子离线可跑且检索结果直观。
/// 真实应用中替换为模型推理，例如:
///   - LanceDB embedding registry (`openai` / `sentence-transformers` feature)，
///     注册后建表时声明源列，写入与查询自动向量化;
///   - 或自己调用任意推理服务得到 Vec<f32>。
fn embed(text: &str) -> Vec<f32> {
  let mut v = vec![0.0f32; DIM];
  for word in text
    .to_lowercase()
    .split(|c: char| !c.is_alphanumeric())
    .filter(|w| w.len() > 2)
  // 粗糙的停用词过滤
  {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in word.bytes() {
      h ^= b as u64;
      h = h.wrapping_mul(0x100000001b3);
    }
    v[(h % DIM as u64) as usize] += 1.0;
  }
  let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
  if norm > 0.0 {
    v.iter_mut().for_each(|x| *x /= norm);
  }
  v
}

/// 打印检索结果。score_col: "_distance"(越小越近) / "_score"(BM25, 越大越好)
/// / "_relevance_score"(混合检索融合分, 越大越好)
fn print_hits(title: &str, batches: &[RecordBatch], score_col: &str) {
  println!("\n== {title} ==");
  for batch in batches {
    let texts = batch
      .column_by_name("text")
      .unwrap()
      .as_any()
      .downcast_ref::<StringArray>()
      .unwrap();
    let cats = batch
      .column_by_name("category")
      .unwrap()
      .as_any()
      .downcast_ref::<StringArray>()
      .unwrap();
    let scores = batch
      .column_by_name(score_col)
      .unwrap()
      .as_any()
      .downcast_ref::<Float32Array>()
      .unwrap();
    for i in 0 .. batch.num_rows() {
      println!(
        "  [{score_col}={:.4}] ({}) {}",
        scores.value(i),
        cats.value(i),
        texts.value(i)
      );
    }
  }
}

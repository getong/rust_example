//! # LanceDB Rust 示例：嵌入式 AI 向量数据库
//!
//! LanceDB 是一个嵌入式、面向 AI 应用的向量数据库，把 Lance（列式数据格式）
//! 打包成一个可直接嵌入 Rust/Python/JS 进程的数据库。它主要解决三类问题：
//!
//! 1. **向量相似度检索（Vector Search）**：把文本、图片、音频等转成向量后，
//!    按“语义相似度”找最近邻。这是 RAG（检索增强生成）系统的核心检索步骤。
//! 2. **AI Embedding 流水线**：内置 Embedding 注册表，写入文本时自动调用
//!    嵌入模型生成向量，查询时也能自动把自然语言转成查询向量。
//! 3. **多路召回 + 重排（Hybrid Search）**：向量检索（语义）+ 全文检索 BM25
//!    （关键词）双路召回，再用 RRF 重排，兼顾语义和关键词，是 RAG 的标配。
//!
//! 此外它也支持常规数据库能力：建表、追加、更新、删除、过滤查询、二级索引等。
//!
//! 本示例演示（可直接 `cargo run` 运行）：
//!   1. 基础操作：建表 / 追加 / 查询 / 更新 / 删除
//!   2. 向量相似度检索 + 向量索引（IVF-Flat 近似最近邻）
//!   3. AI Embedding：自定义 EmbeddingFunction + 注册表，文本自动转向量
//!   4. 全文检索（BM25）+ 混合检索（向量 + 全文 + RRF 重排）

use std::{borrow::Cow, sync::Arc};

use lancedb::arrow::arrow_array::{
  Array, FixedSizeListArray, Float32Array, Float64Array, Int32Array, RecordBatch, StringArray,
  types::Float32Type,
};
use lancedb::arrow::arrow_schema::{DataType, Field, Schema};
use futures::TryStreamExt;
use lance_index::scalar::FullTextSearchQuery;
use lancedb::{
  Result, Table, connect,
  connection::Connection,
  embeddings::{EmbeddingDefinition, EmbeddingFunction},
  index::{Index, scalar::FtsIndexBuilder, vector::IvfFlatIndexBuilder},
  query::{ExecutableQuery, QueryBase, QueryExecutionOptions, Select},
};

/// 商品向量的维度
const VECTOR_DIM: usize = 16;
/// 文本嵌入向量的维度（本示例用 HashEmbedding，真实项目可换成预训练模型）
const EMBEDDING_DIM: usize = 64;

#[tokio::main]
async fn main() -> Result<()> {
  // 数据库可以是一个本地目录（嵌入式的意思就是：无需单独启动服务器）
  let uri = "data/lancedb-demo";
  if std::path::Path::new("data").exists() {
    std::fs::remove_dir_all("data").unwrap();
  }

  let db = connect(uri).execute().await?;
  println!("LanceDB 已连接（嵌入式数据库，本地目录: {uri}）\n");

  demo_basic(&db).await?;
  demo_vector_search(&db).await?;
  demo_embeddings(&db).await?;
  demo_fulltext_hybrid(&db).await?;
  demo_cleanup(&db).await?;
  Ok(())
}

// ---------------------------------------------------------------------------
// 1. 基础操作
// ---------------------------------------------------------------------------
async fn demo_basic(db: &Connection) -> Result<()> {
  println!("======================================");
  println!("1. 基础操作：建表 / 追加 / 查询 / 更新 / 删除");
  println!("======================================\n");

  // 建表：数据用 Arrow 的 RecordBatch 组织（LanceDB 底层是列式 Arrow 存储）
  let products = db
    .create_table("products", make_product_batch(0, 100)?)
    .execute()
    .await?;
  println!("[建表] 已创建 products 表（100 行）\n");

  // 追加数据
  products
    .add(make_product_batch(100, 100)?)
    .execute()
    .await?;
  println!(
    "[追加] 又追加 100 行，当前总行数 = {}\n",
    products.count_rows(None).await?
  );

  // 查询：SQL 过滤 + 列投影 + limit
  println!("[查询] 价格 < 25 的商品（只取 id/name/price 三列）:");
  let rows = products
    .query()
    .only_if("price < 25.0")
    .select(Select::columns(&["id", "name", "price"]))
    .limit(5)
    .execute()
    .await?
    .try_collect::<Vec<_>>()
    .await?;
  print_cols(&rows, &["id", "name", "price"]);

  // 更新
  let res = products
    .update()
    .column("price", "price * 0.9")
    .only_if("id % 10 == 0")
    .execute()
    .await?;
  println!(
    "\n[更新] 对 id 能被 10 整除的商品打 9 折，更新了 {} 行\n",
    res.rows_updated
  );

  // 删除
  let res = products.delete("id > 150").await?;
  println!(
    "[删除] 删除 id > 150 的行，共删除 {} 行\n",
    res.num_deleted_rows
  );

  // 列出所有表
  println!(
    "[列表] 当前数据库中的表: {:?}\n",
    db.table_names().execute().await?
  );
  Ok(())
}

// ---------------------------------------------------------------------------
// 2. 向量相似度检索（LanceDB 的核心能力）
// ---------------------------------------------------------------------------
async fn demo_vector_search(db: &Connection) -> Result<()> {
  println!("======================================");
  println!("2. 向量相似度检索（Vector Search）—— 语义搜索的核心");
  println!("======================================\n");

  let products = db.open_table("products").execute().await?;
  // 追加更多数据，方便后面演示索引（索引训练需要一定量的数据）
  products
    .add(make_product_batch(200, 2000)?)
    .execute()
    .await?;
  println!(
    "已追加 2000 行用于构建索引，当前行数 = {}\n",
    products.count_rows(None).await?
  );

  // 查询向量：这里用“簇 0 的中心”（所有维度都是 0.0）。
  // 我们生成数据时让 id%3==0 的行聚在簇 0，因此最近邻应该都是这一类。
  let query_vec: Vec<f32> = vec![0.0; VECTOR_DIM];

  println!("-- 精确搜索（无索引 = flat scan，逐行计算距离）: 与簇 0 最相似的 5 行 --");
  let rows = products
    .query()
    .nearest_to(query_vec.as_slice())?
    .limit(5)
    .execute()
    .await?
    .try_collect::<Vec<_>>()
    .await?;
  // _distance 是 L2 距离（越小越相似）
  print_cols(&rows, &["id", "name", "category", "_distance"]);

  println!("\n-- 向量搜索 + 过滤（先过滤再检索，只找 category = 'food' 的最近邻）--");
  let rows = products
    .query()
    .nearest_to(query_vec.as_slice())?
    .only_if("category = 'food'")
    .limit(3)
    .execute()
    .await?
    .try_collect::<Vec<_>>()
    .await?;
  print_cols(&rows, &["id", "name", "category", "_distance"]);

  println!("\n-- 创建 IVF-Flat 向量索引（近似最近邻 ANN）--");
  println!("   IVF 先把向量划分到若干个簇，检索时只搜最近的几个簇，");
  println!("   数据量越大提速越明显（百万级向量场景）。");
  products
    .create_index(
      &["vector"],
      Index::IvfFlat(IvfFlatIndexBuilder::default().num_partitions(4)),
    )
    .execute()
    .await?;
  println!("   索引创建完成\n");

  println!("-- 走索引的向量搜索（结果近似但更快）--");
  let rows = products
    .query()
    .nearest_to(query_vec.as_slice())?
    .limit(5)
    .execute()
    .await?
    .try_collect::<Vec<_>>()
    .await?;
  print_cols(&rows, &["id", "name", "category", "_distance"]);
  println!();
  Ok(())
}

// ---------------------------------------------------------------------------
// 3. AI Embedding：文本自动转成向量（RAG / LLM 应用的关键环节）
// ---------------------------------------------------------------------------
async fn demo_embeddings(db: &Connection) -> Result<()> {
  println!("======================================");
  println!("3. AI Embedding：把文本自动转成向量（RAG/LLM 的关键环节）");
  println!("======================================\n");

  // 3.1 定义一个“嵌入函数”。
  //     真实项目中这里换成 OpenAI 嵌入模型 / sentence-transformers /
  //     本地微调模型等；本示例用一个不依赖外部模型的哈希嵌入来演示流程。
  let embedding = Arc::new(HashEmbedding::new("hash-embedding", EMBEDDING_DIM));

  // 3.2 把嵌入函数注册到数据库连接的 EmbeddingRegistry。
  //     注册后，建表时通过 add_embedding 声明的列会被自动向量化。
  db.embedding_registry()
    .register(embedding.name(), embedding.clone())?;
  println!("已注册嵌入函数: {:?}\n", embedding.name());

  // 3.3 建表并声明“text 列需要向量化”，目标向量列名为 vector。
  //     写入时 LanceDB 会自动调用嵌入函数，不需要我们手动算向量。
  let articles = db
    .create_table("articles", make_articles_batch()?)
    .add_embedding(EmbeddingDefinition::new(
      "text",
      embedding.name(),
      Some("vector"),
    ))?
    .execute()
    .await?;
  println!("[自动向量化] articles 表已创建（25 行），text 列已自动生成向量列\n");

  // 3.4 追加数据：继续写纯文本，嵌入同样会被自动计算
  articles.add(make_articles_batch_extra()?).execute().await?;
  let schema = articles.schema().await?;
  let vec_field = schema.field_with_name("vector")?;
  println!(
    "[追加] 追加了新文章，向量列类型 = {:?}\n",
    vec_field.data_type()
  );

  // 3.5 用自然语言做“语义检索”：先嵌入查询文本，再向量检索。
  //     这就是 RAG 中“检索”阶段做的事：query -> embedding -> top-k 文档。
  let query = "How many bones are in the human body?";
  let q = Arc::new(StringArray::from_iter_values([query.to_string()]));
  let qv = embedding.compute_query_embeddings(q)?;
  println!("[语义检索] 问题: \"{}\"\n最相似的 3 条知识:", query);
  let rows = articles
    .query()
    .nearest_to(qv)?
    .limit(3)
    .execute()
    .await?
    .try_collect::<Vec<_>>()
    .await?;
  print_cols(&rows, &["text", "_distance"]);
  println!();
  Ok(())
}

// ---------------------------------------------------------------------------
// 4. 全文检索（BM25） + 混合检索（Vector + FTS + RRF 重排）
// ---------------------------------------------------------------------------
async fn demo_fulltext_hybrid(db: &Connection) -> Result<()> {
  println!("======================================");
  println!("4. 全文检索（BM25） + 混合检索（Vector + FTS + RRF 重排）");
  println!("======================================\n");

  let articles: Table = db.open_table("articles").execute().await?;

  // 4.1 在 text 列上创建全文索引（BM25 倒排索引，按关键词相关度打分）
  articles
    .create_index(&["text"], Index::FTS(FtsIndexBuilder::default()))
    .execute()
    .await?;
  println!("已创建全文索引（BM25）\n");

  // 4.2 纯全文检索：适合精确关键词匹配
  println!("-- 全文检索: \"world records\" --");
  let rows = articles
    .query()
    .full_text_search(FullTextSearchQuery::new("world records".to_string()))
    .limit(5)
    .execute()
    .await?
    .try_collect::<Vec<_>>()
    .await?;
  print_cols(&rows, &["text"]);

  // 4.3 混合检索：向量（语义）+ 全文（关键词）双路召回，RRF 重排合并。
  //     RAG 系统常用这种策略：向量检索抓“意思相近但用词不同”的，
  //     全文检索抓“关键词完全命中”的，再统一打分。
  let query = "How many bones are in the human body?";
  let q = Arc::new(StringArray::from_iter_values([query.to_string()]));
  // 从注册表取出之前注册的嵌入函数，计算查询向量
  let embedding = db
    .embedding_registry()
    .get("hash-embedding")
    .expect("hash-embedding should be registered");
  let qv = embedding.compute_query_embeddings(q)?;

  let mut results = articles
    .query()
    .full_text_search(FullTextSearchQuery::new(query.to_string()))
    .nearest_to(qv)?
    .limit(5)
    .execute_hybrid(QueryExecutionOptions::default())
    .await?;

  println!("-- 混合检索（RRF 重排）: \"{}\" --", query);
  while let Some(batch) = results.try_next().await? {
    let texts = batch
      .column_by_name("text")
      .unwrap()
      .as_any()
      .downcast_ref::<StringArray>()
      .unwrap();
    let scores = batch
      .column_by_name("_relevance_score")
      .map(|c| c.as_any().downcast_ref::<Float32Array>().unwrap().clone());
    for (i, t) in texts.iter().enumerate() {
      match &scores {
        Some(s) => println!("  [{:.4}] {}", s.value(i), t.unwrap()),
        None => println!("  {}", t.unwrap()),
      }
    }
  }
  println!();
  Ok(())
}

// ---------------------------------------------------------------------------
// 5. 清理
// ---------------------------------------------------------------------------
async fn demo_cleanup(db: &Connection) -> Result<()> {
  println!("======================================");
  println!("5. 清理：删除表");
  println!("======================================\n");
  for t in db.table_names().execute().await? {
    db.drop_table(&t, &[]).await?;
    println!("已删除表: {t}");
  }
  println!("\n数据库表: {:?}", db.table_names().execute().await?);
  Ok(())
}

// ===========================================================================
// 数据构造辅助函数
// ===========================================================================

/// 生成一批商品数据（含 id / name / category / price / vector）
fn make_product_batch(start: i32, count: i32) -> Result<RecordBatch> {
  let schema = Arc::new(Schema::new(vec![
    Field::new("id", DataType::Int32, false),
    Field::new("name", DataType::Utf8, true),
    Field::new("category", DataType::Utf8, true),
    Field::new("price", DataType::Float64, true),
    Field::new(
      "vector",
      DataType::FixedSizeList(
        Arc::new(Field::new("item", DataType::Float32, true)),
        VECTOR_DIM as i32,
      ),
      true,
    ),
  ]));

  let ids: Vec<i32> = (start .. start + count).collect();
  let names = ids
    .iter()
    .map(|id| format!("product-{id:03}"))
    .collect::<Vec<String>>();
  let categories = ids
    .iter()
    .map(|id| match id % 3 {
      0 => "book",
      1 => "food",
      _ => "toy",
    })
    .collect::<Vec<&str>>();
  let prices: Vec<f64> = ids.iter().map(|id| 10.0 + (id % 97) as f64 * 0.5).collect();
  let vectors: Vec<Option<Vec<Option<f32>>>> = ids
    .iter()
    .map(|id| Some(vector_for_id(*id).into_iter().map(Some).collect()))
    .collect();

  Ok(RecordBatch::try_new(
    schema,
    vec![
      Arc::new(Int32Array::from(ids)),
      Arc::new(StringArray::from_iter_values(names)),
      Arc::new(StringArray::from_iter_values(categories)),
      Arc::new(Float64Array::from(prices)),
      Arc::new(
        FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(vectors, VECTOR_DIM as i32),
      ),
    ],
  )?)
}

/// 生成商品向量：按 id%3 聚成 3 个簇，簇内距离很近
fn vector_for_id(id: i32) -> Vec<f32> {
  let cluster = (id % 3) as f32;
  (0 .. VECTOR_DIM)
    .map(|i| cluster + 0.01 * (id as f32 / 7.0).sin() + 0.01 * (i as f32 * 1.7).sin())
    .collect()
}

/// 一批 AI 知识事实（25 条）
fn make_articles_batch() -> Result<RecordBatch> {
  let facts = vec![
    "Albert Einstein was a theoretical physicist.",
    "The capital of France is Paris.",
    "The Great Wall of China is one of the Seven Wonders of the World.",
    "Python is a popular programming language.",
    "Mount Everest is the highest mountain in the world.",
    "Leonardo da Vinci painted the Mona Lisa.",
    "Shakespeare wrote Hamlet.",
    "The human body has 206 bones.",
    "The speed of light is approximately 299,792 kilometers per second.",
    "Water boils at 100 degrees Celsius.",
    "The Earth orbits the Sun.",
    "The Pyramids of Giza are located in Egypt.",
    "Coffee is one of the most popular beverages in the world.",
    "Tokyo is the capital city of Japan.",
    "Photosynthesis is the process by which plants make their food.",
    "The Pacific Ocean is the largest ocean on Earth.",
    "Mozart was a prolific composer of classical music.",
    "The Internet is a global network of computers.",
    "Basketball is a sport played with a ball and a hoop.",
    "The first computer virus was created in 1983.",
    "Artificial neural networks are inspired by the human brain.",
    "Deep learning is a subset of machine learning.",
    "IBM's Watson won Jeopardy! in 2011.",
    "The first computer programmer was Ada Lovelace.",
    "The first chatbot was ELIZA, created in the 1960s.",
  ];
  make_text_batch(facts)
}

/// 追加的一批文章（演示“追加时自动向量化”）
fn make_articles_batch_extra() -> Result<RecordBatch> {
  let facts = vec![
    "Rust is a memory-safe systems programming language.",
    "LanceDB is an embedded vector database built for AI applications.",
    "Retrieval augmented generation combines search with large language models.",
  ];
  make_text_batch(facts)
}

/// 构造只有 text 列的 RecordBatch（向量列由 add_embedding 自动生成）
fn make_text_batch(facts: Vec<&str>) -> Result<RecordBatch> {
  let schema = Arc::new(Schema::new(vec![Field::new("text", DataType::Utf8, true)]));
  Ok(RecordBatch::try_new(
    schema,
    vec![Arc::new(StringArray::from_iter_values(facts))],
  )?)
}

// ===========================================================================
// 自定义 EmbeddingFunction（模拟 AI 嵌入模型）
// ===========================================================================

/// 一个不依赖外部模型的“哈希嵌入”：把词哈希到固定维度。
///
/// 原理：文本 -> 拆词 -> 每个词 FNV 哈希到 [0, dim) 的桶里计数 -> L2 归一化。
/// 效果：包含相似词汇的文本，向量彼此更接近，可用来演示向量检索的“语义”。
///
/// 真实项目请换成预训练模型：
///   - 本地：`SentenceTransformersEmbeddings`（lancedb 的 sentence-transformers feature）
///   - 云端：`OpenAIEmbedding` / 自定义 HTTP 调用第三方 embedding API
#[derive(Debug)]
struct HashEmbedding {
  name: String,
  dim: usize,
}

impl HashEmbedding {
  fn new(name: &str, dim: usize) -> Self {
    Self {
      name: name.to_string(),
      dim,
    }
  }

  fn embed_text(&self, text: &str) -> Vec<f32> {
    let mut vec = vec![0.0f32; self.dim];
    let clean: String = text
      .chars()
      .map(|c| {
        if c.is_alphanumeric() {
          c.to_ascii_lowercase()
        } else {
          ' '
        }
      })
      .collect();
    for word in clean.split_whitespace() {
      vec[fnv1a(word) % self.dim] += 1.0;
    }
    let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
      for v in &mut vec {
        *v /= norm;
      }
    }
    vec
  }
}

/// FNV-1a 哈希：简单、快速、字符串分布均匀
fn fnv1a(s: &str) -> usize {
  let mut h: u64 = 0xcbf2_9ce4_8422_2325;
  for b in s.bytes() {
    h ^= b as u64;
    h = h.wrapping_mul(0x0000_0100_0000_01b3);
  }
  h as usize
}

impl EmbeddingFunction for HashEmbedding {
  fn name(&self) -> &str {
    &self.name
  }

  /// 输入类型：文本
  fn source_type(&self) -> Result<Cow<'_, DataType>> {
    Ok(Cow::Owned(DataType::Utf8))
  }

  /// 输出类型：定长浮点向量列
  fn dest_type(&self) -> Result<Cow<'_, DataType>> {
    Ok(Cow::Owned(DataType::new_fixed_size_list(
      DataType::Float32,
      self.dim as i32,
      true,
    )))
  }

  /// 写入数据时调用：文本数组 -> FixedSizeList 向量数组
  fn compute_source_embeddings(&self, source: Arc<dyn Array>) -> Result<Arc<dyn Array>> {
    let input = source
      .as_any()
      .downcast_ref::<StringArray>()
      .ok_or_else(|| lancedb::Error::InvalidInput {
        message: "expected Utf8 (StringArray) input".to_string(),
      })?;
    let values: Vec<f32> = input
      .iter()
      .flatten()
      .flat_map(|t| self.embed_text(t))
      .collect();
    let values = Arc::new(Float32Array::from(values));
    let field = Arc::new(Field::new("item", DataType::Float32, true));
    Ok(Arc::new(FixedSizeListArray::new(
      field,
      self.dim as i32,
      values,
      None,
    )))
  }

  /// 查询时调用：文本 -> 扁平 Float32Array（与 sentence-transformers 的返回格式一致）
  fn compute_query_embeddings(&self, input: Arc<dyn Array>) -> Result<Arc<dyn Array>> {
    let fsl = self.compute_source_embeddings(input)?;
    let fsl = fsl
      .as_any()
      .downcast_ref::<FixedSizeListArray>()
      .ok_or_else(|| lancedb::Error::InvalidInput {
        message: "internal error: expected FixedSizeList".to_string(),
      })?;
    Ok(fsl.values().clone())
  }
}

// ===========================================================================
// 打印辅助
// ===========================================================================

/// 按列名打印一批 RecordBatch（跳过向量这类大列）
fn print_cols(batches: &[RecordBatch], cols: &[&str]) {
  for batch in batches {
    for row in 0 .. batch.num_rows() {
      let cells = cols
        .iter()
        .map(|c| {
          let col = batch.column_by_name(c).unwrap();
          format_cell(col, row)
        })
        .collect::<Vec<_>>()
        .join(" | ");
      println!("  {cells}");
    }
  }
}

/// 把单个单元格格式化成字符串
fn format_cell(col: &Arc<dyn Array>, row: usize) -> String {
  match col.data_type() {
    DataType::Utf8 | DataType::LargeUtf8 => col
      .as_any()
      .downcast_ref::<StringArray>()
      .map(|a| a.value(row).to_string())
      .unwrap_or_default(),
    DataType::Int32 => col
      .as_any()
      .downcast_ref::<Int32Array>()
      .map(|a| a.value(row).to_string())
      .unwrap_or_default(),
    DataType::Float32 => col
      .as_any()
      .downcast_ref::<Float32Array>()
      .map(|a| format!("{:.4}", a.value(row)))
      .unwrap_or_default(),
    DataType::Float64 => col
      .as_any()
      .downcast_ref::<Float64Array>()
      .map(|a| format!("{:.2}", a.value(row)))
      .unwrap_or_default(),
    other => format!("<{other}>"),
  }
}

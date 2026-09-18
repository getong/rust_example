use std::{
  env,
  error::Error,
  fs::{self, File},
  io::{BufRead, BufReader},
  path::{Path, PathBuf},
};

use indicatif::{ProgressBar, ProgressStyle};
use npyz::{NpyFile, Order};
use qdrant_client::{
  Qdrant,
  qdrant::{
    CreateCollectionBuilder, Distance, PointStruct, UpsertPointsBuilder, VectorParamsBuilder,
    vectors_config,
  },
};
use serde::Deserialize;
use serde_json::{Map, Value};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const BATCH_SIZE: usize = 500;

#[derive(Deserialize)]
struct IdRecord {
  row: u64,
  id: String,
}

fn default_dataset() -> Result<PathBuf> {
  let hub = if let Some(path) = env::var_os("HF_HUB_CACHE") {
    PathBuf::from(path)
  } else if let Some(path) = env::var_os("HF_HOME") {
    PathBuf::from(path).join("hub")
  } else {
    PathBuf::from(env::var_os("HOME").ok_or("HOME 未设置，请指定数据集目录")?)
      .join(".cache/huggingface/hub")
  };
  let repo = hub.join("datasets--Jatpeng--ue58-rag-embeddings");
  let revision = fs::read_to_string(repo.join("refs/main"))
    .map_err(|e| format!("无法读取 Hugging Face 缓存，请指定 snapshot 目录: {e}"))?;
  Ok(repo.join("snapshots").join(revision.trim()))
}

fn reader(path: &Path) -> Result<BufReader<File>> {
  Ok(BufReader::new(
    File::open(path).map_err(|e| format!("{}: {e}", path.display()))?,
  ))
}

fn payload_for_row(chunk: &str, id: &str, row: u64) -> Result<Map<String, Value>> {
  let payload: Map<String, Value> = serde_json::from_str(chunk)?;
  let record: IdRecord = serde_json::from_str(id)?;
  if record.row != row || payload.get("id").and_then(Value::as_str) != Some(&record.id) {
    return Err(format!("第 {row} 行的 chunk ID / sidecar row 不匹配，停止导入").into());
  }
  Ok(payload)
}

async fn import(client: &Qdrant, root: &Path, split: &str, limit: Option<u64>) -> Result<()> {
  let dir = root.join(split);
  let mut chunks = reader(&dir.join("chunks.jsonl"))?.lines();
  let mut ids = reader(&dir.join("embeddings.npy.ids.jsonl"))?.lines();
  let npy = NpyFile::new(reader(&dir.join("embeddings.npy"))?)?;
  let (total, dim) = match npy.shape() {
    &[rows, dim] if rows > 0 && dim == 1024 => (rows, dim),
    shape => return Err(format!("不支持的向量形状: {shape:?}，需要 [rows, 1024]").into()),
  };
  if npy.order() != Order::C {
    return Err("向量必须按 C / row-major 顺序存储".into());
  }
  // NpyReader 按元素读取，仅保存当前批次，不加载整个 6.5 GiB 矩阵。
  let mut vectors = npy.data::<f32>()?;
  let collection = format!("ue58_rag_{split}");
  if client.collection_exists(&collection).await? {
    let info = client.collection_info(&collection).await?;
    let config = info
      .result
      .and_then(|i| i.config)
      .and_then(|c| c.params)
      .and_then(|p| p.vectors_config)
      .and_then(|v| v.config);
    match config {
      Some(vectors_config::Config::Params(p))
        if p.size == dim && p.distance == Distance::Cosine as i32 => {}
      _ => return Err(format!("集合 {collection} 必须使用 {dim} 维无名称 Cosine 向量").into()),
    }
  } else {
    client
      .create_collection(
        CreateCollectionBuilder::new(&collection)
          .vectors_config(VectorParamsBuilder::new(dim, Distance::Cosine).on_disk(true))
          .on_disk_payload(true),
      )
      .await?;
  }

  let count = limit.unwrap_or(total).min(total);
  println!("{split}: {total} 条 / {dim} 维，本次导入 {count} 条 → {collection}");
  let pb = ProgressBar::new(count);
  pb.set_style(
    ProgressStyle::with_template(
      "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
    )?
    .progress_chars("#>-"),
  );
  let mut batch = Vec::with_capacity(BATCH_SIZE);
  for row in 0 .. count {
    let chunk = chunks.next().ok_or("chunks.jsonl 行数不足")??;
    let id = ids.next().ok_or("ids.jsonl 行数不足")??;
    let payload =
      payload_for_row(&chunk, &id, row).map_err(|e| format!("{split} 第 {row} 行: {e}"))?;
    let vector: Vec<f32> = vectors
      .by_ref()
      .take(dim as usize)
      .collect::<std::io::Result<_>>()?;
    if vector.len() != dim as usize || vector.iter().any(|v| !v.is_finite()) {
      return Err(format!("{split} 第 {row} 行向量截断或包含非有限值").into());
    }
    // 每个分区独立集合；固定 snapshot 重跑使用相同行号，upsert 不产生重复点。
    batch.push(PointStruct::new(row, vector, payload));
    if batch.len() == BATCH_SIZE || row + 1 == count {
      let written = batch.len() as u64;
      client
        .upsert_points(
          UpsertPointsBuilder::new(
            &collection,
            std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE)),
          )
          .wait(true),
        )
        .await
        .map_err(|e| format!("{collection} 写入至第 {row} 行失败，可重跑: {e}"))?;
      pb.inc(written);
    }
  }
  if count == total && (chunks.next().transpose()?.is_some() || ids.next().transpose()?.is_some()) {
    return Err("JSONL 行数超过向量行数，请检查数据是否来自同一构建".into());
  }
  pb.finish_with_message("导入完成");
  println!("{collection}: 已确认写入 {count} 条");
  Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
  let args: Vec<String> = env::args().skip(1).collect();
  if args.iter().any(|a| a == "--help" || a == "-h") {
    println!("用法: qdrant_load_hf_file [SNAPSHOT_DIR] [engine|docs|all] [LIMIT]\n默认: Hugging Face 缓存 / engine / 全部\nQDRANT_URL 默认 http://localhost:6334 (gRPC)\nLIMIT 用于小批量验证；固定 snapshot 重跑会覆盖相同行号。");
    return Ok(());
  }
  if args.len() > 3 {
    return Err("参数过多，使用 --help 查看用法".into());
  }
  let root = match args.first() {
    Some(path) => PathBuf::from(path),
    None => default_dataset()?,
  };
  let split = args.get(1).map(String::as_str).unwrap_or("engine");
  if !matches!(split, "engine" | "docs" | "all") {
    return Err("分区必须为 engine、docs 或 all".into());
  }
  let limit = args.get(2).map(|s| s.parse::<u64>()).transpose()?;
  if limit == Some(0) {
    return Err("LIMIT 必须大于 0".into());
  }
  println!("数据目录: {}", root.display());
  let url = env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".into());
  let client = Qdrant::from_url(&url)
    .timeout(std::time::Duration::from_secs(120))
    .build()?;
  for part in if split == "all" {
    vec!["docs", "engine"]
  } else {
    vec![split]
  } {
    import(&client, &root, part, limit).await?;
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn preserves_chunk_id_and_nested_payload() {
    let payload = payload_for_row(
      r#"{"id":"chunk-abc","metadata":{"line":42}}"#,
      r#"{"row":0,"id":"chunk-abc"}"#,
      0,
    )
    .unwrap();
    assert_eq!(payload["id"], "chunk-abc");
    assert_eq!(payload["metadata"]["line"], 42);
  }

  #[test]
  fn rejects_misaligned_ids_and_rows() {
    let chunk = r#"{"id":"chunk-abc"}"#;
    assert!(payload_for_row(chunk, r#"{"row":0,"id":"chunk-other"}"#, 0).is_err());
    assert!(payload_for_row(chunk, r#"{"row":1,"id":"chunk-abc"}"#, 0).is_err());
    assert!(payload_for_row("null", r#"{"row":0,"id":"chunk-abc"}"#, 0).is_err());
  }
}

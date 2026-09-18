use std::{
  fs::File,
  io::{BufRead, BufReader},
  sync::Arc,
};

use indicatif::{ProgressBar, ProgressStyle};
use ndarray::Array2;
use npyz::NpyFile;
use qdrant_client::{
  Qdrant,
  qdrant::{
    CreateCollectionBuilder, Distance, PointId, PointStruct, UpsertPointsBuilder,
    VectorParamsBuilder,
  },
};
use serde_json::Value;

const COLLECTION_NAME: &str = "ue58_rag_engine";
const VECTOR_DIM: u64 = 1024;
const BATCH_SIZE: usize = 500; // 每批次上传 500 条数据

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // 1. 初始化 Qdrant 客户端
  let client = Arc::new(Qdrant::from_url("http://localhost:6334").build()?);

  // 2. 创建集合（如果不存在）
  if !client.collection_exists(COLLECTION_NAME).await? {
    client
      .create_collection(
        CreateCollectionBuilder::new(COLLECTION_NAME)
          .vectors_config(VectorParamsBuilder::new(VECTOR_DIM, Distance::Cosine)),
      )
      .await?;
    println!("成功创建 Qdrant 集合: {}", COLLECTION_NAME);
  }

  // 3. 打开数据文件句柄（流式读取，避免 OOM）
  println!("正在准备数据流...");
  let chunks_file = File::open("engine/chunks.jsonl")?;
  let mut chunks_reader = BufReader::new(chunks_file).lines();

  let ids_file = File::open("engine/embeddings.npy.ids.jsonl")?;
  let mut ids_reader = BufReader::new(ids_file).lines();

  // 读取 npy 向量文件
  let npy_file = File::open("engine/embeddings.npy")?;
  let npy_reader = NpyFile::new(BufReader::new(npy_file))?;

  // 获取向量总数（官方为 1,693,105）
  let total_rows = npy_reader.shape()[0] as usize;
  let npy_flat_data = npy_reader.into_vec::<f32>()?;
  // 将一维扁平数组转换为 2D 矩阵 [total_rows, 1024]
  let vectors_matrix = Array2::from_shape_vec((total_rows, VECTOR_DIM as usize), npy_flat_data)?;

  // 4. 设置进度条
  let pb = ProgressBar::new(total_rows as u64);
  pb.set_style(
    ProgressStyle::default_bar()
      .template(
        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan}/. Griffiths] {pos}/{len} ({eta})",
      )?
      .progress_chars("#>-"),
  );

  // 5. 循环分批次读取并上传
  let mut batch_points = Vec::with_capacity(BATCH_SIZE);

  for idx in 0 .. total_rows {
    // 读取对应的文本块（Payload）
    let chunk_line = chunks_reader.next().ok_or("chunks.jsonl 文件行数不足")??;
    let payload: Value = serde_json::from_str(&chunk_line)?;

    // 读取对应的 ID 映射
    let id_line = ids_reader.next().ok_or("ids.jsonl 文件行数不足")??;
    let id_json: Value = serde_json::from_str(&id_line)?;

    // 解析出 ID（支持转为 u64 整数或 UUID 字符串）
    // ⚠️ 转换逻辑：如果原始文件里是字符串型 UUID，则直接用字符串；如果是纯数字，转为 u64
    let point_id: PointId = match id_json["id"].as_u64() {
      Some(num) => num.into(),
      None => id_json["id"]
        .as_str()
        .unwrap_or(&idx.to_string())
        .to_string()
        .into(),
    };

    // 从内存矩阵中提取当前行的 1024 维向量
    let vector: Vec<f32> = vectors_matrix.row(idx).to_vec();

    // 构造 PointStruct
    let point = PointStruct::new(
      point_id,
      vector,
      payload.as_object().unwrap().clone(), // 将 JSON 对象作为 Payload
    );
    batch_points.push(point);

    // 当达到 BATCH_SIZE 时，执行批量上传
    if batch_points.len() == BATCH_SIZE || idx == total_rows - 1 {
      let client_clone = Arc::clone(&client);
      let points_to_upload = std::mem::replace(&mut batch_points, Vec::with_capacity(BATCH_SIZE));

      // 执行 Upsert 操作
      client_clone.upsert_points(
        UpsertPointsBuilder::new(COLLECTION_NAME, points_to_upload)
          .wait(false) // 设为 false 表示异步写入，大幅提升导入速度
      ).await?;

      pb.inc(BATCH_SIZE as u64);
    }
  }

  pb.finish_with_message("🎉 所有数据已成功导入 Qdrant！");
  Ok(())
}

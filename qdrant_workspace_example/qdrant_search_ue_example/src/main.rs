use std::{env, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use qdrant_client::{
  Qdrant,
  qdrant::{Query, QueryPointsBuilder, ScoredPoint},
};
use serde_json::{Value, json};

const HELP: &str = r#"UE58 RAG 搜索（只读）
用法:
  qdrant_search_ue_example [docs|engine] [point|text] [ID|问题] [TOP_K]
示例:
  qdrant_search_ue_example
  qdrant_search_ue_example engine point 0 5
  qdrant_search_ue_example docs text 'How do I use delegates in Unreal Engine?' 5
默认: docs point 0 5
环境变量:
  QDRANT_URL       gRPC 地址，默认 http://localhost:6334
  EMBEDDING_URL    文本搜索必需，OpenAI 兼容 embeddings 完整端点
  EMBEDDING_MODEL  默认 Qwen/Qwen3-Embedding-0.6B（必须与入库模型一致）
  EMBEDDING_API_KEY 可选
  EMBEDDING_INSTRUCTION 可选，默认 UE 文档/源码检索任务描述
point 模式复用已有点向量检索邻居，无需模型服务；不代表自由文本搜索。"#;

fn embedding_from_response(value: &Value) -> Result<Vec<f32>> {
  let values = value
    .pointer("/data/0/embedding")
    .and_then(Value::as_array)
    .context("embedding 响应缺少 data[0].embedding 数组")?;
  ensure!(
    values.len() == 1024,
    "embedding 维度必须为 1024，实际 {}",
    values.len()
  );
  let vector: Vec<f32> = values
    .iter()
    .map(|v| {
      let n = v.as_f64().context("embedding 包含非数值")? as f32;
      ensure!(n.is_finite(), "embedding 包含非有限值");
      Ok(n)
    })
    .collect::<Result<_>>()?;
  ensure!(vector.iter().any(|&v| v != 0.0), "embedding 不能为零向量");
  Ok(vector)
}

async fn embed(text: &str) -> Result<Vec<f32>> {
  ensure!(!text.trim().is_empty(), "搜索问题不能为空");
  let url =
    env::var("EMBEDDING_URL").context("文本搜索需要 EMBEDDING_URL；可先运行默认 point 示例")?;
  let model = env::var("EMBEDDING_MODEL").unwrap_or_else(|_| "Qwen/Qwen3-Embedding-0.6B".into());
  let instruction = env::var("EMBEDDING_INSTRUCTION").unwrap_or_else(|_| {
    "Given an Unreal Engine question, retrieve relevant documentation and source code.".into()
  });
  let input = if instruction.is_empty() {
    text.to_owned()
  } else {
    format!("Instruct: {instruction}\nQuery: {text}")
  };
  let http = reqwest::Client::builder()
    .timeout(Duration::from_secs(120))
    .build()?;
  let mut request = http
    .post(url)
    .json(&json!({"model":model,"input":input,"encoding_format":"float"}));
  if let Ok(key) = env::var("EMBEDDING_API_KEY") {
    request = request.bearer_auth(key);
  }
  let response = request
    .send()
    .await
    .context("调用 embedding 服务失败")?
    .error_for_status()
    .context("embedding 服务返回错误状态")?;
  embedding_from_response(&response.json::<Value>().await?)
}

fn field(point: &ScoredPoint, name: &str) -> String {
  point
    .payload
    .get(name)
    .map(|v| v.clone().into_json())
    .and_then(|v| v.as_str().map(str::to_owned))
    .unwrap_or_default()
}

#[tokio::main]
async fn main() -> Result<()> {
  let args: Vec<String> = env::args().skip(1).collect();
  if args.iter().any(|a| a == "--help" || a == "-h") {
    println!("{HELP}");
    return Ok(());
  }
  ensure!(args.len() <= 4, "参数过多，请使用 --help");
  let split = args.first().map(String::as_str).unwrap_or("docs");
  ensure!(
    matches!(split, "docs" | "engine"),
    "分区必须是 docs 或 engine"
  );
  let mode = args.get(1).map(String::as_str).unwrap_or("point");
  let top_k = args
    .get(3)
    .map(|s| s.parse::<u64>())
    .transpose()?
    .unwrap_or(5);
  ensure!((1 ..= 100).contains(&top_k), "TOP_K 必须为 1..100");
  let query = match mode {
    "point" => {
      let id = args
        .get(2)
        .map(|s| s.parse::<u64>())
        .transpose()?
        .unwrap_or(0);
      println!("以 {split} 的点 {id} 为种子，搜索相似内容（排除种子自身）");
      Query::new_nearest(id)
    }
    "text" => {
      let text = args.get(2).context("text 模式需要用引号括起来的问题")?;
      println!("搜索问题: {text}");
      Query::new_nearest(embed(text).await?)
    }
    _ => bail!("搜索模式必须是 point 或 text"),
  };
  let url = env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".into());
  let client = Qdrant::from_url(&url)
    .timeout(Duration::from_secs(120))
    .build()?;
  let collection = format!("ue58_rag_{split}");
  let response = client
    .query(
      QueryPointsBuilder::new(&collection)
        .query(query)
        .limit(top_k)
        .with_payload(true),
    )
    .await
    .with_context(|| format!("搜索 {collection} 失败，请确认集合和种子点已导入"))?;
  println!(
    "集合: {collection}，命中 {} 条，Qdrant 耗时 {:.3}s",
    response.result.len(),
    response.time
  );
  for (i, point) in response.result.iter().enumerate() {
    println!(
      "\n{}. score={:.4}  {}",
      i + 1,
      point.score,
      field(point, "title")
    );
    println!("   chunk: {}", field(point, "id"));
    println!("   file: {}", field(point, "file_path"));
    let symbol = field(point, "symbol");
    if !symbol.is_empty() {
      println!("   symbol: {symbol}");
    }
    if let Some(metadata) = point.payload.get("metadata") {
      let metadata = metadata.clone().into_json();
      if let Some(url) = metadata.get("url").and_then(Value::as_str) {
        println!("   url: {url}");
      }
    }
    let content = field(point, "content");
    let excerpt: String = content.chars().take(600).collect();
    println!(
      "{excerpt}{}",
      if content.chars().count() > 600 {
        "…"
      } else {
        ""
      }
    );
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn validates_embedding_service_response() {
    assert_eq!(
      embedding_from_response(&json!({"data":[{"embedding":vec![0.5;1024]}]}))
        .unwrap()
        .len(),
      1024
    );
    assert!(embedding_from_response(&json!({"data":[{"embedding":[1.0,2.0]}]})).is_err());
    assert!(embedding_from_response(&json!({"data":[{"embedding":vec![0.0;1024]}]})).is_err());
    assert!(embedding_from_response(&json!({"error":"unavailable"})).is_err());
    let mut invalid = vec![json!(0.5); 1024];
    invalid[0] = json!("bad");
    assert!(embedding_from_response(&json!({"data":[{"embedding":invalid}]})).is_err());
  }
}

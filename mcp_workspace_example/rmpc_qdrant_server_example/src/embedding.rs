use std::{env, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};

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

fn is_loopback(url: &reqwest::Url) -> bool {
  let host = url.host_str().unwrap_or_default();
  host.eq_ignore_ascii_case("localhost")
    || host
      .trim_matches(['[', ']'])
      .parse::<std::net::IpAddr>()
      .is_ok_and(|ip| ip.is_loopback())
}

pub(crate) async fn embed(text: &str) -> Result<Vec<f32>> {
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
  let url = reqwest::Url::parse(&url).context("EMBEDDING_URL 不是有效 URL")?;
  ensure!(
    matches!(url.scheme(), "http" | "https"),
    "EMBEDDING_URL 必须使用 http 或 https"
  );
  let mut builder = reqwest::Client::builder()
    .connect_timeout(Duration::from_secs(5))
    .timeout(Duration::from_secs(120));
  if is_loopback(&url) {
    builder = builder.no_proxy();
  }
  let http = builder.build()?;
  let mut request = http
    .post(url)
    .json(&json!({"model":model,"input":input,"encoding_format":"float"}));
  if let Ok(key) = env::var("EMBEDDING_API_KEY") {
    request = request.bearer_auth(key);
  }
  let response = request.send().await.context(
    "无法连接 embedding 服务；设置 EMBEDDING_URL 不会启动服务。请先在 qdrant_search_ue_example \
     目录运行 uv run --script embedding_server.py，等待模型加载完成，再检查 /health",
  )?;
  let status = response.status();
  if !status.is_success() {
    let body = response.text().await.unwrap_or_default();
    let excerpt: String = body.chars().take(1000).collect();
    bail!(
      "embedding 服务返回 HTTP {status}。检查服务日志和 /health；502 \
       通常表示网关无法连接上游模型服务。响应: {excerpt}"
    );
  }
  embedding_from_response(
    &response
      .json::<Value>()
      .await
      .context("embedding 服务未返回有效 JSON")?,
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn bypasses_proxy_only_for_loopback() {
    for host in ["localhost", "127.0.0.1", "127.0.0.2", "[::1]"] {
      assert!(is_loopback(
        &reqwest::Url::parse(&format!("http://{host}:8000/v1/embeddings")).unwrap()
      ));
    }
    for host in ["example.com", "localhost.example.com", "192.168.1.1"] {
      assert!(!is_loopback(
        &reqwest::Url::parse(&format!("https://{host}/v1/embeddings")).unwrap()
      ));
    }
  }

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

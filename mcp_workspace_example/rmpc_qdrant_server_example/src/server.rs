use std::time::Duration;

use anyhow::{Context, Result, ensure};
use qdrant_client::{
  Qdrant,
  qdrant::{Query, QueryPointsBuilder, ScoredPoint, point_id::PointIdOptions},
};
use rmcp::{
  ServerHandler,
  handler::server::{router::tool::ToolRouter, wrapper::Parameters},
  model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
  schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Split {
  #[default]
  Docs,
  Engine,
}

impl Split {
  fn collection(self) -> &'static str {
    match self {
      Self::Docs => "ue58_rag_docs",
      Self::Engine => "ue58_rag_engine",
    }
  }
}

fn default_limit() -> u64 {
  5
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct TextRequest {
  #[schemars(description = "Natural-language question about Unreal Engine")]
  query: String,
  #[serde(default)]
  split: Split,
  #[serde(default = "default_limit")]
  #[schemars(range(min = 1, max = 100))]
  top_k: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct PointRequest {
  #[schemars(
    description = "Existing numeric Qdrant point ID (import row number, not payload chunk ID)"
  )]
  point_id: u64,
  #[serde(default)]
  split: Split,
  #[serde(default = "default_limit")]
  #[schemars(range(min = 1, max = 100))]
  top_k: u64,
}

#[derive(Clone)]
pub(crate) struct RagServer {
  qdrant: Qdrant,
  tool_router: ToolRouter<Self>,
}

impl RagServer {
  pub(crate) fn new() -> Result<Self> {
    let url = std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6334".into());
    let mut builder = Qdrant::from_url(&url).timeout(Duration::from_secs(120));
    if let Ok(key) = std::env::var("QDRANT_API_KEY") {
      builder = builder.api_key(key);
    }
    Ok(Self {
      qdrant: builder.build()?,
      tool_router: Self::tool_router(),
    })
  }

  async fn retrieve(&self, split: Split, query: Query, top_k: u64, input: Value) -> Result<Value> {
    let collection = split.collection();
    let response = self
      .qdrant
      .query(
        QueryPointsBuilder::new(collection)
          .query(query)
          .limit(top_k)
          .with_payload(true),
      )
      .await
      .with_context(|| {
        format!("搜索 {collection} 失败，请确认 Qdrant 已启动、集合和种子点已导入")
      })?;
    let matches: Vec<Value> = response
      .result
      .into_iter()
      .enumerate()
      .map(|(i, point)| hit(i + 1, point))
      .collect();
    let context = matches
      .iter()
      .map(|item| {
        format!(
          "[{}] {}\nfile: {}\nurl: {}\n{}",
          item["rank"],
          item["payload"]["title"].as_str().unwrap_or_default(),
          item["payload"]["file_path"].as_str().unwrap_or_default(),
          item["payload"]["metadata"]["url"]
            .as_str()
            .unwrap_or_default(),
          item["payload"]["content"].as_str().unwrap_or_default(),
        )
      })
      .collect::<Vec<_>>()
      .join("\n\n");
    Ok(
      json!({"collection": collection, "input": input, "count": matches.len(),
            "qdrant_time_seconds": response.time, "matches": matches, "context": context}),
    )
  }
}

fn hit(rank: usize, point: ScoredPoint) -> Value {
  let id = match point.id.and_then(|id| id.point_id_options) {
    Some(PointIdOptions::Num(id)) => json!(id),
    Some(PointIdOptions::Uuid(id)) => json!(id),
    None => Value::Null,
  };
  let payload: serde_json::Map<String, Value> = point
    .payload
    .into_iter()
    .map(|(key, value)| (key, value.into_json()))
    .collect();
  json!({"rank": rank, "point_id": id, "score": point.score, "payload": payload})
}

fn validate_limit(top_k: u64) -> Result<()> {
  ensure!((1 ..= 100).contains(&top_k), "top_k 必须为 1..100");
  Ok(())
}

fn tool_result(result: Result<Value>) -> CallToolResult {
  match result {
    Ok(value) => CallToolResult::structured(value),
    Err(error) => CallToolResult::structured_error(json!({"error": format!("{error:#}")})),
  }
}

#[tool_router]
impl RagServer {
  #[tool(
    name = "search_ue",
    description = "Search Unreal Engine docs or source by natural-language question. Returns \
                   ranked full payloads and RAG context. Requires the configured embedding \
                   service."
  )]
  async fn search_ue(&self, Parameters(request): Parameters<TextRequest>) -> CallToolResult {
    tool_result(
      async {
        validate_limit(request.top_k)?;
        let vector = crate::embedding::embed(&request.query).await?;
        self
          .retrieve(
            request.split,
            Query::new_nearest(vector),
            request.top_k,
            json!({"mode": "text", "query": request.query}),
          )
          .await
      }
      .await,
    )
  }

  #[tool(
    name = "similar_ue",
    description = "Find content similar to an existing numeric Qdrant point ID, excluding the \
                   seed itself. Reuses its vector; no embedding service needed."
  )]
  async fn similar_ue(&self, Parameters(request): Parameters<PointRequest>) -> CallToolResult {
    tool_result(
      async {
        validate_limit(request.top_k)?;
        self
          .retrieve(
            request.split,
            Query::new_nearest(request.point_id),
            request.top_k,
            json!({"mode": "point", "point_id": request.point_id}),
          )
          .await
      }
      .await,
    )
  }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RagServer {
  fn get_info(&self) -> ServerConfig {
    ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
      .with_server_info(Implementation::new(
        "ue-qdrant-rag",
        env!("CARGO_PKG_VERSION"),
      ))
      .with_instructions(
        "使用 search_ue 检索问题，使用 similar_ue 检索已有点的相似内容。结果含完整 payload 和 \
         context，可用于引用来源并生成回答；相似度不是正确率。",
      )
  }
}

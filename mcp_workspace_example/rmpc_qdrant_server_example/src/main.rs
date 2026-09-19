mod embedding;
mod server;

use anyhow::{Context, Result, bail, ensure};
use rmcp::{
  ServiceExt,
  model::{CallToolRequestParams, CallToolResult, ClientConfig},
  transport::stdio,
};
use serde_json::{Value, json};
use server::RagServer;

const HELP: &str = r#"MCP + Qdrant UE RAG
用法（在 workspace 目录）:
  cargo run -p rmpc_qdrant_example -- serve
  cargo run -p rmpc_qdrant_example -- demo [docs|engine] [point|text] [ID|问题] [TOP_K]
默认运行: demo docs point 0 5
serve 使用 stdio，stdout 仅输出 MCP 消息；日志输出 stderr。
demo 启动内存传输的 MCP client/server，发现工具并通过 MCP 查询真实 Qdrant。
环境变量: QDRANT_URL (默认 http://localhost:6334), QDRANT_API_KEY (可选),
EMBEDDING_URL (文本查询必需), EMBEDDING_MODEL, EMBEDDING_API_KEY, EMBEDDING_INSTRUCTION。
"#;

#[tokio::main]
async fn main() -> Result<()> {
  let args: Vec<String> = std::env::args().skip(1).collect();
  match args.first().map(String::as_str) {
    Some("help" | "-h" | "--help") => println!("{HELP}"),
    Some("serve") => {
      ensure!(args.len() == 1, "serve 不接受额外参数");
      eprintln!("UE Qdrant MCP server running on stdio");
      RagServer::new()?.serve(stdio()).await?.waiting().await?;
    }
    None | Some("demo") => {
      let (tool, arguments) = demo_arguments(args.get(1 ..).unwrap_or_default())?;
      let result = call_demo(RagServer::new()?, tool, arguments).await?;
      if let Some(value) = &result.structured_content {
        println!("{}", serde_json::to_string_pretty(value)?);
      } else {
        for item in &result.content {
          if let Some(text) = item.as_text() {
            println!("{}", text.text);
          }
        }
      }
      ensure!(
        result.is_error != Some(true),
        "MCP 工具查询失败，详见返回结果"
      );
    }
    Some(other) => bail!("未知命令 {other}，使用 --help 查看用法"),
  }
  Ok(())
}

fn demo_arguments(args: &[String]) -> Result<(&'static str, Value)> {
  ensure!(args.len() <= 4, "demo 参数过多");
  let split = args.first().map(String::as_str).unwrap_or("docs");
  ensure!(
    matches!(split, "docs" | "engine"),
    "split 必须是 docs 或 engine"
  );
  let top_k = args
    .get(3)
    .map(|s| s.parse::<u64>())
    .transpose()?
    .unwrap_or(5);
  ensure!((1 ..= 100).contains(&top_k), "top_k 必须为 1..100");
  match args.get(1).map(String::as_str).unwrap_or("point") {
    "point" => Ok((
      "similar_ue",
      json!({"split": split, "top_k": top_k,
            "point_id": args.get(2).map(|s| s.parse::<u64>()).transpose()?.unwrap_or(0)}),
    )),
    "text" => Ok((
      "search_ue",
      json!({"split": split, "top_k": top_k,
            "query": args.get(2).context("text 模式需要问题")?}),
    )),
    _ => bail!("mode 必须是 point 或 text"),
  }
}

async fn call_demo(server: RagServer, tool: &str, arguments: Value) -> Result<CallToolResult> {
  let (server_transport, client_transport) = tokio::io::duplex(65536);
  let server_task = tokio::spawn(async move {
    server.serve(server_transport).await?.waiting().await?;
    Ok::<_, anyhow::Error>(())
  });
  let outcome = async {
    let client = ClientConfig::default().serve(client_transport).await?;
    let result = async {
      let tools = client.peer().list_all_tools().await?;
      ensure!(
        tools.iter().any(|item| item.name == tool),
        "未发现工具 {tool}"
      );
      eprintln!(
        "MCP tools: {}",
        tools
          .iter()
          .map(|t| t.name.as_ref())
          .collect::<Vec<_>>()
          .join(", ")
      );
      Ok::<_, anyhow::Error>(
        client
          .peer()
          .call_tool(
            CallToolRequestParams::new(tool.to_owned())
              .with_arguments(serde_json::from_value(arguments)?),
          )
          .await?,
      )
    }
    .await;
    let cancel = client.cancel().await;
    let result = result?;
    cancel?;
    Ok(result)
  }
  .await;
  // Always terminate the demo server, including initialization/call failures.
  server_task.abort();
  let _ = server_task.await;
  outcome
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn mcp_discovers_tools_and_returns_validation_errors() {
    for (tool, arguments, message) in [
      ("search_ue", json!({"query": "  "}), "不能为空"),
      ("similar_ue", json!({"point_id": 0, "top_k": 101}), "top_k"),
    ] {
      let result = call_demo(RagServer::new().unwrap(), tool, arguments)
        .await
        .unwrap();
      assert_eq!(result.is_error, Some(true));
      assert!(
        result.structured_content.unwrap()["error"]
          .as_str()
          .unwrap()
          .contains(message)
      );
    }
  }

  #[tokio::test]
  #[ignore = "requires populated Qdrant ue58_rag_docs collection"]
  async fn mcp_returns_real_qdrant_results() {
    let result = call_demo(
      RagServer::new().unwrap(),
      "similar_ue",
      json!({"point_id": 0, "top_k": 2}),
    )
    .await
    .unwrap();
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let value = result.structured_content.unwrap();
    assert_eq!(value["collection"], "ue58_rag_docs");
    let hits = value["matches"].as_array().unwrap();
    assert!(!hits.is_empty());
    assert!(hits.len() <= 2);
    assert!(
      hits
        .iter()
        .all(|hit| hit["point_id"] != 0 && hit["score"].is_number())
    );
    assert!(!value["context"].as_str().unwrap().is_empty());
  }
}

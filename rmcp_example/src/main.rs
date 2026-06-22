use rmcp::{
  ServerHandler, ServiceExt,
  handler::server::{router::tool::ToolRouter, wrapper::Parameters},
  model::{
    CallToolRequestParams, ClientInfo, Content, Implementation, ServerCapabilities, ServerInfo,
  },
  schemars, tool, tool_handler, tool_router,
  transport::stdio,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct AnalyzeTextRequest {
  #[schemars(description = "Text that should be analyzed by the MCP tool")]
  text: String,
}

#[derive(Debug, Clone)]
struct RmcpDemoServer {
  tool_router: ToolRouter<Self>,
}

impl RmcpDemoServer {
  fn new() -> Self {
    Self {
      tool_router: Self::tool_router(),
    }
  }
}

impl Default for RmcpDemoServer {
  fn default() -> Self {
    Self::new()
  }
}

#[tool_router]
impl RmcpDemoServer {
  #[tool(
    name = "mcp_overview",
    description = "Explain what rmcp does and how MCP lets an AI client use external tools"
  )]
  fn mcp_overview(&self) -> String {
    [
      "rmcp is the Rust SDK for the Model Context Protocol.",
      "MCP standardizes how an AI client discovers and calls external capabilities.",
      "A server exposes tools, resources, and prompts; a client lists them and invokes them \
       through JSON-RPC messages.",
      "This example exposes two tools: mcp_overview and analyze_text.",
    ]
    .join("\n")
  }

  #[tool(
    name = "analyze_text",
    description = "Count characters, words, lines, and return a short preview for supplied text"
  )]
  fn analyze_text(
    &self,
    Parameters(AnalyzeTextRequest { text }): Parameters<AnalyzeTextRequest>,
  ) -> rmcp::model::CallToolResult {
    let characters = text.chars().count();
    let words = text.split_whitespace().count();
    let lines = text.lines().count();
    let preview: String = text.chars().take(40).collect();
    let structured = json!({
        "summary": format!("characters={characters}, words={words}, lines={lines}"),
        "characters": characters,
        "words": words,
        "lines": lines,
        "preview": preview,
    });

    rmcp::model::CallToolResult::structured(structured)
  }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RmcpDemoServer {
  fn get_info(&self) -> ServerInfo {
    let mut implementation = Implementation::new("rmcp-example-server", env!("CARGO_PKG_VERSION"));
    implementation.title = Some("rmcp 功能演示".to_string());

    ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
      .with_server_info(implementation)
      .with_instructions("演示 rmcp 如何把 Rust 函数发布成 MCP tools，供 AI client 发现和调用。")
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  match std::env::args().nth(1).as_deref() {
    Some("serve") => serve_stdio().await,
    Some("help" | "-h" | "--help") => {
      print_help();
      Ok(())
    }
    Some(other) => {
      eprintln!("unknown command: {other}");
      print_help();
      Ok(())
    }
    None => run_in_process_demo().await,
  }
}

async fn serve_stdio() -> Result<(), Box<dyn std::error::Error>> {
  eprintln!("rmcp demo server is running on stdio");
  RmcpDemoServer::new()
    .serve(stdio())
    .await?
    .waiting()
    .await?;
  Ok(())
}

async fn run_in_process_demo() -> Result<(), Box<dyn std::error::Error>> {
  println!("rmcp / MCP in-process demo\n");
  println!("1. Start a Rust MCP server that exposes tools.");
  println!("2. Start an MCP client over an in-memory duplex transport.");
  println!("3. Let the client discover tools and call one of them.\n");

  let (server_transport, client_transport) = tokio::io::duplex(4096);

  let server_task = tokio::spawn(async move {
    match RmcpDemoServer::new().serve(server_transport).await {
      Ok(server) => {
        if let Err(error) = server.waiting().await {
          eprintln!("server task failed: {error}");
        }
      }
      Err(error) => eprintln!("server initialization failed: {error}"),
    }
  });

  let client = ClientInfo::default().serve(client_transport).await?;

  let tools = client.peer().list_all_tools().await?;
  println!("discovered tools:");
  for tool in &tools {
    println!(
      "- {}: {}",
      tool.name,
      tool.description.as_deref().unwrap_or("no description")
    );
  }

  let overview = client
    .peer()
    .call_tool(CallToolRequestParams::new("mcp_overview"))
    .await?;
  println!("\nmcp_overview result:");
  print_tool_text(&overview.content);

  let arguments = serde_json::from_value(json!({
      "text": "rmcp turns Rust functions into MCP tools.\nAI clients can discover and call them."
  }))?;
  let analysis = client
    .peer()
    .call_tool(CallToolRequestParams::new("analyze_text").with_arguments(arguments))
    .await?;
  println!("\nanalyze_text result:");
  print_tool_text(&analysis.content);
  if let Some(value) = analysis.structured_content {
    println!("structured JSON: {}", serde_json::to_string_pretty(&value)?);
  }

  client.cancel().await?;
  server_task.await?;
  Ok(())
}

fn print_tool_text(content: &[Content]) {
  for item in content {
    if let Some(text) = item.as_text() {
      println!("{}", text.text);
    }
  }
}

fn print_help() {
  println!("Usage:");
  println!("  cargo run          # run the self-contained MCP client/server demo");
  println!("  cargo run -- serve # run an MCP server over stdio for external clients");
}

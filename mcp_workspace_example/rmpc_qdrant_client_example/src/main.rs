use std::{path::PathBuf, process::Stdio, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use rmcp::{
  ServiceExt,
  model::{CallToolRequestParams, ClientConfig},
};
use serde_json::{Value, json};
use tokio::{process::Command, time::timeout};

const HELP: &str = r#"Qdrant RAG MCP client（stdio 子进程）
用法:
  rmpc_qdrant_client_example [tools]
  rmpc_qdrant_client_example text [docs|engine] '问题' [TOP_K]
  rmpc_qdrant_client_example point [docs|engine] POINT_ID [TOP_K]
默认列出 server 工具。TOP_K 默认 5，范围 1–100。
先构建 server 和 client：
  cargo build -p rmpc_qdrant_example -p rmpc_qdrant_client_example
默认启动与 client 二进制同目录的 rmpc_qdrant_example serve。
可通过 MCP_SERVER_BIN 指定 server 可执行文件的路径。
server 继承 QDRANT_URL、QDRANT_API_KEY、EMBEDDING_URL 等环境变量。
stdout 输出 JSON，stderr 输出日志；协议错误或工具执行失败返回非零退出码。
"#;

#[derive(Debug)]
enum Action {
  Help,
  Tools,
  Call {
    name: &'static str,
    arguments: Value,
  },
}

fn parse_args(args: &[String]) -> Result<Action> {
  let Some(command) = args.first().map(String::as_str) else {
    return Ok(Action::Tools);
  };
  match command {
    "help" | "--help" | "-h" => {
      ensure!(args.len() == 1, "help 不接受额外参数");
      Ok(Action::Help)
    }
    "tools" => {
      ensure!(args.len() == 1, "tools 不接受额外参数");
      Ok(Action::Tools)
    }
    "text" | "point" => {
      ensure!(
        (3 ..= 4).contains(&args.len()),
        "用法: {command} [docs|engine] 查询值 [TOP_K]"
      );
      let split = &args[1];
      ensure!(
        matches!(split.as_str(), "docs" | "engine"),
        "split 必须为 docs 或 engine"
      );
      let top_k = args
        .get(3)
        .map(|value| value.parse::<u64>())
        .transpose()
        .context("TOP_K 必须是整数")?
        .unwrap_or(5);
      ensure!((1 ..= 100).contains(&top_k), "TOP_K 必须为 1..100");
      let mut arguments = json!({"split": split, "top_k": top_k});
      let name = if command == "text" {
        arguments["query"] = json!(args[2]);
        "search_ue"
      } else {
        arguments["point_id"] = json!(args[2].parse::<u64>().context("POINT_ID 必须是非负整数")?);
        "similar_ue"
      };
      Ok(Action::Call { name, arguments })
    }
    _ => bail!("未知命令 {command}，使用 --help 查看用法"),
  }
}

fn server_binary() -> Result<PathBuf> {
  if let Some(path) = std::env::var_os("MCP_SERVER_BIN") {
    ensure!(!path.is_empty(), "MCP_SERVER_BIN 不能为空");
    return Ok(path.into());
  }
  let executable = std::env::current_exe().context("无法确定 client 可执行文件路径")?;
  let directory = executable.parent().context("无法确定 client 所在目录")?;
  Ok(directory.join(format!(
    "rmpc_qdrant_example{}",
    std::env::consts::EXE_SUFFIX
  )))
}

#[tokio::main]
async fn main() -> Result<()> {
  let action = parse_args(&std::env::args().skip(1).collect::<Vec<_>>())?;
  if matches!(action, Action::Help) {
    println!("{HELP}");
    return Ok(());
  }
  let binary = server_binary()?;
  let mut command = Command::new(&binary);
  command
    .arg("serve")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .kill_on_drop(true);
  let mut child = command.spawn().with_context(|| {
    format!(
      "无法启动 MCP server {}；请先 cargo build -p rmpc_qdrant_example，或设置 MCP_SERVER_BIN",
      binary.display()
    )
  })?;
  let outcome = async {
    let stdout = child.stdout.take().context("server stdout 未创建")?;
    let stdin = child.stdin.take().context("server stdin 未创建")?;
    let client = timeout(
      Duration::from_secs(15),
      ClientConfig::default().serve((stdout, stdin)),
    )
    .await
    .context("MCP 初始化超时（15 秒）")?
    .context("MCP 初始化失败")?;
    let outcome = timeout(Duration::from_secs(300), async {
      let tools = client
        .peer()
        .list_all_tools()
        .await
        .context("获取 MCP tools 失败")?;
      match action {
        Action::Tools => println!(
          "{}",
          serde_json::to_string_pretty(&json!({"tools": tools}))?
        ),
        Action::Call { name, arguments } => {
          ensure!(
            tools.iter().any(|tool| tool.name == name),
            "server 未提供工具 {name}"
          );
          let result = client
            .peer()
            .call_tool(
              CallToolRequestParams::new(name).with_arguments(serde_json::from_value(arguments)?),
            )
            .await
            .context("MCP tools/call 失败")?;
          // Preserve content, structuredContent and isError for downstream callers.
          println!("{}", serde_json::to_string_pretty(&result)?);
          ensure!(
            result.is_error != Some(true),
            "MCP 工具 {name} 执行失败，详见返回 JSON"
          );
        }
        Action::Help => unreachable!("help handled before connecting"),
      }
      Ok::<_, anyhow::Error>(())
    })
    .await
    .context("MCP 请求超时（300 秒）")
    .and_then(|result| result);
    // Close the MCP transport even when the tool fails.
    let shutdown = client.cancel().await.context("关闭 MCP 连接失败");
    outcome?;
    shutdown?;
    Ok::<_, anyhow::Error>(())
  }
  .await;
  // Reap the child on every path, including failed MCP initialization.
  let cleanup = async {
    match timeout(Duration::from_secs(5), child.wait()).await {
      Ok(status) => {
        status.context("等待 server 退出失败")?;
      }
      Err(_) => {
        child.kill().await.context("终止 server 失败")?;
      }
    }
    Ok::<_, anyhow::Error>(())
  }
  .await;
  outcome?;
  cleanup?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
  }
  #[test]
  fn maps_cli_queries_to_server_tools() {
    let Action::Call { name, arguments } =
      parse_args(&args(&["text", "docs", "中文问题"])).unwrap()
    else {
      panic!("expected call")
    };
    assert_eq!(name, "search_ue");
    assert_eq!(
      arguments,
      json!({"query":"中文问题", "split":"docs", "top_k":5})
    );
    let Action::Call { name, arguments } =
      parse_args(&args(&["point", "engine", "42", "2"])).unwrap()
    else {
      panic!("expected call")
    };
    assert_eq!(name, "similar_ue");
    assert_eq!(
      arguments,
      json!({"point_id":42, "split":"engine", "top_k":2})
    );
  }
  #[test]
  fn rejects_invalid_arguments_before_starting_server() {
    for values in [
      vec!["text"],
      vec!["tools", "extra"],
      vec!["point", "invalid", "0"],
      vec!["point", "docs", "-1"],
      vec!["point", "docs", "0", "0"],
      vec!["text", "docs", "question", "101"],
      vec!["unknown"],
    ] {
      assert!(parse_args(&args(&values)).is_err(), "{values:?}");
    }
    assert!(matches!(parse_args(&[]).unwrap(), Action::Tools));
  }
}

//! Based on docs/upstream/embedded-database.mdx; queries and checks added locally.
use std::{error::Error, path::PathBuf};

use helix_db::{Client, HelixDbSource, HelixError, dsl::prelude::*};
use serde::Deserialize;
use serde_json::Value;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn disk_source(root: PathBuf) -> HelixDbSource {
  HelixDbSource::Disk {
    root,
    database: "embedded-demo".to_string(),
  }
}

fn create_user() -> QueryRequest {
  QueryRequest::write(
    write_batch()
      .var_as(
        "created",
        g().add_n(
          "EmbeddedDemoUser",
          vec![("name", PropertyInput::from("Ada"))],
        ),
      )
      .returning(["created"]),
  )
}

async fn count_users(client: &Client) -> Result<u64> {
  #[derive(Deserialize)]
  struct Counts {
    users: u64,
  }
  let result: Counts = client
    .query(QueryRequest::read(
      read_batch()
        .var_as("users", g().n_with_label("EmbeddedDemoUser").count())
        .returning(["users"]),
    ))
    .send()
    .await?;
  Ok(result.users)
}

async fn insert_and_check(client: &Client) -> Result<u64> {
  let before = count_users(client).await?;
  let created: Value = client.query(create_user()).send().await?;
  println!("新增节点：{}", serde_json::to_string_pretty(&created)?);
  let after = count_users(client).await?;
  if after != before + 1 {
    return Err(format!("节点数量不符合预期：{before} -> {after}").into());
  }
  println!("用户数量：{before} -> {after}");
  Ok(after)
}

async fn memory_demo() -> Result<()> {
  let client = Client::open(HelixDbSource::InMemory {
    database: "embedded-memory-demo".to_string(),
  })
  .await?;
  // Always close the handle, including when a query fails.
  let result = insert_and_check(&client).await;
  let closed = client.close().await;
  result?;
  closed?;
  println!("内存案例完成；数据不跨进程保留。");
  Ok(())
}

async fn disk_demo(root: PathBuf) -> Result<()> {
  // The local object store requires an existing filesystem root.
  std::fs::create_dir_all(&root)?;
  let source = disk_source(root);
  let writer = Client::open(source.clone()).await?;
  let result = insert_and_check(&writer).await;
  let closed = writer.close().await;
  let expected = result?;
  closed?;
  // Flush/close before reopening so the check exercises persisted data.
  let reopened = Client::open(source).await?;
  let result = count_users(&reopened).await;
  let closed = reopened.close().await;
  let actual = result?;
  closed?;
  if actual != expected {
    return Err(format!("持久化验证失败：预期 {expected}，实际 {actual}").into());
  }
  println!("关闭后重新打开，用户数量仍为 {actual}，持久化验证通过。");
  Ok(())
}

async fn check_reader(reader: &Client) -> Result<()> {
  let before = count_users(reader).await?;
  println!("只读查询，用户数量：{before}");
  match reader.query::<Value>(create_user()).send().await {
    Err(HelixError::EmbeddedError { code, details }) if code == "writer_mode_required" => {
      if count_users(reader).await? != before {
        return Err("拒绝写入后节点数量发生变化".into());
      }
      println!("只读句柄已拒绝写入：{details}");
      Ok(())
    }
    Err(error) => Err(error.into()),
    Ok(_) => Err("只读句柄意外允许了写入".into()),
  }
}

async fn reader_demo(root: PathBuf) -> Result<()> {
  let reader = Client::open_reader(disk_source(root)).await?;
  let result = check_reader(&reader).await;
  let closed = reader.close().await;
  result?;
  closed?;
  Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
  let mut args = std::env::args().skip(1);
  let mode = args.next().unwrap_or_else(|| "memory".to_string());
  let root = args
    .next()
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data"));
  if args.next().is_some() {
    return Err("参数过多；使用 --help 查看运行方式".into());
  }
  match mode.as_str() {
    "memory" => memory_demo().await,
    "disk" => disk_demo(root).await,
    "reader" => reader_demo(root).await,
    "-h" | "--help" => {
      println!("cargo run -p helix_embedded_example -- [memory|disk|reader] [数据根目录]");
      println!("默认 memory；reader 需要先运行 disk。disk 每次新增一个示例节点。");
      Ok(())
    }
    _ => Err(format!("未知案例 {mode:?}；可选 memory、disk、reader").into()),
  }
}

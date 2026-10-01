//! Small property graphs: every case seeds a fresh database and verifies real query results.
use helix_db::{Client, HelixDbSource, dsl::prelude::*};
use serde_json::{Value, json};

use crate::Result;

// A single write batch creates named nodes, then references them to create directed edges.
async fn seed(
  client: &Client,
  nodes: &[(&str, &str, &str)],
  edges: &[(&str, &str, &str, i64)],
) -> Result<()> {
  let mut batch = write_batch();
  for &(key, label, name) in nodes {
    batch = batch.var_as(key, g().add_n(label, vec![("name", name)]));
  }
  for (index, &(from, label, to, weight)) in edges.iter().enumerate() {
    batch = batch.var_as(
      &format!("edge_{index}"),
      g()
        .n(NodeRef::var(from))
        .add_e(label, NodeRef::var(to), vec![("weight", weight)]),
    );
  }
  let _: Value = client
    .query(QueryRequest::write(batch.returning([nodes[0].0])))
    .send()
    .await?;
  Ok(())
}

fn named(label: &str, name: &str) -> Traversal<OnNodes> {
  g().n_with_label(label).where_(Predicate::eq("name", name))
}

// Compare unordered rows while preserving duplicates: missing/extra paths must fail.
async fn verify(client: &Client, title: &str, batch: ReadBatch, expected: Value) -> Result<()> {
  let actual: Value = client.query(QueryRequest::read(batch)).send().await?;
  println!("\n{title}\n{}", serde_json::to_string_pretty(&actual)?);
  let normalize = |value: &Value| -> Result<Vec<String>> {
    let rows = value["result"].as_array().ok_or("结果必须为 result 数组")?;
    let mut rows: Vec<_> = rows.iter().map(Value::to_string).collect();
    rows.sort();
    Ok(rows)
  };
  if normalize(&actual)? != normalize(&expected)? {
    return Err(format!("{title} 验证失败；预期 {expected}，实际 {actual}").into());
  }
  Ok(())
}

async fn social(client: &Client) -> Result<()> {
  seed(
    client,
    &[
      ("alice", "Person", "Alice"),
      ("bob", "Person", "Bob"),
      ("carol", "Person", "Carol"),
      ("dave", "Person", "Dave"),
      ("eve", "Person", "Eve"),
    ],
    &[
      ("alice", "FOLLOWS", "bob", 5),
      ("alice", "FOLLOWS", "carol", 1),
      ("bob", "FOLLOWS", "dave", 1),
      ("carol", "FOLLOWS", "dave", 1),
      ("bob", "FOLLOWS", "alice", 1),
      ("bob", "FOLLOWS", "carol", 1),
    ],
  )
  .await?;
  verify(
    client,
    "Alice 直接关注的人",
    read_batch()
      .var_as(
        "result",
        named("Person", "Alice")
          .out(Some("FOLLOWS"))
          .value_map(Some(vec!["name"])),
      )
      .returning(["result"]),
    json!({"result": [{"name":"Bob"}, {"name":"Carol"}]}),
  )
  .await?;
  // Two paths reach Dave; exclude self and existing follows, then deduplicate.
  verify(
    client,
    "二跳发现：关注的人又关注了谁（排除自己和已关注的人）",
    read_batch()
      .var_as("me", named("Person", "Alice"))
      .var_as("following", g().n(NodeRef::var("me")).out(Some("FOLLOWS")))
      .var_as(
        "result",
        g()
          .n(NodeRef::var("following"))
          .out(Some("FOLLOWS"))
          .without("me")
          .without("following")
          .dedup()
          .value_map(Some(vec!["name"])),
      )
      .returning(["result"]),
    json!({"result": [{"name":"Dave"}]}),
  )
  .await?;
  verify(
    client,
    "关系也有属性：Alice 的高互动关注关系（weight >= 3）",
    read_batch()
      .var_as(
        "result",
        named("Person", "Alice")
          .out_e(Some("FOLLOWS"))
          .where_(Predicate::gte("weight", 3_i64))
          .value_map(Some(vec!["weight"])),
      )
      .returning(["result"]),
    json!({"result": [{"weight":5}]}),
  )
  .await
}

async fn recommendation(client: &Client) -> Result<()> {
  seed(
    client,
    &[
      ("alice", "Customer", "Alice"),
      ("bob", "Customer", "Bob"),
      ("carol", "Customer", "Carol"),
      ("book", "Product", "Rust Book"),
      ("keyboard", "Product", "Keyboard"),
      ("mouse", "Product", "Mouse"),
      ("camera", "Product", "Camera"),
    ],
    &[
      ("alice", "BOUGHT", "book", 1),
      ("bob", "BOUGHT", "book", 1),
      ("bob", "BOUGHT", "keyboard", 1),
      ("carol", "BOUGHT", "book", 1),
      ("carol", "BOUGHT", "keyboard", 1),
      ("carol", "BOUGHT", "mouse", 1),
    ],
  )
  .await?;
  // Customer -> purchased product <- other customers -> candidate products.
  verify(
    client,
    "共同购买推荐：买过相同商品的人还买了什么",
    read_batch()
      .var_as("me", named("Customer", "Alice"))
      .var_as("bought", g().n(NodeRef::var("me")).out(Some("BOUGHT")))
      .var_as(
        "result",
        g()
          .n(NodeRef::var("bought"))
          .in_(Some("BOUGHT"))
          .without("me")
          .dedup()
          .out(Some("BOUGHT"))
          .without("bought")
          .dedup()
          .value_map(Some(vec!["name"])),
      )
      .returning(["result"]),
    json!({"result": [{"name":"Keyboard"}, {"name":"Mouse"}]}),
  )
  .await
}

async fn knowledge(client: &Client) -> Result<()> {
  seed(
    client,
    &[
      ("ada", "Author", "Ada"),
      ("lin", "Author", "Lin"),
      ("graph", "Article", "Graph Basics"),
      ("rust", "Article", "Rust Storage"),
      ("topic", "Topic", "Graph Database"),
      ("other", "Topic", "Rust"),
    ],
    &[
      ("ada", "WROTE", "graph", 1),
      ("lin", "WROTE", "rust", 1),
      ("graph", "ABOUT", "topic", 1),
      ("rust", "ABOUT", "other", 1),
      ("rust", "CITES", "graph", 1),
    ],
  )
  .await?;
  // Bind preserves which article led to each author; useful for provenance.
  verify(
    client,
    "知识溯源：图数据库主题对应的文章及作者",
    read_batch()
      .var_as(
        "result",
        named("Topic", "Graph Database")
          .in_(Some("ABOUT"))
          .bind("article")
          .in_(Some("WROTE"))
          .project_bindings(vec![
            BindingProjection::binding("article", "name", "article"),
            BindingProjection::current("name", "author"),
          ]),
      )
      .returning(["result"]),
    json!({"result": [{"article":"Graph Basics", "author":"Ada"}]}),
  )
  .await?;
  verify(
    client,
    "反向引用：哪些文章引用了 Graph Basics",
    read_batch()
      .var_as(
        "result",
        named("Article", "Graph Basics")
          .in_(Some("CITES"))
          .value_map(Some(vec!["name"])),
      )
      .returning(["result"]),
    json!({"result": [{"name":"Rust Storage"}]}),
  )
  .await
}

fn impact() -> ReadBatch {
  read_batch()
    .var_as("origin", named("Service", "Database"))
    .var_as(
      "result",
      g()
        .n(NodeRef::var("origin"))
        .repeat(
          RepeatConfig::new(sub().in_(Some("DEPENDS_ON")))
            .emit_after()
            .max_depth(3),
        )
        .without("origin")
        .dedup()
        .value_map(Some(vec!["name"])),
    )
    .returning(["result"])
}

async fn dependencies(client: &Client) -> Result<()> {
  seed(
    client,
    &[
      ("db", "Service", "Database"),
      ("orders", "Service", "Orders"),
      ("inventory", "Service", "Inventory"),
      ("checkout", "Service", "Checkout"),
      ("web", "Service", "Web"),
      ("search", "Service", "Search"),
    ],
    &[
      ("orders", "DEPENDS_ON", "db", 1),
      ("inventory", "DEPENDS_ON", "db", 1),
      ("checkout", "DEPENDS_ON", "orders", 1),
      ("web", "DEPENDS_ON", "checkout", 1),
    ],
  )
  .await?;
  verify(
    client,
    "影响分析：Database 故障可能影响的服务（最多三跳）",
    impact(),
    json!({"result": [{"name":"Orders"}, {"name":"Inventory"},
      {"name":"Checkout"}, {"name":"Web"}]}),
  )
  .await?;
  let _: Value = client
    .query(QueryRequest::write(
      write_batch()
        .var_as("db", named("Service", "Database"))
        .var_as(
          "removed",
          named("Service", "Orders").drop_edge_labeled(NodeRef::var("db"), "DEPENDS_ON"),
        )
        .returning(["removed"]),
    ))
    .send()
    .await?;
  verify(
    client,
    "移除 Orders -> Database 依赖后重新分析",
    impact(),
    json!({"result": [{"name":"Inventory"}]}),
  )
  .await
}

pub async fn run(mode: &str) -> Result<()> {
  let cases: &[&str] = if mode == "graph-all" {
    &["social", "recommendation", "knowledge", "dependencies"]
  } else {
    &[mode]
  };
  for &case in cases {
    println!("\n=== Graph: {case} ===");
    let client = Client::open(HelixDbSource::InMemory {
      database: format!("graph-{case}"),
    })
    .await?;
    let result = match case {
      "social" => social(&client).await,
      "recommendation" => recommendation(&client).await,
      "knowledge" => knowledge(&client).await,
      "dependencies" => dependencies(&client).await,
      _ => Err(format!("未知图案例：{case}").into()),
    };
    let closed = client.close().await;
    result?;
    closed?;
    println!("{case}：全部结果验证通过。");
  }
  Ok(())
}

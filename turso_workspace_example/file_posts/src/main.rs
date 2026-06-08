use turso::{Builder, Connection, Row};

#[derive(Debug)]
struct Post {
  id: i64,
  title: String,
  content: String,
  created_at: String,
}

#[tokio::main]
async fn main() -> turso::Result<()> {
  let db = Builder::new_local("my-database.db").build().await?;
  let conn = db.connect()?;

  create_table(&conn).await?;
  clear_posts(&conn).await?;

  let post_id = create_post(&conn, "Hello World", "This is my first blog post!").await?;
  println!("Create: inserted post #{post_id}");

  println!("\nRead: post after insert");
  if let Some(post) = find_post(&conn, post_id).await? {
    print_post(&post);
  }

  let rows_updated = update_post(
    &conn,
    post_id,
    "Hello Turso",
    "Updated content from the CRUD example.",
  )
  .await?;
  println!("\nUpdate: updated {rows_updated} row(s)");

  println!("\nRead: post after update");
  if let Some(post) = find_post(&conn, post_id).await? {
    print_post(&post);
  }

  println!("\nRead: all posts before delete");
  for post in list_posts(&conn).await? {
    print_post(&post);
  }

  let rows_deleted = delete_post(&conn, post_id).await?;
  println!("\nDelete: deleted {rows_deleted} row(s)");

  println!("\nRead: all posts after delete");
  for post in list_posts(&conn).await? {
    print_post(&post);
  }

  Ok(())
}

async fn create_table(conn: &Connection) -> turso::Result<()> {
  conn
    .execute(
      r#"CREATE TABLE IF NOT EXISTS posts (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL,
        content TEXT,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP
      )"#,
      (),
    )
    .await?;

  Ok(())
}

async fn clear_posts(conn: &Connection) -> turso::Result<u64> {
  conn.execute("DELETE FROM posts", ()).await
}

async fn create_post(conn: &Connection, title: &str, content: &str) -> turso::Result<i64> {
  let rows_affected = conn
    .execute(
      "INSERT INTO posts (title, content) VALUES (?1, ?2)",
      (title, content),
    )
    .await?;

  println!("Create: inserted {rows_affected} row(s)");
  Ok(conn.last_insert_rowid())
}

async fn find_post(conn: &Connection, id: i64) -> turso::Result<Option<Post>> {
  let mut rows = conn
    .query(
      r#"SELECT id, title, COALESCE(content, ''), created_at
         FROM posts
         WHERE id = ?1"#,
      (id,),
    )
    .await?;

  rows
    .next()
    .await?
    .map(|row| post_from_row(&row))
    .transpose()
}

async fn list_posts(conn: &Connection) -> turso::Result<Vec<Post>> {
  let mut rows = conn
    .query(
      r#"SELECT id, title, COALESCE(content, ''), created_at
         FROM posts
         ORDER BY id"#,
      (),
    )
    .await?;

  let mut posts = Vec::new();
  while let Some(row) = rows.next().await? {
    posts.push(post_from_row(&row)?);
  }

  Ok(posts)
}

async fn update_post(conn: &Connection, id: i64, title: &str, content: &str) -> turso::Result<u64> {
  conn
    .execute(
      "UPDATE posts SET title = ?1, content = ?2 WHERE id = ?3",
      (title, content, id),
    )
    .await
}

async fn delete_post(conn: &Connection, id: i64) -> turso::Result<u64> {
  conn.execute("DELETE FROM posts WHERE id = ?1", (id,)).await
}

fn post_from_row(row: &Row) -> turso::Result<Post> {
  Ok(Post {
    id: row.get(0)?,
    title: row.get(1)?,
    content: row.get(2)?,
    created_at: row.get(3)?,
  })
}

fn print_post(post: &Post) {
  println!(
    "Post #{}: {} | {} | created_at={}",
    post.id, post.title, post.content, post.created_at,
  );
}

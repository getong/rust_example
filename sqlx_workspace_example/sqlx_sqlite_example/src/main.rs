use sqlx::{FromRow, SqlitePool, sqlite::SqlitePoolOptions};

#[derive(Debug, FromRow)]
struct Todo {
  id: i64,
  title: String,
  completed: bool,
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
  let pool = SqlitePoolOptions::new()
    // 使用文件型 SQLite 数据库，程序退出后数据仍保存在项目目录的 todos.db 文件中。
    .max_connections(5)
    .connect("sqlite://todos.db?mode=rwc")
    .await?;

  create_schema(&pool).await?;

  println!("1. Create: 新增待办事项");
  let todo = create_todo(&pool, "learn sqlx with sqlite").await?;
  print_todo(&todo);

  println!("\n2. Read: 查询当前待办列表");
  for todo in list_todos(&pool).await? {
    print_todo(&todo);
  }

  println!("\n3. Update: 标记待办事项已完成");
  mark_todo_completed(&pool, todo.id).await?;
  if let Some(todo) = find_todo(&pool, todo.id).await? {
    print_todo(&todo);
  }

  println!("\n4. Delete: 删除待办事项");
  delete_todo(&pool, todo.id).await?;
  println!("remaining todos: {:?}", list_todos(&pool).await?);

  Ok(())
}

async fn create_schema(pool: &SqlitePool) -> Result<(), sqlx::Error> {
  sqlx::query(
    r#"
        CREATE TABLE IF NOT EXISTS todos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            completed BOOLEAN NOT NULL DEFAULT FALSE,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
  )
  .execute(pool)
  .await?;

  Ok(())
}

async fn create_todo(pool: &SqlitePool, title: &str) -> Result<Todo, sqlx::Error> {
  sqlx::query_as::<_, Todo>(
    r#"
        INSERT INTO todos (title)
        VALUES (?1)
        RETURNING id, title, completed
        "#,
  )
  .bind(title)
  .fetch_one(pool)
  .await
}

async fn list_todos(pool: &SqlitePool) -> Result<Vec<Todo>, sqlx::Error> {
  sqlx::query_as::<_, Todo>(
    r#"
        SELECT id, title, completed
        FROM todos
        ORDER BY id
        "#,
  )
  .fetch_all(pool)
  .await
}

async fn find_todo(pool: &SqlitePool, id: i64) -> Result<Option<Todo>, sqlx::Error> {
  sqlx::query_as::<_, Todo>(
    r#"
        SELECT id, title, completed
        FROM todos
        WHERE id = ?1
        "#,
  )
  .bind(id)
  .fetch_optional(pool)
  .await
}

async fn mark_todo_completed(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
  sqlx::query(
    r#"
        UPDATE todos
        SET completed = TRUE
        WHERE id = ?1
        "#,
  )
  .bind(id)
  .execute(pool)
  .await?;

  Ok(())
}

async fn delete_todo(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
  sqlx::query(
    r#"
        DELETE FROM todos
        WHERE id = ?1
        "#,
  )
  .bind(id)
  .execute(pool)
  .await?;

  Ok(())
}

fn print_todo(todo: &Todo) {
  println!(
    "id={}, title={}, completed={}",
    todo.id, todo.title, todo.completed
  );
}

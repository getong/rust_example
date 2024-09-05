use chrono::{Duration, NaiveDateTime, Utc};
use dotenv::dotenv;
use rand::{distributions::Alphanumeric, Rng};
use sqlx::{postgres::PgPoolOptions, query, query_as, FromRow};
use std::env;

#[derive(Debug, FromRow)]
pub struct MyData {
  pub id: i32,
  pub value: String,
  pub created_at: NaiveDateTime,
  pub updated_at: NaiveDateTime,
}

#[derive(Debug, FromRow)]
struct MyDataValue {
  pub value: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Load environment variables from a .env file
  dotenv().ok();
  let database_url = env::var("DATABASE_URL")?;

  // Create a connection pool with a maximum of 5 connections
  let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await?;

  // Insert random data into the table
  let now = Utc::now().naive_utc();
  let random_name: String = rand::thread_rng()
    .sample_iter(&Alphanumeric)
    .take(10)
    .map(char::from)
    .collect();

  let insert_result =
    query("INSERT INTO my_data (value, created_at, updated_at) VALUES ($1, $2, $3)")
      .bind(random_name)
      .bind(now)
      .bind(now)
      .execute(&pool)
      .await?;

  println!("Inserted {} row(s)", insert_result.rows_affected());

  // Calculate the time 24 hours ago
  let time_24_hours_ago = now - Duration::seconds(86400);

  // Query the database using query_as! macro
  let rows = query_as!(
    MyData,
    "SELECT id, value, created_at, updated_at FROM my_data WHERE updated_at > $1",
    time_24_hours_ago
  )
  .fetch_all(&pool)
  .await?;

  // Process the results
  for row in rows {
    println!(
      "ID: {}, Value: {}, Updated At: {}",
      row.id, row.value, row.updated_at
    );
  }

  // Query the database for only the value column using query_as! macro
  let values = query_as!(
    MyDataValue,
    "SELECT value FROM my_data WHERE updated_at > $1",
    time_24_hours_ago
  )
  .fetch_all(&pool)
  .await?;

  // Process the results
  for value in values {
    println!("Value: {}", value.value);
  }

  Ok(())
}

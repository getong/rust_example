use bigdecimal::BigDecimal;
use sqlx::postgres::PgPoolOptions;
use std::env;

// CREATE TABLE IF NOT EXISTS financial_data (
//     id SERIAL PRIMARY KEY,
//     amount NUMERIC NOT NULL
// );

#[derive(Debug)]
struct FinancialData {
  id: i32,
  amount: BigDecimal,
}

async fn insert_data(pool: &sqlx::PgPool, amount: BigDecimal) -> Result<(), sqlx::Error> {
  sqlx::query!("INSERT INTO financial_data (amount) VALUES ($1)", amount)
    .execute(pool)
    .await?;

  Ok(())
}

async fn fetch_data(pool: &sqlx::PgPool) -> Result<Vec<FinancialData>, sqlx::Error> {
  let rows = sqlx::query!("SELECT id, amount FROM financial_data")
    .fetch_all(pool)
    .await?;

  let data: Vec<FinancialData> = rows
    .into_iter()
    .map(|row| FinancialData {
      id: row.id,
      amount: row.amount,
    })
    .collect();

  Ok(data)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  dotenv::dotenv().ok();
  let database_url = env::var("DATABASE_URL")?;

  let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await?;

  // Insert data
  let amount = BigDecimal::parse_bytes(b"12345.67", 10).unwrap();
  insert_data(&pool, amount.clone()).await?;

  // Fetch and display data
  let data = fetch_data(&pool).await?;
  for record in data {
    println!("ID: {}, Amount: {}", record.id, record.amount);
  }

  Ok(())
}

use bigdecimal::BigDecimal;
use dotenv::dotenv;
use ethereum_types::U256;
use sqlx::{postgres::PgPoolOptions, query, query_as, FromRow};
use std::{env, str::FromStr};

#[derive(Debug, FromRow)]
pub struct MyData {
  pub id: i32,
  pub value_u256: String,
  pub value_bigdecimal: BigDecimal,
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

  // Create a table if it doesn't exist
  query(
    r#"
        CREATE TABLE IF NOT EXISTS my_data (
            id SERIAL PRIMARY KEY,
            value_u256 TEXT NOT NULL,
            value_bigdecimal NUMERIC NOT NULL
        )
        "#,
  )
  .execute(&pool)
  .await?;

  // Insert data into the table
  let value_u256 = U256::from_dec_str("123456789012345678901234567890").unwrap();
  let value_bigdecimal = BigDecimal::from_str("12345.6789").unwrap();

  query(
    r#"
        INSERT INTO my_data (value_u256, value_bigdecimal)
        VALUES ($1, $2)
        "#,
  )
  .bind(value_u256.to_string())
  .bind(value_bigdecimal)
  .execute(&pool)
  .await?;

  // Query data from the table
  let rows: Vec<MyData> = query_as(
    r#"
        SELECT id, value_u256, value_bigdecimal
        FROM my_data
        "#,
  )
  .fetch_all(&pool)
  .await?;

  // Print the queried data
  for row in rows {
    println!("{:?}", row);
  }

  Ok(())
}

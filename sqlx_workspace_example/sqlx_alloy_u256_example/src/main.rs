use std::env;

use alloy_primitives::U256;
use anyhow::{Context, Result, anyhow};
use sqlx::{PgPool, postgres::PgPoolOptions};

const SAMPLE_AMOUNTS: [&str; 5] = [
  "0",
  "340282366920938463463374607431768211455",
  "340282366920938463463374607431768211456",
  "340282366920938463463374607431768211457",
  "115792089237316195423570985008687907853269984665640564039457584007913129639935",
];

const SQL_THRESHOLD: &str = "340282366920938463463374607431768211456";

#[derive(Debug, PartialEq, Eq)]
struct StoredAmount {
  id: i32,
  amount: U256,
}

fn parse_u256(value: &str) -> Result<U256> {
  U256::from_str_radix(value, 10)
    .map_err(|error| anyhow!("failed to parse U256 value {value}: {error}"))
}

async fn insert_amount(pool: &PgPool, amount: U256) -> Result<()> {
  let decimal_text = amount.to_string();

  sqlx::query(
    r#"
      INSERT INTO financial_data (amount)
      VALUES ($1::TEXT::NUMERIC)
    "#,
  )
  .bind(&decimal_text)
  .execute(pool)
  .await
  .with_context(|| format!("failed to store U256 amount {decimal_text} as NUMERIC"))?;

  Ok(())
}

async fn amounts_greater_than(pool: &PgPool, threshold: U256) -> Result<Vec<StoredAmount>> {
  let threshold_text = threshold.to_string();
  let rows = sqlx::query_as::<_, (i32, String)>(
    r#"
      SELECT financial_data.id, financial_data.amount::TEXT AS amount_text
      FROM financial_data
      WHERE financial_data.amount > $1::TEXT::NUMERIC
      ORDER BY financial_data.amount
    "#,
  )
  .bind(&threshold_text)
  .fetch_all(pool)
  .await
  .context("failed to compare U256 NUMERIC values in PostgreSQL")?;

  rows
    .into_iter()
    .map(|(id, amount)| {
      Ok(StoredAmount {
        id,
        amount: parse_u256(&amount)
          .with_context(|| format!("failed to decode NUMERIC amount for row {id}"))?,
      })
    })
    .collect()
}

async fn total_amount(pool: &PgPool) -> Result<String> {
  sqlx::query_scalar(
    r#"
      SELECT COALESCE(SUM(amount), 0)::TEXT
      FROM financial_data
    "#,
  )
  .fetch_one(pool)
  .await
  .context("failed to sum U256 NUMERIC values in PostgreSQL")
}

#[tokio::main]
async fn main() -> Result<()> {
  dotenv::dotenv().ok();
  let database_url = env::var("DATABASE_URL").context("DATABASE_URL is not set")?;

  let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await
    .context("failed to connect to PostgreSQL")?;

  sqlx::migrate!()
    .run(&pool)
    .await
    .context("failed to run database migrations")?;

  for amount in SAMPLE_AMOUNTS {
    insert_amount(&pool, parse_u256(amount)?).await?;
  }

  let threshold = parse_u256(SQL_THRESHOLD)?;
  let matches = amounts_greater_than(&pool, threshold).await?;
  let total = total_amount(&pool).await?;

  println!("SQL threshold: {threshold}");
  println!("Values greater than the threshold (compared by PostgreSQL):");
  for record in matches {
    println!("  id {} = {}", record.id, record.amount);
  }
  println!("SQL SUM(amount): {total}");

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn decimal_text_round_trips_all_sample_u256_values() -> Result<()> {
    for amount in SAMPLE_AMOUNTS {
      let parsed = parse_u256(amount)?;
      assert_eq!(parse_u256(&parsed.to_string())?, parsed);
    }

    Ok(())
  }
}

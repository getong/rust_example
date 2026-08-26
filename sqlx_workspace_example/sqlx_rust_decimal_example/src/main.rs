use std::env;

use anyhow::{Context, Result};
use rust_decimal::Decimal;
use sqlx::{PgPool, postgres::PgPoolOptions};

const LARGE_AMOUNTS: [(&str, &str); 3] = [
  (
    "below_sql_threshold",
    "9999999999999999999999999999999999999999.9999",
  ),
  (
    "equal_to_sql_threshold",
    "10000000000000000000000000000000000000000.0000",
  ),
  (
    "above_sql_threshold",
    "10000000000000000000000000000000000000000.0001",
  ),
];

const SQL_THRESHOLD: &str = "10000000000000000000000000000000000000000.0000";

#[derive(Debug)]
struct FinancialData {
  label: String,
  amount: String,
}

async fn upsert_amount(pool: &PgPool, label: &str, amount: &str) -> Result<()> {
  sqlx::query(
    r#"
      INSERT INTO financial_data (label, amount)
      VALUES ($1, CAST($2::TEXT AS NUMERIC))
      ON CONFLICT (label) DO UPDATE SET amount = EXCLUDED.amount
    "#,
  )
  .bind(label)
  .bind(amount)
  .execute(pool)
  .await
  .with_context(|| format!("failed to store large amount for {label}"))?;

  Ok(())
}

async fn amounts_greater_than(pool: &PgPool, threshold: &str) -> Result<Vec<FinancialData>> {
  let rows = sqlx::query_as::<_, (String, String)>(
    r#"
      SELECT label, amount::TEXT
      FROM financial_data
      WHERE amount > CAST($1::TEXT AS NUMERIC)
      ORDER BY amount
    "#,
  )
  .bind(threshold)
  .fetch_all(pool)
  .await
  .context("failed to compare large NUMERIC values in PostgreSQL")?;

  Ok(
    rows
      .into_iter()
      .map(|(label, amount)| FinancialData { label, amount })
      .collect(),
  )
}

async fn count_greater_than_decimal(pool: &PgPool, threshold: Decimal) -> Result<i64> {
  sqlx::query_scalar("SELECT COUNT(*) FROM financial_data WHERE amount > $1")
    .bind(threshold)
    .fetch_one(pool)
    .await
    .context("failed to compare NUMERIC values with rust_decimal::Decimal")
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

  for (label, amount) in LARGE_AMOUNTS {
    upsert_amount(&pool, label, amount).await?;
  }

  println!("SQL threshold: {SQL_THRESHOLD}");
  println!("Values greater than the threshold (compared by PostgreSQL):");
  for record in amounts_greater_than(&pool, SQL_THRESHOLD).await? {
    println!("  {} = {}", record.label, record.amount);
  }

  let decimal_max = Decimal::MAX;
  let count = count_greater_than_decimal(&pool, decimal_max).await?;
  println!("rust_decimal::Decimal::MAX = {decimal_max}");
  println!("Values greater than Decimal::MAX: {count}");

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use super::*;

  #[test]
  fn sample_amounts_exceed_rust_decimal_range() {
    for (_, amount) in LARGE_AMOUNTS {
      assert!(Decimal::from_str(amount).is_err());
    }
  }
}

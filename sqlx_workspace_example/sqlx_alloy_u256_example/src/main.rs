use std::env;

use alloy_primitives::U256;
use anyhow::{Context, Result, anyhow, ensure};
use sqlx::{PgPool, postgres::PgPoolOptions};

const SAMPLE_AMOUNTS: [(&str, &str); 5] = [
  ("zero", "0"),
  ("below_threshold", "340282366920938463463374607431768211455"),
  (
    "equal_to_threshold",
    "340282366920938463463374607431768211456",
  ),
  ("above_threshold", "340282366920938463463374607431768211457"),
  (
    "u256_max",
    "115792089237316195423570985008687907853269984665640564039457584007913129639935",
  ),
];

const SQL_THRESHOLD: &str = "340282366920938463463374607431768211456";

#[derive(Debug, PartialEq, Eq)]
struct StoredAmount {
  label: String,
  amount: U256,
}

fn parse_u256(value: &str) -> Result<U256> {
  U256::from_str_radix(value, 10)
    .map_err(|error| anyhow!("failed to parse U256 value {value}: {error}"))
}

async fn upsert_amount(pool: &PgPool, label: &str, amount: U256) -> Result<()> {
  sqlx::query(
    r#"
      INSERT INTO u256_values (label, amount)
      VALUES ($1, $2)
      ON CONFLICT (label) DO UPDATE SET amount = EXCLUDED.amount
    "#,
  )
  .bind(label)
  .bind(amount)
  .execute(pool)
  .await
  .with_context(|| format!("failed to store U256 amount for {label}"))?;

  Ok(())
}

async fn amounts_greater_than(pool: &PgPool, threshold: U256) -> Result<Vec<StoredAmount>> {
  let rows = sqlx::query_as::<_, (String, U256)>(
    r#"
      SELECT label, amount
      FROM u256_values
      WHERE amount > $1
      ORDER BY amount
    "#,
  )
  .bind(threshold)
  .fetch_all(pool)
  .await
  .context("failed to compare U256 byte arrays in PostgreSQL")?;

  Ok(
    rows
      .into_iter()
      .map(|(label, amount)| StoredAmount { label, amount })
      .collect(),
  )
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

  for (label, amount) in SAMPLE_AMOUNTS {
    upsert_amount(&pool, label, parse_u256(amount)?).await?;
  }

  let threshold = parse_u256(SQL_THRESHOLD)?;
  let matches = amounts_greater_than(&pool, threshold).await?;

  ensure!(
    matches.len() == 2,
    "expected two values greater than the threshold, got {}",
    matches.len()
  );

  println!("SQL threshold: {threshold}");
  println!("Values greater than the threshold (compared by PostgreSQL):");
  for record in matches {
    println!("  {} = {}", record.label, record.amount);
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fixed_width_big_endian_order_matches_u256_order() -> Result<()> {
    let values = SAMPLE_AMOUNTS
      .into_iter()
      .map(|(_, amount)| parse_u256(amount))
      .collect::<Result<Vec<_>>>()?;

    for pair in values.windows(2) {
      let left = pair[0];
      let right = pair[1];

      assert!(left < right);
      assert!(left.to_be_bytes::<32>() < right.to_be_bytes::<32>());
    }

    Ok(())
  }
}

use std::env;

use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, postgres::PgPoolOptions};

#[derive(sqlx::FromRow, Debug)]
struct Reading {
  time: DateTime<Utc>,
  sensor_id: String,
  temperature: f64,
  humidity: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let database_url = env::var("DATABASE_URL")
    .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/tsdb".to_string());

  let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await?;

  sqlx::migrate!("./migrations").run(&pool).await?;

  seed_readings(&pool).await?;
  print_latest(&pool).await?;

  Ok(())
}

async fn seed_readings(pool: &PgPool) -> Result<(), sqlx::Error> {
  let base = Utc::now() - Duration::minutes(5);

  for offset in 0 .. 5 {
    let offset_minutes = i64::from(offset);
    let time = base + Duration::minutes(offset_minutes);
    let temperature = 20.0 + f64::from(offset) * 0.5;
    let humidity = 40.0 + f64::from(offset) * 0.8;

    sqlx::query(
      r#"
            INSERT INTO sensor_readings (time, sensor_id, temperature, humidity)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (time, sensor_id) DO NOTHING
            "#,
    )
    .bind(time)
    .bind("sensor-A")
    .bind(temperature)
    .bind(humidity)
    .execute(pool)
    .await?;
  }

  Ok(())
}

async fn print_latest(pool: &PgPool) -> Result<(), sqlx::Error> {
  let rows = sqlx::query_as::<_, Reading>(
    r#"
        SELECT time, sensor_id, temperature, humidity
        FROM sensor_readings
        WHERE sensor_id = $1
        ORDER BY time DESC
        LIMIT 5
        "#,
  )
  .bind("sensor-A")
  .fetch_all(pool)
  .await?;

  for reading in rows {
    println!(
      "{} {} temp={}C humidity={}%",
      reading.time, reading.sensor_id, reading.temperature, reading.humidity
    );
  }

  Ok(())
}

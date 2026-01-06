use std::env;

use chrono::{NaiveDateTime, Utc};
use dotenv::dotenv;
use rand::{distr::Alphanumeric, Rng};
use sqlx::{postgres::PgPoolOptions, query_as, FromRow, PgConnection};

#[derive(Debug, FromRow)]
pub struct DailyData {
  pub id: i32,
  pub value: String,
  pub created_at: NaiveDateTime,
  pub updated_at: NaiveDateTime,
}

#[derive(Debug, FromRow)]
pub struct MonthData {
  pub id: i32,
  pub value: String,
  pub created_at: NaiveDateTime,
  pub updated_at: NaiveDateTime,
}

#[derive(Debug, FromRow)]
pub struct MyDataValue {
  pub value: String,
}

// Define a struct to hold the result
pub struct SumResult {
  pub sum: Option<i64>,
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
  let random_name: String = rand::rng()
    .sample_iter(&Alphanumeric)
    .take(10)
    .map(char::from)
    .collect();

  // let insert_result =
  //   query("INSERT INTO daily_data (value, created_at, updated_at) VALUES ($1, $2, $3)")
  //     .bind(random_name.clone())
  //     .bind(now)
  //     .bind(now)
  //     .execute(&pool)
  //     .await?;

  // println!("Inserted {} row(s)", insert_result.rows_affected());

  // let insert_result =
  //   query("INSERT INTO month_data (value, created_at, updated_at) VALUES ($1, $2, $3)")
  //     .bind(random_name.clone())
  //     .bind(now)
  //     .bind(now)
  //     .execute(&pool)
  //     .await?;

  // println!("Inserted {} row(s)", insert_result.rows_affected());

  // Start a conn
  let mut conn = pool.acquire().await?;

  // Insert into daily_data table
  // query("INSERT INTO daily_data (value, created_at, updated_at) VALUES ($1, $2, $3)")
  //   .bind(&random_name)
  //   .bind(now)
  //   .bind(now)
  //   .execute(&mut *conn)
  //   .await?;

  // Insert into month_data table
  // query("INSERT INTO month_data (value, created_at, updated_at) VALUES ($1, $2, $3)")
  //   .bind(&random_name)
  //   .bind(now)
  //   .bind(now)
  //   .execute(&mut *conn)
  //       .await?;

  // Insert into daily_data table and fetch the inserted row
  let inserted_daily_data: DailyData = query_as!(
    DailyData,
    "INSERT INTO daily_data (value, created_at, updated_at) VALUES ($1, $2, $3) RETURNING id, \
     value, created_at, updated_at",
    random_name.clone(),
    now,
    now
  )
  .fetch_one(&mut *conn)
  .await?;

  // Insert into month_data table and fetch the inserted row
  let inserted_month_data: MonthData = query_as!(
    MonthData,
    "INSERT INTO month_data (value, created_at, updated_at) VALUES ($1, $2, $3) RETURNING id, \
     value, created_at, updated_at",
    random_name,
    now,
    now
  )
  .fetch_one(&mut *conn)
  .await?;

  // Fetch all data from daily_data table within the same conn
  let daily_data: Vec<DailyData> = query_as!(
    DailyData,
    "SELECT id, value, created_at, updated_at FROM daily_data"
  )
  .fetch_all(&mut *conn)
  .await?;

  // Fetch all data from month_data table within the same conn
  let month_data: Vec<MonthData> = query_as!(
    MonthData,
    "SELECT id, value, created_at, updated_at FROM month_data"
  )
  .fetch_all(&mut *conn)
  .await?;

  // Fetch all data from daily_data table using the helper function
  let all_daily_data = get_all_daily_data(&mut *conn).await?;

  println!("Inserted data into both tables successfully");

  println!("Inserted into daily_data: {:?}", inserted_daily_data);
  println!("Inserted into month_data: {:?}", inserted_month_data);

  // Print the fetched data
  println!("Daily Data:");
  for data in daily_data {
    println!("{:?}", data);
  }

  println!("Month Data:");
  for data in month_data {
    println!("{:?}", data);
  }

  // Print the fetched data from the helper function
  println!("All Daily Data:");
  for data in all_daily_data {
    println!("{:?}", data);
  }

  // Define the query
  let row: (i64,) = sqlx::query_as("SELECT SUM(id) FROM daily_data where id > $1")
    .bind(1)
    .fetch_one(&mut *conn)
    .await?;

  // Extract the sum
  let sum_of_queries = row.0;

  println!("Sum of queries: {}", sum_of_queries);

  // Execute the query and map the result to the struct
  let result = query_as!(SumResult, "SELECT SUM(id) FROM daily_data where id > $1", 1)
    .fetch_one(&mut *conn)
    .await?;

  // Extract the sum
  let sum_of_queries = result.sum;

  println!("Sum of queries: {:?}", sum_of_queries);

  Ok(())
}

pub async fn get_all_daily_data(
  conn: &mut PgConnection,
) -> Result<Vec<DailyData>, Box<dyn std::error::Error>> {
  let daily_data: Vec<DailyData> = query_as!(
    DailyData,
    "SELECT id, value, created_at, updated_at FROM daily_data"
  )
  .fetch_all(conn)
  .await?;
  Ok(daily_data)
}

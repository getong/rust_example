use anyhow::Result;
use mysql_async::prelude::*;

// docker run --name my-own-mariadb -p 3306:3306 -e MYSQL_ROOT_PASSWORD=123456 -d mariadb:10.4.10

#[derive(Debug, PartialEq, Eq, Clone)]
struct Payment {
  customer_id: i32,
  amount: i32,
  account_name: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
  let payments = vec![
    Payment {
      customer_id: 1,
      amount: 2,
      account_name: None,
    },
    Payment {
      customer_id: 3,
      amount: 4,
      account_name: Some("foo".into()),
    },
    Payment {
      customer_id: 5,
      amount: 6,
      account_name: None,
    },
    Payment {
      customer_id: 7,
      amount: 8,
      account_name: None,
    },
    Payment {
      customer_id: 9,
      amount: 10,
      account_name: Some("bar".into()),
    },
  ];

  let database_url = "mysql://root:123456@localhost:3306"; /* ... */

  let pool = mysql_async::Pool::new(database_url);
  let mut conn = pool.get_conn().await?;

  r"drop DATABASE TEST;".ignore(&mut conn).await?;
  r"CREATE DATABASE TEST;".ignore(&mut conn).await?;
  r"use TEST;".ignore(&mut conn).await?;

  // Create a temporary table
  r"CREATE TEMPORARY TABLE payment (
        customer_id int not null,
        amount int not null,
        account_name text
    )"
  .ignore(&mut conn)
  .await?;

  // Save payments
  r"INSERT INTO payment (customer_id, amount, account_name)
      VALUES (:customer_id, :amount, :account_name)"
    .with(payments.iter().map(|payment| {
      params! {
          "customer_id" => payment.customer_id,
          "amount" => payment.amount,
          "account_name" => payment.account_name.as_ref(),
      }
    }))
    .batch(&mut conn)
    .await?;

  // Load payments from the database. Type inference will work here.
  let loaded_payments = "SELECT customer_id, amount, account_name FROM payment"
    .with(())
    .map(&mut conn, |(customer_id, amount, account_name)| Payment {
      customer_id,
      amount,
      account_name,
    })
    .await?;

  // Dropped connection will go to the pool
  drop(conn);

  // The Pool must be disconnected explicitly because
  // it's an asynchronous operation.
  pool.disconnect().await?;

  assert_eq!(loaded_payments, payments);

  // the async fn returns Result, so
  Ok(())
}

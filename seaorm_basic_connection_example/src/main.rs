use std::time::Duration;

use sea_orm::{ConnectOptions, Database};

fn main() {
  // println!("Hello, world!");
  let rt = tokio::runtime::Runtime::new().unwrap();
  rt.block_on(async {
    let mut opt = ConnectOptions::new("mysql://root:root@localhost/test_db".to_owned());
    opt
      .max_connections(100)
      .min_connections(5)
      .connect_timeout(Duration::from_secs(8))
      .idle_timeout(Duration::from_secs(8))
      .max_lifetime(Duration::from_secs(8))
      .sqlx_logging(true);

    let db = Database::connect(opt).await.unwrap();
    println!("db is {:?}", db);
  });
}

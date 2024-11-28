use mysql::{prelude::Queryable, Pool};

// docker run --name my-own-mariadb -p 3306:3306 -e MYSQL_ROOT_PASSWORD=123456 -d mariadb:10.4.10

#[tokio::main]
async fn main() {
  let pool = Pool::new("mysql://root:123456@localhost:3306").unwrap();

  let mut conn = pool.get_conn().unwrap();

  let result: Vec<String> = conn.query("SELECT 1").unwrap();

  assert_eq!(result.len(), 1);
}

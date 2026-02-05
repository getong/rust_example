use deadpool::managed;

#[derive(Debug)]
pub enum Error {
  Fail,
}

struct Computer {}

impl Computer {
  async fn get_answer(&self) -> i32 {
    42
  }
}

struct Manager {}

impl managed::Manager for Manager {
  type Type = Computer;
  type Error = Error;

  async fn create(&self) -> Result<Computer, Error> {
    Ok(Computer {})
  }

  async fn recycle(&self, _: &mut Computer, _: &managed::Metrics) -> managed::RecycleResult<Error> {
    Ok(())
  }
}

type Pool = managed::Pool<Manager>;

#[tokio::main]
async fn main() {
  let mgr = Manager {};
  let pool = Pool::builder(mgr).build().unwrap();
  let conn = pool.get().await.unwrap();
  let answer = conn.get_answer().await;
  assert_eq!(answer, 42);
}

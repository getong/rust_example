use std::sync::Arc;

use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Api {
  pub num: Arc<Mutex<i64>>,
}

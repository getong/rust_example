use crate::domain::entities::Counter;

pub(crate) trait CounterRepository {
  fn load(&self) -> Counter;
}

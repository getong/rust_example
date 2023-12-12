#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Todo {
  #[prost(uint32, tag = "1")]
  pub id: u32,
  #[prost(string, tag = "2")]
  pub title: ::prost::alloc::string::String,
  #[prost(bool, tag = "3")]
  pub completed: bool,
}

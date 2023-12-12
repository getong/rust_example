#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EchoMessage {
  #[prost(string, tag = "1")]
  pub message: ::prost::alloc::string::String,
}

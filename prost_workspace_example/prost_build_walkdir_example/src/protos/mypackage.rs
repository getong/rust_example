#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MyMessage {
  #[prost(string, tag = "1")]
  pub content: ::prost::alloc::string::String,
}
impl ::prost::Name for MyMessage {
  const NAME: &'static str = "MyMessage";
  const PACKAGE: &'static str = "mypackage";
  fn full_name() -> ::prost::alloc::string::String {
    ::prost::alloc::format!("mypackage.{}", Self::NAME)
  }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OtherMessage {
  #[prost(string, tag = "1")]
  pub data: ::prost::alloc::string::String,
}
impl ::prost::Name for OtherMessage {
  const NAME: &'static str = "OtherMessage";
  const PACKAGE: &'static str = "mypackage";
  fn full_name() -> ::prost::alloc::string::String {
    ::prost::alloc::format!("mypackage.{}", Self::NAME)
  }
}

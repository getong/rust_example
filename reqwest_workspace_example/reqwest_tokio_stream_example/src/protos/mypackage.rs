const PACKAGE: &str = "mypackage";
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MyMessage {
    #[prost(string, tag = "1")]
    pub content: ::prost::alloc::string::String,
}
impl ::prost::Name for MyMessage {
    const PACKAGE: &'static str = PACKAGE;
    const NAME: &'static str = "MyMessage";
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OtherMessage {
    #[prost(string, tag = "1")]
    pub data: ::prost::alloc::string::String,
}
impl ::prost::Name for OtherMessage {
    const PACKAGE: &'static str = PACKAGE;
    const NAME: &'static str = "OtherMessage";
}

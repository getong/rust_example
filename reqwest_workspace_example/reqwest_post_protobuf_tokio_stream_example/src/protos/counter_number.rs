#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadRequest {
    #[prost(string, tag = "1")]
    pub letter: ::prost::alloc::string::String,
    #[prost(int32, tag = "2")]
    pub before_number: i32,
    #[prost(uint32, tag = "3")]
    pub dummy_one: u32,
    #[prost(message, optional, tag = "4")]
    pub dummy_two: ::core::option::Option<SampleSchema>,
    #[prost(int32, repeated, tag = "5")]
    pub dummy_three: ::prost::alloc::vec::Vec<i32>,
}
impl ::prost::Name for ReadRequest {
    const NAME: &'static str = "ReadRequest";
    const PACKAGE: &'static str = "counter_number";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("counter_number.{}", Self::NAME)
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ReadResponse {
    #[prost(int32, tag = "1")]
    pub after_number: i32,
    #[prost(uint32, tag = "2")]
    pub dummy_one: u32,
    #[prost(message, optional, tag = "3")]
    pub dummy_two: ::core::option::Option<SampleSchema>,
    #[prost(int32, repeated, tag = "4")]
    pub dummy_three: ::prost::alloc::vec::Vec<i32>,
}
impl ::prost::Name for ReadResponse {
    const NAME: &'static str = "ReadResponse";
    const PACKAGE: &'static str = "counter_number";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("counter_number.{}", Self::NAME)
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SampleSchema {
    #[prost(bool, tag = "1")]
    pub sample_field_one: bool,
    #[prost(bool, tag = "2")]
    pub sample_field_two: bool,
}
impl ::prost::Name for SampleSchema {
    const NAME: &'static str = "SampleSchema";
    const PACKAGE: &'static str = "counter_number";
    fn full_name() -> ::prost::alloc::string::String {
        ::prost::alloc::format!("counter_number.{}", Self::NAME)
    }
}

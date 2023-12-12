/// A snazzy new shirt!
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Shirt {
  #[prost(string, tag = "1")]
  pub color: ::prost::alloc::string::String,
  #[prost(enumeration = "shirt::Size", tag = "2")]
  pub size: i32,
}
/// Nested message and enum types in `Shirt`.
pub mod shirt {
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
  #[repr(i32)]
  pub enum Size {
    Small = 0,
    Medium = 1,
    Large = 2,
  }
  impl Size {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
      match self {
        Size::Small => "SMALL",
        Size::Medium => "MEDIUM",
        Size::Large => "LARGE",
      }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
      match value {
        "SMALL" => Some(Self::Small),
        "MEDIUM" => Some(Self::Medium),
        "LARGE" => Some(Self::Large),
        _ => None,
      }
    }
  }
}

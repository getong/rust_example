use openraft::{
  AnyError, ErrorSubject, ErrorVerb, RaftTypeConfig, StorageError,
  alias::{LogIdOf, VoteOf},
};

/// Defines metadata key and value
pub(crate) trait StoreMeta<C>
where
  C: RaftTypeConfig,
{
  /// The key used to store in rocksdb
  const KEY: &'static str;

  /// The type of the value to store
  type Value: serde::Serialize + serde::de::DeserializeOwned;

  /// The subject this meta belongs to, and will be embedded into the returned storage error.
  fn subject(v: Option<&Self::Value>) -> ErrorSubject<C>;

  fn read_err(e: impl std::error::Error + 'static) -> StorageError<C> {
    StorageError::new(Self::subject(None), ErrorVerb::Read, AnyError::new(&e))
  }

  fn write_err(v: &Self::Value, e: impl std::error::Error + 'static) -> StorageError<C> {
    StorageError::new(Self::subject(Some(v)), ErrorVerb::Write, AnyError::new(&e))
  }
}

pub(crate) struct LastPurged {}
pub(crate) struct Vote {}

impl<C> StoreMeta<C> for LastPurged
where
  C: RaftTypeConfig,
{
  const KEY: &'static str = "last_purged_log_id";
  type Value = LogIdOf<C>;

  fn subject(_v: Option<&Self::Value>) -> ErrorSubject<C> {
    ErrorSubject::Store
  }
}
impl<C> StoreMeta<C> for Vote
where
  C: RaftTypeConfig,
{
  const KEY: &'static str = "vote";
  type Value = VoteOf<C>;

  fn subject(_v: Option<&Self::Value>) -> ErrorSubject<C> {
    ErrorSubject::Vote
  }
}

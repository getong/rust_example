use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// Node identifier type - using u64 to be compatible with OpenRAFT examples
pub type NodeId = u64;

// Declare the type configuration for our distributed store using the new macro
openraft::declare_raft_types!(
    /// Declare the type configuration for chitchat-openraft store.
    pub TypeConfig:
        D = Request,
        R = Response,
        SnapshotData = std::io::Cursor<Vec<u8>>,
);

// Import commonly used types
pub type LogId = openraft::LogId<TypeConfig>;
pub type StoredMembership = openraft::StoredMembership<TypeConfig>;
pub type SnapshotMeta = openraft::SnapshotMeta<TypeConfig>;
pub type Snapshot = openraft::Snapshot<TypeConfig>;
pub type Entry = <TypeConfig as openraft::RaftTypeConfig>::Entry;
pub type EntryPayload = openraft::EntryPayload<TypeConfig>;
pub type StorageError = openraft::StorageError<TypeConfig>;

// Request types for the distributed store
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Request {
  Set {
    table: Table,
    key: Key,
    value: Value,
  },
  BatchSet {
    table: Table,
    values: Vec<(Key, Value)>,
  },
  Get {
    table: Table,
    key: Key,
  },
  BatchGet {
    table: Table,
    keys: Vec<Key>,
  },
  Upsert {
    table: Table,
    key: Key,
    value: Value,
    upsert_fn: UpsertEnum,
  },
  BatchUpsert {
    table: Table,
    upsert_fn: UpsertEnum,
    values: Vec<(Key, Value)>,
  },
  CreateTable {
    table: Table,
  },
  DropTable {
    table: Table,
  },
  CloneTable {
    from: Table,
    to: Table,
  },
  AllTables,
}

// Response types for the distributed store
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Response {
  Set(Result<(), String>),
  Get(Result<Option<Value>, String>),
  BatchGet(Result<Vec<(Key, Value)>, String>),
  Upsert(Result<UpsertAction, String>),
  BatchUpsert(Result<Vec<(Key, UpsertAction)>, String>),
  CreateTable(Result<(), String>),
  DropTable(Result<(), String>),
  CloneTable(Result<(), String>),
  AllTables(Result<Vec<Table>, String>),
  Empty,
}

// Table identifier
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct Table(pub String);

impl Table {
  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl From<String> for Table {
  fn from(v: String) -> Self {
    Self(v)
  }
}

impl From<&str> for Table {
  fn from(v: &str) -> Self {
    Self(v.to_string())
  }
}

// Key type for the distributed store
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct Key(pub String);

impl From<String> for Key {
  fn from(v: String) -> Self {
    Self(v)
  }
}

impl From<&str> for Key {
  fn from(v: &str) -> Self {
    Self(v.to_string())
  }
}

// Value type for the distributed store
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(transparent)]
pub struct Value(pub Vec<u8>);

impl From<Vec<u8>> for Value {
  fn from(v: Vec<u8>) -> Self {
    Self(v)
  }
}

impl From<String> for Value {
  fn from(v: String) -> Self {
    Self(v.into_bytes())
  }
}

impl From<&str> for Value {
  fn from(v: &str) -> Self {
    Self(v.as_bytes().to_vec())
  }
}

// Upsert function types
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum UpsertEnum {
  Overwrite,
  AppendBytes,
  // Add more upsert functions as needed
}

impl UpsertEnum {
  pub fn upsert(&self, old: Value, new: Value) -> Value {
    match self {
      UpsertEnum::Overwrite => new,
      UpsertEnum::AppendBytes => {
        let mut result = old.0;
        result.extend(new.0);
        Value(result)
      }
    }
  }
}

// Upsert action result
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum UpsertAction {
  Inserted,
  Merged,
  NoChange,
}

// State machine data structure - this is what gets snapshot
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct StateMachineData {
  pub last_applied: Option<LogId>,
  pub last_membership: StoredMembership,
  /// Application data - the actual distributed store
  pub data: BTreeMap<Table, BTreeMap<Key, Value>>,
}

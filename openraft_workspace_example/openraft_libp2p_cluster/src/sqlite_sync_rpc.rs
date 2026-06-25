use serde::{Deserialize, Serialize};

use crate::GroupId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteFlushTask {
  pub openraft_key: String,
  pub data_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteFlushError {
  pub openraft_key: Option<String>,
  pub data_key: Option<String>,
  pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SqliteFlushReport {
  pub synced_openraft_keys: Vec<String>,
  pub missing_redis_keys: Vec<String>,
  pub errors: Vec<SqliteFlushError>,
  pub service_error: Option<String>,
}

impl SqliteFlushReport {
  pub fn service_error(message: impl Into<String>) -> Self {
    Self {
      service_error: Some(message.into()),
      ..Self::default()
    }
  }

  pub fn task_error(task: &SqliteFlushTask, message: impl Into<String>) -> SqliteFlushError {
    SqliteFlushError {
      openraft_key: Some(task.openraft_key.clone()),
      data_key: Some(task.data_key.clone()),
      message: message.into(),
    }
  }
}

#[tarpc::service]
pub trait SqliteSyncRpc {
  async fn flush_pending(group_id: GroupId, tasks: Vec<SqliteFlushTask>) -> SqliteFlushReport;
}

pub type SqliteSyncRpcRequestMessage = tarpc::ClientMessage<SqliteSyncRpcRequest>;
pub type SqliteSyncRpcResponseMessage = tarpc::Response<SqliteSyncRpcResponse>;

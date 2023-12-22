use crate::common::Api;
use crate::Node;
use crate::NodeId;
use crate::Request;
use openraft::error::CheckIsLeaderError;
use poem_openapi::{payload::Json, ApiResponse, Object, OpenApi};
use std::collections::{BTreeMap, BTreeSet};

#[derive(ApiResponse)]
pub enum SearchResponse {
  #[oai(status = 200)]
  Ok(Json<String>),
}

#[derive(ApiResponse)]
pub enum WriteResponse {
  #[oai(status = 200)]
  Ok(Json<String>),
}

#[derive(ApiResponse)]
pub enum ConsistentReadResponse {
  #[oai(status = 200)]
  Ok(Json<String>),
  #[oai(status = 500)]
  Fail,
}

#[derive(Debug, Object, Clone, Eq, PartialEq)]
pub struct AddRequest {
  key: String,
  value: String,
}

#[derive(Debug, Object, Clone, Eq, PartialEq)]
pub struct AddLearnerRequest {
  node_id: u64,
  api_addr: String,
  rpc_addr: String,
}

#[derive(ApiResponse)]
pub enum AddLearnerResponse {
  #[oai(status = 200)]
  Ok,
  #[oai(status = 500)]
  Fail,
}

#[derive(Debug, Object, Clone, Eq, PartialEq)]
pub struct ChangeMembershipRequest {
  members: BTreeSet<NodeId>,
}

#[derive(ApiResponse)]
pub enum ChangeMembershipResponse {
  #[oai(status = 200)]
  Ok,
  #[oai(status = 500)]
  Fail,
}

#[derive(ApiResponse)]
pub enum InitResponse {
  #[oai(status = 200)]
  Ok,
  #[oai(status = 500)]
  Fail,
}

#[derive(Debug, Object, Clone, Eq, PartialEq)]
pub struct MetricInfo {
  key: String,
  value: String,
}

#[derive(ApiResponse)]
pub enum MetricsResponse {
  #[oai(status = 200)]
  Ok(Json<String>),
}

#[OpenApi]
impl Api {
  #[oai(path = "/read", method = "post")]
  pub async fn read(&self, name: Json<String>) -> SearchResponse {
    let state_machine = self.key_values.read().await;
    let value = state_machine.get(&name.0).cloned().unwrap_or_default();

    SearchResponse::Ok(Json(value))
  }

  #[oai(path = "/write", method = "post")]
  pub async fn write(&self, name: Json<AddRequest>) -> WriteResponse {
    let req = Request::Set {
      key: name.0.key,
      value: name.0.value,
    };
    let result = self.raft.client_write(req).await;
    match result {
      Ok(_) => WriteResponse::Ok(Json("ok".to_string())),
      _ => WriteResponse::Ok(Json("failed".to_string())),
    }
  }

  #[oai(path = "/consistent_read", method = "post")]
  pub async fn consistent_read(&self, name: Json<String>) -> ConsistentReadResponse {
    let ret = self.raft.ensure_linearizable().await;

    match ret {
      Ok(_) => {
        let state_machine = self.key_values.read().await;

        let value = state_machine.get(&name.0).cloned().unwrap_or_default();

        let res: Result<String, CheckIsLeaderError<u64, Node>> = Ok(value);
        match res {
          Ok(result) => ConsistentReadResponse::Ok(Json(result)),
          Err(_) => ConsistentReadResponse::Fail,
        }
      }
      _e => ConsistentReadResponse::Fail,
    }
  }

  #[oai(path = "/add-learner", method = "post")]
  pub async fn add_learner(&self, name: Json<AddLearnerRequest>) -> AddLearnerResponse {
    let node = Node {
      rpc_addr: name.0.rpc_addr,
      api_addr: name.0.api_addr,
    };

    let res = self.raft.add_learner(name.0.node_id, node, true).await;
    match res {
      Ok(_) => AddLearnerResponse::Ok,
      _ => AddLearnerResponse::Fail,
    }
  }

  #[oai(path = "/change-membership", method = "post")]
  pub async fn change_membership(
    &self,
    name: Json<ChangeMembershipRequest>,
  ) -> ChangeMembershipResponse {
    let res = self.raft.change_membership(name.0.members, false).await;
    match res {
      Ok(_) => ChangeMembershipResponse::Ok,
      _ => ChangeMembershipResponse::Fail,
    }
  }

  #[oai(path = "/init", method = "post")]
  pub async fn init(&self) -> InitResponse {
    let node = Node {
      api_addr: self.api_addr.clone(),
      rpc_addr: self.rpc_addr.clone(),
    };
    let mut nodes = BTreeMap::new();
    nodes.insert(self.id, node);
    let res = self.raft.initialize(nodes).await;
    match res {
      Ok(_) => InitResponse::Ok,
      _ => InitResponse::Fail,
    }
  }

  #[oai(path = "/metrics", method = "post")]
  pub async fn metrics(&self) -> MetricsResponse {
    let res = self.raft.metrics().borrow().clone();
    // println!("res:{:?}", res);
    MetricsResponse::Ok(Json(res.to_string()))
  }
}

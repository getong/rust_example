use std::collections::{BTreeMap, BTreeSet};

use openraft::{error::decompose::DecomposeResult, ReadPolicy};
use poem_openapi::{payload::Json, ApiResponse, Object, OpenApi};

use crate::{common::Api, Node, NodeId, Request};

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
    let result = self.raft.client_write(req).await.decompose();
    match result {
      Ok(_) => WriteResponse::Ok(Json("ok".to_string())),
      Err(_) => WriteResponse::Ok(Json("failed".to_string())),
    }
  }

  #[oai(path = "/consistent_read", method = "post")]
  pub async fn consistent_read(&self, name: Json<String>) -> ConsistentReadResponse {
    let ret = self
      .raft
      .get_read_linearizer(ReadPolicy::ReadIndex)
      .await
      .decompose()
      .unwrap();

    match ret {
      Ok(linearizer) => {
        // Wait for the linearizer to be ready
        match linearizer.await_ready(&self.raft).await {
          Ok(_) => {
            let state_machine = self.key_values.read().await;
            let value = state_machine.get(&name.0).cloned().unwrap_or_default();
            ConsistentReadResponse::Ok(Json(value))
          }
          Err(_) => ConsistentReadResponse::Fail,
        }
      }
      Err(_e) => ConsistentReadResponse::Fail,
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

  /// Get cluster status and membership info
  pub async fn cluster(&self) -> MetricsResponse {
    let metrics = self.raft.metrics().borrow().clone();
    let learners: Vec<_> = metrics
      .membership_config
      .membership()
      .learner_ids()
      .collect();
    let cluster_info = format!(
      "Node ID: {}\nLeader: {:?}\nMembers: {:?}\nLearners: {:?}\nState: {:?}",
      self.id,
      metrics.current_leader,
      metrics.membership_config.membership().get_joint_config(),
      learners,
      metrics.state
    );
    MetricsResponse::Ok(Json(cluster_info))
  }

  /// Read a value by key (GET method for easy testing)
  #[oai(path = "/read/:key", method = "get")]
  pub async fn read_key(&self, key: poem_openapi::param::Path<String>) -> SearchResponse {
    let state_machine = self.key_values.read().await;
    let value = state_machine.get(&key.0).cloned().unwrap_or_default();
    SearchResponse::Ok(Json(value))
  }

  /// Consistent read a value by key (GET method for easy testing)
  #[oai(path = "/consistent_read/:key", method = "get")]
  pub async fn consistent_read_key(
    &self,
    key: poem_openapi::param::Path<String>,
  ) -> ConsistentReadResponse {
    let ret = self
      .raft
      .get_read_linearizer(ReadPolicy::ReadIndex)
      .await
      .decompose()
      .unwrap();

    match ret {
      Ok(linearizer) => {
        // Wait for the linearizer to be ready
        match linearizer.await_ready(&self.raft).await {
          Ok(_) => {
            let state_machine = self.key_values.read().await;
            let value = state_machine.get(&key.0).cloned().unwrap_or_default();
            ConsistentReadResponse::Ok(Json(value))
          }
          Err(_) => ConsistentReadResponse::Fail,
        }
      }
      Err(_e) => ConsistentReadResponse::Fail,
    }
  }

  #[oai(path = "/metrics", method = "post")]
  pub async fn metrics(&self) -> MetricsResponse {
    let res = self.raft.metrics().borrow().clone();
    MetricsResponse::Ok(Json(res.to_string()))
  }
}

/// Create the API service for the distributed node
pub async fn create_api_service(api: Api) -> impl poem::Endpoint {
  use poem::{middleware::Cors, EndpointExt, Route};
  use poem_openapi::OpenApiService;

  let api_service = OpenApiService::new(api, "Chitchat Poem Tarpc RocksDB Example", "1.0")
    .server("http://localhost:3000/api");

  let ui = api_service.swagger_ui();
  let spec = api_service.spec_endpoint();

  Route::new()
    .nest("/api", api_service)
    .nest("/", ui)
    .nest("/spec", spec)
    .with(Cors::new())
}

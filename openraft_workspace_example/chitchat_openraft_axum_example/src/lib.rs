use aide::OperationOutput;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod api;
pub mod cli;
pub mod demo;
pub mod distributed;
pub mod docs;
pub mod router;
pub mod utils;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ApiResponse {
  pub cluster_id: String,
  // Using serde_json::Value to avoid JsonSchema issues with external types
  pub cluster_state: serde_json::Value,
  pub live_nodes: Vec<String>,
  pub dead_nodes: Vec<String>,
}

impl OperationOutput for ApiResponse {
  type Inner = Self;

  fn operation_response(
    ctx: &mut aide::generate::GenContext,
    operation: &mut aide::openapi::Operation,
  ) -> Option<aide::openapi::Response> {
    <axum::Json<Self> as OperationOutput>::operation_response(ctx, operation)
  }

  fn inferred_responses(
    ctx: &mut aide::generate::GenContext,
    operation: &mut aide::openapi::Operation,
  ) -> Vec<(Option<u16>, aide::openapi::Response)> {
    <axum::Json<Self> as OperationOutput>::inferred_responses(ctx, operation)
  }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SetKeyValueResponse {
  pub status: bool,
}

impl OperationOutput for SetKeyValueResponse {
  type Inner = Self;

  fn operation_response(
    ctx: &mut aide::generate::GenContext,
    operation: &mut aide::openapi::Operation,
  ) -> Option<aide::openapi::Response> {
    <axum::Json<Self> as OperationOutput>::operation_response(ctx, operation)
  }

  fn inferred_responses(
    ctx: &mut aide::generate::GenContext,
    operation: &mut aide::openapi::Operation,
  ) -> Vec<(Option<u16>, aide::openapi::Response)> {
    <axum::Json<Self> as OperationOutput>::inferred_responses(ctx, operation)
  }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClusterMembersResponse {
  pub members: Vec<distributed::Member>,
}

impl OperationOutput for ClusterMembersResponse {
  type Inner = Self;

  fn operation_response(
    ctx: &mut aide::generate::GenContext,
    operation: &mut aide::openapi::Operation,
  ) -> Option<aide::openapi::Response> {
    <axum::Json<Self> as OperationOutput>::operation_response(ctx, operation)
  }

  fn inferred_responses(
    ctx: &mut aide::generate::GenContext,
    operation: &mut aide::openapi::Operation,
  ) -> Vec<(Option<u16>, aide::openapi::Response)> {
    <axum::Json<Self> as OperationOutput>::inferred_responses(ctx, operation)
  }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ServiceUpdateResponse {
  pub status: bool,
  pub message: String,
}

impl OperationOutput for ServiceUpdateResponse {
  type Inner = Self;

  fn operation_response(
    ctx: &mut aide::generate::GenContext,
    operation: &mut aide::openapi::Operation,
  ) -> Option<aide::openapi::Response> {
    <axum::Json<Self> as OperationOutput>::operation_response(ctx, operation)
  }

  fn inferred_responses(
    ctx: &mut aide::generate::GenContext,
    operation: &mut aide::openapi::Operation,
  ) -> Vec<(Option<u16>, aide::openapi::Response)> {
    <axum::Json<Self> as OperationOutput>::inferred_responses(ctx, operation)
  }
}

//! Wire codec for the single `/openraft/rpc/2` request-response protocol.
//!
//! Every RPC kind travels in one envelope tagged with a `kind` field:
//!   - raft ops encode as sonic-rs JSON, with full-snapshot data carried out-of-band in the
//!     envelope's zstd-compressed binary frame (so megabytes of snapshot bytes never pass through
//!     the JSON encoder);
//!   - kv ops keep their protobuf (prost) encoding;
//!   - sqlite-sync and task ops (tarpc envelopes) encode as JSON.

use std::io;

use bytes::Bytes;
use futures::prelude::*;
use libp2p::StreamProtocol;
use prost::Message;
use serde::{Serialize, de::DeserializeOwned};

use crate::network::rpc::{RaftRpcOp, RaftRpcRequest, UnifiedRpcRequest, UnifiedRpcResponse};

/// Requests carry raft full snapshots (out-of-band binary frame), so the
/// request limit is far above a typical RPC.
const REQUEST_MAX: u64 = 64 * 1024 * 1024;
const RESPONSE_MAX: u64 = 10 * 1024 * 1024;

/// zstd level for the snapshot frame: 3 is the ratio/speed sweet spot for
/// bulk state-machine data.
const SNAPSHOT_ZSTD_LEVEL: i32 = 3;

/// `kind` tag values on the wire. 0 is reserved (absent field in old
/// envelopes); never reuse a value for a different payload encoding.
const KIND_RAFT: u32 = 1;
const KIND_KV: u32 = 2;
const KIND_SQLITE_SYNC: u32 = 3;
const KIND_TASK: u32 = 4;
const KIND_WASM_SYNC: u32 = 5;

#[derive(Clone)]
pub struct UnifiedCodec {
  request_size_maximum: u64,
  response_size_maximum: u64,
}

impl Default for UnifiedCodec {
  fn default() -> Self {
    Self {
      request_size_maximum: REQUEST_MAX,
      response_size_maximum: RESPONSE_MAX,
    }
  }
}

impl UnifiedCodec {
  pub fn set_request_size_maximum(mut self, request_size_maximum: u64) -> Self {
    self.request_size_maximum = request_size_maximum;
    self
  }

  pub fn set_response_size_maximum(mut self, response_size_maximum: u64) -> Self {
    self.response_size_maximum = response_size_maximum;
    self
  }
}

/// Wire framing: decoded from a `Bytes` buffer, so `payload` and `binary`
/// are zero-copy refcounted slices of the read buffer.
#[derive(Clone, PartialEq, Message)]
struct ProtoEnvelope {
  /// Kind-specific encoding of the RPC value (JSON or protobuf).
  #[prost(bytes = "bytes", tag = "1")]
  payload: Bytes,
  /// Out-of-band binary frame (zstd); raft full-snapshot data.
  #[prost(bytes = "bytes", tag = "2")]
  binary: Bytes,
  /// Which `UnifiedRpc*` variant the payload decodes into.
  #[prost(uint32, tag = "3")]
  kind: u32,
}

impl libp2p::request_response::Codec for UnifiedCodec {
  type Protocol = StreamProtocol;
  type Request = UnifiedRpcRequest;
  type Response = UnifiedRpcResponse;

  fn read_request<T>(
    &mut self,
    _: &Self::Protocol,
    io: &mut T,
  ) -> impl Future<Output = io::Result<Self::Request>> + Send
  where
    T: AsyncRead + Unpin + Send,
  {
    let limit = self.request_size_maximum;
    async move {
      let envelope = read_envelope(io, limit).await?;
      decode_unified_request(envelope)
    }
  }

  fn read_response<T>(
    &mut self,
    _: &Self::Protocol,
    io: &mut T,
  ) -> impl Future<Output = io::Result<Self::Response>> + Send
  where
    T: AsyncRead + Unpin + Send,
  {
    let limit = self.response_size_maximum;
    async move {
      let envelope = read_envelope(io, limit).await?;
      decode_unified_response(envelope)
    }
  }

  fn write_request<T>(
    &mut self,
    _: &Self::Protocol,
    io: &mut T,
    req: Self::Request,
  ) -> impl Future<Output = io::Result<()>> + Send
  where
    T: AsyncWrite + Unpin + Send,
  {
    let data = encode_unified_request(req);
    async move { write_encoded(io, data).await }
  }

  fn write_response<T>(
    &mut self,
    _: &Self::Protocol,
    io: &mut T,
    resp: Self::Response,
  ) -> impl Future<Output = io::Result<()>> + Send
  where
    T: AsyncWrite + Unpin + Send,
  {
    let data = encode_unified_response(resp);
    async move { write_encoded(io, data).await }
  }
}

fn encode_unified_request(req: UnifiedRpcRequest) -> io::Result<Vec<u8>> {
  let envelope = match req {
    UnifiedRpcRequest::Raft(mut raft) => {
      // Full-snapshot data bypasses the JSON payload: pulled out of the op,
      // zstd-compressed, and carried in the envelope's binary frame.
      let snapshot_data = match &mut raft.op {
        RaftRpcOp::FullSnapshot { data, .. } => std::mem::take(data),
        _ => Vec::new(),
      };
      let binary = if snapshot_data.is_empty() {
        Bytes::new()
      } else {
        Bytes::from(
          zstd::stream::encode_all(snapshot_data.as_slice(), SNAPSHOT_ZSTD_LEVEL)
            .map_err(invalid_data)?,
        )
      };
      ProtoEnvelope {
        payload: encode_json(&raft)?.into(),
        binary,
        kind: KIND_RAFT,
      }
    }
    UnifiedRpcRequest::Kv(kv) => ProtoEnvelope {
      payload: kv.encode_to_vec().into(),
      binary: Bytes::new(),
      kind: KIND_KV,
    },
    UnifiedRpcRequest::SqliteSync(msg) => ProtoEnvelope {
      payload: encode_json(&msg)?.into(),
      binary: Bytes::new(),
      kind: KIND_SQLITE_SYNC,
    },
    UnifiedRpcRequest::Task(msg) => ProtoEnvelope {
      payload: encode_json(&msg)?.into(),
      binary: Bytes::new(),
      kind: KIND_TASK,
    },
    UnifiedRpcRequest::WasmSync(msg) => ProtoEnvelope {
      payload: encode_json(&msg)?.into(),
      binary: Bytes::new(),
      kind: KIND_WASM_SYNC,
    },
  };
  Ok(envelope.encode_to_vec())
}

fn decode_unified_request(envelope: ProtoEnvelope) -> io::Result<UnifiedRpcRequest> {
  match envelope.kind {
    KIND_RAFT => {
      let mut request: RaftRpcRequest = decode_json(&envelope.payload)?;
      attach_snapshot_binary(&mut request, &envelope.binary)?;
      Ok(UnifiedRpcRequest::Raft(request))
    }
    KIND_KV => Ok(UnifiedRpcRequest::Kv(
      Message::decode(envelope.payload).map_err(invalid_data)?,
    )),
    KIND_SQLITE_SYNC => Ok(UnifiedRpcRequest::SqliteSync(decode_json(
      &envelope.payload,
    )?)),
    KIND_TASK => Ok(UnifiedRpcRequest::Task(decode_json(&envelope.payload)?)),
    KIND_WASM_SYNC => Ok(UnifiedRpcRequest::WasmSync(decode_json(&envelope.payload)?)),
    other => Err(invalid_data(format!("unknown rpc request kind: {other}"))),
  }
}

fn encode_unified_response(resp: UnifiedRpcResponse) -> io::Result<Vec<u8>> {
  let envelope = match resp {
    UnifiedRpcResponse::Raft(raft) => ProtoEnvelope {
      payload: encode_json(&raft)?.into(),
      binary: Bytes::new(),
      kind: KIND_RAFT,
    },
    UnifiedRpcResponse::Kv(kv) => ProtoEnvelope {
      payload: kv.encode_to_vec().into(),
      binary: Bytes::new(),
      kind: KIND_KV,
    },
    UnifiedRpcResponse::SqliteSync(msg) => ProtoEnvelope {
      payload: encode_json(&msg)?.into(),
      binary: Bytes::new(),
      kind: KIND_SQLITE_SYNC,
    },
    UnifiedRpcResponse::Task(msg) => ProtoEnvelope {
      payload: encode_json(&msg)?.into(),
      binary: Bytes::new(),
      kind: KIND_TASK,
    },
    UnifiedRpcResponse::WasmSync(mut msg) => {
      // Wasm chunk bytes bypass the JSON payload the same way raft
      // snapshot data does: pulled out of the response, zstd-compressed,
      // carried in the envelope's binary frame.
      let chunk_data = match &mut msg {
        crate::wasm_sync::WasmSyncResponse::Chunk { data, .. } => std::mem::take(data),
        _ => Vec::new(),
      };
      let binary = if chunk_data.is_empty() {
        Bytes::new()
      } else {
        Bytes::from(
          zstd::stream::encode_all(chunk_data.as_slice(), SNAPSHOT_ZSTD_LEVEL)
            .map_err(invalid_data)?,
        )
      };
      ProtoEnvelope {
        payload: encode_json(&msg)?.into(),
        binary,
        kind: KIND_WASM_SYNC,
      }
    }
  };
  Ok(envelope.encode_to_vec())
}

fn decode_unified_response(envelope: ProtoEnvelope) -> io::Result<UnifiedRpcResponse> {
  match envelope.kind {
    KIND_RAFT => Ok(UnifiedRpcResponse::Raft(decode_json(&envelope.payload)?)),
    KIND_KV => Ok(UnifiedRpcResponse::Kv(
      Message::decode(envelope.payload).map_err(invalid_data)?,
    )),
    KIND_SQLITE_SYNC => Ok(UnifiedRpcResponse::SqliteSync(decode_json(
      &envelope.payload,
    )?)),
    KIND_TASK => Ok(UnifiedRpcResponse::Task(decode_json(&envelope.payload)?)),
    KIND_WASM_SYNC => {
      let mut response: crate::wasm_sync::WasmSyncResponse = decode_json(&envelope.payload)?;
      if !envelope.binary.is_empty() {
        let crate::wasm_sync::WasmSyncResponse::Chunk { data, .. } = &mut response else {
          return Err(invalid_data(
            "binary frame on a non-chunk wasm sync response",
          ));
        };
        *data = zstd::stream::decode_all(&envelope.binary[..]).map_err(invalid_data)?;
      }
      Ok(UnifiedRpcResponse::WasmSync(response))
    }
    other => Err(invalid_data(format!("unknown rpc response kind: {other}"))),
  }
}

/// Reattach an out-of-band snapshot frame to the decoded raft request.
fn attach_snapshot_binary(request: &mut RaftRpcRequest, binary: &[u8]) -> io::Result<()> {
  if binary.is_empty() {
    return Ok(());
  }
  let RaftRpcOp::FullSnapshot { data, .. } = &mut request.op else {
    return Err(invalid_data("binary frame on a non-snapshot raft request"));
  };
  *data = zstd::stream::decode_all(binary).map_err(invalid_data)?;
  Ok(())
}

async fn read_envelope<T>(io: &mut T, limit: u64) -> io::Result<ProtoEnvelope>
where
  T: AsyncRead + Unpin + Send,
{
  let mut buf = Vec::new();
  io.take(limit).read_to_end(&mut buf).await?;
  ProtoEnvelope::decode(Bytes::from(buf)).map_err(invalid_data)
}

async fn write_encoded<T>(io: &mut T, data: io::Result<Vec<u8>>) -> io::Result<()>
where
  T: AsyncWrite + Unpin + Send,
{
  let data = data?;
  io.write_all(data.as_ref()).await?;
  Ok(())
}

fn encode_json<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
  sonic_rs::to_vec(value).map_err(invalid_data)
}

fn decode_json<T: DeserializeOwned>(bytes: &[u8]) -> io::Result<T> {
  sonic_rs::from_slice(bytes).map_err(invalid_data)
}

fn invalid_data<E>(err: E) -> io::Error
where
  E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
  io::Error::new(io::ErrorKind::InvalidData, err)
}

#[cfg(test)]
mod tests {
  use futures::io::Cursor;
  use libp2p::request_response::Codec;

  use super::*;
  use crate::{
    NodeId,
    network::rpc::RaftRpcResponse,
    proto::raft_kv::{GetValueRequest, RaftKvRequest, raft_kv_request::Op as KvRequestOp},
    typ::{SnapshotMeta, Vote},
  };

  fn protocol() -> StreamProtocol {
    StreamProtocol::new("/test/rpc/2")
  }

  async fn roundtrip_request(req: UnifiedRpcRequest) -> (usize, UnifiedRpcRequest) {
    let mut codec = UnifiedCodec::default();
    let mut buf = Cursor::new(Vec::new());
    codec
      .write_request(&protocol(), &mut buf, req)
      .await
      .expect("write rpc request");
    let encoded = buf.into_inner();
    let encoded_len = encoded.len();
    let mut read = Cursor::new(encoded);
    let decoded = codec
      .read_request(&protocol(), &mut read)
      .await
      .expect("read rpc request");
    (encoded_len, decoded)
  }

  #[tokio::test]
  async fn full_snapshot_roundtrips_out_of_band() {
    let data = b"openraft-snapshot-payload-".repeat(50_000);
    let req = UnifiedRpcRequest::Raft(RaftRpcRequest {
      group_id: "users".to_string(),
      op: RaftRpcOp::FullSnapshot {
        vote: Vote::new(1, NodeId::from("node-a")),
        meta: SnapshotMeta::default(),
        data: data.clone(),
      },
    });

    let (encoded_len, decoded) = roundtrip_request(req).await;
    // The snapshot bytes travel lz4-compressed outside the JSON payload, so
    // the wire size must be well under the raw snapshot size (a JSON number
    // array would be ~4x larger instead).
    assert!(
      encoded_len < data.len() / 2,
      "expected compressed out-of-band frame: encoded={encoded_len}, raw={}",
      data.len()
    );

    let UnifiedRpcRequest::Raft(decoded) = decoded else {
      panic!("expected raft request");
    };
    assert_eq!(decoded.group_id, "users");
    match decoded.op {
      RaftRpcOp::FullSnapshot {
        data: decoded_data, ..
      } => assert_eq!(decoded_data, data),
      other => panic!("expected FullSnapshot, got {other:?}"),
    }
  }

  #[tokio::test]
  async fn empty_snapshot_roundtrips() {
    let req = UnifiedRpcRequest::Raft(RaftRpcRequest {
      group_id: "orders".to_string(),
      op: RaftRpcOp::FullSnapshot {
        vote: Vote::new(2, NodeId::from("node-b")),
        meta: SnapshotMeta::default(),
        data: Vec::new(),
      },
    });

    let (_, decoded) = roundtrip_request(req).await;
    match decoded {
      UnifiedRpcRequest::Raft(RaftRpcRequest {
        op: RaftRpcOp::FullSnapshot { data, .. },
        ..
      }) => assert!(data.is_empty()),
      other => panic!("expected FullSnapshot, got {other:?}"),
    }
  }

  #[tokio::test]
  async fn raft_metrics_request_roundtrips() {
    let req = UnifiedRpcRequest::Raft(RaftRpcRequest {
      group_id: "products".to_string(),
      op: RaftRpcOp::GetMetrics,
    });

    let (_, decoded) = roundtrip_request(req).await;
    match decoded {
      UnifiedRpcRequest::Raft(request) => {
        assert_eq!(request.group_id, "products");
        assert!(matches!(request.op, RaftRpcOp::GetMetrics));
      }
      other => panic!("expected Raft, got {other:?}"),
    }
  }

  #[tokio::test]
  async fn kv_request_roundtrips_as_protobuf() {
    let req = UnifiedRpcRequest::Kv(RaftKvRequest {
      group_id: "users".to_string(),
      op: Some(KvRequestOp::Get(GetValueRequest {
        key: "alpha".to_string(),
      })),
    });

    let (_, decoded) = roundtrip_request(req).await;
    match decoded {
      UnifiedRpcRequest::Kv(request) => {
        assert_eq!(request.group_id, "users");
        match request.op {
          Some(KvRequestOp::Get(get)) => assert_eq!(get.key, "alpha"),
          other => panic!("expected Get, got {other:?}"),
        }
      }
      other => panic!("expected Kv, got {other:?}"),
    }
  }

  #[tokio::test]
  async fn wasm_sync_chunk_roundtrips_out_of_band() {
    let mut codec = UnifiedCodec::default();
    let data = b"wasm-module-chunk-".repeat(10_000);
    let resp = UnifiedRpcResponse::WasmSync(crate::wasm_sync::WasmSyncResponse::Chunk {
      sha256: "a".repeat(64),
      index: 3,
      data: data.clone(),
    });
    let mut buf = Cursor::new(Vec::new());
    codec
      .write_response(&protocol(), &mut buf, resp)
      .await
      .expect("write wasm sync response");
    let encoded = buf.into_inner();
    // Chunk bytes must travel compressed outside the JSON payload, not as a
    // JSON number array.
    assert!(encoded.len() < data.len() / 2);
    let mut read = Cursor::new(encoded);
    let decoded = codec
      .read_response(&protocol(), &mut read)
      .await
      .expect("read wasm sync response");
    match decoded {
      UnifiedRpcResponse::WasmSync(crate::wasm_sync::WasmSyncResponse::Chunk {
        sha256,
        index,
        data: decoded_data,
      }) => {
        assert_eq!(sha256, "a".repeat(64));
        assert_eq!(index, 3);
        assert_eq!(decoded_data, data);
      }
      other => panic!("expected wasm sync chunk, got {other:?}"),
    }

    // Requests and non-chunk responses stay pure JSON.
    let req = UnifiedRpcRequest::WasmSync(crate::wasm_sync::WasmSyncRequest::Chunk {
      sha256: "b".repeat(64),
      index: 0,
    });
    let (_, decoded) = roundtrip_request(req).await;
    match decoded {
      UnifiedRpcRequest::WasmSync(crate::wasm_sync::WasmSyncRequest::Chunk { sha256, index }) => {
        assert_eq!(sha256, "b".repeat(64));
        assert_eq!(index, 0);
      }
      other => panic!("expected wasm sync chunk request, got {other:?}"),
    }
  }

  #[tokio::test]
  async fn response_roundtrips() {
    let mut codec = UnifiedCodec::default();
    let resp = UnifiedRpcResponse::Raft(RaftRpcResponse::Error("boom".to_string()));
    let mut buf = Cursor::new(Vec::new());
    codec
      .write_response(&protocol(), &mut buf, resp)
      .await
      .expect("write rpc response");
    let mut read = Cursor::new(buf.into_inner());
    let decoded = codec
      .read_response(&protocol(), &mut read)
      .await
      .expect("read rpc response");
    match decoded {
      UnifiedRpcResponse::Raft(RaftRpcResponse::Error(message)) => assert_eq!(message, "boom"),
      other => panic!("expected raft Error, got {other:?}"),
    }
  }
}

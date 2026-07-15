use std::{io, marker::PhantomData};

use bytes::Bytes;
use futures::prelude::*;
use libp2p::StreamProtocol;
use prost::Message;
use serde::{Serialize, de::DeserializeOwned};

use crate::network::rpc::{RaftRpcOp, RaftRpcRequest, RaftRpcResponse};

const DEFAULT_REQUEST_MAX: u64 = 1024 * 1024;
const DEFAULT_RESPONSE_MAX: u64 = 10 * 1024 * 1024;
/// Raft requests carry full snapshots as an out-of-band binary frame, so the
/// raft protocol accepts far larger requests than the generic default.
const RAFT_REQUEST_MAX: u64 = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct ProtoCodec {
  request_size_maximum: u64,
  response_size_maximum: u64,
}

impl Default for ProtoCodec {
  fn default() -> Self {
    Self {
      request_size_maximum: RAFT_REQUEST_MAX,
      response_size_maximum: DEFAULT_RESPONSE_MAX,
    }
  }
}

impl ProtoCodec {
  pub fn set_request_size_maximum(mut self, request_size_maximum: u64) -> Self {
    self.request_size_maximum = request_size_maximum;
    self
  }

  pub fn set_response_size_maximum(mut self, response_size_maximum: u64) -> Self {
    self.response_size_maximum = response_size_maximum;
    self
  }
}

pub struct SerdeCodec<Req, Resp> {
  request_size_maximum: u64,
  response_size_maximum: u64,
  _marker: PhantomData<(Req, Resp)>,
}

impl<Req, Resp> Clone for SerdeCodec<Req, Resp> {
  fn clone(&self) -> Self {
    Self {
      request_size_maximum: self.request_size_maximum,
      response_size_maximum: self.response_size_maximum,
      _marker: PhantomData,
    }
  }
}

impl<Req, Resp> Default for SerdeCodec<Req, Resp> {
  fn default() -> Self {
    Self {
      request_size_maximum: DEFAULT_REQUEST_MAX,
      response_size_maximum: DEFAULT_RESPONSE_MAX,
      _marker: PhantomData,
    }
  }
}

impl<Req, Resp> SerdeCodec<Req, Resp> {
  pub fn set_request_size_maximum(mut self, request_size_maximum: u64) -> Self {
    self.request_size_maximum = request_size_maximum;
    self
  }

  pub fn set_response_size_maximum(mut self, response_size_maximum: u64) -> Self {
    self.response_size_maximum = response_size_maximum;
    self
  }
}

impl<Req, Resp> libp2p::request_response::Codec for SerdeCodec<Req, Resp>
where
  Req: Serialize + DeserializeOwned + Send,
  Resp: Serialize + DeserializeOwned + Send,
{
  type Protocol = StreamProtocol;
  type Request = Req;
  type Response = Resp;

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
      decode_payload(&envelope.payload)
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
      decode_payload(&envelope.payload)
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
    let data = encode_envelope(&req);
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
    let data = encode_envelope(&resp);
    async move { write_encoded(io, data).await }
  }
}

/// Wire framing for all envelope-based protocols. Decoded from a `Bytes`
/// buffer, so both fields are zero-copy refcounted slices of the read buffer
/// rather than fresh allocations.
#[derive(Clone, PartialEq, Message)]
struct ProtoEnvelope {
  /// sonic-rs JSON encoding of the RPC value.
  #[prost(bytes = "bytes", tag = "1")]
  payload: Bytes,
  /// Out-of-band binary frame (lz4, size-prepended). Raft full-snapshot data
  /// travels here so megabytes of snapshot bytes never pass through the JSON
  /// encoder (which would turn them into a JSON number array, ~4x the size
  /// plus two extra full copies).
  #[prost(bytes = "bytes", tag = "2")]
  binary: Bytes,
}

impl libp2p::request_response::Codec for ProtoCodec {
  type Protocol = StreamProtocol;
  type Request = RaftRpcRequest;
  type Response = RaftRpcResponse;

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
      let mut request: RaftRpcRequest = decode_payload(&envelope.payload)?;
      attach_snapshot_binary(&mut request, &envelope.binary)?;
      Ok(request)
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
      decode_payload(&envelope.payload)
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
    let data = encode_raft_request(req);
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
    let data = encode_envelope(&resp);
    async move { write_encoded(io, data).await }
  }
}

/// Encode a raft request, routing full-snapshot data around the JSON payload:
/// the snapshot bytes are pulled out of the op, lz4-compressed, and carried
/// in the envelope's binary frame. All other ops encode as plain JSON.
fn encode_raft_request(mut req: RaftRpcRequest) -> io::Result<Vec<u8>> {
  let snapshot_data = match &mut req.op {
    RaftRpcOp::FullSnapshot { data, .. } => std::mem::take(data),
    _ => Vec::new(),
  };
  let payload = encode_payload(&req)?;
  let binary = if snapshot_data.is_empty() {
    Bytes::new()
  } else {
    Bytes::from(lz4_flex::compress_prepend_size(&snapshot_data))
  };
  let envelope = ProtoEnvelope {
    payload: payload.into(),
    binary,
  };
  Ok(envelope.encode_to_vec())
}

/// Reattach an out-of-band snapshot frame to the decoded request.
fn attach_snapshot_binary(request: &mut RaftRpcRequest, binary: &[u8]) -> io::Result<()> {
  if binary.is_empty() {
    return Ok(());
  }
  let RaftRpcOp::FullSnapshot { data, .. } = &mut request.op else {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "binary frame on a non-snapshot raft request",
    ));
  };
  *data = lz4_flex::decompress_size_prepended(binary)
    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
  Ok(())
}

async fn read_envelope<T>(io: &mut T, limit: u64) -> io::Result<ProtoEnvelope>
where
  T: AsyncRead + Unpin + Send,
{
  let mut buf = Vec::new();
  io.take(limit).read_to_end(&mut buf).await?;
  ProtoEnvelope::decode(Bytes::from(buf))
    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

fn encode_envelope<V>(value: &V) -> io::Result<Vec<u8>>
where
  V: Serialize,
{
  let payload = encode_payload(value)?;
  let envelope = ProtoEnvelope {
    payload: payload.into(),
    binary: Bytes::new(),
  };
  Ok(envelope.encode_to_vec())
}

async fn write_encoded<T>(io: &mut T, data: io::Result<Vec<u8>>) -> io::Result<()>
where
  T: AsyncWrite + Unpin + Send,
{
  let data = data?;
  io.write_all(data.as_ref()).await?;
  Ok(())
}

fn encode_payload<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
  sonic_rs::to_vec(value).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

fn decode_payload<T: DeserializeOwned>(bytes: &[u8]) -> io::Result<T> {
  sonic_rs::from_slice(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

#[derive(Clone)]
pub struct ProstCodec<Req, Resp> {
  request_size_maximum: u64,
  response_size_maximum: u64,
  _marker: PhantomData<(Req, Resp)>,
}

impl<Req, Resp> Default for ProstCodec<Req, Resp> {
  fn default() -> Self {
    Self {
      request_size_maximum: DEFAULT_REQUEST_MAX,
      response_size_maximum: DEFAULT_RESPONSE_MAX,
      _marker: PhantomData,
    }
  }
}

impl<Req, Resp> ProstCodec<Req, Resp> {
  pub fn set_request_size_maximum(mut self, request_size_maximum: u64) -> Self {
    self.request_size_maximum = request_size_maximum;
    self
  }

  pub fn set_response_size_maximum(mut self, response_size_maximum: u64) -> Self {
    self.response_size_maximum = response_size_maximum;
    self
  }
}

impl<Req, Resp> libp2p::request_response::Codec for ProstCodec<Req, Resp>
where
  Req: Message + Default + Send,
  Resp: Message + Default + Send,
{
  type Protocol = StreamProtocol;
  type Request = Req;
  type Response = Resp;

  fn read_request<T>(
    &mut self,
    _: &Self::Protocol,
    io: &mut T,
  ) -> impl Future<Output = io::Result<Self::Request>> + Send
  where
    T: AsyncRead + Unpin + Send,
  {
    let limit = self.request_size_maximum;
    async move { read_message(io, limit).await }
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
    async move { read_message(io, limit).await }
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
    let data = Ok(req.encode_to_vec());
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
    let data = Ok(resp.encode_to_vec());
    async move { write_encoded(io, data).await }
  }
}

async fn read_message<T, M>(io: &mut T, limit: u64) -> io::Result<M>
where
  T: AsyncRead + Unpin + Send,
  M: Message + Default,
{
  let mut buf = Vec::new();
  io.take(limit).read_to_end(&mut buf).await?;
  M::decode(buf.as_slice()).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

#[cfg(test)]
mod tests {
  use futures::io::Cursor;
  use libp2p::request_response::Codec;

  use super::*;
  use crate::{
    NodeId,
    typ::{SnapshotMeta, Vote},
  };

  fn protocol() -> StreamProtocol {
    StreamProtocol::new("/test/raft/1")
  }

  async fn roundtrip_request(req: RaftRpcRequest) -> (usize, RaftRpcRequest) {
    let mut codec = ProtoCodec::default();
    let mut buf = Cursor::new(Vec::new());
    codec
      .write_request(&protocol(), &mut buf, req)
      .await
      .expect("write raft request");
    let encoded = buf.into_inner();
    let encoded_len = encoded.len();
    let mut read = Cursor::new(encoded);
    let decoded = codec
      .read_request(&protocol(), &mut read)
      .await
      .expect("read raft request");
    (encoded_len, decoded)
  }

  #[tokio::test]
  async fn full_snapshot_roundtrips_out_of_band() {
    let data = b"openraft-snapshot-payload-".repeat(50_000);
    let req = RaftRpcRequest {
      group_id: "users".to_string(),
      op: RaftRpcOp::FullSnapshot {
        vote: Vote::new(1, NodeId::from("node-a")),
        meta: SnapshotMeta::default(),
        data: data.clone(),
      },
    };

    let (encoded_len, decoded) = roundtrip_request(req).await;
    // The snapshot bytes travel lz4-compressed outside the JSON payload, so
    // the wire size must be well under the raw snapshot size (a JSON number
    // array would be ~4x larger instead).
    assert!(
      encoded_len < data.len() / 2,
      "expected compressed out-of-band frame: encoded={encoded_len}, raw={}",
      data.len()
    );

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
    let req = RaftRpcRequest {
      group_id: "orders".to_string(),
      op: RaftRpcOp::FullSnapshot {
        vote: Vote::new(2, NodeId::from("node-b")),
        meta: SnapshotMeta::default(),
        data: Vec::new(),
      },
    };

    let (_, decoded) = roundtrip_request(req).await;
    match decoded.op {
      RaftRpcOp::FullSnapshot { data, .. } => assert!(data.is_empty()),
      other => panic!("expected FullSnapshot, got {other:?}"),
    }
  }

  #[tokio::test]
  async fn non_snapshot_request_roundtrips() {
    let req = RaftRpcRequest {
      group_id: "products".to_string(),
      op: RaftRpcOp::GetMetrics,
    };

    let (_, decoded) = roundtrip_request(req).await;
    assert_eq!(decoded.group_id, "products");
    assert!(matches!(decoded.op, RaftRpcOp::GetMetrics));
  }

  #[tokio::test]
  async fn response_roundtrips() {
    let mut codec = ProtoCodec::default();
    let resp = RaftRpcResponse::Error("boom".to_string());
    let mut buf = Cursor::new(Vec::new());
    codec
      .write_response(&protocol(), &mut buf, resp)
      .await
      .expect("write raft response");
    let mut read = Cursor::new(buf.into_inner());
    let decoded = codec
      .read_response(&protocol(), &mut read)
      .await
      .expect("read raft response");
    match decoded {
      RaftRpcResponse::Error(message) => assert_eq!(message, "boom"),
      other => panic!("expected Error, got {other:?}"),
    }
  }
}

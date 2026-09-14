use std::{io, marker::PhantomData};

use futures::prelude::*;
use libp2p::StreamProtocol;
use prost::Message;
use serde::{Serialize, de::DeserializeOwned};

use crate::network::rpc::{RaftRpcRequest, RaftRpcResponse};

const DEFAULT_REQUEST_MAX: u64 = 1024 * 1024;
const DEFAULT_RESPONSE_MAX: u64 = 10 * 1024 * 1024;

#[derive(Clone)]
pub struct ProtoCodec {
  request_size_maximum: u64,
  response_size_maximum: u64,
}

impl Default for ProtoCodec {
  fn default() -> Self {
    Self {
      request_size_maximum: DEFAULT_REQUEST_MAX,
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

#[derive(Clone, PartialEq, Message)]
struct ProtoEnvelope {
  #[prost(bytes, tag = "1")]
  payload: Vec<u8>,
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
      let payload = read_envelope(io, limit).await?;
      decode_payload(&payload)
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
      let payload = read_envelope(io, limit).await?;
      decode_payload(&payload)
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
    async move { write_envelope(io, &req).await }
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
    async move { write_envelope(io, &resp).await }
  }
}

async fn read_envelope<T>(io: &mut T, limit: u64) -> io::Result<Vec<u8>>
where
  T: AsyncRead + Unpin + Send,
{
  let mut buf = Vec::new();
  io.take(limit).read_to_end(&mut buf).await?;
  let envelope = ProtoEnvelope::decode(buf.as_slice())
    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
  Ok(envelope.payload)
}

async fn write_envelope<T, V>(io: &mut T, value: &V) -> io::Result<()>
where
  T: AsyncWrite + Unpin + Send,
  V: Serialize,
{
  let payload = encode_payload(value)?;
  let envelope = ProtoEnvelope { payload };
  let data = envelope.encode_to_vec();
  io.write_all(data.as_ref()).await?;
  Ok(())
}

fn encode_payload<T: Serialize>(value: &T) -> io::Result<Vec<u8>> {
  serde_json::to_vec(value).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

fn decode_payload<T: DeserializeOwned>(bytes: &[u8]) -> io::Result<T> {
  serde_json::from_slice(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
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
    async move { write_message(io, &req).await }
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
    async move { write_message(io, &resp).await }
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

async fn write_message<T, M>(io: &mut T, message: &M) -> io::Result<()>
where
  T: AsyncWrite + Unpin + Send,
  M: Message,
{
  let data = message.encode_to_vec();
  io.write_all(data.as_ref()).await?;
  Ok(())
}

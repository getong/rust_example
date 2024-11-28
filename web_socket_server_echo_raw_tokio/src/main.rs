use std::borrow::BorrowMut;

use tokio::net::TcpListener;
use wtx::{
  web_socket::{
    handshake::{WebSocketAccept, WebSocketAcceptRaw},
    FrameBufferVec, OpCode, WebSocketServer,
  },
  ReadBuffer, Stream,
};
pub(crate) fn _host_from_args() -> String {
  std::env::args()
    .nth(1)
    .unwrap_or_else(|| "127.0.0.1:8080".to_owned())
}

pub(crate) async fn _accept_conn_and_echo_frames(
  fb: &mut FrameBufferVec,
  rb: &mut ReadBuffer,
  stream: impl Send + Stream + Sync,
) -> wtx::Result<()> {
  let (_, mut ws) = WebSocketAcceptRaw {
    fb,
    headers_buffer: &mut <_>::default(),
    key_buffer: &mut <_>::default(),
    rb,
    stream,
  }
  .accept()
  .await?;
  _handle_frames(fb, &mut ws).await?;
  Ok(())
}

pub(crate) async fn _handle_frames<RB>(
  fb: &mut FrameBufferVec,
  ws: &mut WebSocketServer<RB, impl Stream>,
) -> wtx::Result<()>
where
  RB: BorrowMut<ReadBuffer>,
{
  loop {
    let mut frame = ws.read_msg(fb).await?;
    match frame.op_code() {
      OpCode::Binary | OpCode::Text => {
        ws.write_frame(&mut frame).await?;
      }
      OpCode::Close => break,
      _ => {}
    }
  }
  Ok(())
}

#[tokio::main]
async fn main() -> wtx::Result<()> {
  let listener = TcpListener::bind(_host_from_args()).await?;
  loop {
    let (stream, _) = listener.accept().await?;
    let _jh = tokio::spawn(async move {
      if let Err(err) = tokio::task::unconstrained(_accept_conn_and_echo_frames(
        &mut <_>::default(),
        &mut <_>::default(),
        stream,
      ))
      .await
      {
        println!("{err}");
      }
    });
  }
}

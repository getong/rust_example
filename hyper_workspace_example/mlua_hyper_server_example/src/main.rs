use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};
use hyper::{service::Service, body::Incoming, Request, Response};
use hyper_util::{rt::TokioExecutor, rt::TokioIo, server::conn::auto};
use std::{future::Future, net::SocketAddr, pin::Pin, sync::Arc};
use tokio::net::TcpListener;

use mlua::{
  chunk, Error as LuaError, Function, Lua, String as LuaString, Table, UserData, UserDataMethods,
};


struct LuaRequest(SocketAddr, Request<Incoming>);

impl UserData for LuaRequest {
  fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
    methods.add_method("remote_addr", |_lua, req, ()| Ok((req.0).to_string()));
    methods.add_method("method", |_lua, req, ()| Ok((req.1).method().to_string()));
  }
}

#[derive(Debug, Clone)]
pub struct Svc(Arc<Lua>, Arc<SocketAddr>);

impl Service<Request<Incoming>> for Svc {
  type Response = Response<BoxBody<Bytes, hyper::Error>>;
  type Error = LuaError;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

  fn call(&self, req: Request<Incoming>) -> Self::Future {
    // If handler returns an error then generate 5xx response
    let lua_clone = self.0.clone();
    let peer_addr = self.1.clone();
    let lua_req = LuaRequest(*peer_addr, req);
    Box::pin(async move {
      let handler: Function = lua_clone.named_registry_value("http_handler")?;
      match handler.call_async::<_, Table>(lua_req).await {
        Ok(lua_resp) => {
          let status = lua_resp.get::<_, Option<u16>>("status")?.unwrap_or(200);
          let mut resp = Response::builder().status(status);

          // Set headers
          if let Some(headers) = lua_resp.get::<_, Option<Table>>("headers")? {
            for pair in headers.pairs::<String, LuaString>() {
              let (h, v) = pair?;
              resp = resp.header(&h, v.as_bytes());
            }
          }

          let body = lua_resp
            .get::<_, Option<LuaString>>("body")?
            .map(|b| {
              // Full::new(Bytes::from(b.clone().as_bytes()))
                Full::new(Bytes::copy_from_slice(b.clone().as_bytes()))
                .map_err(|never| match never {})
                .boxed()
            })
            .unwrap_or_else(|| {
              Empty::<Bytes>::new()
                .map_err(|never| match never {})
                .boxed()
            });

          Ok(resp.body(body).unwrap())
        }
        Err(err) => {
          eprintln!("{}", err);
          Ok(
            Response::builder()
              .status(500)
              .body(Full::new(Bytes::from("Internal Server Error".as_bytes()))
                    .map_err(|never| match never {})
                    .boxed())
              .unwrap(),
          )
        }
      }
    })
  }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
  let lua = Arc::new(Lua::new());

  // Create Lua handler function
  let handler: Function = lua
    .load(chunk! {
        function(req)
            return {
                status = 200,
                headers = {
                    ["X-Req-Method"] = req:method(),
                    ["X-Remote-Addr"] = req:remote_addr(),
                },
                body = "Hello from Lua!\n"
            }
        end
    })
    .eval()
    .expect("cannot create Lua handler");

  // Store it in the Registry
  lua
    .set_named_registry_value("http_handler", handler)
    .expect("cannot store Lua handler");

  let addr = ([127, 0, 0, 1], 3000).into();

  let listener = TcpListener::bind(addr).await.unwrap();
  loop {
    let (stream, peer_addr) = listener.accept().await.unwrap();
    let io = TokioIo::new(stream);

    let svc = Svc(lua.clone(), Arc::new(peer_addr));
    tokio::task::spawn(async move {
      if let Err(err) = auto::Builder::new(TokioExecutor::new())
        .serve_connection(io, svc)
        .await

      {
        println!("Error serving connection: {:?}", err);
      }
    });
  }
}

use std::{net::SocketAddr, path::Path};

#[volo::main]
async fn main() {
  let _addr: SocketAddr = "[::]:8080".parse().unwrap();

  // hotrestart initialize
  volo::hotrestart::DEFAULT_HOT_RESTART
    .initialize(Path::new("/tmp"), 1)
    .await
    .unwrap();

  // volo_gen::nthrift::test::idl::LearnServiceServer::new(S)
  //     .byted()
  //     .run(addr)
  //     .await
  //     .unwrap();
}

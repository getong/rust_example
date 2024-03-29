use std::{env, path::PathBuf};

fn main() {
  let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
  tonic_build::configure()
    .file_descriptor_set_path(out_dir.join("helloworld_descriptor.bin"))
    .compile(&["proto/helloworld.proto"], &["proto"])
    .unwrap();
  build_json_codec_service();
}

// Manually define the json.helloworld.Greeter service which used a custom JsonCodec to use json
// serialization instead of protobuf for sending messages on the wire.
// This will result in generated client and server code which relies on its request, response and
// codec types being defined in a module `crate::common`.
//
// See the client/server examples defined in `src/json-codec` for more information.
fn build_json_codec_service() {
  let greeter_service = tonic_build::manual::Service::builder()
    .name("Greeter")
    .package("json.helloworld")
    .method(
      tonic_build::manual::Method::builder()
        .name("say_hello")
        .route_name("SayHello")
        .input_type("crate::common::HelloRequest")
        .output_type("crate::common::HelloResponse")
        .codec_path("crate::common::JsonCodec")
        .build(),
    )
    .build();

  tonic_build::manual::Builder::new().compile(&[greeter_service]);
}

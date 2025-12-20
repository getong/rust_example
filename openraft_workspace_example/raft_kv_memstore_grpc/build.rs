fn main() -> Result<(), Box<dyn std::error::Error>> {
  println!("cargo:rerun-if-changed=src/*");
  let mut config = prost_build::Config::new();
  config.protoc_arg("--experimental_allow_proto3_optional");
  let proto_files = [
    "proto/raft.proto",
    "proto/app_types.proto",
    "proto/app.proto",
  ];

  // TODO: remove serde

  tonic_prost_build::configure()
    .btree_map(".")
    .type_attribute(
      "openraftpb.Node",
      "#[derive(serde::Serialize, serde::Deserialize)]",
    )
    .type_attribute(
      "openraftpb.SetRequest",
      "#[derive(serde::Serialize, serde::Deserialize)]",
    )
    .type_attribute(
      "openraftpb.Response",
      "#[derive(serde::Serialize, serde::Deserialize)]",
    )
    .type_attribute(
      "openraftpb.LeaderId",
      "#[derive(serde::Serialize, serde::Deserialize)]",
    )
    .type_attribute(
      "openraftpb.Vote",
      "#[derive(serde::Serialize, serde::Deserialize)]",
    )
    .type_attribute(
      "openraftpb.NodeIdSet",
      "#[derive(serde::Serialize, serde::Deserialize)]",
    )
    .type_attribute(
      "openraftpb.Membership",
      "#[derive(serde::Serialize, serde::Deserialize)]",
    )
    .type_attribute(
      "openraftpb.Entry",
      "#[derive(serde::Serialize, serde::Deserialize)]",
    )
    .compile_with_config(config, &proto_files, &["proto"])?;
  Ok(())
}

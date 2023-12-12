extern crate capnpc;

fn main() {
  ::capnpc::CompilerCommand::new()
    .src_prefix("schema") // 1
    .file("schema/point.capnp") // 2
    .run()
    .expect("compiling schema");
}

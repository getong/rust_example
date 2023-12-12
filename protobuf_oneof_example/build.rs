// use protobuf::descriptor::field_descriptor_proto::Type;
// use protobuf::reflect::FieldDescriptor;
// use protobuf::reflect::MessageDescriptor;
use protobuf_codegen::Codegen;
// use protobuf_codegen::Customize;
// use protobuf_codegen::CustomizeCallback;

fn main() {
  Codegen::new()
    .cargo_out_dir("protos")
    .include("src")
    .inputs(&["src/oneof_example.proto"])
    .run_from_script();
}

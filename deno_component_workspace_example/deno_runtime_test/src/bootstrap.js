import { example_custom_op } from "ext:core/ops";

function exampleCustomOp(str) {
  return example_custom_op(str);
}

globalThis.ExampleExtension = { exampleCustomOp };

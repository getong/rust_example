import { example_custom_op } from "ext:core/ops";

function exampleCustomOp(str) {
  return example_custom_op(str);
}

globalThis.ExampleExtension = { exampleCustomOp };

// CommonJS compatibility helper
globalThis.__createCommonJSContext = function() {
  const module = { exports: {} };
  const exports = module.exports;
  return { module, exports };
};

// Pre-import all dependencies to ensure they're registered
import "npm:is-odd@0.1.2";
import "npm:is-number@3.0.0";
import "npm:kind-of@3.2.2";

// Now import the main module
import isEven from "npm:is-even@1.0.0";

declare const ExampleExtension: {
  exampleCustomOp: (text: string) => string;
};

console.log("Testing npm import with pre-loaded dependencies");

console.log("Is 2 even?", isEven(2));
console.log("Is 3 even?", isEven(3));

const message = ExampleExtension.exampleCustomOp("npm support works!");
console.log(`Message from Rust: ${message}`);

console.log("Test completed successfully!");

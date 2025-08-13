import isEven from "npm:is-even@1.0.0";

declare const ExampleExtension: {
  exampleCustomOp: (text: string) => string;
};

console.log("Testing npm import with is-even");

console.log("Is 2 even?", isEven(2));
console.log("Is 3 even?", isEven(3));

const message = ExampleExtension.exampleCustomOp("npm support works!");
console.log(`Message from Rust: ${message}`);

console.log("Test completed successfully!");
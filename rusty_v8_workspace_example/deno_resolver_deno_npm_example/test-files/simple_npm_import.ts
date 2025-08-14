import chalk from "npm:chalk@5.3.0";

declare const ExampleExtension: {
  exampleCustomOp: (text: string) => string;
};

console.log(chalk.blue("Testing npm import with chalk"));

const message = ExampleExtension.exampleCustomOp("npm support works!");
console.log(chalk.green(`Message from Rust: ${message}`));

console.log(chalk.yellow("Test completed successfully!"));
